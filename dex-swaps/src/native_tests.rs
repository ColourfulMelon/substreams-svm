use crate::{map_events, native::Collector, pb, project, project_with_limits, InstructionPosition, Limits, ProjectError, Provenance, SwapKind};
use flate2::read::GzDecoder;
use prost::Message;
use std::io::Read;
use substreams_solana::pb::sf::solana::r#type::v1 as source;

fn fixture() -> source::Block {
    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("../fixtures/solana-mainnet-350000026.pb.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    source::Block::decode(bytes.as_slice()).unwrap()
}

#[test]
fn complete_block_native_projection_preserves_legacy_fields_and_original_positions() {
    let block = fixture();
    let projection = project(&block).unwrap();
    assert!(!projection.transactions.is_empty());
    let mut instructions = 0;
    let mut logs = 0;
    let mut summaries = 0;
    for transaction in &projection.transactions {
        let original = &block.transactions[transaction.transaction_index as usize];
        assert_eq!(original.meta.as_ref().unwrap().err, None);
        assert_eq!(transaction.signature, original.transaction.as_ref().unwrap().signatures[0]);
        for row in &transaction.swaps {
            summaries += usize::from(row.kind == SwapKind::AggregatorSummary);
            match row.provenance {
                Provenance::Instruction { position, .. } => {
                    instructions += 1;
                    let message = original.transaction.as_ref().unwrap().message.as_ref().unwrap();
                    assert!((position.instruction_index as usize) < message.instructions.len());
                    if let Some(inner) = position.inner_instruction_index {
                        let group = original
                            .meta
                            .as_ref()
                            .unwrap()
                            .inner_instructions
                            .iter()
                            .find(|group| group.index == position.instruction_index)
                            .unwrap();
                        assert!((inner as usize) < group.instructions.len());
                    }
                }
                Provenance::Log { log_index, .. } => {
                    logs += 1;
                    assert!((log_index as usize) < original.meta.as_ref().unwrap().log_messages.len());
                }
            }
        }
    }
    assert!(instructions > 0 && logs > 0 && summaries > 0);
    assert_eq!(projection.clock.slot, 350000026);
    assert_eq!(projection.clock.transaction_count, block.transactions.len() as u32);
    let qualified = projection
        .transactions
        .iter()
        .flat_map(|tx| &tx.swaps)
        .filter(|row| row.invocation_outcome == crate::InvocationOutcome::Succeeded && row.swap.protocol == pb::Protocol::MeteoraDlmm as i32)
        .count();
    assert_eq!(qualified, 5);
    let native = projection.into_legacy();
    let wrapper = map_events(block).unwrap();
    assert_eq!(native, wrapper);
    assert_eq!(native.encode_to_vec(), wrapper.encode_to_vec());
    assert_eq!(native.transactions.len(), 156);
    assert_eq!(native.transactions.iter().map(|tx| tx.swaps.len()).sum::<usize>(), 231);
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(native.encode_to_vec())),
        "c1e611c718878724d851e446c0a13bec1b28e682ebf99e2ab7b4a2d221b78218"
    );
}

use sha2::Digest;

#[test]
fn empty_missing_time_and_skipped_slot_block_has_a_clock() {
    let mut block = fixture();
    block.transactions.clear();
    block.block_time = None;
    block.parent_slot = block.slot - 5;
    let projection = project(&block).unwrap();
    assert!(projection.transactions.is_empty());
    assert_eq!(projection.clock.block_time, None);
    assert_eq!(projection.clock.parent_slot, block.slot - 5);
}

#[test]
fn structural_failure_and_capacity_return_no_partial_projection() {
    let mut block = fixture();
    let success = block
        .transactions
        .iter()
        .position(|tx| tx.meta.as_ref().is_some_and(|meta| meta.err.is_none()))
        .unwrap();
    block.transactions[success]
        .transaction
        .as_mut()
        .unwrap()
        .message
        .as_mut()
        .unwrap()
        .instructions
        .push(source::CompiledInstruction {
            program_id_index: u32::MAX,
            ..Default::default()
        });
    assert!(matches!(project(&block), Err(ProjectError::InvalidBlock(_))));
    let block = fixture();
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_transactions: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_instructions: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_log_messages: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_log_bytes: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
    assert!(matches!(
        project_with_limits(
            &block,
            Limits {
                max_swaps: 1,
                ..Default::default()
            }
        ),
        Err(ProjectError::Capacity)
    ));
}

#[test]
fn collector_keeps_distinct_event_positions_and_aggregator_kind() {
    let mut collector = Collector::new(10);
    let position = InstructionPosition {
        instruction_index: 8,
        inner_instruction_index: Some(3),
    };
    collector.at_instruction(position);
    collector.push(pb::Swap {
        protocol: pb::Protocol::MeteoraDlmm as i32,
        ..Default::default()
    });
    collector.push(pb::Swap {
        protocol: pb::Protocol::JupiterV6 as i32,
        ..Default::default()
    });
    collector.at_log(99);
    collector.push(pb::Swap {
        protocol: pb::Protocol::OkxDex as i32,
        ..Default::default()
    });
    let rows = collector.finish().unwrap();
    assert_eq!(rows[0].provenance, Provenance::Instruction { position, event_index: 0 });
    assert_eq!(rows[1].provenance, Provenance::Instruction { position, event_index: 1 });
    assert_eq!(rows[2].provenance, Provenance::Log { log_index: 99, event_index: 0 });
    assert_eq!(rows[0].kind, SwapKind::PoolExecution);
    assert_eq!(rows[1].kind, SwapKind::AggregatorSummary);
    assert_eq!(rows[2].kind, SwapKind::AggregatorSummary);
}
