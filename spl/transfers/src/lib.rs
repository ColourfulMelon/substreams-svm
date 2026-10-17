//! Native, storage-independent projection of SPL token transfers from complete
//! Solana blocks.
//!
//! [`project`] validates a complete `sf.solana.type.v1.Block` structurally and
//! returns the token operations covered by a declared [`Coverage`], each with
//! its original transaction, outer instruction and inner instruction index.
//! It needs no Substreams runtime, store, network access or protobuf runtime:
//! callers pass a [`model::Block`] that borrows from whichever generated model
//! they already decoded, built with [`block_view!`] for Prost or
//! [`block_view_buffa!`] for Buffa. The `spl_transfers`
//! Substreams package is a thin wrapper over the same function.
//!
//! ```ignore
//! spl_transfers::block_view!(fn view_block, firehose_protos::sf::solana::r#type::v1);
//!
//! let projection = spl_transfers::project(&view_block(&block), spl_transfers::Coverage::TRANSFER_CHECKED)?;
//! for operation in &projection.operations {
//!     // operation.transaction_index, operation.instruction_index, operation.amount, ...
//! }
//! ```
//!
//! [`decode`] is the instruction decoder shared with `spl-token-common`, so the
//! `spl_token` Substreams package and the projection agree on account roles.

pub mod decode;
mod error;
pub mod model;
mod project;
mod validate;

pub use decode::{decode, DecodeError, Decoded, OperationKind, TokenProgram, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
pub use error::{Position, ProjectError, TransactionError, UnsupportedMessage};
pub use project::{project, Clock, Coverage, CoverageError, Operation, Projection, Pubkey, Signature, SPEC_REVISION};
pub use validate::{validate_block, validate_transaction, Keys};
