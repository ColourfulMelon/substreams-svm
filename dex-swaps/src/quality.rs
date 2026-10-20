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
    independent_proceeds: bool,
    cashback: Option<(usize, u64)>,
}

// Primary Token Batch ABI: bounded framing and exact account consumption.
// This supplies conservation ledger entries, never normalized swap principals.
fn legacy_batch_ledger(
    tx: &ConfirmedTransaction,
    ix: &substreams_solana::block_view::InstructionView,
    index: u32,
    mints: &TokenMintLookup,
    successful: &HashSet<u32>,
) -> Option<Vec<(Vec<u8>, Vec<u8>, Vec<u8>, u64)>> {
    let program = ix.program_id();
    let data = ix.data();
    let accounts = ix.accounts();
    if program.0 != &token::PROGRAM_ID || data.first() != Some(&255)
        || !successful.contains(&index)
        || tx.meta.as_option()?.err.is_set() { return None; }
    let mut cursor = 1usize;
    let mut account_cursor = 0usize;
    let mut children = Vec::new();
    let resolved = tx.resolved_accounts();
    let meta = tx.meta.as_option()?;
    while cursor < data.len() {
        let count = usize::from(*data.get(cursor)?);
        let length = usize::from(*data.get(cursor + 1)?);
        cursor += 2;
        if count != 3 || length != 9 { return None; }
        let child = data.get(cursor..cursor.checked_add(length)?)?;
        let roles = accounts.get(account_cursor..account_cursor.checked_add(count)?)?;
        if child[0] != 3 { return None; }
        let mint = mints.mint_for(roles[0].0)?;
        if mints.mint_for(roles[1].0).as_ref() != Some(&mint) { return None; }
        for role in &roles[..2] {
            let position = resolved.iter().position(|key| *key == role.0)?;
            let balances: Vec<_> = meta.pre_token_balances.iter().chain(&meta.post_token_balances)
                .filter(|b| b.account_index as usize == position).collect();
            if balances.is_empty() || balances.iter().any(|b| b.mint != base58::encode(&mint)
                || b.program_id != base58::encode(token::PROGRAM_ID)) { return None; }
        }
        children.push((mint, roles[0].0.clone(), roles[1].0.clone(), u64::from_le_bytes(child[1..9].try_into().ok()?)));
        cursor += length;
        account_cursor += count;
    }
    if children.is_empty() || account_cursor != accounts.len() { return None; }
    Some(children)
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
    let caught_failure = tx.meta.as_option().is_some_and(|meta| {
        meta.log_messages.iter().any(|log| {
            let mut words = log.split_whitespace();
            words.next() == Some("Program") && words.next().is_some() && words.next() == Some("failed:")
        })
    });
    let successful_invocations = if caught_failure
        || instructions.iter().any(|ix| {
            ix.program_id().0 == &token_2022::PROGRAM_ID
                || ix.program_id().0 == &token::PROGRAM_ID && ix.data().first() == Some(&255)
                || ix.program_id().0 == &crate::native_venues::ORCA_V1
                || ix.program_id().0 == &substreams_solana_idls::spl::token_swap::PROGRAM_ID
                || ix.program_id().0 == &crate::mercurial::PROGRAM_ID
                || ix.program_id().0 == &crate::oasis::PROGRAM_ID
                || ix.program_id().0 == &crate::saber::PROGRAM_ID
                || ix.program_id().0 == &substreams_solana_idls::solfi::v2::PROGRAM_ID && matches!(ix.data().len(), 25 | 153)
                || crate::jupiter_lend::layout(ix).is_some()
                || crate::rfq::layout(ix).is_some()
                || crate::spl_swap_forks::layout(ix).is_some()
                || crate::proprietary_swaps::layout(ix).is_some()
                || crate::riptide::layout(ix).is_some()
                || crate::archer::layout(ix).is_some()
                || crate::triangle::layout(ix).is_some()
                || crate::quay::layout(ix).is_some()
                || crate::anchor_markets::layout(ix).is_some()
                || crate::omnipair::layout(ix).is_some()
                || crate::fraudsworth::layout(ix).is_some()
                || crate::jupiter_perps::layout(ix).is_some()
                || crate::numeraire::layout(ix).is_some()
                || crate::futarchy::layout(ix).is_some()
                || crate::guacswap::layout(ix).is_some()
                || crate::anchor_swap::layout(ix).is_some()
                || crate::raydium_cpmm_fork::layout(ix).is_some()
                || crate::exponent::layout(ix).is_some()
                || crate::janus::layout(ix).is_some()
                || crate::peg::layout(ix).is_some()
                || crate::sanctum_infinity::layout(ix).is_some()
                || crate::gator::layout(ix).is_some()
                || crate::suffixed_anchor_swaps::applies(ix)
                || crate::current_proprietary::applies(ix)
                || crate::hadron::layout(ix).is_some()
                || crate::legacy_amm::layout(ix).is_some()
                || crate::picon_dlmm::layout(ix).is_some()
                || crate::m0_native_order::layout(ix).is_some()
                || crate::patch_bxlw::layout(ix).is_some()
                || crate::pancakeswap::legacy_layout(ix).is_some()
                || ix.program_id().0 == &substreams_solana_idls::meteora::daam::PROGRAM_ID
                || ix.program_id().0 == &substreams_solana_idls::meteora::dlmm::PROGRAM_ID
                || ix.program_id().0 == &pumpfun::PROGRAM_ID && (ix.data().starts_with(&[225, 154, 117, 22, 215, 86, 246, 103])
                    || ix.data().starts_with(&pumpfun::instructions::SELL_V3)
                    || matches!(crate::pumpfun::unpack_trade_instruction(ix.data()), Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)
                        | pumpfun::instructions::PumpFunInstruction::SellV2(_))))
                || crate::native_venues::defituna_two_hop(ix).is_some()
                || ix.program_id().0.as_slice() == substreams_solana_macro::b58!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH") && ix.data().first() == Some(&17)
                || ix.program_id().0.as_slice() == substreams_solana_macro::b58!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp")
                    && ix.data().len() == 25 && ix.data()[24] == (48 ^ 58 ^ 3)
        }) {
        crate::logs::successful_invocations(tx)
    } else {
        HashSet::new()
    };
    let archer_fills = crate::archer::fills(tx);
    let mut transfers = Vec::new();
    let mut other_token_operations = HashSet::new();
    let mut initializations = HashMap::<Vec<u8>, usize>::new();
    let mut closes = HashMap::<Vec<u8>, usize>::new();
    let mut burns = HashMap::<(Vec<u8>, Vec<u8>), i128>::new();
    let mut batch_ledger = Vec::<(Vec<u8>, Vec<u8>, Vec<u8>, u64)>::new();
    for (index, ix) in instructions.iter().enumerate() {
        if caught_failure && !successful_invocations.contains(&(index as u32)) {
            continue;
        }
        let program = ix.program_id();
        let accounts = ix.accounts();
        let data = ix.data();
        // Legacy Token Batch (255) serializes [account_count, data_length,
        // instruction_data] children against sequential account slices. Only
        // exact three-account Transfer children enter this ledger-only path.
        // They never become synthetic principal evidence or source indices.
        let batch = legacy_batch_ledger(tx, ix, index as u32, mints, &successful_invocations);
        let parsed_batch = batch.is_some();
        if let Some(children) = batch { batch_ledger.extend(children); }
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
            && !parsed_batch
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
    for (mint, source, destination, amount) in batch_ledger {
        *known_receipts.entry((destination, mint.clone())).or_default() += i128::from(amount);
        *debits.entry((source, mint)).or_default() += i128::from(amount);
    }
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
    if let Some(meta) = tx.meta.as_option() {
        for (balances, sign) in [(&meta.pre_token_balances, -1i128), (&meta.post_token_balances, 1)] {
            for balance in balances {
                if let (Some(account), Some(amount)) = (accounts.get(balance.account_index as usize), balance.ui_token_amount.as_option()) {
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
    // These Anchor venues require exact whole-account conservation in
    // addition to their two native principal CPIs. A missing balance cannot
    // turn a transfer label into a verified price.
    // Fresh, closed Anchor user accounts have neither token snapshot. Prove
    // their exact lifecycle and the complete transaction's native ledger,
    // including rent funding/refunds and WSOL unwrapping, before using them.
    let mut fresh_anchor_closes = HashSet::new();
    if instructions.iter().any(|ix| crate::anchor_markets::layout(ix).is_some()
        || crate::omnipair::layout(ix).is_some() || crate::fraudsworth::layout(ix).is_some())
        && closes.iter().any(|(account,count)| *count==1 && initializations.get(account)==Some(&1) && !observed_balances.contains(account))
    {
        if let Some(meta) = tx.meta.as_option() {
            let keys = tx.resolved_accounts();
            let mut native = HashMap::<Vec<u8>,i128>::new();
            let mut valid = true;
            for (position,call) in instructions.iter().enumerate() {
                let a=call.accounts(); let d=call.data(); let p=call.program_id().0;
                if p.iter().all(|v| *v==0) {
                    let amount = if d.len()==52 && d[..4]==[0;4] && a.len()==2 {
                        Some(u64::from_le_bytes(d[4..12].try_into().unwrap()))
                    } else if d.len()==12 && d[..4]==[2,0,0,0] && a.len()==2 {
                        Some(u64::from_le_bytes(d[4..12].try_into().unwrap()))
                    } else { None };
                    let Some(amount)=amount else { valid=false; break; };
                    if !successful_invocations.contains(&(position as u32)) { valid=false; break; }
                    *native.entry(a[0].0.clone()).or_default()-=i128::from(amount);
                    *native.entry(a[1].0.clone()).or_default()+=i128::from(amount);
                } else if [token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&p.as_slice()) {
                    if let Some(t)=transfers.iter().find(|t|t.index==position as u32 && t.mint.as_slice()==crate::SOL_MINT) {
                        if p.as_slice()!=token::PROGRAM_ID || !successful_invocations.contains(&t.index) { valid=false; break; }
                        *native.entry(t.source.clone()).or_default()-=i128::from(t.amounts[0]);
                        *native.entry(t.destination.clone()).or_default()+=i128::from(t.amounts[0]);
                    } else if d.as_slice()==[9] {
                        if a.len()!=3 || !successful_invocations.contains(&(position as u32)) { valid=false; break; }
                        let Some(index)=keys.iter().position(|key|*key==a[0].0) else { valid=false; break; };
                        let Some(before)=meta.pre_balances.get(index) else { valid=false; break; };
                        let residual=i128::from(*before)+native.get(a[0].0).copied().unwrap_or(0);
                        if residual<0 || meta.post_balances.get(index)!=Some(&0) { valid=false; break; }
                        *native.entry(a[0].0.clone()).or_default()-=residual;
                        *native.entry(a[1].0.clone()).or_default()+=residual;
                    }
                }
            }
            if let Some(payer)=keys.first() { *native.entry((*payer).clone()).or_default()-=i128::from(meta.fee); }
            if valid && keys.iter().enumerate().all(|(index,key)| meta.post_balances.get(index).zip(meta.pre_balances.get(index))
                .map(|(after,before)|i128::from(*after)-i128::from(*before))==Some(native.get(*key).copied().unwrap_or(0)))
            {
                for (account,count) in &closes {
                    if *count!=1 || initializations.get(account)!=Some(&1) || observed_balances.contains(account) { continue; }
                    let Some(index)=keys.iter().position(|key|*key==account) else { continue; };
                    if meta.pre_balances.get(index)!=Some(&0) || meta.post_balances.get(index)!=Some(&0) { continue; }
                    let Some((close_position,close))=instructions.iter().enumerate().find(|(n,c)| c.program_id().0.as_slice()==token::PROGRAM_ID
                        && c.data().as_slice()==[9] && c.accounts().len()==3 && c.accounts()[0].0==account
                        && successful_invocations.contains(&(*n as u32))) else { continue; };
                    let Some((init_position,_))=instructions.iter().enumerate().find(|(n,c)| c.program_id().0.as_slice()==token::PROGRAM_ID
                        && c.data().len()==33 && c.data()[0]==18 && c.accounts().len()==2 && c.accounts()[0].0==account
                        && c.data()[1..]==*close.accounts()[2].0 && mints.mint_for(account).as_ref()==Some(c.accounts()[1].0)
                        && successful_invocations.contains(&(*n as u32))) else { continue; };
                    let creates:Vec<_>=instructions.iter().enumerate().filter(|(_,c)|c.program_id().0.iter().all(|v|*v==0)
                        && c.accounts().get(1).is_some_and(|a|a.0==account)).collect();
                    if creates.len()!=1 || creates[0].0>=init_position
                        || creates[0].1.data().len()!=52 || creates[0].1.data()[..4]!=[0;4]
                        || creates[0].1.data()[12..20]!=165u64.to_le_bytes()
                        || creates[0].1.data()[20..52]!=token::PROGRAM_ID
                        || !successful_invocations.contains(&(creates[0].0 as u32))
                        || init_position>=close_position || instructions[close_position+1..].iter().any(|c|c.accounts().iter().any(|a|a.0==account)) { continue; }
                    fresh_anchor_closes.insert(account.clone());
                }
            }
        }
    }
    let mut anchor_receipts = HashSet::new();
    for (index, ix) in instructions.iter().enumerate() {
        let Some(layout)=crate::native_venues::layout(ix).filter(|l|[pb::Protocol::Ghosty,pb::Protocol::Memefun,pb::Protocol::Omnipair,pb::Protocol::DrFraudsworth].contains(&l.protocol))else{continue;};
        let depth=if ix.is_root(){1}else{ix.stack_height()};
        let end=instructions.iter().enumerate().skip(index+1)
            .find(|(_,child)|child.is_root() || child.stack_height()>0&&child.stack_height()<=depth)
            .map_or(instructions.len(),|(position,_)|position);
        let settled=match layout.protocol {
            pb::Protocol::Omnipair=>crate::omnipair::settlement(tx,index as u32,ix,&instructions[index+1..end],mints,&successful_invocations),
            pb::Protocol::DrFraudsworth=>crate::fraudsworth::settlement(tx,index as u32,ix,&instructions[index+1..end],mints,&successful_invocations),
            _=>crate::anchor_markets::settlement(tx,index as u32,ix,&instructions[index+1..end],mints,&successful_invocations),
        };
        if !settled{continue;}
        let accounts=ix.accounts();
        if layout.users.iter().chain(&layout.vaults).any(|position|{
            let address=accounts[*position].0;
            let Some(mint)=mints.mint_for(address)else{return true;};
                let synchronized = mint.as_slice() == crate::SOL_MINT
                    && instructions.iter().enumerate().any(|(position, child)| {
                        child.program_id().0 == &token::PROGRAM_ID && child.data().as_slice() == [17]
                            && child.accounts().len() == 1 && child.accounts()[0].0 == address
                            && successful_invocations.contains(&(position as u32))
                    })
                    && instructions.iter().enumerate().filter(|(_, child)| {
                        (child.program_id().0 == &token::PROGRAM_ID || child.program_id().0 == &token_2022::PROGRAM_ID)
                            && child.accounts().iter().any(|a| a.0 == address)
                    }).all(|(position, child)| {
                        if !successful_invocations.contains(&(position as u32)) { return false; }
                        let data = child.data();
                        let a = child.accounts();
                        transfers.iter().any(|t| t.index == position as u32)
                            || child.program_id().0 == &token::PROGRAM_ID && data.as_slice() == [17] && a.len() == 1
                            || data.as_slice() == [9] && a.len() >= 3 && a[0].0 == address
                            || (data.as_slice() == [1] || matches!(data.first(), Some(16 | 18)) && data.len() == 33)
                                && a.len() >= 2 && a[0].0 == address && a[1].0.as_slice() == crate::SOL_MINT
                    });
            if other_token_operations.contains(address)&&!synchronized
                || !observed_start.contains(address)&&initializations.get(address)!=Some(&1)
                || !observed_end.contains(address)&&!proven_closed.contains(address)&&!fresh_anchor_closes.contains(address){return true;}
            let incoming:Vec<_>=transfers.iter().filter(|t|t.destination==*address&&t.mint==mint).collect();
            if incoming.iter().any(|t|!t.net_verified
                || instructions[t.index as usize].program_id().0.iter().all(|byte|*byte==0)
                    && !instructions.iter().enumerate().any(|(position,child)|position>t.index as usize
                        && child.program_id().0==&token::PROGRAM_ID && child.data().as_slice()==[17]
                        && child.accounts().len()==1 && child.accounts()[0].0==address
                        && successful_invocations.contains(&(position as u32)))){return true;}
            let received:i128=incoming.iter().map(|t|i128::from(*t.amounts.iter().min().unwrap())).sum();
            let spent=debits.get(&(address.clone(),mint.clone())).copied().unwrap_or(0);
            if deltas.get(address).copied()==Some(received-spent){return false;}
            if mint.as_slice()==crate::SOL_MINT && fresh_anchor_closes.contains(address) && received>=spent {return false;}
            // Classic CloseAccount unwraps remaining WSOL, including a positive
            // pre-existing token balance. It is a lifecycle debit, never swap
            // proceeds. Bind its owner and prohibit later account accesses.
            if mint.as_slice()!=crate::SOL_MINT || !proven_closed.contains(address){return true;}
            let Some(meta)=tx.meta.as_option()else{return true;};
            let resolved=tx.resolved_accounts();
            let Some(position)=resolved.iter().position(|key|*key==address)else{return true;};
            let Some(before)=meta.pre_token_balances.iter().find(|v|v.account_index as usize==position)else{return true;};
            let Some(start)=before.ui_token_amount.as_option().and_then(|v|v.amount.parse::<i128>().ok())else{return true;};
            let Some((closed,close))=instructions.iter().enumerate().find(|(n,c)|c.program_id().0==&token::PROGRAM_ID
                && c.data().as_slice()==[9] && c.accounts().len()==3 && c.accounts()[0].0==address
                && successful_invocations.contains(&(*n as u32)))else{return true;};
            start+received-spent<0 || before.program_id!=base58::encode(token::PROGRAM_ID)
                || before.owner!=base58::encode(close.accounts()[2].0)
                    && !instructions.iter().enumerate().any(|(n,c)|n<closed && c.program_id().0==&token::PROGRAM_ID
                        && c.data().len()==35 && c.data()[..3]==[6,2,1] && c.data()[3..]==*close.accounts()[2].0
                        && c.accounts().len()==2 && c.accounts()[0].0==address
                        && base58::encode(c.accounts()[1].0)==before.owner
                        && successful_invocations.contains(&(n as u32)))
                || meta.post_balances.get(position)!=Some(&0)
                || meta.pre_balances.get(position).is_none_or(|lamports|i128::from(*lamports)<start)
                || transfers.iter().any(|t|t.index as usize>closed&&(t.source==*address||t.destination==*address))
                || instructions.iter().enumerate().any(|(n,c)|n>closed&&c.accounts().iter().any(|a|a.0==address))
        }){continue;}
        anchor_receipts.insert(index as u32);
    }
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
        if !crate::meteora_daam::observed_settlement(index as u32, ix, &instructions[index + 1..end], &successful_invocations,
            input.amounts[0], input.net_verified.then(|| *input.amounts.iter().min().unwrap()),
            output.amounts[0], *output.amounts.iter().min().unwrap())
        { continue; }
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

    let mut jupiter_lend_receipts = HashMap::new();
    for (index, ix) in instructions.iter().enumerate() {
        let Some(swap) = crate::jupiter_lend::layout(ix) else { continue };
        if !successful_invocations.contains(&(index as u32)) { continue; }
        let a = ix.accounts();
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        let end = instructions.iter().enumerate().skip(index + 1)
            .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
            .map_or(instructions.len(), |(end, _)| end);
        let children = &instructions[index + 1..end];
        let oracle_prefix = usize::from(a.len() > 25);
        if oracle_prefix == 1 {
            let Some(oracle) = children.first() else { continue };
            let roles = oracle.accounts();
            if oracle.program_id().0.as_slice() != crate::jupiter_lend::ORACLE_PROGRAM
                || oracle.stack_height() != depth + 1
                || !successful_invocations.contains(&((index + 1) as u32))
                || oracle.data().len() != 10
                || !oracle.data().starts_with(&crate::jupiter_lend::GET_CENTER_PRICE)
                || roles.len() != a.len() - 24
                || roles.iter().zip(&a[24..]).any(|(actual, declared)| actual.0 != declared.0)
            { continue; }
        }
        let calls = &children[oracle_prefix..];
        // Proven direct settlement: pre-operate, user debit, incoming operate,
        // outgoing operate, and its one nested vault debit. Supply-changing,
        // claim, ATA-creation and additional nested variants stay diagnostic.
        if calls.len() != 5 || calls.iter().enumerate().any(|(offset, child)| {
            child.stack_height() != depth + if offset == 4 { 2 } else { 1 }
                || !successful_invocations.contains(&((index + 1 + oracle_prefix + offset) as u32))
        }) { continue; }
        let scoped: Vec<_> = transfers.iter().enumerate().filter(|(_, t)| t.index > index as u32 && t.index < end as u32).collect();
        if scoped.len() != 2 { continue; }
        let (input_position, input) = scoped[0];
        let (output_position, output) = scoped[1];
        let (incoming, outgoing) = (swap.input_token, swap.output_token);
        let net_input = *input.amounts.iter().min().unwrap();
        let net_output = *output.amounts.iter().min().unwrap();
        if input.index != (index + oracle_prefix + 2) as u32 || output.index != (index + oracle_prefix + 5) as u32
            || input.source != *a[2 + incoming].0 || input.destination != *a[13 + incoming].0 || input.authority != *a[0].0
            || output.source != *a[13 + outgoing].0 || output.destination != *a[swap.destination].0 || output.authority != *a[19].0
            || input.mint != *a[7 + incoming].0 || output.mint != *a[7 + outgoing].0
            || !input.net_verified || !output.net_verified || net_input == 0 || net_output == 0
            || [(input.source.as_slice(), input.mint.as_slice()), (input.destination.as_slice(), input.mint.as_slice()),
                (output.source.as_slice(), output.mint.as_slice()), (output.destination.as_slice(), output.mint.as_slice())]
                .iter().any(|(account, mint)| mints.mint_for(account).as_deref() != Some(*mint))
            || calls[1].program_id().0 != a[21 + incoming].0 || calls[4].program_id().0 != a[21 + outgoing].0
            || [1, 4].iter().any(|offset| calls[*offset].data().len() != 10 || calls[*offset].data()[0] != 12 || calls[*offset].accounts().len() != 4)
            || swap.exact_input && (input.amounts[0] != swap.amount || net_output < swap.threshold)
            || !swap.exact_input && (net_output != swap.amount || input.amounts[0] > swap.threshold)
        { continue; }
        let pre = &calls[0];
        let p = pre.accounts();
        if pre.program_id().0.as_slice() != crate::jupiter_lend::LIQUIDITY_PROGRAM
            || pre.data().len() != 40 || !pre.data().starts_with(&crate::jupiter_lend::PRE_OPERATE)
            || pre.data()[8..40] != *a[7 + incoming].0 || p.len() != 7
        { continue; }
        let mut valid = true;
        for (slot, role) in [1, 19, 15 + incoming, 17 + incoming, 13 + incoming, 9 + incoming, 21 + incoming].iter().enumerate() {
            let expected = if [2, 3].contains(&slot) && a[*role].0.as_slice() == crate::jupiter_lend::PROGRAM_ID {
                crate::jupiter_lend::LIQUIDITY_PROGRAM.as_slice()
            } else { a[*role].0.as_slice() };
            if p[slot].0.as_slice() != expected { valid = false; }
        }
        for (offset, side) in [(2, incoming), (3, outgoing)] {
            let operate = &calls[offset];
            let o = operate.accounts();
            let data = operate.data();
            if operate.program_id().0.as_slice() != crate::jupiter_lend::LIQUIDITY_PROGRAM
                || data.len() != 105 || !data.starts_with(&crate::jupiter_lend::OPERATE) || o.len() != 13
                || data[40..72] != *a[swap.recipient].0 || data[72..104] != *a[swap.recipient].0
                || data[104] != 1
            { valid = false; break; }
            // Observed incoming repayments are slightly below the vault's
            // receipt. Price the full physical user debit, including that
            // difference; never substitute the internal borrow amount.
            let supply = i128::from_le_bytes(data[8..24].try_into().unwrap());
            let borrow = i128::from_le_bytes(data[24..40].try_into().unwrap());
            // Proven pure supply or pure borrow settlement. Incoming deposits
            // and repayments can be below the physical debit due to revenue;
            // outgoing withdrawals/borrowing must match the exact vault debit.
            // Mixed supply/borrow and zero operations remain diagnostic.
            let incoming_valid = supply == 0 && borrow < 0 && borrow >= -i128::from(net_input)
                || borrow == 0 && supply > 0 && supply <= i128::from(net_input);
            let outgoing_valid = supply == 0 && borrow == i128::from(output.amounts[0])
                || borrow == 0 && supply == -i128::from(output.amounts[0]);
            if offset == 2 && !incoming_valid || offset == 3 && !outgoing_valid
            { valid = false; break; }
            for (slot, role) in [1, 19, 9 + side, 7 + side, 13 + side, 15 + side, 17 + side, 11 + side].iter().enumerate() {
                let expected = if [5, 6].contains(&slot) && a[*role].0.as_slice() == crate::jupiter_lend::PROGRAM_ID {
                    crate::jupiter_lend::LIQUIDITY_PROGRAM.as_slice()
                } else { a[*role].0.as_slice() };
                if o[slot].0.as_slice() != expected { valid = false; }
            }
            let destination = if offset == 2 { crate::jupiter_lend::LIQUIDITY_PROGRAM.as_slice() } else { a[swap.destination].0.as_slice() };
            if o[8].0.as_slice() != destination || o[9].0.as_slice() != destination
                || o[10].0.as_slice() != crate::jupiter_lend::LIQUIDITY_PROGRAM || o[11].0.as_slice() != crate::jupiter_lend::LIQUIDITY_PROGRAM
                || o[12].0 != a[21 + side].0
            { valid = false; }
        }
        if valid { jupiter_lend_receipts.insert(index as u32, (input_position, output_position)); }
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
    let mut native_pump_multihop_receipts = HashSet::new();
    if expand_native {
        for (&index, &(input_position, output_position)) in &jupiter_lend_receipts {
            let ix = &instructions[index as usize];
            let a = ix.accounts();
            let (input, output) = (&transfers[input_position], &transfers[output_position]);
            swaps.push(pb::Swap {
                protocol: pb::Protocol::JupiterLend.into(),
                program_id: crate::jupiter_lend::PROGRAM_ID.to_vec(), amm: crate::jupiter_lend::PROGRAM_ID.to_vec(),
                amm_pool: a[1].0.clone(), user: a[0].0.clone(),
                stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
                input_mint: input.mint.clone(), input_amount: input.amounts[0],
                output_mint: output.mint.clone(), output_amount: *output.amounts.iter().min().unwrap(),
                source_index: Some(index), source_transfer_index: Some(input.index),
                ..Default::default()
            });
        }
        for (&index, &(input_position, output_position)) in &oasis_receipts {
            let ix = &instructions[index as usize];
            let a = ix.accounts();
            let (input, output) = (&transfers[input_position], &transfers[output_position]);
            swaps.push(pb::Swap {
                protocol: pb::Protocol::Oasis.into(),
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
                protocol: pb::Protocol::Mercurial.into(),
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
                    protocol: pb::Protocol::Defituna.into(),
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
                protocol: pb::Protocol::MeteoraDaam.into(),
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
            let layout = if [crate::proprietary_swaps::QUANTUM.as_slice(), crate::proprietary_swaps::BINARYFI.as_slice()].contains(&ix.program_id().0.as_slice()) {
                let Some(bound) = crate::proprietary_swaps::bound_layout(ix, mints) else { continue };
                bound
            } else if ix.program_id().0.as_slice() == crate::archer::PROGRAM_ID {
                let Some(event) = archer_fills.get(&(index as u32)) else { continue };
                let Some(bound) = crate::archer::bound_layout(ix,event) else { continue };
                bound
            } else { layout };
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
            if [crate::spl_swap_forks::DOOAR.as_slice(), crate::spl_swap_forks::DEXLAB.as_slice(),crate::spl_swap_forks::PENGUIN.as_slice()].contains(&ix.program_id().0.as_slice())
                && !crate::spl_swap_forks::settlement(index as u32, tx, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
            { continue; }
            if [crate::proprietary_swaps::QUANTUM.as_slice(), crate::proprietary_swaps::BINARYFI.as_slice()].contains(&ix.program_id().0.as_slice())
                && !crate::proprietary_swaps::settlement(index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
            { continue; }
            if ix.program_id().0.as_slice() == crate::archer::PROGRAM_ID
                && !archer_fills.get(&(index as u32)).is_some_and(|event| crate::archer::settlement(index as u32, ix, &instructions[index + 1..end as usize], event, mints, &successful_invocations))
            { continue; }
            if ix.program_id().0.as_slice() == crate::triangle::PROGRAM_ID
                && !crate::triangle::settlement(index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
            { continue; }
            if ix.program_id().0.as_slice() == crate::riptide::PROGRAM_ID
                && !crate::riptide::settlement(tx, index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
            { continue; }
            if ix.program_id().0 == &substreams_solana_idls::meteora::dlmm::PROGRAM_ID
                && ix.data().starts_with(&substreams_solana_idls::meteora::dlmm::instructions::SWAP2)
                    && ix.data().get(24..28).is_some_and(|count| count == [0, 0, 0, 0])
                && matches!(ix.data().len(), 32 | 34 | 56)
                && !crate::meteora_dlmm::padded_swap2_settlement(index as u32, ix, &instructions[index + 1..end as usize], &successful_invocations)
            { continue; }
            if [pb::Protocol::Ghosty,pb::Protocol::Memefun,pb::Protocol::Omnipair,pb::Protocol::DrFraudsworth].contains(&layout.protocol)
                && !anchor_receipts.contains(&(index as u32))
            {continue;}
            if ix.program_id().0.as_slice() == crate::quay::PROGRAM_ID
                && !crate::quay::settlement(tx, index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
            { continue; }
            if ix.program_id().0.as_slice() == crate::hadron::PROGRAM_ID
                && crate::hadron::settlement(index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations).is_none()
            { continue; }
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
                            .is_some_and(|program| program.0.as_slice() == substreams_solana_macro::b58!("vo1tWgqZMjG61Z2T9qUaMYKqZ75CYzMuaZ2LZP1n7HV"))
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
                if layout.protocol == pb::Protocol::M0NativeOrder
                    && (!input.net_verified || !output.net_verified
                        || crate::m0_native_order::settlement(index as u32,tx,ix,
                            &instructions[index+1..end as usize],mints,&successful_invocations)
                            != Some((input.amounts[0],*output.amounts.iter().min().unwrap()))) { continue; }
                if layout.protocol == pb::Protocol::PiconDlmm
                    && (!input.net_verified || !output.net_verified
                        || crate::picon_dlmm::settlement(index as u32,tx,ix,
                            &instructions[index+1..end as usize],mints,&successful_invocations)
                            != Some((input.amounts[0],*output.amounts.iter().min().unwrap()))) { continue; }
                if layout.protocol==pb::Protocol::Pancakeswap && ix.data().starts_with(&crate::pancakeswap::LEGACY_SWAP)
                    && (!input.net_verified || !output.net_verified
                        || crate::pancakeswap::legacy_settlement(index as u32,tx,ix,
                            &instructions[index+1..end as usize],mints,&successful_invocations)
                            !=Some((input.amounts[0],*output.amounts.iter().min().unwrap()))) { continue; }
                let patch_receipt = if [pb::Protocol::PatchBuyback,pb::Protocol::Bxlw].contains(&layout.protocol) {
                    let Some(receipt) = crate::patch_bxlw::settlement(index as u32,tx,ix,
                        &instructions[index+1..end as usize],mints,&successful_invocations) else { continue };
                    if !input.net_verified || !output.net_verified
                        || receipt != (input.amounts[0],*output.amounts.iter().min().unwrap()) { continue; }
                    Some(receipt)
                } else { None };
                let legacy_input = if [pb::Protocol::AldrinV1,pb::Protocol::Cropper].contains(&layout.protocol) {
                    let Some((gross,net)) = crate::legacy_amm::settlement(index as u32,tx,ix,
                        &instructions[index+1..end as usize],mints,&successful_invocations) else { continue };
                    if !input.net_verified || !output.net_verified || output.amounts[0] != net
                        || input.index != index as u32 + 1 || input.amounts[0] > gross { continue; }
                    Some(gross)
                } else { None };

                let receipt=match layout.protocol {
                    pb::Protocol::Gator=>Some(crate::gator::settlement(tx,index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations)),
                    pb::Protocol::AnchorSwap=>Some(crate::anchor_swap::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations)),
                    pb::Protocol::RaydiumCpmmFork=>Some(crate::raydium_cpmm_fork::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations)),
                    pb::Protocol::Exponent=>Some(crate::exponent::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations)),
                    pb::Protocol::Janus=>Some(crate::janus::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations)),
                    pb::Protocol::Peg=>Some(crate::peg::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations)),
                    pb::Protocol::SanctumInfinity=>Some(crate::sanctum_infinity::settlement(tx,index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations)),
                    _=>None,
                };
                if receipt.is_some_and(|v|v!=Some((if layout.protocol==pb::Protocol::AnchorSwap{layout.amount}else{input.amounts[0]},*output.amounts.iter().min().unwrap()))){continue;}
                if [pb::Protocol::Futarchy,pb::Protocol::Guacswap].contains(&layout.protocol) {
                    let receipt=if layout.protocol==pb::Protocol::Futarchy{crate::futarchy::settlement(index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations)}
                        else{crate::guacswap::settlement(index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations)};
                    if receipt!=Some((input.amounts[0],*output.amounts.iter().min().unwrap())){continue;}
                }
                if layout.protocol==pb::Protocol::JupiterPerps
                    && crate::jupiter_perps::settlement(index as u32,ix,&instructions[index+1..end as usize],&successful_invocations)
                        !=Some((input.amounts[0],*output.amounts.iter().min().unwrap())) {continue;}
                if layout.protocol==pb::Protocol::Numeraire
                    && !crate::numeraire::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations,*output.amounts.iter().min().unwrap()) {continue;}

                if crate::suffixed_anchor_swaps::applies(ix)
                    && crate::suffixed_anchor_swaps::settlement(index as u32, tx, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
                        != Some((input.amounts[0], output.amounts[0])) { continue; }
                if crate::current_proprietary::applies(ix)
                    && crate::current_proprietary::settlement(index as u32, tx, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
                        != Some((input.amounts[0], output.amounts[0])) { continue; }
                if layout.protocol == pb::Protocol::Saber
                    || layout.protocol == pb::Protocol::SolfiV2 && matches!(ix.data().len(), 25 | 153)
                {
                    let calls = &instructions[index + 1..end as usize];
                    let principal = if layout.protocol == pb::Protocol::Saber {
                        crate::saber::settlement(ix, calls)
                    } else {
                        crate::native_venues::solfi_extended_settlement(ix, calls)
                    };
                    let reverse = usize::from(input.source == *accounts[7].0);
                    if principal != Some((input.amounts[0], output.amounts[0]))
                        || !successful_invocations.contains(&(index as u32))
                        || calls.iter().enumerate().any(|(offset, _)| !successful_invocations.contains(&((index + offset + 1) as u32)))
                        || layout.protocol == pb::Protocol::SolfiV2
                            && (input.mint != *accounts[8 + reverse].0
                                || output.mint != *accounts[9 - reverse].0)
                    { continue; }
                }
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
                let input_amount = if let Some(gross) = legacy_input { gross }
                else if [pb::Protocol::Hadron,pb::Protocol::AnchorSwap].contains(&layout.protocol) {
                    // Typed direct fee plus the pool principal equals the full
                    // user's requested debit; the helper excludes quote fields.
                    layout.amount
                } else if layout.protocol == pb::Protocol::Invariant {
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
                    || ((!layout.exact_input || layout.input_limit) && input_amount > layout.amount
                        && !(layout.protocol == pb::Protocol::PatchBuyback && patch_receipt.is_some()))
                    || *output.amounts.iter().min().unwrap() < layout.minimum_output
                    || (output.fee_token && output.mint != crate::SOL_MINT && !output.net_verified)
                {
                    continue;
                }
                candidates.push(pb::Swap {
                    protocol: (layout.protocol as i32).into(),
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
            if input.mint.as_slice() == mint.as_slice() {
                continue;
            }
            // A declared host payout may alias user_token_out when both net receipts are proven.
            // Require the complete committed current event to distinguish it
            // from the primary receipt; never sum fees or pick by amount size.
            let (output, host_fee_index) = if outputs.len() == 1 {
                (outputs[0], None)
            } else if outputs.len() == 2
                && layout.host_fee_account.as_deref() == Some(layout.user_token_out.as_slice())
                && outputs.iter().all(|t| t.net_verified)
            {
                let events: Vec<_> = instructions.iter().enumerate()
                    .filter(|(position, child)| *position > outputs[1].index as usize && *position < end as usize
                        && child.stack_height() == depth + 1 && successful_invocations.contains(&(*position as u32)))
                    .filter_map(|(_, child)| crate::meteora_dlmm::output_host_fee_event(child, &layout, input.amounts[0], mint))
                    .collect();
                if events.len() != 1 || (outputs[1].amounts[0], outputs[0].amounts[0]) != events[0] {
                    continue;
                }
                (outputs[1], Some(outputs[0].index))
            } else {
                continue;
            };
            // Additional user flows (including refunds) need their own complete
            // net-debit proof; do not silently ignore them when normalizing.
            if scoped.iter().any(|t| {
                t.index != input.index
                    && t.index != output.index
                    && Some(t.index) != host_fee_index
                    && [t.source.as_slice(), t.destination.as_slice()]
                        .iter()
                        .any(|account| *account == layout.user_token_in.as_slice() || *account == layout.user_token_out.as_slice())
            }) {
                continue;
            }
            let native = pb::Swap {
                protocol: pb::Protocol::MeteoraDlmm.into(),
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
                    let Some(meta) = tx.meta.as_option() else { continue };
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
                        protocol: pb::Protocol::Pumpfun.into(),
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
                native_pump_multihop_receipts.insert((index as u32, input.index.min(output.index),
                    group[2].0.clone(), accounts[user].0.clone(), input_mint.clone(), input.amounts[0],
                    output_mint.clone(), *output.amounts.iter().min().unwrap()));
                swaps.push(pb::Swap {
                    protocol: (if is_curve { pb::Protocol::Pumpfun } else { pb::Protocol::PumpfunAmm } as i32).into(),
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
    let mut rfq_receipts = HashMap::new();
        // RFQ fills identify a taker and maker, not reserve-vault liquidity.
        // Verify their exact user debit/receipt, including native SOL paths.
        for (index, ix) in instructions.iter().enumerate() {
            if caught_failure && !successful_invocations.contains(&(index as u32)) {
                continue;
            }
            let Some(fill) = crate::rfq::layout(ix) else { continue };
            let strict_rfq = ix.program_id().0.as_slice() != crate::rfq::PROGRAM_ID;
            if strict_rfq && !successful_invocations.contains(&(index as u32)) { continue; }
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
            let input_mint = accounts[fill.input_mint].0;
            let output_mint = accounts[fill.output_mint].0;
            let source = accounts[fill.input_source].0;
            let destination = accounts[fill.output_destination].0;
            let inputs: Vec<_> = scoped.iter().filter(|t| &t.source == source && &t.mint == input_mint).collect();
            let outputs: Vec<_> = scoped.iter().filter(|t| &t.destination == destination && &t.mint == output_mint).collect();
            if input_mint == output_mint || inputs.len() != 1 || outputs.len() != 1 {
                continue;
            }
            let (input, output) = (inputs[0], outputs[0]);
            if strict_rfq && (!successful_invocations.contains(&input.index) || !successful_invocations.contains(&output.index)
                || !input.net_verified || !output.net_verified)
                || input.amounts[0] != fill.input_amount
                || output.amounts[0] != fill.output_amount
                || instructions[input.index as usize].program_id().0 != accounts[if fill.input_source == 0 { 10 } else { fill.input_token_program }].0
                || instructions[output.index as usize].program_id().0 != accounts[if fill.output_source == 1 { 10 } else { fill.output_token_program }].0
                || input.destination.as_slice() != accounts[fill.input_destination].0.as_slice()
                || output.source.as_slice() != accounts[fill.output_source].0.as_slice()
                || (!instructions[input.index as usize].program_id().0.iter().all(|byte| *byte == 0) && input.authority.as_slice() != accounts[0].0.as_slice())
                || (!instructions[output.index as usize].program_id().0.iter().all(|byte| *byte == 0)
                    && output.authority.as_slice() != accounts[1].0.as_slice())
                || (output.fee_token && output.mint != crate::SOL_MINT && !output.net_verified)
                || fill.input_accounts.iter().any(|position| {
                    accounts[*position].0 != ix.program_id().0 && mints.mint_for(accounts[*position].0).as_ref() != Some(input_mint)
                })
                || fill.output_accounts.iter().any(|position| {
                    accounts[*position].0 != ix.program_id().0 && mints.mint_for(accounts[*position].0).as_ref() != Some(output_mint)
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
            if strict_rfq && fill.input_accounts.iter().chain(&fill.output_accounts).any(|position| {
                let address = accounts[*position].0;
                if address == ix.program_id().0 { return false; }
                let Some(mint) = mints.mint_for(address) else { return true };
                // A committed classic SyncNative changes WSOL's spendable
                // balance by explicit System credits already in the ledger.
                // Admit it only when every token access has a typed, committed
                // transfer/lifecycle receipt; unknown mint/extension operations
                // remain blocked, and the complete balance equation still holds.
                let synchronized = mint.as_slice() == crate::SOL_MINT
                    && instructions.iter().enumerate().any(|(position, child)| {
                        child.program_id().0 == &token::PROGRAM_ID && child.data().as_slice() == [17]
                            && child.accounts().len() == 1 && child.accounts()[0].0 == address
                            && successful_invocations.contains(&(position as u32))
                    })
                    && instructions.iter().enumerate().filter(|(_, child)| {
                        (child.program_id().0 == &token::PROGRAM_ID || child.program_id().0 == &token_2022::PROGRAM_ID)
                            && child.accounts().iter().any(|a| a.0 == address)
                    }).all(|(position, child)| {
                        if !successful_invocations.contains(&(position as u32)) { return false; }
                        let data = child.data();
                        let a = child.accounts();
                        transfers.iter().any(|t| t.index == position as u32)
                            || child.program_id().0 == &token::PROGRAM_ID && data.as_slice() == [17] && a.len() == 1
                            || data.as_slice() == [9] && a.len() >= 3 && a[0].0 == address
                            || (data.as_slice() == [1] || matches!(data.first(), Some(16 | 18)) && data.len() == 33)
                                && a.len() >= 2 && a[0].0 == address && a[1].0.as_slice() == crate::SOL_MINT
                    });
                if other_token_operations.contains(address) && !synchronized
                    || !observed_start.contains(address) && initializations.get(address) != Some(&1)
                    || !observed_end.contains(address) && !proven_closed.contains(address) { return true; }
                let incoming: Vec<_> = transfers.iter().filter(|t| t.destination == *address && t.mint == mint).collect();
                if incoming.iter().any(|t| !t.net_verified
                    || instructions[t.index as usize].program_id().0.iter().all(|byte| *byte == 0)
                        && !instructions.iter().enumerate().any(|(position, child)| {
                            position > t.index as usize && child.program_id().0 == &token::PROGRAM_ID
                                && child.data().as_slice() == [17] && child.accounts().len() == 1
                                && child.accounts()[0].0 == address && successful_invocations.contains(&(position as u32))
                        })) { return true; }
                let received: i128 = incoming.iter().map(|t| i128::from(*t.amounts.iter().min().unwrap())).sum();
                deltas.get(address).copied() != Some(received - debits.get(&(address.clone(), mint)).copied().unwrap_or(0))
            }) { continue; }
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
            let native = pb::Swap {
                protocol: pb::Protocol::JupiterRfq.into(),
                program_id: ix.program_id().0.clone(),
                amm: ix.program_id().0.clone(),
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
            };
            rfq_receipts.insert(index as u32, (native.clone(), input.index, output.index));
            if expand_native { swaps.push(native); }
        }

    // An older native curve can be extended in its own committed invocation
    // immediately before a V3 sell. Bind the rent funding to the full exact
    // ExtendAccount account/event ABI; an arbitrary earlier transfer is not
    // an independently proven extension.
    let mut proven_pump_extensions = HashMap::<usize, (u32, Vec<u8>, Vec<u8>)>::new();
    for (position, call) in instructions.iter().enumerate() {
        let a = call.accounts();
        if call.program_id().0 != &pumpfun::PROGRAM_ID || call.data().as_slice() != [234,102,194,203,150,72,62,229]
            || a.len() != 5 || a[2].0.iter().any(|n| *n != 0) || a[4].0.as_slice() != pumpfun::PROGRAM_ID
            || !successful_invocations.contains(&(position as u32)) { continue; }
        let depth = if call.is_root() { 1 } else { call.stack_height() };
        let children: Vec<_> = instructions.iter().enumerate().skip(position + 1).take_while(|(_, c)| !c.is_root()
            && (c.stack_height() == 0 || c.stack_height() > depth)).collect();
        if children.len() != 2 || children.iter().any(|(i,c)| c.stack_height() != depth + 1
            || !successful_invocations.contains(&(*i as u32))) { continue; }
        let (funding_index, funding) = children[0]; let event = children[1].1;
        let f = funding.accounts(); let e = event.accounts();
        if funding.program_id().0.iter().any(|n| *n != 0) || funding.data().len() != 12
            || funding.data()[..4] != [2,0,0,0] || f.len() != 2 || f[0].0 != a[1].0 || f[1].0 != a[0].0
            || event.program_id().0 != &pumpfun::PROGRAM_ID || e.len() != 1 || e[0].0 != a[3].0
            || event.data().len() != 104 || event.data()[..16] != [228,69,165,46,81,203,154,29,97,97,215,144,93,146,22,124]
            || event.data()[16..48] != a[0].0[..] || event.data()[48..80] != a[1].0[..]
            || u64::from_le_bytes(event.data()[80..88].try_into().unwrap()) >= u64::from_le_bytes(event.data()[88..96].try_into().unwrap())
        { continue; }
        proven_pump_extensions.insert(position, (funding_index as u32, a[0].0.clone(), a[1].0.clone()));
    }

    // Legacy/V2 native sells pay the user and all fees directly from owned
    // curve lamports. Build every independently token-proven contribution
    // before checking shared curve/fee credits and each isolated user's net.
    let mut legacy_native_sells = Vec::new();
    let mut pump_fee_returns = None;
    for (index, ix) in instructions.iter().enumerate() {
        if ix.program_id().0 != &pumpfun::PROGRAM_ID || !successful_invocations.contains(&(index as u32)) { continue; }
        let a = ix.accounts();
        let (roles, amount, minimum) = match crate::pumpfun::unpack_trade_instruction(ix.data()) {
            Ok(pumpfun::instructions::PumpFunInstruction::Sell(args)) if a.len() == 16 && a[7].0.iter().all(|b| *b == 0)
                && a[11].0.as_slice() == pumpfun::PROGRAM_ID => ([3, 2, 6, 5, 4, 9, 10, 1, 15, 8], args.amount, args.min_sol_output),
            Ok(pumpfun::instructions::PumpFunInstruction::SellV2(args)) if matches!(a.len(), 26 | 27) && a[2].0.as_slice() == crate::SOL_MINT
                && (a[4].0.as_slice() == token::PROGRAM_ID || a[4].0.as_slice() == token_2022::PROGRAM_ID)
                && a[5].0 == &base58::decode("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL").unwrap() && a[23].0.iter().all(|b| *b == 0)
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
        if roles[0] == 10 && a[4].0.as_slice() == token_2022::PROGRAM_ID {
            // Native SOL uses no quote-token CPI. This alternative interface
            // program is admitted only in the exact fee/base/event subtree.
            let children = &instructions[index + 1..end];
            if children.len() != 3 || children.iter().any(|child| child.stack_height() != depth + 1)
                || children[0].program_id().0.as_slice() != crate::pump_curve::FEE_PROGRAM
                || children[0].data().len() != 57 || children[0].data()[..9] != [154,237,138,92,162,2,162,187,1]
                || children[0].data()[25..].iter().any(|v| *v != 0)
                || children[1].program_id().0 != a[token_program].0 || children[1].data().len() != 10 || children[1].data()[0] != 12
                || children[2].program_id().0.as_slice() != pumpfun::PROGRAM_ID { continue; }
        }
        if a.len() == 27 {
            // The finalized V2 suffix is inert: it aliases no canonical role,
            // has no token identity or child accesses, and no native movement.
            let address = a[26].0;
            let balance = tx.resolved_accounts().iter().position(|key| *key == address).and_then(|p|
                tx.meta.as_option()?.pre_balances.get(p).zip(tx.meta.as_option()?.post_balances.get(p)));
            if a[..26].iter().any(|key| key.0 == address) || mints.mint_for(address).is_some()
                || balance.is_none_or(|(before, after)| before != after)
                || instructions[index + 1..end].iter().any(|child| child.accounts().iter().any(|key| key.0 == address)) { continue; }
        }
        let base: Vec<_> = transfers.iter().filter(|t| t.index > index as u32 && (t.index as usize) < end && t.depth == depth + 1
            && t.source == *a[source].0 && t.destination == *a[vault].0 && t.authority == *a[user].0 && t.mint == *a[mint].0
            && instructions[t.index as usize].program_id().0 == a[token_program].0 && t.net_verified && successful_invocations.contains(&t.index)).collect();
        if base.len() != 1 || base[0].amounts[0] != amount || other_token_operations.contains(a[source].0) { continue; }
        let base = base[0];
        let initialized_zero = initializations.get(a[source].0) == Some(&1) && !observed_start.contains(a[source].0)
            && tx.resolved_accounts().iter().position(|key| *key == a[source].0)
                .and_then(|p| tx.meta.as_option()?.pre_balances.get(p)).is_some_and(|balance| *balance == 0);
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
        let Some((cashback_bps, cashback_amount)) = crate::pump_curve::cashback(event_ix.data()) else { continue };
        let cashback = if cashback_amount == 0 && cashback_bps == 0 { None } else {
            if roles[0] != 10 || cashback_amount == 0 || quote.creator_fee != 0 || event_ix.data().get(217..225) != Some([0; 8].as_slice()) { continue; }
            let expected = solana_pubkey::Pubkey::find_program_address(&[b"user_volume_accumulator", a[user].0],
                &solana_pubkey::Pubkey::new_from_array(pumpfun::PROGRAM_ID)).0.to_bytes();
            if a[19].0.as_slice() != expected || protected.iter().any(|role| a[*role].0 == a[19].0) { continue; }
            let fee_calls: Vec<_> = instructions.iter().enumerate().skip(index + 1).take(end - index - 1)
                .filter(|(_, child)| child.program_id().0.as_slice() == crate::pump_curve::FEE_PROGRAM).collect();
            if fee_calls.len() != 1 { continue; }
            let (fee_index, fee_call) = fee_calls[0];
            let f = fee_call.accounts();
            let data = fee_call.data();
            if a[22].0.as_slice() != crate::pump_curve::FEE_PROGRAM || fee_call.stack_height() != depth + 1 || fee_index >= base.index as usize || f.len() != 2
                || f[0].0 != a[21].0 || f[1].0.as_slice() != pumpfun::PROGRAM_ID || data.len() != 57
                || data[..9] != [154, 237, 138, 92, 162, 2, 162, 187, 1] || data[25..].iter().any(|b| *b != 0)
                || !successful_invocations.contains(&(fee_index as u32)) { continue; }
            let Some(fees) = pump_fee_returns.get_or_insert_with(|| crate::pump_curve::fee_returns(tx)).get(&(fee_index as u32)) else { continue };
            if fees.lp != 0 || fees.creator != cashback_bps || fees.protocol.checked_add(fees.creator).is_none_or(|bps| bps > 10_000)
                || (u128::from(quote.amount) * u128::from(cashback_bps) + 9_999) / 10_000 != u128::from(cashback_amount)
                || (u128::from(quote.amount) * u128::from(fees.protocol) + 9_999) / 10_000 != u128::from(quote.fee) { continue; }
            Some((19, cashback_amount))
        };
        let Some(net) = quote.amount.checked_sub(quote.fee).and_then(|n| n.checked_sub(quote.creator_fee)).and_then(|n| n.checked_sub(cashback_amount)) else { continue };
        if event.is_buy || event.mint.as_slice() != a[mint].0.as_slice() || event.user.as_slice() != a[user].0.as_slice()
            || event.token_amount != amount || quote.mint.iter().any(|b| *b != 0) || quote.amount != event.sol_amount
            || net == 0 || net < minimum || quote.fee < quote.buyback_fee
        { continue; }
        // Canonical post-reserves are state witnesses from the same committed
        // native sell, never event output amounts. Exact token input and unique
        // inverse CP rounding independently recover that leg's gross proceeds;
        // runtime-bound fee returns determine its net. Whole-transaction user,
        // curve and fee ledgers below still have to match exactly.
        let independent_proceeds = (|| {
            if base.amounts.iter().any(|n| *n != amount) || other_token_operations.contains(a[vault].0)
                || !observed_start.contains(a[vault].0) || !observed_end.contains(a[vault].0) { return None; }
            let vault_flows: Vec<_> = transfers.iter().filter(|t| t.source == *a[vault].0 || t.destination == *a[vault].0).collect();
            if vault_flows.iter().any(|t| t.mint != *a[mint].0 || !t.net_verified || !successful_invocations.contains(&t.index)) { return None; }
            let mut change = 0i128;
            for flow in vault_flows {
                let delta = if flow.destination == *a[vault].0 { i128::from(*flow.amounts.iter().min()?) } else { -i128::from(flow.amounts[0]) };
                change = change.checked_add(delta)?;
            }
            if deltas.get(a[vault].0).copied() != Some(change) { return None; }
            let fee_config = if roles[0] == 3 { 12 } else { 21 };
            let fee_program = if roles[0] == 3 { 13 } else { 22 };
            if a[fee_program].0.as_slice() != crate::pump_curve::FEE_PROGRAM { return None; }
            let calls: Vec<_> = instructions.iter().enumerate().skip(index + 1).take(end - index - 1)
                .filter(|(_, child)| child.program_id().0.as_slice() == crate::pump_curve::FEE_PROGRAM).collect();
            if calls.len() != 1 { return None; }
            let (fee_index, fee_call) = calls[0];
            let f = fee_call.accounts();
            let data = fee_call.data();
            if fee_call.stack_height() != depth + 1 || fee_index >= base.index as usize || f.len() != 2
                || f[0].0 != a[fee_config].0 || f[1].0.as_slice() != pumpfun::PROGRAM_ID
                || data.len() != 57 || data[..9] != [154, 237, 138, 92, 162, 2, 162, 187, 1]
                || data[25..].iter().any(|b| *b != 0) || !successful_invocations.contains(&(fee_index as u32)) { return None; }
            let fees = *pump_fee_returns.get_or_insert_with(|| crate::pump_curve::fee_returns(tx)).get(&(fee_index as u32))?;
            let event_sol = u64::from_le_bytes(event_ix.data().get(105..113)?.try_into().ok()?);
            let event_tokens = u64::from_le_bytes(event_ix.data().get(113..121)?.try_into().ok()?);
            let (gross, proceeds, protocol, creator, _) = crate::pump_curve::sell(amount, event_tokens, event_sol, fees)?;
            (gross == quote.amount && proceeds == net && protocol == quote.fee && creator == quote.creator_fee.checked_add(cashback_amount)?).then_some(())
        })().is_some();
        legacy_native_sells.push(PumpNativeSell { instruction: index, roles, input: base.index, net, gross: quote.amount,
            protocol_fee: quote.fee - quote.buyback_fee, creator_fee: quote.creator_fee, buyback_fee: quote.buyback_fee, independent_proceeds, cashback });
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
            if ix.program_id().0.iter().all(|b| *b == 0) && a.len() >= 2 && successful_invocations.contains(&(position as u32)) {
                let data = ix.data();
                let funding = if data.len() == 52 && data[..4] == [0; 4] {
                    Some(u64::from_le_bytes(data[4..12].try_into().unwrap()))
                } else if data.len() >= 92 && data[..4] == [3, 0, 0, 0] && matches!(a.len(), 2 | 3) {
                    // System CreateAccountWithSeed encodes base, bincode string,
                    // lamports, space and owner. Its committed funding debit
                    // remains separate from the user's actual swap proceeds.
                    let seed_len = u64::from_le_bytes(data[36..44].try_into().unwrap());
                    if seed_len <= 32 && data.len() == 92 + seed_len as usize
                        && &data[4..36] == a[if a.len() == 3 { 2 } else { 0 }].0.as_slice()
                    {
                        Some(u64::from_le_bytes(data[44 + seed_len as usize..52 + seed_len as usize].try_into().unwrap()))
                    } else { None }
                } else { None };
                if let Some(amount) = funding {
                    *native_changes.entry(a[0].0.clone()).or_default() -= i128::from(amount);
                    *native_changes.entry(a[1].0.clone()).or_default() += i128::from(amount);
                }
            }
        }
        if let Some(meta) = tx.meta.as_option() {
            if let Some(payer) = resolved.first() { *native_changes.entry((*payer).clone()).or_default() -= i128::from(meta.fee); }
            // Source-bound Titanium Pump settlement: exact Atlas gain is swept
            // to its funding account and wrapped, with an early WSOL close and
            // exact paired re-creation/restore. Require independent CP proceeds.
            let titan_settlements: HashMap<_, _> = legacy_native_sells.iter().filter(|sell| sell.independent_proceeds)
                .filter_map(|sell| crate::pumpfun::titan_native_settlement(tx, &instructions, sell.instruction, sell.net, &successful_invocations)
                    .map(|settlement| (sell.instruction, settlement))).collect();
            for (source, settlement) in &titan_settlements {
                let net = legacy_native_sells.iter().find(|sell| sell.instruction == *source).unwrap().net;
                *native_changes.entry(settlement.atlas.clone()).or_default() -= i128::from(net);
                *native_changes.entry(settlement.funder.clone()).or_default() += i128::from(net);
                *native_changes.entry(settlement.quote.clone()).or_default() -= i128::from(settlement.refund);
                *native_changes.entry(settlement.funder.clone()).or_default() += i128::from(settlement.refund);
            }
            let mut proven_volume_refunds = HashSet::new();
            let protected: HashSet<_> = legacy_native_sells.iter().flat_map(|sell| {
                let a = instructions[sell.instruction].accounts();
                [sell.roles[0], sell.roles[2], sell.roles[7], sell.roles[8], sell.roles[9]].into_iter()
                    .chain(sell.cashback.map(|(role, _)| role)).map(move |role| a[role].0.clone())
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
                    || titan_settlements.values().any(|settlement| settlement.close == position)
                    || closes.get(a[0].0) != Some(&1) || protected.contains(a[0].0)
                    || instructions.iter().enumerate().skip(position + 1).any(|(p, later)| later.accounts().iter().any(|key| key.0 == a[0].0)
                        && !(later.is_root() && later.program_id().0.as_slice() == substreams_solana_macro::b58!("D3og1Asnw7dPBcToJ3gQ2mRup728xsEn6Sg62mTm1enm")
                            && later.data().as_slice() == [12, 0, 255] && later.accounts().len() == 3 && later.accounts()[2].0 == a[0].0
                            && later.accounts()[0].0 != a[0].0 && later.accounts()[1].0 != a[0].0
                            && successful_invocations.contains(&(p as u32)) && instructions.get(p + 1).is_none_or(|next| next.is_root())))
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
                if let Some((role, amount)) = sell.cashback { *native_changes.entry(a[role].0.clone()).or_default() += i128::from(amount); }
            }
            // A once-created, once-destroyed wrapper account can return rent
            // directly, without a System transfer. Prove its original funding
            // and exclusive owner invocations, then check every other account
            // available to the close invocation. Only the original funder can
            // receive its remainder; unknown CPI or balance changes reject it.
            for (position, ownership) in instructions.iter().enumerate() {
                let a = ownership.accounts();
                let data = ownership.data();
                if ownership.program_id().0.iter().any(|b| *b != 0) || ownership.stack_height() != 2
                    || !successful_invocations.contains(&(position as u32)) { continue; }
                let Some(parent_position) = (0..position).rev().find(|p| instructions[*p].is_root()) else { continue };
                let parent = &instructions[parent_position];
                if !successful_invocations.contains(&(parent_position as u32)) { continue; }
                let (state, funder, funding, creation_accesses) = if data.len() == 52 && data[..4] == [0; 4] && a.len() == 2
                    && parent.program_id().0.as_slice() == &data[20..52]
                {
                    (a[1].0, a[0].0, u64::from_le_bytes(data[4..12].try_into().unwrap()), vec![position])
                } else if data.len() == 36 && data[..4] == [1, 0, 0, 0] && a.len() == 1
                    && parent.program_id().0.as_slice() == &data[4..36]
                {
                    let setup: Vec<_> = instructions.iter().enumerate().skip(parent_position + 1).take_while(|(_, child)| !child.is_root())
                        .filter(|(_, child)| child.accounts().iter().any(|key| key.0 == a[0].0)).collect();
                    if setup.len() != 3 || setup[2].0 != position { continue; }
                    let (funding_position, transfer) = setup[0];
                    let (allocate_position, allocate) = setup[1];
                    let f = transfer.accounts();
                    if transfer.program_id().0.iter().any(|n| *n != 0) || transfer.stack_height() != 2 || transfer.data().len() != 12
                        || transfer.data()[..4] != [2, 0, 0, 0] || f.len() != 2 || f[1].0 != a[0].0
                        || allocate.program_id().0.iter().any(|n| *n != 0) || allocate.stack_height() != 2 || allocate.data().len() != 12
                        || allocate.data()[..4] != [8, 0, 0, 0] || allocate.accounts().len() != 1 || allocate.accounts()[0].0 != a[0].0
                        || !successful_invocations.contains(&(funding_position as u32)) || !successful_invocations.contains(&(allocate_position as u32)) { continue; }
                    (a[0].0, f[0].0, u64::from_le_bytes(transfer.data()[4..12].try_into().unwrap()), vec![funding_position, allocate_position, position])
                } else { continue; };
                if protected.contains(state) || !legacy_native_sells.iter().any(|sell|
                    instructions[sell.instruction].accounts()[sell.roles[2]].0 == funder) { continue; }
                let accesses: Vec<_> = instructions.iter().enumerate().filter(|(_, call)| call.accounts().iter().any(|key| key.0 == state)).collect();
                if accesses.len() != creation_accesses.len() + 2 || accesses[0].0 != parent_position
                    || accesses[1..accesses.len() - 1].iter().map(|(p, _)| *p).ne(creation_accesses.iter().copied()) { continue; }
                let (close_position, close) = accesses[accesses.len() - 1];
                if close_position <= position || !close.is_root() || close.program_id().0 != parent.program_id().0
                    || !successful_invocations.contains(&(close_position as u32))
                    || !close.accounts().iter().any(|key| key.0 == funder)
                    || instructions.iter().filter(|call| call.program_id().0 == parent.program_id().0).count() != 2 { continue; }
                // No descendant can access the owned remainder. All explicit
                // movements are already recorded; every other parent/close
                // account must balance exactly. Conservation leaves the actual
                // initial funding as the only possible refund to its funder.
                let children_valid = [parent_position, close_position].iter().all(|root| {
                    instructions.iter().enumerate().skip(root + 1).take_while(|(_, child)| !child.is_root()).all(|(p, child)|
                        successful_invocations.contains(&(p as u32)) && child.stack_height() > 1)
                });
                let Some(source_position) = resolved.iter().position(|key| *key == state) else { continue };
                if !children_valid || meta.pre_balances.get(source_position) != Some(&0) || meta.post_balances.get(source_position) != Some(&0)
                    || parent.accounts().iter().chain(close.accounts().iter()).filter(|key| key.0 != funder && key.0 != state).any(|key| {
                        let observed = resolved.iter().position(|address| *address == key.0).and_then(|p|
                            meta.post_balances.get(p).zip(meta.pre_balances.get(p)).map(|(after, before)| i128::from(*after) - i128::from(*before)));
                        observed != Some(native_changes.get(key.0).copied().unwrap_or(0))
                    }) { continue; }
                let refund = i128::from(funding);
                if native_changes.get(state).copied() != Some(refund) { continue; }
                *native_changes.entry(state.clone()).or_default() -= refund;
                *native_changes.entry(funder.clone()).or_default() += refund;
            }
            // This retained leaf close has one previously funded state account
            // and a token source already closed by SPL. The state is accessed
            // only here, and the committed leaf has no CPIs: its observed debit
            // is necessarily an owner-authorized direct debit under Solana's
            // runtime rules. With the third account already at zero, its exact
            // pre-balance can only be credited to the declared seller wallet.
            for (position, close) in instructions.iter().enumerate() {
                let a = close.accounts();
                if !close.is_root() || close.program_id().0.as_slice() != substreams_solana_macro::b58!("D3og1Asnw7dPBcToJ3gQ2mRup728xsEn6Sg62mTm1enm")
                    || close.data().as_slice() != [12, 0, 255] || a.len() != 3 || a.iter().enumerate().any(|(p, key)| a[p + 1..].iter().any(|other| key.0 == other.0))
                    || protected.contains(a[1].0) || !successful_invocations.contains(&(position as u32))
                    || instructions.get(position + 1).is_some_and(|next| !next.is_root())
                    || instructions.iter().filter(|call| call.accounts().iter().any(|key| key.0 == a[1].0)).count() != 1
                    || !legacy_native_sells.iter().any(|sell| { let s = instructions[sell.instruction].accounts();
                        s[sell.roles[2]].0 == a[0].0 && s[sell.roles[3]].0 == a[2].0 })
                    || closes.get(a[2].0) != Some(&1) || !proven_closed.contains(a[2].0)
                    || native_changes.get(a[1].0).is_some_and(|n| *n != 0) { continue; }
                let Some(state) = resolved.iter().position(|key| *key == a[1].0) else { continue };
                let Some(source) = resolved.iter().position(|key| *key == a[2].0) else { continue };
                let Some((&before, &after)) = meta.pre_balances.get(state).zip(meta.post_balances.get(state)) else { continue };
                if before == 0 || after != 0 || meta.post_balances.get(source) != Some(&0)
                    || meta.pre_balances.get(source).map(|n| -i128::from(*n)) != native_changes.get(a[2].0).copied()
                    || !instructions[..position].iter().enumerate().any(|(p, call)| call.data().as_slice() == [9]
                        && (call.program_id().0.as_slice() == token::PROGRAM_ID || call.program_id().0.as_slice() == token_2022::PROGRAM_ID)
                        && call.accounts().first().is_some_and(|key| key.0 == a[2].0) && successful_invocations.contains(&(p as u32))) { continue; }
                *native_changes.entry(a[1].0.clone()).or_default() -= i128::from(before);
                *native_changes.entry(a[0].0.clone()).or_default() += i128::from(before);
            }
            for sell in &legacy_native_sells {
                let a = instructions[sell.instruction].accounts();
                // A repeated user's aggregate credit cannot allocate individual
                // legs. Require every contributing leg to independently recover
                // its proceeds, then retain all existing physical ledger guards.
                let user_sells: Vec<_> = legacy_native_sells.iter().filter(|other|
                    instructions[other.instruction].accounts()[other.roles[2]].0 == a[sell.roles[2]].0).collect();
                if user_sells.len() != 1 && user_sells.iter().any(|other| !other.independent_proceeds) { continue; }
                if let Some(settlement) = titan_settlements.get(&sell.instruction) {
                    if !successful_invocations.contains(&(settlement.parent as u32)) || [&settlement.funder, &settlement.quote].iter().any(|address| {
                        let exact = resolved.iter().position(|key| *key == *address).and_then(|p|
                            meta.post_balances.get(p).zip(meta.pre_balances.get(p)).map(|(after, before)| i128::from(*after) - i128::from(*before)));
                        exact != native_changes.get(*address).copied()
                    }) { continue; }
                }
                let verified = [sell.roles[0], sell.roles[2], sell.roles[7], sell.roles[8], sell.roles[9]].into_iter()
                    .chain(sell.cashback.map(|(role, _)| role)).all(|role| {
                    let address = a[role].0;
                    let exact = resolved.iter().position(|key| *key == address).and_then(|p|
                        meta.post_balances.get(p).zip(meta.pre_balances.get(p)).map(|(after, before)| i128::from(*after) - i128::from(*before)));
                    if exact != native_changes.get(address).copied() { return false; }
                    instructions.iter().enumerate().filter(|(_, call)| call.program_id().0 == &pumpfun::PROGRAM_ID
                        && call.accounts().iter().any(|key| key.0 == address)).all(|(position, call)| {
                        if legacy_native_sells.iter().any(|s| s.instruction == position) { return true; }
                        if proven_volume_refunds.contains(&position) { return true; }
                        if let Ok(pumpfun::instructions::PumpFunInstruction::BuyExactQuoteInV2(args)) = crate::pumpfun::unpack_trade_instruction(call.data()) {
                            let b = call.accounts();
                            if b.len() != 27 || b[2].0.iter().all(|n| *n == 0) || b[2].0.as_slice() == crate::SOL_MINT
                                || b[3].0.as_slice() != token_2022::PROGRAM_ID || b[4].0.as_slice() != token::PROGRAM_ID
                                || b[24].0.iter().any(|n| *n != 0) || b[26].0.as_slice() != pumpfun::PROGRAM_ID
                                || !successful_invocations.contains(&(position as u32)) { return false; }
                            let depth = if call.is_root() { 1 } else { call.stack_height() };
                            let children: Vec<_> = instructions.iter().enumerate().skip(position + 1).take_while(|(_, c)| !c.is_root()
                                && (c.stack_height() == 0 || c.stack_height() > depth)).collect();
                            if children.len() != 7 || children.iter().any(|(i,c)| c.stack_height() != depth + 1
                                || !successful_invocations.contains(&(*i as u32))) { return false; }
                            let fee = children[0].1; let fa = fee.accounts();
                            if fee.program_id().0.as_slice() != crate::pump_curve::FEE_PROGRAM || fa.len() != 2
                                || fa[0].0 != b[22].0 || fa[1].0.as_slice() != pumpfun::PROGRAM_ID
                                || fee.data().len() != 57 || fee.data()[..9] != [154,237,138,92,162,2,162,187,1]
                                || fee.data()[25..57] != b[2].0[..] { return false; }
                            let event = children[6].1; let ea = event.accounts();
                            let Ok(Some(trade)) = pumpfun::events::unpack_trade_event_minimal(event.data()) else { return false; };
                            let Ok(quote) = pumpfun::events::unpack_quote_trade_amount(event.data()) else { return false; };
                            if event.program_id().0 != &pumpfun::PROGRAM_ID || ea.len() != 1 || ea[0].0 != b[25].0
                                || !trade.is_buy || trade.user.as_slice() != b[13].0.as_slice() || trade.mint.as_slice() != b[1].0.as_slice()
                                || quote.mint.as_slice() != b[2].0.as_slice() { return false; }
                            let receipts: Vec<_> = transfers.iter().filter(|t| t.index > position as u32
                                && t.index < children[6].0 as u32).collect();
                            if receipts.len() != 5 || receipts.iter().any(|t| !t.net_verified || t.depth != depth + 1) { return false; }
                            let base = receipts[0];
                            if base.index != children[1].0 as u32 || base.source != *b[11].0 || base.destination != *b[14].0
                                || base.authority != *b[10].0 || base.mint != *b[1].0 || base.amounts[0] != trade.token_amount
                                || *base.amounts.iter().min().unwrap() < args.min_tokens_out
                                || instructions[base.index as usize].program_id().0 != b[3].0 { return false; }
                            let mut total = 0u64;
                            for (offset, destination) in [17,12,7,9].iter().enumerate() {
                                let t = receipts[offset + 1];
                                if t.index != children[offset + 2].0 as u32 || t.source != *b[15].0 || t.destination != *b[*destination].0
                                    || t.authority != *b[13].0 || t.mint != *b[2].0 || instructions[t.index as usize].program_id().0 != b[4].0
                                { return false; }
                                let Some(next) = total.checked_add(t.amounts[0]) else { return false; }; total = next;
                            }
                            return total == args.spendable_quote_in;
                        }

                        // Native buys settle through explicit System transfers.
                        // Bind every contribution before permitting a shared
                        // seller's wallet/fee accounts to occur in this call.
                        if matches!(crate::pumpfun::unpack_trade_instruction(call.data()), Ok(pumpfun::instructions::PumpFunInstruction::BuyExactSolIn(_))) {
                            let b = call.accounts();
                            if b.len() != 18 || b[7].0.iter().any(|n| *n != 0) || b[11].0.as_slice() != pumpfun::PROGRAM_ID
                                || !successful_invocations.contains(&(position as u32)) { return false; }
                            let depth = if call.is_root() { 1 } else { call.stack_height() };
                            let children: Vec<_> = instructions.iter().enumerate().skip(position + 1).take_while(|(_, child)| !child.is_root()
                                && (child.stack_height() == 0 || child.stack_height() > depth)).collect();
                            if children.len() != 7 || children.iter().any(|(p, child)| child.stack_height() != depth + 1
                                || !successful_invocations.contains(&(*p as u32))) { return false; }
                            let events: Vec<_> = children.iter().filter(|(_, child)| child.program_id().0.as_slice() == pumpfun::PROGRAM_ID).collect();
                            if events.len() != 1 { return false; }
                            let event = events[0].1;
                            let ea = event.accounts();
                            let Ok(Some(trade)) = pumpfun::events::unpack_trade_event_minimal(event.data()) else { return false };
                            let Ok(quote) = pumpfun::events::unpack_quote_trade_amount(event.data()) else { return false };
                            if ea.len() != 1 || ea[0].0 != b[10].0 || !trade.is_buy || trade.user.as_slice() != b[6].0.as_slice()
                                || trade.mint.as_slice() != b[2].0.as_slice() || quote.mint.iter().any(|n| *n != 0)
                                || quote.amount != trade.sol_amount || quote.fee < quote.buyback_fee { return false; }
                            let outputs: Vec<_> = transfers.iter().filter(|t| t.index as usize > position
                                && (t.index as usize) <= children.last().unwrap().0 && t.mint == *b[2].0 && t.source == *b[4].0
                                && t.destination == *b[5].0 && t.authority == *b[3].0 && t.depth == depth + 1 && t.net_verified).collect();
                            if outputs.len() != 1 || outputs[0].amounts[0] != trade.token_amount { return false; }
                            for (recipient, amount) in [(3, quote.amount), (9, quote.creator_fee), (1, quote.fee - quote.buyback_fee), (17, quote.buyback_fee)] {
                                if children.iter().filter(|(_, child)| child.program_id().0.iter().all(|n| *n == 0)
                                    && child.data().len() == 12 && child.data()[..4] == [2, 0, 0, 0]
                                    && child.accounts().len() == 2 && child.accounts()[0].0 == b[6].0 && child.accounts()[1].0 == b[recipient].0
                                    && u64::from_le_bytes(child.data()[4..12].try_into().unwrap()) == amount).count() != 1 { return false; }
                            }
                            return children.iter().all(|(_, child)| child.program_id().0.iter().all(|n| *n == 0)
                                || child.program_id().0 == b[8].0 || child.program_id().0.as_slice() == crate::pump_curve::FEE_PROGRAM
                                || child.program_id().0.as_slice() == pumpfun::PROGRAM_ID);
                        }
                        // Idempotent volume initialization has no implicit
                        // native debit. Its optional owned creation is recorded
                        // separately, and its sole event binds payer and user.
                        if call.data().as_slice() == [94, 6, 202, 115, 255, 96, 232, 183] {
                            let b = call.accounts();
                            if b.len() != 6 || b[3].0.iter().any(|n| *n != 0) || b[5].0.as_slice() != pumpfun::PROGRAM_ID
                                || !successful_invocations.contains(&(position as u32))
                                || b[2].0.as_slice() != solana_pubkey::Pubkey::find_program_address(&[b"user_volume_accumulator", b[1].0],
                                    &solana_pubkey::Pubkey::new_from_array(pumpfun::PROGRAM_ID)).0.to_bytes() { return false; }
                            let depth = if call.is_root() { 1 } else { call.stack_height() };
                            let children: Vec<_> = instructions.iter().enumerate().skip(position + 1).take_while(|(_, child)| !child.is_root()
                                && (child.stack_height() == 0 || child.stack_height() > depth)).collect();
                            if !matches!(children.len(), 1 | 2) || children.iter().any(|(p, child)| child.stack_height() != depth + 1
                                || !successful_invocations.contains(&(*p as u32))) { return false; }
                            let event = children.last().unwrap().1;
                            if event.program_id().0.as_slice() != pumpfun::PROGRAM_ID || event.accounts().len() != 1 || event.accounts()[0].0 != b[4].0
                                || event.data().len() != 88 || event.data().get(..16) != Some([228, 69, 165, 46, 81, 203, 154, 29, 134, 36, 13, 72, 232, 101, 130, 216].as_slice())
                                || event.data().get(16..48) != Some(b[0].0.as_slice()) || event.data().get(48..80) != Some(b[1].0.as_slice()) { return false; }
                            if children.len() == 2 {
                                let create = children[0].1;
                                return create.program_id().0.iter().all(|n| *n == 0) && create.data().len() == 52 && create.data()[..4] == [0; 4]
                                    && create.data()[20..52] == pumpfun::PROGRAM_ID && create.accounts().len() == 2
                                    && create.accounts()[0].0 == b[0].0 && create.accounts()[1].0 == b[2].0;
                            }
                            return true;
                        }
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
        let (buy, amount, limit) = match crate::pumpfun::unpack_trade_instruction(ix.data()) {
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
            || instructions.iter().enumerate().filter(|(position, call)| call.program_id().0 == &pumpfun::PROGRAM_ID
                && call.accounts().iter().any(|account| account.0 == a[5].0) && !proven_pump_extensions.contains_key(position)).count() != 1
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
            let earlier_extension = !buy && proven_pump_extensions.values().any(|(funding, pool, user)|
                *funding == t.index && t.index < index as u32 && pool == a[5].0 && user == a[8].0);
            if t.source != *a[8].0 || t.destination != *a[5].0
                || !earlier_extension && (t.index <= index as u32 || t.index as usize >= end || t.depth != depth + 1)
                || !instructions[t.index as usize].program_id().0.iter().all(|byte| *byte == 0)
                || !successful_invocations.contains(&t.index) || if buy { t.amounts[0] != user_lamports } else { t.index >= base.index }
            { continue; }
            t.amounts[0]
        } else { 0 };
        let Some(meta) = tx.meta.as_option() else { continue };
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
    pending.sort_by_key(|swap| matches!(swap.protocol.to_i32(), 4 | 5));
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
                if [crate::rfq::UPDATED_PROGRAM_ID.as_slice(), crate::rfq::V2_PROGRAM_ID.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some((native, input, output)) = rfq_receipts.get(&(index as u32)) else { continue };
                    let identity = (index as u32, *input, *output);
                    if swap.amm_pool != native.amm_pool || swap.user != native.user
                        || swap.input_mint != native.input_mint || swap.output_mint != native.output_mint
                        || (swap.input_amount, swap.output_amount) != (native.input_amount, native.output_amount)
                        || expected_transfer.is_some_and(|position| position != input.min(output).to_owned())
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol = pb::Protocol::JupiterRfq.into();
                    swap.program_id = ix.program_id().0.clone();
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0 == &substreams_solana_idls::meteora::dlmm::PROGRAM_ID
                    && ix.data().starts_with(&substreams_solana_idls::meteora::dlmm::instructions::SWAP2)
                    && ix.data().get(24..28).is_some_and(|count| count == [0, 0, 0, 0])
                    && matches!(ix.data().len(), 32 | 34 | 56)
                {
                    if !crate::meteora_dlmm::padded_swap2_settlement(index as u32, ix, &instructions[index + 1..end as usize], &successful_invocations)
                        || scoped.last().is_none_or(|t| !t.net_verified || *t.amounts.iter().min().unwrap() < u64::from_le_bytes(ix.data()[16..24].try_into().unwrap()))
                    { continue; }
                }
                if [crate::proprietary_swaps::QUANTUM.as_slice(), crate::proprietary_swaps::BINARYFI.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout) = crate::proprietary_swaps::bound_layout(ix, mints) else { continue };
                    if !crate::proprietary_swaps::settlement(index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
                        || scoped.len() != 2 { continue; }
                    let (input, output) = (scoped[0], scoped[1]);
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if !input.net_verified || !output.net_verified || net == 0
                        || input.index != index as u32 + 1 || output.index != end - 1
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[0].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || input.amounts[0] != layout.amount || net < layout.minimum_output
                        || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index)
                        || native_evidence.contains(&identity)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0.as_slice() == crate::archer::PROGRAM_ID {
                    let Some(event) = archer_fills.get(&(index as u32)) else { continue };
                    let Some(layout) = crate::archer::bound_layout(ix,event) else { continue };
                    if !crate::archer::settlement(index as u32, ix, &instructions[index + 1..end as usize], event, mints, &successful_invocations)
                        || scoped.len() != 2 { continue; }
                    let (input, output) = (scoped[0], scoped[1]);
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if !input.net_verified || !output.net_verified || net == 0
                        || input.index != index as u32 + 1 || output.index != end - 1
                        || swap.amm_pool != *accounts[1].0 || swap.user != *accounts[0].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || input.amounts[0] != layout.amount || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index) || native_evidence.contains(&identity)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0.as_slice()==substreams_solana_idls::pancakeswap::PROGRAM_ID
                    && crate::native_venues::layout(ix).is_none() { continue; }
                if ix.program_id().0.as_slice()==substreams_solana_idls::pancakeswap::PROGRAM_ID && ix.data().starts_with(&crate::pancakeswap::LEGACY_SWAP) {
                    let Some(layout)=crate::pancakeswap::legacy_layout(ix) else { continue };
                    let Some((gross,net))=crate::pancakeswap::legacy_settlement(index as u32,tx,ix,
                        &instructions[index+1..end as usize],mints,&successful_invocations) else { continue };
                    if scoped.len()!=2 { continue; }
                    let (input,output)=(scoped[0],scoped[1]);let identity=(index as u32,input.index,output.index);
                    if !input.net_verified || !output.net_verified || (input.amounts[0],*output.amounts.iter().min().unwrap())!=(gross,net)
                        || swap.amm_pool!=*accounts[layout.pool].0 || swap.user!=*accounts[layout.authority].0
                        || swap.input_mint!=input.mint || swap.output_mint!=output.mint
                        || (swap.input_amount,swap.output_amount)!=(gross,net)
                        || expected_transfer.is_some_and(|position|position!=input.index)
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol=(layout.protocol as i32).into();evidence=Some(identity);break 'candidate;
                }
                if [crate::patch_bxlw::PATCH.as_slice(),crate::patch_bxlw::BXLW.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout) = crate::patch_bxlw::layout(ix) else { continue };
                    let Some((gross,net)) = crate::patch_bxlw::settlement(index as u32,tx,ix,
                        &instructions[index+1..end as usize],mints,&successful_invocations) else { continue };
                    if scoped.len() != 2 { continue; }
                    let (input,output) = (scoped[0],scoped[1]); let identity = (index as u32,input.index,output.index);
                    if !input.net_verified || !output.net_verified || (input.amounts[0],*output.amounts.iter().min().unwrap()) != (gross,net)
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || (swap.input_amount,swap.output_amount) != (gross,net)
                        || expected_transfer.is_some_and(|position|position != input.index)
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol = (layout.protocol as i32).into(); evidence = Some(identity); break 'candidate;
                }
                if ix.program_id().0.as_slice() == crate::m0_native_order::PROGRAM_ID {
                    let Some(layout) = crate::m0_native_order::layout(ix) else { continue };
                    let Some((gross,net)) = crate::m0_native_order::settlement(index as u32,tx,ix,
                        &instructions[index+1..end as usize],mints,&successful_invocations) else { continue };
                    if scoped.len() != 2 { continue; }
                    let (input,output) = (scoped[0],scoped[1]); let identity = (index as u32,input.index,output.index);
                    if !input.net_verified || !output.net_verified || (input.amounts[0],output.amounts[0]) != (gross,net)
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || (swap.input_amount,swap.output_amount) != (gross,net)
                        || expected_transfer.is_some_and(|position|position != input.index)
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol = (layout.protocol as i32).into(); evidence = Some(identity); break 'candidate;
                }
                if ix.program_id().0.as_slice() == crate::picon_dlmm::PROGRAM_ID {
                    let Some(layout) = crate::picon_dlmm::layout(ix) else { continue };
                    let Some((gross,net)) = crate::picon_dlmm::settlement(index as u32,tx,ix,
                        &instructions[index+1..end as usize],mints,&successful_invocations) else { continue };
                    if scoped.len() != 2 { continue; }
                    let (input,output) = (scoped[0],scoped[1]);
                    let identity = (index as u32,input.index,output.index);
                    if !input.net_verified || !output.net_verified || output.amounts[0] != net
                        || input.amounts[0] != gross || swap.amm_pool != *accounts[layout.pool].0
                        || swap.user != *accounts[layout.authority].0 || swap.input_mint != input.mint
                        || swap.output_mint != output.mint || (swap.input_amount,swap.output_amount) != (gross,net)
                        || expected_transfer.is_some_and(|position|position != input.index)
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol = (layout.protocol as i32).into(); evidence = Some(identity); break 'candidate;
                }
                if [crate::legacy_amm::ALDRIN.as_slice(),crate::legacy_amm::CROPPER.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout) = crate::legacy_amm::layout(ix) else { continue };
                    let Some((gross,net)) = crate::legacy_amm::settlement(index as u32,tx,ix,
                        &instructions[index+1..end as usize],mints,&successful_invocations) else { continue };
                    let Some(input) = scoped.iter().find(|t|t.index == index as u32 + 1) else { continue };
                    let Some(output) = scoped.iter().find(|t|t.index == end - 1) else { continue };
                    let identity = (index as u32,input.index,output.index);
                    if !input.net_verified || !output.net_verified || output.amounts[0] != net
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || (swap.input_amount,swap.output_amount) != (gross,net)
                        || expected_transfer.is_some_and(|position| position != input.index)
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol = (layout.protocol as i32).into();
                    evidence = Some(identity); break 'candidate;
                }
                if ix.program_id().0.as_slice() == crate::hadron::PROGRAM_ID {
                    let Some(layout) = crate::hadron::layout(ix) else { continue };
                    let Some((input_position, output_position)) = crate::hadron::settlement(index as u32, ix,
                        &instructions[index + 1..end as usize], mints, &successful_invocations) else { continue };
                    let Some(input) = scoped.iter().find(|t| t.index == input_position) else { continue };
                    let Some(output) = scoped.iter().find(|t| t.index == output_position) else { continue };
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input_position, output_position);
                    if !input.net_verified || !output.net_verified || net == 0 || net < layout.minimum_output
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || (swap.input_amount, swap.output_amount) != (layout.amount, net)
                        || expected_transfer.is_some_and(|position| position != input_position)
                        || native_evidence.contains(&identity)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if [crate::gator::PROGRAM_ID.as_slice(),crate::peg::PROGRAM_ID.as_slice(),crate::sanctum_infinity::PROGRAM_ID.as_slice(),crate::anchor_swap::PROGRAM_ID.as_slice(),crate::raydium_cpmm_fork::PROGRAM_ID.as_slice(),crate::exponent::PROGRAM_ID.as_slice(),crate::janus::PROGRAM_ID.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout)=crate::native_venues::layout(ix)else{continue;};
                    let receipt=match layout.protocol {
                        pb::Protocol::Gator=>crate::gator::settlement(tx,index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations),
                        pb::Protocol::AnchorSwap=>crate::anchor_swap::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations),
                        pb::Protocol::RaydiumCpmmFork=>crate::raydium_cpmm_fork::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations),
                        pb::Protocol::Exponent=>crate::exponent::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations),
                        pb::Protocol::Janus=>crate::janus::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations),
                        pb::Protocol::Peg=>crate::peg::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations),
                        _=>crate::sanctum_infinity::settlement(tx,index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations),
                    };
                    let Some((gross,net))=receipt else{continue;};
                    let side=if layout.protocol==pb::Protocol::AnchorSwap{usize::from(scoped.iter().any(|t|t.source==*accounts[7].0&&t.destination==*accounts[5].0&&t.authority==*accounts[10].0))}else if layout.fixed_users{0}else{usize::from(mints.mint_for(accounts[4].0).as_deref()!=ix.data().get(8..40))};
                    let users=if layout.fixed_users{[layout.users[0],layout.users[1]]}else{[layout.users[side],layout.users[1-side]]};
                    let vaults=[layout.vaults[side],layout.vaults[1-side]];
                    let Some(input)=scoped.iter().find(|t|t.source==*accounts[users[0]].0&&t.destination==*accounts[vaults[0]].0)else{continue;};
                    let Some(output)=scoped.iter().find(|t|t.source==*accounts[vaults[1]].0&&t.destination==*accounts[users[1]].0)else{continue;};
                    let identity=(index as u32,input.index,output.index);
                    if scoped.iter().filter(|t| ![pb::Protocol::Exponent,pb::Protocol::Janus].contains(&layout.protocol) || t.depth==depth+1).count()!=if [pb::Protocol::AnchorSwap,pb::Protocol::Exponent,pb::Protocol::Janus].contains(&layout.protocol){3}else{2} || !input.net_verified || !output.net_verified
                        || (if layout.protocol==pb::Protocol::AnchorSwap{layout.amount}else{input.amounts[0]},*output.amounts.iter().min().unwrap())!=(gross,net)
                        || swap.amm_pool!=*accounts[layout.pool].0 || swap.user!=*accounts[layout.authority].0
                        || swap.input_mint!=input.mint || swap.output_mint!=output.mint
                        || [users[0],vaults[0]].iter().any(|r|mints.mint_for(accounts[*r].0).as_deref()!=Some(input.mint.as_slice()))
                        || [users[1],vaults[1]].iter().any(|r|mints.mint_for(accounts[*r].0).as_deref()!=Some(output.mint.as_slice()))
                        || (swap.input_amount,swap.output_amount)!=(gross,net)
                        || expected_transfer.is_some_and(|position|position!=if layout.protocol==pb::Protocol::AnchorSwap{input.index.min(output.index)}else{input.index}) || native_evidence.contains(&identity){continue;}
                    swap.protocol=(layout.protocol as i32).into();evidence=Some(identity);break 'candidate;
                }
                if [crate::anchor_markets::GHOSTY.as_slice(),crate::anchor_markets::MEMEFUN.as_slice(),crate::omnipair::PROGRAM_ID.as_slice(),crate::fraudsworth::PROGRAM_ID.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout)=crate::native_venues::layout(ix)else{continue;};
                    if !anchor_receipts.contains(&(index as u32))
                        || scoped.len()!=2 {continue;}
                    let (input,output)=(scoped[0],scoped[1]);let net=*output.amounts.iter().min().unwrap();
                    let identity=(index as u32,input.index,output.index);
                    if !input.net_verified || !output.net_verified || net==0 || net<layout.minimum_output
                        || swap.amm_pool!=*accounts[layout.pool].0 || swap.user!=*accounts[layout.authority].0
                        || swap.input_mint!=input.mint || swap.output_mint!=output.mint
                        || (swap.input_amount,swap.output_amount)!=(layout.amount,net)
                        || expected_transfer.is_some_and(|position|position!=input.index) || native_evidence.contains(&identity)
                    {continue;}
                    swap.protocol=(layout.protocol as i32).into();
                    evidence=Some(identity);break 'candidate;
                }
                if [crate::jupiter_perps::PROGRAM_ID.as_slice(),crate::numeraire::PROGRAM_ID.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout)=crate::native_venues::layout(ix)else{continue;};
                    if scoped.len()!=2 {continue;}
                    let Some(input)=scoped.iter().find(|t|t.source==*accounts[layout.users[0]].0&&t.destination==*accounts[layout.vaults[0]].0&&t.authority==*accounts[layout.authority].0)else{continue;};
                    let Some(output)=scoped.iter().find(|t|t.source==*accounts[layout.vaults[1]].0&&t.destination==*accounts[layout.users[1]].0)else{continue;};let net=*output.amounts.iter().min().unwrap();
                    let valid=if layout.protocol==pb::Protocol::JupiterPerps {
                        crate::jupiter_perps::settlement(index as u32,ix,&instructions[index+1..end as usize],&successful_invocations)==Some((input.amounts[0],net))
                    }else{crate::numeraire::settlement(index as u32,tx,ix,&instructions[index+1..end as usize],mints,&successful_invocations,net)};
                    let identity=(index as u32,input.index,output.index);
                    if !valid || !input.net_verified || !output.net_verified || net==0
                        || input.mint==output.mint || swap.amm_pool!=*accounts[layout.pool].0 || swap.user!=*accounts[layout.authority].0
                        || swap.input_mint!=input.mint || swap.output_mint!=output.mint
                        || [layout.users[0],layout.vaults[0]].iter().any(|role|mints.mint_for(accounts[*role].0).as_deref()!=Some(input.mint.as_slice()))
                        || [layout.users[1],layout.vaults[1]].iter().any(|role|mints.mint_for(accounts[*role].0).as_deref()!=Some(output.mint.as_slice()))
                        || (swap.input_amount,swap.output_amount)!=(input.amounts[0],net)
                        || expected_transfer.is_some_and(|position|position!=input.index.min(output.index)) || native_evidence.contains(&identity)
                    {continue;}
                    swap.protocol=(layout.protocol as i32).into();evidence=Some(identity);break 'candidate;
                }
                if [crate::futarchy::PROGRAM_ID.as_slice(),crate::guacswap::PROGRAM_ID.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout)=crate::native_venues::layout(ix)else{continue;};
                    if scoped.len()!=if layout.protocol==pb::Protocol::Guacswap{3}else{2}{continue;}
                    let (input,output)=(scoped[0],scoped[1]);let net=*output.amounts.iter().min().unwrap();
                    let receipt=if layout.protocol==pb::Protocol::Futarchy{crate::futarchy::settlement(index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations)}
                        else{crate::guacswap::settlement(index as u32,ix,&instructions[index+1..end as usize],mints,&successful_invocations)};
                    let identity=(index as u32,input.index,output.index);
                    if receipt!=Some((input.amounts[0],net)) || !input.net_verified || !output.net_verified || net==0
                        || input.mint==output.mint || swap.amm_pool!=*accounts[layout.pool].0 || swap.user!=*accounts[layout.authority].0
                        || swap.input_mint!=input.mint || swap.output_mint!=output.mint
                        || [layout.users[0],layout.vaults[0]].iter().any(|role|mints.mint_for(accounts[*role].0).as_deref()!=Some(input.mint.as_slice()))
                        || [layout.users[1],layout.vaults[1]].iter().any(|role|mints.mint_for(accounts[*role].0).as_deref()!=Some(output.mint.as_slice()))
                        || (swap.input_amount,swap.output_amount)!=(input.amounts[0],net)
                        || expected_transfer.is_some_and(|position|position!=input.index) || native_evidence.contains(&identity)
                    {continue;}
                    swap.protocol=(layout.protocol as i32).into();evidence=Some(identity);break 'candidate;
                }
                if ix.program_id().0.as_slice() == crate::quay::PROGRAM_ID {
                    let Some(layout) = crate::quay::layout(ix) else { continue };
                    if !crate::quay::settlement(tx, index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
                        || scoped.len() != 2 { continue; }
                    let (input, output) = (scoped[0], scoped[1]);
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if !input.net_verified || !output.net_verified || net == 0
                        || input.index != index as u32 + 1 || output.index != end - 1
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || input.amounts[0] != layout.amount || net < layout.minimum_output
                        || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index) || native_evidence.contains(&identity)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0.as_slice() == crate::riptide::PROGRAM_ID {
                    let Some(layout) = crate::riptide::layout(ix) else { continue };
                    if !crate::riptide::settlement(tx, index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
                        || scoped.len() != 2 { continue; }
                    let (input, output) = (scoped[0], scoped[1]);
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if !input.net_verified || !output.net_verified || net == 0
                        || input.index != index as u32 + 1 || output.index != end - 1
                        || swap.protocol != pb::Protocol::Riptide
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index) || native_evidence.contains(&identity)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0.as_slice() == crate::triangle::PROGRAM_ID {
                    let Some(layout) = crate::triangle::layout(ix) else { continue };
                    let principals: Vec<_> = scoped.iter().copied().filter(|t| t.depth == depth + 1).collect();
                    if !crate::triangle::settlement(index as u32, ix, &instructions[index + 1..end as usize], mints, &successful_invocations)
                        || principals.len() != 2 { continue; }
                    let (input, output) = (principals[0], principals[1]);
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if !input.net_verified || !output.net_verified || net == 0
                        || input.index != index as u32 + 1 || output.index != end - 1
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[0].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || input.amounts[0] != layout.amount || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index) || native_evidence.contains(&identity)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if [crate::spl_swap_forks::DOOAR.as_slice(), crate::spl_swap_forks::DEXLAB.as_slice(),crate::spl_swap_forks::PENGUIN.as_slice()].contains(&ix.program_id().0.as_slice()) {
                    let Some(layout) = crate::spl_swap_forks::layout(ix) else { continue };
                    if !crate::spl_swap_forks::settlement(index as u32, tx, ix, &instructions[index + 1..end as usize], mints, &successful_invocations) {
                        continue;
                    }
                    let inputs: Vec<_> = scoped.iter().filter(|t| t.index == index as u32 + 1).collect();
                    let outputs: Vec<_> = scoped.iter().filter(|t| t.index == end - 1).collect();
                    if inputs.len() != 1 || outputs.len() != 1 { continue; }
                    let (input, output) = (inputs[0], outputs[0]);
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if !input.net_verified || !output.net_verified || net == 0
                        || swap.amm_pool != *accounts[0].0 || swap.user != *accounts[2].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || input.amounts[0] > layout.amount || net < layout.minimum_output
                        || (swap.input_amount, swap.output_amount) != (input.amounts[0], net)
                        || expected_transfer.is_some_and(|position| position != input.index)
                        || native_evidence.contains(&identity)
                        || scoped.iter().any(|t| t.index != input.index && t.index != output.index)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
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
                    if swap.protocol == pb::Protocol::Pumpfun && swap.amm_pool == *pool && swap.user == *accounts[3].0
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
                if ix.program_id().0 == &crate::jupiter_lend::PROGRAM_ID {
                    let Some(&(input_position, output_position)) = jupiter_lend_receipts.get(&(index as u32)) else { continue };
                    let (input, output) = (&transfers[input_position], &transfers[output_position]);
                    let identity = (index as u32, input.index, output.index);
                    if swap.protocol == pb::Protocol::JupiterLend && swap.amm_pool == *accounts[1].0 && swap.user == *accounts[0].0
                        && swap.input_mint == input.mint && swap.output_mint == output.mint
                        && (swap.input_amount, swap.output_amount) == (input.amounts[0], *output.amounts.iter().min().unwrap())
                        && !expected_transfer.is_some_and(|transfer| transfer != input.index)
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
                if swap.protocol == pb::Protocol::Pumpfun
                    && swap.output_mint == crate::SOL_MINT
                    && matches!(
                        crate::pumpfun::unpack_trade_instruction(ix.data()),
                        Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)
                            | pumpfun::instructions::PumpFunInstruction::SellV2(_)
                            | pumpfun::instructions::PumpFunInstruction::SellV3(_))
                    )
                {
                    if matches!(crate::pumpfun::unpack_trade_instruction(ix.data()),
                        Ok(pumpfun::instructions::PumpFunInstruction::Sell(_))) && accounts.len() == 16
                        || matches!(crate::pumpfun::unpack_trade_instruction(ix.data()),
                            Ok(pumpfun::instructions::PumpFunInstruction::SellV2(_))) && matches!(accounts.len(), 26 | 27)
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
                        .enumerate()
                        .filter(|(position, call)| call.program_id().0 == &swap.amm && call.accounts().iter().any(|a| a.0 == &swap.amm_pool)
                            && !proven_pump_extensions.contains_key(position))
                        .count();
                    let explicit_sol = transfers
                        .iter()
                        .any(|t| t.mint == crate::SOL_MINT && (t.source == swap.amm_pool || t.destination == swap.amm_pool));
                    let pool_delta = tx.resolved_accounts().iter().position(|a| *a == &swap.amm_pool).and_then(|position| {
                        tx.meta
                            .as_option()
                            .and_then(|meta| Some(i128::from(*meta.pre_balances.get(position)?) - i128::from(*meta.post_balances.get(position)?)))
                    });
                    // A bounded same-curve buy/sell pair has independently
                    // chained native reserves and complete wallet/rent ledgers.
                    if let Some((net, input_index)) = crate::pumpfun::round_trip_native_sell(tx, &instructions, index, &successful_invocations) {
                        if swap.amm_pool == *accounts[5].0 && swap.user == *accounts[8].0
                            && swap.input_mint == *accounts[1].0 && swap.output_mint == crate::SOL_MINT
                            && scoped.iter().any(|t| t.index == input_index && t.amounts[0] == swap.input_amount
                                && t.mint == swap.input_mint && t.net_verified)
                            && expected_transfer.is_none_or(|position| position == input_index)
                        {
                            let identity = (index as u32, input_index, input_index);
                            if !native_evidence.contains(&identity) {
                                if swap.output_amount != net {
                                    let mut original = swap.clone();
                                    original.verification_failure = "event_amounts_replaced_by_exact_user_flows".into();
                                    rejected.push(original);
                                }
                                swap.output_amount = net;
                                evidence = Some(identity);
                                break 'candidate;
                            }
                        }
                    }
                    // V3 retains protocol/creator fees in the curve and pays
                    // buyback fees to its declared recipient. Prove the net
                    // user proceeds against both exact lamport deltas; the
                    // event's gross quote alone cannot verify this variant.
                    if calls == 1
                        && matches!(
                            crate::pumpfun::unpack_trade_instruction(ix.data()),
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
                                    || !(t.index > index as u32 && t.index < end && t.depth == depth + 1
                                        || proven_pump_extensions.values().any(|(funding, pool, user)| *funding == t.index && t.index < index as u32
                                            && pool == &swap.amm_pool && user == &swap.user))
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
                            let meta = tx.meta.as_option()?;
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
                if ix.program_id().0 == &substreams_solana_idls::meteora::dlmm::PROGRAM_ID {
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
                    if inputs.len() != 1 || inputs[0].amounts[0] != swap.input_amount {
                        continue;
                    }
                    let input = inputs[0];
                    let (output, host_fee_index) = if outputs.len() == 1 {
                        (outputs[0], None)
                    } else if outputs.len() == 2
                        && layout.host_fee_account.as_deref() == Some(layout.user_token_out.as_slice())
                        && outputs.iter().all(|t| t.net_verified)
                    {
                        let events: Vec<_> = instructions.iter().enumerate()
                            .filter(|(position, child)| *position > outputs[1].index as usize && *position < end as usize
                                && child.stack_height() == depth + 1 && successful_invocations.contains(&(*position as u32)))
                            .filter_map(|(_, child)| crate::meteora_dlmm::output_host_fee_event(child, &layout, input.amounts[0], &swap.output_mint))
                            .collect();
                        if events.len() != 1 || (outputs[1].amounts[0], outputs[0].amounts[0]) != events[0] {
                            continue;
                        }
                        (outputs[1], Some(outputs[0].index))
                    } else {
                        continue;
                    };
                    if !output.net_verified || !output.amounts.contains(&swap.output_amount)
                        || host_fee_index.is_some() && scoped.iter().any(|t|
                            t.index != input.index && t.index != output.index && Some(t.index) != host_fee_index
                                && [t.source.as_slice(), t.destination.as_slice()].iter().any(|account|
                                    *account == layout.user_token_in.as_slice() || *account == layout.user_token_out.as_slice()))
                    {
                        continue;
                    }
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
                    if !crate::meteora_daam::observed_settlement(index as u32, ix, &instructions[index + 1..end as usize], &successful_invocations,
                        input.amounts[0], input.net_verified.then(|| *input.amounts.iter().min().unwrap()),
                        output.amounts[0], *output.amounts.iter().min().unwrap())
                    { continue; }
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
                if [substreams_solana_macro::b58!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp").as_slice(),
                    substreams_solana_macro::b58!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH").as_slice(),
                    substreams_solana_macro::b58!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE").as_slice(),
                    substreams_solana_macro::b58!("DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD").as_slice()].contains(&ix.program_id().0.as_slice())
                    && crate::native_venues::layout(ix).is_none(){continue;}
                if [substreams_solana_macro::b58!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C").as_slice(),
                    substreams_solana_macro::b58!("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN").as_slice()].contains(&ix.program_id().0.as_slice())
                    && crate::native_venues::layout(ix).is_none()
                { continue; }
                if crate::suffixed_anchor_swaps::applies(ix) {
                    let Some(layout) = crate::suffixed_anchor_swaps::layout(ix) else { continue };
                    let Some((input_amount, gross_output)) = crate::suffixed_anchor_swaps::settlement(index as u32, tx, ix, &instructions[index + 1..end as usize], mints, &successful_invocations) else { continue };
                    let Some(input) = scoped.iter().find(|t| t.source == *accounts[layout.users[0]].0 && layout.vaults.iter().any(|v| t.destination == *accounts[*v].0)) else { continue };
                    let Some(output) = scoped.iter().find(|t| layout.vaults.iter().any(|v| t.source == *accounts[*v].0) && t.destination == *accounts[layout.users[1]].0) else { continue };
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if scoped.len() != 2 || !input.net_verified || !output.net_verified || net == 0 || net < layout.minimum_output
                        || input.amounts[0] != input_amount || output.amounts[0] != gross_output
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || (swap.input_amount, swap.output_amount) != (input_amount, net)
                        || expected_transfer.is_some_and(|position| position != input.index.min(output.index))
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol = (layout.protocol as i32).into();
                    swap.program_id = ix.program_id().0.clone();
                    swap.amm = ix.program_id().0.clone();
                    evidence = Some(identity);
                    break 'candidate;
                }
                if crate::current_proprietary::applies(ix) {
                    let Some(layout) = crate::current_proprietary::layout(ix) else { continue };
                    let Some((input_amount, gross_output)) = crate::current_proprietary::settlement(index as u32, tx, ix, &instructions[index + 1..end as usize], mints, &successful_invocations) else { continue };
                    let Some(input) = scoped.iter().find(|t| t.source == *accounts[layout.users[0]].0 && t.destination == *accounts[layout.vaults[0]].0) else { continue };
                    let Some(output) = scoped.iter().find(|t| t.source == *accounts[layout.vaults[1]].0 && t.destination == *accounts[layout.users[1]].0) else { continue };
                    let net = *output.amounts.iter().min().unwrap();
                    let identity = (index as u32, input.index, output.index);
                    if scoped.len() != 2 || !input.net_verified || !output.net_verified || net == 0 || net < layout.minimum_output
                        || input.amounts[0] != input_amount || output.amounts[0] != gross_output
                        || swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || (swap.input_amount, swap.output_amount) != (input_amount, net)
                        || expected_transfer.is_some_and(|position| position != input.index.min(output.index))
                        || native_evidence.contains(&identity) { continue; }
                    swap.protocol = (layout.protocol as i32).into();
                    swap.program_id = ix.program_id().0.clone();
                    swap.amm = ix.program_id().0.clone();
                    evidence = Some(identity);
                    break 'candidate;
                }
                // A routed candidate cannot turn an unsupported SolFi call
                // into a trade through generic amount matching below.
                if ix.program_id().0.as_slice() == substreams_solana_idls::solfi::v2::PROGRAM_ID
                    && crate::native_venues::layout(ix).is_none()
                { continue; }
                if ix.program_id().0.as_slice() == crate::saber::PROGRAM_ID
                    || ix.program_id().0.as_slice() == substreams_solana_idls::solfi::v2::PROGRAM_ID && matches!(ix.data().len(), 25 | 153)
                {
                    let Some(layout) = crate::native_venues::layout(ix) else { continue };
                    let calls = &instructions[index + 1..end as usize];
                    let Some((input_amount, output_amount)) = (if layout.protocol == pb::Protocol::Saber {
                        crate::saber::settlement(ix, calls)
                    } else {
                        crate::native_venues::solfi_extended_settlement(ix, calls)
                    }) else { continue };
                    if !successful_invocations.contains(&(index as u32))
                        || calls.iter().enumerate().any(|(offset, _)| !successful_invocations.contains(&((index + offset + 1) as u32)))
                    { continue; }
                    let Some(input) = scoped.iter().find(|t| t.index == index as u32 + 1) else { continue };
                    let Some(output) = scoped.iter().find(|t| t.index == index as u32 + 2) else { continue };
                    let reverse = usize::from(layout.protocol == pb::Protocol::SolfiV2 && input.source == *accounts[7].0);
                    let source = accounts[if layout.protocol == pb::Protocol::Saber { 3 } else { 6 + reverse }].0;
                    let destination = accounts[if layout.protocol == pb::Protocol::Saber { 6 } else { 7 - reverse }].0;
                    let vault_in = accounts[if layout.protocol == pb::Protocol::Saber { 4 } else { 4 + reverse }].0;
                    let vault_out = accounts[if layout.protocol == pb::Protocol::Saber { 5 } else { 5 - reverse }].0;
                    let identity = (index as u32, input.index, output.index);
                    if swap.amm_pool != *accounts[layout.pool].0 || swap.user != *accounts[layout.authority].0
                        || swap.input_mint != input.mint || swap.output_mint != output.mint
                        || mints.mint_for(source).as_ref() != Some(&input.mint)
                        || mints.mint_for(destination).as_ref() != Some(&output.mint)
                        || mints.mint_for(vault_in).as_ref() != Some(&input.mint)
                        || mints.mint_for(vault_out).as_ref() != Some(&output.mint)
                        || layout.protocol == pb::Protocol::SolfiV2
                            && (input.mint != *accounts[8 + reverse].0 || output.mint != *accounts[9 - reverse].0)
                        || input.amounts[0] != input_amount || output.amounts.iter().min() != Some(&output_amount)
                        || (swap.input_amount, swap.output_amount) != (input_amount, output_amount)
                        || expected_transfer.is_some_and(|position| position != input.index)
                        || native_evidence.contains(&identity)
                    { continue; }
                    evidence = Some(identity);
                    break 'candidate;
                }
                if ix.program_id().0.as_slice() == substreams_solana_macro::b58!("FLUXubRmkEi2q6K3Y9kBPg9248ggaZVsoSFhtJHSrm1X") {
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
                // Use the same bounded native argument parser as PumpSwap
                // event decoding. This also guards Jupiter-derived candidates;
                // an event or exact amount coincidence cannot bypass a malformed
                // native payload or the caller's exact input/minimum constraint.
                // The existing bounded multi-hop builder independently proves
                // each physical intermediate leg. Single-trade account/argument
                // layouts cannot describe those declared five-account groups.
                let proven_pump_multihop = expected_transfer.is_some_and(|position|
                    native_pump_multihop_receipts.contains(&(index as u32, position,
                        swap.amm_pool.clone(), swap.user.clone(), swap.input_mint.clone(), swap.input_amount,
                        swap.output_mint.clone(), swap.output_amount)));
                if ix.program_id().0 == &pumpswap::PROGRAM_ID && !proven_pump_multihop {
                    let Ok(a) = pumpswap::accounts::TradeTokenAccounts::try_from(ix) else { continue };
                    let (source, destination) = if swap.input_mint == a.quote_mint.to_bytes()
                        && swap.output_mint == a.base_mint.to_bytes() {
                        (a.user_quote_token_account.to_bytes().to_vec(), a.user_base_token_account.to_bytes().to_vec())
                    } else if swap.input_mint == a.base_mint.to_bytes()
                        && swap.output_mint == a.quote_mint.to_bytes() {
                        (a.user_base_token_account.to_bytes().to_vec(), a.user_quote_token_account.to_bytes().to_vec())
                    } else { continue };
                    let inputs: Vec<_> = scoped.iter().filter(|t| t.source == source && t.mint == swap.input_mint).collect();
                    let outputs: Vec<_> = scoped.iter().filter(|t| t.destination == destination && t.mint == swap.output_mint).collect();
                    let total = inputs.iter().try_fold(0u64, |n, t| n.checked_add(t.amounts[0]));
                    if swap.user != a.user.to_bytes() || swap.amm_pool != a.pool.to_bytes()
                        || inputs.is_empty() || outputs.len() != 1
                        || !total.is_some_and(|input| crate::pumpfun_amm::amounts_satisfy(ix, input, outputs[0].amounts[0], &instructions[index + 1..end as usize]))
                    { continue; }
                }
                // Explicit quote-token curve settlement must satisfy the same
                // native argument and typed role contract, including candidates
                // originating from a Jupiter route rather than the native event.
                if ix.program_id().0 == &pumpfun::PROGRAM_ID && !proven_pump_multihop
                    && swap.input_mint != crate::SOL_MINT && swap.output_mint != crate::SOL_MINT {
                    use pumpfun::instructions::PumpFunInstruction::*;
                    let (base, quote, pool, user, source, destination, buy, exact, amount, limit) = match crate::pumpfun::unpack_trade_instruction(ix.data()) {
                        Ok(BuyV2(a)) => (1,2,10,13,15,14,true,false,a.amount,a.max_sol_cost),
                        Ok(BuyExactQuoteInV2(a)) => (1,2,10,13,15,14,true,true,a.spendable_quote_in,a.min_tokens_out),
                        Ok(SellV2(a)) => (1,2,10,13,14,15,false,true,a.amount,a.min_sol_output),
                        Ok(BuyV3(a)) => (1,2,5,8,10,9,true,false,a.amount,a.max_sol_cost),
                        Ok(BuyExactQuoteInV3(a)) => (1,2,5,8,10,9,true,true,a.spendable_quote_in,a.min_tokens_out),
                        Ok(SellV3(a)) => (1,2,5,8,9,10,false,true,a.amount,a.min_sol_output),
                        _ => continue,
                    };
                    if accounts.len() <= destination.max(source).max(user).max(pool)
                        || swap.user != *accounts[user].0 || swap.amm_pool != *accounts[pool].0
                        || swap.input_mint != *accounts[if buy { quote } else { base }].0
                        || swap.output_mint != *accounts[if buy { base } else { quote }].0 { continue; }
                    let inputs: Vec<_> = scoped.iter().filter(|t| t.source == *accounts[source].0 && t.mint == swap.input_mint).collect();
                    let outputs: Vec<_> = scoped.iter().filter(|t| t.destination == *accounts[destination].0 && t.mint == swap.output_mint).collect();
                    let Some(input) = inputs.iter().try_fold(0u64, |n,t| n.checked_add(t.amounts[0])) else { continue };
                    if inputs.is_empty() || outputs.len() != 1
                        || if exact && buy { outputs[0].amounts[0] < limit || input != amount && !crate::pumpfun::exact_quote_budget_satisfies(ix, input, outputs[0].amounts[0], &instructions[index + 1..end as usize]) }
                            else if exact { input != amount || outputs[0].amounts[0] < limit }
                            else { input > limit || outputs[0].amounts[0] < amount }
                    { continue; }
                }
                // Dynamic AMM v1 logs report vault-share conversions, which can
                // differ from the transferred atoms. Its explicit user accounts
                // let us use exact CPI flows instead of rounding the event.
                let user_accounts = if swap.protocol == pb::Protocol::MeteoraAmm {
                    amm::accounts::get_swap_accounts(ix)
                        .ok()
                        .map(|a| (a.user_source_token.to_bytes().to_vec(), a.user_destination_token.to_bytes().to_vec()))
                } else if swap.protocol == pb::Protocol::PumpfunAmm {
                    pumpswap::accounts::TradeTokenAccounts::try_from(ix).ok().and_then(|a| {
                        if swap.input_mint == a.quote_mint.to_bytes() && swap.output_mint == a.base_mint.to_bytes() {
                            Some((a.user_quote_token_account.to_bytes().to_vec(), a.user_base_token_account.to_bytes().to_vec()))
                        } else if swap.input_mint == a.base_mint.to_bytes() && swap.output_mint == a.quote_mint.to_bytes() {
                            Some((a.user_base_token_account.to_bytes().to_vec(), a.user_quote_token_account.to_bytes().to_vec()))
                        } else {
                            None
                        }
                    })
                } else if swap.protocol == pb::Protocol::Pumpfun {
                    use pumpfun::instructions::PumpFunInstruction::*;
                    let (base, quote) = match crate::pumpfun::unpack_trade_instruction(ix.data()) {
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
                } else if swap.protocol == pb::Protocol::JupiterRfq {
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
                            && if matches!(swap.protocol.to_i32(), 4 | 5) {
                                t.amounts.contains(&swap.output_amount)
                            } else {
                                t.amounts.iter().min() == Some(&swap.output_amount)
                            }
                    }) {
                        let identity = (index as u32, input.index, output.index);
                        if expected_transfer.is_some_and(|transfer| transfer != input.index.min(output.index)) {
                            continue;
                        }
                        if !matches!(swap.protocol.to_i32(), 4 | 5) && native_evidence.contains(&identity) {
                            continue;
                        }
                        if matches!(swap.protocol.to_i32(), 4 | 5) {
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
    fn finalized_live_principal_cases_preserve_old_legs_and_reject_unproven_lifecycles() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage/live-principal28");
        let cases = std::fs::read_to_string(dir.join("cases.tsv")).unwrap();
        let preserved = std::fs::read_to_string(dir.join("preserved.tsv")).unwrap();
        let mut checked = HashSet::new();
        for row in cases.lines() {
            let f: Vec<_> = row.split('\t').collect();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(dir.join(format!("{}.pb", f[0]))).unwrap()).unwrap();
            let source: u32 = f[1].parse().unwrap();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let native: Vec<_> = decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();
            if f[10]=="0" { assert!(native.is_empty(),"zero-output source {}:{}",f[0],source); }
            else {
                assert_eq!(native.len(),1,"missing or duplicate {}:{}",f[0],source);
                let s=native[0];
                assert_eq!((s.protocol.to_i32(),s.input_amount,s.output_amount,s.source_transfer_index),
                    (f[2].parse().unwrap(),f[6].parse().unwrap(),f[8].parse().unwrap(),Some(f[9].parse().unwrap())));
                for (value,expected) in [(&s.amm_pool,f[3]),(&s.user,f[4]),(&s.input_mint,f[5]),(&s.output_mint,f[7])] {
                    assert_eq!(base58::encode(value),expected);
                }
                assert!(s.transfer_verified);
                assert!(!decoded.coverage_candidates.iter().any(|c|c.source_index==source));
                let walk:Vec<_>=tx.walk_instructions().collect();
                let layout=crate::native_venues::layout(&walk[source as usize]).unwrap();
                let input:usize=f[9].parse().unwrap();
                let output=walk.iter().enumerate().skip(input+1).find(|(_,c)|c.program_id().0.as_slice()==token::PROGRAM_ID
                    && c.accounts().first().is_some_and(|a|a.0==walk[source as usize].accounts()[layout.vaults[1]].0)).unwrap().0;
                let closed_role=if layout.protocol==pb::Protocol::Ghosty {layout.users[0]} else {layout.users[1]};
                let closed=walk[source as usize].accounts()[closed_role].0.clone();
                for mode in 0..18 {
                    let mut wrong=tx.clone();
                    let keys=wrong.resolved_accounts();
                    let account_index=keys.iter().position(|key|**key==closed).unwrap();
                    let vault_index=keys.iter().position(|key|*key==walk[source as usize].accounts()[layout.vaults[1]].0).unwrap();
                    let mut position=0usize;
                    let message=wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
                    let meta=wrong.meta.as_option_mut().unwrap();
                    for (root,ix) in message.instructions.iter_mut().enumerate() {
                        for (data,a,n) in std::iter::once((&mut ix.data,&mut ix.accounts,position)).chain(
                            meta.inner_instructions.iter_mut().find(|g|g.index==root as u32).into_iter().flat_map(|g|g.instructions.iter_mut())
                                .enumerate().map(|(offset,c)|(&mut c.data,&mut c.accounts,position+offset+1))) {
                            match mode {
                                0 if n==source as usize => data[0]^=1,
                                1 if n==source as usize => {let offset=if layout.protocol==pb::Protocol::Deriverse {16}else{8};
                                    let amount=u64::from_le_bytes(data[offset..offset+8].try_into().unwrap());data[offset..offset+8].copy_from_slice(&(amount+2).to_le_bytes());},
                                2 if n==source as usize => a.swap(layout.pool,layout.authority),
                                3 if n==input => {let amount=u64::from_le_bytes(data[1..9].try_into().unwrap());data[1..9].copy_from_slice(&(amount+1).to_le_bytes());},
                                4 if n==output => {let amount=u64::from_le_bytes(data[1..9].try_into().unwrap());data[1..9].copy_from_slice(&(amount+1).to_le_bytes());},
                                5 if n==output => data[1..9].fill(0),
                                6 if n==output => {let last=a.len()-1;a[last]=a[1];},
                                7 if n==source as usize => {let offset=if layout.protocol==pb::Protocol::Deriverse {24}else{16};
                                    data[offset..offset+8].copy_from_slice(&(s.output_amount+1).to_le_bytes());},
                                8 if data.as_slice()==[9] && a.first()==Some(&(account_index as u8)) => data[0]=5,
                                9 if data.as_slice()==[9] && a.first()==Some(&(account_index as u8)) => a[2]=a[1].wrapping_add(1),
                                10 if data.len()==33 && data[0]==18 && a.first()==Some(&(account_index as u8)) => data[1]^=1,
                                11 if data.len()==52 && data[..4]==[0;4] && a.get(1)==Some(&(account_index as u8)) => data[20]^=1,
                                _=>{},
                            }
                        }
                        position+=1+meta.inner_instructions.iter().find(|g|g.index==root as u32).map_or(0,|g|g.instructions.len());
                    }
                    match mode {
                        12 => meta.pre_balances[account_index]+=1,
                        13 => meta.post_balances[account_index]+=1,
                        14 => meta.log_messages.clear(),
                        15 => {let program=base58::encode(walk[source as usize].program_id().0);
                            let log=meta.log_messages.iter_mut().find(|l|**l==format!("Program {program} success")).unwrap();
                            *log=format!("Program {program} failed: custom program error: 0x1");},
                        16 => {let b=meta.post_token_balances.iter_mut().find(|b|b.account_index as usize==vault_index).unwrap();
                            let amount=b.ui_token_amount.as_option_mut().unwrap();amount.amount=(amount.amount.parse::<u64>().unwrap()+1).to_string();},
                        17 => meta.fee+=1,
                        _=>{},
                    }
                    // Lifecycle mutations apply to the fresh Anchor accounts;
                    // Deriverse instead binds both persistent vault ledgers.
                    if layout.protocol==pb::Protocol::Deriverse && matches!(mode,8..=13|17) {continue;}
                    assert!(crate::process_transaction(wrong).map_or(true,|t|!t.swaps.iter().any(|s|s.source_index==Some(source))),
                        "unproven swap {} mode={mode}",f[0]);
                }
            }
            if checked.insert(f[0]) {
                let expected=preserved.lines().filter(|r|r.split('\t').next()==Some(f[0])).count()
                    +cases.lines().filter(|r|r.split('\t').next()==Some(f[0])&&r.ends_with("\t1")).count();
                assert_eq!(decoded.swaps.len(),expected,"unexpected financial row {}",f[0]);
                for old in preserved.lines().filter(|r|r.split('\t').next()==Some(f[0])) {
                    let v:Vec<_>=old.split('\t').collect();let old_source:u32=v[1].parse().unwrap();
                    let s=decoded.swaps.iter().find(|s|s.source_index==Some(old_source)).unwrap();
                    assert_eq!((s.protocol.to_i32(),s.input_amount,s.output_amount),(v[2].parse().unwrap(),v[5].parse().unwrap(),v[7].parse().unwrap()));
                    assert_eq!((base58::encode(&s.amm_pool),base58::encode(&s.input_mint),base58::encode(&s.output_mint)),(v[3].into(),v[4].into(),v[6].into()));
                }
            }
        }
        assert_eq!(checked.len(),40);
    }
    #[test]
    fn committed_legacy_token_batch_supplies_only_bounded_conservation_entries() {
        let signature = "3NZVd5DcNKVvQbvCt923moutpM7vmmCuCvpMrW8XFTKqNFiB2YcuxnSBUW8cc2rQ44GZbtmc6v4WozzNFYMUVk8Q";
        let bytes = std::fs::read(format!("{}/fixtures/swap-coverage/riptide-native/{signature}.pb", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
        let mints = TokenMintLookup::new(&tx, tx.meta.as_option().unwrap());
        let committed = crate::logs::successful_invocations(&tx);
        let ix = tx.walk_instructions().nth(10).unwrap();
        let ledger = legacy_batch_ledger(&tx, &ix, 10, &mints, &committed).unwrap();
        assert_eq!(ledger.len(), 2);
        assert_eq!((base58::encode(&ledger[0].0), ledger[0].3), ("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(), 1977756));
        assert_eq!((base58::encode(&ledger[1].0), ledger[1].3), ("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".into(), 1979819));
        for (n, entry) in ledger.iter().enumerate() {
            assert_eq!(entry.1, *ix.accounts()[n * 3].0);
            assert_eq!(entry.2, *ix.accounts()[n * 3 + 1].0);
        }
        let result = crate::process_transaction(tx.clone()).unwrap();
        let riptide = result.swaps.iter().find(|s| s.source_index == Some(6)).unwrap();
        assert_eq!((riptide.protocol, riptide.source_transfer_index, riptide.input_amount, riptide.output_amount),
            (pb::Protocol::Riptide.into(), Some(7), 2379629, 1543197));
        assert!(!result.swaps.iter().any(|s| s.source_index == Some(10) || s.source_transfer_index == Some(10)));
        let (g, p) = tx.meta.as_option().unwrap().inner_instructions.iter().enumerate().find_map(|(g, group)|
            group.instructions.iter().position(|c| c.data.first() == Some(&255)).map(|p| (g, p))).unwrap();
        for mode in 0..20 {
            let mut wrong = tx.clone();
            let meta = wrong.meta.as_option_mut().unwrap();
            let calls = &mut meta.inner_instructions[g].instructions;
            let source = calls[p].accounts[0] as u32;
            match mode {
                0 => calls[p].data[1] = 2,
                1 => calls[p].data[2] = 8,
                2 => calls[p].data[3] = 255,
                3 => { calls[p].data.pop(); }
                4 => calls[p].data.push(0),
                5 => { calls[p].accounts.pop(); }
                6 => { let extra = calls[p].accounts[0]; calls[p].accounts.push(extra); }
                7 => calls[p].data = vec![255],
                8 => calls[p].program_id_index = tx.resolved_accounts().iter().position(|a| a.as_slice() == token_2022::PROGRAM_ID).unwrap() as u32,
                9 | 10 => {
                    for b in meta.pre_token_balances.iter_mut().chain(&mut meta.post_token_balances).filter(|b| b.account_index == source) {
                        if mode == 9 { b.mint = base58::encode(crate::SOL_MINT); }
                        else { b.program_id = base58::encode(token_2022::PROGRAM_ID); }
                    }
                }
                11 => calls[p].accounts[0] = tx.meta.as_option().unwrap().inner_instructions[g].instructions[p - 2].accounts[0],
                12 => meta.err = buffa::MessageField::some(Default::default()),
                13 => {},
                14 => { meta.log_messages.truncate(32); meta.log_messages.push("Log truncated".into()); }
                15 => meta.log_messages[38] = "Program FLiNTXPwppyoJabCoxc2uiiRygAHpmMXajiDXo2Ub1z failed: custom program error: 1".into(),
                16 => meta.log_messages[31] = format!("Program {} invoke [3]", base58::encode(token_2022::PROGRAM_ID)),
                17 => calls[p].data[12] = 4,
                18 => calls[p].data[14] = 9,
                _ => { meta.pre_token_balances.retain(|b| b.account_index != source); meta.post_token_balances.retain(|b| b.account_index != source); }
            }
            let mut successful = crate::logs::successful_invocations(&wrong);
            if mode == 13 { successful.remove(&10); }
            let mints = TokenMintLookup::new(&wrong, wrong.meta.as_option().unwrap());
            let ix = wrong.walk_instructions().nth(10).unwrap();
            assert!(legacy_batch_ledger(&wrong, &ix, 10, &mints, &successful).is_none(), "invalid batch mode {mode}");
        }
    }
    #[test]
    fn native_pump_current_formats_have_exact_finalized_proceeds() {
        let cases: &[(&[u8], u32, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/0.transaction.pb"), 6, 4653976),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/1.transaction.pb"), 6, 1157095),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/2.transaction.pb"), 6, 2125365),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/3.transaction.pb"), 13, 745093291),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/4.transaction.pb"), 11, 856322133),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/5.transaction.pb"), 3, 82566168),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/6.transaction.pb"), 2, 963586),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/7.transaction.pb"), 3, 21939681),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/8.transaction.pb"), 13, 4580970),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/9.transaction.pb"), 3, 311620397),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/10.transaction.pb"), 3, 23357402),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/11.transaction.pb"), 9, 15948633),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/12.transaction.pb"), 17, 5479039),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/13.transaction.pb"), 6, 79905684),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/14.transaction.pb"), 16, 32092021),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/15.transaction.pb"), 6, 23500479),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/16.transaction.pb"), 12, 2602311),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/17.transaction.pb"), 11, 890323),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/18.transaction.pb"), 13, 13410842),
        ];
        for (case, &(bytes, source, net)) in cases.iter().enumerate() {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let call = tx.walk_instructions().nth(source as usize).unwrap();
            let a = call.accounts();
            let legacy = matches!(crate::pumpfun::unpack_trade_instruction(call.data()), Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)));
            let pool = tx.resolved_accounts().iter().position(|key| *key == a[if legacy { 3 } else { 10 }].0).unwrap();
            let call_data = call.data().clone();
            for mode in 0..6 {
                let mut altered = tx.clone();
                if mode == 1 { altered.meta.as_option_mut().unwrap().post_balances[pool] += 1; }
                if mode == 2 || mode == 3 {
                    let event = altered.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                        .find(|i| i.data.starts_with(&[228, 69, 165, 46, 81, 203, 154, 29, 189, 219, 127, 211, 78, 230, 97, 238])).unwrap();
                    let offset = if mode == 2 { 48 } else { 177 };
                    let old = u64::from_le_bytes(event.data[offset..offset + 8].try_into().unwrap());
                    event.data[offset..offset + 8].copy_from_slice(&(old + 1).to_le_bytes());
                }
                if mode == 4 || mode == 5 {
                    let msg = altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
                    if let Some(i) = msg.instructions.iter_mut().find(|i| i.data == call_data) {
                        if mode == 4 { i.accounts.swap(if legacy { 1 } else { 6 }, if legacy { 8 } else { 16 }); }
                        else { let n = u64::from_le_bytes(i.data[8..16].try_into().unwrap()); i.data[8..16].copy_from_slice(&(n + 1).to_le_bytes()); }
                    } else {
                        let i = altered.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                            .find(|i| i.data == call_data).unwrap();
                        if mode == 4 { i.accounts.swap(if legacy { 1 } else { 6 }, if legacy { 8 } else { 16 }); }
                        else { let n = u64::from_le_bytes(i.data[8..16].try_into().unwrap()); i.data[8..16].copy_from_slice(&(n + 1).to_le_bytes()); }
                    }
                }
                let mut pending = None;
                let mut swaps: Vec<_> = altered.walk_instructions().enumerate().filter_map(|(index, ix)| crate::pumpfun::handle_instruction(&mut pending, &ix, index as u32)).collect();
                let mints = TokenMintLookup::new(&altered, altered.meta.as_option().unwrap());
                verify_with_fees(&altered, &mints, &mut swaps, true, |_, _| None);
                let actual: Vec<_> = swaps.iter().filter(|s| s.source_index == Some(source)).collect();
                if mode == 0 {
                    assert_eq!(actual.len(), 1, "finalized native case {case}");
                    assert_eq!(actual[0].source_transfer_index, Some(source + if case < 3 { 3 } else { 2 }));
                    assert_eq!(actual[0].output_amount, net, "exact proceeds case {case}");
                    assert!(actual[0].transfer_verified);
                } else { assert!(actual.is_empty(), "invalid native witness case {case} mode {mode}"); }
            }
        }
    }

    #[test]
    fn native_pump_cashback_requires_exact_volume_credit() {
        let cases: &[(&[u8], u32, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/5.transaction.pb"), 3, 82566168),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/6.transaction.pb"), 2, 963586),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/7.transaction.pb"), 3, 21939681),
            (include_bytes!("../fixtures/swap-coverage/pump-native-cashback/10.transaction.pb"), 3, 23357402),
        ];
        for &(bytes, source, net) in cases {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let ix = tx.walk_instructions().nth(source as usize).unwrap();
            let accounts = ix.accounts();
            let accumulator = tx.resolved_accounts().iter().position(|a| *a == accounts[19].0).unwrap();
            for mode in 0..6 {
                let mut altered = tx.clone();
                if mode == 1 { altered.meta.as_option_mut().unwrap().post_balances[accumulator] += 1; }
                if mode == 2 {
                    let meta = altered.meta.as_option_mut().unwrap();
                    let event = meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                        .find(|i| i.data.starts_with(&[228, 69, 165, 46, 81, 203, 154, 29, 189, 219, 127, 211, 78, 230, 97, 238])).unwrap();
                    event.data[283] ^= 1; // canonical cashback amount
                }
                if mode == 3 {
                    let logs = &mut altered.meta.as_option_mut().unwrap().log_messages;
                    let i = logs.iter().position(|l| l.starts_with("Program return: pfee")).unwrap();
                    logs.insert(i, logs[i].clone());
                }
                if mode == 4 {
                    let logs = &mut altered.meta.as_option_mut().unwrap().log_messages;
                    let i = logs.iter().position(|l| l.starts_with("Program return: pfee")).unwrap();
                    logs[i] = "Log truncated".into();
                }
                if mode == 5 {
                    let msg = altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
                    if let Some(call) = msg.instructions.iter_mut().find(|i| i.data.starts_with(&[93, 246, 130, 60, 231, 233, 64, 178])) {
                        call.accounts.swap(19, 16);
                    } else {
                        let meta = altered.meta.as_option_mut().unwrap();
                        let call = meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                            .find(|i| i.data.starts_with(&[93, 246, 130, 60, 231, 233, 64, 178])).unwrap();
                        call.accounts.swap(19, 16);
                    }
                }
                let mut pending = None;
                let mut swaps: Vec<_> = altered.walk_instructions().enumerate().filter_map(|(index, ix)| crate::pumpfun::handle_instruction(&mut pending, &ix, index as u32)).collect();
                let mints = TokenMintLookup::new(&altered, altered.meta.as_option().unwrap());
                verify_with_fees(&altered, &mints, &mut swaps, true, |_, _| None);
                let pump: Vec<_> = swaps.iter().filter(|s| s.protocol == pb::Protocol::Pumpfun).collect();
                if mode == 0 {
                    assert_eq!(pump.len(), 1, "native cashback source {source}");
                    assert_eq!(pump[0].source_index, Some(source));
                    assert_eq!(pump[0].output_amount, net);
                    assert!(pump[0].transfer_verified);
                } else { assert!(pump.is_empty(), "invalid cashback witness mode {mode}"); }
            }
        }
    }

    #[test]
    fn repeated_native_pump_sells_require_independent_per_leg_proceeds() {
        let cases: &[(&[u8], &[u64])] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/0.transaction.pb"), &[266827905, 264185740, 367344813]),
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/1.transaction.pb"), &[401979300, 407887179, 571690696]),
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/2.transaction.pb"), &[330365453, 368830768, 362135137]),
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/3.transaction.pb"), &[407032786, 626447093]),
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/4.transaction.pb"), &[467404344, 503834393]),
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/5.transaction.pb"), &[433468504, 427114519, 723389266]),
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/6.transaction.pb"), &[367657763, 387430851, 399359232]),
            (include_bytes!("../fixtures/swap-coverage/pump-repeated-native/7.transaction.pb"), &[336219813, 361657330, 381427368]),
        ];
        for &(bytes, expected) in cases {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            assert_eq!(crate::pump_curve::fee_returns(&tx).len(), expected.len(), "fee return runtime binding");
            let calls: Vec<_> = tx.walk_instructions().enumerate().filter(|(_, ix)| ix.program_id().0 == &pumpfun::PROGRAM_ID
                && matches!(crate::pumpfun::unpack_trade_instruction(ix.data()), Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)))).map(|(i, _)| i as u32).collect();
            assert_eq!(calls.len(), expected.len());
            for mode in 0..12 {
                let mut altered = tx.clone();
                if mode == 10 {
                    let ix = tx.walk_instructions().nth(calls[0] as usize).unwrap();
                    let accounts = ix.accounts();
                    let resolved = tx.resolved_accounts();
                    let vault = resolved.iter().position(|a| *a == accounts[4].0).unwrap();
                    let pool = resolved.iter().position(|a| *a == accounts[3].0).unwrap();
                    let meta = altered.meta.as_option_mut().unwrap();
                    // Prior donations increase physical balances without
                    // changing the native curve's virtual reserves. They must
                    // neither invent a migration offset nor change the prices.
                    for balance in meta.pre_token_balances.iter_mut().chain(&mut meta.post_token_balances)
                        .filter(|b| b.account_index as usize == vault) {
                        let amount = balance.ui_token_amount.as_option_mut().unwrap();
                        amount.amount = (amount.amount.parse::<u64>().unwrap() + 1_000_000_000).to_string();
                    }
                    meta.pre_balances[pool] += 1_000_000;
                    meta.post_balances[pool] += 1_000_000;
                } else if mode == 1 || mode == 9 {
                    let meta = altered.meta.as_option_mut().unwrap();
                    let event = meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                        .find(|ix| ix.data.get(8..16) == Some(pumpfun::events::TRADE.as_slice())).unwrap();
                    if mode == 1 { event.data[105..113].fill(0); }
                    else {
                        let input = u64::from_le_bytes(event.data[56..64].try_into().unwrap());
                        event.data[105..113].copy_from_slice(&1u64.to_le_bytes());
                        event.data[113..121].copy_from_slice(&(input + 1).to_le_bytes());
                    }
                } else if mode == 2 || mode == 3 {
                    let meta = altered.meta.as_option_mut().unwrap();
                    let events: Vec<_> = meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                        .filter(|ix| ix.data.get(8..16) == Some(pumpfun::events::TRADE.as_slice())).collect();
                    // Redistribute one lamport between committed event claims.
                    // Aggregate user/pool/fee credits remain unchanged.
                    for (i, event) in events.into_iter().take(2).enumerate() {
                        for offset in if mode == 2 { vec![48, event.data.len() - 48] } else { vec![177] } {
                            let original = u64::from_le_bytes(event.data[offset..offset + 8].try_into().unwrap());
                            let value = if i == 0 { original + 1 } else { original - 1 };
                            event.data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
                        }
                    }
                } else if mode == 4 {
                    let meta = altered.meta.as_option_mut().unwrap();
                    let ix = meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                        .find(|ix| ix.data.starts_with(&[154, 237, 138, 92, 162, 2, 162, 187])).unwrap();
                    ix.data[8] = 0; // native curve flag is part of the typed CPI
                } else if mode == 5 {
                    let logs = &mut altered.meta.as_option_mut().unwrap().log_messages;
                    let i = logs.iter().position(|l| l.starts_with("Program return: pfee")).unwrap();
                    logs.insert(i, logs[i].clone());
                } else if mode == 6 {
                    let logs = &mut altered.meta.as_option_mut().unwrap().log_messages;
                    let i = logs.iter().position(|l| l.starts_with("Program return: pfee")).unwrap();
                    logs[i].push('A');
                } else if mode == 7 {
                    let meta = altered.meta.as_option_mut().unwrap();
                    let ix = meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                        .find(|ix| ix.data.starts_with(&[154, 237, 138, 92, 162, 2, 162, 187])).unwrap();
                    ix.accounts.swap(0, 1);
                } else if mode == 8 {
                    let logs = &mut altered.meta.as_option_mut().unwrap().log_messages;
                    let i = logs.iter().position(|l| l.starts_with("Program return: pfee")).unwrap();
                    logs[i] = "Log truncated".into();
                }
                let mut pending = None;
                let mut swaps: Vec<_> = altered.walk_instructions().enumerate().filter_map(|(index, ix)| crate::pumpfun::handle_instruction(&mut pending, &ix, index as u32)).collect();
                let mints = TokenMintLookup::new(&altered, altered.meta.as_option().unwrap());
                verify_with_fees(&altered, &mints, &mut swaps, true,
                    |_, _| (mode != 11).then_some(crate::token_fees::TransferFee { basis_points: 0, maximum_fee: 0 }));
                let pump: Vec<_> = swaps.iter().filter(|s| s.protocol == pb::Protocol::Pumpfun).collect();
                if mode == 0 || mode == 10 || mode == 11 {
                    assert_eq!(pump.len(), expected.len(), "all independently settled original legs");
                    for ((s, net), source) in pump.iter().zip(expected).zip(&calls) {
                        assert_eq!(s.source_index, Some(*source));
                        assert_eq!(s.source_transfer_index, Some(source + 2));
                        assert_eq!(s.output_amount, *net);
                        assert!(s.transfer_verified);
                    }
                } else {
                    assert!(pump.is_empty(), "missing/redistributed/invalid independent witness mode {mode}");
                }
            }
        }
    }

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
            assert_eq!(leg.protocol, pb::Protocol::Pumpfun);
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
        let original = &tx.meta.as_option().unwrap().inner_instructions.iter().find(|g| g.index == 1).unwrap().instructions;
        let native = original[1].accounts.clone();
        let input = original[4].accounts.clone();
        for mode in 0..33 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_option_mut().unwrap();
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
                    let amount = balance.ui_token_amount.as_option_mut().unwrap();
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
        assert_eq!((first.protocol, first.source_index, first.source_transfer_index), (pb::Protocol::Pumpfun.into(), Some(9), Some(10)));
        assert_eq!((first.input_amount, first.output_amount), (31_238_843, 896_182_574_050));
        assert_eq!(first.input_mint, crate::SOL_MINT);
        assert_eq!((decoded.swaps[1].input_amount, decoded.swaps[1].output_amount), (896_182_574_050, 3_898_557_180_441));
        assert_eq!(decoded.swaps[1].source_transfer_index, Some(14));
        assert!(!decoded.coverage_candidates.iter().any(|c| c.reason == "multihop_without_all_verified_pools"));
        for mode in 0..17 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_option_mut().unwrap();
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
    fn finalized_native_sells_with_seeded_wsol_funding_preserve_exact_proceeds() {
        let fixtures = [
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/0.transaction.pb").as_slice(), 3674892520122u64, 146101095u64),
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/1.transaction.pb").as_slice(), 27746164395248u64, 1073914637u64),
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/2.transaction.pb").as_slice(), 40288505198799u64, 1451114076u64),
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/3.transaction.pb").as_slice(), 46813406508987u64, 1684902330u64),
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/4.transaction.pb").as_slice(), 35392381708333u64, 1361483540u64),
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/5.transaction.pb").as_slice(), 28399267367389u64, 1100046313u64),
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/6.transaction.pb").as_slice(), 16019211731366u64, 627855680u64),
            (include_bytes!("../fixtures/swap-coverage/pump-seeded-wsol/7.transaction.pb").as_slice(), 35696936606446u64, 1375983428u64),
        ];
        for (bytes, input, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let native = tx.walk_instructions().nth(2).unwrap();
            let a = native.accounts();
            let (pool, user, mint) = (a[3].0.clone(), a[6].0.clone(), a[2].0.clone());
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let sell = decoded.swaps.iter().find(|s| s.source_index == Some(2)).expect("original finalized sell proceeds");
            assert_eq!((sell.input_amount, sell.output_amount, sell.source_transfer_index), (input, output, Some(4)));
            assert_eq!((sell.amm_pool.clone(), sell.user.clone(), sell.input_mint.clone()), (pool, user, mint));
            assert_eq!(sell.output_mint, crate::SOL_MINT);
            assert!(sell.transfer_verified);
            for mode in 0..11 {
                let mut altered = tx.clone();
                let message = altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
                let pump_program = message.instructions[2].program_id_index;
                let create = message.instructions.iter_mut().find(|ix| ix.data.starts_with(&[3, 0, 0, 0])).unwrap();
                let seller = create.accounts[0] as usize;
                let meta = altered.meta.as_option_mut().unwrap();
                match mode {
                    0 => create.data[52] ^= 1,
                    1 => { create.data.pop(); }
                    2 => create.data[36..44].copy_from_slice(&u64::MAX.to_le_bytes()),
                    3 => create.data[36..44].copy_from_slice(&33u64.to_le_bytes()),
                    4 => create.data[36..44].copy_from_slice(&0u64.to_le_bytes()),
                    5 => create.data[4] ^= 1,
                    6 => create.accounts[0] = create.accounts[1],
                    7 => create.accounts[1] = create.accounts[0],
                    8 => create.program_id_index = pump_program,
                    9 => meta.post_balances[seller] += 1,
                    10 => {
                        let success = meta.log_messages.iter_mut().find(|log| log.as_str() == "Program 11111111111111111111111111111111 success").unwrap();
                        *success = "Program 11111111111111111111111111111111 failed: custom program error: 0x1".into();
                    }
                    _ => unreachable!(),
                }
                assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|s| s.source_index != Some(2))),
                    "seeded funding mutation {mode}");
            }
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
            let meta = altered.meta.as_option_mut().unwrap();
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
                    let amount = balances.iter_mut().find(|balance| balance.account_index == index).unwrap().ui_token_amount.as_option_mut().unwrap();
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
            let meta = altered.meta.as_option_mut().unwrap();
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
            let message = altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
            let meta = altered.meta.as_option_mut().unwrap();
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
            let meta = altered.meta.as_option_mut().unwrap();
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
                    let balance = meta.post_token_balances.iter_mut().find(|b| b.account_index == u32::from(account)).unwrap().ui_token_amount.as_option_mut().unwrap();
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
            let meta = altered.meta.as_option_mut().unwrap();
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
        let calls = &mut opaque.meta.as_option_mut().unwrap().inner_instructions.iter_mut().find(|g| g.index == 3).unwrap().instructions;
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
                .as_option()
                .unwrap()
                .pre_token_balances
                .iter()
                .find(|balance| balance.account_index == position)
                .unwrap()
                .clone();
            assert_eq!(start.ui_token_amount.as_option().unwrap().amount, "0");
            let decode = |transaction| crate::process_transaction(transaction).unwrap();
            let decoded = decode(tx.clone());
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(8)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!(swaps[0].protocol, pb::Protocol::MeteoraDbc);
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
                .as_option_mut()
                .unwrap()
                .pre_token_balances
                .iter_mut()
                .find(|balance| balance.account_index == position)
                .unwrap()
                .ui_token_amount
                .as_option_mut()
                .unwrap()
                .amount = "100".into();
            for group in &mut nonzero_start.meta.as_option_mut().unwrap().inner_instructions {
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
                        .as_option_mut()
                        .unwrap()
                        .pre_token_balances
                        .retain(|balance| balance.account_index != position),
                    1 => altered.meta.as_option_mut().unwrap().post_token_balances.push(start.clone()),
                    2 => {
                        altered
                            .transaction
                            .as_option_mut()
                            .unwrap()
                            .message
                            .as_option_mut()
                            .unwrap()
                            .instructions
                            .iter_mut()
                            .find(|ix| ix.data.as_slice() == [9] && ix.accounts.first() == Some(&(position as u8)))
                            .unwrap()
                            .data[0] = 5
                    }
                    3 => {
                        let message = altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
                        let close = message
                            .instructions
                            .iter()
                            .find(|ix| ix.data.as_slice() == [9] && ix.accounts.first() == Some(&(position as u8)))
                            .unwrap()
                            .clone();
                        message.instructions.push(close);
                    }
                    4 => {
                        for group in &mut altered.meta.as_option_mut().unwrap().inner_instructions {
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
                            .as_option_mut()
                            .unwrap()
                            .pre_token_balances
                            .iter_mut()
                            .find(|balance| balance.account_index == position)
                            .unwrap()
                            .ui_token_amount
                            .as_option_mut()
                            .unwrap()
                            .amount = "100".into()
                    }
                    6 => {
                        let close = altered
                            .transaction
                            .as_option_mut()
                            .unwrap()
                            .message
                            .as_option_mut()
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
            let message = cpi_close.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
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
            let meta = cpi_close.meta.as_option_mut().unwrap();
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
                let logs = &mut altered.meta.as_option_mut().unwrap().log_messages;
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
        let swap = expected.swaps.iter().find(|swap| swap.protocol == pb::Protocol::Pumpfun).unwrap();
        assert_eq!(swap.output_amount, 47_690_764);
        assert_eq!(swap.source_transfer_index, Some(7));
        let ix = tx
            .walk_instructions()
            .find(|ix| {
                matches!(
                    crate::pumpfun::unpack_trade_instruction(ix.data()),
                    Ok(pumpfun::instructions::PumpFunInstruction::SellV3(_))
                )
            })
            .unwrap();
        let fee_recipient = ix.accounts()[13].0.clone();
        let pool = swap.amm_pool.clone();
        for account in [fee_recipient, pool] {
            let mut changed = tx.clone();
            let position = changed.resolved_accounts().iter().position(|key| **key == account).unwrap();
            changed.meta.as_option_mut().unwrap().post_balances[position] += 1;
            let decoded = crate::process_transaction(changed).unwrap();
            assert!(decoded.swaps.iter().all(|swap| swap.protocol != pb::Protocol::Pumpfun));
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
            let mints = TokenMintLookup::new(&tx, tx.meta.as_option().unwrap());
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
        let mints = TokenMintLookup::new(&tx, tx.meta.as_option().unwrap());
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
        let mints = TokenMintLookup::new(&tx, tx.meta.as_option().unwrap());
        let native = crate::process_transaction(tx.clone()).unwrap().swaps[2].clone();
        let mut wrapper = native.clone();
        wrapper.protocol = pb::Protocol::JupiterV6.into();
        let mut swaps = vec![wrapper, native];
        let rejected = verify_and_deduplicate(&tx, &mints, &mut swaps, false);
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].protocol, pb::Protocol::OrcaWhirlpool);
        assert_eq!(rejected[0].verification_failure, "duplicate_native_cpi");
    }
}
