//! Latest Solana/Buffa and legacy Prost models feed the same borrowed projector.
mod common;

use buffa::{Message, MessageField};
use common::*;
use spl_transfers::{project, Coverage};
use substreams_solana_buffa::pb::sf::solana::r#type::v1 as buffa_pb;

spl_transfers::block_view_buffa!(fn view_buffa_block, buffa_pb);

fn buffa_block(fixture: &[u8]) -> buffa_pb::Block {
    buffa_pb::Block::decode_from_slice(&gunzip(fixture)).unwrap()
}

fn assert_equivalent(prost: &pb::Block, buffa: &buffa_pb::Block) {
    assert_eq!(view_block(prost), view_buffa_block(buffa).unwrap());
    for coverage in [Coverage::TRANSFER_CHECKED, Coverage::ALL] {
        assert_eq!(project(&view_block(prost), coverage), project(&view_buffa_block(buffa).unwrap(), coverage));
    }
}

#[test]
fn every_pinned_block_has_identical_borrowed_fields_and_projection() {
    for fixture in [BLOCK_349019252, BLOCK_350000009, BLOCK_350000010, BLOCK_350000026] {
        let prost = decode_block(fixture);
        let buffa = buffa_block(fixture);
        assert_equivalent(&prost, &buffa);

        let source = &buffa.transactions[0].transaction;
        let view = view_buffa_block(&buffa).unwrap();
        assert!(std::ptr::eq(view.transactions[0].signatures[0], source.signatures[0].as_slice()));
        assert!(std::ptr::eq(
            view.transactions[0].message.as_ref().unwrap().account_keys[0],
            source.message.account_keys[0].as_slice()
        ));
    }
}

#[test]
fn absent_required_messages_never_become_default_instances() {
    let prost = decode_block(BLOCK_350000009);
    let buffa = buffa_block(BLOCK_350000009);
    for field in ["transaction", "message", "header", "meta"] {
        let mut old = prost.clone();
        let mut new = buffa.clone();
        let old_tx = &mut old.transactions[1366];
        let new_tx = &mut new.transactions[1366];
        match field {
            "transaction" => {
                old_tx.transaction = None;
                new_tx.transaction = MessageField::none();
            }
            "message" => {
                old_tx.transaction.as_mut().unwrap().message = None;
                new_tx.transaction.as_option_mut().unwrap().message = MessageField::none();
            }
            "header" => {
                old_tx.transaction.as_mut().unwrap().message.as_mut().unwrap().header = None;
                new_tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().header = MessageField::none();
            }
            "meta" => {
                old_tx.meta = None;
                new_tx.meta = MessageField::none();
            }
            _ => unreachable!(),
        }
        assert_equivalent(&old, &new);
        assert!(project(&view_buffa_block(&new).unwrap(), Coverage::ALL).is_err(), "missing {field}");
    }
}

#[test]
fn present_default_error_still_marks_a_transaction_failed() {
    let mut old = decode_block(BLOCK_350000009);
    let mut new = buffa_block(BLOCK_350000009);
    old.transactions[1366].meta.as_mut().unwrap().err = Some(pb::TransactionError::default());
    new.transactions[1366].meta.as_option_mut().unwrap().err = MessageField::some(buffa_pb::TransactionError::default());
    assert_equivalent(&old, &new);
    let result = project(&view_buffa_block(&new).unwrap(), Coverage::ALL).unwrap();
    assert!(result.operations.iter().all(|operation| operation.transaction_index != 1366));
}

#[test]
fn optional_clock_fields_distinguish_absent_from_present_zero() {
    let mut old = decode_block(BLOCK_350000009);
    let mut new = buffa_block(BLOCK_350000009);
    old.block_time = None;
    new.block_time = MessageField::none();
    old.block_height = None;
    new.block_height = MessageField::none();
    assert_equivalent(&old, &new);
    assert_eq!(
        (view_buffa_block(&new).unwrap().block_time, view_buffa_block(&new).unwrap().block_height),
        (None, None)
    );

    old.block_time = Some(pb::UnixTimestamp::default());
    new.block_time = MessageField::some(buffa_pb::UnixTimestamp::default());
    old.block_height = Some(pb::BlockHeight::default());
    new.block_height = MessageField::some(buffa_pb::BlockHeight::default());
    assert_equivalent(&old, &new);
    assert_eq!(
        (view_buffa_block(&new).unwrap().block_time, view_buffa_block(&new).unwrap().block_height),
        (Some(0), Some(0))
    );
}

#[test]
fn only_legacy_and_v0_message_shapes_are_admitted() {
    let original = buffa_block(BLOCK_350000009);
    for (version, versioned) in [(None, false), (None, true), (Some(0), true)] {
        let mut block = original.clone();
        let message = block.transactions[1366].transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
        message.version = version;
        message.versioned = versioned;
        assert!(view_buffa_block(&block).is_ok());
    }
    for (version, versioned, inline_configuration) in [
        (Some(1), true, false),
        (Some(u32::MAX), true, false),
        (Some(0), false, false),
        (None, false, true),
        (Some(0), true, true),
    ] {
        for failed in [false, true] {
            let mut block = original.clone();
            let transaction = &mut block.transactions[1366];
            let message = transaction.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
            message.version = version;
            message.versioned = versioned;
            if inline_configuration {
                message.transaction_config = MessageField::some(buffa_pb::TransactionConfig::default());
            }
            if failed {
                transaction.meta.as_option_mut().unwrap().err = MessageField::some(buffa_pb::TransactionError::default());
            }
            assert_eq!(
                view_buffa_block(&block).unwrap_err(),
                spl_transfers::UnsupportedMessage {
                    transaction_index: 1366,
                    version,
                    versioned,
                    inline_configuration
                }
            );
        }
    }
}
