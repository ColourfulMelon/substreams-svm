//! A bounded market registry seeded from a finalized, owner/PDA-verified
//! snapshot, then updated by successful Phoenix initializations. Registry
//! ordinals preserve market reincarnations and earlier calls in the same block.
use std::collections::{BTreeMap, BTreeSet};

use substreams_solana::{base58, pb::sf::solana::r#type::v1::Block};

use crate::phoenix::PROGRAM_ID;

pub(crate) const SNAPSHOT_SLOT: u64 = 455747960;
const SEED: &[u8; 255 * 608] = include_bytes!("../fixtures/phoenix/market-registry-455747960.bin");

pub(crate) fn ordinal(transaction_index: usize, instruction_index: u32) -> u64 {
    ((transaction_index as u64 + 1) << 32) + u64::from(instruction_index) + 1
}

pub(crate) fn updates(block: &Block) -> Vec<(u64, String, Vec<u8>)> {
    let mut updates = Vec::new();
    // initialBlock is SNAPSHOT_SLOT+1. This also handles skipped slots at the
    // boundary: the first produced block has a parent at/before the snapshot.
    if block.slot > SNAPSHOT_SLOT && block.parent_slot <= SNAPSHOT_SLOT {
        let mut seeds = BTreeMap::new();
        for row in SEED.chunks_exact(608) {
            let market = &row[..32];
            let header = &row[32..];
            seeds.insert(format!("m:{}", base58::encode(market)), [vec![0], header.to_vec()].concat());
            for (mint, decimals) in [(&header[48..80], &header[40..44]), (&header[128..160], &header[120..124])] {
                seeds.insert(format!("d:{}", base58::encode(mint)), decimals.to_vec());
            }
        }
        updates.extend(seeds.into_iter().map(|(key, value)| (0, key, value)));
        updates.push((0, "__phoenix_snapshot".into(), SNAPSHOT_SLOT.to_le_bytes().to_vec()));
    }
    for (transaction_index, tx) in block.transactions.iter().enumerate() {
        if !tx.is_successful() {
            continue;
        }
        let instructions: Vec<_> = tx.walk_instructions().collect();
        if !instructions
            .iter()
            .any(|ix| ix.program_id().0.as_slice() == PROGRAM_ID && ix.data().first() != Some(&15))
        {
            continue;
        }
        let Some(meta) = tx.meta.as_option() else {
            continue;
        };
        let mut decimals = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for balance in meta.pre_token_balances.iter().chain(&meta.post_token_balances) {
            if !balance.program_id.is_empty() && balance.program_id != "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" {
                continue;
            }
            let Some(amount) = balance.ui_token_amount.as_option() else {
                continue;
            };
            if amount.decimals > 19 || base58::decode(&balance.mint).map_or(true, |mint| mint.len() != 32) {
                continue;
            }
            if decimals
                .insert(balance.mint.clone(), amount.decimals)
                .is_some_and(|prior| prior != amount.decimals)
            {
                ambiguous.insert(balance.mint.clone());
            }
        }
        let transaction_ordinal = ordinal(transaction_index, 0) - 1;
        for (mint, decimals) in decimals {
            if !ambiguous.contains(&mint) {
                updates.push((transaction_ordinal, format!("d:{mint}"), decimals.to_le_bytes().to_vec()));
            }
        }
        let initializations: Vec<_> = instructions
            .iter()
            .enumerate()
            .filter(|(_, ix)| ix.program_id().0.as_slice() == PROGRAM_ID && ix.data().first() == Some(&100))
            .collect();
        if initializations.is_empty() {
            continue;
        }
        let successful = crate::logs::successful_invocations(tx);
        for (index, ix) in initializations {
            if !successful.contains(&(index as u32)) {
                continue;
            }
            let data = ix.data();
            let accounts = ix.accounts();
            if accounts.len() != 10
                || !matches!(data.len(), 84 | 88)
                || accounts[0].0.as_slice() != PROGRAM_ID
                || accounts[1].0.as_slice() != substreams_solana_macro::b58!("7aDTsspkQNGKmrexAN7FLx9oxU3iPczSSvHNggyuqYkR")
                || accounts[8].0.iter().any(|byte| *byte != 0)
                || accounts[9].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
            {
                continue;
            }
            let mut definition = vec![1];
            for account in &accounts[4..8] {
                definition.extend_from_slice(account.0);
            }
            definition.extend_from_slice(&data[1..]);
            updates.push((
                ordinal(transaction_index, index as u32),
                format!("m:{}", base58::encode(accounts[2].0)),
                definition,
            ));
        }
    }
    updates.sort_by(|left, right| left.0.cmp(&right.0));
    updates
}
