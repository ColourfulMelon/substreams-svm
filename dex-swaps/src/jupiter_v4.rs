//! Attribute v4 events to completed AMM calls in their own runtime route.
//! An event cannot borrow a pool from another route or a later invocation.
use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
use substreams_solana_idls::jupiter;

struct Frame {
    program: String,
    index: usize,
    pools: Vec<(Vec<u8>, Vec<u8>, usize)>,
}

pub(crate) fn decode_logs(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    if !tx.is_successful() {
        return Vec::new();
    }
    let instructions: Vec<_> = tx.walk_instructions().collect();
    if !instructions.iter().any(|ix| ix.program_id().0 == &jupiter::v4::PROGRAM_ID) {
        return Vec::new();
    }
    let committed = crate::logs::successful_invocations(tx);
    let mut cursor = 0;
    let mut stack = Vec::<Frame>::new();
    let mut swaps = Vec::new();
    for log in tx.meta.as_ref().into_iter().flat_map(|meta| &meta.log_messages) {
        let words: Vec<_> = log.split_whitespace().collect();
        if words.first() == Some(&"Program") && words.get(2) == Some(&"invoke") && words.len() == 4 {
            let Some(depth) = words[3]
                .strip_prefix('[')
                .and_then(|word| word.strip_suffix(']'))
                .and_then(|word| word.parse::<u32>().ok())
            else {
                return swaps;
            };
            let Ok(program) = substreams_solana::base58::decode(words[1]) else {
                return swaps;
            };
            if depth as usize != stack.len() + 1 {
                return swaps;
            }
            let Some(index) = instructions
                .iter()
                .enumerate()
                .skip(cursor)
                .find(|(_, ix)| ix.program_id().0 == &program && (if ix.is_root() { 1 } else { ix.stack_height() }) == depth)
                .map(|(index, _)| index)
            else {
                return swaps;
            };
            cursor = index + 1;
            stack.push(Frame {
                program: words[1].to_owned(),
                index,
                pools: Vec::new(),
            });
        } else if words.first() == Some(&"Program") && matches!(words.get(2), Some(&"success") | Some(&"failed:")) {
            let Some(frame) = stack.pop() else { return swaps };
            if frame.program != words[1] {
                return swaps;
            }
            if words[2] != "success" || !committed.contains(&(frame.index as u32)) {
                continue;
            }
            if let Some(parent) = stack
                .iter_mut()
                .rev()
                .find(|parent| instructions[parent.index].program_id().0 == &jupiter::v4::PROGRAM_ID)
            {
                if instructions[frame.index].program_id().0 == &jupiter::v4::PROGRAM_ID {
                    parent.pools.extend(frame.pools);
                } else if let Some(pool) = crate::routed_pool::extract_pool(&instructions[frame.index]) {
                    parent.pools.push((instructions[frame.index].program_id().0.clone(), pool, frame.index));
                }
            }
        } else {
            let Some(frame) = stack.last() else { continue };
            if instructions[frame.index].program_id().0 != &jupiter::v4::PROGRAM_ID || !committed.contains(&(frame.index as u32)) {
                continue;
            }
            let Some(data) = parse_program_data(log) else { continue };
            let Ok(jupiter::v4::events::JupiterV4Event::Swap(event)) = jupiter::v4::events::unpack(&data) else {
                continue;
            };
            // Route authority is explicit, including sponsored transactions.
            // Nested adapter events inherit it; direct exact-out adapters have
            // their own IDL-declared authority instead of guessing a fee payer.
            let user = stack.iter().rev().find_map(|parent| {
                use jupiter::v4::instructions::JupiterV4Instruction::*;
                let ix = &instructions[parent.index];
                if ix.program_id().0 != &jupiter::v4::PROGRAM_ID {
                    return None;
                }
                let position = match jupiter::v4::instructions::unpack(ix.data()).ok()? {
                    Route(_) => 1,
                    Whirlpoolswapexactoutput(_) => 2,
                    Raydiumswapexactoutput(_) => 17,
                    Raydiumclmmswapexactoutput(_) => 1,
                    _ => return None,
                };
                ix.accounts().get(position).map(|account| account.0.clone())
            });
            let amm = event.amm.to_bytes().to_vec();
            let frame = stack.last_mut().unwrap();
            let matching: Vec<_> = frame
                .pools
                .iter()
                .enumerate()
                .filter(|(_, (program, _, _))| program == &amm)
                .map(|(i, _)| i)
                .collect();
            // Pending same-program calls without intervening events are
            // ambiguous. Retain an unverified event, never pick the last pool.
            let source = if user.is_some() && matching.len() == 1 {
                Some(frame.pools.remove(matching[0]))
            } else {
                None
            };
            swaps.push(pb::Swap {
                program_id: jupiter::v4::PROGRAM_ID.to_vec(),
                protocol: pb::Protocol::JupiterV4 as i32,
                amm,
                amm_pool: source.as_ref().map_or_else(Vec::new, |(_, pool, _)| pool.clone()),
                user: user.unwrap_or_default(),
                stack_height: source.as_ref().map_or(0, |(_, _, index)| {
                    if instructions[*index].is_root() {
                        1
                    } else {
                        instructions[*index].stack_height()
                    }
                }),
                input_mint: event.input_mint.to_bytes().to_vec(),
                input_amount: event.input_amount,
                output_mint: event.output_mint.to_bytes().to_vec(),
                output_amount: event.output_amount,
                source_index: source.map(|(_, _, index)| index as u32),
                ..Default::default()
            });
        }
    }
    swaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::base58;

    fn fixture(name: &str) -> ConfirmedTransaction {
        let bytes = match name {
            "jupiter-v4-three-hop" => include_bytes!("../fixtures/swap-coverage/jupiter-v4-three-hop.transaction.pb").as_slice(),
            "jupiter-v4-after-v6" => include_bytes!("../fixtures/swap-coverage/jupiter-v4-after-v6.transaction.pb").as_slice(),
            "jupiter-v4-two-routes" => include_bytes!("../fixtures/swap-coverage/jupiter-v4-two-routes.transaction.pb").as_slice(),
            _ => panic!("unknown fixture"),
        };
        substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap()
    }

    #[test]
    fn finalized_repeated_amm_calls_bind_to_their_own_route_and_pool() {
        let oracle = include_str!("../fixtures/swap-coverage/jupiter-v4-route-legs.tsv");
        for name in ["jupiter-v4-three-hop", "jupiter-v4-after-v6", "jupiter-v4-two-routes"] {
            let tx = fixture(name);
            let swaps = decode_logs(&tx);
            let expected: Vec<_> = oracle
                .lines()
                .map(|line| line.split('\t').collect::<Vec<_>>())
                .filter(|fields| fields[0] == name)
                .collect();
            assert_eq!(swaps.len(), expected.len(), "{name}");
            for (swap, fields) in swaps.iter().zip(expected) {
                assert_eq!(swap.source_index, fields[1].parse::<u32>().ok());
                assert_eq!(base58::encode(&swap.amm_pool), if fields[2] == "-" { "" } else { fields[2] });
                assert_eq!(base58::encode(&swap.user), fields[3]);
                assert_eq!(base58::encode(&swap.input_mint), fields[4]);
                assert_eq!(swap.input_amount, fields[5].parse::<u64>().unwrap());
                assert_eq!(base58::encode(&swap.output_mint), fields[6]);
                assert_eq!(swap.output_amount, fields[7].parse::<u64>().unwrap());
                assert_eq!(swap.stack_height, if swap.source_index.is_some() { 2 } else { 0 });
                assert!(swap.source_log_index.is_none());
            }
        }
    }

    #[test]
    fn sponsored_route_uses_declared_authority_and_unknown_authority_is_unverified() {
        let mut tx = fixture("jupiter-v4-three-hop");
        let message = tx.transaction.as_mut().unwrap().message.as_mut().unwrap();
        let authority = message.account_keys[0].clone();
        message.account_keys.insert(0, vec![7; 32]);
        for ix in &mut message.instructions {
            ix.program_id_index += 1;
            for account in &mut ix.accounts {
                *account += 1;
            }
        }
        for group in &mut tx.meta.as_mut().unwrap().inner_instructions {
            for ix in &mut group.instructions {
                ix.program_id_index += 1;
                for account in &mut ix.accounts {
                    *account += 1;
                }
            }
        }
        let swaps = decode_logs(&tx);
        assert_eq!(swaps.len(), 3);
        assert!(swaps.iter().all(|swap| swap.user == authority && swap.source_index.is_some()));
        // This mutation checks the ABI authority, not financial CPI validation.
        tx.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[2].data.pop();
        let swaps = decode_logs(&tx);
        assert_eq!(swaps.len(), 3);
        assert!(swaps
            .iter()
            .all(|swap| swap.user.is_empty() && swap.amm_pool.is_empty() && swap.source_index.is_none()));
    }

    #[test]
    fn foreign_program_data_cannot_impersonate_a_route_event() {
        let mut tx = fixture("jupiter-v4-three-hop");
        let logs = &mut tx.meta.as_mut().unwrap().log_messages;
        let event = logs.iter().find(|log| log.starts_with("Program data: UWzjvs3QCsQ")).unwrap().clone();
        let index = logs
            .iter()
            .position(|log| log.starts_with("Program Tokenkeg") && log.contains("invoke [3]"))
            .unwrap();
        logs.insert(index + 1, event);
        assert_eq!(decode_logs(&tx), decode_logs(&fixture("jupiter-v4-three-hop")));
    }

    #[test]
    fn failed_or_incomplete_ancestors_cannot_prove_a_swap() {
        let program = base58::encode(jupiter::v4::PROGRAM_ID);
        let whirlpool = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
        for failure in [true, false] {
            let mut tx = fixture("jupiter-v4-three-hop");
            let logs = &mut tx.meta.as_mut().unwrap().log_messages;
            let index = logs.iter().position(|log| log == &format!("Program {program} success")).unwrap();
            if failure {
                logs[index] = format!("Program {program} failed: custom program error: 1");
            } else {
                logs.truncate(index);
            }
            assert!(decode_logs(&tx).is_empty());
        }
        let mut tx = fixture("jupiter-v4-three-hop");
        let logs = &mut tx.meta.as_mut().unwrap().log_messages;
        let index = logs.iter().position(|log| log == &format!("Program {whirlpool} success")).unwrap();
        logs[index] = format!("Program {whirlpool} failed: custom program error: 1");
        let swaps = decode_logs(&tx);
        assert_eq!(swaps.len(), 3);
        assert!(swaps[0].amm_pool.is_empty() && swaps[0].source_index.is_none());
        assert_eq!(swaps[1].source_index, Some(6));
        assert_eq!(swaps[2].source_index, Some(9));
        let mut tx = fixture("jupiter-v4-three-hop");
        tx.meta
            .as_mut()
            .unwrap()
            .log_messages
            .push("Program 11111111111111111111111111111111 invoke [1]".into());
        assert_eq!(decode_logs(&tx), decode_logs(&fixture("jupiter-v4-three-hop")));
    }

    #[test]
    fn ambiguous_pending_calls_cannot_choose_the_latest_pool() {
        let mut tx = fixture("jupiter-v4-three-hop");
        let logs = &mut tx.meta.as_mut().unwrap().log_messages;
        let first = logs.iter().position(|log| log.starts_with("Program data: UWzjvs3QCsQ")).unwrap();
        let event = logs.remove(first);
        let second = logs.iter().position(|log| log.starts_with("Program data: UWzjvs3QCsQ")).unwrap();
        logs.insert(second, event);
        let swaps = decode_logs(&tx);
        assert_eq!(swaps.len(), 3);
        assert!(swaps.iter().all(|swap| swap.amm_pool.is_empty() && swap.source_index.is_none()));
    }
}
