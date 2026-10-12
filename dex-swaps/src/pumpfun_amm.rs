use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpswap;
use substreams_solana_idls::pumpswap::accounts::TradeTokenAccounts;
use substreams_solana_idls::pumpswap::events::TradeDirection;

pub(crate) struct PendingTrade {
    pool: Vec<u8>,
    base_mint: Vec<u8>,
    quote_mint: Vec<u8>,
}

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpswap::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    // Permissive trade-event decoder lives in `substreams_solana_idls::pumpswap`
    // — read the comment there for the rationale (pump.fun has appended fields
    // to BuyEvent/SellEvent multiple times since launch and we don't want each
    // sink chasing the IDL).
    let event = pumpswap::events::unpack_trade_event_minimal(instruction.data()).ok()??;
    let trade = pending_trade.take()?;

    let is_buy = matches!(event.direction, TradeDirection::Buy);
    let (input_amount, output_amount) = if is_buy {
        (event.quote_amount, event.base_amount)
    } else {
        (event.base_amount, event.quote_amount)
    };

    Some(pb::Swap {
        protocol: pb::Protocol::PumpfunAmm as i32,
        program_id: pumpswap::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: pumpswap::PROGRAM_ID.to_vec(),
        amm_pool: trade.pool,
        user: event.user.to_vec(),
        input_mint: if is_buy { trade.quote_mint.clone() } else { trade.base_mint.clone() },
        input_amount,
        output_mint: if is_buy { trade.base_mint } else { trade.quote_mint },
        output_amount,
        ..Default::default()
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpswap::PROGRAM_ID {
        return None;
    }
    // `routed_pool::Tracker::observe` only needs the pool address (accounts[0])
    // for cross-protocol routing attribution. We deliberately don't run the
    // full strict `TradeTokenAccounts::try_from` here — pre-filter inner CPI
    // fixtures may carry only the pool slot, and the strict path would reject
    // them with `Missing` for trailing accounts.
    if !is_trade_instruction(instruction) {
        return None;
    }
    instruction.accounts().get(0).map(|a| a.0.to_vec())
}

fn is_trade_instruction(instruction: &InstructionView) -> bool {
    let data = instruction.data();
    // Finalized legacy sells also carry the same one/two boolean suffixes
    // already accepted for buys. Only normalize this exact discriminator,
    // bounded length and valid booleans; accounts and receipts stay typed.
    let data = if data.starts_with(&pumpswap::instructions::SELL)
        && matches!(data.len(), 25 | 26) && data[24..].iter().all(|flag| *flag <= 1)
    { &data[..24] } else { data.as_slice() };
    matches!(
        pumpswap::instructions::unpack(data),
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::BuyV2(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteInV2(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::SellV2(_))
    )
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    let data = instruction.data();
    // Finalized legacy sells also carry the same one/two boolean suffixes
    // already accepted for buys. Only normalize this exact discriminator,
    // bounded length and valid booleans; accounts and receipts stay typed.
    let data = if data.starts_with(&pumpswap::instructions::SELL)
        && matches!(data.len(), 25 | 26) && data[24..].iter().all(|flag| *flag <= 1)
    { &data[..24] } else { data.as_slice() };
    match pumpswap::instructions::unpack(data) {
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyV2(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteInV2(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::SellV2(_)) => {
            // IDL-canonical account layout — see the comment + IDX_* constants
            // in `substreams_solana_idls::pumpswap::accounts`. Hand-indexing
            // `accounts.get(N)` here previously caused an off-by-one
            // (`base_mint` = accounts[4], should have been [3]) which let the
            // user's *base token account* leak into the `mint` slot, producing
            // ~165 spurious `(mint0, mint1)` pairs per pool downstream.
            let accounts = TradeTokenAccounts::try_from(instruction).ok()?;
            Some(PendingTrade {
                pool: accounts.pool.to_bytes().to_vec(),
                base_mint: accounts.base_mint.to_bytes().to_vec(),
                quote_mint: accounts.quote_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, Message, MessageHeader, Transaction, TransactionStatusMeta,
    };

    #[test]
    fn finalized_sell_boolean_suffixes_preserve_exact_principal_receipts() {
        let fixtures: &[(&[u8], u32, u32, u64, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/0.transaction.pb").as_slice(), 6, 8, 1054011869465, 738636214),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/1.transaction.pb").as_slice(), 1, 3, 30000, 35368891),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/2.transaction.pb").as_slice(), 4, 6, 3526053355674, 1398305543),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/3.transaction.pb").as_slice(), 3, 5, 414776809974, 724043666),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/4.transaction.pb").as_slice(), 5, 7, 30000, 328177372),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/5.transaction.pb").as_slice(), 5, 7, 30000, 65169439),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/6.transaction.pb").as_slice(), 5, 7, 30000, 335755796),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/7.transaction.pb").as_slice(), 4, 6, 197072404302, 62942423),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/8.transaction.pb").as_slice(), 1, 3, 30000, 75798865),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/9.transaction.pb").as_slice(), 7, 9, 980187577377, 584456716),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/10.transaction.pb").as_slice(), 6, 8, 951168762158, 783003378),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/11.transaction.pb").as_slice(), 1, 3, 30000, 75831409),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/12.transaction.pb").as_slice(), 4, 6, 1060087878201, 482014443),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/13.transaction.pb").as_slice(), 7, 9, 984687619076, 595460134),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/14.transaction.pb").as_slice(), 6, 8, 950289115848, 718834932),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/15.transaction.pb").as_slice(), 6, 8, 873156127824, 736846889),
        ];
        for &(bytes, source, first, input, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let ix = tx.walk_instructions().nth(source as usize).unwrap();
            let original_data = ix.data().clone();
            let a = ix.accounts();
            let (pool, user, input_mint, output_mint) = (a[0].0.clone(), a[1].0.clone(), a[3].0.clone(), a[4].0.clone());
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(source)).collect();
            assert_eq!(swaps.len(), 1, "one physical sell with separately paid fees");
            let sell = swaps[0];
            assert_eq!((sell.input_amount, sell.output_amount, sell.source_transfer_index), (input, output, Some(first)));
            assert_eq!((sell.amm_pool.clone(), sell.user.clone(), sell.input_mint.clone(), sell.output_mint.clone()),
                (pool, user, input_mint, output_mint));
            assert_eq!(sell.protocol, pb::Protocol::PumpfunAmm as i32);
            assert!(sell.transfer_verified);
            for mode in 0..6 {
                let mut altered = tx.clone();
                let message = altered.transaction.as_mut().unwrap().message.as_mut().unwrap();
                let native = if let Some(position) = message.instructions.iter().position(|ix| ix.data == original_data) {
                    &mut message.instructions[position].data
                } else {
                    &mut altered.meta.as_mut().unwrap().inner_instructions.iter_mut()
                        .flat_map(|group| &mut group.instructions).find(|ix| ix.data == original_data).unwrap().data
                };
                match mode {
                    0 => native[24] = 2,
                    1 => { native.resize(26, 0); native[25] = 2; }
                    2 => native.resize(27, 0),
                    3 => native.truncate(23),
                    4 => native[0] ^= 1,
                    5 => {
                        let program = substreams_solana::base58::encode(pumpswap::PROGRAM_ID);
                        for log in &mut altered.meta.as_mut().unwrap().log_messages {
                            if log.as_str() == format!("Program {program} success") {
                                *log = format!("Program {program} failed: custom program error: 0x1");
                            }
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(crate::process_transaction(altered).map_or(true, |decoded|
                    decoded.swaps.iter().all(|s| s.source_index != Some(source))), "sell suffix mutation {mode}");
            }
        }
    }

    /// PumpSwap `buy` instruction discriminator from `pumpswap/idl.json`.
    const BUY_DISC: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
    /// PumpSwap `sell` instruction discriminator.
    const SELL_DISC: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

    fn buy_body() -> Vec<u8> {
        // BuyInstruction { base_amount_out: u64, max_quote_amount_in: u64 }
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b
    }

    fn sell_body() -> Vec<u8> {
        // SellInstruction { base_amount_in: u64, min_quote_amount_out: u64 }
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b
    }

    /// Build a ConfirmedTransaction with one PumpSwap instruction.
    fn make_tx(disc: [u8; 8], body: Vec<u8>, accounts: &[[u8; 32]]) -> ConfirmedTransaction {
        let fee_payer = [0xfe; 32];
        let program = pumpswap::PROGRAM_ID;
        let mut keys: Vec<Vec<u8>> = vec![fee_payer.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8);
        }
        let mut data = disc.to_vec();
        data.extend_from_slice(&body);

        ConfirmedTransaction {
            transaction: Some(Transaction {
                signatures: vec![vec![0u8; 64]],
                message: Some(Message {
                    header: Some(MessageHeader {
                        num_required_signatures: 1,
                        num_readonly_signed_accounts: 0,
                        num_readonly_unsigned_accounts: 0,
                    }),
                    account_keys: keys,
                    recent_blockhash: vec![0u8; 32],
                    instructions: vec![CompiledInstruction {
                        program_id_index: 1,
                        accounts: acc_idx,
                        data,
                    }],
                    versioned: false,
                    address_table_lookups: vec![],
                }),
            }),
            meta: Some(TransactionStatusMeta::default()),
        }
    }

    #[test]
    fn buy_decode_resolves_canonical_mints() {
        let pool = [0x01; 32];
        let user = [0x02; 32];
        let global_config = [0x03; 32];
        let base_mint = [0x04; 32];
        let quote_mint = [0x05; 32];
        let user_base_ta = [0x06; 32];
        let user_quote_ta = [0x07; 32];
        let pool_base_ta = [0x08; 32];
        let pool_quote_ta = [0x09; 32];
        let protocol_fee_recipient = [0x0a; 32];
        let protocol_fee_recipient_ta = [0x0b; 32];

        let tx = make_tx(
            BUY_DISC,
            buy_body(),
            &[
                pool,
                user,
                global_config,
                base_mint,
                quote_mint,
                user_base_ta,
                user_quote_ta,
                pool_base_ta,
                pool_quote_ta,
                protocol_fee_recipient,
                protocol_fee_recipient_ta,
            ],
        );
        let ix = tx.walk_instructions().next().unwrap();
        let trade = decode_trade_instruction(&ix).expect("buy instruction must decode");

        assert_eq!(trade.pool, pool.to_vec());
        assert_eq!(trade.base_mint, base_mint.to_vec(), "base_mint must come from accounts[3], not [4]");
        assert_eq!(trade.quote_mint, quote_mint.to_vec(), "quote_mint must come from accounts[4], not [5]");

        // Regression: the previous adapter read accounts[4]/[5] which is
        // exactly (quote_mint, user_base_token_account). Make sure neither
        // slot leaks the user's TA back in.
        assert_ne!(trade.base_mint, user_base_ta.to_vec());
        assert_ne!(trade.quote_mint, user_base_ta.to_vec());
    }

    #[test]
    fn sell_decode_uses_same_layout_as_buy() {
        let pool = [0x21; 32];
        let user = [0x22; 32];
        let global_config = [0x23; 32];
        let base_mint = [0x24; 32];
        let quote_mint = [0x25; 32];
        let accounts: Vec<[u8; 32]> = vec![pool, user, global_config, base_mint, quote_mint]
            .into_iter()
            .chain((6u8..12u8).map(|i| [i; 32]))
            .collect();

        let tx = make_tx(SELL_DISC, sell_body(), &accounts);
        let ix = tx.walk_instructions().next().unwrap();
        let trade = decode_trade_instruction(&ix).expect("sell instruction must decode");

        assert_eq!(trade.pool, pool.to_vec());
        assert_eq!(trade.base_mint, base_mint.to_vec());
        assert_eq!(trade.quote_mint, quote_mint.to_vec());
    }
}
