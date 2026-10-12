//! Riptide native exact-input swaps. Contract: publisher `riptide-amm` 2.1.1,
//! generated SwapExactIn and SlippageTolerance; executable/program ID is pinned.
//! Price comes from two committed direct token principals, never the oracle,
//! quoted threshold, market reserves or a router's aggregate output.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58, base58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("riptK81hDxhe5pW5jSzSM9iRA8azgEgLJ4dXkPtBS7j");
const MEMO: [u8; 32] = b58!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
const SYSVAR: [u8; 32] = b58!("Sysvar1nstructions1111111111111111111111111");
const JIT: [u8; 32] = b58!("jitodontfront111111111111111111111111111111");
const JUPITER_JIT: [u8; 32] = b58!("jitodontfront1111111111111111JustUseJupiter");

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a = ix.accounts();
    let d = ix.data();
    if ix.program_id().0.as_slice() != PROGRAM_ID || !matches!(a.len(), 12 | 13)
        || d.len() < 12 || d[0] != 2 || d[9] > 1
        || a[10].0.as_slice() != MEMO || a[11].0.as_slice() != SYSVAR
        || a.len() == 13 && ![JIT.as_slice(), JUPITER_JIT.as_slice()].contains(&a[12].0.as_slice())
        || a[..8].iter().enumerate().any(|(n, key)| a[..n].iter().any(|other| key.0 == other.0))
        || a[8..10].iter().any(|p| ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&p.0.as_slice()))
    { return None; }
    // Complete Borsh enum, including its bounded payload, followed by one
    // boolean. Remaining bytes cannot silently introduce a different format.
    let length = match d[10] { 0 => 12, 1 => 28, 2 => 20, _ => return None };
    if d.len() != length || d[length - 1] > 1 { return None; }
    let amount = u64::from_le_bytes(d[1..9].try_into().ok()?);
    if amount == 0 { return None; }
    let input = usize::from(d[9] == 0);
    Some(Layout {
        protocol: Protocol::Riptide, pool: 1, authority: 0,
        users: [4 + input, 5 - input], vaults: [6 + input, 7 - input],
        amount, exact_input: true, input_limit: d[length - 1] == 1,
        input_first: true, fixed_users: true, minimum_output: 0,
    })
}

