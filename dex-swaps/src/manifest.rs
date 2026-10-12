//! Manifest matches deposited balances without a user Token CPI. Only its
//! committed, instruction-scoped FillLog supplies the actual ledger amounts.
use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{b58, base58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms");
const FILL: [u8; 8] = [58, 230, 242, 3, 75, 113, 4, 169];

/// Fully decode BatchUpdate's Borsh hints, cancellations and placements. A
/// post-only/global placement or cancel-only batch cannot take liquidity.
pub(crate) fn market_index(ix: &InstructionView) -> Option<usize> {
    let data = ix.data();
    let accounts = ix.accounts();
    if ix.program_id().0.as_slice() != PROGRAM_ID || data.first() != Some(&6) || accounts.len() < 3 || accounts[2].0.iter().any(|byte| *byte != 0) {
        return None;
    }
    let mut offset = match data.get(1)? {
        0 => 2,
        1 => {
            data.get(2..6)?;
            6
        }
        _ => return None,
    };
    let cancels = u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?) as usize;
    offset += 4;
    if cancels > (data.len() - offset) / 9 {
        return None;
    }
    for _ in 0..cancels {
        data.get(offset..offset + 8)?;
        offset += 8;
        offset += match data.get(offset)? {
            0 => 1,
            1 => {
                data.get(offset + 1..offset + 5)?;
                5
            }
            _ => return None,
        };
    }
    let orders = u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?) as usize;
    offset += 4;
    if orders != (data.len() - offset) / 19 || (data.len() - offset) % 19 != 0 {
        return None;
    }
    let mut can_take = false;
    for order in data[offset..].chunks_exact(19) {
        if order[13] > 1 || order[18] > 5 {
            return None;
        }
        let amount = u64::from_le_bytes(order[..8].try_into().ok()?);
        can_take |= amount > 0 && !matches!(order[18], 2 | 3);
    }
    can_take.then_some(1)
}

