//! Official @hadron-fi/sdk-v2 0.1.9: SwapExactIn opcode3, 26B payload.
//! Typed fee and pool principals define the user's total input cost; quotes,
//! curve edits, oracle values and close_pool withdrawals never become prices.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{block_view::InstructionView};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("HADRoNbLovyqhCsocfYQYB7QdfCAAinN9HTePvBCVDQ8");

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a = ix.accounts();
    let d = ix.data();
    if ix.program_id().0.as_slice() != PROGRAM_ID || !(16..=20).contains(&a.len())
        || d.len() != 26 || d[0] != 3 || d[1] > 1
        || a[14].0.as_slice() != b58!("SysvarC1ock11111111111111111111111111111111")
        || [0, 1].iter().any(|i| ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&a[*i].0.as_slice()))
        || a[8..14].iter().enumerate().any(|(n, key)| a[8..8+n].iter().any(|other| key.0 == other.0))
        || a[2].0 == a[7].0
    { return None; }
    let amount = u64::from_le_bytes(d[2..10].try_into().ok()?);
    if amount == 0 { return None; }
    Some(Layout {
        protocol: Protocol::Hadron, pool: 2, authority: 7, users: [8, 11], vaults: [9, 10],
        amount, exact_input: true, input_limit: false, input_first: true, fixed_users: true,
        minimum_output: u64::from_le_bytes(d[10..18].try_into().ok()?),
    })
}

