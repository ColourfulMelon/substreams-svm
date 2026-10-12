//! Archer Swap (15) legacy SwapParams and scoped physical SyncFillEvent.
//! Native amount/threshold use lots. They are never treated as token atoms.
//! The pinned native fill binds them to exact committed physical principals.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use base64::{engine::general_purpose::STANDARD, Engine};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::{HashMap, HashSet};
use substreams_solana::{b58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("Archer8kgiavM61GyusMzaaS2ft5sALtNsD1HxkUPMhy");
const FILL: [u8; 8] = [59, 204, 114, 143, 34, 225, 40, 189];

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a = ix.accounts();
    let d = ix.data();
    // Source pin fa6128d1: fixed eleven accounts plus 1..64 maker books.
    // All audited legacy calls have no builder fee and zero threshold.
    if ix.program_id().0.as_slice() != PROGRAM_ID
        || !(12..=75).contains(&a.len())
        || d.len() != 19
        || d[0] != 15
        || d[17] > 1
        || d[18] != 0
        || d[9..17].iter().any(|x| *x != 0)
        || u64::from_le_bytes(d[1..9].try_into().ok()?) == 0
        || a[9..11]
            .iter()
            .any(|p| ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&p.0.as_slice()))
        || a[3].0 == a[4].0
        || a[5].0 == a[6].0
        || a[7].0 == a[8].0
        || a[0].0 == a[1].0
        || [5, 6].iter().any(|v| [7, 8].iter().any(|u| a[*v].0 == a[*u].0))
    {
        return None;
    }
    let bid = d[17] == 0;
    Some(Layout {
        protocol: Protocol::Archer,
        pool: 1,
        authority: 0,
        users: if bid { [8, 7] } else { [7, 8] },
        vaults: if bid { [6, 5] } else { [5, 6] },
        // No raw lot comparison: bound_layout substitutes the event's atoms.
        amount: u64::MAX,
        exact_input: true,
        input_limit: false,
        input_first: true,
        fixed_users: true,
        minimum_output: 0,
    })
}

// Match runtime frames to the instruction walk, exactly as commitment proof
// does. A child program cannot forge its parent's Program data receipt.
// Duplicate fill events, malformed traces and truncated tails stay unproven.
pub(crate) fn fills(tx: &ConfirmedTransaction) -> HashMap<u32, Vec<u8>> {
    let ix: Vec<_> = tx.walk_instructions().collect();
    let mut cursor = 0;
    let mut stack: Vec<(String, Option<u32>, bool)> = Vec::new();
    let mut fills = HashMap::new();
    let mut duplicates = HashSet::new();
    for log in tx.meta.as_ref().into_iter().flat_map(|m| &m.log_messages) {
        if log == "Log truncated" {
            break;
        }
        let words: Vec<_> = log.split_whitespace().collect();
        if words.first() == Some(&"Program") && words.len() >= 3 {
            if words[2] == "invoke" && words.len() == 4 {
                let Some(depth) = words[3].strip_prefix('[').and_then(|s| s.strip_suffix(']')).and_then(|s| s.parse::<u32>().ok()) else {
                    stack.clear();
                    continue;
                };
                let Ok(p) = substreams_solana::base58::decode(words[1]) else {
                    stack.clear();
                    continue;
                };
                let found = ix
                    .iter()
                    .enumerate()
                    .skip(cursor)
                    .find(|(_, v)| v.program_id().0 == &p && (if v.is_root() { 1 } else { v.stack_height() }) == depth)
                    .map(|(n, _)| n);
                let valid = found.is_some() && depth as usize == stack.len() + 1 && stack.iter().all(|v| v.2);
                if let Some(n) = found {
                    cursor = n + 1;
                }
                stack.push((words[1].into(), found.map(|n| n as u32), valid));
            } else if words[2] == "success" || words[2] == "failed:" {
                if stack.last().is_some_and(|v| v.0 == words[1]) {
                    stack.pop();
                } else {
                    stack.clear();
                }
            }
        }
        let Some((_, Some(source), true)) = stack.last() else { continue };
        if ix[*source as usize].program_id().0.as_slice() != PROGRAM_ID {
            continue;
        }
        let Some(encoded) = log.strip_prefix("Program data: ") else { continue };
        let Ok(data) = STANDARD.decode(encoded) else { continue };
        if !data.starts_with(&FILL) {
            continue;
        }
        if data.len() != 209 || fills.insert(*source, data).is_some() {
            duplicates.insert(*source);
        }
    }
    for source in duplicates {
        fills.remove(&source);
    }
    fills
}

pub(crate) fn bound_layout(ix: &InstructionView, event: &[u8]) -> Option<Layout> {
    let mut l = layout(ix)?;
    let a = ix.accounts();
    let d = ix.data();
    if event.len() != 209
        || !event.starts_with(&FILL)
        || event[8..40] != *a[0].0
        || event[40..72] != *a[1].0
        || event[72] != d[17]
        || event[73..105] != *a[l.vaults[0] - 2].0
        || event[105..137] != *a[l.vaults[1] - 2].0
        || event[137..145] != d[1..9]
        || event[145..153] != d[9..17]
        || event[169..201] != *a[2].0
        || event[201..209].iter().any(|x| *x != 0)
    {
        return None;
    }
    l.amount = u64::from_le_bytes(event[153..161].try_into().ok()?);
    if l.amount == 0 || u64::from_le_bytes(event[161..169].try_into().ok()?) == 0 {
        return None;
    }
    Some(l)
}

