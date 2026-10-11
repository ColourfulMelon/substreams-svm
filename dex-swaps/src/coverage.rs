//! Retain unclassified token-exchange invocations without publishing a price.
use std::collections::BTreeSet;

use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
use substreams_solana_idls::spl::{token, token_2022};

use crate::token_mints::TokenMintLookup;

pub(crate) fn candidates(tx: &ConfirmedTransaction, mints: &TokenMintLookup, swaps: &[pb::Swap]) -> Vec<pb::CoverageCandidate> {
    let instructions: Vec<_> = tx.walk_instructions().collect();
    let mut result = Vec::new();
    for (index, ix) in instructions.iter().enumerate() {
        let program = ix.program_id().0;
        if program == &token::PROGRAM_ID || program == &token_2022::PROGRAM_ID || program.iter().all(|byte| *byte == 0) {
            continue;
        }
        let depth = if ix.is_root() { 1 } else { ix.stack_height() };
        if depth == 0 {
            continue;
        }
        let mut exchange_mints = BTreeSet::new();
        let mut transfer_count = 0;
        let mut conversion = false;
        for child in instructions.iter().skip(index + 1) {
            if child.is_root() || child.stack_height() <= depth {
                break;
            }
            if child.stack_height() != depth + 1 {
                continue;
            }
            let child_program = child.program_id().0;
            let data = child.data();
            let accounts = child.accounts();
            if child_program == &token::PROGRAM_ID || child_program == &token_2022::PROGRAM_ID {
                let mint = match data.first() {
                    Some(3) if data.len() == 9 && accounts.len() >= 3 => mints.mint_for(accounts[0].0),
                    Some(12) if data.len() == 10 && accounts.len() >= 4 => Some(accounts[1].0.clone()),
                    Some(26) if data.len() == 19 && data[1] == 1 && accounts.len() >= 4 => Some(accounts[1].0.clone()),
                    Some(7 | 8) if data.len() == 9 && accounts.len() >= 3 => {
                        conversion = true;
                        Some(if data[0] == 7 { accounts[0].0.clone() } else { accounts[1].0.clone() })
                    }
                    Some(14 | 15) if data.len() == 10 && accounts.len() >= 3 => {
                        conversion = true;
                        Some(if data[0] == 14 { accounts[0].0.clone() } else { accounts[1].0.clone() })
                    }
                    _ => None,
                };
                if let Some(mint) = mint {
                    exchange_mints.insert(mint);
                    transfer_count += 1;
                }
            } else if child_program.iter().all(|byte| *byte == 0) && data.len() == 12 && data[..4] == [2, 0, 0, 0] && accounts.len() >= 2 {
                exchange_mints.insert(crate::SOL_MINT.to_vec());
                transfer_count += 1;
            }
        }
        let data = ix.data();
        let fixed = if program == &substreams_solana_idls::pumpswap::PROGRAM_ID && data.starts_with(&[43, 100, 73, 19, 233, 246, 111, 148]) {
            Some(16)
        } else if program == &substreams_solana_idls::pumpfun::bonding_curve::PROGRAM_ID && data.starts_with(&[225, 154, 117, 22, 215, 86, 246, 103]) {
            Some(13)
        } else {
            None
        };
        let accounts = ix.accounts();
        let missing_hop = fixed
            .filter(|fixed| accounts.len() > *fixed && (accounts.len() - fixed) % 5 == 0)
            .is_some_and(|fixed| {
                let end = instructions
                    .iter()
                    .enumerate()
                    .skip(index + 1)
                    .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
                    .map(|(end, _)| end as u32)
                    .unwrap_or(instructions.len() as u32);
                accounts[fixed..].chunks_exact(5).any(|group| {
                    !swaps
                        .iter()
                        .any(|swap| swap.source_index.is_some_and(|source| source >= index as u32 && source < end) && &swap.amm_pool == group[2].0)
                })
            });
        if !missing_hop && swaps.iter().any(|swap| swap.source_index == Some(index as u32)) {
            continue;
        }
        // Phoenix can fill from already deposited balances without a Token
        // CPI. Retain swap/marketable-order intents for event/lot accounting,
        // while excluding post-only placements and cancel/deposit/withdraw.
        // This is diagnostic coverage, never evidence for a financial price.
        let orderbook_swap = crate::openbook::market_index(ix).is_some()
            || program.as_slice() == substreams_solana::b58!("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY")
                && (matches!(data.first(), Some(0 | 1)) || (matches!(data.first(), Some(2 | 3)) && matches!(data.get(1), Some(1 | 2))));
        let known_swap = crate::native_venues::layout(ix).is_some() || crate::rfq::layout(ix).is_some() || fixed.is_some() || orderbook_swap;
        if exchange_mints.len() < 2 && !known_swap {
            continue;
        }
        result.push(pb::CoverageCandidate {
            program_id: program.clone(),
            source_index: index as u32,
            stack_height: depth,
            instruction_tag: ix.data().iter().take(8).copied().collect(),
            mints: exchange_mints.into_iter().collect(),
            transfer_count,
            reason: if missing_hop {
                "multihop_without_all_verified_pools"
            } else if orderbook_swap {
                "orderbook_swap_requires_fill_accounting"
            } else if known_swap {
                "recognized_swap_without_verified_flows"
            } else if conversion {
                "mint_burn_exchange_unclassified"
            } else {
                "token_exchange_unclassified"
            }
            .into(),
        });
    }
    result
}