/// Return the pool-input and primary-output positions. An optional input fee
/// goes directly from the same user account to the declared recipient before
/// the pool principal. Both exact gross debits together must equal amountIn.
pub(crate) fn settlement(source: u32, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup, successful: &HashSet<u32>) -> Option<(u32, u32)> {
    let l = layout(ix)?;
    let a = ix.accounts();
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if depth == 0 || !matches!(calls.len(), 2 | 3) || !successful.contains(&source)
        || calls.iter().enumerate().any(|(n, c)| c.is_root() || c.stack_height() != depth + 1 || !successful.contains(&(source + n as u32 + 1)))
    { return None; }
    let input_mint = mints.mint_for(a[8].0)?;
    let output_mint = mints.mint_for(a[11].0)?;
    if input_mint == output_mint || mints.mint_for(a[9].0)? != input_mint || mints.mint_for(a[10].0)? != output_mint { return None; }
    let x = ix.data()[1] == 1;
    let mut input_total = 0u64;
    for (n, c) in calls.iter().enumerate() {
        let output = n == calls.len() - 1;
        let fee = calls.len() == 3 && n == 0;
        let (from, to, authority, mint, program) = if output {
            (10, 11, 2, &output_mint, if x { 1 } else { 0 })
        } else {
            (8, if fee { 13 } else { 9 }, 7, &input_mint, if x { 0 } else { 1 })
        };
        let d = c.data();
        let ca = c.accounts();
        let checked = d.first() == Some(&12);
        if c.program_id().0 != a[program].0 || d.len() != if checked { 10 } else { 9 }
            || d[0] != if checked { 12 } else { 3 } || ca.len() != if checked { 4 } else { 3 }
            || ca[0].0 != a[from].0 || ca[if checked { 2 } else { 1 }].0 != a[to].0 || ca[if checked { 3 } else { 2 }].0 != a[authority].0
            || mints.mint_for(a[to].0).as_ref() != Some(mint)
            || checked && (ca[1].0.as_slice() != mint.as_slice() || !a[16..].iter().any(|key| key.0.as_slice() == mint.as_slice()))
        { return None; }
        let amount = u64::from_le_bytes(d[1..9].try_into().ok()?);
        if amount == 0 { return None; }
        if output {
            if amount < l.minimum_output { return None; }
        } else {
            input_total = input_total.checked_add(amount)?;
        }
    }
    (input_total == l.amount).then_some((source + calls.len() as u32 - 1, source + calls.len() as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn finalized_hadron_prices_include_user_fee_and_exact_net_receipt() {
        let mut shapes = HashSet::new();
        for line in include_str!("../fixtures/swap-coverage/hadron-native/cases.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/swap-coverage/hadron-native/{}.pb", env!("CARGO_MANIFEST_DIR"), f[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let index = f[1].parse::<u32>().unwrap();
            let source = tx.walk_instructions().nth(index as usize).unwrap();
            let native_data = source.data().clone();
            let source_depth = source.stack_height();
            let decode = |tx| crate::process_transaction_with_metadata(tx, None, |_, _| None, |_, _| None, |mint, _| {
                let raw = std::fs::read(format!("{}/fixtures/swap-coverage/hadron-native/fee-mints/{}.bin", env!("CARGO_MANIFEST_DIR"), base58::encode(mint))).ok()?;
                let (schedule, initialized) = crate::token_fees::owned_mint(&token_2022::PROGRAM_ID, &raw)?;
                if !initialized { return None; }
                schedule.at(1055)
            });
            let decoded = decode(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(index)).collect();
            assert_eq!(swaps.len(), 1, "{} index {index}", f[0]);
            let swap = swaps[0];
            assert_eq!(swap.protocol, Protocol::Hadron);
            assert!(swap.transfer_verified);
            assert_eq!(base58::encode(&swap.amm_pool), f[2]);
            assert_eq!(base58::encode(&swap.user), f[3]);
            assert_eq!(base58::encode(&swap.input_mint), f[4]);
            assert_eq!(swap.input_amount, f[5].parse::<u64>().unwrap());
            assert_eq!(base58::encode(&swap.output_mint), f[6]);
            assert_eq!(swap.output_amount, f[7].parse::<u64>().unwrap());
            assert_eq!(swap.source_transfer_index, Some(f[8].parse::<u32>().unwrap()));
            let fee = f[12].parse::<u64>().unwrap();
            let first_child = tx.walk_instructions().nth(index as usize + 1).unwrap();
            let shape = (f[11].parse::<usize>().unwrap(), fee > 0, first_child.data()[0], native_data[1], source.is_root());
            if !shapes.insert(shape) { continue; }
            if source.is_root() {
                for mode in 0..14 {
                    let mut wrong = tx.clone();
                    let ix = wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().find(|ix| ix.data == native_data).unwrap();
                    let accounts = ix.accounts.clone();
                    match mode {
                        0 => ix.data[0] = 20,
                        1 => ix.data[1] = 2,
                        2 => ix.data[2] ^= 1,
                        3 => ix.data[2..10].fill(0),
                        4 => { ix.data.pop(); }
                        5 => ix.data.push(0),
                        6 => ix.data[10..18].copy_from_slice(&(swap.output_amount + 1).to_le_bytes()),
                        7 => ix.accounts[2] = accounts[7],
                        8 => ix.accounts[8] = accounts[11],
                        9 => ix.accounts[9] = accounts[10],
                        10 => ix.accounts[13] = accounts[9],
                        11 => ix.accounts[14] = accounts[2],
                        12 => ix.accounts[0] = accounts[2],
                        _ => ix.accounts.truncate(15),
                    }
                    assert!(decode(wrong).map_or(true, |d| d.swaps.iter().all(|s| s.source_index != Some(index))),
                        "{} index {index} invalid root Hadron {mode}", f[0]);
                }
                continue;
            }
            for mode in 0..25 {
                let mut wrong = tx.clone();
                let meta = wrong.meta.as_option_mut().unwrap();
                let (g, p) = meta.inner_instructions.iter().enumerate().find_map(|(g, xs)|
                    xs.instructions.iter().position(|ix| ix.data == native_data).map(|p| (g, p))).unwrap();
                let calls = &mut meta.inner_instructions[g].instructions;
                let accounts = calls[p].accounts.clone();
                let input = p + if fee > 0 { 2 } else { 1 };
                let output = input + 1;
                let output_checked = calls[output].data[0] == 12;
                match mode {
                    0 => calls[p].data[0] = 20,
                    1 => calls[p].data[1] = 2,
                    2 => calls[p].data[2] ^= 1,
                    3 => calls[p].data[2..10].fill(0),
                    4 => { calls[p].data.pop(); }
                    5 => calls[p].data.push(0),
                    6 => calls[p].data[10..18].copy_from_slice(&(swap.output_amount + 1).to_le_bytes()),
                    7 => calls[p].accounts[2] = accounts[7],
                    8 => calls[p].accounts[8] = accounts[11],
                    9 => calls[p].accounts[9] = accounts[10],
                    10 => calls[p].accounts[13] = accounts[9],
                    11 => calls[p].accounts[14] = accounts[2],
                    12 => calls[p].accounts[0] = accounts[2],
                    13 => calls[input].data[1] ^= 1,
                    14 => calls[input].accounts[0] = accounts[2],
                    15 => calls[input].accounts.last_mut().map(|key| *key = accounts[2]).unwrap(),
                    16 => calls[output].accounts[0] = accounts[9],
                    17 => calls[output].accounts[if output_checked { 2 } else { 1 }] = accounts[13],
                    18 => calls[output].accounts.last_mut().map(|key| *key = accounts[7]).unwrap(),
                    19 => calls[output].data[1..9].fill(0),
                    20 => calls[output].stack_height = Some(source_depth + 2),
                    21 => { let extra = calls[input].clone(); calls.insert(output + 1, extra); }
                    22 => meta.err = buffa::MessageField::some(Default::default()),
                    23 => {
                        let success = meta.log_messages.iter_mut().find(|log| **log == format!("Program {} success", base58::encode(PROGRAM_ID))).unwrap();
                        *success = format!("Program {} failed: custom program error: 1", base58::encode(PROGRAM_ID));
                    }
                    _ => calls[p].accounts.truncate(15),
                }
                assert!(decode(wrong).map_or(true, |d| d.swaps.iter().all(|s| s.source_index != Some(index))),
                    "{} index {index} invalid Hadron {mode}", f[0]);
            }
        }
    }
}
