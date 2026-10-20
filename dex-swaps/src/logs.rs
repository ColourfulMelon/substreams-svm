use common::solana::{is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_id};

/// A successful transaction can catch a failed CPI. Only acknowledge an
/// invocation after its entire ancestor chain returns successfully. The
/// transaction's successful status proves a root's commitment when the runtime
/// explicitly truncates logs; unfinished inner frames remain unproven.
pub(crate) fn invocation_outcomes(tx: &substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction) -> InvocationOutcomes {
    let instructions: Vec<_> = tx.walk_instructions().collect();
    let mut cursor = 0;
    let mut stack: Vec<(String, Vec<u32>, bool)> = Vec::new();
    let mut outcomes = InvocationOutcomes::default();
    let mut truncated = false;
    for log in tx.meta.as_option().into_iter().flat_map(|meta| &meta.log_messages) {
        if log == "Log truncated" {
            // The collector can retain shorter messages after dropping a
            // larger one. Those later logs no longer prove a complete trace.
            truncated = true;
            break;
        }
        let words: Vec<_> = log.split_whitespace().collect();
        if words.first() != Some(&"Program") || words.len() < 3 {
            continue;
        }
        if words[2] == "invoke" && words.len() == 4 {
            let Some(depth) = words[3]
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .and_then(|value| value.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(program) = substreams_solana::base58::decode(words[1]) else { continue };
            let matched = instructions
                .iter()
                .enumerate()
                .skip(cursor)
                .find(|(_, ix)| ix.program_id().0 == &program && (if ix.is_root() { 1 } else { ix.stack_height() }) == depth)
                .map(|(index, _)| index);
            let valid = matched.is_some() && depth as usize == stack.len() + 1 && stack.iter().all(|entry| entry.2);
            if let Some(index) = matched {
                cursor = index + 1;
            }
            stack.push((words[1].to_owned(), matched.map(|index| vec![index as u32]).unwrap_or_default(), valid));
        } else if words[2] == "success" || words[2] == "failed:" {
            if stack.last().map(|entry| entry.0.as_str()) != Some(words[1]) {
                stack.clear();
                continue;
            }
            let (_, descendants, valid) = stack.pop().unwrap();
            if words[2] == "success" && valid {
                if let Some(parent) = stack.last_mut() {
                    parent.1.extend(descendants);
                } else {
                    outcomes.completed.extend(descendants);
                }
            } else if words[2] == "failed:" && valid {
                // Completed children roll back with their failed ancestor.
                outcomes.rolled_back.extend(descendants);
            }
        }
    }
    if truncated && tx.meta.as_option().is_some_and(|meta| meta.err.is_unset()) {
        if let Some((_, committed_children, true)) = stack.first() {
            // A successful transaction cannot roll back its root. Its already
            // completed children are final even if a later sibling exhausts
            // the log budget. Pending inner-frame children are deliberately
            // excluded: that ancestor may fail and be caught after truncation.
            outcomes.completed.extend(committed_children.iter().copied());
        }
    }
    outcomes
}

#[derive(Default)]
pub(crate) struct InvocationOutcomes {
    pub completed: std::collections::HashSet<u32>,
    pub rolled_back: std::collections::HashSet<u32>,
}

pub(crate) fn successful_invocations(tx: &substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction) -> std::collections::HashSet<u32> {
    invocation_outcomes(tx).completed
}

#[derive(Debug, PartialEq)]
pub(crate) enum ProgramLog<'a> {
    Enter { invoke_depth: Option<u32> },
    Data(&'a str),
    Exit,
}

/// Parse each transaction log once before sharing it with the protocol states.
/// Keep the existing permissive header matching: scope changes, malformed
/// depths and foreign-program data must have exactly the same meaning.
pub(crate) struct ParsedLog<'a> {
    pub(crate) message: &'a str,
    pub(crate) program_id: Option<Vec<u8>>,
    pub(crate) invoke: bool,
    invoke_depth: Option<u32>,
    exit: bool,
}

impl<'a> ParsedLog<'a> {
    pub(crate) fn new(message: &'a str) -> Self {
        let program_id = parse_program_id(message);
        let invoke = program_id.is_some() && is_invoke(message);
        let invoke_depth = if invoke { parse_invoke_depth(message) } else { None };
        let exit = program_id.is_some() && !invoke && (is_success(message) || is_failed(message));
        Self { message, program_id, invoke, invoke_depth, exit }
    }
}

