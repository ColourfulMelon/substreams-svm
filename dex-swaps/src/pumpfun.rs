use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpfun::bonding_curve as pumpfun;

use crate::SOL_MINT;

pub(crate) struct PendingTrade {
    bonding_curve: Vec<u8>,
}

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &pumpfun::PROGRAM_ID {
        return None;
    }

    if let Some(trade) = decode_trade_instruction(instruction) {
        *pending_trade = Some(trade);
        return None;
    }

    // Permissive trade-event decoder lives in
    // `substreams_solana_idls::pumpfun::bonding_curve` — the on-chain
    // TradeEvent has grown past the IDL's V0..V3 fixed lengths, so the strict
    // decoder rejects every current event. The minimal helper reads the
    // stable leading layout at fixed offsets and ignores trailing fields.
    let event = pumpfun::events::unpack_trade_event_minimal(instruction.data()).ok()??;
    let trade = pending_trade.take()?;

    let mint = event.mint.to_vec();
    Some(pb::Swap {
        protocol: pb::Protocol::Pumpfun as i32,
        program_id: pumpfun::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: pumpfun::PROGRAM_ID.to_vec(),
        amm_pool: trade.bonding_curve,
        user: event.user.to_vec(),
        input_mint: if event.is_buy { SOL_MINT.to_vec() } else { mint.clone() },
        input_amount: if event.is_buy { event.sol_amount } else { event.token_amount },
        output_mint: if event.is_buy { mint } else { SOL_MINT.to_vec() },
        output_amount: if event.is_buy { event.token_amount } else { event.sol_amount },
        ..Default::default()
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpfun::PROGRAM_ID {
        return None;
    }
    decode_trade_instruction(instruction).map(|t| t.bonding_curve)
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    let bonding_curve = match pumpfun::instructions::unpack(instruction.data()) {
        Ok(pumpfun::instructions::PumpFunInstruction::Buy(_)) | Ok(pumpfun::instructions::PumpFunInstruction::BuyExactSolIn(_)) => {
            pumpfun::accounts::get_buy_accounts(instruction).ok()?.bonding_curve
        }
        Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)) => pumpfun::accounts::get_sell_accounts(instruction).ok()?.bonding_curve,
        _ => return None,
    };
    Some(PendingTrade {
        bonding_curve: bonding_curve.to_bytes().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, Message, MessageHeader, Transaction, TransactionStatusMeta,
    };

    #[test]
    fn emits_bonding_curve_buys_including_exact_sol_input() {
        for discriminator in [[102, 6, 61, 18, 1, 218, 235, 234], pumpfun::instructions::BUY_EXACT_SOL_IN] {
            let mut accounts: Vec<Vec<u8>> = (0..12).map(|i| vec![i; 32]).collect();
            accounts.push(pumpfun::PROGRAM_ID.to_vec());
            let mut buy = discriminator.to_vec();
            buy.extend_from_slice(&1_000u64.to_le_bytes());
            buy.extend_from_slice(&10_000u64.to_le_bytes());
            buy.push(1); // current track_volume flag

            let mut event = vec![0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
            event.extend_from_slice(&pumpfun::events::TRADE);
            event.extend_from_slice(&accounts[2]);
            event.extend_from_slice(&1_000u64.to_le_bytes());
            event.extend_from_slice(&10_000u64.to_le_bytes());
            event.push(1);
            event.extend_from_slice(&accounts[6]);

            let tx = ConfirmedTransaction {
                transaction: Some(Transaction {
                    message: Some(Message {
                        header: Some(MessageHeader {
                            num_required_signatures: 1,
                            ..Default::default()
                        }),
                        account_keys: accounts,
                        instructions: vec![
                            CompiledInstruction {
                                program_id_index: 12,
                                accounts: (0..12).collect(),
                                data: buy,
                            },
                            CompiledInstruction {
                                program_id_index: 12,
                                accounts: vec![12],
                                data: event,
                            },
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                meta: Some(TransactionStatusMeta::default()),
            };
            let mut pending = None;
            let swaps: Vec<_> = tx.walk_instructions().filter_map(|ix| handle_instruction(&mut pending, &ix)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!(swaps[0].amm_pool, vec![3; 32]);
            assert_eq!(swaps[0].input_mint, SOL_MINT.to_vec());
            assert_eq!(swaps[0].output_mint, vec![2; 32]);
            assert_eq!(swaps[0].input_amount, 1_000);
            assert_eq!(swaps[0].output_amount, 10_000);
        }
    }
}
