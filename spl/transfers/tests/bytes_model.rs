//! The projector over a `bytes::Bytes` model, as firehose-protos generates with
//! prost 0.14 and `.bytes(".")`: same `block_view!`, same records, no re-encode.

mod common;

use common::*;
use prost::Message;
use spl_transfers::{project, Coverage};

/// Mirror of `sf.solana.type.v1` with `Bytes` fields, restricted to the fields
/// `block_view!` reads (prost skips the others while decoding).
mod bytes_pb {
    use prost::bytes::Bytes;

    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Block {
        #[prost(string, tag = "1")]
        pub previous_blockhash: String,
        #[prost(string, tag = "2")]
        pub blockhash: String,
        #[prost(uint64, tag = "3")]
        pub parent_slot: u64,
        #[prost(message, repeated, tag = "4")]
        pub transactions: Vec<ConfirmedTransaction>,
        #[prost(message, optional, tag = "6")]
        pub block_time: Option<UnixTimestamp>,
        #[prost(message, optional, tag = "7")]
        pub block_height: Option<BlockHeight>,
        #[prost(uint64, tag = "20")]
        pub slot: u64,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct ConfirmedTransaction {
        #[prost(message, optional, tag = "1")]
        pub transaction: Option<Transaction>,
        #[prost(message, optional, tag = "2")]
        pub meta: Option<TransactionStatusMeta>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Transaction {
        #[prost(bytes = "bytes", repeated, tag = "1")]
        pub signatures: Vec<Bytes>,
        #[prost(message, optional, tag = "2")]
        pub message: Option<Message>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Message {
        #[prost(message, optional, tag = "1")]
        pub header: Option<MessageHeader>,
        #[prost(bytes = "bytes", repeated, tag = "2")]
        pub account_keys: Vec<Bytes>,
        #[prost(message, repeated, tag = "4")]
        pub instructions: Vec<CompiledInstruction>,
        #[prost(message, repeated, tag = "6")]
        pub address_table_lookups: Vec<MessageAddressTableLookup>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct MessageHeader {
        #[prost(uint32, tag = "1")]
        pub num_required_signatures: u32,
        #[prost(uint32, tag = "2")]
        pub num_readonly_signed_accounts: u32,
        #[prost(uint32, tag = "3")]
        pub num_readonly_unsigned_accounts: u32,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct MessageAddressTableLookup {
        #[prost(bytes = "bytes", tag = "1")]
        pub account_key: Bytes,
        #[prost(bytes = "bytes", tag = "2")]
        pub writable_indexes: Bytes,
        #[prost(bytes = "bytes", tag = "3")]
        pub readonly_indexes: Bytes,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct TransactionStatusMeta {
        #[prost(message, optional, tag = "1")]
        pub err: Option<TransactionError>,
        #[prost(message, repeated, tag = "5")]
        pub inner_instructions: Vec<InnerInstructions>,
        #[prost(message, repeated, tag = "7")]
        pub pre_token_balances: Vec<TokenBalance>,
        #[prost(message, repeated, tag = "8")]
        pub post_token_balances: Vec<TokenBalance>,
        #[prost(bytes = "bytes", repeated, tag = "12")]
        pub loaded_writable_addresses: Vec<Bytes>,
        #[prost(bytes = "bytes", repeated, tag = "13")]
        pub loaded_readonly_addresses: Vec<Bytes>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct TransactionError {
        #[prost(bytes = "bytes", tag = "1")]
        pub err: Bytes,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct InnerInstructions {
        #[prost(uint32, tag = "1")]
        pub index: u32,
        #[prost(message, repeated, tag = "2")]
        pub instructions: Vec<InnerInstruction>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct InnerInstruction {
        #[prost(uint32, tag = "1")]
        pub program_id_index: u32,
        #[prost(bytes = "bytes", tag = "2")]
        pub accounts: Bytes,
        #[prost(bytes = "bytes", tag = "3")]
        pub data: Bytes,
        #[prost(uint32, optional, tag = "4")]
        pub stack_height: Option<u32>,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct CompiledInstruction {
        #[prost(uint32, tag = "1")]
        pub program_id_index: u32,
        #[prost(bytes = "bytes", tag = "2")]
        pub accounts: Bytes,
        #[prost(bytes = "bytes", tag = "3")]
        pub data: Bytes,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct TokenBalance {
        #[prost(uint32, tag = "1")]
        pub account_index: u32,
        #[prost(string, tag = "2")]
        pub mint: String,
        #[prost(string, tag = "4")]
        pub owner: String,
        #[prost(string, tag = "5")]
        pub program_id: String,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct UnixTimestamp {
        #[prost(int64, tag = "1")]
        pub timestamp: i64,
    }
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct BlockHeight {
        #[prost(uint64, tag = "1")]
        pub block_height: u64,
    }
}

spl_transfers::block_view!(fn view_bytes_block, bytes_pb);

#[test]
fn bytes_model_projects_identically() {
    for fixture in [BLOCK_349019252, BLOCK_350000009, BLOCK_350000010, BLOCK_350000026] {
        let encoded = prost::bytes::Bytes::from(gunzip(fixture));
        let vec_block = pb::Block::decode(encoded.clone()).unwrap();
        // Decoding from `Bytes` lets byte fields share the input allocation.
        let bytes_block = bytes_pb::Block::decode(encoded.clone()).unwrap();
        let signature = &bytes_block.transactions[0].transaction.as_ref().unwrap().signatures[0];
        assert!(encoded.as_ptr_range().contains(&signature.as_ptr()), "zero-copy byte fields");

        assert_eq!(view_bytes_block(&bytes_block), view_block(&vec_block));
        for coverage in [Coverage::TRANSFER_CHECKED, Coverage::ALL] {
            assert_eq!(project(&view_bytes_block(&bytes_block), coverage), project(&view_block(&vec_block), coverage));
        }
    }
}
