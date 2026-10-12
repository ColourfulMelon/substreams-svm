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
    for log in tx.meta.as_ref().into_iter().flat_map(|meta| &meta.log_messages) {
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
    if truncated && tx.meta.as_ref().is_some_and(|meta| meta.err.is_none()) {
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

pub(crate) enum ProgramLog<'a> {
    Enter { invoke_depth: Option<u32> },
    Data(&'a str),
    Exit,
}

pub(crate) fn scoped_program_log<'a>(log_message: &'a str, program_id: &[u8], is_invoked: &mut bool) -> Option<ProgramLog<'a>> {
    let matches_program = parse_program_id(log_message).map_or(false, |id| id == program_id);

    if is_invoke(log_message) && matches_program {
        *is_invoked = true;
        return Some(ProgramLog::Enter {
            invoke_depth: parse_invoke_depth(log_message),
        });
    }

    if matches_program && (is_success(log_message) || is_failed(log_message)) {
        *is_invoked = false;
        return Some(ProgramLog::Exit);
    }

    if !*is_invoked {
        return None;
    }

    Some(ProgramLog::Data(log_message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

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
        partial_suffix.meta.as_mut().unwrap().log_messages.extend([
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
            let meta = altered.meta.as_mut().unwrap();
            match mode {
                0 => meta.err = Some(Default::default()),
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
