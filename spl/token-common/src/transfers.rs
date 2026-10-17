use proto::pb::solana::spl::token::v1 as pb;
use spl_transfers::{DecodeError, OperationKind};
use substreams_solana::block_view::InstructionView;

/// Decodes amount-moving instructions with the decoder shared with the native
/// `spl_transfers` projector. Zero amounts are kept, unchecked `Transfer`
/// leaves the mint unknown, and mints and burns get their own kinds.
pub fn unpack_transfers(
    instruction: &InstructionView,
    program_id: &[u8],
    is_spl_token_program: fn(&[u8]) -> bool,
) -> Result<Option<pb::instruction::Instruction>, DecodeError> {
    if !is_spl_token_program(program_id) {
        return Ok(None);
    }
    let accounts = instruction.accounts();
    let accounts: Vec<&[u8]> = accounts.iter().map(|account| account.0.as_slice()).collect();
    let Some(decoded) = spl_transfers::decode(instruction.data(), &accounts)? else {
        return Ok(None);
    };
    let transfer = pb::Transfer {
        // authority
        authority: decoded.authority.to_vec(),
        multisig_authority: decoded.extra_accounts.iter().map(|account| account.to_vec()).collect(),

        // event
        source: decoded.source.map(<[u8]>::to_vec).unwrap_or_default(),
        destination: decoded.destination.map(<[u8]>::to_vec).unwrap_or_default(),
        amount: decoded.amount,
        mint: decoded.mint.map(<[u8]>::to_vec),
        decimals: decoded.decimals.map(u32::from),
    };
    Ok(Some(match decoded.kind {
        OperationKind::Transfer | OperationKind::TransferChecked => pb::instruction::Instruction::Transfer(transfer.into()),
        OperationKind::MintTo | OperationKind::MintToChecked => pb::instruction::Instruction::Mint(transfer.into()),
        OperationKind::Burn | OperationKind::BurnChecked => pb::instruction::Instruction::Burn(transfer.into()),
    }))
}
