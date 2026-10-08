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

pub(crate) fn verify_and_deduplicate(tx: &ConfirmedTransaction, mints: &TokenMintLookup, swaps: &mut Vec<pb::Swap>) -> Vec<pb::Swap> {
    let instructions: Vec<_> = tx.walk_instructions().collect();
    let mut transfers = Vec::new();
    let mut other_token_operations = HashSet::new();
    for (index, ix) in instructions.iter().enumerate() {
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
        if parsed.is_none() && !matches!(data.first(), Some(1 | 16 | 18 | 21 | 22)) && (program.0 == &token::PROGRAM_ID || program.0 == &token_2022::PROGRAM_ID)
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
                    if fee >= amount { continue; }
                    if let Some(net) = amount.checked_sub(fee).filter(|net| *net > 0) {
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

    // One incoming transfer plus known outgoing debits proves the exact
    // receipt, even when a router forwards tokens or a pool pays a fee later.
    // Mint/burn/close and other operations make this evidence unavailable.
    let mut receipts = HashMap::<Vec<u8>, usize>::new();
    let mut debits = HashMap::<Vec<u8>, i128>::new();
    for transfer in &transfers {
        *receipts.entry(transfer.destination.clone()).or_default() += 1;
        *debits.entry(transfer.source.clone()).or_default() += i128::from(transfer.amounts[0]);
    }
    let mut deltas = HashMap::<Vec<u8>, i128>::new();
    let accounts = tx.resolved_accounts();
    if let Some(meta) = &tx.meta {
        for (balances, sign) in [(&meta.pre_token_balances, -1i128), (&meta.post_token_balances, 1)] {
            for balance in balances {
                if let (Some(account), Some(amount)) = (accounts.get(balance.account_index as usize), balance.ui_token_amount.as_ref()) {
                    if let Ok(value) = amount.amount.parse::<u64>() {
                        *deltas.entry((*account).clone()).or_default() += sign * i128::from(value);
                    }
                }
            }
        }
    }
    for transfer in &mut transfers {
        let account = &transfer.destination;
        if transfer.fee_token && transfer.mint != crate::SOL_MINT && receipts.get(account) == Some(&1) && !other_token_operations.contains(account) {
            let net = deltas
                .get(account)
                .and_then(|delta| u64::try_from(delta + debits.get(account).unwrap_or(&0)).ok())
                .filter(|net| *net > 0 && *net <= transfer.amounts[0]);
            if let Some(net) = net {
                transfer.net_verified = true;
                if !transfer.amounts.contains(&net) {
                    transfer.amounts.push(net);
                }
            }
        }
    }

    // New native venues are normalized from recognized instruction layouts
    // and exact, immediate token CPIs. Never synthesize a trade from arbitrary
    // same-program transfers (deposit, withdraw, lending, quote update).
    for (index, ix) in instructions.iter().enumerate() {
        let Some(layout) = crate::native_venues::layout(ix) else { continue; };
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        if depth == 0 { continue; }
        let end = instructions.iter().enumerate().skip(index + 1)
            .find(|(_, child)| child.is_root() || (child.stack_height() > 0 && child.stack_height() <= depth))
            .map(|(end, _)| end as u32).unwrap_or(instructions.len() as u32);
        let accounts = ix.accounts();
        let pool = accounts[layout.pool].0;
        let user = accounts[layout.authority].0;
        let scoped: Vec<_> = transfers.iter().filter(|t| t.index > index as u32 && t.index < end && t.depth == depth + 1).collect();
        let mut candidates = Vec::new();
        for reverse in [false, true] {
            if reverse && layout.input_first { continue; }
            let a = usize::from(reverse);
            let b = 1 - a;
            let source = accounts[layout.users[a]].0;
            let destination = accounts[layout.users[b]].0;
            let vault_in = accounts[layout.vaults[a]].0;
            let vault_out = accounts[layout.vaults[b]].0;
            if source == destination || source == vault_in || destination == vault_out || vault_in == vault_out { continue; }
            let inputs: Vec<_> = scoped.iter().filter(|t| &t.source == source && &t.destination == vault_in && &t.authority == user).collect();
            let outputs: Vec<_> = scoped.iter().filter(|t| &t.source == vault_out && &t.destination == destination).collect();
            if inputs.len() != 1 || outputs.len() != 1 { continue; }
            let (input, output) = (inputs[0], outputs[0]);
            if input.mint == output.mint || mints.mint_for(vault_in).as_ref() != Some(&input.mint)
                || mints.mint_for(vault_out).as_ref() != Some(&output.mint)
                || mints.mint_for(source).as_ref() != Some(&input.mint)
                || mints.mint_for(destination).as_ref() != Some(&output.mint)
                || (layout.exact_input && !layout.input_limit && input.amounts[0] != layout.amount)
                || ((!layout.exact_input || layout.input_limit) && input.amounts[0] > layout.amount)
                || (output.fee_token && output.mint != crate::SOL_MINT && !output.net_verified) { continue; }
            candidates.push(pb::Swap {
                protocol: layout.protocol as i32,
                program_id: ix.program_id().0.clone(),
                stack_height: depth,
                amm: ix.program_id().0.clone(),
                amm_pool: pool.clone(),
                user: user.clone(),
                input_mint: input.mint.clone(),
                input_amount: input.amounts[0],
                output_mint: output.mint.clone(),
                output_amount: *output.amounts.iter().min().unwrap(),
                source_index: Some(index as u32),
                source_transfer_index: Some(input.index.min(output.index)),
                ..Default::default()
            });
        }
        if candidates.len() == 1 { swaps.push(candidates.pop().unwrap()); }
    }

    let mut rejected = Vec::new();
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
                if expected_source.is_some_and(|source| source != index as u32) { continue; }
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
                    && matches!(pumpfun::instructions::unpack(ix.data()), Ok(pumpfun::instructions::PumpFunInstruction::Sell(_)))
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
                    if calls == 1 && !explicit_sol && pool_delta == Some(i128::from(swap.output_amount)) {
                        if let Some(input) = scoped.iter().find(|t| t.mint == swap.input_mint && t.amounts.contains(&swap.input_amount)) {
                            let identity = (index as u32, input.index, index as u32);
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
                    pumpswap::accounts::TradeAccounts::try_from(ix).ok().and_then(|a| {
                        if swap.input_mint == a.quote_mint.to_bytes() && swap.output_mint == a.base_mint.to_bytes() {
                            Some((a.user_quote_token_account.to_bytes().to_vec(), a.user_base_token_account.to_bytes().to_vec()))
                        } else if swap.input_mint == a.base_mint.to_bytes() && swap.output_mint == a.quote_mint.to_bytes() {
                            Some((a.user_base_token_account.to_bytes().to_vec(), a.user_quote_token_account.to_bytes().to_vec()))
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };
                let corrected = user_accounts.and_then(|(source, destination)| {
                    let inputs: Vec<_> = scoped.iter().filter(|t| t.source == source && t.mint == swap.input_mint).collect();
                    let outputs: Vec<_> = scoped.iter().filter(|t| t.destination == destination && t.mint == swap.output_mint).collect();
                    if inputs.is_empty() || outputs.len() != 1 {
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
                    if expected_transfer.is_some_and(|transfer| transfer != input_index.min(output_index)) { continue; }
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
                    for output in scoped.iter().filter(|t| t.mint == swap.output_mint && t.amounts.contains(&swap.output_amount)) {
                        let identity = (index as u32, input.index, output.index);
                        if expected_transfer.is_some_and(|transfer| transfer != input.index.min(output.index)) { continue; }
                        if !matches!(swap.protocol, 4 | 5) && native_evidence.contains(&identity) {
                            continue;
                        }
                        evidence = Some(identity);
                        break 'candidate;
                    }
                }
            }
        }
        match evidence {
            Some(identity) if matches!(swap.protocol, 4 | 5) && native_evidence.contains(&identity) => {
                swap.source_index = Some(identity.0);
                swap.source_transfer_index = Some(identity.1.min(identity.2));
                swap.verification_failure = "duplicate_native_cpi".into();
                rejected.push(swap);
            }
            Some(identity) => {
                swap.source_index = Some(identity.0);
                swap.source_transfer_index = Some(identity.1.min(identity.2));
                swap.transfer_verified = true;
                native_evidence.insert(identity);
                verified.push(swap);
            }
            None => {
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
            assert_eq!(verify_and_deduplicate(&tx, &mints, &mut swaps).len(), 1);
            assert!(swaps.is_empty());
        }
    }

    #[test]
    fn public_aqua_route_has_exact_verified_and_ordered_transfers() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/aqua-orca-multihop.transaction.pb").to_vec()).unwrap();
        let decoded = crate::process_transaction(tx).unwrap();
        assert_eq!(decoded.swaps.len(), 3);
        assert!(decoded.rejected_swaps.is_empty());
        assert!(decoded.swaps.iter().all(|swap| swap.transfer_verified));
        assert!(decoded.swaps.windows(2).all(|swaps| swaps[0].source_index < swaps[1].source_index));
    }

    #[test]
    fn changed_pool_mint_or_amount_never_becomes_a_verified_swap() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/aqua-orca-multihop.transaction.pb").to_vec()).unwrap();
        let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        for field in 0..3 {
            let mut swap = decoded.swaps[2].clone();
            match field {
                0 => swap.amm_pool = vec![9; 32],
                1 => swap.output_mint = vec![8; 32],
                _ => swap.output_amount += 1,
            }
            let mut swaps = vec![swap];
            let rejected = verify_and_deduplicate(&tx, &mints, &mut swaps);
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
        let rejected = verify_and_deduplicate(&tx, &mints, &mut swaps);
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].protocol, pb::Protocol::OrcaWhirlpool as i32);
        assert_eq!(rejected[0].verification_failure, "duplicate_native_cpi");
    }
}
