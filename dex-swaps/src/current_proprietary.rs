//! Typed deployed swap variants with complete committed principal receipts.
//! Original program binaries, SDKs, and finalized RPC/PB evidence are retained
//! in current-proprietary; loan-position changes never enter this decoder.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) fn applies(ix: &InstructionView) -> bool {
    let p = ix.program_id(); let d = ix.data();
    p.0.as_slice() == b58!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp") && d.len() == 25 && d[24] == (48 ^ 58 ^ 3)
        || p.0.as_slice() == b58!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH") && d.first() == Some(&17)
        || p.0.as_slice() == b58!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE") && d.first() == Some(&14)
        || p.0.as_slice() == b58!("DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD") && d.first() == Some(&26) && ix.accounts().len() == 14
}

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    if !applies(ix) { return None; }
    let a = ix.accounts();
    let mut data = ix.data().clone();
    let (protocol, pool, authority, users, vaults, offset, minimum) = if ix.program_id().0.as_slice() == b58!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp") {
        if data.len() != 25 || data[24] != (48 ^ 58 ^ 3) || a.len() != 18 { return None; }
        for (i, byte) in data.iter_mut().enumerate() {
            *byte ^= [58, 255, 47, 255, 226, 186, 235, 195][i % 8] ^ ((i / 8) as u16).to_le_bytes()[i % 2];
        }
        let direction = u64::from_le_bytes(data[16..24].try_into().ok()?);
        if direction > 1 || a[6].0.as_slice() != b58!("SysvarC1ock11111111111111111111111111111111")
            || [7, 8].iter().any(|p| a[*p].0.as_slice() != token::PROGRAM_ID) || a[10].0 == a[11].0 { return None; }
        let side = direction as usize;
        (Protocol::Humidifi, 1, 0, [4 + side, 5 - side], [2 + side, 3 - side], 8, 0)
    } else if ix.program_id().0.as_slice() == b58!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH") {
        if data.len() != 19 || data[0] != 17 || data[1] > 1 || ![0, 4].contains(&data[18]) || a.len() != 14
            || a[7].0 == a[8].0 || [9, 10].iter().any(|p| ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&a[*p].0.as_slice())) { return None; }
        let side = usize::from(data[1] == 0);
        (Protocol::Tesserav, 1, 2, [5 + side, 6 - side], [3 + side, 4 - side], 2, u64::from_le_bytes(data[10..18].try_into().ok()?))
    } else if ix.program_id().0.as_slice() == b58!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE") {
        if data.len() != 18 || data[0] != 14 || data[1] > 1 || a.len() != 15
            || a[10].0.as_slice() != b58!("Sysvar1nstructions1111111111111111111111111")
            || a[6].0 == a[7].0 || [11, 12].iter().any(|p| ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&a[*p].0.as_slice())) { return None; }
        let side = data[1] as usize;
        (Protocol::GoonfiV2, 1, 0, [2 + side, 3 - side], [4 + side, 5 - side], 2, u64::from_le_bytes(data[10..18].try_into().ok()?))
    } else if ix.program_id().0.as_slice() == b58!("DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD") {
        if data.len() != 32 || data[0] != 26 || data[1] > 1 || data[2..4] != [0, 0] || a.len() != 14
            || a[13].0.as_slice() != token::PROGRAM_ID || a[1].0 == a[2].0
            || i64::from_le_bytes(data[16..24].try_into().ok()?) <= 0 || i64::from_le_bytes(data[24..32].try_into().ok()?) < 0 { return None; }
        let side = data[1] as usize;
        (Protocol::Deriverse, 5, 0, [11 + side, 12 - side], [3 + side, 4 - side], 16, u64::from_le_bytes(data[24..32].try_into().ok()?))
    } else { return None; };
    let amount = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    if amount == 0 || a[pool].0 == a[authority].0 || users.iter().chain(&vaults).any(|p| a[*p].0 == a[pool].0)
        || a[users[0]].0 == a[users[1]].0 || a[vaults[0]].0 == a[vaults[1]].0
        || users.iter().any(|u| vaults.iter().any(|v| a[*u].0 == a[*v].0)) { return None; }
    Some(Layout { protocol, pool, authority, users, vaults, amount, minimum_output: minimum,
        exact_input: true, input_limit: false, input_first: true, fixed_users: true })
}

