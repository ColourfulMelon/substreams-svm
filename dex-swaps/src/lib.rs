mod boop;
mod byreal;
mod coverage;
mod darklake;
mod dumpfun;
mod jupiter_v4;
mod jupiter_v6;
mod logs;
mod meteora_amm;
mod meteora_daam;
mod meteora_dlmm;
mod moonshot;
mod native_venues;
mod okx;
pub mod openbook;
mod openbook_metadata;
mod orca_whirlpool;
mod pancakeswap;
pub mod phoenix;
mod phoenix_metadata;
mod pumpfun;
mod pumpfun_amm;
mod quality;
mod raydium_amm_v4;
mod raydium_clmm;
mod raydium_cpmm;
mod raydium_launchpad;
mod rfq;
mod routed_pool;
mod spl_token_swap;
pub mod token_fees;
mod token_mints;

use std::collections::VecDeque;

use common::solana::{get_fee_payer, get_signers};
use proto::pb::dex::swaps::v1 as pb;
use substreams::{
    errors::Error,
    log,
    store::{StoreGet, StoreGetRaw, StoreNew, StoreSet, StoreSetRaw},
};
use substreams_solana::{
    base58,
    pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction},
};

pub(crate) const SOL_MINT: [u8; 32] = [
    6, 155, 136, 87, 254, 171, 129, 132, 251, 104, 127, 99, 70, 24, 192, 53, 218, 196, 57, 220, 26, 235, 59, 85, 152, 160, 240, 0, 0, 0, 0, 1,
]; // So11111111111111111111111111111111111111111

#[substreams::handlers::store]
fn store_phoenix_markets(block: Block, output: StoreSetRaw) {
    for (ordinal, key, value) in phoenix_metadata::updates(&block) {
        output.set(ordinal, key, &value);
    }
}

#[substreams::handlers::store]
fn store_openbook_markets(block: Block, output: StoreSetRaw) {
    for (ordinal, key, value) in openbook_metadata::updates(&block) {
        output.set(ordinal, key, &value);
    }
}

#[substreams::handlers::map]
fn map_events(block: Block, markets: StoreGetRaw, openbook_markets: StoreGetRaw) -> Result<pb::Events, Error> {
    if markets.get_last("__phoenix_snapshot").as_deref() != Some(&phoenix_metadata::SNAPSHOT_SLOT.to_le_bytes()) {
        return Err(Error::msg("Phoenix registry snapshot missing or mismatched"));
    }
    if openbook_markets.get_last("__openbook_snapshot").as_deref() != Some(&openbook_metadata::SNAPSHOT_SLOT.to_le_bytes()) {
        return Err(Error::msg("OpenBook registry snapshot missing or mismatched"));
    }
    let slot = block.slot;
    Ok(pb::Events {
        parent_slot: block.parent_slot,
        parent_hash: block.previous_blockhash,
        transactions: block
            .transactions
            .into_iter()
            .enumerate()
            .filter_map(|(index, tx)| {
                if !tx.is_successful() {
                    return None;
                }
                let mut transaction = process_transaction_with_markets(
                    tx,
                    Some(slot),
                    |address, instruction_index| {
                        let ordinal = phoenix_metadata::ordinal(index, instruction_index);
                        let definition = markets.get_at(ordinal, format!("m:{}", base58::encode(address)))?;
                        phoenix::Market::from_definition(address, &definition, |mint| {
                            let value = markets.get_at(ordinal, format!("d:{}", base58::encode(mint)))?;
                            Some(u32::from_le_bytes(value.as_slice().try_into().ok()?))
                        })
                    },
                    |address, instruction_index| {
                        let ordinal = phoenix_metadata::ordinal(index, instruction_index);
                        let definition = openbook_markets.get_at(ordinal, format!("m:{}", base58::encode(address)))?;
                        // 0 is the owned snapshot, 1 contains fields proven by a
                        // completed creation and signed metadata. Empty is closed.
                        if !matches!(definition.first(), Some(0 | 1)) {
                            return None;
                        }
                        openbook::Market::from_owned_header(address, &openbook::PROGRAM_ID, &definition[1..])
                    },
                )?;
                transaction.source_index = Some(index as u32);
                Some(transaction)
            })
            .collect(),
    })
}

