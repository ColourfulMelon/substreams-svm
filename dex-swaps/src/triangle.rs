//! Triangle's swap-only contract from its program-owned deployed executable.
//! ELF e366e2f33caa37d6ae75e00b773b189cb2b8c612cfeb20d9d205def67770cbf8,
//! deployment slot 452701164. Dispatch5179 opcode4->3931; group owner4110,
//! group parser7029, outcome7059, vault/market bindings4242..4295.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{block_view::InstructionView};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::token_2022;

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("Tri3NG4HkZ6DddYPKoX2ehgkqFtDuej9Aspw5BmvSo4");
const PREDICT: [u8; 32] = b58!("prediCtPZCttYMvm2W3PtxmMxLmT1dtN7riU6Cxh6tM");
const CASH: [u8; 32] = b58!("CASHx9KJUStyftLFWGvEVf59SGeG9sh5FfcnZMVPCASH");
const CASH_VAULT: [u8; 32] = b58!("Fw7d4wF9mCvbmsgEfvXRrgBiocTbEzKiHwheBNJXhDrh");
const AUTHORITY: [u8; 32] = b58!("Ae52671Qs6UYCdvCmiEw3DVSNq3yfjMaZ3AjQHxJkw24");
const PRICE_SIGNER: [u8; 32] = b58!("8xeaWCsJYxRoudEZGJWURdfrtFhLYZz9b4iHJnW5tb3d");

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a = ix.accounts();
    let d = ix.data();
    if ix.program_id().0.as_slice() != PROGRAM_ID || a.len() != 17
        || d.len() != 13 || d[0] != 4 || d[2] > 1 || d[3] > 1 || d[12] > 4
        || a[4].0.as_slice() != CASH_VAULT || a[7].0.as_slice() != AUTHORITY || a[10].0.as_slice() != CASH
        || a[13].0.as_slice() != token_2022::PROGRAM_ID || a[14].0.as_slice() != token_2022::PROGRAM_ID
        || a[15].0.as_slice() != PREDICT || a[16].0.as_slice() != PRICE_SIGNER
        || a[..13].iter().enumerate().any(|(n, key)| a[..n].iter().any(|other| key.0 == other.0))
    { return None; }
    let amount = u64::from_le_bytes(d[4..12].try_into().ok()?);
    if amount == 0 { return None; }
    let outcome_vault = 5 + d[2] as usize;
    let sell = d[3] == 1;
    Some(Layout {
        protocol: Protocol::Triangle, pool: 1, authority: 0,
        users: if sell { [3, 2] } else { [2, 3] },
        vaults: if sell { [outcome_vault, 4] } else { [4, outcome_vault] },
        amount, exact_input: true, input_limit: false, input_first: true,
        fixed_users: true, minimum_output: 0,
    })
}

