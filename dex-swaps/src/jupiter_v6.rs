use std::collections::HashMap;

use common::solana::get_fee_payer;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::jupiter;

// Anchor event self-CPI prefix, then the program-owned on-chain IDL event tag.
const EVENT_CPI: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];
const SWAP: [u8; 8] = [64, 198, 205, 232, 38, 8, 113, 226];
const SWAPS: [u8; 8] = [152, 47, 78, 235, 192, 96, 110, 106];

pub(crate) fn decode_instruction(tx: &ConfirmedTransaction, index: usize, instructions: &[InstructionView]) -> Vec<pb::Swap> {
    let instruction = &instructions[index];
    let data = instruction.data();
    if instruction.program_id().0 != &jupiter::v6::PROGRAM_ID || data.len() < 16 || data[..8] != EVENT_CPI {
        return Vec::new();
    }
    let (records, plural) = if data[8..16] == SWAP && data.len() == 128 {
        (&data[16..], false)
    } else if data[8..16] == SWAPS && data.len() >= 20 {
        let count = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
        if count.checked_mul(112) != Some(data.len() - 20) {
            return Vec::new();
        }
        (&data[20..], true)
    } else {
        return Vec::new();
    };
    let depth = if instruction.is_root() { 1 } else { instruction.stack_height() };
    let parent = instructions[..index]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, ix)| (if ix.is_root() { 1 } else { ix.stack_height() }) < depth);
    let route = parent.filter(|(_, ix)| ix.program_id().0 == &jupiter::v6::PROGRAM_ID);
    let mut user = get_fee_payer(tx).unwrap_or_default();
    let mut history = Vec::new();
    if let Some((parent_index, parent)) = route {
        // Every route variant declares the transfer authority explicitly.
        // Sponsored transactions cannot use the fee payer as trader identity.
        let authority = match parent.data().get(..8) {
            Some(
                [229, 23, 203, 151, 122, 227, 173, 42]
                | [150, 86, 71, 116, 167, 93, 14, 104]
                | [208, 51, 239, 151, 123, 43, 237, 92]
                | [209, 152, 83, 147, 124, 254, 216, 233]
                | [53, 96, 229, 202, 216, 187, 250, 24],
            ) => Some(1),
            Some([193, 32, 155, 51, 65, 214, 156, 129] | [230, 121, 143, 80, 119, 159, 106, 170] | [176, 209, 105, 168, 154, 125, 69, 62]) => Some(2),
            Some([187, 100, 250, 204, 49, 196, 175, 20] | [157, 138, 184, 82, 21, 244, 243, 36]) => Some(0),
            _ => None,
        };
        if let Some(authority) = authority.and_then(|position| parent.accounts().get(position).map(|account| account.0.clone())) {
            user = authority;
            // Legacy events consume one preceding leg; a plural event consumes
            // the complete vector since the previous sibling swap event. Never
            // take a pool from another route or a later same-program call.
            let begin = instructions[parent_index + 1..index]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, ix)| {
                    ix.program_id().0 == &jupiter::v6::PROGRAM_ID
                        && ix.stack_height() == depth
                        && ix.data().len() >= 16
                        && ix.data()[..8] == EVENT_CPI
                        && (ix.data()[8..16] == SWAP || ix.data()[8..16] == SWAPS)
                })
                .map_or(parent_index + 1, |(offset, _)| parent_index + offset + 2);
            let caught_failure = tx.meta.as_ref().is_some_and(|meta| {
                meta.log_messages.iter().any(|log| {
                    let mut words = log.split_whitespace();
                    words.next() == Some("Program") && words.next().is_some() && words.next() == Some("failed:")
                })
            });
            let successful = if caught_failure {
                crate::logs::successful_invocations(tx)
            } else {
                Default::default()
            };
            for (offset, ix) in instructions[begin..index].iter().enumerate() {
                let source = begin + offset;
                if (!caught_failure || successful.contains(&(source as u32))) && ix.stack_height() >= depth {
                    if let Some(pool) = crate::routed_pool::extract_pool(ix) {
                        history.push((ix.program_id().0.clone(), pool, source as u32));
                    }
                }
            }
        }
    }
    let mut counts = HashMap::new();
    for record in records.chunks_exact(112) {
        let amm = if plural { &record[80..112] } else { &record[..32] };
        *counts.entry(amm.to_vec()).or_insert(0usize) += 1;
    }
    let mut used = HashMap::new();
    records
        .chunks_exact(112)
        .map(|record| {
            let (amm, input, input_amount, output, output_amount) = if plural {
                (&record[80..112], &record[..32], &record[32..40], &record[40..72], &record[72..80])
            } else {
                (&record[..32], &record[32..64], &record[64..72], &record[72..104], &record[104..112])
            };
            let occurrence = used.entry(amm.to_vec()).or_insert(0usize);
            let matching: Vec<_> = history.iter().filter(|(program, _, _)| program.as_slice() == amm).collect();
            // Count/order agreement is required; ambiguity stays diagnostic and
            // cannot assign every vector element to the last observed pool.
            let source = if matching.len() == counts[amm] {
                matching.get(*occurrence).copied()
            } else {
                None
            };
            *occurrence += 1;
            pb::Swap {
                program_id: jupiter::v6::PROGRAM_ID.to_vec(),
                protocol: pb::Protocol::JupiterV6 as i32,
                stack_height: depth,
                amm: amm.to_vec(),
                amm_pool: source.map_or_else(Vec::new, |(_, pool, _)| pool.clone()),
                user: user.clone(),
                input_mint: input.to_vec(),
                input_amount: u64::from_le_bytes(input_amount.try_into().unwrap()),
                output_mint: output.to_vec(),
                output_amount: u64::from_le_bytes(output_amount.try_into().unwrap()),
                source_index: source.map(|(_, _, index)| *index),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routed_pool::test_fixture::make_tx;
    use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions};
    use substreams_solana_idls::pumpfun;

    fn event(plural: bool, amms: &[[u8; 32]]) -> Vec<u8> {
        let mut data = EVENT_CPI.to_vec();
        data.extend_from_slice(if plural { &SWAPS } else { &SWAP });
        if plural {
            data.extend_from_slice(&(amms.len() as u32).to_le_bytes());
        }
        for (n, amm) in amms.iter().enumerate() {
            if !plural {
                data.extend_from_slice(amm);
            }
            data.extend_from_slice(&[11; 32]);
            data.extend_from_slice(&(100u64 + n as u64).to_le_bytes());
            data.extend_from_slice(&[12; 32]);
            data.extend_from_slice(&(90u64 + n as u64).to_le_bytes());
            if plural {
                data.extend_from_slice(amm);
            }
        }
        data
    }

    fn route(pools: &[[u8; 32]], events: Vec<Vec<u8>>) -> ConfirmedTransaction {
        let mut tx = make_tx(jupiter::v6::PROGRAM_ID, &[[20; 32], [22; 32]], vec![229, 23, 203, 151, 122, 227, 173, 42]);
        let message = tx.transaction.as_mut().unwrap().message.as_mut().unwrap();
        message.account_keys.push(pumpfun::amm::PROGRAM_ID.to_vec());
        let mut instructions = Vec::new();
        for pool in pools {
            let begin = message.account_keys.len();
            for account in [*pool, [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]] {
                message.account_keys.push(account.to_vec());
            }
            let mut data = vec![102, 6, 61, 18, 1, 218, 235, 234];
            data.extend_from_slice(&100u64.to_le_bytes());
            data.extend_from_slice(&90u64.to_le_bytes());
            instructions.push(InnerInstruction {
                program_id_index: 4,
                accounts: (begin..begin + 6).map(|n| n as u8).collect(),
                data,
                stack_height: Some(2),
            });
        }
        instructions.extend(events.into_iter().map(|data| InnerInstruction {
            program_id_index: 1,
            accounts: vec![],
            data,
            stack_height: Some(2),
        }));
        tx.meta.as_mut().unwrap().inner_instructions = vec![InnerInstructions { index: 0, instructions }];
        tx
    }

    #[test]
    fn legacy_swap_uses_its_scoped_pool_and_transfer_authority() {
        let tx = route(&[[9; 32]], vec![event(false, &[pumpfun::amm::PROGRAM_ID])]);
        let instructions: Vec<_> = tx.walk_instructions().collect();
        let swap = decode_instruction(&tx, instructions.len() - 1, &instructions).pop().unwrap();
        assert_eq!(swap.amm_pool, vec![9; 32]);
        assert_eq!(swap.user, vec![22; 32]);
        assert_eq!((swap.input_amount, swap.output_amount), (100, 90));
        assert_eq!(swap.source_index, Some(1));
    }

    #[test]
    fn plural_swaps_preserve_each_occurrence_of_the_same_program() {
        let tx = route(&[[9; 32], [10; 32]], vec![event(true, &[pumpfun::amm::PROGRAM_ID, pumpfun::amm::PROGRAM_ID])]);
        let instructions: Vec<_> = tx.walk_instructions().collect();
        let swaps = decode_instruction(&tx, instructions.len() - 1, &instructions);
        assert_eq!(swaps.len(), 2);
        assert_eq!((swaps[0].amm_pool.clone(), swaps[1].amm_pool.clone()), (vec![9; 32], vec![10; 32]));
        assert_eq!((swaps[0].source_index, swaps[1].source_index), (Some(1), Some(2)));
        assert_eq!((swaps[1].input_amount, swaps[1].output_amount), (101, 91));
    }

    #[test]
    fn finalized_event_vectors_match_independent_onchain_idl_decoding() {
        use substreams_solana::base58;
        let mut events = 0;
        let mut records = 0;
        for line in include_str!("../fixtures/swap-coverage/jupiter-events.tsv").lines() {
            let fields: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/swap-coverage/{}", env!("CARGO_MANIFEST_DIR"), fields[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            assert_eq!(base58::encode(tx.hash()), fields[1]);
            let instructions: Vec<_> = tx.walk_instructions().collect();
            let swaps = decode_instruction(&tx, fields[2].parse().unwrap(), &instructions);
            let actual: Vec<_> = swaps
                .iter()
                .map(|swap| {
                    format!(
                        "{}:{}:{}:{}:{}",
                        base58::encode(&swap.amm),
                        base58::encode(&swap.input_mint),
                        swap.input_amount,
                        base58::encode(&swap.output_mint),
                        swap.output_amount
                    )
                })
                .collect();
            assert_eq!(actual.join("|"), fields[3], "event fields differ in {} at {}", fields[0], fields[2]);
            records += swaps.len();
            events += 1;
        }
        assert_eq!((events, records), (109, 203));
    }

    #[test]
    fn legacy_event_boundaries_and_later_routes_do_not_shift_pool_identity() {
        let mut tx = route(
            &[[9; 32], [10; 32]],
            vec![event(false, &[pumpfun::amm::PROGRAM_ID]), event(false, &[pumpfun::amm::PROGRAM_ID])],
        );
        let first_event = tx.meta.as_mut().unwrap().inner_instructions[0].instructions.remove(2);
        tx.meta.as_mut().unwrap().inner_instructions[0].instructions.insert(1, first_event);
        let message = tx.transaction.as_mut().unwrap().message.as_mut().unwrap();
        let route_ix = message.instructions[0].clone();
        message.instructions.push(route_ix);
        let mut later_call = tx.meta.as_ref().unwrap().inner_instructions[0].instructions[0].clone();
        let begin = message.account_keys.len();
        for account in [[15; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]] {
            message.account_keys.push(account.to_vec());
        }
        later_call.accounts = (begin..begin + 6).map(|n| n as u8).collect();
        tx.meta.as_mut().unwrap().inner_instructions.push(InnerInstructions {
            index: 1,
            instructions: vec![
                later_call,
                InnerInstruction {
                    program_id_index: 1,
                    accounts: vec![],
                    data: event(true, &[pumpfun::amm::PROGRAM_ID]),
                    stack_height: Some(2),
                },
            ],
        });
        let instructions: Vec<_> = tx.walk_instructions().collect();
        for (index, pool, source) in [(2, 9, 1), (4, 10, 3), (7, 15, 6)] {
            let swap = decode_instruction(&tx, index, &instructions).pop().unwrap();
            assert_eq!(swap.amm_pool, vec![pool; 32]);
            assert_eq!(swap.source_index, Some(source));
        }
    }

    #[test]
    fn caught_failed_calls_and_truncated_runtime_logs_cannot_supply_a_pool() {
        use substreams_solana::base58;
        let mut tx = route(&[[9; 32], [10; 32]], vec![event(true, &[pumpfun::amm::PROGRAM_ID])]);
        let jup = base58::encode(jupiter::v6::PROGRAM_ID);
        let pump = base58::encode(pumpfun::amm::PROGRAM_ID);
        tx.meta.as_mut().unwrap().log_messages = vec![
            format!("Program {jup} invoke [1]"),
            format!("Program {pump} invoke [2]"),
            format!("Program {pump} failed: custom program error: 1"),
            format!("Program {pump} invoke [2]"),
            format!("Program {pump} success"),
            format!("Program {jup} invoke [2]"),
            format!("Program {jup} success"),
            format!("Program {jup} success"),
        ];
        {
            let instructions: Vec<_> = tx.walk_instructions().collect();
            let swap = decode_instruction(&tx, 3, &instructions).pop().unwrap();
            assert_eq!(swap.amm_pool, vec![10; 32]);
            assert_eq!(swap.source_index, Some(2));
        }
        tx.meta.as_mut().unwrap().log_messages.pop();
        let instructions: Vec<_> = tx.walk_instructions().collect();
        let swap = decode_instruction(&tx, 3, &instructions).pop().unwrap();
        assert!(swap.amm_pool.is_empty() && swap.source_index.is_none());
    }

    #[test]
    fn malformed_unknown_and_ambiguous_events_never_guess_a_pool() {
        let original = event(true, &[pumpfun::amm::PROGRAM_ID]);
        for mode in 0..4 {
            let mut data = original.clone();
            match mode {
                0 => data.push(0),
                1 => data[16] = 255,
                2 => data[8] = 0,
                3 => data[0] = 0,
                _ => unreachable!(),
            }
            let tx = route(&[[9; 32]], vec![data]);
            let instructions: Vec<_> = tx.walk_instructions().collect();
            assert!(decode_instruction(&tx, instructions.len() - 1, &instructions).is_empty());
        }
        for (pools, amms) in [(vec![[9; 32]], vec![[42; 32]]), (vec![[9; 32], [10; 32]], vec![pumpfun::amm::PROGRAM_ID])] {
            let tx = route(&pools, vec![event(true, &amms)]);
            let instructions: Vec<_> = tx.walk_instructions().collect();
            let swaps = decode_instruction(&tx, instructions.len() - 1, &instructions);
            assert!(swaps.iter().all(|swap| swap.amm_pool.is_empty() && swap.source_index.is_none()));
        }
        let tx = make_tx([99; 32], &[], original);
        let instructions: Vec<_> = tx.walk_instructions().collect();
        assert!(decode_instruction(&tx, 0, &instructions).is_empty());
    }
}