/// Decode one successful transaction for both streaming and offline coverage audits.
pub fn process_transaction(tx: ConfirmedTransaction) -> Option<pb::Transaction> {
    process_transaction_with_phoenix(tx, None, |_, _| None)
}

/// Supply market metadata at the actual instruction ordinal. Missing metadata
/// leaves the original transaction replayable and never guesses lot sizes.
pub fn process_transaction_with_phoenix(
    tx: ConfirmedTransaction,
    expected_slot: Option<u64>,
    market_lookup: impl FnMut(&[u8], u32) -> Option<phoenix::Market>,
) -> Option<pb::Transaction> {
    process_transaction_with_markets(tx, expected_slot, market_lookup, |_, _| None)
}

/// Resolve each exchange's metadata at the instruction's execution ordinal.
/// Runtime ledger events and wallet transfers retain separate proof identities.
pub fn process_transaction_with_markets(
    tx: ConfirmedTransaction,
    expected_slot: Option<u64>,
    market_lookup: impl FnMut(&[u8], u32) -> Option<phoenix::Market>,
    openbook_lookup: impl FnMut(&[u8], u32) -> Option<openbook::Market>,
) -> Option<pb::Transaction> {
    process_transaction_with_metadata(tx, expected_slot, market_lookup, openbook_lookup, |_, _| None)
}

