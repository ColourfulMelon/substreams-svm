//! Bind normalized swaps to successful on-chain instruction/transfer evidence.
//! Amount similarity is never enough: both exact token legs and the AMM/pool
//! invocation must match. Unverifiable rows are retained as diagnostics.
use std::collections::{HashMap, HashSet};

use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::{
    meteora::amm,
    pumpfun::bonding_curve as pumpfun,
    pumpswap,
    spl::{token, token_2022},
};

use crate::token_mints::TokenMintLookup;

struct Transfer {
    index: u32,
    depth: u32,
    mint: Vec<u8>,
    amounts: Vec<u64>,
    source: Vec<u8>,
    destination: Vec<u8>,
    authority: Vec<u8>,
    fee_token: bool,
    net_verified: bool,
}

pub(crate) fn verify_and_deduplicate(tx: &ConfirmedTransaction, mints: &TokenMintLookup, swaps: &mut Vec<pb::Swap>, expand_native: bool) -> Vec<pb::Swap> {
    let instructions: Vec<_> = tx.walk_instructions().collect();
    // A successful transaction may catch a failed CPI. Its completed inner
    // transfers roll back too, so require runtime success for all evidence
    // whenever the transaction contains a caught invocation failure.
    let caught_failure = tx.meta.as_ref().is_some_and(|meta| {
        meta.log_messages.iter().any(|log| {
            let mut words = log.split_whitespace();
            words.next() == Some("Program") && words.next().is_some() && words.next() == Some("failed:")
        })
    });
    let successful_invocations = if caught_failure
        || instructions
            .iter()
            .any(|ix| ix.program_id().0 == &token_2022::PROGRAM_ID && ix.data().starts_with(&[26, 3]))
    {
        crate::logs::successful_invocations(tx)
    } else {
        HashSet::new()
    };
    let mut transfers = Vec::new();
    let mut other_token_operations = HashSet::new();
    let mut initializations = HashMap::<Vec<u8>, usize>::new();
    let mut closes = HashMap::<Vec<u8>, usize>::new();
    let mut burns = HashMap::<(Vec<u8>, Vec<u8>), i128>::new();
    for (index, ix) in instructions.iter().enumerate() {
        if caught_failure && !successful_invocations.contains(&(index as u32)) {
            continue;
        }
        let program = ix.program_id();
        let accounts = ix.accounts();
        let data = ix.data();
        let parsed = if program.0 == &token::PROGRAM_ID || program.0 == &token_2022::PROGRAM_ID {
            match data.first() {
                Some(3) if data.len() == 9 && accounts.len() >= 2 => mints.mint_for(accounts[0].0).map(|mint| (mint, 1, 0, 1)),
                Some(12) if data.len() == 10 && accounts.len() >= 3 => Some((accounts[1].0.clone(), 1, 0, 2)),
                // Token-2022 TransferFeeExtension::TransferCheckedWithFee.
                Some(26) if data.len() == 19 && data[1] == 1 && accounts.len() >= 3 => Some((accounts[1].0.clone(), 2, 0, 2)),
                _ => None,
            }
        } else if program.0.iter().all(|byte| *byte == 0) && data.len() == 12 && data[..4] == [2, 0, 0, 0] && accounts.len() >= 2 {
            Some((base58::decode("So11111111111111111111111111111111111111112").unwrap(), 4, 0, 1))
        } else {
            None
        };
        if (program.0 == &token::PROGRAM_ID || program.0 == &token_2022::PROGRAM_ID) && !accounts.is_empty() {
            if data.as_slice() == [1] || (matches!(data.first(), Some(16 | 18)) && data.len() == 33) {
                *initializations.entry(accounts[0].0.clone()).or_default() += 1;
            } else if data.as_slice() == [9] && accounts.len() >= 3 {
                *closes.entry(accounts[0].0.clone()).or_default() += 1;
            }
        }
        let is_burn = (program.0 == &token::PROGRAM_ID || program.0 == &token_2022::PROGRAM_ID)
            && ((data.first() == Some(&8) && data.len() == 9) || (data.first() == Some(&15) && data.len() == 10))
            && accounts.len() >= 3
            && mints.mint_for(accounts[0].0).as_ref() == Some(accounts[1].0);
        if is_burn {
            *burns.entry((accounts[0].0.clone(), accounts[1].0.clone())).or_default() += i128::from(u64::from_le_bytes(data[1..9].try_into().unwrap()));
        }
        // Owner/delegate/freeze changes do not mint, burn or debit tokens.
        // Validate their actual SPL encodings before excluding them from the
        // balance-conservation blocker set.
        let balance_preserving = match data.first() {
            Some(4) => data.len() == 9 && accounts.len() >= 3,
            Some(5) => data.len() == 1 && accounts.len() >= 2,
            Some(6) => accounts.len() >= 2 && matches!(data.get(1), Some(2 | 3)) && ((data.len() == 3 && data[2] == 0) || (data.len() == 35 && data[2] == 1)),
            Some(10 | 11) => data.len() == 1 && accounts.len() >= 3,
            Some(13) => data.len() == 10 && accounts.len() >= 4,
            _ => false,
        };
        if parsed.is_none()
            && !balance_preserving
            && !is_burn
            && data.as_slice() != [9]
            && !matches!(data.first(), Some(1 | 16 | 18 | 21 | 22))
            && (program.0 == &token::PROGRAM_ID || program.0 == &token_2022::PROGRAM_ID)
        {
            for account in &accounts {
                other_token_operations.insert(account.0.clone());
            }
        }
        if let Some((mint, offset, source, destination)) = parsed {
            let amount = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            if amount > 0 {
                let mut amounts = vec![amount];
                if program.0 == &token_2022::PROGRAM_ID && data.first() == Some(&26) {
                    let fee = u64::from_le_bytes(data[11..19].try_into().unwrap());
                    if fee > amount {
                        continue;
                    }
                    if let Some(net) = amount.checked_sub(fee) {
                        if net != amount {
                            amounts.push(net);
                        }
                    }
                }
                transfers.push(Transfer {
                    index: index as u32,
                    depth: ix.stack_height(),
                    mint,
                    amounts,
                    source: accounts[source].0.clone(),
                    destination: accounts[destination].0.clone(),
                    authority: accounts.get(destination + 1).map(|a| a.0.clone()).unwrap_or_default(),
                    fee_token: program.0 == &token_2022::PROGRAM_ID,
                    net_verified: program.0 != &token_2022::PROGRAM_ID || data.first() == Some(&26),
                });
            }
        }
    }

    // An authorized self-harvest credits this transfer's withheld fee back to
    // its receiver. Its economic receipt is exactly the gross transfer, even
    // if older withheld fees are collected too (those are not swap proceeds).
    // A prior harvest, close or reinitialization can remove those fees first;
    // require the first such operation to be the actual refund.
    for transfer in &mut transfers {
        if !transfer.fee_token {
            continue;
        }
        let refund = instructions.iter().enumerate().skip(transfer.index as usize + 1).find(|(_, ix)| {
            if ix.program_id().0 != &token_2022::PROGRAM_ID || !ix.accounts().iter().any(|account| account.0 == &transfer.destination) {
                return false;
            }
            let data = ix.data();
            (data.first() == Some(&26) && data.get(1) != Some(&1)) || matches!(data.first(), Some(1 | 9 | 16 | 18))
        });
        let refunded = refund.is_some_and(|(index, ix)| {
            if !successful_invocations.contains(&(index as u32)) {
                return false;
            }
            let data = ix.data();
            let accounts = ix.accounts();
            if data.len() != 3 || data[..2] != [26, 3] || data[2] == 0 || accounts.len() < 3 + data[2] as usize {
                return false;
            }
            accounts[0].0 == &transfer.mint
                && accounts[1].0 == &transfer.destination
                && accounts[accounts.len() - data[2] as usize..]
                    .iter()
                    .any(|account| account.0 == &transfer.destination)
        });
        if refunded {
            transfer.net_verified = true;
            transfer.amounts.truncate(1);
        }
    }

    // One incoming transfer plus known outgoing debits proves the exact
    // receipt, even when a router forwards tokens or a pool pays a fee later.
    // Mint/burn/close and other operations make this evidence unavailable.
    let mut receipt_counts = HashMap::<(Vec<u8>, Vec<u8>), usize>::new();
    let mut gross_receipts = HashMap::<(Vec<u8>, Vec<u8>), i128>::new();
    let mut known_receipts = HashMap::<(Vec<u8>, Vec<u8>), i128>::new();
    let mut debits = burns;
    for transfer in &transfers {
        let key = (transfer.destination.clone(), transfer.mint.clone());
        if transfer.net_verified || transfer.mint == crate::SOL_MINT {
            *known_receipts.entry(key).or_default() += i128::from(*transfer.amounts.iter().min().unwrap());
        } else {
            *receipt_counts.entry(key.clone()).or_default() += 1;
            *gross_receipts.entry(key).or_default() += i128::from(transfer.amounts[0]);
        }
        *debits.entry((transfer.source.clone(), transfer.mint.clone())).or_default() += i128::from(transfer.amounts[0]);
    }
    let mut deltas = HashMap::<Vec<u8>, i128>::new();
    let mut observed_balances = HashSet::new();
    let accounts = tx.resolved_accounts();
    if let Some(meta) = &tx.meta {
        for (balances, sign) in [(&meta.pre_token_balances, -1i128), (&meta.post_token_balances, 1)] {
            for balance in balances {
                if let (Some(account), Some(amount)) = (accounts.get(balance.account_index as usize), balance.ui_token_amount.as_ref()) {
                    if let Ok(value) = amount.amount.parse::<u64>() {
                        *deltas.entry((*account).clone()).or_default() += sign * i128::from(value);
                        observed_balances.insert((*account).clone());
                    }
                }
            }
        }
    }
    // A temporary account created once and closed once has zero starting and
    // ending spendable tokens. Its outgoing debits prove a unique receipt,
    // including Token-2022 fees and explicit burns; mints and unknown
    // extension operations still block it.
    for (account, count) in &closes {
        if *count == 1 && initializations.get(account) == Some(&1) && !observed_balances.contains(account) {
            deltas.insert(account.clone(), 0);
        } else {
            other_token_operations.insert(account.clone());
        }
    }
    for transfer in &mut transfers {
        let account = &transfer.destination;
        if transfer.fee_token && !transfer.net_verified && transfer.mint != crate::SOL_MINT && !other_token_operations.contains(account) {
            let key = (account.clone(), transfer.mint.clone());
            // Fees are nonnegative. Exact conservation of all gross receipts
            // proves every receipt had zero withheld fees, even on repeated
            // routed hops. Otherwise only a single incoming receipt is solved.
            let received = deltas
                .get(account)
                .map(|delta| delta + debits.get(&key).unwrap_or(&0) - known_receipts.get(&key).unwrap_or(&0));
            let net = if received == gross_receipts.get(&key).copied() {
                Some(transfer.amounts[0])
            } else if receipt_counts.get(&key) == Some(&1) {
                received
                    .and_then(|net| u64::try_from(net).ok())
                    .filter(|net| *net > 0 && *net <= transfer.amounts[0])
            } else {
                None
            };
            if let Some(net) = net {
                transfer.net_verified = true;
                if !transfer.amounts.contains(&net) {
                    transfer.amounts.push(net);
                }
            }
        }
    }

    let mut rejected = Vec::new();
    if expand_native {
        // New native venues are normalized from recognized instruction layouts
        // and exact, immediate token CPIs. Never synthesize a trade from arbitrary
        // same-program transfers (deposit, withdraw, lending, quote update).
        for (index, ix) in instructions.iter().enumerate() {
            if caught_failure && !successful_invocations.contains(&(index as u32)) {
                continue;
            }
            let Some(layout) = crate::native_venues::layout(ix) else {
                continue;
            };
            let depth = if ix.is_root() { 1 } else { ix.stack_height() };
            if depth == 0 {
                continue;
            }
            let end = instructions
                .iter()
                .enumerate()
                .skip(index + 1)
                .find(|(_, child)| child.is_root() || (child.stack_height() > 0 && child.stack_height() <= depth))
                .map(|(end, _)| end as u32)
                .unwrap_or(instructions.len() as u32);
            let accounts = ix.accounts();
            let pool = accounts[layout.pool].0;
            let user = accounts[layout.authority].0;
            let scoped: Vec<_> = transfers
                .iter()
                .filter(|t| {
                    if t.index <= index as u32 || t.index >= end {
                        return false;
                    }
                    if t.depth == depth + 1 {
                        return true;
                    }
                    // Stabble V2 pays output through its declared vault program.
                    // Accept only that immediate withdrawal's Token CPI, with
                    // the audited program/opcode, never arbitrary descendants.
                    layout.protocol == pb::Protocol::Stabble
                        && t.depth == depth + 2
                        && accounts
                            .get(12)
                            .is_some_and(|program| program.0.as_slice() == substreams_solana::b58!("vo1tWgqZMjG61Z2T9qUaMYKqZ75CYzMuaZ2LZP1n7HV"))
                        && instructions[..t.index as usize]
                            .iter()
                            .rev()
                            .find(|child| (if child.is_root() { 1 } else { child.stack_height() }) < t.depth)
                            .is_some_and(|parent| {
                                parent.program_id().0 == accounts[12].0
                                    && parent.data().len() == 24
                                    && parent.data().starts_with(&[242, 80, 163, 0, 196, 221, 194, 194])
                            })
                })
                .collect();
            let mut candidates = Vec::new();
            for reverse in [false, true] {
                if reverse && layout.input_first {
                    continue;
                }
                let a = usize::from(reverse);
                let b = 1 - a;
                let source = accounts[layout.users[if layout.fixed_users { 0 } else { a }]].0;
                let destination = accounts[layout.users[if layout.fixed_users { 1 } else { b }]].0;
                let vault_in = accounts[layout.vaults[a]].0;
                let vault_out = accounts[layout.vaults[b]].0;
                if source == destination || source == vault_in || destination == vault_out || vault_in == vault_out {
                    continue;
                }
                let inputs: Vec<_> = scoped
                    .iter()
                    .filter(|t| &t.source == source && &t.destination == vault_in && &t.authority == user)
                    .collect();
                let outputs: Vec<_> = scoped.iter().filter(|t| &t.source == vault_out && &t.destination == destination).collect();
                if inputs.len() != 1 || outputs.len() != 1 {
                    continue;
                }
                let (input, output) = (inputs[0], outputs[0]);
                if layout.protocol == pb::Protocol::Onedex {
                    // The only other direct transfers are optional fees from
                    // the input vault to the two declared recipients. They
                    // already belong to the user's gross input, and must not
                    // be added again or mistaken for output liquidity.
                    let fees: Vec<_> = scoped.iter().filter(|t| t.index != input.index && t.index != output.index).collect();
                    if output.authority.as_slice() != accounts[2].0.as_slice()
                        || fees.len() > 2
                        || fees.iter().any(|t| {
                            t.source != input.destination
                                || t.mint != input.mint
                                || t.authority.as_slice() != accounts[2].0.as_slice()
                                || (t.destination.as_slice() != accounts[8].0.as_slice() && t.destination.as_slice() != accounts[9].0.as_slice())
                        })
                        || (fees.len() == 2 && fees[0].destination == fees[1].destination)
                        || fees
                            .iter()
                            .try_fold(0u64, |total, t| total.checked_add(t.amounts[0]))
                            .map_or(true, |total| total >= input.amounts[0])
                    {
                        continue;
                    }
                }
                let input_amount = if layout.protocol == pb::Protocol::Invariant {
                    // Its optional referral is paid directly from the same
                    // user account. Include that debit in the user's price,
                    // without treating the referral recipient as a pool.
                    let debits: Vec<_> = scoped.iter().filter(|t| t.source == input.source && t.mint == input.mint).collect();
                    if debits.len() > 2
                        || debits.iter().any(|t| {
                            t.authority != input.authority
                                || (t.destination != input.destination && !accounts[10..].iter().any(|account| account.0 == &t.destination))
                        })
                    {
                        continue;
                    }
                    let Some(total) = debits.iter().try_fold(0u64, |total, t| total.checked_add(t.amounts[0])) else {
                        continue;
                    };
                    total
                } else {
                    input.amounts[0]
                };

                if input.mint == output.mint
                    || mints.mint_for(vault_in).as_ref() != Some(&input.mint)
                    || mints.mint_for(vault_out).as_ref() != Some(&output.mint)
                    || mints.mint_for(source).as_ref() != Some(&input.mint)
                    || mints.mint_for(destination).as_ref() != Some(&output.mint)
                    || (layout.protocol == pb::Protocol::Defituna
                        && (mints.mint_for(accounts[9].0).as_ref() != Some(accounts[5].0) || mints.mint_for(accounts[10].0).as_ref() != Some(accounts[6].0)))
                    || (layout.exact_input && !layout.input_limit && input_amount != layout.amount)
                    || ((!layout.exact_input || layout.input_limit) && input_amount > layout.amount)
                    || *output.amounts.iter().min().unwrap() < layout.minimum_output
                    || (output.fee_token && output.mint != crate::SOL_MINT && !output.net_verified)
                {
                    continue;
                }
                candidates.push(pb::Swap {
                    protocol: layout.protocol as i32,
                    program_id: ix.program_id().0.clone(),
                    stack_height: depth,
                    amm: ix.program_id().0.clone(),
                    amm_pool: pool.clone(),
                    user: user.clone(),
                    input_mint: input.mint.clone(),
                    input_amount,
                    output_mint: output.mint.clone(),
                    output_amount: *output.amounts.iter().min().unwrap(),
                    source_index: Some(index as u32),
                    source_transfer_index: Some(input.index.min(output.index)),
                    ..Default::default()
                });
            }
            if candidates.len() == 1 {
                let native = candidates.pop().unwrap();
                if let Some(existing) = swaps.iter_mut().find(|swap| {
                    swap.source_index.is_none()
                        && swap.protocol == native.protocol
                        && swap.amm_pool == native.amm_pool
                        && swap.user == native.user
                        && swap.input_mint == native.input_mint
                        && swap.output_mint == native.output_mint
                }) {
                    if (existing.input_amount, existing.output_amount) != (native.input_amount, native.output_amount) {
                        let mut original = existing.clone();
                        original.source_index = native.source_index;
                        original.source_transfer_index = native.source_transfer_index;
                        original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                        rejected.push(original);
                    }
                    *existing = native;
                } else {
                    swaps.push(native);
                }
            }
        }

        // Pump's multi-hop instructions expose five accounts per real venue.
        // Intermediate coins travel vault-to-vault, bypassing user ATAs. Preserve
        // every pool leg, including transfers executed by the next curve's CPI.
        for (index, ix) in instructions.iter().enumerate() {
            if caught_failure && !successful_invocations.contains(&(index as u32)) {
                continue;
            }
            let data = ix.data();
            let accounts = ix.accounts();
            let is_curve = ix.program_id().0 == &pumpfun::PROGRAM_ID && data.starts_with(&[225, 154, 117, 22, 215, 86, 246, 103]);
            let is_pool = ix.program_id().0 == &pumpswap::PROGRAM_ID && data.starts_with(&[43, 100, 73, 19, 233, 246, 111, 148]);
            if !is_curve && !is_pool {
                continue;
            }
            let fixed = if is_curve { 13 } else { 16 };
            if accounts.len() < fixed + 5 || (accounts.len() - fixed) % 5 != 0 {
                continue;
            }
            if is_pool && data.len() != 24 {
                continue;
            }
            if is_curve {
                let Some(length) = data.get(16..20).and_then(|bytes| bytes.try_into().ok()).map(u32::from_le_bytes) else {
                    continue;
                };
                if data.len() != 20 + length as usize || length as usize != (accounts.len() - fixed) / 5 {
                    continue;
                }
            }
            let Some(amount) = data
                .get(8..16)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u64::from_le_bytes)
                .filter(|amount| *amount > 0)
            else {
                continue;
            };
            let (user, source, destination, authority) = if is_curve { (3, 5, 6, 4) } else { (0, 1, 2, 0) };
            let Some(mut current_mint) = mints.mint_for(accounts[source].0) else {
                continue;
            };
            let depth = if ix.is_root() { 1 } else { ix.stack_height() };
            let end = instructions
                .iter()
                .enumerate()
                .skip(index + 1)
                .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
                .map(|(end, _)| end as u32)
                .unwrap_or(instructions.len() as u32);
            let scoped: Vec<_> = transfers.iter().filter(|t| t.index > index as u32 && t.index < end).collect();
            let groups: Vec<_> = accounts[fixed..].chunks_exact(5).collect();
            let mut path = Vec::new();
            for group in &groups {
                let buy = if current_mint.as_slice() == group[1].0 {
                    true
                } else if current_mint.as_slice() == group[0].0 {
                    false
                } else {
                    break;
                };
                let (input_vault, output_vault, output_mint) = if buy {
                    (group[4].0, group[3].0, group[0].0)
                } else {
                    (group[3].0, group[4].0, group[1].0)
                };
                path.push((current_mint.clone(), output_mint.clone(), input_vault.clone(), output_vault.clone()));
                current_mint = output_mint.clone();
            }
            if path.len() != groups.len() || mints.mint_for(accounts[destination].0).as_ref() != Some(&current_mint) {
                continue;
            }
            for (hop, group) in groups.iter().enumerate() {
                // The child curve emits its own physical leg. Parent pool hops
                // must not be labelled as curves or duplicate the child venue.
                if is_pool
                    && instructions[index + 1..end as usize].iter().any(|child| {
                        child.program_id().0 == &pumpfun::PROGRAM_ID
                            && child.data().starts_with(&[225, 154, 117, 22, 215, 86, 246, 103])
                            && child
                                .accounts()
                                .get(13..)
                                .is_some_and(|groups| groups.chunks_exact(5).any(|accounts| accounts[2].0 == group[2].0))
                    })
                {
                    continue;
                }
                let (input_mint, output_mint, vault_in, vault_out) = &path[hop];
                let source = if hop == 0 { accounts[source].0 } else { &path[hop - 1].3 };
                let destination = if hop + 1 == path.len() { accounts[destination].0 } else { &path[hop + 1].2 };
                let inputs: Vec<_> = scoped
                    .iter()
                    .filter(|t| {
                        &t.source == source
                            && &t.destination == vault_in
                            && &t.mint == input_mint
                            && (hop > 0 || t.authority.as_slice() == accounts[authority].0.as_slice())
                    })
                    .collect();
                let outputs: Vec<_> = scoped
                    .iter()
                    .filter(|t| &t.source == vault_out && &t.destination == destination && &t.mint == output_mint)
                    .collect();
                if inputs.len() != 1 || outputs.len() != 1 {
                    continue;
                }
                let (input, output) = (inputs[0], outputs[0]);
                if (hop == 0 && input.amounts[0] != amount)
                    || (output.fee_token && !output.net_verified)
                    || mints.mint_for(vault_in).as_ref() != Some(input_mint)
                    || mints.mint_for(vault_out).as_ref() != Some(output_mint)
                {
                    continue;
                }
                swaps.push(pb::Swap {
                    protocol: if is_curve { pb::Protocol::Pumpfun } else { pb::Protocol::PumpfunAmm } as i32,
                    program_id: ix.program_id().0.clone(),
                    amm: ix.program_id().0.clone(),
                    amm_pool: group[2].0.clone(),
                    user: accounts[user].0.clone(),
                    stack_height: depth,
                    input_mint: input_mint.clone(),
                    output_mint: output_mint.clone(),
                    input_amount: input.amounts[0],
                    output_amount: *output.amounts.iter().min().unwrap(),
                    source_index: Some(index as u32),
                    source_transfer_index: Some(input.index.min(output.index)),
                    ..Default::default()
                });
            }
        }
    }
    let mut verified = Vec::new();
    let mut native_evidence = HashSet::new();
    // Prefer a native event over its Jupiter wrapper, including when wrapper
    // amounts reflect a transfer fee. Never deduplicate by amount alone.
    let mut pending = std::mem::take(swaps);
    pending.sort_by_key(|swap| matches!(swap.protocol, 4 | 5));
    for mut swap in pending {
        let expected_source = swap.source_index;
        let expected_transfer = swap.source_transfer_index;
        swap.transfer_verified = false;
        swap.source_index = None;
        swap.source_transfer_index = None;
        swap.verification_failure.clear();
        let mut evidence = None;
        if swap.input_amount > 0
            && swap.output_amount > 0
            && swap.input_mint.len() == 32
            && swap.output_mint.len() == 32
            && swap.input_mint != swap.output_mint
            && swap.amm.len() == 32
            && swap.amm_pool.len() == 32
        {
            'candidate: for (index, ix) in instructions.iter().enumerate() {
                if caught_failure && !successful_invocations.contains(&(index as u32)) {
                    continue;
                }
                if expected_source.is_some_and(|source| source != index as u32) {
                    continue;
                }
                let accounts = ix.accounts();
                if ix.program_id().0 != &swap.amm || !accounts.iter().any(|account| account.0 == &swap.amm_pool) {
                    continue;
                }
                let depth = if ix.is_root() { 1 } else { ix.stack_height() };
                if depth == 0 {
                    continue;
                }
                let end = instructions
                    .iter()
                    .enumerate()
                    .skip(index + 1)
                    .find(|(_, child)| child.is_root() || (child.stack_height() > 0 && child.stack_height() <= depth))
                    .map(|(end, _)| end as u32)
                    .unwrap_or(instructions.len() as u32);
                let scoped: Vec<_> = transfers
                    .iter()
                    .filter(|transfer| {
                        transfer.index > index as u32
                            && transfer.index < end
                            && accounts.iter().any(|account| account.0 == &transfer.source)
                            && accounts.iter().any(|account| account.0 == &transfer.destination)
                    })
                    .collect();
                // Pump's sell instruction debits program-owned lamports
                // directly, without a System CPI. Only accept its exact pool
                // balance delta when this is the sole pool-bearing invocation
                // and there are no explicit SOL transfers touching that pool.
                if swap.protocol == pb::Protocol::Pumpfun as i32
                    && swap.output_mint == crate::SOL_MINT
                    && matches!(
                        pumpfun::instructions::unpack(ix.data()),
                        Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)
                            | pumpfun::instructions::PumpFunInstruction::SellV2(_)
                            | pumpfun::instructions::PumpFunInstruction::SellV3(_))
                    )
                {
                    let calls = instructions
                        .iter()
                        .filter(|call| call.program_id().0 == &swap.amm && call.accounts().iter().any(|a| a.0 == &swap.amm_pool))
                        .count();
                    let explicit_sol = transfers
                        .iter()
                        .any(|t| t.mint == crate::SOL_MINT && (t.source == swap.amm_pool || t.destination == swap.amm_pool));
                    let pool_delta = tx.resolved_accounts().iter().position(|a| *a == &swap.amm_pool).and_then(|position| {
                        tx.meta
                            .as_ref()
                            .and_then(|meta| Some(i128::from(*meta.pre_balances.get(position)?) - i128::from(*meta.post_balances.get(position)?)))
                    });
                    // V3 retains protocol/creator fees in the curve and pays
                    // buyback fees to its declared recipient. Prove the net
                    // user proceeds against both exact lamport deltas; the
                    // event's gross quote alone cannot verify this variant.
                    if calls == 1
                        && !explicit_sol
                        && matches!(
                            pumpfun::instructions::unpack(ix.data()),
                            Ok(pumpfun::instructions::PumpFunInstruction::SellV3(_))
                        )
                    {
                        let amounts = instructions
                            .iter()
                            .skip(index + 1)
                            .take(end as usize - index - 1)
                            .filter(|child| child.program_id().0 == &pumpfun::PROGRAM_ID)
                            .find_map(|child| {
                                let event = pumpfun::events::unpack_trade_event_minimal(child.data()).ok()??;
                                if event.is_buy || event.mint != swap.input_mint.as_slice() || event.user != swap.user.as_slice() {
                                    return None;
                                }
                                pumpfun::events::unpack_quote_trade_amount(child.data()).ok()
                            });
                        let proven = amounts.and_then(|amounts| {
                            let net = amounts.amount.checked_sub(amounts.fee)?.checked_sub(amounts.creator_fee)?;
                            let recipient = accounts.get(13)?.0;
                            let recipient_calls = instructions
                                .iter()
                                .filter(|call| call.program_id().0 == &pumpfun::PROGRAM_ID && call.accounts().iter().any(|a| a.0 == recipient))
                                .count();
                            let explicit_recipient_sol = transfers
                                .iter()
                                .any(|t| t.mint == crate::SOL_MINT && (&t.source == recipient || &t.destination == recipient));
                            let position = tx.resolved_accounts().iter().position(|a| *a == recipient)?;
                            let meta = tx.meta.as_ref()?;
                            let delta = i128::from(*meta.post_balances.get(position)?) - i128::from(*meta.pre_balances.get(position)?);
                            if net == 0
                                || recipient_calls != 1
                                || explicit_recipient_sol
                                || delta != i128::from(amounts.buyback_fee)
                                || pool_delta != Some(i128::from(net.checked_add(amounts.buyback_fee)?))
                            {
                                return None;
                            }
                            let source = accounts.get(9)?.0;
                            let destination = accounts.get(6)?.0;
                            let input = scoped.iter().find(|t| {
                                t.mint == swap.input_mint
                                    && t.depth == depth + 1
                                    && t.amounts[0] == swap.input_amount
                                    && &t.source == source
                                    && &t.destination == destination
                                    && t.authority == swap.user
                            })?;
                            Some((net, input.index))
                        });
                        if let Some((net, input_index)) = proven {
                            let identity = (index as u32, input_index, input_index);
                            if !native_evidence.contains(&identity) {
                                let mut original = swap.clone();
                                original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                                rejected.push(original);
                                swap.output_amount = net;
                                evidence = Some(identity);
                                break 'candidate;
                            }
                        }
                    }
                    if calls == 1 && !explicit_sol && pool_delta == Some(i128::from(swap.output_amount)) {
                        if let Some(input) = scoped.iter().find(|t| t.mint == swap.input_mint && t.amounts.contains(&swap.input_amount)) {
                            let identity = (index as u32, input.index, input.index);
                            if !native_evidence.contains(&identity) {
                                evidence = Some(identity);
                                break 'candidate;
                            }
                        }
                    }
                }
                // Dynamic AMM v1 logs report vault-share conversions, which can
                // differ from the transferred atoms. Its explicit user accounts
                // let us use exact CPI flows instead of rounding the event.
                let user_accounts = if swap.protocol == pb::Protocol::MeteoraAmm as i32 {
                    amm::accounts::get_swap_accounts(ix)
                        .ok()
                        .map(|a| (a.user_source_token.to_bytes().to_vec(), a.user_destination_token.to_bytes().to_vec()))
                } else if swap.protocol == pb::Protocol::PumpfunAmm as i32 {
                    pumpswap::accounts::TradeTokenAccounts::try_from(ix).ok().and_then(|a| {
                        if swap.input_mint == a.quote_mint.to_bytes() && swap.output_mint == a.base_mint.to_bytes() {
                            Some((a.user_quote_token_account.to_bytes().to_vec(), a.user_base_token_account.to_bytes().to_vec()))
                        } else if swap.input_mint == a.base_mint.to_bytes() && swap.output_mint == a.quote_mint.to_bytes() {
                            Some((a.user_base_token_account.to_bytes().to_vec(), a.user_quote_token_account.to_bytes().to_vec()))
                        } else {
                            None
                        }
                    })
                } else if swap.protocol == pb::Protocol::Pumpfun as i32 {
                    use pumpfun::instructions::PumpFunInstruction::*;
                    let (base, quote) = match pumpfun::instructions::unpack(ix.data()) {
                        Ok(Buy(_) | BuyExactSolIn(_) | Sell(_)) => (5, 6),
                        Ok(BuyV2(_) | BuyExactQuoteInV2(_) | SellV2(_)) => (14, 15),
                        Ok(BuyV3(_) | BuyExactQuoteInV3(_) | SellV3(_)) => (9, 10),
                        _ => (usize::MAX, usize::MAX),
                    };
                    accounts.get(base).zip(accounts.get(quote)).map(|(base, quote)| {
                        if swap.input_mint == crate::SOL_MINT {
                            (swap.user.clone(), base.0.clone())
                        } else if swap.input_mint == mints.mint_for(base.0).unwrap_or_default() {
                            (base.0.clone(), quote.0.clone())
                        } else {
                            (quote.0.clone(), base.0.clone())
                        }
                    })
                } else {
                    None
                };
                let corrected = user_accounts.and_then(|(source, destination)| {
                    let inputs: Vec<_> = scoped.iter().filter(|t| t.source == source && t.mint == swap.input_mint).collect();
                    let outputs: Vec<_> = scoped.iter().filter(|t| t.destination == destination && t.mint == swap.output_mint).collect();
                    if inputs.is_empty() || outputs.len() != 1 || (outputs[0].fee_token && outputs[0].mint != crate::SOL_MINT && !outputs[0].net_verified) {
                        return None;
                    }
                    let total = inputs.iter().try_fold(0u64, |total, t| total.checked_add(t.amounts[0]))?;
                    // The verified recipient delta (or explicit checked fee)
                    // supplies net output when it differs from the CPI debit.
                    let amount = outputs[0].amounts.iter().copied().min()?;
                    Some((total, amount, inputs[0].index, outputs[0].index))
                });
                if let Some((input, output, input_index, output_index)) = corrected {
                    let identity = (index as u32, input_index, output_index);
                    if expected_transfer.is_some_and(|transfer| transfer != input_index.min(output_index)) {
                        continue;
                    }
                    if !native_evidence.contains(&identity) {
                        if (swap.input_amount, swap.output_amount) != (input, output) {
                            let mut original = swap.clone();
                            original.source_index = Some(index as u32);
                            original.source_transfer_index = Some(input_index.min(output_index));
                            original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                            rejected.push(original);
                        }
                        swap.input_amount = input;
                        swap.output_amount = output;
                        evidence = Some(identity);
                        break 'candidate;
                    }
                }
                for input in scoped.iter().filter(|t| {
                    t.mint == swap.input_mint
                        && (t.amounts.contains(&swap.input_amount)
                            || scoped
                                .iter()
                                .filter(|leg| leg.source == t.source && leg.mint == t.mint)
                                .try_fold(0u64, |total, leg| total.checked_add(leg.amounts[0]))
                                == Some(swap.input_amount))
                }) {
                    for output in scoped.iter().filter(|t| {
                        t.mint == swap.output_mint
                            && (!t.fee_token || t.mint == crate::SOL_MINT || t.net_verified)
                            && if matches!(swap.protocol, 4 | 5) {
                                t.amounts.contains(&swap.output_amount)
                            } else {
                                t.amounts.iter().min() == Some(&swap.output_amount)
                            }
                    }) {
                        let identity = (index as u32, input.index, output.index);
                        if expected_transfer.is_some_and(|transfer| transfer != input.index.min(output.index)) {
                            continue;
                        }
                        if !matches!(swap.protocol, 4 | 5) && native_evidence.contains(&identity) {
                            continue;
                        }
                        if matches!(swap.protocol, 4 | 5) {
                            let net = *output.amounts.iter().min().unwrap();
                            if swap.output_amount != net {
                                let mut original = swap.clone();
                                original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                                rejected.push(original);
                                swap.output_amount = net;
                            }
                        }
                        evidence = Some(identity);
                        break 'candidate;
                    }
                }
            }
        }
        match evidence {
            Some(identity)
                if native_evidence
                    .iter()
                    .any(|prior: &(u32, u32, u32)| prior.0 == identity.0 && prior.1.min(prior.2) == identity.1.min(identity.2)) =>
            {
                swap.source_index = Some(identity.0);
                swap.source_transfer_index = Some(identity.1.min(identity.2));
                swap.verification_failure = "duplicate_native_cpi".into();
                rejected.push(swap);
            }
            Some(identity) if swap.input_amount > 0 && swap.output_amount > 0 => {
                swap.source_index = Some(identity.0);
                swap.source_transfer_index = Some(identity.1.min(identity.2));
                swap.transfer_verified = true;
                native_evidence.insert(identity);
                verified.push(swap);
            }
            _ => {
                swap.verification_failure = "pool_mints_or_amounts_not_verified".into();
                rejected.push(swap);
            }
        }
    }
    verified.sort_by_key(|swap| (swap.source_index, swap.source_transfer_index));
    *swaps = verified;
    rejected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_v3_sell_requires_both_pool_and_buyback_lamport_evidence() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/swap-coverage/25.transaction.pb").to_vec()).unwrap();
        let expected = crate::process_transaction(tx.clone()).unwrap();
        let swap = expected.swaps.iter().find(|swap| swap.protocol == pb::Protocol::Pumpfun as i32).unwrap();
        assert_eq!(swap.output_amount, 47_690_764);
        assert_eq!(swap.source_transfer_index, Some(7));
        let ix = tx
            .walk_instructions()
            .find(|ix| {
                matches!(
                    pumpfun::instructions::unpack(ix.data()),
                    Ok(pumpfun::instructions::PumpFunInstruction::SellV3(_))
                )
            })
            .unwrap();
        let fee_recipient = ix.accounts()[13].0.clone();
        let pool = swap.amm_pool.clone();
        for account in [fee_recipient, pool] {
            let mut changed = tx.clone();
            let position = changed.resolved_accounts().iter().position(|key| **key == account).unwrap();
            changed.meta.as_mut().unwrap().post_balances[position] += 1;
            let decoded = crate::process_transaction(changed).unwrap();
            assert!(decoded.swaps.iter().all(|swap| swap.protocol != pb::Protocol::Pumpfun as i32));
        }
    }

    #[test]
    fn public_native_sol_sell_and_fee_token_receipts_require_exact_evidence() {
        for fixture in [
            include_bytes!("../fixtures/pumpfun-native-sell.transaction.pb").as_slice(),
            include_bytes!("../fixtures/raydium-transfer-fee.transaction.pb").as_slice(),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            assert_eq!(decoded.swaps.len(), 1, "{:?}", decoded.rejected_swaps);
            assert!(decoded.swaps[0].transfer_verified);
            let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            let mut swap = decoded.swaps[0].clone();
            swap.output_amount += 1;
            let mut swaps = vec![swap];
            assert_eq!(verify_and_deduplicate(&tx, &mints, &mut swaps, false).len(), 1);
            assert!(swaps.is_empty());
        }
    }

    #[test]
    fn public_aqua_route_has_exact_verified_and_ordered_transfers() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/aqua-orca-multihop.transaction.pb").to_vec()).unwrap();
        let decoded = crate::process_transaction(tx).unwrap();
        assert_eq!(decoded.swaps.len(), 3);
        assert!(decoded
            .rejected_swaps
            .iter()
            .all(|swap| swap.verification_failure == "event_amounts_replaced_by_exact_user_flows"));
        assert_eq!(decoded.swaps[1].output_amount, 2_170_348_465_518);
        assert!(decoded.swaps.iter().all(|swap| swap.transfer_verified));
        assert!(decoded.swaps.windows(2).all(|swaps| swaps[0].source_index < swaps[1].source_index));
    }

    #[test]
    fn changed_pool_mint_or_amount_never_becomes_a_verified_swap() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/aqua-orca-multihop.transaction.pb").to_vec()).unwrap();
        let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        for field in 0..4 {
            let mut swap = decoded.swaps[2].clone();
            match field {
                0 => swap.amm_pool = vec![9; 32],
                1 => swap.output_mint = vec![8; 32],
                2 => swap.output_amount += 1,
                _ => {
                    swap = decoded.swaps[1].clone();
                    swap.output_amount = 2_214_641_291_345;
                }
            }
            let mut swaps = vec![swap];
            let rejected = verify_and_deduplicate(&tx, &mints, &mut swaps, false);
            assert!(swaps.is_empty());
            assert_eq!(rejected.len(), 1);
        }
    }

    #[test]
    fn jupiter_wrapper_is_deduplicated_by_transfer_identity() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/aqua-orca-multihop.transaction.pb").to_vec()).unwrap();
        let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
        let native = crate::process_transaction(tx.clone()).unwrap().swaps[2].clone();
        let mut wrapper = native.clone();
        wrapper.protocol = pb::Protocol::JupiterV6 as i32;
        let mut swaps = vec![wrapper, native];
        let rejected = verify_and_deduplicate(&tx, &mints, &mut swaps, false);
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].protocol, pb::Protocol::OrcaWhirlpool as i32);
        assert_eq!(rejected[0].verification_failure, "duplicate_native_cpi");
    }
}
