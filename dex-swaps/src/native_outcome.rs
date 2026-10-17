//! Conservative invocation evidence. This does not change protocol decoding.
use crate::{native::instruction_positions, InstructionPosition, InvocationOutcome};
use std::collections::BTreeMap;
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Evidence {
    pub outcome: InvocationOutcome,
    pub parent: Option<InstructionPosition>,
}

pub(crate) fn prove(tx: &ConfirmedTransaction) -> Option<BTreeMap<InstructionPosition, Evidence>> {
    let meta = tx.meta.as_option()?;
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
        let Some((program, event)) = parse_log(log).ok()? else { continue };
        if let Event::Invoke(depth) = event {
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
        } else {
            let index = stack.pop()?;
            if expected[index].1 != program {
                return None;
            }
            outcomes[index] = match event {
                Event::Success => InvocationOutcome::Succeeded,
                Event::Failure => InvocationOutcome::Failed,
                Event::Invoke(_) => unreachable!(),
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
            .iter()
            .enumerate()
            .map(|(index, (position, _, _))| {
                (
                    *position,
                    Evidence {
                        outcome: outcomes[index],
                        parent: parents[index].map(|parent| expected[parent].0),
                    },
                )
            })
            .collect(),
    )
}

#[derive(Clone, Copy)]
enum Event {
    Invoke(u32),
    Success,
    Failure,
}

/// Runtime invocation grammar must be exact. Protocol decoder helpers are
/// deliberately permissive and cannot serve as execution-success evidence.
fn parse_log(log: &str) -> Result<Option<(Vec<u8>, Event)>, ()> {
    let Some(rest) = log.strip_prefix("Program ") else { return Ok(None) };
    let Some((identity, tail)) = rest.split_once(' ') else { return Ok(None) };
    if matches!(identity, "log:" | "data:" | "return:") {
        return Ok(None);
    }
    let event = if tail == "success" {
        Event::Success
    } else if tail.strip_prefix("failed: ").is_some_and(|message| !message.is_empty()) {
        // The failure message may itself end in the word "success".
        Event::Failure
    } else if let Some(depth) = tail.strip_prefix("invoke [").and_then(|rest| rest.strip_suffix(']')) {
        let parsed: u32 = depth.parse().map_err(|_| ())?;
        if parsed == 0 || parsed.to_string() != depth {
            return Err(());
        }
        Event::Invoke(parsed)
    } else if tail.starts_with("invoke") || tail.starts_with("success") || tail.starts_with("failed") {
        return Err(());
    } else {
        return Ok(None);
    };
    let program = base58::decode(identity).map_err(|_| ())?;
    if program.len() != 32 || base58::encode(&program) != identity {
        return Err(());
    }
    Ok(Some((program, event)))
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
                    })
                    .into(),
                    account_keys: vec![a.to_vec(), b.to_vec(), c.to_vec()],
                    instructions: vec![pb::CompiledInstruction {
                        program_id_index: 0,
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .into(),
            })
            .into(),
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
            })
            .into(),
        }
    }

    #[test]
    fn successful_trace_proves_each_original_instruction() {
        let result = prove(&transaction()).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.values().all(|outcome| outcome.outcome == InvocationOutcome::Succeeded));
    }

    #[test]
    fn caught_parent_failure_revokes_preceding_successful_event() {
        let mut tx = transaction();
        tx.meta.as_option_mut().unwrap().log_messages[4] = format!("Program {} failed: caught inner call ended with success", base58::encode([2; 32]));
        let result = prove(&tx).unwrap();
        assert_eq!(
            result[&InstructionPosition {
                instruction_index: 0,
                inner_instruction_index: None
            }]
                .outcome,
            InvocationOutcome::Succeeded
        );
        assert_eq!(
            result[&InstructionPosition {
                instruction_index: 0,
                inner_instruction_index: Some(1)
            }]
                .outcome,
            InvocationOutcome::Failed
        );
    }

    #[test]
    fn truncated_unknown_depth_and_misaligned_traces_never_prove_success() {
        for mutation in 0..3 {
            let mut tx = transaction();
            match mutation {
                0 => {
                    tx.meta.as_option_mut().unwrap().log_messages.pop();
                }
                1 => tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[0].stack_height = None,
                2 => tx.meta.as_option_mut().unwrap().log_messages[1] = format!("Program {} invoke [2]", base58::encode([9; 32])),
                _ => unreachable!(),
            }
            assert!(prove(&tx).is_none());
        }
    }
}

#[cfg(test)]
mod exact_log_tests {
    use super::*;
    #[test]
    fn failure_suffix_cannot_be_mistaken_for_success() {
        let key = base58::encode([2; 32]);
        assert!(matches!(
            parse_log(&format!("Program {key} failed: custom error ending in success")).unwrap(),
            Some((_, Event::Failure))
        ));
    }
    #[test]
    fn malformed_runtime_grammar_is_rejected() {
        let key = base58::encode([2; 32]);
        for tail in ["invoke [2] extra", "invoke [02]", "invoke [0]", "success extra", "failed:"] {
            assert!(parse_log(&format!("Program {key} {tail}")).is_err());
        }
        assert!(parse_log("Program log: Program fake success").unwrap().is_none());
    }
}