/// Two direct principals surround, optionally, one typed complete-set
/// mint/merge. Collateral and paired outcome mints are inventory operations;
/// only the final direct user transfer is priced. Net receipt remains the
/// caller's complete Token-2022 conservation/owned-fee proof.
pub(crate) fn settlement(source: u32, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup, successful: &HashSet<u32>) -> bool {
    let Some(l) = layout(ix) else { return false };
    let a = ix.accounts();
    let d = ix.data();
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if depth == 0 || !matches!(calls.len(), 2 | 6) || !successful.contains(&source)
        || calls.iter().enumerate().any(|(n, _)| !successful.contains(&(source + 1 + n as u32)))
    { return false; }
    let outcome_mint = 11 + d[2] as usize;
    let sell = d[3] == 1;
    let mint_roles = if sell { [outcome_mint, 10] } else { [10, outcome_mint] };
    for (n, c) in [calls.first().unwrap(), calls.last().unwrap()].into_iter().enumerate() {
        let cash = mint_roles[n] == 10;
        let data = c.data();
        let ca = c.accounts();
        let (from, to, authority) = if n == 0 { (l.users[0], l.vaults[0], 0) } else { (l.vaults[1], l.users[1], 7) };
        let roles = if cash { vec![from, 10, to, authority] } else { vec![from, to, authority] };
        if c.is_root() || c.stack_height() != depth + 1 || c.program_id().0.as_slice() != token_2022::PROGRAM_ID
            || ca.len() != roles.len() || data.len() != if cash { 10 } else { 9 }
            || data[0] != if cash { 12 } else { 3 } || cash && data[9] != 6
            || ca.iter().zip(roles).any(|(key, role)| key.0 != a[role].0)
            || mints.mint_for(a[from].0).as_deref() != Some(a[mint_roles[n]].0.as_slice())
            || mints.mint_for(a[to].0).as_deref() != Some(a[mint_roles[n]].0.as_slice())
        { return false; }
        let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
        if amount == 0 || n == 0 && amount != l.amount { return false; }
    }
    if calls.len() == 2 { return true; }
    let p = &calls[1];
    let pd = p.data();
    let pa = p.accounts();
    let roles = [7, 8, 10, 11, 12, 4, 9, 5, 6, 13, 14];
    if p.is_root() || p.stack_height() != depth + 1 || p.program_id().0.as_slice() != PREDICT
        || pd.len() != 16 || pd[..8] != if sell { [148, 141, 236, 47, 174, 126, 69, 111] } else { [124, 189, 27, 43, 216, 40, 147, 66] }
        || pa.len() != roles.len() || pa.iter().zip(roles).any(|(key, role)| key.0 != a[role].0)
    { return false; }
    let amount = u64::from_le_bytes(pd[8..16].try_into().unwrap());
    if amount == 0 { return false; }
    for (n, c) in calls[2..5].iter().enumerate() {
        let cash = if sell { n == 2 } else { n == 0 };
        let roles = if cash {
            if sell { vec![9, 10, 4, 8] } else { vec![4, 10, 9, 7] }
        } else {
            let side = if sell { n } else { n - 1 };
            if sell { vec![5 + side, 11 + side, 7] } else { vec![11 + side, 5 + side, 8] }
        };
        let ca = c.accounts();
        let cd = c.data();
        if c.is_root() || c.stack_height() != depth + 2 || c.program_id().0.as_slice() != token_2022::PROGRAM_ID
            || ca.len() != roles.len() || ca.iter().zip(roles).any(|(key, role)| key.0 != a[role].0)
            || cd.len() != if cash { 10 } else { 9 } || cd[0] != if cash { 12 } else if sell { 8 } else { 7 }
            || cash && cd[9] != 6 || u64::from_le_bytes(cd[1..9].try_into().unwrap()) != amount
        { return false; }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn finalized_triangle_user_principals_exclude_complete_set_collateral() {
        let mut shapes = HashSet::new();
        for line in include_str!("../fixtures/swap-coverage/triangle-native/cases.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/swap-coverage/triangle-native/{}.pb", env!("CARGO_MANIFEST_DIR"), f[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let index = f[1].parse::<u32>().unwrap();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(index)).collect();
            assert_eq!(swaps.len(), 1, "{} index {index}", f[0]);
            let s = swaps[0];
            assert!(s.transfer_verified);
            assert_eq!(s.protocol, Protocol::Triangle);
            assert_eq!(base58::encode(&s.amm_pool), f[2]);
            assert_eq!(base58::encode(&s.user), f[3]);
            assert_eq!(base58::encode(&s.input_mint), f[4]);
            assert_eq!(s.input_amount, f[5].parse::<u64>().unwrap());
            assert_eq!(base58::encode(&s.output_mint), f[6]);
            assert_eq!(s.output_amount, f[7].parse::<u64>().unwrap());
            assert_eq!(s.source_transfer_index, Some(f[8].parse::<u32>().unwrap()));
            let source = tx.walk_instructions().nth(index as usize).unwrap();
            let native_data = source.data().clone();
            let conversion = f[9] != "inventory";
            if !shapes.insert((native_data[2], native_data[3], conversion)) { continue; }
            for mode in 0..if conversion { 38 } else { 28 } {
                let mut wrong = tx.clone();
                let meta = wrong.meta.as_option_mut().unwrap();
                let (g, p) = meta.inner_instructions.iter().enumerate().find_map(|(g, group)|
                    group.instructions.iter().position(|ix| ix.data == native_data).map(|p| (g, p))).unwrap();
                let calls = &mut meta.inner_instructions[g].instructions;
                let a = calls[p].accounts.clone();
                let last = p + if conversion { 6 } else { 2 };
                match mode {
                    0 => calls[p].data[0] = 3,
                    1 => { calls[p].data.pop(); }
                    2 => calls[p].data.push(0),
                    3 => calls[p].data[2] = 2,
                    4 => calls[p].data[3] = 2,
                    5 => calls[p].data[12] = 5,
                    6 => calls[p].data[4] ^= 1,
                    7 => calls[p].data[4..12].fill(0),
                    8 => calls[p].accounts[1] = a[0],
                    9 => calls[p].accounts[4] = a[2],
                    10 => calls[p].accounts[7] = a[0],
                    11 => calls[p].accounts[10] = a[11],
                    12 => calls[p].accounts[11 + native_data[2] as usize] = a[10],
                    13 => calls[p].accounts[13] = a[1],
                    14 => calls[p].accounts[14] = a[1],
                    15 => calls[p].accounts[15] = a[1],
                    16 => calls[p].accounts[16] = a[0],
                    17 => calls[p + 1].accounts[0] = a[1],
                    18 => calls[p + 1].data[1..9].fill(0),
                    19 => calls[last].accounts[0] = a[1],
                    20 => calls[last].accounts[1] = a[1],
                    21 => calls[last].data[1..9].fill(0),
                    22 => calls[last].stack_height = Some(99),
                    23 => { let extra = calls[p + 1].clone(); calls.insert(last + 1, extra); }
                    24 => meta.err = buffa::MessageField::some(Default::default()),
                    25 => {
                        let success = meta.log_messages.iter_mut().find(|log| **log == format!("Program {} success", base58::encode(PROGRAM_ID))).unwrap();
                        *success = format!("Program {} failed: custom program error: 1", base58::encode(PROGRAM_ID));
                    }
                    26 => calls[last].data[0] = 26,
                    27 => {
                        let destination = a[if native_data[3] == 1 { 2 } else { 3 }] as u32;
                        if let Some(balance) = meta.post_token_balances.iter_mut().find(|b| b.account_index == destination) {
                            let amount = balance.ui_token_amount.as_option_mut().unwrap();
                            amount.amount = (amount.amount.parse::<u64>().unwrap() + 1).to_string();
                        } else {
                            meta.pre_token_balances.clear();
                            meta.post_token_balances.clear();
                        }
                    }
                    28 => calls[p + 2].data[0] ^= 1,
                    29 => calls[p + 2].data[8] ^= 1,
                    30 => calls[p + 2].accounts[1] = a[1],
                    31 => calls[p + 2].accounts[3] = a[10],
                    32 => calls[p + 3].accounts[0] = a[0],
                    33 => calls[p + 3].data[1] ^= 1,
                    34 => calls[p + 4].data[0] = 3,
                    35 => calls[p + 4].accounts[0] = a[0],
                    36 => calls[p + 5].data[1] ^= 1,
                    _ => calls[p + 5].stack_height = Some(3),
                }
                assert!(crate::process_transaction(wrong).map_or(true, |d| d.swaps.iter().all(|s| s.source_index != Some(index))),
                    "{} index {index} invalid settlement {mode}", f[0]);
            }
            for priority in 1..=4 {
                // The deployed handler accepts these priority selectors. They
                // choose execution policy, never the settled financial atoms.
                let mut changed = tx.clone();
                for group in &mut changed.meta.as_option_mut().unwrap().inner_instructions {
                    for ix in &mut group.instructions {
                        if ix.data == native_data { ix.data[12] = priority; }
                    }
                }
                let decoded = crate::process_transaction(changed).unwrap();
                let restored = decoded.swaps.iter().find(|s| s.source_index == Some(index)).unwrap();
                assert_eq!((restored.input_amount, restored.output_amount), (s.input_amount, s.output_amount));
            }

        }
    }
}