pub(crate) fn settlement(source: u32, tx: &ConfirmedTransaction, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup, successful: &HashSet<u32>) -> Option<(u64, u64)> {
    let l = layout(ix)?;
    let a = ix.accounts();
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if calls.len() != 2 || !successful.contains(&source)
        || calls.iter().enumerate().any(|(n, child)| child.stack_height() != depth + 1 || !successful.contains(&(source + n as u32 + 1))) { return None; }
    let (input_mint, output_mint, input_program, output_program, output_authority) = match l.protocol {
        Protocol::Humidifi => (10 + usize::from(l.users[0] == 5), 11 - usize::from(l.users[0] == 5), 7 + usize::from(l.users[0] == 5), 8 - usize::from(l.users[0] == 5), 1),
        Protocol::Tesserav => (7 + usize::from(l.users[0] == 6), 8 - usize::from(l.users[0] == 6), 9 + usize::from(l.users[0] == 6), 10 - usize::from(l.users[0] == 6), 0),
        Protocol::GoonfiV2 => (6 + usize::from(l.users[0] == 3), 7 - usize::from(l.users[0] == 3), 11 + usize::from(l.users[0] == 3), 12 - usize::from(l.users[0] == 3), 1),
        Protocol::Deriverse => (1 + usize::from(l.users[0] == 12), 2 - usize::from(l.users[0] == 12), 13, 13, l.vaults[1]),
        _ => return None,
    };
    for (position, mint) in [(l.users[0], input_mint), (l.vaults[0], input_mint), (l.users[1], output_mint), (l.vaults[1], output_mint)] {
        if mints.mint_for(a[position].0).as_deref() != Some(a[mint].0.as_slice()) { return None; }
    }
    let mut principal = [0; 2];
    for (n, (from, to, authority, mint, program)) in [(l.users[0], l.vaults[0], l.authority, input_mint, input_program),
        (l.vaults[1], l.users[1], output_authority, output_mint, output_program)].into_iter().enumerate() {
        let matches: Vec<_> = calls.iter().filter(|child| {
            let c = child.accounts(); let d = child.data();
            child.program_id().0 == a[program].0 && match (d.first(), d.len(), c.len()) {
                (Some(3), 9, 3) => c[0].0 == a[from].0 && c[1].0 == a[to].0 && c[2].0 == a[authority].0,
                // The primary Token transfer contract permits an explicit
                // signer list. Deriverse repeats the exact single authority;
                // unrelated or additional signers remain unsupported.
                (Some(3), 9, 4) if l.protocol == Protocol::Deriverse => c[0].0 == a[from].0 && c[1].0 == a[to].0
                    && c[2].0 == a[authority].0 && c[3].0 == c[2].0,
                (Some(12), 10, 4) => c[0].0 == a[from].0 && c[1].0 == a[mint].0 && c[2].0 == a[to].0 && c[3].0 == a[authority].0,
                _ => false,
            }
        }).collect();
        if matches.len() != 1 { return None; }
        principal[n] = u64::from_le_bytes(matches[0].data()[1..9].try_into().ok()?);
    }
    if l.vaults.iter().any(|position| !vault_ledger(tx, a[*position].0, mints, successful)) { return None; }
    (principal[0] == l.amount && principal[1] > 0 && principal[1] >= l.minimum_output).then_some((principal[0], principal[1]))
}

