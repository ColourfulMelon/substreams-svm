//! Bind implicit native Pump sell amounts to committed reserve-state witnesses,
//! exact token inputs, runtime-scoped fees, and unique integer rounding bounds.
use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD, Engine};
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::pumpfun::bonding_curve as pumpfun;

pub const FEE_PROGRAM: [u8; 32] = substreams_solana_macro::b58!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ");

/// Recover the only pre-trade SOL reserve compatible with the canonical
/// post-reserve pair and exact token input. Constant-product floor rounding
/// implies postS = ceil(preS * preT / postT); reject intervals with two possible
/// reserves instead of allocating the transaction's aggregate user credit.
pub(crate) fn sell(input: u64, post_tokens: u64, post_sol: u64, fees: Fees) -> Option<(u64, u64, u64, u64, u64)> {
    if input == 0 || post_sol == 0 || fees.lp != 0 || fees.protocol.checked_add(fees.creator)? > 10_000 { return None; }
    let pre_tokens = post_tokens.checked_sub(input)?;
    if pre_tokens == 0 { return None; }
    let lower = u128::from(post_sol - 1).checked_mul(u128::from(post_tokens))? / u128::from(pre_tokens) + 1;
    let upper = u128::from(post_sol).checked_mul(u128::from(post_tokens))? / u128::from(pre_tokens);
    if lower != upper || lower > u128::from(u64::MAX) { return None; }
    let pre_sol = lower as u64;
    let gross = pre_sol.checked_sub(post_sol)?;
    if u128::from(input).checked_mul(lower)? / u128::from(post_tokens) != u128::from(gross) { return None; }
    let protocol: u64 = ((u128::from(gross) * u128::from(fees.protocol) + 9_999) / 10_000).try_into().ok()?;
    let creator: u64 = ((u128::from(gross) * u128::from(fees.creator) + 9_999) / 10_000).try_into().ok()?;
    let net = gross.checked_sub(protocol)?.checked_sub(creator)?;
    (net > 0).then_some((gross, net, protocol, creator, pre_sol))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Fees { pub lp: u64, pub protocol: u64, pub creator: u64 }

/// Current primary TradeEvent's cashback extension follows its bounded
/// instruction-name string. Older events with no extension prove no cashback.
pub(crate) fn cashback(data: &[u8]) -> Option<(u64, u64)> {
    if data.len() <= 266 { return Some((0, 0)); }
    let name_len = u32::from_le_bytes(data.get(266..270)?.try_into().ok()?) as usize;
    if name_len > 64 { return None; }
    let name_end = 270usize.checked_add(name_len)?;
    if data.get(270..name_end)? != b"sell" || *data.get(name_end)? > 1 { return None; }
    let bps = u64::from_le_bytes(data.get(name_end + 1..name_end + 9)?.try_into().ok()?);
    let amount = u64::from_le_bytes(data.get(name_end + 9..name_end + 17)?.try_into().ok()?);
    (bps <= 10_000).then_some((bps, amount))
}

/// Bind each exact fee return to the native instruction walk and its live
/// runtime frame. Foreign, duplicate, failed, or truncated returns cannot
/// provide a per-leg witness; completed descendants still require commitment.
pub(crate) fn fee_returns(tx: &ConfirmedTransaction) -> HashMap<u32, Fees> {
    let instructions: Vec<_> = tx.walk_instructions().collect();
    let committed = crate::logs::successful_invocations(tx);
    let mut cursor = 0;
    let mut stack: Vec<(usize, Option<Fees>, usize)> = Vec::new();
    let mut returns = HashMap::new();
    for log in tx.meta.as_option().into_iter().flat_map(|meta| &meta.log_messages) {
        if log == "Log truncated" { break; }
        let words: Vec<_> = log.split_whitespace().collect();
        if words.first() != Some(&"Program") || words.len() < 3 { continue; }
        if words[2] == "invoke" && words.len() == 4 {
            let Some(depth) = words[3].strip_prefix('[').and_then(|s| s.strip_suffix(']')).and_then(|s| s.parse::<u32>().ok()) else { return HashMap::new() };
            let Ok(program) = base58::decode(words[1]) else { return HashMap::new() };
            let Some(ix) = instructions.get(cursor) else { return HashMap::new() };
            if ix.program_id().0 != &program || (if ix.is_root() { 1 } else { ix.stack_height() }) != depth
                || depth as usize != stack.len() + 1 { return HashMap::new(); }
            stack.push((cursor, None, 0));
            cursor += 1;
        } else if words[1] == "return:" {
            let Some(frame) = stack.last_mut() else { return HashMap::new() };
            if words.len() != 4 || words[2] != base58::encode(instructions[frame.0].program_id().0) { return HashMap::new(); }
            if instructions[frame.0].program_id().0.as_slice() != FEE_PROGRAM { continue; }
            frame.2 += 1;
            let Ok(bytes) = STANDARD.decode(words[3]) else { continue };
            if bytes.len() != 24 { continue; }
            frame.1 = Some(Fees {
                lp: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
                protocol: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
                creator: u64::from_le_bytes(bytes[16..].try_into().unwrap()),
            });
        } else if words[2] == "success" || words[2] == "failed:" {
            let Some((position, fees, count)) = stack.pop() else { return HashMap::new() };
            if words[1] != base58::encode(instructions[position].program_id().0) { return HashMap::new(); }
            if words[2] == "success" && count == 1 && committed.contains(&(position as u32))
                && stack.last().is_some_and(|frame| instructions[frame.0].program_id().0.as_slice() == pumpfun::PROGRAM_ID)
            {
                if let Some(fees) = fees { returns.insert(position as u32, fees); }
            }
        }
    }
    returns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_sell_requires_one_integer_pre_reserve_and_exact_fee_math() {
        let zero = Fees { lp: 0, protocol: 0, creator: 0 };
        assert_eq!(sell(1, 100, 100, zero), Some((1, 1, 0, 0, 101)));
        // Both preS=1 and preS=2 have postS=1 here. The observed transaction
        // credit must never choose one of two compatible individual trades.
        assert!(sell(1, 2, 1, zero).is_none());
        for (input, tokens, sol) in [(0, 100, 100), (100, 100, 100), (101, 100, 100), (1, 100, 0)] {
            assert!(sell(input, tokens, sol, zero).is_none());
        }
        assert!(sell(1, 100, 100, Fees { lp: 1, ..zero }).is_none());
        assert!(sell(1, 100, 100, Fees { protocol: u64::MAX, creator: 1, ..zero }).is_none());
    }
}