pub(crate) fn settlement(
    source: u32,
    ix: &InstructionView,
    calls: &[InstructionView],
    event: &[u8],
    mints: &TokenMintLookup,
    successful: &HashSet<u32>,
) -> bool {
    let Some(l) = bound_layout(ix, event) else { return false };
    let a = ix.accounts();
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if !successful.contains(&source)
        || calls.len() != 2
        || calls
            .iter()
            .enumerate()
            .any(|(n, c)| c.stack_height() != depth + 1 || !successful.contains(&(source + 1 + n as u32)))
    {
        return false;
    }
    for (n, c) in calls.iter().enumerate() {
        let p = a[l.vaults[n] + 4].0;
        let checked = p.as_slice() == token_2022::PROGRAM_ID;
        let ca = c.accounts();
        let d = c.data();
        let mint = a[l.vaults[n] - 2].0;
        let (src, dst, authority) = if n == 0 { (l.users[0], l.vaults[0], 0) } else { (l.vaults[1], l.users[1], 1) };
        if c.program_id().0 != p
            || d.len() != if checked { 10 } else { 9 }
            || d[0] != if checked { 12 } else { 3 }
            || ca.len() != if checked { 4 } else { 3 }
            || ca[0].0 != a[src].0
            || ca[if checked { 2 } else { 1 }].0 != a[dst].0
            || ca[if checked { 3 } else { 2 }].0 != a[authority].0
            || mints.mint_for(a[src].0).as_deref() != Some(mint.as_slice())
            || mints.mint_for(a[dst].0).as_deref() != Some(mint.as_slice())
            || checked && ca[1].0 != mint
            || d[1..9] != event[153 + n * 8..161 + n * 8]
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finalized_legacy_lots_bind_native_fill_and_physical_principals() {
        for (bytes, source, input, output) in [
            (
                include_bytes!("../fixtures/swap-coverage/archer/4sDhFLbGeY3JEMXuJpDBUJNrePEFbAhJb8eGYwyjQkP2JEU7EXyXLBbqNR9vEq3tyf6xoZ3KZgatJiWzkYvQFE6H.pb")
                    .as_slice(),
                9u32,
                160011448u64,
                1459000000u64,
            ),
            (
                include_bytes!("../fixtures/swap-coverage/archer/5aaMw8wa1G99Z8Y5U273biRLTndnvvrQ81WMr5VNjUXD49fmMQS8QoJAz8QTDecUEpjmVozGAkutUH6FAgi6nxqn.pb")
                    .as_slice(),
                2u32,
                2125000000u64,
                233108250u64,
            ),
            (
                include_bytes!("../fixtures/swap-coverage/archer/3ivbQWmdW3pxKP4HPbiTZpsuGo32onvvfFM8kak3p2yJ9QTZ3eakVosBp7wXVAzPR64WEFCCw43LL99ZnUvyx4En.pb")
                    .as_slice(),
                4u32,
                251701830u64,
                2295000000u64,
            ),
            (
                include_bytes!("../fixtures/swap-coverage/archer/2okDa7q4EYCMzKZA7ny1SNWbBuuxhfEd3akXHqDG7FHi5c7j5zUKDLJiqraXudxvEwc9dszZXk5sGJWNAa22vtAM.pb")
                    .as_slice(),
                6u32,
                1914000000u64,
                209979198u64,
            ),
            (
                include_bytes!("../fixtures/swap-coverage/archer/RyqxHRNvShmVUShq9U18Y3MCMMrYmnR3gRnbBGDa5VMYKQqWukBwqaDTuJwH4uDcSxCh6LThJG1cUZG7caDtEr3.pb")
                    .as_slice(),
                4u32,
                3000000u64,
                3000000u64,
            ),
            (
                include_bytes!("../fixtures/swap-coverage/archer/3UyXfgYG1w8pd76uxpUfw7deXSQCUSJqQqgt7Es9CnKbLro6RYaQZjFJauhAEsrkV46jgzxTqRzNKwqS92cKgjSY.pb")
                    .as_slice(),
                2u32,
                27497008u64,
                169400u64,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let views: Vec<_> = tx.walk_instructions().collect();
            let ix = &views[source as usize];
            let native = ix.data().clone();
            let depth = ix.stack_height();
            let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            let event = fills(&tx).remove(&source).unwrap();
            assert_eq!(bound_layout(ix, &event).unwrap().amount, input);
            assert!(settlement(
                source,
                ix,
                &views[source as usize + 1..source as usize + 3],
                &event,
                &mints,
                &crate::logs::successful_invocations(&tx)
            ));
            for mode in 0..29 {
                let mut altered = tx.clone();
                if mode < 17 {
                    let group = altered
                        .meta
                        .as_mut()
                        .unwrap()
                        .inner_instructions
                        .iter_mut()
                        .find(|g| g.instructions.iter().any(|i| i.data == native))
                        .unwrap();
                    let p = group.instructions.iter().position(|i| i.data == native).unwrap();
                    let a = group.instructions[p].accounts.clone();
                    let l = layout(ix).unwrap();
                    let calls = &mut group.instructions;
                    let checked = calls[p + 2].data[0] == 12;
                    let input_checked = calls[p + 1].data[0] == 12;
                    match mode {
                        0 => {}
                        1 => calls[p].data[0] = 14,
                        2 => calls[p].data.push(0),
                        3 => calls[p].data[1] ^= 1,
                        4 => calls[p].data[9] = 1,
                        5 => calls[p].data[17] = 2,
                        6 => calls[p].data[18] = 1,
                        7 => calls[p + 1].accounts[0] = a[1],
                        8 => calls[p + 1].accounts[if input_checked { 2 } else { 1 }] = a[0],
                        9 => calls[p + 1].data[1] ^= 1,
                        10 => calls[p + 2].accounts[0] = a[l.vaults[0]],
                        11 => calls[p + 2].accounts[if checked { 2 } else { 1 }] = a[0],
                        12 => calls[p + 2].accounts[if checked { 3 } else { 2 }] = a[0],
                        13 => calls[p + 2].data[1] ^= 1,
                        14 => calls[p + 1].stack_height = Some(depth + 2),
                        15 => {
                            let extra = calls[p + 1].clone();
                            calls.insert(p + 2, extra);
                        }
                        _ => calls[p].accounts[9] = a[0],
                    }
                } else {
                    let logs = &mut altered.meta.as_mut().unwrap().log_messages;
                    let n = logs
                        .iter()
                        .position(|l| l.starts_with("Program data: ") && STANDARD.decode(&l[14..]).is_ok_and(|b| b.starts_with(&FILL)))
                        .unwrap();
                    let mut e = STANDARD.decode(&logs[n][14..]).unwrap();
                    match mode {
                        17..=24 => e[[8, 40, 72, 73, 105, 137, 153, 161][mode - 17]] ^= 1,
                        25 => e[201] = 1,
                        26 => {
                            let duplicate = logs[n].clone();
                            logs.insert(n + 1, duplicate);
                        }
                        27 => {
                            let moved = logs.remove(n);
                            let child = logs.iter().position(|l| l.contains("Token") && l.contains("invoke")).unwrap();
                            logs.insert(child + 1, moved);
                        }
                        _ => {
                            let success = format!("Program {} success", substreams_solana::base58::encode(&PROGRAM_ID));
                            let end = logs.iter().position(|l| l == &success).unwrap();
                            logs[end] = success.replace(" success", " failed: custom program error: 1");
                        }
                    }
                    if (17..=25).contains(&mode) {
                        logs[n] = format!("Program data: {}", STANDARD.encode(e));
                    }
                }
                let decoded = crate::process_transaction_with_metadata(
                    altered,
                    None,
                    |_, _| None,
                    |_, _| None,
                    |mint, _| {
                        let data = match substreams_solana::base58::encode(mint).as_str() {
                            "SKHYhSjuRWHgikq8eRKbtBbpABgJSkd7ytQV14i9EQ3" => {
                                include_bytes!("../fixtures/swap-coverage/archer/fee-mints/SKHYhSjuRWHgikq8eRKbtBbpABgJSkd7ytQV14i9EQ3.bin").as_slice()
                            }
                            "USDvUSpnhCr9yBgj3UyVrD239HRUv4RsHwH2FxsWuMk" => {
                                include_bytes!("../fixtures/swap-coverage/archer/fee-mints/USDvUSpnhCr9yBgj3UyVrD239HRUv4RsHwH2FxsWuMk.bin").as_slice()
                            }
                            _ => return None,
                        };
                        let (fees, initialized) = crate::token_fees::owned_mint(&token_2022::PROGRAM_ID, data)?;
                        assert!(initialized);
                        fees.at(1055)
                    },
                )
                .unwrap();
                let found: Vec<_> = decoded
                    .swaps
                    .iter()
                    .filter(|s| s.source_index == Some(source) && s.amm.as_slice() == PROGRAM_ID)
                    .collect();
                if mode == 0 {
                    assert_eq!(found.len(), 1);
                    let s = found[0];
                    assert_eq!((s.input_amount, s.output_amount), (input, output));
                    assert!(s.transfer_verified);
                    assert_eq!(s.protocol, Protocol::Archer as i32);
                    assert_eq!(s.source_transfer_index, Some(source + 1));
                    assert_eq!(s.amm_pool, *ix.accounts()[1].0);
                    assert_eq!(s.user, *ix.accounts()[0].0);
                    assert!(decoded.coverage_candidates.iter().all(|c| c.source_index != source));
                } else {
                    assert!(found.is_empty(), "unproven Archer mode {mode} source {source}");
                }
            }
        }
    }
}
