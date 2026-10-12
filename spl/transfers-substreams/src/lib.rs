//! `spl_transfers:map_events`: a thin Substreams wrapper over the native
//! [`spl_transfers::project`]. The module params are a [`Coverage`] flag list.
use proto::pb::solana::spl::transfers::v1 as pb;
use spl_transfers::{Coverage, OperationKind, Projection, TokenProgram};
use substreams::errors::Error;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

spl_transfers::block_view!(pub fn view_block, substreams_solana::pb::sf::solana::r#type::v1);

/// The module body, callable natively for parity checks.
pub fn map_events_impl(params: &str, block: &Block) -> Result<pb::Events, Error> {
    let coverage = Coverage::parse(params)?;
    let projection = spl_transfers::project(&view_block(block), coverage)?;
    Ok(to_proto(&projection))
}

pub fn to_proto(projection: &Projection) -> pb::Events {
    let clock = &projection.clock;
    pb::Events {
        clock: Some(pb::Clock {
            slot: clock.slot,
            parent_slot: clock.parent_slot,
            blockhash: clock.blockhash.to_vec(),
            previous_blockhash: clock.previous_blockhash.to_vec(),
            block_time: clock.block_time,
            block_height: clock.block_height,
            transaction_count: clock.transaction_count,
            coverage: Some(pb::Coverage {
                token_2022: clock.coverage.token_2022,
                inner_instructions: clock.coverage.inner_instructions,
                unchecked_transfers: clock.coverage.unchecked_transfers,
                mint_burn: clock.coverage.mint_burn,
            }),
            spec_revision: clock.spec_revision,
        }),
        operations: projection
            .operations
            .iter()
            .map(|operation| pb::Operation {
                transaction_index: operation.transaction_index,
                instruction_index: operation.instruction_index,
                inner_instruction_index: operation.inner_instruction_index,
                stack_height: operation.stack_height,
                signature: operation.signature.to_vec(),
                program: match operation.program {
                    TokenProgram::Token => pb::TokenProgram::Token,
                    TokenProgram::Token2022 => pb::TokenProgram::Token2022,
                }
                .into(),
                kind: match operation.kind {
                    OperationKind::Transfer => pb::OperationKind::Transfer,
                    OperationKind::TransferChecked => pb::OperationKind::TransferChecked,
                    OperationKind::MintTo => pb::OperationKind::MintTo,
                    OperationKind::MintToChecked => pb::OperationKind::MintToChecked,
                    OperationKind::Burn => pb::OperationKind::Burn,
                    OperationKind::BurnChecked => pb::OperationKind::BurnChecked,
                }
                .into(),
                amount: operation.amount,
                decimals: operation.decimals.map(u32::from),
                mint: operation.mint.map(|key| key.to_vec()),
                source: operation.source.map(|key| key.to_vec()),
                destination: operation.destination.map(|key| key.to_vec()),
                authority: operation.authority.to_vec(),
                extra_accounts: operation.extra_accounts.iter().map(|key| key.to_vec()).collect(),
                source_owner: operation.source_owner.map(|key| key.to_vec()),
                destination_owner: operation.destination_owner.map(|key| key.to_vec()),
            })
            .collect(),
    }
}

// Exported only in the WASM build, so native crates can link this one.
#[cfg(target_arch = "wasm32")]
mod handler {
    use super::*;

    #[substreams::handlers::map]
    fn map_events(params: String, block: Block) -> Result<pb::Events, Error> {
        map_events_impl(&params, &block)
    }
}
