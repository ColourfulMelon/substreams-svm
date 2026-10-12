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
    metadata_net: bool,
}

struct PumpNativeSell {
    instruction: usize,
    roles: [usize; 10],
    input: u32,
    net: u64,
    gross: u64,
    protocol_fee: u64,
    creator_fee: u64,
    buyback_fee: u64,
}

#[cfg(test)]
pub(crate) fn verify_and_deduplicate(tx: &ConfirmedTransaction, mints: &TokenMintLookup, swaps: &mut Vec<pb::Swap>, expand_native: bool) -> Vec<pb::Swap> {
    verify_with_fees(tx, mints, swaps, expand_native, |_, _| None)
}

pub(crate) fn verify_with_fees(
    tx: &ConfirmedTransaction,
    mints: &TokenMintLookup,
    swaps: &mut Vec<pb::Swap>,
    expand_native: bool,
    mut fee_lookup: impl FnMut(&[u8], u32) -> Option<crate::token_fees::TransferFee>,
) -> Vec<pb::Swap> {
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
        || instructions.iter().any(|ix| {
            ix.program_id().0 == &token_2022::PROGRAM_ID
                || ix.program_id().0 == &crate::native_venues::ORCA_V1
                || ix.program_id().0 == &substreams_solana_idls::spl::token_swap::PROGRAM_ID
                || ix.program_id().0 == &crate::mercurial::PROGRAM_ID
                || ix.program_id().0 == &crate::oasis::PROGRAM_ID
                || ix.program_id().0 == &substreams_solana_idls::meteora::dlmm::PROGRAM_ID
                || ix.program_id().0 == &pumpfun::PROGRAM_ID && (ix.data().starts_with(&[225, 154, 117, 22, 215, 86, 246, 103])
                    || ix.data().starts_with(&pumpfun::instructions::SELL_V3)
                    || matches!(pumpfun::instructions::unpack(ix.data()), Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)
                        | pumpfun::instructions::PumpFunInstruction::SellV2(_))))
                || crate::native_venues::defituna_two_hop(ix).is_some()
        }) {
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
                let metadata_net = if program.0 == &token_2022::PROGRAM_ID {
                    if let Some(fee) = fee_lookup(&mint, index as u32) {
                        // Arithmetic is evidence only for a completed transfer
                        // whose entire ancestor chain also committed.
                        if !successful_invocations.contains(&(index as u32)) {
                            continue;
                        }
                        let Some(net) = fee.received(amount) else { continue };
                        if data.first() == Some(&26) && amounts.iter().min() != Some(&net) {
                            continue;
                        }
                        if !amounts.contains(&net) {
                            amounts.push(net);
                        }
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                transfers.push(Transfer {
                    index: index as u32,
                    depth: ix.stack_height(),
                    mint,
                    amounts,
                    source: accounts[source].0.clone(),
                    destination: accounts[destination].0.clone(),
                    authority: accounts.get(destination + 1).map(|a| a.0.clone()).unwrap_or_default(),
                    fee_token: program.0 == &token_2022::PROGRAM_ID,
                    net_verified: program.0 != &token_2022::PROGRAM_ID || data.first() == Some(&26) || metadata_net,
                    metadata_net: metadata_net && data.first() != Some(&26),
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
    let mut observed_start = HashSet::new();
    let mut observed_end = HashSet::new();
    let accounts = tx.resolved_accounts();
    if let Some(meta) = &tx.meta {
        for (balances, sign) in [(&meta.pre_token_balances, -1i128), (&meta.post_token_balances, 1)] {
            for balance in balances {
                if let (Some(account), Some(amount)) = (accounts.get(balance.account_index as usize), balance.ui_token_amount.as_ref()) {
                    if let Ok(value) = amount.amount.parse::<u64>() {
                        *deltas.entry((*account).clone()).or_default() += sign * i128::from(value);
                        observed_balances.insert((*account).clone());
                        if sign < 0 {
                            observed_start.insert((*account).clone());
                        } else {
                            observed_end.insert((*account).clone());
                        }
                    }
                }
            }
        }
    }
    // A temporary account created once and closed once has zero starting and
    // ending spendable tokens. Its outgoing debits prove a unique receipt,
    // including Token-2022 fees and explicit burns; mints and unknown
    // extension operations still block it.
    let mut proven_closed = HashSet::new();
    for (account, count) in &closes {
        if *count == 1 && initializations.get(account) == Some(&1) && !observed_balances.contains(account) {
            deltas.insert(account.clone(), 0);
        } else {
            // A previously initialized account can receive tokens, burn or
            // forward them, then close. A committed CloseAccount proves its
            // final spendable balance is zero. Keep the observed starting
            // balance rather than rejecting every closed, observed account.
            // Reinitialization, a post balance, an unproven CPI close or token
            // access after closure cannot supply this missing end balance.
            let committed_close =
                (*count == 1 && !initializations.contains_key(account) && observed_start.contains(account) && !observed_end.contains(account))
                    .then(|| {
                        instructions.iter().enumerate().find(|(position, ix)| {
                            (ix.program_id().0 == &token::PROGRAM_ID || ix.program_id().0 == &token_2022::PROGRAM_ID)
                                && ix.data().as_slice() == [9]
                                && ix.accounts().len() >= 3
                                && ix.accounts()[0].0 == account
                                && (ix.is_root() || successful_invocations.contains(&(*position as u32)))
                        })
                    })
                    .flatten();
            if let Some((position, _)) = committed_close {
                if !instructions[position + 1..].iter().any(|ix| {
                    (ix.program_id().0 == &token::PROGRAM_ID || ix.program_id().0 == &token_2022::PROGRAM_ID) && ix.accounts().iter().any(|a| a.0 == account)
                }) {
                    proven_closed.insert(account.clone());
                    continue;
                }
            }
            other_token_operations.insert(account.clone());
        }
    }
    // An omitted post balance alone is not evidence of a zero end balance.
    // Conservation needs an actual close with the lifecycle proof above.
    for account in observed_start.difference(&observed_end) {
        if !closes.contains_key(account) {
            other_token_operations.insert(account.clone());
        }
    }
    // A complete destination ledger independently checks metadata arithmetic.
    // Disagreement invalidates only the derived receipt; the established
    // single-receipt conservation proof may still recover the actual amount.
    let inconsistent: HashSet<_> = transfers
        .iter()
        .filter_map(|transfer| {
            let key = (transfer.destination.clone(), transfer.mint.clone());
            if !transfer.metadata_net || other_token_operations.contains(&transfer.destination) || receipt_counts.contains_key(&key) {
                return None;
            }
            let observed = deltas.get(&transfer.destination)? + debits.get(&key).unwrap_or(&0);
            (known_receipts.get(&key).copied() != Some(observed)).then_some(key)
        })
        .collect();
    for transfer in &mut transfers {
        let key = (transfer.destination.clone(), transfer.mint.clone());
        if transfer.metadata_net && inconsistent.contains(&key) {
            *known_receipts.entry(key.clone()).or_default() -= i128::from(*transfer.amounts.iter().min().unwrap());
            *receipt_counts.entry(key.clone()).or_default() += 1;
            *gross_receipts.entry(key).or_default() += i128::from(transfer.amounts[0]);
            transfer.net_verified = false;
            transfer.amounts.truncate(1);
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
            // A committed non-native Token-2022 close also requires zero
            // withheld fees. Unknown extension operations are already blocked;
            // without them, the unknown receipts must conserve their full gross.
            let net = if proven_closed.contains(account) && received != gross_receipts.get(&key).copied() {
                None
            } else if received == gross_receipts.get(&key).copied() {
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

    // DAMM v2 a85c926607433f23f0ea60f4ca7b1ae92f4156cb sends input,
    // then the swap output, then an optional current-swap referral payment.
    // A referral may alias either user account. Bind the first two actual
    // transfer instructions, including zero-amount calls, so a referral can
    // never stand in for a zero output or become a second financial swap.
    let mut damm_receipts = HashMap::new();
    for (index, ix) in instructions.iter().enumerate() {
        if ix.program_id().0 != &substreams_solana_idls::meteora::daam::PROGRAM_ID || (caught_failure && !successful_invocations.contains(&(index as u32))) {
            continue;
        }
        let Some(layout) = crate::native_venues::layout(ix) else { continue };
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        if depth == 0 {
            continue;
        }
        let end = instructions
            .iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_, child)| child.is_root() || (child.stack_height() > 0 && child.stack_height() <= depth))
            .map_or(instructions.len(), |(position, _)| position);
        let calls: Vec<_> = instructions
            .iter()
            .enumerate()
            .take(end)
            .skip(index + 1)
            .filter(|(_, child)| {
                let program = child.program_id().0;
                let data = child.data();
                child.stack_height() == depth + 1
                    && (program == &token::PROGRAM_ID || program == &token_2022::PROGRAM_ID)
                    && ((data.first() == Some(&3) && data.len() == 9)
                        || (data.first() == Some(&12) && data.len() == 10)
                        || (data.len() == 19 && data[..2] == [26, 1]))
            })
            .collect();
        if !matches!(calls.len(), 2 | 3)
            || calls
                .iter()
                .any(|(position, _)| caught_failure && !successful_invocations.contains(&(*position as u32)))
        {
            continue;
        }
        let Some((input_position, input)) = transfers.iter().enumerate().find(|(_, t)| t.index == calls[0].0 as u32) else {
            continue;
        };
        let Some((output_position, output)) = transfers.iter().enumerate().find(|(_, t)| t.index == calls[1].0 as u32) else {
            continue;
        };
        let accounts = ix.accounts();
        let (input_vault, output_vault, input_program, output_program, output_mint) = if input.mint == *accounts[6].0 {
            (4, 5, 9, 10, 7)
        } else if input.mint == *accounts[7].0 {
            (5, 4, 10, 9, 6)
        } else {
            continue;
        };
        if input.source != *accounts[2].0
            || input.destination != *accounts[input_vault].0
            || input.authority != *accounts[8].0
            || output.source != *accounts[output_vault].0
            || output.destination != *accounts[3].0
            || output.authority != *accounts[0].0
            || output.mint != *accounts[output_mint].0
            || input.mint == output.mint
            || !output.net_verified
            || calls[0].1.program_id().0 != accounts[input_program].0
            || calls[1].1.program_id().0 != accounts[output_program].0
            || input.amounts[0] > layout.amount
            || (layout.exact_input && !layout.input_limit && input.amounts[0] != layout.amount)
            || *output.amounts.iter().min().unwrap() < layout.minimum_output
        {
            continue;
        }
        if calls.len() == 3 {
            let referral = calls[2].1;
            let data = referral.data();
            let a = referral.accounts();
            let checked = data.first() != Some(&3);
            let destination = if checked { 2 } else { 1 };
            let Some(source) = a.first() else { continue };
            let (mint, program) = if source.0 == accounts[4].0 {
                (6, 9)
            } else if source.0 == accounts[5].0 {
                (7, 10)
            } else {
                continue;
            };
            if a.len() <= destination + 1
                || a[destination].0 != accounts[11].0
                || a[destination + 1].0 != accounts[0].0
                || referral.program_id().0 != accounts[program].0
                || (checked && a[1].0 != accounts[mint].0)
                || mints.mint_for(source.0).as_ref() != Some(accounts[mint].0)
            {
                continue;
            }
        }
        damm_receipts.insert(index as u32, (input_position, output_position));
    }

    let mut mercurial_receipts = HashMap::new();
    for (index, ix) in instructions.iter().enumerate() {
        let Some(exchange) = crate::mercurial::layout(ix) else { continue };
        if !successful_invocations.contains(&(index as u32)) {
            continue;
        }
        let accounts = ix.accounts();
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        let end = instructions.iter().enumerate().skip(index + 1)
            .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
            .map_or(instructions.len() as u32, |(end, _)| end as u32);
        let scoped: Vec<_> = transfers.iter().enumerate().filter(|(_, t)| {
            t.index > index as u32 && t.index < end && t.depth == depth + 1
                && successful_invocations.contains(&t.index)
        }).collect();
        let token_calls: Vec<_> = instructions.iter().enumerate().skip(index + 1).take(end as usize - index - 1)
            .filter(|(_, child)| child.stack_height() == depth + 1
                && [token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&child.program_id().0.as_slice()))
            .map(|(i, _)| i as u32).collect();
        if scoped.len() != 2 || token_calls != scoped.iter().map(|(_, t)| t.index).collect::<Vec<_>>() {
            continue;
        }
        let (input_position, input) = scoped[0];
        let (output_position, output) = scoped[1];
        let vaults = &accounts[4..exchange.source];
        if input.source != *accounts[exchange.source].0 || input.authority != *accounts[3].0
            || output.destination != *accounts[exchange.destination].0 || output.authority != *accounts[2].0
            || !vaults.iter().any(|vault| vault.0 == &input.destination)
            || !vaults.iter().any(|vault| vault.0 == &output.source)
            || input.destination == output.source || input.mint == output.mint
            || mints.mint_for(&input.destination).as_ref() != Some(&input.mint)
            || mints.mint_for(&output.source).as_ref() != Some(&output.mint)
            || instructions[input.index as usize].program_id().0 != accounts[1].0
            || instructions[output.index as usize].program_id().0 != accounts[1].0
            || input.amounts[0] != exchange.amount || output.amounts[0] < exchange.minimum_output
        {
            continue;
        }
        mercurial_receipts.insert(index as u32, (input_position, output_position));
    }

    let mut oasis_receipts = HashMap::new();
    for (index, ix) in instructions.iter().enumerate() {
        let Some((limit, x_to_y)) = crate::oasis::layout(ix) else { continue };
        if !successful_invocations.contains(&(index as u32)) { continue; }
        let a = ix.accounts();
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        let end = instructions.iter().enumerate().skip(index + 1)
            .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
            .map_or(instructions.len(), |(end, _)| end);
        // The observed variant has input, output, then a zero referral CPI.
        // Every child must commit, with no hidden token operations or nesting.
        let calls = &instructions[index + 1..end];
        if calls.len() != 3 || calls.iter().enumerate().any(|(offset, child)| {
            child.stack_height() != depth + 1 || child.program_id().0 != a[14].0
                || child.data().len() != 9 || child.data()[0] != 3 || child.accounts().len() != 3
                || !successful_invocations.contains(&((index + 1 + offset) as u32))
        }) { continue; }
        let (in_mint, out_mint, in_vault, out_vault, source, destination, referral) =
            if x_to_y { (2, 3, 4, 5, 6, 7, 10) } else { (3, 2, 5, 4, 7, 6, 9) };
        let referral_accounts = calls[2].accounts();
        if calls[2].data()[1..9] != [0; 8] || referral_accounts[0].0 != a[out_vault].0
            || referral_accounts[1].0 != a[referral].0 || referral_accounts[2].0 != a[12].0
        { continue; }
        let scoped: Vec<_> = transfers.iter().enumerate().filter(|(_, t)| {
            t.index > index as u32 && t.index < end as u32
        }).collect();
        if scoped.len() != 2 { continue; }
        let (input_position, input) = scoped[0];
        let (output_position, output) = scoped[1];
        if input.index != index as u32 + 1 || output.index != index as u32 + 2
            || input.source != *a[source].0 || input.destination != *a[in_vault].0 || input.authority != *a[8].0
            || output.source != *a[out_vault].0 || output.destination != *a[destination].0 || output.authority != *a[12].0
            || input.mint != *a[in_mint].0 || output.mint != *a[out_mint].0
            || mints.mint_for(a[in_vault].0).as_ref() != Some(a[in_mint].0)
            || mints.mint_for(a[out_vault].0).as_ref() != Some(a[out_mint].0)
            || input.amounts[0] > limit
        { continue; }
        oasis_receipts.insert(index as u32, (input_position, output_position));
    }

    let mut two_hop_receipts = HashMap::new();
    for (index, ix) in instructions.iter().enumerate() {
        let Some((amount, threshold, exact_input)) = crate::native_venues::defituna_two_hop(ix) else {
            continue;
        };
        if !successful_invocations.contains(&(index as u32)) {
            continue;
        }
        let accounts = ix.accounts();
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        let end = instructions
            .iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
            .map_or(instructions.len() as u32, |(end, _)| end as u32);
        let mut legs = Vec::new();
        for (source, destination, authority, mint, program) in [(8, 9, 14, 2, 5), (10, 11, 0, 3, 6), (12, 13, 1, 4, 7)] {
            let matched: Vec<_> = transfers
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    t.index > index as u32
                        && t.index < end
                        && t.depth == depth + 1
                        && t.source == *accounts[source].0
                        && t.destination == *accounts[destination].0
                        && t.authority == *accounts[authority].0
                        && t.mint == *accounts[mint].0
                        && instructions[t.index as usize].program_id().0 == accounts[program].0
                        && successful_invocations.contains(&t.index)
                })
                .collect();
            if matched.len() != 1
                || mints.mint_for(accounts[source].0).as_ref() != Some(accounts[mint].0)
                || mints.mint_for(accounts[destination].0).as_ref() != Some(accounts[mint].0)
                || !legs.is_empty() && matched[0].1.fee_token && !matched[0].1.net_verified
            {
                break;
            }
            legs.push(matched[0].0);
        }
        if legs.len() != 3
            || transfers[legs[0]].index >= transfers[legs[1]].index
            || transfers[legs[1]].index >= transfers[legs[2]].index
            || transfers
                .iter()
                .filter(|t| t.index > index as u32 && t.index < end && t.depth == depth + 1)
                .count()
                != 3
        {
            continue;
        }
        let input = &transfers[legs[0]];
        let output = &transfers[legs[2]];
        let net = *output.amounts.iter().min().unwrap();
        if net == 0
            || input.amounts[0] > if exact_input { amount } else { threshold }
            || net < if exact_input { threshold } else { amount }
            || !exact_input && net != amount
            || transfers[legs[1]].amounts.iter().min() == Some(&0)
        {
            continue;
        }
        two_hop_receipts.insert(index as u32, legs);
    }

    let mut rejected = Vec::new();
    let mut native_sol_multihop_receipts = HashMap::new();
    if expand_native {
        for (&index, &(input_position, output_position)) in &oasis_receipts {
            let ix = &instructions[index as usize];
            let a = ix.accounts();
            let (input, output) = (&transfers[input_position], &transfers[output_position]);
            swaps.push(pb::Swap {
                protocol: pb::Protocol::Oasis as i32,
                program_id: crate::oasis::PROGRAM_ID.to_vec(), amm: crate::oasis::PROGRAM_ID.to_vec(),
                amm_pool: a[1].0.clone(), user: a[8].0.clone(),
                stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
                input_mint: input.mint.clone(), input_amount: input.amounts[0],
                output_mint: output.mint.clone(), output_amount: output.amounts[0],
                source_index: Some(index), source_transfer_index: Some(input.index),
                ..Default::default()
            });
        }
        for (&index, &(input_position, output_position)) in &mercurial_receipts {
            let ix = &instructions[index as usize];
            let accounts = ix.accounts();
            let (input, output) = (&transfers[input_position], &transfers[output_position]);
            swaps.push(pb::Swap {
                protocol: pb::Protocol::Mercurial as i32,
                program_id: crate::mercurial::PROGRAM_ID.to_vec(),
                amm: crate::mercurial::PROGRAM_ID.to_vec(),
                amm_pool: accounts[0].0.clone(), user: accounts[3].0.clone(),
                stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
                input_mint: input.mint.clone(), input_amount: input.amounts[0],
                output_mint: output.mint.clone(), output_amount: output.amounts[0],
                source_index: Some(index), source_transfer_index: Some(input.index),
                ..Default::default()
            });
        }
        for (&index, legs) in &two_hop_receipts {
            let ix = &instructions[index as usize];
            let accounts = ix.accounts();
            for hop in 0..2 {
                let (input, output) = (&transfers[legs[hop]], &transfers[legs[hop + 1]]);
                swaps.push(pb::Swap {
                    protocol: pb::Protocol::Defituna as i32,
                    program_id: ix.program_id().0.clone(),
                    amm: ix.program_id().0.clone(),
                    amm_pool: accounts[hop].0.clone(),
                    user: accounts[14].0.clone(),
                    stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
                    input_mint: input.mint.clone(),
                    input_amount: input.amounts[0],
                    output_mint: output.mint.clone(),
                    output_amount: *output.amounts.iter().min().unwrap(),
                    source_index: Some(index),
                    source_transfer_index: Some(input.index.min(output.index)),
                    ..Default::default()
                });
            }
        }
        for (index, ix) in instructions.iter().enumerate() {
            let Some(&(input_position, output_position)) = damm_receipts.get(&(index as u32)) else {
                continue;
            };
            let (input, output) = (&transfers[input_position], &transfers[output_position]);
            let a = ix.accounts();
            let native = pb::Swap {
                protocol: pb::Protocol::MeteoraDaam as i32,
                program_id: ix.program_id().0.clone(),
                amm: ix.program_id().0.clone(),
                stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
                amm_pool: a[1].0.clone(),
                user: a[8].0.clone(),
                input_mint: input.mint.clone(),
                input_amount: input.amounts[0],
                output_mint: output.mint.clone(),
                output_amount: *output.amounts.iter().min().unwrap(),
                source_index: Some(index as u32),
                source_transfer_index: Some(input.index.min(output.index)),
                ..Default::default()
            };
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
            if layout.protocol == pb::Protocol::MeteoraDaam {
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
                if layout.protocol == pb::Protocol::Fluxbeam
                    && (output.authority != *accounts[1].0
                        || input.mint != *accounts[9].0
                        || output.mint != *accounts[10].0
                        || instructions[input.index as usize].program_id().0 != accounts[11].0
                        || instructions[output.index as usize].program_id().0 != accounts[12].0)
                {
                    continue;
                }
                if layout.protocol == pb::Protocol::OpenbookV2
                    && (output.authority != *accounts[3].0
                        || instructions[input.index as usize].program_id().0 != accounts[13].0
                        || instructions[output.index as usize].program_id().0 != accounts[13].0)
                {
                    continue;
                }
                if layout.protocol == pb::Protocol::RaydiumLaunchpad {
                    // The IDL pins base/quote vaults, mint roles and pool PDA.
                    // New event versions must not prevent exact native CPIs
                    // from contributing to mint pricing or routed deduplication.
                    if output.authority.as_slice() != accounts[1].0.as_slice()
                        || mints.mint_for(accounts[7].0).as_ref() != Some(accounts[9].0)
                        || mints.mint_for(accounts[8].0).as_ref() != Some(accounts[10].0)
                        || ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&accounts[11].0.as_slice())
                        || ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&accounts[12].0.as_slice())
                        || instructions[input.index as usize].program_id().0 != accounts[if layout.users[0] == 5 { 11 } else { 12 }].0
                        || instructions[output.index as usize].program_id().0 != accounts[if layout.users[1] == 5 { 11 } else { 12 }].0
                    {
                        continue;
                    }
                }
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
                        || (fees.len() == 2 && fees[0].destination == fees[1].destination
                            && accounts[8].0 != accounts[9].0)
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

        // All six typed DLMM swap variants expose the native user/reserve
        // accounts. A completed swap and its exact immediate token legs are
        // sufficient even if its legacy event is absent or a new event format
        // is emitted. Never infer a trade from an unrecognized instruction.
        for (index, ix) in instructions.iter().enumerate() {
            let Some(layout) = crate::meteora_dlmm::decode_swap_instruction(ix) else {
                continue;
            };
            if !successful_invocations.contains(&(index as u32)) {
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
                .map_or(instructions.len() as u32, |(position, _)| position as u32);
            let scoped: Vec<_> = transfers
                .iter()
                .filter(|t| t.index > index as u32 && t.index < end && t.depth == depth + 1 && successful_invocations.contains(&t.index))
                .collect();
            let inputs: Vec<_> = scoped
                .iter()
                .filter(|t| {
                    t.source == layout.user_token_in
                        && t.authority == layout.user
                        && ((t.destination == layout.reserve_x
                            && t.mint == layout.token_x_mint
                            && instructions[t.index as usize].program_id().0 == &layout.token_x_program)
                            || (t.destination == layout.reserve_y
                                && t.mint == layout.token_y_mint
                                && instructions[t.index as usize].program_id().0 == &layout.token_y_program))
                })
                .collect();
            if inputs.len() != 1 {
                continue;
            }
            let input = inputs[0];
            let (reserve, mint, program) = if input.mint == layout.token_x_mint {
                (&layout.reserve_y, &layout.token_y_mint, &layout.token_y_program)
            } else {
                (&layout.reserve_x, &layout.token_x_mint, &layout.token_x_program)
            };
            let outputs: Vec<_> = scoped
                .iter()
                .filter(|t| {
                    t.source == *reserve
                        && t.mint == *mint
                        && t.destination == layout.user_token_out
                        && t.authority == layout.lb_pair
                        && instructions[t.index as usize].program_id().0 == program
                        && t.net_verified
                })
                .collect();
            if outputs.len() != 1 || input.mint.as_slice() == mint.as_slice() {
                continue;
            }
            let output = outputs[0];
            // Additional user flows (including refunds) need their own complete
            // net-debit proof; do not silently ignore them when normalizing.
            if scoped.iter().any(|t| {
                t.index != input.index
                    && t.index != output.index
                    && [t.source.as_slice(), t.destination.as_slice()]
                        .iter()
                        .any(|account| *account == layout.user_token_in.as_slice() || *account == layout.user_token_out.as_slice())
            }) {
                continue;
            }
            let native = pb::Swap {
                protocol: pb::Protocol::MeteoraDlmm as i32,
                program_id: substreams_solana_idls::meteora::dlmm::PROGRAM_ID.to_vec(),
                amm: substreams_solana_idls::meteora::dlmm::PROGRAM_ID.to_vec(),
                stack_height: depth,
                amm_pool: layout.lb_pair,
                user: layout.user,
                input_mint: input.mint.clone(),
                input_amount: input.amounts[0],
                output_mint: output.mint.clone(),
                output_amount: *output.amounts.iter().min().unwrap(),
                source_index: Some(index as u32),
                source_transfer_index: Some(input.index.min(output.index)),
                ..Default::default()
            };
            if let Some(existing) = swaps.iter_mut().find(|swap| {
                swap.protocol == native.protocol
                    && swap.source_index == native.source_index
                    && swap.amm_pool == native.amm_pool
                    && swap.user == native.user
                    && swap.input_mint == native.input_mint
                    && swap.output_mint == native.output_mint
            }) {
                if (existing.input_amount, existing.output_amount) != (native.input_amount, native.output_amount) {
                    let mut original = existing.clone();
                    original.source_transfer_index = native.source_transfer_index;
                    original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                    rejected.push(original);
                }
                *existing = native;
            } else {
                swaps.push(native);
            }
        }

        // RFQ fills identify a taker and maker, not reserve-vault liquidity.
        // Verify their exact user debit/receipt, including native SOL paths.
        for (index, ix) in instructions.iter().enumerate() {
            if caught_failure && !successful_invocations.contains(&(index as u32)) {
                continue;
            }
            let Some(fill) = crate::rfq::layout(ix) else { continue };
            let accounts = ix.accounts();
            let depth = if ix.is_root() { 1 } else { ix.stack_height() };
            if depth == 0 {
                continue;
            }
            let end = instructions
                .iter()
                .enumerate()
                .skip(index + 1)
                .find(|(_, child)| child.is_root() || (child.stack_height() > 0 && child.stack_height() <= depth))
                .map_or(instructions.len(), |(end, _)| end);
            let scoped: Vec<_> = transfers
                .iter()
                .filter(|t| t.index > index as u32 && (t.index as usize) < end && t.depth == depth + 1)
                .collect();
            let input_mint = accounts[6].0;
            let output_mint = accounts[8].0;
            let source = accounts[fill.input_source].0;
            let destination = accounts[fill.output_destination].0;
            let inputs: Vec<_> = scoped.iter().filter(|t| &t.source == source && &t.mint == input_mint).collect();
            let outputs: Vec<_> = scoped.iter().filter(|t| &t.destination == destination && &t.mint == output_mint).collect();
            if input_mint == output_mint || inputs.len() != 1 || outputs.len() != 1 {
                continue;
            }
            let (input, output) = (inputs[0], outputs[0]);
            if input.amounts[0] != fill.input_amount
                || output.amounts[0] != fill.output_amount
                || instructions[input.index as usize].program_id().0 != accounts[if fill.input_source == 0 { 10 } else { 7 }].0
                || instructions[output.index as usize].program_id().0 != accounts[if fill.output_source == 1 { 10 } else { 9 }].0
                || input.destination.as_slice() != accounts[fill.input_destination].0.as_slice()
                || output.source.as_slice() != accounts[fill.output_source].0.as_slice()
                || (!instructions[input.index as usize].program_id().0.iter().all(|byte| *byte == 0) && input.authority.as_slice() != accounts[0].0.as_slice())
                || (!instructions[output.index as usize].program_id().0.iter().all(|byte| *byte == 0)
                    && output.authority.as_slice() != accounts[1].0.as_slice())
                || (output.fee_token && output.mint != crate::SOL_MINT && !output.net_verified)
                || [2, 3].iter().any(|position| {
                    accounts[*position].0.as_slice() != crate::rfq::PROGRAM_ID && mints.mint_for(accounts[*position].0).as_ref() != Some(input_mint)
                })
                || [4, 5].iter().any(|position| {
                    accounts[*position].0.as_slice() != crate::rfq::PROGRAM_ID && mints.mint_for(accounts[*position].0).as_ref() != Some(output_mint)
                })
            {
                continue;
            }
            let child_instructions: Vec<_> = instructions[index + 1..end]
                .iter()
                .enumerate()
                .filter(|(offset, child)| {
                    child.stack_height() == depth + 1 && (!caught_failure || successful_invocations.contains(&((index + 1 + offset) as u32)))
                })
                .map(|(offset, child)| (index + 1 + offset, child))
                .collect();
            let mut native_proven = true;
            for transfer in [input, output] {
                // Native SOL credited to a WSOL account must be synced by the
                // declared contract, rather than reporting an unsynced balance.
                let token_destination = if transfer.index == input.index {
                    fill.input_destination
                } else {
                    fill.output_destination
                };
                if instructions[transfer.index as usize].program_id().0.iter().all(|byte| *byte == 0)
                    && !matches!(token_destination, 0 | 1)
                    && child_instructions
                        .iter()
                        .filter(|(position, child)| {
                            *position > transfer.index as usize
                                && child.program_id().0 == &token::PROGRAM_ID
                                && child.data().as_slice() == [17]
                                && child.accounts().first().is_some_and(|a| a.0 == accounts[token_destination].0)
                        })
                        .count()
                        != 1
                {
                    native_proven = false;
                }
            }
            if fill.unwrap_input || fill.unwrap_output {
                let temporary = accounts[11].0;
                let initializes: Vec<_> = child_instructions
                    .iter()
                    .filter(|(_, child)| {
                        child.program_id().0 == &token::PROGRAM_ID
                            && child.data().len() == 33
                            && child.data()[0] == 18
                            && child.data()[1..] == accounts[1].0[..]
                            && child.accounts().len() >= 2
                            && child.accounts()[0].0 == temporary
                            && child.accounts()[1].0.as_slice() == crate::SOL_MINT
                    })
                    .collect();
                let closes: Vec<_> = child_instructions
                    .iter()
                    .filter(|(_, child)| {
                        child.program_id().0 == &token::PROGRAM_ID
                            && child.data().as_slice() == [9]
                            && child.accounts().len() >= 3
                            && child.accounts()[0].0 == temporary
                            && child.accounts()[1].0 == accounts[1].0
                            && child.accounts()[2].0 == accounts[1].0
                    })
                    .collect();
                let unwraps: Vec<_> = scoped
                    .iter()
                    .filter(|t| {
                        t.destination.as_slice() == temporary.as_slice()
                            && instructions[t.index as usize].program_id().0 == &token::PROGRAM_ID
                            && t.mint == crate::SOL_MINT
                            && t.source.as_slice() == accounts[if fill.unwrap_input { 2 } else { 5 }].0.as_slice()
                            && t.authority.as_slice() == accounts[if fill.unwrap_input { 0 } else { 1 }].0.as_slice()
                            && t.amounts[0] == if fill.unwrap_input { fill.input_amount } else { fill.output_amount }
                    })
                    .collect();
                if initializes.len() != 1
                    || closes.len() != 1
                    || unwraps.len() != 1
                    || initializes[0].0 >= unwraps[0].index as usize
                    || closes[0].0 <= unwraps[0].index as usize
                    || (fill.unwrap_output && closes[0].0 >= output.index as usize)
                {
                    native_proven = false;
                }
            }
            if !native_proven {
                continue;
            }
            swaps.push(pb::Swap {
                protocol: pb::Protocol::JupiterRfq as i32,
                program_id: crate::rfq::PROGRAM_ID.to_vec(),
                amm: crate::rfq::PROGRAM_ID.to_vec(),
                // Real maker identity; protocol explicitly marks an RFQ source.
                // Pair columns keep its different asset markets separate.
                amm_pool: accounts[1].0.clone(),
                user: accounts[0].0.clone(),
                input_mint: input_mint.clone(),
                input_amount: input.amounts[0],
                output_mint: output_mint.clone(),
                output_amount: *output.amounts.iter().min().unwrap(),
                stack_height: depth,
                source_index: Some(index as u32),
                source_transfer_index: Some(input.index.min(output.index)),
                ..Default::default()
            });
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
                // A native-SOL curve buy debits the wallet into the curve
                // itself. Its declared WSOL accounts are mint witnesses only;
                // the curve's WSOL vault may not even exist.
                let native_sol_buy = is_curve && hop == 0 && input_mint.as_slice() == crate::SOL_MINT
                    && group[1].0.as_slice() == crate::SOL_MINT;
                if native_sol_buy && (accounts[8].0.as_slice() != token::PROGRAM_ID
                    || accounts[9].0.as_slice() != token_2022::PROGRAM_ID
                    || accounts[10].0.iter().any(|byte| *byte != 0)
                    || accounts[12].0.as_slice() != pumpfun::PROGRAM_ID
                    || !successful_invocations.contains(&(index as u32)))
                { continue; }
                let vault_in = if native_sol_buy { group[2].0 } else { vault_in };
                let source = if native_sol_buy { accounts[user].0 } else if hop == 0 { accounts[source].0 } else { &path[hop - 1].3 };
                let destination = if hop + 1 == path.len() { accounts[destination].0 } else { &path[hop + 1].2 };
                let inputs: Vec<_> = scoped
                    .iter()
                    .filter(|t| {
                        &t.source == source
                            && &t.destination == vault_in
                            && &t.mint == input_mint
                            && (hop > 0 || if native_sol_buy { t.source.as_slice() == accounts[authority].0.as_slice() }
                                else { t.authority.as_slice() == accounts[authority].0.as_slice() })
                            && (!native_sol_buy || t.depth == depth + 1 && successful_invocations.contains(&t.index)
                                && instructions[t.index as usize].program_id().0.iter().all(|byte| *byte == 0))
                    })
                    .collect();
                let outputs: Vec<_> = scoped
                    .iter()
                    .filter(|t| &t.source == vault_out && &t.destination == destination && &t.mint == output_mint
                        && (!native_sol_buy || t.depth == depth + 1 && t.authority.as_slice() == group[2].0.as_slice()
                            && successful_invocations.contains(&t.index)))
                    .collect();
                // A last curve hop pays native SOL directly from its own
                // lamports. Its declared WSOL accounts are only mint witnesses.
                // Bind one committed token receipt and native event to exact
                // pool/buyback deltas; no current fee settings are inferred.
                if is_curve && hop + 1 == path.len() && output_mint.as_slice() == crate::SOL_MINT
                    && group[1].0.as_slice() == crate::SOL_MINT
                {
                    if inputs.len() != 1 || !outputs.is_empty()
                        || accounts[8].0.as_slice() != token::PROGRAM_ID
                        || accounts[9].0.as_slice() != token_2022::PROGRAM_ID
                        || accounts[10].0.iter().any(|byte| *byte != 0)
                        || accounts[12].0.as_slice() != pumpfun::PROGRAM_ID
                        || !successful_invocations.contains(&(index as u32))
                    { continue; }
                    if path.iter().zip(&groups).any(|(leg, group)| leg.0 != *group[0].0) { continue; }
                    let initial_inputs: Vec<_> = scoped.iter().filter(|t| t.source == *accounts[5].0
                        && t.destination == path[0].2 && t.mint == path[0].0 && t.authority == *accounts[authority].0
                        && t.depth == depth + 1 && instructions[t.index as usize].program_id().0 == accounts[9].0
                        && successful_invocations.contains(&t.index)).collect();
                    if initial_inputs.len() != 1 || initial_inputs[0].amounts[0] != amount { continue; }
                    let input = inputs[0];
                    // Exact source conservation independently checks gross
                    // debits; an inferred destination fee cannot hide a bad CPI.
                    if [&initial_inputs[0].source, &input.source].iter().any(|source| {
                        let mint = mints.mint_for(source);
                        if mint.is_none() || other_token_operations.contains(*source)
                            || !observed_start.contains(*source)
                            || !observed_end.contains(*source) && !proven_closed.contains(*source)
                        { return true; }
                        let mint = mint.unwrap();
                        let incoming: Vec<_> = transfers.iter().filter(|t| &t.destination == *source && t.mint == mint).collect();
                        if incoming.iter().any(|t| !t.net_verified) { return true; }
                        let received: i128 = incoming.iter().map(|t| i128::from(*t.amounts.iter().min().unwrap())).sum();
                        deltas.get(*source).copied() != Some(received - debits.get(&((*source).clone(), mint)).copied().unwrap_or(0))
                    }) { continue; }
                    let input_authority = if hop == 0 { accounts[authority].0 } else { groups[hop - 1][2].0 };
                    if input.depth != depth + 1 || input.authority != *input_authority
                        || instructions[input.index as usize].program_id().0 != accounts[9].0
                        || !successful_invocations.contains(&input.index) || !input.net_verified
                        || mints.mint_for(vault_in).as_ref() != Some(input_mint)
                        || hop == 0 && input.amounts[0] != amount
                    { continue; }
                    let events: Vec<_> = instructions.iter().enumerate().skip(input.index as usize + 1)
                        .take(end as usize - input.index as usize - 1).filter_map(|(position, child)| {
                            let a = child.accounts();
                            if child.program_id().0 != &pumpfun::PROGRAM_ID || child.stack_height() != depth + 1
                                || a.len() != 1 || a[0].0 != accounts[11].0
                                || !successful_invocations.contains(&(position as u32))
                            { return None; }
                            let event = pumpfun::events::unpack_trade_event_minimal(child.data()).ok()??;
                            if event.is_buy || event.mint.as_slice() != input_mint.as_slice()
                                || event.user.as_slice() != accounts[user].0.as_slice()
                                || event.token_amount != *input.amounts.iter().min().unwrap()
                            { return None; }
                            let quote = pumpfun::events::unpack_quote_trade_amount(child.data()).ok()?;
                            (quote.mint.iter().all(|byte| *byte == 0) && quote.amount == event.sol_amount).then_some(quote)
                        }).collect();
                    if events.len() != 1 { continue; }
                    let quote = &events[0];
                    let Some(net) = quote.amount.checked_sub(quote.fee).and_then(|n| n.checked_sub(quote.creator_fee)).filter(|n| *n > 0)
                        else { continue };
                    let pool = group[2].0;
                    let recipient = accounts[7].0;
                    // The buyback recipient is a WSOL account. Its native
                    // credit uses a committed empty self-CPI before SyncNative.
                    // Count that exact settlement helper separately from swaps.
                    let settlements: Vec<_> = instructions.iter().enumerate().skip(input.index as usize + 1)
                        .take(end as usize - input.index as usize - 1).filter(|(position, child)| {
                            let a = child.accounts();
                            child.program_id().0 == &pumpfun::PROGRAM_ID && child.stack_height() == depth + 1
                                && child.data().as_slice() == [228, 69, 165, 46, 81, 203, 154, 29]
                                && a.len() == 3 && a[0].0 == accounts[11].0 && a[1].0 == pool && a[2].0 == recipient
                                && successful_invocations.contains(&(*position as u32))
                        }).collect();
                    if settlements.len() != 1 { continue; }
                    let settlement_index = settlements[0].0;
                    let Some(sync) = instructions.get(settlement_index + 1).filter(|_| settlement_index + 1 < end as usize) else { continue };
                    let sync_accounts = sync.accounts();
                    if sync.program_id().0 != accounts[8].0 || sync.stack_height() != depth + 1
                        || sync.data().as_slice() != [17] || sync_accounts.len() != 1 || sync_accounts[0].0 != recipient
                        || !successful_invocations.contains(&((settlement_index + 1) as u32))
                        || mints.mint_for(recipient).as_deref() != Some(crate::SOL_MINT.as_slice())
                    { continue; }
                    if pool == recipient || instructions.iter().filter(|call| call.program_id().0 == &pumpfun::PROGRAM_ID
                        && call.accounts().iter().any(|a| a.0 == pool)).count() != 2
                        || instructions.iter().filter(|call| call.program_id().0 == &pumpfun::PROGRAM_ID
                            && call.accounts().iter().any(|a| a.0 == recipient)).count() != 2
                        || transfers.iter().any(|t| t.mint == crate::SOL_MINT
                            && [&t.source, &t.destination].iter().any(|a| *a == pool || *a == recipient))
                    { continue; }
                    let Some(meta) = tx.meta.as_ref() else { continue };
                    let resolved = tx.resolved_accounts();
                    let Some(pool_position) = resolved.iter().position(|a| *a == pool) else { continue };
                    let Some(recipient_position) = resolved.iter().position(|a| *a == recipient) else { continue };
                    let Some(pool_delta) = meta.pre_balances.get(pool_position).zip(meta.post_balances.get(pool_position))
                        .map(|(before, after)| i128::from(*before) - i128::from(*after)) else { continue };
                    let Some(recipient_delta) = meta.post_balances.get(recipient_position).zip(meta.pre_balances.get(recipient_position))
                        .map(|(after, before)| i128::from(*after) - i128::from(*before)) else { continue };
                    if recipient_delta != i128::from(quote.buyback_fee)
                        || net.checked_add(quote.buyback_fee).map(i128::from) != Some(pool_delta)
                    { continue; }
                    native_sol_multihop_receipts.insert(index as u32, (input.index, net, pool.clone()));
                    swaps.push(pb::Swap {
                        protocol: pb::Protocol::Pumpfun as i32,
                        program_id: ix.program_id().0.clone(), amm: ix.program_id().0.clone(),
                        amm_pool: pool.clone(), user: accounts[user].0.clone(), stack_height: depth,
                        input_mint: input_mint.clone(), input_amount: input.amounts[0],
                        output_mint: output_mint.clone(), output_amount: net,
                        source_index: Some(index as u32), source_transfer_index: Some(input.index),
                        ..Default::default()
                    });
                    continue;
                }
                if inputs.len() != 1 || outputs.len() != 1 {
                    continue;
                }
                let (input, output) = (inputs[0], outputs[0]);
                if (hop == 0 && input.amounts[0] != amount)
                    || native_sol_buy && scoped.iter().any(|t| t.index != input.index && t.mint == crate::SOL_MINT
                        && (t.source == *accounts[user].0 || t.destination == *group[2].0))
                    || (output.fee_token && !output.net_verified)
                    || !native_sol_buy && mints.mint_for(vault_in).as_ref() != Some(input_mint)
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
    // Legacy/V2 native sells pay the user and all fees directly from owned
    // curve lamports. Build every independently token-proven contribution
    // before checking shared curve/fee credits and each isolated user's net.
    let mut legacy_native_sells = Vec::new();
    for (index, ix) in instructions.iter().enumerate() {
        if ix.program_id().0 != &pumpfun::PROGRAM_ID || !successful_invocations.contains(&(index as u32)) { continue; }
        let a = ix.accounts();
        let (roles, amount, minimum) = match pumpfun::instructions::unpack(ix.data()) {
            Ok(pumpfun::instructions::PumpFunInstruction::Sell(args)) if a.len() == 16 && a[7].0.iter().all(|b| *b == 0)
                && a[11].0.as_slice() == pumpfun::PROGRAM_ID => ([3, 2, 6, 5, 4, 9, 10, 1, 15, 8], args.amount, args.min_sol_output),
            Ok(pumpfun::instructions::PumpFunInstruction::SellV2(args)) if a.len() == 26 && a[2].0.as_slice() == crate::SOL_MINT
                && a[4].0.as_slice() == token::PROGRAM_ID && a[5].0 == &base58::decode("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL").unwrap() && a[23].0.iter().all(|b| *b == 0)
                && a[25].0.as_slice() == pumpfun::PROGRAM_ID => ([10, 1, 13, 14, 11, 3, 24, 6, 8, 16], args.amount, args.min_sol_output),
            _ => continue,
        };
        let [pool, mint, user, source, vault, token_program, event_authority, fee_recipient, buyback, creator] = roles;
        let protected = [pool, user, fee_recipient, buyback, creator];
        if a[mint].0.as_slice() == crate::SOL_MINT || a[source].0 == a[vault].0
            || (a[token_program].0.as_slice() != token::PROGRAM_ID && a[token_program].0.as_slice() != token_2022::PROGRAM_ID)
            || mints.mint_for(a[source].0).as_ref() != Some(a[mint].0) || mints.mint_for(a[vault].0).as_ref() != Some(a[mint].0)
            || protected.iter().enumerate().any(|(p, role)| protected[p + 1..].iter().any(|other| a[*role].0 == a[*other].0))
        { continue; }
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        let end = instructions.iter().enumerate().skip(index + 1).find(|(_, child)| child.is_root()
            || child.stack_height() > 0 && child.stack_height() <= depth).map(|(position, _)| position).unwrap_or(instructions.len());
        let base: Vec<_> = transfers.iter().filter(|t| t.index > index as u32 && (t.index as usize) < end && t.depth == depth + 1
            && t.source == *a[source].0 && t.destination == *a[vault].0 && t.authority == *a[user].0 && t.mint == *a[mint].0
            && instructions[t.index as usize].program_id().0 == a[token_program].0 && t.net_verified && successful_invocations.contains(&t.index)).collect();
        if base.len() != 1 || base[0].amounts[0] != amount || other_token_operations.contains(a[source].0) { continue; }
        let base = base[0];
        let initialized_zero = initializations.get(a[source].0) == Some(&1) && !observed_start.contains(a[source].0)
            && tx.resolved_accounts().iter().position(|key| *key == a[source].0)
                .and_then(|p| tx.meta.as_ref()?.pre_balances.get(p)).is_some_and(|balance| *balance == 0);
        if !observed_start.contains(a[source].0) && !initialized_zero
            || !observed_end.contains(a[source].0) && !proven_closed.contains(a[source].0)
                && !(initialized_zero && closes.get(a[source].0) == Some(&1))
        { continue; }
        let incoming: Vec<_> = transfers.iter().filter(|t| t.destination == *a[source].0 && t.mint == base.mint).collect();
        if incoming.iter().any(|t| !t.net_verified) { continue; }
        let received: i128 = incoming.iter().map(|t| i128::from(*t.amounts.iter().min().unwrap())).sum();
        if deltas.get(a[source].0).copied() != Some(received - debits.get(&(a[source].0.clone(), base.mint.clone())).copied().unwrap_or(0)) { continue; }
        let event_calls: Vec<_> = instructions.iter().enumerate().skip(index + 1).take(end - index - 1)
            .filter(|(_, child)| child.program_id().0 == &pumpfun::PROGRAM_ID
                && child.data().get(8..16) == Some(pumpfun::events::TRADE.as_slice())).collect();
        if event_calls.len() != 1 { continue; }
        let (event_index, event_ix) = event_calls[0];
        if event_ix.stack_height() != depth + 1 || event_ix.accounts().len() != 1 || event_ix.accounts()[0].0 != a[event_authority].0
            || !successful_invocations.contains(&(event_index as u32)) || event_ix.data().get(137..169) != Some(a[fee_recipient].0.as_slice())
        { continue; }
        let Ok(Some(event)) = pumpfun::events::unpack_trade_event_minimal(event_ix.data()) else { continue };
        let Ok(quote) = pumpfun::events::unpack_quote_trade_amount(event_ix.data()) else { continue };
        let Some(net) = quote.amount.checked_sub(quote.fee).and_then(|n| n.checked_sub(quote.creator_fee)) else { continue };
        if event.is_buy || event.mint.as_slice() != a[mint].0.as_slice() || event.user.as_slice() != a[user].0.as_slice()
            || event.token_amount != amount || quote.mint.iter().any(|b| *b != 0) || quote.amount != event.sol_amount
            || net == 0 || net < minimum || quote.fee < quote.buyback_fee
        { continue; }
        legacy_native_sells.push(PumpNativeSell { instruction: index, roles, input: base.index, net, gross: quote.amount,
            protocol_fee: quote.fee - quote.buyback_fee, creator_fee: quote.creator_fee, buyback_fee: quote.buyback_fee });
    }
    let mut legacy_native_receipts = HashMap::<u32, (u32, u64, Vec<u8>, Vec<u8>)>::new();
    if !legacy_native_sells.is_empty() {
        let resolved = tx.resolved_accounts();
        let mut native_changes = HashMap::<Vec<u8>, i128>::new();
        for t in transfers.iter().filter(|t| t.mint == crate::SOL_MINT && successful_invocations.contains(&t.index)) {
            let program = instructions[t.index as usize].program_id().0;
            if program.iter().all(|b| *b == 0) || program == &token::PROGRAM_ID
                && mints.mint_for(&t.source).as_ref().is_some_and(|mint| mint.as_slice() == crate::SOL_MINT)
                && mints.mint_for(&t.destination).as_ref().is_some_and(|mint| mint.as_slice() == crate::SOL_MINT)
            {
                *native_changes.entry(t.source.clone()).or_default() -= i128::from(t.amounts[0]);
                *native_changes.entry(t.destination.clone()).or_default() += i128::from(t.amounts[0]);
            }
        }
        for (position, ix) in instructions.iter().enumerate() {
            let a = ix.accounts();
            if ix.program_id().0.iter().all(|b| *b == 0) && ix.data().len() == 52 && ix.data()[..4] == [0; 4]
                && a.len() >= 2 && successful_invocations.contains(&(position as u32))
            {
                let amount = i128::from(u64::from_le_bytes(ix.data()[4..12].try_into().unwrap()));
                *native_changes.entry(a[0].0.clone()).or_default() -= amount;
                *native_changes.entry(a[1].0.clone()).or_default() += amount;
            }
        }
        if let Some(meta) = tx.meta.as_ref() {
            if let Some(payer) = resolved.first() { *native_changes.entry((*payer).clone()).or_default() -= i128::from(meta.fee); }
            let mut proven_volume_refunds = HashSet::new();
            let protected: HashSet<_> = legacy_native_sells.iter().flat_map(|sell| {
                let a = instructions[sell.instruction].accounts();
                [sell.roles[0], sell.roles[2], sell.roles[7], sell.roles[8], sell.roles[9]].map(|role| a[role].0.clone())
            }).collect();
            for (position, ix) in instructions.iter().enumerate() {
                let a = ix.accounts();
                if ix.program_id().0 != &pumpfun::PROGRAM_ID || ix.data().as_slice() != [249, 69, 164, 218, 150, 103, 84, 138]
                    || a.len() != 4 || a[3].0.as_slice() != pumpfun::PROGRAM_ID || a[0].0 == a[1].0
                    || protected.contains(a[1].0) || !successful_invocations.contains(&(position as u32))
                    || instructions.iter().filter(|call| call.program_id().0 == &pumpfun::PROGRAM_ID
                        && call.data().as_slice() == ix.data().as_slice() && call.accounts().get(1).is_some_and(|key| key.0 == a[1].0)).count() != 1
                    || instructions[position + 1..].iter().any(|later| later.accounts().iter().any(|key| key.0 == a[1].0))
                { continue; }
                let depth = if ix.is_root() { 1 } else { ix.stack_height() };
                let children: Vec<_> = instructions.iter().enumerate().skip(position + 1).take_while(|(_, child)| !child.is_root()
                    && (child.stack_height() == 0 || child.stack_height() > depth)).collect();
                if children.len() != 1 { continue; }
                let (event_index, event) = children[0];
                if event.program_id().0 != &pumpfun::PROGRAM_ID || event.stack_height() != depth + 1
                    || event.accounts().len() != 1 || event.accounts()[0].0 != a[2].0
                    || event.data().get(..16) != Some([228, 69, 165, 46, 81, 203, 154, 29, 146, 159, 189, 172, 146, 88, 56, 244].as_slice())
                    || event.data().get(16..48) != Some(a[0].0.as_slice()) || !successful_invocations.contains(&(event_index as u32))
                { continue; }
                let Some(index) = resolved.iter().position(|key| *key == a[1].0) else { continue };
                let Some((&before, &after)) = meta.pre_balances.get(index).zip(meta.post_balances.get(index)) else { continue };
                if after != 0 { continue; }
                let refund = i128::from(before) + native_changes.get(a[1].0).copied().unwrap_or(0);
                if refund < 0 { continue; }
                *native_changes.entry(a[1].0.clone()).or_default() -= refund;
                *native_changes.entry(a[0].0.clone()).or_default() += refund;
                proven_volume_refunds.insert(position);
            }
            for (position, ix) in instructions.iter().enumerate() {
                let a = ix.accounts();
                if (ix.program_id().0 != &token::PROGRAM_ID && ix.program_id().0 != &token_2022::PROGRAM_ID)
                    || ix.data().as_slice() != [9] || a.len() < 3 || !successful_invocations.contains(&(position as u32))
                    || closes.get(a[0].0) != Some(&1) || protected.contains(a[0].0)
                    || instructions[position + 1..].iter().any(|later| later.accounts().iter().any(|key| key.0 == a[0].0))
                { continue; }
                let Some(index) = resolved.iter().position(|key| *key == a[0].0) else { continue };
                let Some((&before, &after)) = meta.pre_balances.get(index).zip(meta.post_balances.get(index)) else { continue };
                if after != 0 { continue; }
                let refund = i128::from(before) + native_changes.get(a[0].0).copied().unwrap_or(0);
                if refund < 0 { continue; }
                *native_changes.entry(a[0].0.clone()).or_default() -= refund;
                *native_changes.entry(a[1].0.clone()).or_default() += refund;
            }
            for sell in &legacy_native_sells {
                let a = instructions[sell.instruction].accounts();
                for (role, change) in [(sell.roles[0], -i128::from(sell.gross)), (sell.roles[2], i128::from(sell.net)),
                    (sell.roles[7], i128::from(sell.protocol_fee)), (sell.roles[8], i128::from(sell.buyback_fee)),
                    (sell.roles[9], i128::from(sell.creator_fee))]
                { *native_changes.entry(a[role].0.clone()).or_default() += change; }
            }
            // A once-created, once-destroyed wrapper account can return rent
            // directly, without a System transfer. Prove its original funding
            // and exclusive owner invocations, then check every other account
            // available to the close invocation. Only the original funder can
            // receive its remainder; unknown CPI or balance changes reject it.
            for (position, create) in instructions.iter().enumerate() {
                let a = create.accounts();
                if create.program_id().0.iter().any(|b| *b != 0) || create.data().len() != 52
                    || create.data()[..4] != [0; 4] || a.len() != 2 || create.stack_height() != 2
                    || !successful_invocations.contains(&(position as u32)) || protected.contains(a[1].0)
                    || !legacy_native_sells.iter().any(|sell| instructions[sell.instruction].accounts()[sell.roles[2]].0 == a[0].0)
                { continue; }
                let Some(parent_position) = (0..position).rev().find(|p| instructions[*p].is_root()) else { continue };
                let parent = &instructions[parent_position];
                if parent.program_id().0.as_slice() != &create.data()[20..52]
                    || !successful_invocations.contains(&(parent_position as u32)) { continue; }
                let accesses: Vec<_> = instructions.iter().enumerate().filter(|(_, call)| call.accounts().iter().any(|key| key.0 == a[1].0)).collect();
                if accesses.len() != 3 || accesses[0].0 != parent_position || accesses[1].0 != position { continue; }
                let (close_position, close) = accesses[2];
                if close_position <= position || !close.is_root() || close.program_id().0 != parent.program_id().0
                    || !successful_invocations.contains(&(close_position as u32))
                    || !close.accounts().iter().any(|key| key.0 == a[0].0)
                    || instructions.iter().filter(|call| call.program_id().0 == parent.program_id().0).count() != 2
                { continue; }
                let children_valid = [parent_position, close_position].iter().all(|root| {
                    instructions.iter().enumerate().skip(root + 1).take_while(|(_, child)| !child.is_root()).all(|(p, child)|
                        successful_invocations.contains(&(p as u32)) && child.stack_height() == 2
                            && (p == position || child.program_id().0.iter().all(|b| *b == 0)
                                && child.data().len() == 12 && child.data()[..4] == [2, 0, 0, 0]))
                });
                let Some(source_position) = resolved.iter().position(|key| *key == a[1].0) else { continue };
                if !children_valid || meta.pre_balances.get(source_position) != Some(&0) || meta.post_balances.get(source_position) != Some(&0)
                    || parent.accounts().iter().chain(close.accounts().iter()).filter(|key| key.0 != a[0].0 && key.0 != a[1].0).any(|key| {
                        let observed = resolved.iter().position(|address| *address == key.0).and_then(|p|
                            meta.post_balances.get(p).zip(meta.pre_balances.get(p)).map(|(after, before)| i128::from(*after) - i128::from(*before)));
                        observed != Some(native_changes.get(key.0).copied().unwrap_or(0))
                    })
                { continue; }
                let refund = i128::from(u64::from_le_bytes(create.data()[4..12].try_into().unwrap()));
                if native_changes.get(a[1].0).copied() != Some(refund) { continue; }
                *native_changes.entry(a[1].0.clone()).or_default() -= refund;
                *native_changes.entry(a[0].0.clone()).or_default() += refund;
            }
            for sell in &legacy_native_sells {
                let a = instructions[sell.instruction].accounts();
                // One native payout per user makes its observed proceeds
                // independent even when several users sell into one curve.
                if legacy_native_sells.iter().filter(|other| instructions[other.instruction].accounts()[other.roles[2]].0 == a[sell.roles[2]].0).count() != 1 { continue; }
                let verified = [sell.roles[0], sell.roles[2], sell.roles[7], sell.roles[8], sell.roles[9]].iter().all(|role| {
                    let address = a[*role].0;
                    let exact = resolved.iter().position(|key| *key == address).and_then(|p|
                        meta.post_balances.get(p).zip(meta.pre_balances.get(p)).map(|(after, before)| i128::from(*after) - i128::from(*before)));
                    if exact != native_changes.get(address).copied() { return false; }
                    instructions.iter().enumerate().filter(|(_, call)| call.program_id().0 == &pumpfun::PROGRAM_ID
                        && call.accounts().iter().any(|key| key.0 == address)).all(|(position, call)| {
                        if legacy_native_sells.iter().any(|s| s.instruction == position) { return true; }
                        if proven_volume_refunds.contains(&position) { return true; }
                        // A canonical committed account extension can only
                        // contribute its separately recorded rent-funding CPI.
                        let roles = call.accounts();
                        if call.data().as_slice() != [234, 102, 194, 203, 150, 72, 62, 229] || roles.len() != 5
                            || roles[2].0.iter().any(|b| *b != 0) || roles[4].0.as_slice() != pumpfun::PROGRAM_ID
                            || !successful_invocations.contains(&(position as u32)) { return false; }
                        let depth = if call.is_root() { 1 } else { call.stack_height() };
                        instructions.iter().enumerate().skip(position + 1).take_while(|(_, child)| !child.is_root()
                            && (child.stack_height() == 0 || child.stack_height() > depth)).all(|(p, child)| {
                            let accounts = child.accounts();
                            successful_invocations.contains(&(p as u32)) && child.stack_height() == depth + 1
                                && (child.program_id().0.iter().all(|b| *b == 0) && child.data().len() == 12 && child.data()[..4] == [2, 0, 0, 0]
                                    && accounts.len() == 2 && accounts[0].0 == roles[1].0 && accounts[1].0 == roles[0].0
                                    || child.program_id().0 == &pumpfun::PROGRAM_ID && accounts.len() == 1 && accounts[0].0 == roles[3].0
                                        && child.data().get(..8) == Some([228, 69, 165, 46, 81, 203, 154, 29].as_slice()))
                        })
                    })
                });
                if verified { legacy_native_receipts.insert(sell.instruction as u32,
                    (sell.input, sell.net, a[sell.roles[0]].0.clone(), a[sell.roles[2]].0.clone())); }
            }
        }
    }
    // Several V3 swaps on distinct curves can pay the same buyback recipient.
    // A whole-transaction credit is usable only after every contributing call
    // independently proves its exact native settlement. Unknown calls still
    // count against the recipient below and cannot borrow another leg's fees.
    let mut v3_buyback_contributions = HashMap::<Vec<u8>, (usize, u64)>::new();
    for (index, ix) in instructions.iter().enumerate() {
        if ix.program_id().0 != &pumpfun::PROGRAM_ID || !successful_invocations.contains(&(index as u32)) { continue; }
        let (buy, amount, limit) = match pumpfun::instructions::unpack(ix.data()) {
            Ok(pumpfun::instructions::PumpFunInstruction::BuyV3(args)) => (true, args.amount, args.max_sol_cost),
            Ok(pumpfun::instructions::PumpFunInstruction::SellV3(args)) => (false, args.amount, args.min_sol_output),
            _ => continue,
        };
        let a = ix.accounts();
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        if a.len() != 17 || depth == 0 || a[2].0.as_slice() != crate::SOL_MINT
            || (a[3].0.as_slice() != token::PROGRAM_ID && a[3].0.as_slice() != token_2022::PROGRAM_ID)
            || a[4].0.as_slice() != token::PROGRAM_ID || a[14].0.iter().any(|byte| *byte != 0)
            || a[16].0.as_slice() != pumpfun::PROGRAM_ID || a[5].0 == a[13].0 || a[5].0 == a[8].0 || a[13].0 == a[8].0
            || mints.mint_for(a[6].0).as_ref() != Some(a[1].0) || mints.mint_for(a[9].0).as_ref() != Some(a[1].0)
            || instructions.iter().filter(|call| call.program_id().0 == &pumpfun::PROGRAM_ID
                && call.accounts().iter().any(|account| account.0 == a[5].0)).count() != 1
        { continue; }
        let end = instructions.iter().enumerate().skip(index + 1)
            .find(|(_, child)| child.is_root() || child.stack_height() > 0 && child.stack_height() <= depth)
            .map(|(end, _)| end).unwrap_or(instructions.len());
        let base: Vec<_> = transfers.iter().filter(|t| t.index > index as u32 && (t.index as usize) < end
            && t.depth == depth + 1 && t.mint.as_slice() == a[1].0.as_slice()
            && t.source == *a[if buy { 6 } else { 9 }].0 && t.destination == *a[if buy { 9 } else { 6 }].0
            && t.authority == *a[if buy { 5 } else { 8 }].0 && t.net_verified
            && instructions[t.index as usize].program_id().0 == a[3].0
            && successful_invocations.contains(&t.index)).collect();
        if base.len() != 1 || base[0].amounts[0] != amount { continue; }
        let base = base[0];
        // Source conservation independently checks the gross token debit;
        // destination fee arithmetic alone must not validate a changed CPI.
        let source = &base.source;
        if other_token_operations.contains(source) || !observed_start.contains(source)
            || !observed_end.contains(source) && !proven_closed.contains(source)
        { continue; }
        let incoming: Vec<_> = transfers.iter().filter(|t| t.destination == *source && t.mint == base.mint).collect();
        if incoming.iter().any(|t| !t.net_verified) { continue; }
        let received: i128 = incoming.iter().map(|t| i128::from(*t.amounts.iter().min().unwrap())).sum();
        if deltas.get(source).copied() != Some(received - debits.get(&(source.clone(), base.mint.clone())).copied().unwrap_or(0)) { continue; }
        if instructions[index + 1..end].iter().filter(|child| child.program_id().0 == &pumpfun::PROGRAM_ID
            && child.data().get(8..16) == Some(pumpfun::events::TRADE.as_slice())).count() != 1
        { continue; }
        let events: Vec<_> = instructions.iter().enumerate().skip(index + 1).take(end - index - 1)
            .filter_map(|(position, child)| {
                let accounts = child.accounts();
                if child.program_id().0 != &pumpfun::PROGRAM_ID || child.stack_height() != depth + 1
                    || accounts.len() != 1 || accounts[0].0 != a[15].0 || !successful_invocations.contains(&(position as u32))
                { return None; }
                let event = pumpfun::events::unpack_trade_event_minimal(child.data()).ok()??;
                if event.is_buy != buy || event.mint.as_slice() != a[1].0.as_slice() || event.user.as_slice() != a[8].0.as_slice()
                    || event.token_amount != if buy { base.amounts[0] } else { *base.amounts.iter().min().unwrap() }
                { return None; }
                let quote = pumpfun::events::unpack_quote_trade_amount(child.data()).ok()?;
                (quote.mint.iter().all(|byte| *byte == 0) && quote.amount == event.sol_amount).then_some(quote)
            }).collect();
        if events.len() != 1 { continue; }
        let quote = &events[0];
        let user_lamports = if buy { quote.amount.checked_add(quote.fee).and_then(|n| n.checked_add(quote.creator_fee)) }
            else { quote.amount.checked_sub(quote.fee).and_then(|n| n.checked_sub(quote.creator_fee)) };
        let Some(user_lamports) = user_lamports.filter(|n| *n > 0 && if buy { *n <= limit } else { *n >= limit }) else { continue };
        let touches: Vec<_> = transfers.iter().filter(|t| t.mint == crate::SOL_MINT
            && (t.source == *a[5].0 || t.destination == *a[5].0)).collect();
        // A buy has one gross wallet-to-curve payment. A sell can have one
        // earlier curve-growth payment; it is funding, never sale proceeds.
        if touches.len() > 1 || buy && touches.len() != 1 { continue; }
        let funding = if let Some(t) = touches.first() {
            if t.source != *a[8].0 || t.destination != *a[5].0 || t.index <= index as u32 || t.index as usize >= end
                || t.depth != depth + 1 || !instructions[t.index as usize].program_id().0.iter().all(|byte| *byte == 0)
                || !successful_invocations.contains(&t.index) || if buy { t.amounts[0] != user_lamports } else { t.index >= base.index }
            { continue; }
            t.amounts[0]
        } else { 0 };
        let Some(meta) = tx.meta.as_ref() else { continue };
        let resolved = tx.resolved_accounts();
        let Some(position) = resolved.iter().position(|account| *account == a[5].0) else { continue };
        let Some(pool_delta) = meta.post_balances.get(position).zip(meta.pre_balances.get(position))
            .map(|(after, before)| i128::from(*after) - i128::from(*before)) else { continue };
        let expected = if buy { user_lamports.checked_sub(quote.buyback_fee).map(i128::from) }
            else { user_lamports.checked_add(quote.buyback_fee).map(|n| i128::from(funding) - i128::from(n)) };
        if expected != Some(pool_delta) { continue; }
        let entry = v3_buyback_contributions.entry(a[13].0.clone()).or_default();
        // Overflow leaves the proven count incomplete, so aggregate credit
        // cannot pass even when another valid contribution follows it.
        if let Some(total) = entry.1.checked_add(quote.buyback_fee) { entry.0 += 1; entry.1 = total; }
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
                if ix.program_id().0 == &crate::native_venues::ORCA_V1
                    || ix.program_id().0 == &substreams_solana_idls::spl::token_swap::PROGRAM_ID
                {
                    let Some(layout) = crate::native_venues::layout(ix) else { continue };
                    if !successful_invocations.contains(&(index as u32)) {
                        continue;
                    }
                    let inputs: Vec<_> = scoped.iter().filter(|t| {
                        t.depth == depth + 1 && t.source == *accounts[3].0
                            && t.destination == *accounts[4].0 && t.authority == *accounts[2].0
                            && instructions[t.index as usize].program_id().0 == accounts[9].0
                            && successful_invocations.contains(&t.index)
                    }).collect();
                    let outputs: Vec<_> = scoped.iter().filter(|t| {
                        t.depth == depth + 1 && t.source == *accounts[5].0
                            && t.destination == *accounts[6].0 && t.authority == *accounts[1].0
                            && instructions[t.index as usize].program_id().0 == accounts[9].0
                            && successful_invocations.contains(&t.index)
                    }).collect();
                    if inputs.len() != 1 || outputs.len() != 1 {
                        continue;
                    }
                    let (input, output) = (inputs[0], outputs[0]);
                    // SPL v2 swaps mint owner/optional host fees in LP tokens,
                    // between the user's input and output transfers. Neither
                    // a liquidity operation nor an arbitrary MintTo is a swap.
                    let calls = &instructions[index + 1..end as usize];
                    if !(2..=4).contains(&calls.len()) || input.index != index as u32 + 1
                        || output.index != end - 1 || accounts[7].0 == &input.mint || accounts[7].0 == &output.mint
                        || calls.iter().enumerate().any(|(offset, child)| {
                            child.stack_height() != depth + 1 || child.program_id().0 != accounts[9].0
                                || child.data().len() != 9 || child.accounts().len() != 3
                                || !successful_invocations.contains(&((index + offset + 1) as u32))
                        })
                    { continue; }
                    let fees = &calls[1..calls.len() - 1];
                    if fees.len() == 2 && accounts.len() != 11
                        || fees.iter().enumerate().any(|(position, fee)| {
                            let a = fee.accounts();
                            let destination = if fees.len() == 2 && position == 0 { 10 } else { 8 };
                            fee.data()[0] != 7 || a[0].0 != accounts[7].0 || a[1].0 != accounts[destination].0
                                || a[2].0 != accounts[1].0
                                || mints.mint_for(a[1].0).as_ref() != Some(accounts[7].0)
                                || [3, 4, 5, 6].iter().any(|i| accounts[*i].0 == a[1].0)
                        })
                    { continue; }
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if swap.amm_pool != *accounts[0].0 || swap.user != *accounts[2].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || input.mint == output.mint
                        || mints.mint_for(accounts[4].0).as_ref() != Some(&input.mint)
                        || mints.mint_for(accounts[5].0).as_ref() != Some(&output.mint)
                        || input.amounts[0] > layout.amount || net < layout.minimum_output
                        || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index.min(output.index))
                        || native_evidence.contains(&identity)
                        || scoped.iter().any(|t| t.index != input.index && t.index != output.index
                            && [t.source.as_slice(), t.destination.as_slice()].iter()
                                .any(|account| *account == accounts[3].0.as_slice() || *account == accounts[6].0.as_slice()))
                    {
                        continue;
                    }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if let Some(&(input_index, net, ref pool)) = native_sol_multihop_receipts.get(&(index as u32)).filter(|receipt| receipt.2 == swap.amm_pool) {
                    let input = transfers.iter().find(|t| t.index == input_index).unwrap();
                    let identity = (index as u32, input_index, input_index);
                    if swap.protocol == pb::Protocol::Pumpfun as i32 && swap.amm_pool == *pool && swap.user == *accounts[3].0
                        && swap.input_mint == input.mint && swap.output_mint == crate::SOL_MINT
                        && (swap.input_amount, swap.output_amount) == (input.amounts[0], net)
                        && !expected_transfer.is_some_and(|position| position != input_index)
                        && !native_evidence.contains(&identity)
                    {
                        evidence = Some(identity);
                        break 'candidate;
                    }
                    continue;
                }
                if ix.program_id().0 == &crate::oasis::PROGRAM_ID {
                    let Some(&(input_position, output_position)) = oasis_receipts.get(&(index as u32)) else { continue };
                    let (input, output) = (&transfers[input_position], &transfers[output_position]);
                    let identity = (index as u32, input.index, output.index);
                    if swap.amm_pool == *accounts[1].0 && swap.user == *accounts[8].0
                        && swap.input_mint == input.mint && swap.output_mint == output.mint
                        && (swap.input_amount, swap.output_amount) == (input.amounts[0], output.amounts[0])
                        && !expected_transfer.is_some_and(|transfer| transfer != input.index)
                        && !native_evidence.contains(&identity)
                    {
                        evidence = Some(identity);
                        break 'candidate;
                    }
                    continue;
                }
                if ix.program_id().0 == &crate::mercurial::PROGRAM_ID {
                    let Some(&(input_position, output_position)) = mercurial_receipts.get(&(index as u32)) else { continue };
                    let (input, output) = (&transfers[input_position], &transfers[output_position]);
                    let identity = (index as u32, input.index, output.index);
                    if swap.amm_pool == *accounts[0].0 && swap.user == *accounts[3].0
                        && swap.input_mint == input.mint && swap.output_mint == output.mint
                        && (swap.input_amount, swap.output_amount) == (input.amounts[0], output.amounts[0])
                        && !expected_transfer.is_some_and(|transfer| transfer != input.index)
                        && !native_evidence.contains(&identity)
                    {
                        evidence = Some(identity);
                        break 'candidate;
                    }
                    continue;
                }
                if let Some(legs) = two_hop_receipts.get(&(index as u32)) {
                    for hop in 0..2 {
                        let (input, output) = (&transfers[legs[hop]], &transfers[legs[hop + 1]]);
                        let identity = (index as u32, input.index, output.index);
                        if swap.amm_pool == *accounts[hop].0
                            && swap.user == *accounts[14].0
                            && swap.input_mint == input.mint
                            && swap.output_mint == output.mint
                            && swap.input_amount == input.amounts[0]
                            && swap.output_amount == *output.amounts.iter().min().unwrap()
                            && !expected_transfer.is_some_and(|transfer| transfer != input.index.min(output.index))
                            && !native_evidence.contains(&identity)
                        {
                            evidence = Some(identity);
                            break 'candidate;
                        }
                    }
                    continue;
                }
                // Pump's sell instruction debits program-owned lamports
                // directly, without a System CPI. Only accept its exact pool
                // balance delta when this is the sole pool-bearing invocation
                // and no ambiguous explicit SOL transfers touch that pool.
                if swap.protocol == pb::Protocol::Pumpfun as i32
                    && swap.output_mint == crate::SOL_MINT
                    && matches!(
                        pumpfun::instructions::unpack(ix.data()),
                        Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)
                            | pumpfun::instructions::PumpFunInstruction::SellV2(_)
                            | pumpfun::instructions::PumpFunInstruction::SellV3(_))
                    )
                {
                    if matches!(pumpfun::instructions::unpack(ix.data()),
                        Ok(pumpfun::instructions::PumpFunInstruction::Sell(_))) && accounts.len() == 16
                        || matches!(pumpfun::instructions::unpack(ix.data()),
                            Ok(pumpfun::instructions::PumpFunInstruction::SellV2(_))) && accounts.len() == 26
                    {
                        if let Some((input_index, net, pool, user)) = legacy_native_receipts.get(&(index as u32)) {
                            let input = transfers.iter().find(|t| t.index == *input_index).unwrap();
                            let identity = (index as u32, *input_index, *input_index);
                            if swap.amm_pool == *pool && swap.user == *user && swap.input_mint == input.mint
                                && swap.input_amount == input.amounts[0] && !native_evidence.contains(&identity)
                                && expected_transfer.is_none_or(|position| position == *input_index)
                                && (expand_native || swap.output_amount == *net)
                            {
                                if swap.output_amount != *net {
                                    let mut original = swap.clone();
                                    original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                                    rejected.push(original);
                                }
                                swap.output_amount = *net;
                                evidence = Some(identity);
                                break 'candidate;
                            }
                        }
                        continue;
                    }
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
                        && matches!(
                            pumpfun::instructions::unpack(ix.data()),
                            Ok(pumpfun::instructions::PumpFunInstruction::SellV3(_))
                        )
                    {
                        let amounts = instructions
                            .iter()
                            .enumerate()
                            .skip(index + 1)
                            .take(end as usize - index - 1)
                            .filter(|(index, child)| child.program_id().0 == &pumpfun::PROGRAM_ID
                                && successful_invocations.contains(&(*index as u32)))
                            .find_map(|(_, child)| {
                                let event = pumpfun::events::unpack_trade_event_minimal(child.data()).ok()??;
                                if event.is_buy || event.mint != swap.input_mint.as_slice() || event.user != swap.user.as_slice() {
                                    return None;
                                }
                                pumpfun::events::unpack_quote_trade_amount(child.data()).ok()
                            });
                        let proven = amounts.and_then(|amounts| {
                            // V3 can grow an older curve account before selling.
                            // A committed wallet-to-curve System transfer is
                            // funding, not proceeds. Add it back to the pool's
                            // net debit; every other pool-touching SOL flow is
                            // still ambiguous and rejected.
                            let (funding, funding_index) = if explicit_sol {
                                let touches: Vec<_> = transfers.iter().filter(|t| t.mint == crate::SOL_MINT
                                    && (t.source == swap.amm_pool || t.destination == swap.amm_pool)).collect();
                                if touches.len() != 1 { return None; }
                                let t = touches[0];
                                if accounts.len() != 17 || t.source != swap.user || t.destination != swap.amm_pool
                                    || t.index <= index as u32 || t.index >= end || t.depth != depth + 1
                                    || !instructions[t.index as usize].program_id().0.iter().all(|byte| *byte == 0)
                                    || !successful_invocations.contains(&(index as u32)) || !successful_invocations.contains(&t.index)
                                { return None; }
                                (t.amounts[0], Some(t.index))
                            } else { (0, None) };
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
                            let recipient_credit = if recipient_calls == 1 { Some(amounts.buyback_fee) }
                                else { v3_buyback_contributions.get(recipient)
                                    .filter(|(proven_calls, _)| *proven_calls == recipient_calls).map(|(_, total)| *total) };
                            if net == 0
                                || explicit_recipient_sol
                                || recipient_credit.map(i128::from) != Some(delta)
                                || pool_delta.map(|delta| delta + i128::from(funding)) != Some(i128::from(net.checked_add(amounts.buyback_fee)?))
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
                                    && (funding == 0 || successful_invocations.contains(&t.index))
                            })?;
                            if funding_index.is_some_and(|index| index >= input.index) { return None; }
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
                // DLMM events report gross token transfers. Use only the
                // declared native user/reserve legs to correct a fee receipt.
                if swap.protocol == pb::Protocol::MeteoraDlmm as i32 {
                    let Some(layout) = crate::meteora_dlmm::decode_swap_instruction(ix) else {
                        continue;
                    };
                    if layout.lb_pair != swap.amm_pool || layout.user != swap.user {
                        continue;
                    }
                    let (input_reserve, output_reserve, input_program, output_program) =
                        if swap.input_mint == layout.token_x_mint && swap.output_mint == layout.token_y_mint {
                            (&layout.reserve_x, &layout.reserve_y, &layout.token_x_program, &layout.token_y_program)
                        } else if swap.input_mint == layout.token_y_mint && swap.output_mint == layout.token_x_mint {
                            (&layout.reserve_y, &layout.reserve_x, &layout.token_y_program, &layout.token_x_program)
                        } else {
                            continue;
                        };
                    let inputs: Vec<_> = scoped
                        .iter()
                        .filter(|t| {
                            t.depth == depth + 1
                                && t.mint == swap.input_mint
                                && t.source == layout.user_token_in
                                && &t.destination == input_reserve
                                && t.authority == layout.user
                                && instructions[t.index as usize].program_id().0 == input_program
                        })
                        .collect();
                    let outputs: Vec<_> = scoped
                        .iter()
                        .filter(|t| {
                            t.depth == depth + 1
                                && t.mint == swap.output_mint
                                && &t.source == output_reserve
                                && t.destination == layout.user_token_out
                                && t.authority == layout.lb_pair
                                && instructions[t.index as usize].program_id().0 == output_program
                        })
                        .collect();
                    if inputs.len() != 1
                        || outputs.len() != 1
                        || !outputs[0].net_verified
                        || inputs[0].amounts[0] != swap.input_amount
                        || !outputs[0].amounts.contains(&swap.output_amount)
                    {
                        continue;
                    }
                    let (input, output) = (inputs[0], outputs[0]);
                    let identity = (index as u32, input.index, output.index);
                    if expected_transfer.is_some_and(|transfer| transfer != input.index.min(output.index)) || native_evidence.contains(&identity) {
                        continue;
                    }
                    let net = *output.amounts.iter().min().unwrap();
                    if swap.output_amount != net {
                        let mut original = swap.clone();
                        original.source_index = Some(index as u32);
                        original.source_transfer_index = Some(input.index.min(output.index));
                        original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                        rejected.push(original);
                        swap.output_amount = net;
                    }
                    // The event CPI runs one level deeper than its swap.
                    // Fee metadata can make that event verifiable directly,
                    // so always retain the matched native invocation's depth.
                    swap.stack_height = depth;
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0 == &substreams_solana_idls::meteora::daam::PROGRAM_ID {
                    let Some(&(input_position, output_position)) = damm_receipts.get(&(index as u32)) else {
                        continue;
                    };
                    let (input, output) = (&transfers[input_position], &transfers[output_position]);
                    if swap.amm_pool != *accounts[1].0
                        || swap.user != *accounts[8].0
                        || swap.input_mint != input.mint
                        || swap.output_mint != output.mint
                        || expected_transfer.is_some_and(|position| position != input.index.min(output.index))
                    {
                        continue;
                    }
                    let identity = (index as u32, input.index, output.index);
                    if native_evidence.contains(&identity) {
                        continue;
                    }
                    let net = *output.amounts.iter().min().unwrap();
                    if (swap.input_amount, swap.output_amount) != (input.amounts[0], net) {
                        let mut original = swap.clone();
                        original.source_index = Some(index as u32);
                        original.source_transfer_index = Some(input.index.min(output.index));
                        original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                        rejected.push(original);
                    }
                    swap.input_amount = input.amounts[0];
                    swap.output_amount = net;
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0.as_slice() == substreams_solana::b58!("FLUXubRmkEi2q6K3Y9kBPg9248ggaZVsoSFhtJHSrm1X") {
                    let Some(layout) = crate::native_venues::layout(ix) else { continue };
                    let inputs: Vec<_> = scoped
                        .iter()
                        .filter(|t| {
                            t.depth == depth + 1
                                && t.source == *accounts[3].0
                                && t.destination == *accounts[4].0
                                && t.authority == *accounts[2].0
                                && t.mint == *accounts[9].0
                                && instructions[t.index as usize].program_id().0 == accounts[11].0
                        })
                        .collect();
                    let outputs: Vec<_> = scoped
                        .iter()
                        .filter(|t| {
                            t.depth == depth + 1
                                && t.source == *accounts[5].0
                                && t.destination == *accounts[6].0
                                && t.authority == *accounts[1].0
                                && t.mint == *accounts[10].0
                                && instructions[t.index as usize].program_id().0 == accounts[12].0
                        })
                        .collect();
                    if inputs.len() != 1 || outputs.len() != 1 {
                        continue;
                    }
                    let (input, output) = (inputs[0], outputs[0]);
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if swap.amm_pool != *accounts[0].0
                        || swap.user != *accounts[2].0
                        || swap.input_mint != input.mint
                        || swap.output_mint != output.mint
                        || input.amounts[0] > layout.amount
                        || net < layout.minimum_output
                        || (output.fee_token && !output.net_verified)
                        || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index.min(output.index))
                        || native_evidence.contains(&identity)
                    {
                        continue;
                    }
                    evidence = Some(identity);
                    break 'candidate;
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
                } else if swap.protocol == pb::Protocol::JupiterRfq as i32 {
                    crate::rfq::layout(ix).map(|fill| (accounts[fill.input_source].0.clone(), accounts[fill.output_destination].0.clone()))
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
    fn finalized_dlmm_native_depth_is_independent_of_fee_lookup() {
        let tx: ConfirmedTransaction = substreams::proto::decode(&include_bytes!("../fixtures/token-fees/dlmm-native-depth.transaction.pb").to_vec()).unwrap();
        let native = tx.walk_instructions().nth(5).unwrap();
        assert_eq!(native.stack_height(), 2);
        assert_eq!(native.program_id().0, &substreams_solana_idls::meteora::dlmm::PROGRAM_ID);
        for metadata_available in [false, true] {
            let decoded = crate::process_transaction_with_metadata(tx.clone(), Some(455893380), |_, _| None, |_, _| None,
                |_, _| metadata_available.then_some(crate::token_fees::TransferFee { basis_points: 0, maximum_fee: 0 })).unwrap();
            let swap = decoded.swaps.iter().find(|s| s.source_index == Some(5)).unwrap();
            assert_eq!((swap.input_amount, swap.output_amount), (1076685, 1230636845));
            assert_eq!(swap.source_transfer_index, Some(7));
            assert_eq!(swap.stack_height, 2);
            assert!(swap.transfer_verified);
        }
    }

    #[test]
    fn finalized_native_sol_multihop_sells_retain_exact_net_lamports() {
        // Amounts come from independent token/reserve and buyback balance proofs.
        let cases: &[(&[u8], u32, u32, u64, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-1.transaction.pb"), 7, 10, 48461705044, 1954063),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-2.transaction.pb"), 5, 8, 4581765332578, 298987882),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-3.transaction.pb"), 7, 10, 2098011953543, 104373622),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-4.transaction.pb"), 7, 10, 6457640989385, 375091096),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-5.transaction.pb"), 5, 8, 18604350247708, 966350547),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-6.transaction.pb"), 7, 10, 1332355492220, 65999938),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-7.transaction.pb"), 5, 8, 223025273342, 7480281),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-8.transaction.pb"), 5, 8, 9608082829, 515882),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-9.transaction.pb"), 7, 10, 57026121234, 3014773),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-10.transaction.pb"), 7, 10, 17670471773, 1160717),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-11.transaction.pb"), 5, 8, 26741534976982, 1473276973),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-12.transaction.pb"), 5, 8, 9589486165628, 285692843),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-13.transaction.pb"), 7, 10, 1382987880258, 69102091),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-14.transaction.pb"), 7, 10, 2141497211958, 107475397),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-15.transaction.pb"), 7, 10, 1193223508861, 44832292),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-16.transaction.pb"), 5, 8, 95943653669590, 2930489199),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-17.transaction.pb"), 5, 8, 37043602365279, 1434049791),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-18.transaction.pb"), 7, 10, 1034697410229, 35057362),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-19.transaction.pb"), 5, 8, 49435474364362, 1747155080),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-20.transaction.pb"), 5, 8, 96131703973627, 2930493081),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-21.transaction.pb"), 5, 8, 49525436833680, 1747156229),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-22.transaction.pb"), 9, 12, 932631130121, 31896284),
            (include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-23.transaction.pb"), 5, 8, 36979388876189, 1434049498),
        ];
        for &(bytes, native_index, transfer_index, input_atoms, net_lamports) in cases {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let decoded = crate::process_transaction(tx).unwrap();
            let legs: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(native_index)
                && s.output_mint == crate::SOL_MINT).collect();
            assert_eq!(legs.len(), 1, "native SOL sell source {native_index}");
            let leg = legs[0];
            assert_eq!(leg.source_transfer_index, Some(transfer_index));
            assert_eq!((leg.input_amount, leg.output_amount), (input_atoms, net_lamports));
            assert_eq!(leg.protocol, pb::Protocol::Pumpfun as i32);
            assert_eq!(leg.stack_height, 3);
            assert!(leg.transfer_verified);
            assert!(!decoded.coverage_candidates.iter().any(|c| c.reason == "multihop_without_all_verified_pools"));
        }
    }

    #[test]
    fn native_sol_multihop_sell_rejects_inconsistent_settlement() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-sell-1.transaction.pb").to_vec(),
        ).unwrap();
        let original = &tx.meta.as_ref().unwrap().inner_instructions.iter().find(|g| g.index == 1).unwrap().instructions;
        let native = original[1].accounts.clone();
        let input = original[4].accounts.clone();
        for mode in 0..33 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_mut().unwrap();
            let calls = &mut meta.inner_instructions.iter_mut().find(|g| g.index == 1).unwrap().instructions;
            match mode {
                0 => calls[1].data[8] ^= 1,
                1 => calls[4].data[1] ^= 1,
                2 => calls[4].accounts[3] = native[3],
                3 => calls[4].accounts[0] = native[5],
                4 => calls[4].accounts[2] = native[7],
                5 => calls[4].program_id_index = calls[1].program_id_index,
                6 => calls[4].stack_height = Some(5),
                7 => calls[7].data[16] ^= 1,
                8 => calls[7].data[56] ^= 1,
                9 => calls[7].data[48] ^= 1,
                10 => calls[7].data[65] ^= 1,
                11 => calls[7].data.truncate(16),
                12 => calls[7].accounts[0] = native[3],
                13 => calls[7].stack_height = Some(5),
                14 => calls[5].data.push(0),
                15 => calls[5].accounts[1] = native[3],
                16 => calls[5].accounts[2] = native[6],
                17 => calls[5].accounts[0] = native[3],
                18 => calls[5].stack_height = Some(5),
                19 => calls[6].data[0] = 9,
                20 => calls[6].accounts[0] = native[6],
                21 => calls[6].program_id_index = calls[4].program_id_index,
                22 => calls[6].stack_height = Some(5),
                23 => meta.post_balances[native[20] as usize] += 1,
                24 => meta.post_balances[native[7] as usize] += 1,
                25 | 26 => {
                    let balances = if mode == 25 { &mut meta.pre_token_balances } else { &mut meta.post_token_balances };
                    let account = if mode == 25 { input[0] } else { input[2] };
                    let balance = balances.iter_mut().find(|b| b.account_index == u32::from(account)).unwrap();
                    let amount = balance.ui_token_amount.as_mut().unwrap();
                    amount.amount = (amount.amount.parse::<u64>().unwrap() + 1).to_string();
                }
                27 => calls[1].accounts[8] = native[9],
                28 => calls[1].accounts[11] = native[3],
                29 => calls[1].data.push(0),
                30 => calls[1].accounts[19] = native[18],
                31 | 32 => {
                    let program = base58::encode(pumpfun::PROGRAM_ID);
                    let last = meta.log_messages.iter().rposition(|l| l == &format!("Program {program} success")).unwrap();
                    if mode == 31 { meta.log_messages.truncate(last); }
                    else { meta.log_messages[last] = format!("Program {program} failed: custom program error: 1"); }
                }
                _ => unreachable!(),
            }
            assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|s|
                !(s.source_index == Some(7) && s.output_mint == crate::SOL_MINT))), "native SOL sell mutation {mode}");
        }
    }

    #[test]
    fn finalized_native_sol_multihop_buy_keeps_both_physical_curve_legs() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/pump-native-sol-multihop-buy.transaction.pb").to_vec(),
        ).unwrap();
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        assert_eq!(decoded.swaps.len(), 2);
        let first = &decoded.swaps[0];
        assert_eq!((first.protocol, first.source_index, first.source_transfer_index), (pb::Protocol::Pumpfun as i32, Some(9), Some(10)));
        assert_eq!((first.input_amount, first.output_amount), (31_238_843, 896_182_574_050));
        assert_eq!(first.input_mint, crate::SOL_MINT);
        assert_eq!((decoded.swaps[1].input_amount, decoded.swaps[1].output_amount), (896_182_574_050, 3_898_557_180_441));
        assert_eq!(decoded.swaps[1].source_transfer_index, Some(14));
        assert!(!decoded.coverage_candidates.iter().any(|c| c.reason == "multihop_without_all_verified_pools"));
        for mode in 0..17 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_mut().unwrap();
            let calls = &mut meta.inner_instructions.iter_mut().find(|g| g.index == 3).unwrap().instructions;
            let a = calls[1].accounts.clone();
            match mode {
                0 => calls[1].data[8] ^= 1,
                1 => calls[2].data[4] ^= 1,
                2 => calls[2].accounts[0] = a[15],
                3 => calls[2].accounts[1] = a[3],
                4 => calls[2].stack_height = Some(5),
                5 => calls[6].data[1..9].fill(0),
                6 => calls[6].accounts[3] = a[3],
                7 => calls[6].accounts[0] = a[6],
                8 => calls[6].stack_height = Some(5),
                9 => { let duplicate = calls[2].clone(); calls.insert(3, duplicate); }
                10 | 11 | 12 | 13 => {
                    let program = if mode == 12 { base58::encode(substreams_solana_idls::pumpswap::PROGRAM_ID) }
                        else if mode == 13 { base58::encode(token_2022::PROGRAM_ID) } else { base58::encode(pumpfun::PROGRAM_ID) };
                    let end = if mode == 12 || mode == 13 { meta.log_messages.iter().position(|l| l == &format!("Program {program} success")).unwrap() }
                        else { meta.log_messages.iter().rposition(|l| l == &format!("Program {program} success")).unwrap() };
                    if mode == 10 { meta.log_messages.truncate(end); }
                    else { meta.log_messages[end] = format!("Program {program} failed: custom program error: 1"); }
                }
                14 => calls[1].accounts[8] = a[9],
                15 => calls[1].data.push(0),
                _ => calls[1].accounts[14] = a[13],
            }
            assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|s|
                !(s.source_index == Some(9) && s.input_mint == crate::SOL_MINT))), "native SOL multi-hop mode {mode}");
        }
    }

    #[test]
    fn finalized_legacy_and_v2_native_sells_have_exact_net_receipts() {
        let fixtures: &[(&[u8], &[(u32, u32, u64, u64)])] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/4b5xRRH61QCrSiWCjKCjeXAtGmeja1e6MAi4CpFgLdrQSjM1Ro5RbfH1rdSVLkT2n1YiNX81SBWaS2oQ7PZpSCEn.transaction.pb").as_slice(), &[(3, 6, 133963973296, 3710455)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/5Xw7D7N2u5RxmdrYKvbntnhWKP3T6or8buRqQsZ7Yurni1S3M4SUWaCDeTJCWXBpLFfD9rQobf5XteiEFxXTKM6Z.transaction.pb").as_slice(), &[(3, 6, 13062704252309, 368099300)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/3mAJZQwLDEZpJzACW2jfHhbLzXvaB2j3bkGN1hTj9mbTj7QWVsRWfCNcsNUuqTLFZ4VV91RZo2uPK85kTnYw6zFP.transaction.pb").as_slice(), &[(3, 6, 1128916629009, 31201657)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/5Pz9g2q6UnNiY9V4fpr7TsJJtwxDB7sf1PfttcY5628Rzy42mpvgVFtopr8RpLdc2AJxUTAFSb42VcwkZycV3MQP.transaction.pb").as_slice(), &[(3, 6, 358276105896, 10238019)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/4JofKx1AzNjqCUqctMGHUnVZK8dyJttqHhjs7rVjF39Qn8B8XM1MNdkVvi7srSar6tqS9YUJvrTQdgipCdM41Xxf.transaction.pb").as_slice(), &[(3, 5, 195724127499, 5404867)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/5iHZfCrUvrWtsXMSKiHfWFAxLo4FzgUUqbMhXA4jVPqh1eC4jZQmpVdRrWeajEWhCHM72nT95xjBfXUqx1qVkTsU.transaction.pb").as_slice(), &[(17, 19, 353134727201, 9763296)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/26Q6hJXhxRNXy9LmtkSQ8eX9p8CcjZxk4f82JxGGgxwCFJqtxEV6NcZd8cUmfdWrN188vKS4GvyhuTHGBpfNN2pd.transaction.pb").as_slice(), &[(3, 6, 911379232096, 25380819)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/3xFXJ4jbLaEfwLACBU8EDV5amTtyLBqCRfPPM8weMbDc27U7wujQsUTefQsb1YoCqCRoWa5FqDdUQYPrwiUSHLPi.transaction.pb").as_slice(), &[(8, 11, 23958121557, 662020)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/35qbhnWqhQ4e7eAJ4yzHhCLVvAcPsTQGdTXXEFjvZ32WnRjNZw96b4uDeBMBUEdYAfuVGWU6nAtMgVY3vkBgRVVN.transaction.pb").as_slice(), &[(2, 4, 103975629473576, 4156222862), (6, 8, 7127223838410, 251661465), (10, 12, 7037325258482, 244818582), (14, 16, 6826357804497, 234071979)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/4dH6TUc4NgzxAGG27zY6WmbfD2cKk1eeew5BL5BnoyDVLZP4QPi3ne2rw1ygycVGkzuYMTmAmw4xXuPnUTxbMo9Z.transaction.pb").as_slice(), &[(7, 10, 295281484069, 8187298)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/5AVsjgf2xiAyeeoJmYjKkRvGBVDMXes5T3VpdKA3P2Y9mJQSvqXaoCr6cYSF22Qmn3wifnix7JAzHRkAMxJS4UX9.transaction.pb").as_slice(), &[(3, 6, 2115327702796, 10665)]),
            (include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/4jaQ6JV9xBoQYqXs8tai5uk2sf1ZcQppFTJxj3SanSV9tDR7UYysbp8HjqZ57M76wArDBr3oazNzRU6JkJ1HDH1n.transaction.pb").as_slice(), &[(4, 6, 8302045803901, 253796500)]),
        ];
        for (bytes, legs) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let decoded = crate::process_transaction(tx).unwrap();
            for &(source, transfer, input, output) in *legs {
                let sell = decoded.swaps.iter().find(|swap| swap.source_index == Some(source))
                    .expect("independently settled legacy/V2 native sell");
                assert_eq!((sell.input_amount, sell.output_amount, sell.source_transfer_index), (input, output, Some(transfer)));
                assert_eq!(sell.output_mint, crate::SOL_MINT);
                assert!(sell.transfer_verified);
            }
        }
    }

    #[test]
    fn native_legacy_sells_reject_inconsistent_settlement_and_unknown_contributors() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/4b5xRRH61QCrSiWCjKCjeXAtGmeja1e6MAi4CpFgLdrQSjM1Ro5RbfH1rdSVLkT2n1YiNX81SBWaS2oQ7PZpSCEn.transaction.pb").to_vec(),
        ).unwrap();
        let root_program = base58::encode(tx.walk_instructions().nth(2).unwrap().program_id().0);
        let pump_program = base58::encode(pumpfun::PROGRAM_ID);
        let token_program = base58::encode(token_2022::PROGRAM_ID);
        for mode in 0..33 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_mut().unwrap();
            let calls = &mut meta.inner_instructions.iter_mut().find(|group| group.index == 2).unwrap().instructions;
            let a = calls[0].accounts.clone();
            match mode {
                0..=4 => {
                    let role = [6, 8, 16, 13, 10][mode];
                    meta.post_balances[a[role] as usize] += 1;
                }
                5 => calls[0].data[8] ^= 1,
                6 => calls[0].data[16..24].copy_from_slice(&3_710_456u64.to_le_bytes()),
                7 => calls[3].data[1] ^= 1,
                8 => calls[4].data[48] ^= 1,
                9 => calls[4].data[56] ^= 1,
                10 => calls[4].data[177] ^= 1,
                11 => calls[4].data[225] ^= 1,
                12 => calls[4].data[137] ^= 1,
                13 => calls[4].data[65] ^= 1,
                14 => calls[4].data[64] = 1,
                15 => calls[4].accounts[0] = a[13],
                16 => calls[3].accounts[0] = a[11],
                17 => calls[3].accounts[2] = a[14],
                18 => calls[3].accounts[3] = a[10],
                19 => calls[0].accounts[10] = a[13],
                20 => calls[0].accounts[3] = a[4],
                21 => calls[0].accounts[2] = a[1],
                22 | 23 => {
                    let mut extra = calls[0].clone();
                    extra.accounts = vec![a[if mode == 22 { 10 } else { 8 }]];
                    extra.data = vec![0; 8];
                    calls.push(extra);
                    let position = meta.log_messages.iter().rposition(|line| line == &format!("Program {root_program} success")).unwrap();
                    meta.log_messages.splice(position..position, [format!("Program {pump_program} invoke [2]"), format!("Program {pump_program} success")]);
                }
                24 => {
                    let extra = calls[4].clone(); calls.push(extra);
                    let position = meta.log_messages.iter().position(|line| line == &format!("Program {pump_program} success")).unwrap() + 1;
                    meta.log_messages.splice(position..position, [format!("Program {pump_program} invoke [3]"), format!("Program {pump_program} success")]);
                }
                25..=27 => {
                    let target = if mode == 25 { &root_program } else if mode == 26 { &pump_program } else { &token_program };
                    let position = meta.log_messages.iter().position(|line| line == &format!("Program {target} success")).unwrap();
                    meta.log_messages[position] = format!("Program {target} failed: custom program error: 0x1");
                }
                28 => meta.log_messages.clear(),
                29 => calls[1].data[4] ^= 1,
                30 | 31 => {
                    let index = u32::from(a[if mode == 30 { 14 } else { 11 }]);
                    let balances = if mode == 30 { &mut meta.pre_token_balances } else { &mut meta.post_token_balances };
                    let amount = balances.iter_mut().find(|balance| balance.account_index == index).unwrap().ui_token_amount.as_mut().unwrap();
                    amount.amount = (amount.amount.parse::<u64>().unwrap() + 1).to_string();
                }
                32 => calls[0].accounts[5] = a[4],
                _ => unreachable!(),
            }
            assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|swap|
                swap.source_index != Some(3))), "legacy/V2 native settlement mutation {mode}");
        }
        // Rent returned by a volume-account close must have its own committed
        // event and balance proof before it can be separated from sale proceeds.
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/17.transaction.pb").to_vec(),
        ).unwrap();
        assert!(crate::process_transaction(tx.clone()).unwrap().swaps.iter().any(|swap|
            swap.source_index == Some(9) && swap.output_amount == 411_317_077));
        for mode in 0..8 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_mut().unwrap();
            let calls = &mut meta.inner_instructions.iter_mut().find(|group| group.index == 3).unwrap().instructions;
            let close = calls[9].accounts.clone();
            match mode {
                0 => calls[9].accounts[0] = close[1],
                1 => calls[9].accounts[1] = close[0],
                2 => calls[9].data[0] ^= 1,
                3 => calls[10].accounts[0] = close[0],
                4 => calls[10].data[16] ^= 1,
                5 => calls[10].stack_height = Some(2),
                6 => meta.pre_balances[close[1] as usize] += 1,
                7 => meta.post_balances[close[1] as usize] = 1,
                _ => unreachable!(),
            }
            assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|swap|
                swap.source_index != Some(9))), "native volume refund mutation {mode}");
        }
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/pump-legacy-v2-sells/4jaQ6JV9xBoQYqXs8tai5uk2sf1ZcQppFTJxj3SanSV9tDR7UYysbp8HjqZ57M76wArDBr3oazNzRU6JkJ1HDH1n.transaction.pb").to_vec(),
        ).unwrap();
        for mode in 0..10 {
            let mut altered = tx.clone();
            let message = altered.transaction.as_mut().unwrap().message.as_mut().unwrap();
            let meta = altered.meta.as_mut().unwrap();
            let protocol_fee = meta.inner_instructions.iter().find(|group| group.index == 2).unwrap().instructions[0].accounts[6];
            let create = &mut meta.inner_instructions.iter_mut().find(|group| group.index == 1).unwrap().instructions[0];
            let source = create.accounts[1] as usize;
            match mode {
                0 => create.accounts[0] = protocol_fee,
                1 => create.data[20] ^= 1,
                2 => create.accounts[1] = protocol_fee,
                3 => create.stack_height = Some(3),
                4 => meta.pre_balances[source] = 1,
                5 => meta.post_balances[source] = 1,
                6 => message.instructions[3].program_id_index = message.instructions[2].program_id_index,
                7 => message.instructions[3].accounts[1] = message.instructions[3].accounts[0],
                8 => {
                    let fee = message.instructions[3].accounts[3] as usize;
                    meta.post_balances[fee] += 1;
                }
                9 => meta.inner_instructions.iter_mut().find(|group| group.index == 3).unwrap().instructions[0].data[4] ^= 1,
                _ => unreachable!(),
            }
            assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|swap|
                swap.source_index != Some(4))), "native wrapper rent refund mutation {mode}");
        }
    }

    #[test]
    fn finalized_v3_swaps_share_only_independently_proven_buyback_credit() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/pump-v3-shared-buyback-recipient.transaction.pb").to_vec(),
        ).unwrap();
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        let sell = decoded.swaps.iter().find(|s| s.source_index == Some(11)).expect("independently settled native sell");
        assert_eq!((sell.input_amount, sell.output_amount, sell.source_transfer_index), (7_670_888_229_084, 228_543_099, Some(12)));
        assert_eq!(sell.output_mint, crate::SOL_MINT);
        let buy = decoded.swaps.iter().find(|s| s.source_index == Some(23)).unwrap();
        assert_eq!((buy.input_amount, buy.output_amount, buy.source_transfer_index), (228_543_098, 1_329_769_873_727, Some(24)));
        assert_eq!(decoded.swaps.len(), 2);
        assert!(decoded.swaps.iter().all(|s| s.transfer_verified));
        let root_program = base58::encode(tx.walk_instructions().nth(2).unwrap().program_id().0);
        for mode in 0..37 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_mut().unwrap();
            let calls = &mut meta.inner_instructions.iter_mut().find(|g| g.index == 2).unwrap().instructions;
            let sell = calls[8].accounts.clone();
            let buy = calls[20].accounts.clone();
            match mode {
                0 => meta.post_balances[sell[13] as usize] += 1,
                1 => meta.post_balances[sell[5] as usize] += 1,
                2 => meta.post_balances[buy[5] as usize] += 1,
                3 => calls[9].data[1] ^= 1,
                4 => calls[21].data[1] ^= 1,
                5 => calls[22].data[4] ^= 1,
                6 => calls[10].data[56] ^= 1,
                7 => calls[23].data[56] ^= 1,
                8 => calls[10].data[64] = 1,
                9 => calls[23].data[64] = 0,
                10 => calls[20].accounts[13] = buy[8],
                11 => calls[20].accounts[5] = sell[5],
                12 => calls[10].accounts[0] = sell[13],
                13 => calls[23].accounts[0] = buy[13],
                14 => calls[20].data[8] ^= 1,
                15 => calls[8].data[8] ^= 1,
                16..=19 => {
                    let account = match mode { 16 => sell[6], 17 => sell[9], 18 => buy[6], _ => buy[9] };
                    let balance = meta.post_token_balances.iter_mut().find(|b| b.account_index == u32::from(account)).unwrap().ui_token_amount.as_mut().unwrap();
                    balance.amount = (balance.amount.parse::<u64>().unwrap() + 1).to_string();
                }
                20 => { let duplicate = calls[10].clone(); calls.insert(11, duplicate); }
                21 => { let duplicate = calls[23].clone(); calls.insert(24, duplicate); }
                22 | 23 | 36 => {
                    let position = if mode == 22 { 87 } else if mode == 23 { 85 } else {
                        meta.log_messages.iter().rposition(|l| *l == format!("Program {root_program} success")).unwrap()
                    };
                    let program = if mode == 36 { root_program.clone() } else { base58::encode(pumpfun::PROGRAM_ID) };
                    assert_eq!(meta.log_messages[position], format!("Program {program} success"));
                    meta.log_messages[position] = format!("Program {program} failed: custom program error: 1");
                }
                24 => calls[22].accounts[0] = sell[5],
                25 => calls[21].accounts[3] = buy[8],
                26 => calls[9].accounts[3] = sell[5],
                27 => calls[21].program_id_index = calls[22].program_id_index,
                28 | 29 => {
                    let (event, amount) = if mode == 28 { (23, 1_072_177u64) } else { (10, 1_099_321u64) };
                    let offsets: Vec<_> = calls[event].data.windows(8).enumerate().filter_map(|(position, bytes)|
                        (bytes == amount.to_le_bytes()).then_some(position)).collect();
                    assert_eq!(offsets.len(), 1);
                    calls[event].data[offsets[0]] ^= 1;
                }
                30 => {
                    let mut payment = calls[22].clone();
                    payment.accounts = vec![sell[8], sell[13]];
                    payment.data[4..12].copy_from_slice(&1u64.to_le_bytes());
                    calls.push(payment);
                }
                31 => calls[23].data.truncate(16),
                32 => calls[23].data[65] ^= 1,
                33 => calls[20].accounts[2] = buy[1],
                34 => calls[20].stack_height = Some(3),
                _ => { let mut unknown = calls[20].clone(); unknown.data = vec![0]; calls.push(unknown); }
            }
            assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|s|
                s.source_index != Some(11))), "shared buyback settlement mode {mode}");
        }
    }

    #[test]
    fn finalized_v3_sell_separates_curve_funding_from_sale_proceeds() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/pump-native-sol-sell-funded.transaction.pb").to_vec(),
        ).unwrap();
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        let sell = decoded.swaps.iter().find(|s| s.source_index == Some(8)).unwrap();
        assert_eq!((sell.input_amount, sell.output_amount, sell.source_transfer_index), (71_818_190_491, 1_988_665, Some(10)));
        assert_eq!(sell.output_mint, crate::SOL_MINT);
        assert_eq!(decoded.swaps.iter().filter(|s| s.source_index == Some(8)).count(), 1);
        for mode in 0..14 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_mut().unwrap();
            let calls = &mut meta.inner_instructions.iter_mut().find(|g| g.index == 3).unwrap().instructions;
            let a = calls[0].accounts.clone();
            match mode {
                0 => calls[1].data[4] ^= 1,
                1 => calls[1].accounts[0] = a[5],
                2 => calls[1].accounts[1] = a[8],
                3 => calls[1].stack_height = Some(4),
                4 => calls[2].data[1] ^= 1,
                5 => calls[2].accounts[3] = a[5],
                6 => meta.post_balances[a[5] as usize] += 1,
                7 => meta.post_balances[a[13] as usize] += 1,
                8 | 9 => {
                    let program = if mode == 8 { base58::encode(pumpfun::PROGRAM_ID) } else { base58::encode([0; 32]) };
                    let start = meta.log_messages.iter().position(|l| l == &format!("Program {} invoke [2]", base58::encode(pumpfun::PROGRAM_ID))).unwrap();
                    let end = meta.log_messages.iter().enumerate().skip(start).find(|(_, l)| **l == format!("Program {program} success")).unwrap().0;
                    meta.log_messages[end] = format!("Program {program} failed: custom program error: 1");
                }
                10 => { let duplicate = calls[1].clone(); calls.insert(2, duplicate); }
                11 => calls.swap(1, 2),
                12 => calls[0].accounts[13] = a[8],
                _ => calls[1].program_id_index = calls[2].program_id_index,
            }
            assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|s|
                s.source_index != Some(8))), "funded V3 sell mode {mode}");
        }
        // An implicit SOL payout has no output Token/System CPI. Preserve a
        // known sell intent for investigation even when only one mint remains
        // visible and no event can prove proceeds.
        let mut opaque = tx;
        let calls = &mut opaque.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g| g.index == 3).unwrap().instructions;
        calls[1].data.clear();
        calls[3].data.clear();
        let decoded = crate::process_transaction(opaque).unwrap();
        assert!(decoded.swaps.iter().all(|s| s.source_index != Some(8)));
        assert!(decoded.coverage_candidates.iter().any(|c| c.source_index == 8
            && c.reason == "recognized_swap_without_verified_flows" && c.mints.len() == 1));
    }

    #[test]
    fn observed_accounts_burned_and_closed_after_a_swap_prove_the_receipt() {
        use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions};
        for (fixture, expected_input, expected_output) in [
            (
                include_bytes!("../fixtures/token-fees/dbc-burned-closed-output-1.transaction.pb").as_slice(),
                1_991_396u64,
                191_137_459_852u64,
            ),
            (
                include_bytes!("../fixtures/token-fees/dbc-burned-closed-output-2.transaction.pb").as_slice(),
                9_960_986u64,
                955_273_467_705u64,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let native = tx.walk_instructions().nth(8).unwrap();
            let destination = native.accounts()[4].0.clone();
            drop(native);
            let position = tx.resolved_accounts().iter().position(|key| **key == destination).unwrap() as u32;
            let start = tx
                .meta
                .as_ref()
                .unwrap()
                .pre_token_balances
                .iter()
                .find(|balance| balance.account_index == position)
                .unwrap()
                .clone();
            assert_eq!(start.ui_token_amount.as_ref().unwrap().amount, "0");
            let decode = |transaction| crate::process_transaction(transaction).unwrap();
            let decoded = decode(tx.clone());
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(8)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!(swaps[0].protocol, pb::Protocol::MeteoraDbc as i32);
            assert_eq!((swaps[0].input_amount, swaps[0].output_amount), (expected_input, expected_output));
            assert!(swaps[0].transfer_verified);
            let burn_data = tx.walk_instructions().nth(21).unwrap().data().clone();
            assert_eq!(burn_data[0], 8);
            assert_eq!(u64::from_le_bytes(burn_data[1..9].try_into().unwrap()), expected_output);
            // A nonzero observed starting balance must be subtracted, rather
            // than treating every successfully closed account as starting at 0.
            let mut nonzero_start = tx.clone();
            nonzero_start
                .meta
                .as_mut()
                .unwrap()
                .pre_token_balances
                .iter_mut()
                .find(|balance| balance.account_index == position)
                .unwrap()
                .ui_token_amount
                .as_mut()
                .unwrap()
                .amount = "100".into();
            for group in &mut nonzero_start.meta.as_mut().unwrap().inner_instructions {
                for instruction in &mut group.instructions {
                    if instruction.data == burn_data {
                        instruction.data[1..9].copy_from_slice(&(expected_output + 100).to_le_bytes());
                    }
                }
            }
            let decoded = decode(nonzero_start);
            assert_eq!(
                decoded.swaps.iter().find(|swap| swap.source_index == Some(8)).unwrap().output_amount,
                expected_output
            );
            for mode in 0..7 {
                let mut altered = tx.clone();
                match mode {
                    0 => altered
                        .meta
                        .as_mut()
                        .unwrap()
                        .pre_token_balances
                        .retain(|balance| balance.account_index != position),
                    1 => altered.meta.as_mut().unwrap().post_token_balances.push(start.clone()),
                    2 => {
                        altered
                            .transaction
                            .as_mut()
                            .unwrap()
                            .message
                            .as_mut()
                            .unwrap()
                            .instructions
                            .iter_mut()
                            .find(|ix| ix.data.as_slice() == [9] && ix.accounts.first() == Some(&(position as u8)))
                            .unwrap()
                            .data[0] = 5
                    }
                    3 => {
                        let message = altered.transaction.as_mut().unwrap().message.as_mut().unwrap();
                        let close = message
                            .instructions
                            .iter()
                            .find(|ix| ix.data.as_slice() == [9] && ix.accounts.first() == Some(&(position as u8)))
                            .unwrap()
                            .clone();
                        message.instructions.push(close);
                    }
                    4 => {
                        for group in &mut altered.meta.as_mut().unwrap().inner_instructions {
                            for instruction in &mut group.instructions {
                                if instruction.data == burn_data {
                                    instruction.data[1..9].copy_from_slice(&(expected_output + 1).to_le_bytes());
                                }
                            }
                        }
                    }
                    5 => {
                        altered
                            .meta
                            .as_mut()
                            .unwrap()
                            .pre_token_balances
                            .iter_mut()
                            .find(|balance| balance.account_index == position)
                            .unwrap()
                            .ui_token_amount
                            .as_mut()
                            .unwrap()
                            .amount = "100".into()
                    }
                    6 => {
                        let close = altered
                            .transaction
                            .as_mut()
                            .unwrap()
                            .message
                            .as_mut()
                            .unwrap()
                            .instructions
                            .iter_mut()
                            .find(|ix| ix.data.as_slice() == [9] && ix.accounts.first() == Some(&(position as u8)))
                            .unwrap();
                        close.accounts[0] = close.accounts[1];
                    }
                    _ => unreachable!(),
                }
                assert!(
                    decode(altered).swaps.iter().all(|swap| swap.source_index != Some(8)),
                    "closed receipt mode {mode}"
                );
            }
            // Model the same close through a CPI. Its enclosing invocation
            // must finish too; transaction success alone cannot prove a child.
            let mut cpi_close = tx.clone();
            let message = cpi_close.transaction.as_mut().unwrap().message.as_mut().unwrap();
            let close_root = message
                .instructions
                .iter()
                .position(|ix| ix.data.as_slice() == [9] && ix.accounts.first() == Some(&(position as u8)))
                .unwrap();
            let close = message.instructions[close_root].clone();
            let wrapper = message.instructions[0].clone();
            let wrapper_program = base58::encode(&message.account_keys[wrapper.program_id_index as usize]);
            let token_program = base58::encode(&message.account_keys[close.program_id_index as usize]);
            message.instructions[close_root] = wrapper;
            let meta = cpi_close.meta.as_mut().unwrap();
            meta.inner_instructions.push(InnerInstructions {
                index: close_root as u32,
                instructions: vec![InnerInstruction {
                    program_id_index: close.program_id_index,
                    accounts: close.accounts,
                    data: close.data,
                    stack_height: Some(2),
                }],
            });
            let original_close = meta
                .log_messages
                .iter()
                .rposition(|line| line == &format!("Program {token_program} invoke [1]"))
                .unwrap();
            meta.log_messages.truncate(original_close);
            meta.log_messages.extend([
                format!("Program {wrapper_program} invoke [1]"),
                format!("Program {token_program} invoke [2]"),
                format!("Program {token_program} success"),
                format!("Program {wrapper_program} success"),
            ]);
            assert_eq!(
                decode(cpi_close.clone())
                    .swaps
                    .iter()
                    .find(|swap| swap.source_index == Some(8))
                    .unwrap()
                    .output_amount,
                expected_output
            );
            for mode in 0..3 {
                let mut altered = cpi_close.clone();
                let logs = &mut altered.meta.as_mut().unwrap().log_messages;
                match mode {
                    0 => {
                        logs.truncate(logs.len() - 2);
                    }
                    1 => {
                        let index = logs.len() - 2;
                        logs[index] = format!("Program {token_program} failed: custom program error: 0x1");
                    }
                    2 => {
                        *logs.last_mut().unwrap() = format!("Program {wrapper_program} failed: custom program error: 0x1");
                    }
                    _ => unreachable!(),
                }
                assert!(
                    decode(altered).swaps.iter().all(|swap| swap.source_index != Some(8)),
                    "unproven CPI close mode {mode}"
                );
            }
        }
    }

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
