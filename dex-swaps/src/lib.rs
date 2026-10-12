mod boop;
mod byreal;
mod coverage;
mod darklake;
mod dumpfun;
mod jupiter_v4;
mod jupiter_v6;
mod logs;
mod manifest;
mod mercurial;
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
    store::{Appender, StoreAdd, StoreAddInt64, StoreAppend, StoreGet, StoreGetRaw, StoreGetString, StoreNew, StoreSet, StoreSetRaw},
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
fn map_token_fee_updates(block: Block) -> Result<pb::TokenFeeUpdates, Error> {
    // Mainnet's owned EpochSchedule is retained with the fee fixtures: no
    // warmup, firstNormalEpoch/Slot=0, slotsPerEpoch=432000. This package is
    // pinned to network solana; other schedules require a different package.
    let epoch = block.slot / 432_000;
    Ok(pb::TokenFeeUpdates {
        changes: token_fees::native_changes(&block, epoch)
            .into_iter()
            .map(|(ordinal, mint, change)| pb::TokenFeeUpdate {
                ordinal,
                mint,
                change: change.encode(),
            })
            .collect(),
    })
}

#[substreams::handlers::store]
fn store_token_fee_mints(updates: pb::TokenFeeUpdates, output: StoreSetRaw) {
    for update in updates.changes {
        if matches!(
            token_fees::Change::decode(&update.change),
            Some(
                token_fees::Change::Snapshot { .. }
                    | token_fees::Change::InitializeFee(_)
                    | token_fees::Change::SetFee(_)
                    | token_fees::Change::UnknownInitializeFee
                    | token_fees::Change::UnknownSetFee
            )
        ) {
            output.set(update.ordinal, base58::encode(&update.mint), &[1]);
        }
    }
}

#[substreams::handlers::store]
fn store_token_fee_history(updates: pb::TokenFeeUpdates, mints: StoreGetRaw, output: StoreAppend<String>) {
    for update in updates.changes {
        if update.mint.is_empty() {
            let slot = u64::from_le_bytes(update.change.as_slice().try_into().expect("invalid fee snapshot marker"));
            output.append(update.ordinal, "__token22_snapshot", slot.to_string());
            continue;
        }
        let key = base58::encode(&update.mint);
        // Account initialization/closure is retained only for addresses that
        // have ever carried a fee configuration, keeping token-account churn
        // out of the registry. An unknown mutation keeps the address tracked.
        if mints.get_at(update.ordinal, &key).is_none() {
            continue;
        }
        output.append(update.ordinal, key, update.change.iter().map(|byte| format!("{byte:02x}")).collect::<String>());
    }
}

#[substreams::handlers::store]
fn store_token_fee_seed_count(updates: pb::TokenFeeUpdates, output: StoreAddInt64) {
    for update in updates.changes {
        if matches!(token_fees::Change::decode(&update.change), Some(token_fees::Change::Snapshot { .. })) {
            output.add(update.ordinal, "__token22_seeds", 1);
        }
    }
}