/// Persistent pool vaults independently bind receipts even when the user's
/// temporary WSOL recipient is closed. Replay the complete token ledger, so
/// another invocation using the same vault cannot hide a changed native CPI.
pub(crate) fn vault_ledger(tx: &ConfirmedTransaction, vault: &[u8], mints: &TokenMintLookup, successful: &HashSet<u32>) -> bool {
    let Some(meta) = tx.meta.as_ref() else { return false };
    let resolved = tx.resolved_accounts();
    let Some(position) = resolved.iter().position(|a| a.as_slice() == vault) else { return false };
    let Some(mint) = mints.mint_for(vault) else { return false };
    let amount = |balances: &[substreams_solana::pb::sf::solana::r#type::v1::TokenBalance]| {
        let balance = balances.iter().find(|b| b.account_index as usize == position)?;
        if substreams_solana::base58::decode(&balance.mint).ok()? != mint { return None; }
        balance.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()
    };
    let (Some(pre), Some(post)) = (amount(&meta.pre_token_balances), amount(&meta.post_token_balances)) else { return false };
    let mut change = 0i128;
    for (index, ix) in tx.walk_instructions().enumerate() {
        if ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&ix.program_id().0.as_slice()) { continue; }
        let a = ix.accounts(); let d = ix.data();
        if !a.iter().any(|a| a.0.as_slice() == vault) { continue; }
        if !successful.contains(&(index as u32)) { return false; }
        let (source,destination,declared) = match (d.first(),d.len(),a.len()) {
            (Some(3),9,3..=14) => (Some(a[0].0),Some(a[1].0),None),
            (Some(12),10,4..=15) => (Some(a[0].0),Some(a[2].0),Some(a[1].0)),
            (Some(7),9,3..=14) | (Some(14),10,3..=14) => (None,Some(a[1].0),Some(a[0].0)),
            (Some(8),9,3..=14) | (Some(15),10,3..=14) => (Some(a[0].0),None,Some(a[1].0)),
            _ => return false,
        };
        if declared.is_some_and(|m| m.as_slice()!=mint.as_slice()) { return false; }
        let n = i128::from(u64::from_le_bytes(d[1..9].try_into().unwrap()));
        if source.is_some_and(|a|a.as_slice()==vault) { change -= n; }
        if destination.is_some_and(|a|a.as_slice()==vault) { change += n; }
    }
    i128::from(post)-i128::from(pre)==change
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::base58;
    use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

    #[test]
    fn finalized_current_spot_formats_bind_principals_and_reject_routed_bypasses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage/current-proprietary");
        let mut positives = 0;
        let mut invalid = 0;
        for line in include_str!("../fixtures/swap-coverage/current-proprietary/cases.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(root.join(format!("{}.pb", f[0]))).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let source: u32 = f[1].parse().unwrap();
            let source_ix = tx.walk_instructions().nth(source as usize).unwrap();
            let source_data = source_ix.data().clone();
            let parsed = layout(&source_ix).unwrap();
            let lookup = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            let calls: Vec<_> = tx.walk_instructions().skip(source as usize+1).take(2).collect();
            let successful: HashSet<u32> = (0..tx.walk_instructions().count() as u32).collect();
            assert!(settlement(source, &tx, &source_ix, &calls, &lookup, &successful).is_some(), "original native settlement {}", f[0]);
            let native = crate::process_transaction(tx.clone()).unwrap().swaps.into_iter()
                .find(|x| x.source_index == Some(source)).unwrap_or_else(|| panic!("finalized proprietary spot swap {} source={source}", f[0]));
            assert_eq!(native.protocol, parsed.protocol as i32);
            for (actual, expected) in [(&native.program_id, f[3]), (&native.amm, f[3]), (&native.amm_pool, f[4]),
                (&native.user, f[5]), (&native.input_mint, f[6]), (&native.output_mint, f[8])] {
                assert_eq!(base58::encode(actual), expected);
            }
            assert_eq!(native.input_amount.to_string(), f[7]);
            assert_eq!(native.output_amount.to_string(), f[9]);
            assert_eq!(native.source_transfer_index, Some(f[2].parse().unwrap()));
            assert!(native.transfer_verified);
            positives += 1;
            for mode in 0..15 {
                let mut wrong = tx.clone();
                if mode < 6 || mode == 11 || mode == 12 {
                    let modify = |data: &mut Vec<u8>, accounts: &mut Vec<u8>| match mode {
                        0 => data[if parsed.protocol == Protocol::Humidifi { 24 } else { 0 }] ^= 128,
                        1 => data[if parsed.protocol == Protocol::Humidifi { 8 } else if parsed.protocol == Protocol::Deriverse { 16 } else { 2 }] ^= 1,
                        2 => accounts.swap(parsed.users[0], parsed.vaults[0]),
                        3 => accounts.swap(parsed.pool, parsed.users[0]),
                        4 => data[if parsed.protocol == Protocol::Humidifi { 16 } else { 1 }] ^= 1,
                        5 => { data.pop(); },
                        11 => {
                            if parsed.protocol == Protocol::Humidifi { data[16] ^= 3; }
                            else { let offset = if parsed.protocol == Protocol::Deriverse { 24 } else { 10 }; data[offset..offset+8].fill(255); }
                        },
                        12 => {
                            let (program,mint) = match parsed.protocol { Protocol::Humidifi => (7,10),Protocol::Tesserav => (9,7),Protocol::GoonfiV2 => (11,6),_ => (13,1) };
                            accounts.swap(program,mint);
                        },
                        _ => unreachable!(),
                    };
                    let message = wrong.transaction.as_mut().unwrap().message.as_mut().unwrap();
                    if let Some(ix) = message.instructions.iter_mut().find(|x| x.data == source_data) {
                        modify(&mut ix.data, &mut ix.accounts);
                    } else {
                        let ix = wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                            .find(|x| x.data == source_data).unwrap();
                        modify(&mut ix.data, &mut ix.accounts);
                    }
                } else if mode == 6 || mode == 7 || mode == 10 || mode == 13 {
                    let target = source + 1 + calls.iter().position(|child| {
                        let a = child.accounts();
                        a[0].0 == source_ix.accounts()[if mode == 6 { parsed.users[0] } else { parsed.vaults[1] }].0
                    }).unwrap() as u32;
                    let roots = wrong.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions.len();
                    let mut flat = 0;
                    let mut changed = false;
                    for root_position in 0..roots {
                        flat += 1;
                        if let Some(group) = wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g| g.index == root_position as u32) {
                            for i in &mut group.instructions {
                                if flat == target {
                                    if mode == 6 || mode == 13 { i.data[1] ^= 1; } else if mode == 7 { i.accounts.swap(0, 1); }
                                    else { let last = i.accounts.len()-1; i.accounts[last] = i.accounts[1]; }
                                    changed = true;
                                }
                                flat += 1;
                            }
                        }
                    }
                    assert!(changed);
                } else if mode == 14 {
                    wrong.meta.as_mut().unwrap().post_token_balances.clear();
                } else if mode == 8 {
                    wrong.meta.as_mut().unwrap().log_messages.clear();
                } else {
                    let logs = &mut wrong.meta.as_mut().unwrap().log_messages;
                    let depth = if source_ix.is_root() { 1 } else { source_ix.stack_height() };
                    let ordinal = tx.walk_instructions().take(source as usize + 1).filter(|ix| ix.program_id().0 == source_ix.program_id().0
                        && (if ix.is_root() { 1 } else { ix.stack_height() }) == depth).count()-1;
                    let begin = logs.iter().enumerate().filter(|(_,log)| *log == &format!("Program {} invoke [{depth}]", f[3]))
                        .nth(ordinal).unwrap().0;
                    let end = logs.iter().enumerate().skip(begin+1).find(|(_,log)| *log == &format!("Program {} success", f[3])).unwrap().0;
                    logs[end] = format!("Program {} failed: custom program error: 1", f[3]);
                }
                assert!(!crate::process_transaction(wrong.clone()).unwrap().swaps.iter().any(|x| x.source_index == Some(source)),
                    "{} native mutation {mode}", f[0]);
                let lookup = TokenMintLookup::new(&wrong, wrong.meta.as_ref().unwrap());
                let mut routed = native.clone(); routed.protocol = 5;
                routed.program_id = substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                let mut rows = vec![routed];
                crate::quality::verify_and_deduplicate(&wrong, &lookup, &mut rows, false);
                assert!(rows.is_empty(), "{} routed mutation {mode}", f[0]);
                invalid += 2;
            }
            let lookup = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            for mode in 0..3 {
                let mut routed = native.clone(); routed.protocol = 5;
                routed.program_id = substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                if mode == 1 { routed.user = source_ix.accounts()[parsed.pool].0.clone(); }
                if mode == 2 { routed.amm_pool = source_ix.accounts()[parsed.users[0]].0.clone(); }
                let mut rows = vec![routed];
                crate::quality::verify_and_deduplicate(&tx, &lookup, &mut rows, false);
                if mode == 0 { assert_eq!(rows.len(), 1); assert_eq!(rows[0].protocol, parsed.protocol as i32); }
                else { assert!(rows.is_empty()); invalid += 1; }
            }
        }
        assert_eq!(positives, 86); assert_eq!(invalid, 2752);
        eprintln!("Current proprietary finalized positives={positives}; invalid native/routed cases={invalid}");
    }
}