/// Fee metadata must describe the mint at this transfer's actual execution
/// ordinal. Unknown metadata falls back only to independently proven receipts.
pub fn process_transaction_with_metadata(
    tx: ConfirmedTransaction,
    expected_slot: Option<u64>,
    mut market_lookup: impl FnMut(&[u8], u32) -> Option<phoenix::Market>,
    mut openbook_lookup: impl FnMut(&[u8], u32) -> Option<openbook::Market>,
    fee_lookup: impl FnMut(&[u8], u32) -> Option<token_fees::TransferFee>,
) -> Option<pb::Transaction> {
    if !tx.is_successful() {
        return None;
    }
    let tx_meta = tx.meta.as_ref()?;
    let mut swaps = Vec::new();
    let mut boop_state = boop::State::new();
    let mut byreal_state = byreal::State::new();
    let mut darklake_state = darklake::State::new();
    let mut dumpfun_state = dumpfun::State::new();
    let mut pumpfun_pending = None;
    let mut pumpfun_amm_pending = None;
    let mut meteora_amm_state = meteora_amm::State::new();
    let mut meteora_daam_pending: VecDeque<meteora_daam::PendingSwap> = VecDeque::new();
    let mut meteora_dlmm_pending: VecDeque<meteora_dlmm::PendingSwap> = VecDeque::new();
    let mut moonshot_state = moonshot::State::new();
    let mut okx_state = okx::State::new();
    let mut pancakeswap_state = pancakeswap::State::new();
    let mut raydium_launchpad_pending = None;
    let mut raydium_amm_v4_state = raydium_amm_v4::State::new();
    let mut raydium_clmm_state = raydium_clmm::State::new();
    let mut raydium_cpmm_state = raydium_cpmm::State::new();
    let mut orca_whirlpool_state = orca_whirlpool::State::new();
    let mut routed_pools = routed_pool::Tracker::new();
    let token_mints = token_mints::TokenMintLookup::new(&tx, tx_meta);

    let instructions: Vec<_> = tx.walk_instructions().collect();
    for (index, instruction) in instructions.iter().enumerate() {
        routed_pools.observe(instruction);
        swaps.extend(jupiter_v6::decode_instruction(&tx, index, &instructions));
        byreal_state.handle_instruction(&instruction, &token_mints);
        if let Some(swap) = pumpfun::handle_instruction(&mut pumpfun_pending, &instruction) {
            swaps.push(swap);
        }
        if let Some(swap) = pumpfun_amm::handle_instruction(&mut pumpfun_amm_pending, &instruction) {
            swaps.push(swap);
        }
        meteora_amm_state.handle_instruction(&instruction, &token_mints);
        if let Some(swap) = meteora_daam::handle_instruction(&mut meteora_daam_pending, &instruction) {
            swaps.push(swap);
        }
        if let Some(swap) = meteora_dlmm::handle_instruction(&mut meteora_dlmm_pending, &instruction) {
            swaps.push(swap);
        }
        okx_state.handle_instruction(&instruction);
        pancakeswap_state.handle_instruction(&instruction, &token_mints);
        if let Some(swap) = raydium_launchpad::handle_instruction(&mut raydium_launchpad_pending, &instruction) {
            swaps.push(swap);
        }

        raydium_amm_v4_state.handle_instruction(&instruction, &token_mints);
        raydium_clmm_state.handle_instruction(&instruction, &token_mints);
        raydium_cpmm_state.handle_instruction(&instruction);
        orca_whirlpool_state.handle_instruction(&instruction, &token_mints);

        if let Some(swap) = spl_token_swap::handle_instruction(&instruction, &token_mints) {
            log::info!("SPL Token Swap 🚨 {}", base58::encode(&swap.program_id));
            swaps.push(swap);
        }
    }

    let mut jupiter_v4_state = jupiter_v4::State::new();
    for log_message in tx_meta.log_messages.iter() {
        if let Some(swap) = jupiter_v4_state.handle_log(&tx, log_message, &routed_pools) {
            swaps.push(swap);
        }
        if let Some(swap) = boop_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = byreal_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = darklake_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = dumpfun_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = meteora_amm_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = moonshot_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = okx_state.handle_log(log_message) {
            log::info!("OKX Swap 🟢 {} {}", base58::encode(&swap.program_id), base58::encode(&swap.amm_pool));
            swaps.push(swap);
        }
        if let Some(swap) = pancakeswap_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = raydium_amm_v4_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = raydium_clmm_state.handle_log(log_message, &token_mints) {
            swaps.push(swap);
        }
        if let Some(swap) = raydium_cpmm_state.handle_log(log_message) {
            swaps.push(swap);
        }
        if let Some(swap) = orca_whirlpool_state.handle_log(log_message) {
            swaps.push(swap);
        }
    }

    let rejected_swaps = quality::verify_with_fees(&tx, &token_mints, &mut swaps, true, fee_lookup);
    for (index, ix) in instructions.iter().enumerate() {
        if ix.program_id().0.as_slice() != phoenix::PROGRAM_ID || !matches!(ix.data().first(), Some(0 | 1 | 2 | 3)) {
            continue;
        }
        let accounts = ix.accounts();
        let Some(address) = accounts.get(2) else {
            continue;
        };
        let Some(market) = market_lookup(address.0, index as u32) else {
            continue;
        };
        let Some(fill) = phoenix::decode_fill(&tx, index, &market, expected_slot) else {
            continue;
        };
        // Another decoder may already have a verified wrapper for this exact
        // invocation. Never add a second financial row for the same fill.
        if swaps.iter().any(|swap| swap.source_index == Some(fill.instruction_index)) {
            continue;
        }
        swaps.push(pb::Swap {
            protocol: pb::Protocol::Phoenix as i32,
            program_id: phoenix::PROGRAM_ID.to_vec(),
            stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
            amm: phoenix::PROGRAM_ID.to_vec(),
            amm_pool: fill.market.to_vec(),
            user: fill.trader.to_vec(),
            input_mint: fill.input_mint.to_vec(),
            input_amount: fill.input_amount,
            output_mint: fill.output_mint.to_vec(),
            output_amount: fill.output_amount,
            source_index: Some(fill.instruction_index),
            source_event_index: Some(fill.log_instruction_index),
            orderbook_fill_verified: true,
            ..Default::default()
        });
    }
    for (index, ix) in instructions.iter().enumerate() {
        let Some(pool) = openbook::market_index(ix) else {
            continue;
        };
        let accounts = ix.accounts();
        let Some(address) = accounts.get(pool) else {
            continue;
        };
        let Some(market) = openbook_lookup(address.0, index as u32) else {
            continue;
        };
        let Some(fills) = openbook::decode_fills(&tx, index, &market) else {
            continue;
        };
        for fill in fills {
            swaps.push(pb::Swap {
                protocol: pb::Protocol::OpenbookV2 as i32,
                program_id: openbook::PROGRAM_ID.to_vec(),
                stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
                amm: openbook::PROGRAM_ID.to_vec(),
                amm_pool: fill.market.to_vec(),
                user: fill.trader.to_vec(),
                input_mint: fill.input_mint.to_vec(),
                input_amount: fill.input_amount,
                output_mint: fill.output_mint.to_vec(),
                output_amount: fill.output_amount,
                source_index: Some(fill.instruction_index),
                source_log_index: Some(fill.log_index),
                orderbook_fill_verified: true,
                ..Default::default()
            });
        }
    }
    swaps.sort_by_key(|swap| (swap.source_index, swap.source_transfer_index.or(swap.source_event_index), swap.source_log_index));
    let coverage_candidates = coverage::candidates(&tx, &token_mints, &swaps);
    if swaps.is_empty() && rejected_swaps.is_empty() && coverage_candidates.is_empty() {
        return None;
    }
    let coverage_transaction = if coverage_candidates.is_empty() {
        Vec::new()
    } else {
        substreams::proto::encode(&tx).expect("transaction encoding")
    };

    Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed(),
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(&tx).unwrap_or_default(),
        signers: get_signers(&tx).unwrap_or_default(),
        swaps,
        source_index: None,
        rejected_swaps,
        coverage_candidates,
        coverage_transaction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_token_exchanges_and_zero_output_swaps_remain_replayable() {
        for (fixture, program, reason) in [
            (
                include_bytes!("../fixtures/swap-coverage/78.transaction.pb").as_slice(),
                "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9",
                "mint_burn_exchange_unclassified",
            ),
            (
                include_bytes!("../fixtures/swap-coverage/79.transaction.pb").as_slice(),
                "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc",
                "recognized_swap_without_verified_flows",
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let decoded = process_transaction(tx.clone()).unwrap();
            assert!(decoded.swaps.is_empty());
            assert!(decoded
                .coverage_candidates
                .iter()
                .any(|candidate| base58::encode(&candidate.program_id) == program && candidate.reason == reason));
            assert_eq!(substreams::proto::decode::<ConfirmedTransaction>(&decoded.coverage_transaction).unwrap(), tx);
        }
        let mut tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/swap-coverage/76.transaction.pb").to_vec()).unwrap();
        let keys = tx.resolved_accounts();
        let program_index = keys
            .iter()
            .position(|key| key.as_slice() == substreams_solana_idls::solfi::v2::PROGRAM_ID)
            .unwrap() as u32;
        for group in &mut tx.meta.as_mut().unwrap().inner_instructions {
            for instruction in &mut group.instructions {
                if instruction.program_id_index == program_index {
                    instruction.data[0] = 255;
                }
            }
        }
        let decoded = process_transaction(tx.clone()).unwrap();
        let candidate = decoded.coverage_candidates.iter().find(|candidate| candidate.source_index == 23).unwrap();
        assert_eq!(candidate.reason, "token_exchange_unclassified");
        assert_eq!(candidate.transfer_count, 2);
        assert_eq!(candidate.mints.len(), 2);
        assert_eq!(candidate.instruction_tag[0], 255);
        assert!(decoded.swaps.iter().all(|swap| swap.source_index != Some(23)));
        assert_eq!(substreams::proto::decode::<ConfirmedTransaction>(&decoded.coverage_transaction).unwrap(), tx);
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/swap-coverage/62.transaction.pb").to_vec()).unwrap();
        let decoded = process_transaction(tx.clone()).unwrap();
        assert!(decoded.coverage_candidates.iter().all(|candidate| candidate.source_index != 8));
        let partial: Vec<_> = decoded.swaps.into_iter().filter(|swap| swap.source_index != Some(12)).collect();
        let mints = token_mints::TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
        assert!(coverage::candidates(&tx, &mints, &partial)
            .iter()
            .any(|candidate| candidate.source_index == 8 && candidate.reason == "multihop_without_all_verified_pools"));
    }

    #[test]
    fn phoenix_swaps_without_wallet_transfers_remain_replayable() {
        let program = substreams_solana::b58!("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY");
        for fixture in [
            include_bytes!("../fixtures/phoenix-immediate-1.transaction.pb").as_slice(),
            include_bytes!("../fixtures/phoenix-immediate-2.transaction.pb").as_slice(),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            for without_transfers in [false, true] {
                let mut tx = tx.clone();
                if without_transfers {
                    let programs = tx.resolved_accounts().iter().map(|a| (*a).clone()).collect::<Vec<_>>();
                    for group in &mut tx.meta.as_mut().unwrap().inner_instructions {
                        group.instructions.retain(|ix| programs[ix.program_id_index as usize].as_slice() == program);
                    }
                }
                let decoded = process_transaction(tx).unwrap();
                let candidates: Vec<_> = decoded.coverage_candidates.iter().filter(|c| c.program_id.as_slice() == program).collect();
                assert_eq!(candidates.len(), 1);
                assert_eq!(candidates[0].reason, "orderbook_swap_requires_fill_accounting");
                if without_transfers {
                    assert_eq!(candidates[0].transfer_count, 0);
                }
                assert!(!decoded.coverage_transaction.is_empty());
                assert!(decoded.swaps.iter().all(|swap| swap.amm.as_slice() != program));
            }
        }
        for data in [vec![2, 0], vec![3, 0], vec![10], vec![12], vec![13]] {
            let tx = crate::routed_pool::test_fixture::make_tx(program, &[[1; 32], [2; 32]], data);
            assert!(process_transaction(tx).map_or(true, |decoded| decoded.coverage_candidates.is_empty()));
        }
    }

    #[test]
    fn finalized_swap_variants_preserve_independently_audited_legs() {
        for line in include_str!("../fixtures/swap-coverage/cases.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/swap-coverage/{}", env!("CARGO_MANIFEST_DIR"), fields[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            assert_eq!(base58::encode(tx.hash()), fields[1]);
            let decoded = process_transaction(tx).expect(fields[0]);
            let actual: std::collections::HashSet<_> = decoded
                .swaps
                .iter()
                .map(|swap| {
                    assert!(swap.transfer_verified);
                    format!(
                        "{}:{}:{}:{}:{}:{}:{}:{}",
                        swap.protocol,
                        swap.source_index.unwrap(),
                        base58::encode(&swap.amm_pool),
                        base58::encode(&swap.user),
                        base58::encode(&swap.input_mint),
                        swap.input_amount,
                        base58::encode(&swap.output_mint),
                        swap.output_amount
                    )
                })
                .collect();
            for expected in fields[2].split('|') {
                assert!(actual.contains(expected), "missing audited leg in {}: {}", fields[0], expected);
            }
            let identities: std::collections::HashSet<_> = decoded.swaps.iter().map(|s| (s.source_index, s.source_transfer_index)).collect();
            assert_eq!(identities.len(), decoded.swaps.len(), "duplicate physical CPI in {}", fields[0]);
        }
    }

    #[test]
    fn restores_missing_legs_from_public_transactions() {
        for (fixture, pool, input_mint, output_mint, count) in [
            (
                include_bytes!("../fixtures/privacy.transaction.pb").as_slice(),
                "2YwxAtr1XjYHm7VkraEVj4P4n7YL4m7NDsLjf4pUhZwX",
                "So11111111111111111111111111111111111111112",
                "H5hygVvXiYxk2a3BVtjiqcDJK8TdHTB5u5U1fXEuBAGS",
                3,
            ),
            (
                include_bytes!("../fixtures/knet.transaction.pb").as_slice(),
                "B1JkXQH1yvTQtRavmAStwoy2Pu54SsjLqfWphL9VwpGu",
                "CfVs3waH2Z9TM397qSkaipTDhA9wWgtt8UchZKfwkYiu",
                "So11111111111111111111111111111111111111112",
                1,
            ),
            (
                include_bytes!("../fixtures/oilinu.transaction.pb").as_slice(),
                "9Bvmg9W8yFv3bKAWeCBNWwKtCNYi8BgKqGKTDtsrRPVx",
                "So11111111111111111111111111111111111111112",
                "GJqCjtgEwqdFWVRsDs8JXKFoTeRVZeHs1RL4ccvrpump",
                3,
            ),
        ] {
            let transaction = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let decoded = process_transaction(transaction).expect("successful swap must be emitted");
            assert_eq!(decoded.swaps.len(), count, "pool {} rejected {:?}", pool, decoded.rejected_swaps);
            let swap = decoded
                .swaps
                .iter()
                .find(|swap| base58::encode(&swap.amm_pool) == pool)
                .expect("pool leg must be present");
            assert_eq!(base58::encode(&swap.input_mint), input_mint);
            assert_eq!(base58::encode(&swap.output_mint), output_mint);
            assert!(swap.input_amount > 0 && swap.output_amount > 0);
        }
    }
}
