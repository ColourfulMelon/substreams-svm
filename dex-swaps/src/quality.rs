//! Bind normalized swaps to successful on-chain instruction/transfer evidence.
//! Amount similarity is never enough: both exact token legs and the AMM/pool
//! invocation must match. Unverifiable rows are retained as diagnostics.
use std::collections::{HashMap, HashSet};

use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::{
    meteora::amm,
    pumpswap,
    spl::{token, token_2022},
};

use crate::token_mints::TokenMintLookup;

struct Transfer {
    index: u32,
    mint: Vec<u8>,
    amounts: Vec<u64>,
    source: Vec<u8>,
    destination: Vec<u8>,
    fee_token: bool,
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
        if parsed.is_none() && (program.0 == &token::PROGRAM_ID || program.0 == &token_2022::PROGRAM_ID) {
            for account in &accounts {
                other_token_operations.insert(account.0.clone());
            }
        }
        if let Some((mint, offset, source, destination)) = parsed {
            let amount = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            if amount > 0 {
                transfers.push(Transfer {
                    index: index as u32,
                    mint,
                    amounts: vec![amount],
                    source: accounts[source].0.clone(),
                    destination: accounts[destination].0.clone(),
                    fee_token: program.0 == &token_2022::PROGRAM_ID,
                });
            }
        }
    }

    // A single transfer touching an account has an exact independently observed
    // net balance delta. This supports withheld Token-2022 fees without tolerances
    // or guessing a percentage. Multi-transfer accounts provide no such evidence.
    let mut touches = HashMap::<Vec<u8>, usize>::new();
    for transfer in &transfers {
        *touches.entry(transfer.source.clone()).or_default() += 1;
        *touches.entry(transfer.destination.clone()).or_default() += 1;
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
        for (account, sign) in [(&transfer.source, -1i128), (&transfer.destination, 1)] {
            if transfer.fee_token && touches.get(account) == Some(&1) && !other_token_operations.contains(account) {
                if let Some(delta) = deltas
                    .get(account)
                    .and_then(|delta| u64::try_from(delta * sign).ok())
                    .filter(|delta| *delta > 0)
                {
                    if !transfer.amounts.contains(&delta) {
                        transfer.amounts.push(delta);
                    }
                }
            }
        }
    }

    let mut rejected = Vec::new();
    let mut verified = Vec::new();
    let mut native_evidence = HashSet::new();
    // Prefer a native event over its Jupiter wrapper, including when wrapper
    // amounts reflect a transfer fee. Never deduplicate by amount alone.
    let mut pending = std::mem::take(swaps);
    pending.sort_by_key(|swap| matches!(swap.protocol, 4 | 5));
    for mut swap in pending {
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
                    // A fee token's single-receipt balance delta proves the net
                    // received amount. Do not infer it on multi-transfer accounts.
                    let amount = if outputs[0].fee_token && touches.get(&destination) == Some(&1) && !other_token_operations.contains(&destination) {
                        deltas
                            .get(&destination)
                            .and_then(|d| u64::try_from(*d).ok())
                            .filter(|d| *d > 0)
                            .unwrap_or(outputs[0].amounts[0])
                    } else {
                        outputs[0].amounts[0]
                    };
                    Some((total, amount, inputs[0].index, outputs[0].index))
                });
                if let Some((input, output, input_index, output_index)) = corrected {
                    let identity = (index as u32, input_index, output_index);
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