pub(crate) fn scoped_program_log<'a>(log: &ParsedLog<'a>, program_id: &[u8], is_invoked: &mut bool) -> Option<ProgramLog<'a>> {
    let matches_program = log.program_id.as_deref() == Some(program_id);

    if log.invoke && matches_program {
        *is_invoked = true;
        return Some(ProgramLog::Enter { invoke_depth: log.invoke_depth });
    }

    if matches_program && log.exit {
        *is_invoked = false;
        return Some(ProgramLog::Exit);
    }

    if !*is_invoked {
        return None;
    }

    Some(ProgramLog::Data(log.message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn shared_log_headers_preserve_independent_protocol_scopes() {
        let first = [0; 32];
        let second = substreams_solana_idls::spl::token::PROGRAM_ID;
        let first_id = base58::encode(first);
        let second_id = base58::encode(second);
        let mut first_active = false;
        let mut second_active = false;
        // Foreign headers remain data for an already active protocol. This
        // deliberately preserves the existing handler semantics; commitment
        // and rolled-back CPI filtering still belong to invocation_outcomes.
        for (message, first_event, second_event) in [
            ("Program data: outside".to_string(), None, None),
            (format!("Program {first_id} invoke [1]"), Some(0), None),
            (format!("Program {second_id} invoke [2]"), Some(1), Some(0)),
            ("Program data: nested".to_string(), Some(1), Some(1)),
            (format!("Program {second_id} failed: custom program error: 1"), Some(1), Some(2)),
            ("Log truncated".to_string(), Some(1), None),
            (format!("Program {first_id} success  "), Some(2), None),
            ("Program data: outside".to_string(), None, None),
        ] {
            let log = ParsedLog::new(&message);
            for (program, active, event, depth) in [
                (first.as_slice(), &mut first_active, first_event, 1),
                (second.as_slice(), &mut second_active, second_event, 2),
            ] {
                let expected = match event {
                    Some(0) => Some(ProgramLog::Enter { invoke_depth: Some(depth) }),
                    Some(1) => Some(ProgramLog::Data(message.as_str())),
                    Some(2) => Some(ProgramLog::Exit),
                    _ => None,
                };
                assert_eq!(scoped_program_log(&log, program, active), expected);
            }
        }
        assert!(!first_active && !second_active);
    }

    #[test]
    fn shared_log_headers_keep_malformed_depths_and_invalid_ids_compatible() {
        let program = [0; 32];
        let id = base58::encode(program);
        for message in [format!("Program {id} invoke [bad]"), format!("Program {id} invoke [1")] {
            let mut active = false;
            assert_eq!(scoped_program_log(&ParsedLog::new(&message), &program, &mut active),
                Some(ProgramLog::Enter { invoke_depth: None }));
            assert!(active);
        }
        let mut active = true;
        for message in ["Program invalid invoke [1]", "Program invalid success", "Program log: failed: custom error"] {
            assert_eq!(scoped_program_log(&ParsedLog::new(message), &program, &mut active),
                Some(ProgramLog::Data(message)));
            assert!(active);
        }
    }

    #[test]
    fn runtime_truncation_keeps_completed_root_children_but_not_pending_inner_work() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(
            &include_bytes!("../fixtures/native-venues/spl-swap-recent-1.transaction.pb").to_vec(),
        ).unwrap();
        let outcome = invocation_outcomes(&tx);
        for index in [32, 33, 34] { assert!(outcome.completed.contains(&index)); }
        // A later DAMM invocation and its completed Token children remain in
        // an unfinished inner frame, which could roll back after truncation.
        for index in [36, 37, 38] { assert!(!outcome.completed.contains(&index)); }
        let mut partial_suffix = tx.clone();
        let pending = base58::encode(tx.walk_instructions().nth(36).unwrap().program_id().0);
        partial_suffix.meta.as_option_mut().unwrap().log_messages.extend([
            format!("Program {pending} success"),
            "Program log: short message after truncation".into(),
        ]);
        let outcome = invocation_outcomes(&partial_suffix);
        for index in [32, 33, 34] { assert!(outcome.completed.contains(&index)); }
        for index in [36, 37, 38] { assert!(!outcome.completed.contains(&index)); }
        let native = base58::encode(substreams_solana_idls::spl::token_swap::PROGRAM_ID);
        let root = tx.walk_instructions().take(33).filter(|ix| ix.is_root()).last().unwrap().program_id().0.clone();
        for mode in 0..7 {
            let mut altered = tx.clone();
            let meta = altered.meta.as_option_mut().unwrap();
            match mode {
                0 => meta.err = buffa::MessageField::some(Default::default()),
                1 => { meta.log_messages.pop(); }
                2 => *meta.log_messages.last_mut().unwrap() = "Program log: Log truncated".into(),
                3 => {
                    let end = meta.log_messages.iter().position(|log| log == &format!("Program {native} success")).unwrap();
                    meta.log_messages.truncate(end);
                    meta.log_messages.push("Log truncated".into());
                }
                4 | 5 => {
                    let end = meta.log_messages.iter().position(|log| log == &format!("Program {native} success")).unwrap();
                    meta.log_messages.truncate(end + 1);
                    if mode == 4 {
                        meta.log_messages.push(format!("Program {} failed: custom program error: 1", base58::encode(&root)));
                    } else {
                        meta.log_messages[end] = format!("Program {native} failed: custom program error: 1");
                    }
                    meta.log_messages.push("Log truncated".into());
                }
                _ => {
                    let invoke = format!("Program {} invoke [1]", base58::encode(&root));
                    // The router also has an earlier, fully returned root
                    // call. Corrupt the root containing this swap.
                    let position = meta.log_messages.iter().rposition(|log| log == &invoke).unwrap();
                    meta.log_messages[position] = invoke.replace("[1]", "[4]");
                }
            }
            let outcome = invocation_outcomes(&altered);
            for index in [32, 33, 34] { assert!(!outcome.completed.contains(&index), "mode {mode}, index {index}"); }
            if matches!(mode, 4 | 5) { assert!(outcome.rolled_back.contains(&32)); }
        }
    }
}