#[substreams::handlers::store]
fn store_token_fee_state(updates: pb::TokenFeeUpdates, history: StoreGetString, output: StoreSetRaw) {
    for update in updates.changes {
        if update.mint.is_empty() {
            output.set(update.ordinal, "__token22_snapshot", &update.change);
            continue;
        }
        let key = base58::encode(&update.mint);
        let Some(records) = history.get_at(update.ordinal, &key) else { continue };
        let state = token_fees::State::from_history(&records).expect("invalid native Token-2022 fee history");
        output.set(update.ordinal, key, &state.encode());
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
    let token_mints = token_mints::TokenMintLookup::new(&tx, tx_meta);

    let instructions: Vec<_> = tx.walk_instructions().collect();
    for (index, instruction) in instructions.iter().enumerate() {
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
        if let Some(swap) = meteora_dlmm::handle_instruction(&mut meteora_dlmm_pending, &instruction, index as u32) {
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

    swaps.extend(jupiter_v4::decode_logs(&tx));
    for log_message in tx_meta.log_messages.iter() {
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
    swaps.extend(manifest::decode_fills(&tx));
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
    fn defituna_two_hop_preserves_both_pool_legs_and_requires_complete_flows() {
        let tx =
            substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/swap-coverage/defituna-two-hop.transaction.pb").to_vec()).unwrap();
        let decoded = process_transaction(tx.clone()).unwrap();
        assert_eq!(decoded.swaps.len(), 2);
        assert!(decoded.rejected_swaps.is_empty());
        for (hop, (input, output, transfer)) in [(13_448_909, 1_495_460, 4), (1_495_460, 671_227_733, 5)].into_iter().enumerate() {
            let swap = &decoded.swaps[hop];
            let accounts = tx.walk_instructions().nth(3).unwrap().accounts();
            assert_eq!(swap.amm_pool, *accounts[hop].0);
            assert_eq!(swap.user, *accounts[14].0);
            assert_eq!((swap.input_amount, swap.output_amount), (input, output));
            assert_eq!(swap.source_index, Some(3));
            assert_eq!(swap.source_transfer_index, Some(transfer));
            assert!(swap.transfer_verified);
        }
        let mints = token_mints::TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
        for hop in 0..2 {
            assert!(coverage::candidates(&tx, &mints, &decoded.swaps[hop..hop + 1])
                .iter()
                .any(|candidate| candidate.source_index == 3 && candidate.reason == "multihop_without_all_verified_pools"));
        }
        for mode in 0..15 {
            let mut altered = tx.clone();
            let native_data = tx.walk_instructions().nth(3).unwrap().data().to_vec();
            for group in &mut altered.meta.as_mut().unwrap().inner_instructions {
                for ix in &mut group.instructions {
                    if ix.data == native_data {
                        match mode {
                            0 => {
                                ix.data.pop();
                            }
                            1 => ix.data[25] = 2,
                            2 => ix.data[24] = 2,
                            3 => ix.data[8..16].copy_from_slice(&13_448_908u64.to_le_bytes()),
                            4 => ix.data[16..24].copy_from_slice(&671_227_734u64.to_le_bytes()),
                            5 => ix.accounts[1] = ix.accounts[0],
                            6 => ix.accounts[11] = ix.accounts[10],
                            7 => ix.accounts[6] = ix.accounts[21],
                            8 => ix.data[64] = 0,
                            _ => {}
                        }
                    }
                }
            }
            if mode == 9 {
                altered.meta.as_mut().unwrap().log_messages.clear();
            }
            if mode == 10 {
                let logs = &mut altered.meta.as_mut().unwrap().log_messages;
                let end = logs
                    .iter()
                    .rposition(|log| log == "Program fUSioN9YKKSa3CUC2YUc4tPkHJ5Y6XW1yz8y6F7qWz9 success")
                    .unwrap();
                logs.truncate(end);
            }
            if mode == 11 {
                for log in &mut altered.meta.as_mut().unwrap().log_messages {
                    if log == "Program fUSioN9YKKSa3CUC2YUc4tPkHJ5Y6XW1yz8y6F7qWz9 success" {
                        *log = "Program fUSioN9YKKSa3CUC2YUc4tPkHJ5Y6XW1yz8y6F7qWz9 failed: custom program error: 1".into();
                    }
                }
            }
            if mode >= 12 {
                let keys: Vec<_> = altered.resolved_accounts().iter().map(|key| (*key).clone()).collect();
                let token_program = substreams_solana_idls::spl::token::PROGRAM_ID;
                let group = altered
                    .meta
                    .as_mut()
                    .unwrap()
                    .inner_instructions
                    .iter_mut()
                    .find(|group| group.instructions.iter().any(|ix| ix.data == native_data))
                    .unwrap();
                let position = group
                    .instructions
                    .iter()
                    .position(|ix| keys[ix.program_id_index as usize].as_slice() == token_program && ix.data.as_slice() == [12, 164, 209, 22, 0, 0, 0, 0, 0, 6])
                    .unwrap();
                match mode {
                    12 => {
                        group.instructions.remove(position);
                    }
                    13 => {
                        let duplicate = group.instructions[position].clone();
                        group.instructions.insert(position, duplicate);
                    }
                    _ => group.instructions[position].accounts[3] = group.instructions[position].accounts[2],
                }
            }
            assert!(process_transaction(altered).map_or(true, |decoded| decoded.swaps.is_empty()), "mode {mode}");
        }
        let mut exact_out = tx;
        let native_data = exact_out.walk_instructions().nth(3).unwrap().data().to_vec();
        for group in &mut exact_out.meta.as_mut().unwrap().inner_instructions {
            for ix in &mut group.instructions {
                if ix.data == native_data {
                    ix.data[24] = 0;
                    ix.data[8..16].copy_from_slice(&671_227_733u64.to_le_bytes());
                    ix.data[16..24].copy_from_slice(&13_448_909u64.to_le_bytes());
                }
            }
        }
        assert_eq!(process_transaction(exact_out).unwrap().swaps, decoded.swaps);
    }

    #[test]
    fn stabble_nested_and_dlmm_receipts_use_each_transfers_owned_fee() {
        let tx =
            substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/token-fees/stabble-dlmm-shared-output.transaction.pb").to_vec())
                .unwrap();
        let mint = substreams_solana::b58!("8RVBk8vxLiUHueLUW1f4izFVqN3nWippLhkohKg6EGkS");
        let bytes = include_bytes!("../fixtures/token-fees/stabble-shared-output-mint.bin");
        let (schedule, initialized) = token_fees::owned_mint(&substreams_solana_idls::spl::token_2022::PROGRAM_ID, bytes).unwrap();
        assert!(initialized);
        let fee = schedule.at(1055).unwrap();
        let without_metadata = process_transaction(tx.clone()).unwrap();
        assert!(without_metadata
            .swaps
            .iter()
            .all(|swap| swap.source_index != Some(7) && swap.source_index != Some(13)));
        let mut fee_ordinals = Vec::new();
        let decoded = process_transaction_with_metadata(
            tx.clone(),
            None,
            |_, _| None,
            |_, _| None,
            |address, ordinal| {
                if address != mint {
                    return None;
                }
                fee_ordinals.push(ordinal);
                Some(fee)
            },
        )
        .unwrap();
        for (source, protocol, input, output) in [
            (7, pb::Protocol::Stabble, 2_855_716_822, 90_210_921),
            (13, pb::Protocol::MeteoraDlmm, 282_715_965_415, 8_913_852_960),
        ] {
            let legs: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(source)).collect();
            assert_eq!(legs.len(), 1);
            assert_eq!(legs[0].protocol, protocol as i32);
            assert_eq!((legs[0].input_amount, legs[0].output_amount), (input, output));
            assert_eq!(legs[0].output_mint, mint);
            assert!(legs[0].transfer_verified);
        }
        assert!(fee_ordinals.contains(&10) && fee_ordinals.contains(&11) && fee_ordinals.contains(&16));
        // The same receiver's aggregate balance cannot assign either receipt.
        assert_eq!(90_210_921u64 + 8_913_852_960, 9_004_063_881);
        let mut incorrect = fee;
        incorrect.basis_points = 0;
        let decoded =
            process_transaction_with_metadata(tx.clone(), None, |_, _| None, |_, _| None, |address, _| (address == mint).then_some(incorrect)).unwrap();
        assert!(decoded.swaps.iter().all(|swap| swap.source_index != Some(7) && swap.source_index != Some(13)));
        let native_data = tx.walk_instructions().nth(7).unwrap().data().to_vec();
        let mut undeclared_vault = tx;
        for group in &mut undeclared_vault.meta.as_mut().unwrap().inner_instructions {
            for instruction in &mut group.instructions {
                if instruction.data == native_data {
                    instruction.accounts[12] = instruction.accounts[13];
                }
            }
        }
        let decoded =
            process_transaction_with_metadata(undeclared_vault, None, |_, _| None, |_, _| None, |address, _| (address == mint).then_some(fee)).unwrap();
        assert!(decoded.swaps.iter().all(|swap| swap.source_index != Some(7)));
        assert!(decoded
            .swaps
            .iter()
            .any(|swap| swap.source_index == Some(13) && swap.output_amount == 8_913_852_960));
    }

    #[test]
    fn delegated_pump_routes_require_distinct_verified_physical_hops() {
        for (fixture, parent, child, hops) in [
            (include_bytes!("../fixtures/swap-coverage/pump-delegated-1.transaction.pb").as_slice(), 19, 20, 2),
            (include_bytes!("../fixtures/swap-coverage/pump-delegated-2.transaction.pb").as_slice(), 6, 7, 2),
            (include_bytes!("../fixtures/swap-coverage/pump-delegated-3.transaction.pb").as_slice(), 4, 5, 1),
            (include_bytes!("../fixtures/swap-coverage/pump-delegated-4.transaction.pb").as_slice(), 22, 23, 1),
            (include_bytes!("../fixtures/swap-coverage/pump-delegated-5.transaction.pb").as_slice(), 27, 28, 1),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let decoded = process_transaction(tx.clone()).unwrap();
            let legs: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(child)).cloned().collect();
            assert_eq!(legs.len(), hops);
            assert!(legs.iter().all(|swap| swap.transfer_verified));
            let mints = token_mints::TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            assert!(coverage::candidates(&tx, &mints, &legs).iter().all(|candidate| candidate.source_index != parent));
            if hops == 2 {
                let mut repeated_identity = legs.clone();
                repeated_identity[1].source_transfer_index = repeated_identity[0].source_transfer_index;
                assert!(coverage::candidates(&tx, &mints, &repeated_identity).iter().any(|candidate|
                    candidate.source_index == parent && candidate.reason == "multihop_without_all_verified_pools"));
            }
            let parent_data = tx.walk_instructions().nth(parent as usize).unwrap().data().to_vec();
            for malformed in 0..3 {
                let mut altered = tx.clone();
                for instruction in &mut altered.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions {
                    if instruction.data == parent_data {
                        match malformed {
                            0 => { instruction.data.pop(); }
                            1 => instruction.data[8..16].fill(0),
                            _ => { instruction.accounts.pop(); }
                        }
                    }
                }
                for group in &mut altered.meta.as_mut().unwrap().inner_instructions {
                    for instruction in &mut group.instructions {
                        if instruction.data == parent_data {
                            match malformed {
                                0 => { instruction.data.pop(); }
                                1 => instruction.data[8..16].fill(0),
                                _ => { instruction.accounts.pop(); }
                            }
                        }
                    }
                }
                let mints = token_mints::TokenMintLookup::new(&altered, altered.meta.as_ref().unwrap());
                assert!(coverage::candidates(&altered, &mints, &legs).iter().any(|candidate| candidate.source_index == parent));
            }
            for invalid in 0..7 {
                let mut legs = legs.clone();
                match invalid {
                    0 => { legs.remove(0); }
                    1 => legs[0].transfer_verified = false,
                    2 => legs[0].input_amount = 0,
                    3 => legs[0].output_mint = vec![0; 32],
                    4 => legs[0].amm_pool = vec![0; 32],
                    5 => legs[0].source_index = Some(0),
                    _ => legs[0].source_transfer_index = None,
                }
                assert!(coverage::candidates(&tx, &mints, &legs).iter().any(|candidate|
                    candidate.source_index == parent && candidate.reason == "multihop_without_all_verified_pools"));
            }
        }
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
