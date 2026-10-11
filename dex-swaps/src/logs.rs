use common::solana::{is_failed, is_invoke, is_success, parse_invoke_depth, parse_program_id};

/// A successful transaction can catch a failed CPI. Only acknowledge an
/// invocation after its entire ancestor chain returns successfully. Missing
/// or truncated logs cannot prove success.
pub(crate) fn invocation_outcomes(tx: &substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction) -> InvocationOutcomes {
    let instructions: Vec<_> = tx.walk_instructions().collect();
    let mut cursor = 0;
    let mut stack: Vec<(String, Vec<u32>, bool)> = Vec::new();
    let mut outcomes = InvocationOutcomes::default();
    for log in tx.meta.as_ref().into_iter().flat_map(|meta| &meta.log_messages) {
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