pub(crate) fn decode_fills(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    if !tx.is_successful() {
        return Vec::new();
    }
    let instructions: Vec<_> = tx.walk_instructions().collect();
    if !instructions.iter().any(|ix| market_index(ix).is_some()) {
        return Vec::new();
    }
    let committed = crate::logs::successful_invocations(tx);
    let mut cursor = 0;
    let mut stack = Vec::<(String, usize)>::new();
    let mut fills = Vec::new();
    for (log_index, log) in tx.meta.as_ref().into_iter().flat_map(|meta| &meta.log_messages).enumerate() {
        let words: Vec<_> = log.split_whitespace().collect();
        if words.first() == Some(&"Program") && words.get(2) == Some(&"invoke") && words.len() == 4 {
            let Some(depth) = words[3].strip_prefix('[').and_then(|x| x.strip_suffix(']')).and_then(|x| x.parse::<u32>().ok()) else {
                return fills;
            };
            let Ok(program) = base58::decode(words[1]) else { return Vec::new() };
            if depth as usize != stack.len() + 1 {
                return fills;
            }
            let Some(index) = instructions
                .iter()
                .enumerate()
                .skip(cursor)
                .find(|(_, ix)| ix.program_id().0 == &program && (if ix.is_root() { 1 } else { ix.stack_height() }) == depth)
                .map(|(index, _)| index)
            else {
                return fills;
            };
            cursor = index + 1;
            stack.push((words[1].to_owned(), index));
        } else if words.first() == Some(&"Program") && matches!(words.get(2), Some(&"success") | Some(&"failed:")) {
            let Some((program, _)) = stack.pop() else { return Vec::new() };
            if program != words[1] {
                return fills;
            }
        } else if let Some((_, index)) = stack.last() {
            let ix = &instructions[*index];
            let Some(market) = market_index(ix) else { continue };
            if !committed.contains(&(*index as u32)) {
                continue;
            }
            let Some(data) = parse_program_data(log).filter(|data| data.starts_with(&FILL)) else {
                continue;
            };
            let accounts = ix.accounts();
            if data.len() != 232
                || data[216] > 1
                || data[217] > 1
                || data[218..].iter().any(|byte| *byte != 0)
                || &data[8..40] != accounts[market].0.as_slice()
                || &data[72..104] != accounts[0].0.as_slice()
                || data[104..136] == data[136..168]
                || data[104..136].iter().all(|byte| *byte == 0)
                || data[136..168].iter().all(|byte| *byte == 0)
            {
                continue;
            }
            let price = u128::from_le_bytes(data[168..184].try_into().unwrap());
            let base = u64::from_le_bytes(data[184..192].try_into().unwrap());
            let quote = u64::from_le_bytes(data[192..200].try_into().unwrap());
            let Some(product) = price.checked_mul(u128::from(base)) else { continue };
            let floor = product / 1_000_000_000_000_000_000;
            let ceiling = floor + u128::from(product % 1_000_000_000_000_000_000 != 0);
            // Full and partial matches round differently. The ledger's exact
            // quote amount must be either the floor or ceiling of its price.
            if price == 0 || base == 0 || quote == 0 || (u128::from(quote) != floor && u128::from(quote) != ceiling) {
                continue;
            }
            let (input_mint, input_amount, output_mint, output_amount) = if data[216] == 1 {
                (&data[136..168], quote, &data[104..136], base)
            } else {
                (&data[104..136], base, &data[136..168], quote)
            };
            fills.push(pb::Swap {
                protocol: pb::Protocol::Manifest as i32,
                program_id: PROGRAM_ID.to_vec(),
                amm: PROGRAM_ID.to_vec(),
                amm_pool: data[8..40].to_vec(),
                user: data[72..104].to_vec(),
                stack_height: if ix.is_root() { 1 } else { ix.stack_height() },
                input_mint: input_mint.to_vec(),
                input_amount,
                output_mint: output_mint.to_vec(),
                output_amount,
                source_index: Some(*index as u32),
                source_log_index: Some(log_index as u32),
                orderbook_fill_verified: true,
                ..Default::default()
            });
        }
    }
    fills
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routed_pool::test_fixture::make_tx;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions};

    fn batch(kinds: &[u8]) -> Vec<u8> {
        let mut data = vec![6, 0];
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&(kinds.len() as u32).to_le_bytes());
        for kind in kinds {
            data.extend_from_slice(&100u64.to_le_bytes());
            data.extend_from_slice(&2u32.to_le_bytes());
            data.extend_from_slice(&[0, u8::from(*kind == 1)]);
            data.extend_from_slice(&0u32.to_le_bytes());
            data.push(*kind);
        }
        data
    }

    fn fill(buy: bool, sequence: u64) -> Vec<u8> {
        let mut data = FILL.to_vec();
        for key in [[9; 32], [2; 32], [1; 32], [3; 32], [4; 32]] {
            data.extend_from_slice(&key);
        }
        data.extend_from_slice(&2_000_000_000_000_000_000u128.to_le_bytes());
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&200u64.to_le_bytes());
        data.extend_from_slice(&sequence.to_le_bytes());
        data.extend_from_slice(&(21u64 + sequence - 13).to_le_bytes());
        data.extend_from_slice(&[u8::from(buy), 0]);
        data.extend_from_slice(&[0; 14]);
        data
    }

    #[test]
    fn ledger_fills_keep_exact_taker_amounts_direction_and_runtime_identity() {
        let mut tx = make_tx(PROGRAM_ID, &[[1; 32], [9; 32], [0; 32]], batch(&[0, 1]));
        let program = base58::encode(PROGRAM_ID);
        tx.meta.as_mut().unwrap().log_messages = vec![
            format!("Program {program} invoke [1]"),
            format!("Program data: {}", STANDARD.encode(fill(false, 13))),
            format!("Program data: {}", STANDARD.encode(fill(true, 14))),
            format!("Program {program} success"),
        ];
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        assert_eq!(decoded.swaps.len(), 2);
        assert!(decoded.rejected_swaps.is_empty());
        for (i, swap) in decoded.swaps.iter().enumerate() {
            assert!(swap.orderbook_fill_verified && !swap.transfer_verified);
            assert_eq!(swap.amm_pool, vec![9; 32]);
            assert_eq!(swap.user, vec![1; 32]);
            assert_eq!(swap.source_index, Some(0));
            assert_eq!(swap.source_log_index, Some(i as u32 + 1));
            assert_eq!((swap.input_amount, swap.output_amount), if i == 0 { (100, 200) } else { (200, 100) });
        }
        assert!(decoded.coverage_candidates.is_empty());
        let mut unrelated_incomplete_trace = tx.clone();
        unrelated_incomplete_trace
            .meta
            .as_mut()
            .unwrap()
            .log_messages
            .push("Program 11111111111111111111111111111111 invoke [2]".into());
        assert_eq!(decode_fills(&unrelated_incomplete_trace), decoded.swaps);
        for mode in 0..10 {
            let mut altered = tx.clone();
            let logs = &mut altered.meta.as_mut().unwrap().log_messages;
            let mut data = fill(false, 13);
            match mode {
                0 => {
                    logs.clear();
                }
                1 => {
                    logs.pop();
                }
                2 => {
                    data.pop();
                }
                3 => data[8] ^= 1,
                4 => data[72] ^= 1,
                5 => data[216] = 2,
                6 => data[218] = 1,
                7 => data[184..192].fill(0),
                8 => data[192..200].copy_from_slice(&201u64.to_le_bytes()),
                _ => data[104..136].fill(0),
            }
            if mode >= 2 {
                logs[1] = format!("Program data: {}", STANDARD.encode(data));
                logs.remove(2);
            }
            assert!(decode_fills(&altered).is_empty(), "mode {mode}");
        }
        tx.meta.as_mut().unwrap().log_messages = vec![format!("Program {program} invoke [1]"), format!("Program {program} success")];
        let decoded = crate::process_transaction(tx).unwrap();
        assert!(decoded.swaps.is_empty());
        assert_eq!(decoded.coverage_candidates[0].reason, "orderbook_swap_requires_fill_accounting");
    }

    #[test]
    fn nested_foreign_logs_and_rolled_back_ancestors_do_not_prove_a_fill() {
        let mut tx = make_tx(PROGRAM_ID, &[[1; 32], [9; 32], [0; 32]], batch(&[0]));
        let program = base58::encode(PROGRAM_ID);
        let child = base58::encode([7; 32]);
        tx.transaction.as_mut().unwrap().message.as_mut().unwrap().account_keys.push(vec![7; 32]);
        tx.meta.as_mut().unwrap().inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![InnerInstruction {
                program_id_index: 5,
                accounts: vec![],
                data: vec![],
                stack_height: Some(2),
            }],
        }];
        tx.meta.as_mut().unwrap().log_messages = vec![
            format!("Program {program} invoke [1]"),
            format!("Program {child} invoke [2]"),
            format!("Program data: {}", STANDARD.encode(fill(false, 13))),
            format!("Program {child} success"),
            format!("Program {program} success"),
        ];
        assert!(decode_fills(&tx).is_empty());
        let outer = base58::encode([8; 32]);
        let message = tx.transaction.as_mut().unwrap().message.as_mut().unwrap();
        message.account_keys.push(vec![8; 32]);
        message.instructions[0].program_id_index = 6;
        message.instructions[0].data.clear();
        tx.meta.as_mut().unwrap().inner_instructions[0].instructions = vec![
            InnerInstruction {
                program_id_index: 5,
                accounts: vec![],
                data: vec![],
                stack_height: Some(2),
            },
            InnerInstruction {
                program_id_index: 1,
                accounts: vec![2, 3, 4],
                data: batch(&[0]),
                stack_height: Some(3),
            },
        ];
        tx.meta.as_mut().unwrap().log_messages = vec![
            format!("Program {outer} invoke [1]"),
            format!("Program {child} invoke [2]"),
            format!("Program {program} invoke [3]"),
            format!("Program data: {}", STANDARD.encode(fill(false, 13))),
            format!("Program {program} success"),
            format!("Program {child} failed: custom program error: 1"),
            format!("Program {outer} success"),
        ];
        assert!(decode_fills(&tx).is_empty());
        tx.meta.as_mut().unwrap().log_messages[5] = format!("Program {child} success");
        assert_eq!(decode_fills(&tx).len(), 1);
    }

    #[test]
    fn finalized_unfilled_batches_do_not_create_financial_prices() {
        for fixture in [
            include_bytes!("../fixtures/native-venues/manifest-unfilled-batch-1.transaction.pb").as_slice(),
            include_bytes!("../fixtures/native-venues/manifest-unfilled-batch-2.transaction.pb").as_slice(),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let decoded = crate::process_transaction(tx).unwrap();
            assert!(decoded.swaps.is_empty());
            assert!(decoded
                .coverage_candidates
                .iter()
                .any(|candidate| candidate.program_id == PROGRAM_ID && candidate.reason == "orderbook_swap_requires_fill_accounting"));
        }
    }

    #[test]
    fn batch_payload_and_marketability_are_fully_decoded() {
        for (kinds, expected) in [
            (vec![0], true),
            (vec![1], true),
            (vec![2, 3], false),
            (vec![4, 5], true),
            (vec![], false),
            (vec![6], false),
        ] {
            let tx = make_tx(PROGRAM_ID, &[[1; 32], [9; 32], [0; 32]], batch(&kinds));
            assert_eq!(market_index(&tx.walk_instructions().next().unwrap()).is_some(), expected);
        }
        for mode in 0..4 {
            let mut data = batch(&[0]);
            match mode {
                0 => {
                    data.pop();
                }
                1 => data.push(0),
                2 => data[1] = 2,
                _ => data[23] = 2,
            }
            let tx = make_tx(PROGRAM_ID, &[[1; 32], [9; 32], [0; 32]], data);
            assert!(market_index(&tx.walk_instructions().next().unwrap()).is_none());
        }
    }
}
