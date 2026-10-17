//! Conservative invocation evidence. This does not change protocol decoding.
use crate::{native::instruction_positions, InstructionPosition, InvocationOutcome};
use common::solana::{is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_id};
use std::collections::BTreeMap;
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

pub(crate) fn prove(tx: &ConfirmedTransaction) -> Option<BTreeMap<InstructionPosition, InvocationOutcome>> {
    let meta = tx.meta.as_ref()?;
    let expected = instruction_positions(tx)
        .into_iter()
        .zip(tx.walk_instructions())
        .map(|(position, instruction)| {
            let depth = if position.inner_instruction_index.is_none() {
                Some(1)
            } else {
                instruction.maybe_stack_height()
            };
            (position, instruction.program_id().0.to_vec(), depth)
        })
        .collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut parents: Vec<Option<usize>> = vec![None; expected.len()];
    let mut outcomes = vec![InvocationOutcome::Unknown; expected.len()];
    for log in &meta.log_messages {
        let Some(program) = parse_program_id(log) else { continue };
        if is_invoke(log) {
            let depth = parse_invoke_depth(log)?;
            if depth as usize != stack.len() + 1 {
                return None;
            }
            while let Some((position, candidate, candidate_depth)) = expected.get(cursor) {
                if candidate == &program && *candidate_depth == Some(depth) {
                    break;
                }
                if !without_logs(*position, candidate) {
                    return None;
                }
                cursor += 1;
            }
            let (_, candidate, candidate_depth) = expected.get(cursor)?;
            if candidate != &program || *candidate_depth != Some(depth) {
                return None;
            }
            parents[cursor] = stack.last().copied();
            stack.push(cursor);
            cursor += 1;
        } else if is_success(log) || is_failed(log) {
            let index = stack.pop()?;
            if expected[index].1 != program {
                return None;
            }
            outcomes[index] = if is_success(log) {
                InvocationOutcome::Succeeded
            } else {
                InvocationOutcome::Failed
            };
        }
    }
    if !stack.is_empty() {
        return None;
    }
    while let Some((position, candidate, _)) = expected.get(cursor) {
        if !without_logs(*position, candidate) {
            return None;
        }
        cursor += 1;
    }
    // A successful event self-CPI can still be rolled back by an enclosing
    // invocation that fails and is caught. Propagate every ancestor outcome.
    for index in 0..outcomes.len() {
        if let Some(parent) = parents[index] {
            outcomes[index] = match (outcomes[index], outcomes[parent]) {
                (_, InvocationOutcome::Failed) => InvocationOutcome::Failed,
                (_, InvocationOutcome::Unknown) => InvocationOutcome::Unknown,
                (outcome, InvocationOutcome::Succeeded) => outcome,
            };
        }
    }
    Some(
        expected
            .into_iter()
            .zip(outcomes)
            .map(|((position, _, _), outcome)| (position, outcome))
            .collect(),
    )
}

fn without_logs(position: InstructionPosition, program: &[u8]) -> bool {
    // These top-level runtime/precompile instructions do not produce program
    // invocation logs. Never skip an arbitrary program to force alignment.
    position.inner_instruction_index.is_none()
        && matches!(
            base58::encode(program).as_str(),
            "ComputeBudget111111111111111111111111111111"
                | "Ed25519SigVerify111111111111111111111111111"
                | "KeccakSecp256k11111111111111111111111111111"
                | "Secp256r1SigVerify1111111111111111111111111"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1 as pb;

    fn transaction() -> pb::ConfirmedTransaction {
        let a = [1; 32];
        let b = [2; 32];
        let c = [3; 32];
        let logs = vec![
            format!("Program {} invoke [1]", base58::encode(a)),
            format!("Program {} invoke [2]", base58::encode(b)),
            format!("Program {} invoke [3]", base58::encode(c)),
            format!("Program {} success", base58::encode(c)),
            format!("Program {} success", base58::encode(b)),
            format!("Program {} success", base58::encode(a)),
        ];
        pb::ConfirmedTransaction {
            transaction: Some(pb::Transaction {
                signatures: vec![vec![1; 64]],
                message: Some(pb::Message {
                    header: Some(pb::MessageHeader {
                        num_required_signatures: 1,
                        ..Default::default()
                    }),
                    account_keys: vec![a.to_vec(), b.to_vec(), c.to_vec()],
                    instructions: vec![pb::CompiledInstruction {
                        program_id_index: 0,
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            }),
            meta: Some(pb::TransactionStatusMeta {
                inner_instructions: vec![pb::InnerInstructions {
                    index: 0,
                    instructions: vec![
                        pb::InnerInstruction {
                            program_id_index: 1,
                            stack_height: Some(2),
                            ..Default::default()
                        },
                        pb::InnerInstruction {
                            program_id_index: 2,
                            stack_height: Some(3),
                            ..Default::default()
                        },
                    ],
                }],
                log_messages: logs,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn successful_trace_proves_each_original_instruction() {
        let result = prove(&transaction()).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.values().all(|outcome| *outcome == InvocationOutcome::Succeeded));
    }

    #[test]
    fn caught_parent_failure_revokes_preceding_successful_event() {
        let mut tx = transaction();
        tx.meta.as_mut().unwrap().log_messages[4] = format!("Program {} failed: caught", base58::encode([2; 32]));
        let result = prove(&tx).unwrap();
        assert_eq!(
            result[&InstructionPosition {
                instruction_index: 0,
                inner_instruction_index: None
            }],
            InvocationOutcome::Succeeded
        );
        assert_eq!(
            result[&InstructionPosition {
                instruction_index: 0,
                inner_instruction_index: Some(1)
            }],
            InvocationOutcome::Failed
        );
    }

    #[test]
    fn truncated_unknown_depth_and_misaligned_traces_never_prove_success() {
        for mutation in 0..3 {
            let mut tx = transaction();
            match mutation {
                0 => {
                    tx.meta.as_mut().unwrap().log_messages.pop();
                }
                1 => tx.meta.as_mut().unwrap().inner_instructions[0].instructions[0].stack_height = None,
                2 => tx.meta.as_mut().unwrap().log_messages[1] = format!("Program {} invoke [2]", base58::encode([9; 32])),
                _ => unreachable!(),
            }
            assert!(prove(&tx).is_none());
        }
    }
}