pub(crate) fn settlement(tx: &ConfirmedTransaction, source: u32, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup, successful: &HashSet<u32>) -> bool {
    let Some(l) = layout(ix) else { return false };
    let a = ix.accounts();
    let resolved = tx.resolved_accounts();
    let Some(meta) = tx.meta.as_ref() else { return false };
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if depth == 0 || calls.len() != 2 || !successful.contains(&source)
        || a.len() == 13 && (mints.mint_for(a[12].0).is_some()
            || a[..12].iter().any(|key| key.0 == a[12].0)
            || calls.iter().any(|c| c.accounts().iter().any(|key| key.0 == a[12].0)))
    { return false; }
    if a.len() == 13 {
        let Some(index) = resolved.iter().position(|key| *key == a[12].0) else { return false };
        if meta.pre_balances.get(index) != meta.post_balances.get(index) { return false; }
    }
    let input = usize::from(ix.data()[9] == 0);
    for (n, c) in calls.iter().enumerate() {
        let side = if n == 0 { input } else { 1 - input };
        let (from, to, authority) = if n == 0 { (l.users[0], l.vaults[0], 0) } else { (l.vaults[1], l.users[1], 1) };
        let ca = c.accounts();
        let cd = c.data();
        let with_fee = c.program_id().0.as_slice() == token_2022::PROGRAM_ID && cd.len() == 19 && cd[..2] == [26, 1];
        let checked = cd.len() == 10 && cd[0] == 12;
        if c.is_root() || c.stack_height() != depth + 1 || !successful.contains(&(source + 1 + n as u32))
            || c.program_id().0 != a[8 + side].0 || ca.len() != 4 || !(with_fee || checked)
            || ca.iter().zip([from, 2 + side, to, authority]).any(|(key, role)| key.0 != a[role].0)
            || mints.mint_for(a[from].0).as_deref() != Some(a[2 + side].0.as_slice())
            || mints.mint_for(a[to].0).as_deref() != Some(a[2 + side].0.as_slice())
        { return false; }
        let offset = if with_fee { 2 } else { 1 };
        let Some(vault) = resolved.iter().position(|key| *key == a[6 + side].0) else { return false };
        let balances: Vec<_> = meta.pre_token_balances.iter().chain(&meta.post_token_balances)
            .filter(|b| b.account_index as usize == vault).collect();
        if balances.is_empty() || balances.iter().any(|b| b.mint != base58::encode(a[2 + side].0)
            || b.owner != base58::encode(a[1].0) || b.program_id != base58::encode(a[8 + side].0)
            || b.ui_token_amount.as_ref().is_none_or(|v| v.decimals != u32::from(cd[offset + 8])))
        { return false; }
        let amount = u64::from_le_bytes(cd[offset..offset + 8].try_into().unwrap());
        if amount == 0 || n == 0 && (if l.input_limit { amount > l.amount } else { amount != l.amount })
            || with_fee && u64::from_le_bytes(cd[11..19].try_into().unwrap()) >= amount
        { return false; }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalized_riptide_principals_match_native_contract_and_physical_receipts() {
        let mut shapes = HashSet::new();
        let mut positives = 0;
        for line in include_str!("../fixtures/swap-coverage/riptide-native/cases.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/swap-coverage/riptide-native/{}.pb", env!("CARGO_MANIFEST_DIR"), f[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let source = f[1].parse::<u32>().unwrap();
            let result = crate::process_transaction(tx.clone()).unwrap();
            let rows: Vec<_> = result.swaps.iter().filter(|s| s.source_index == Some(source)).collect();
            assert_eq!(rows.len(), 1, "{} source {source}", f[0]);
            let s = rows[0];
            assert!(s.transfer_verified);
            assert_eq!(s.protocol, Protocol::Riptide as i32);
            assert_eq!(base58::encode(&s.amm_pool), f[2]);
            assert_eq!(base58::encode(&s.user), f[3]);
            assert_eq!(base58::encode(&s.input_mint), f[4]);
            assert_eq!(s.input_amount, f[5].parse::<u64>().unwrap());
            assert_eq!(base58::encode(&s.output_mint), f[6]);
            assert_eq!(s.output_amount, f[7].parse::<u64>().unwrap());
            assert_eq!(s.source_transfer_index, Some(f[8].parse::<u32>().unwrap()));
            positives += 1;
            let native = tx.walk_instructions().nth(source as usize).unwrap();
            let a = native.accounts();
            let d = native.data().clone();
            let shape = (a.len(), d[9], d[d.len() - 1], a[8].0.clone(), a[9].0.clone(), a.get(12).map(|k| k.0.clone()));
            if native.is_root() || !shapes.insert(shape) { continue; }
            // Locate the exact flattened source, including identical calls in
            // the same transaction; payload equality cannot identify a leg.
            let (g, p) = {
                let mut index = 0u32;
                let mut location = None;
                let groups = &tx.meta.as_ref().unwrap().inner_instructions;
                for root in 0..tx.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions.len() {
                    index += 1;
                    if let Some((g, group)) = groups.iter().enumerate().find(|(_, group)| group.index as usize == root) {
                        for p in 0..group.instructions.len() {
                            if index == source { location = Some((g, p)); }
                            index += 1;
                        }
                    }
                }
                location.unwrap()
            };
            let occurrence = tx.walk_instructions().take(source as usize).filter(|c| c.program_id().0.as_slice() == PROGRAM_ID).count();
            for mode in 0..34 {
                if mode == 33 && a.len() == 12 { continue; }
                let mut wrong = tx.clone();
                let meta = wrong.meta.as_mut().unwrap();
                let calls = &mut meta.inner_instructions[g].instructions;
                let roles = calls[p].accounts.clone();
                match mode {
                    0 => calls[p].data[0] = 3,
                    1 => { calls[p].data.pop(); }
                    2 => calls[p].data.push(0),
                    3 => calls[p].data[9] = 2,
                    4 => calls[p].data[10] = 3,
                    5 => calls[p].data[11] = 2,
                    6 => calls[p].data[1..9].fill(0),
                    7 => { let n = u64::from_le_bytes(d[1..9].try_into().unwrap()) - 1; calls[p].data[1..9].copy_from_slice(&n.to_le_bytes()); }
                    8 => calls[p].accounts[0] = roles[1],
                    9 => calls[p].accounts[1] = roles[0],
                    10 => calls[p].accounts[2] = roles[3],
                    11 => calls[p].accounts.swap(4, 5),
                    12 => calls[p].accounts.swap(6, 7),
                    13 => calls[p].accounts[8] = roles[1],
                    14 => calls[p].accounts[10] = roles[11],
                    15 => calls[p].accounts[11] = roles[10],
                    16 => calls[p + 1].accounts[0] = roles[1],
                    17 => calls[p + 2].accounts[3] = roles[0],
                    18 | 19 => { let c = &mut calls[p + 1 + usize::from(mode == 19)]; let offset = if c.data[0] == 26 { 2 } else { 1 }; c.data[offset..offset + 8].fill(0); }
                    20 => calls[p + 2].stack_height = Some(99),
                    21 | 22 => { let c = &mut calls[p + 1 + usize::from(mode == 22)]; let decimals = if c.data[0] == 26 { 10 } else { 9 }; c.data[decimals] ^= 1; }
                    23 => { let extra = calls[p + 1].clone(); calls.insert(p + 3, extra); }
                    24 => meta.err = Some(Default::default()),
                    25 => {
                        let position = meta.log_messages.iter().enumerate().filter(|(_, l)| **l == format!("Program {} success", base58::encode(PROGRAM_ID))).nth(occurrence).unwrap().0;
                        meta.log_messages[position] = format!("Program {} failed: custom program error: 1", base58::encode(PROGRAM_ID));
                    }
                    26 => {
                        let text = format!("Program {} invoke [{}]", base58::encode(PROGRAM_ID), native.stack_height());
                        let position = meta.log_messages.iter().enumerate().filter(|(_, l)| **l == text).nth(occurrence).unwrap().0;
                        meta.log_messages.truncate(position + 1);
                        meta.log_messages.push("Log truncated".into());
                    }
                    27 | 28 => { let c = &mut calls[p + 1 + usize::from(mode == 28)]; if c.data[0] == 26 { c.data[11..19].fill(255); } else { c.data[0] = 26; } }
                    29..=31 => {
                        let vault = roles[6] as u32;
                        for b in meta.pre_token_balances.iter_mut().chain(&mut meta.post_token_balances).filter(|b| b.account_index == vault) {
                            if mode == 29 { b.owner = base58::encode(a[0].0); }
                            else if mode == 30 { b.mint = base58::encode(a[3].0); }
                            else { b.program_id = base58::encode(PROGRAM_ID); }
                        }
                    }
                    32 => { if roles.len() == 13 { calls[p].accounts[12] = roles[0]; } else { calls[p].accounts.push(roles[0]); } }
                    _ => meta.pre_balances[roles[12] as usize] += 1,
                }
                let result = crate::process_transaction(wrong);
                assert!(!result.is_some_and(|r| r.swaps.iter().any(|s| s.source_index == Some(source))), "{} source {source} mode {mode}", f[0]);
            }
        }
        assert_eq!(positives, 388);
    }
}
