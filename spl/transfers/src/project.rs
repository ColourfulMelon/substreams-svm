use crate::decode::{decode, OperationKind, TokenProgram};
use crate::error::{Position, ProjectError, TransactionError};
use crate::model::{Block, InnerInstructions, Instruction, Meta, TokenBalance, Transaction};
use crate::validate::{decode_key, validate, Keys};
use std::fmt;

/// Revision of the projection semantics, carried on every [`Clock`].
pub const SPEC_REVISION: u32 = 1;

pub type Pubkey = [u8; 32];
pub type Signature = [u8; 64];

/// Declared coverage: which operations a projection emits.
///
/// Every projection only covers executed instructions of successful
/// transactions. The default, [`Coverage::TRANSFER_CHECKED`], is the first
/// advertised scope: classic Token program `TransferChecked` in top-level
/// instructions. Each flag adds a separately declared class of operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Coverage {
    /// Token-2022 operations. Amounts are the requested ones: transfer fees
    /// withheld by the transfer-fee extension are not deducted.
    pub token_2022: bool,
    /// Inner (CPI) instructions.
    pub inner_instructions: bool,
    /// Unchecked `Transfer`, whose mint and decimals stay unknown.
    pub unchecked_transfers: bool,
    /// `MintTo`, `MintToChecked`, `Burn` and `BurnChecked`.
    pub mint_burn: bool,
}

impl Coverage {
    /// Classic Token program `TransferChecked` in top-level instructions.
    pub const TRANSFER_CHECKED: Self = Self {
        token_2022: false,
        inner_instructions: false,
        unchecked_transfers: false,
        mint_burn: false,
    };

    /// Every operation this crate decodes.
    pub const ALL: Self = Self {
        token_2022: true,
        inner_instructions: true,
        unchecked_transfers: true,
        mint_burn: true,
    };

    const FLAGS: [&'static str; 4] = ["token_2022", "inner_instructions", "unchecked_transfers", "mint_burn"];

    pub fn includes(&self, program: TokenProgram, kind: OperationKind, inner: bool) -> bool {
        let kind_covered = match kind {
            OperationKind::TransferChecked => true,
            OperationKind::Transfer => self.unchecked_transfers,
            OperationKind::MintTo | OperationKind::MintToChecked | OperationKind::Burn | OperationKind::BurnChecked => self.mint_burn,
        };
        kind_covered && (program == TokenProgram::Token || self.token_2022) && (!inner || self.inner_instructions)
    }

    /// Parses a comma-separated flag list: `token_2022`, `inner_instructions`,
    /// `unchecked_transfers`, `mint_burn`, or `all`. An empty list is
    /// [`Coverage::TRANSFER_CHECKED`].
    pub fn parse(params: &str) -> Result<Self, CoverageError> {
        let mut coverage = Self::TRANSFER_CHECKED;
        for flag in params.split(',').map(str::trim).filter(|flag| !flag.is_empty()) {
            match flag {
                "token_2022" => coverage.token_2022 = true,
                "inner_instructions" => coverage.inner_instructions = true,
                "unchecked_transfers" => coverage.unchecked_transfers = true,
                "mint_burn" => coverage.mint_burn = true,
                "all" => coverage = Self::ALL,
                unknown => return Err(CoverageError(unknown.to_string())),
            }
        }
        Ok(coverage)
    }

    /// Enabled flags, in [`Coverage::parse`] syntax.
    pub fn flags(&self) -> Vec<&'static str> {
        let enabled = [self.token_2022, self.inner_instructions, self.unchecked_transfers, self.mint_burn];
        Self::FLAGS.iter().zip(enabled).filter(|(_, on)| *on).map(|(flag, _)| *flag).collect()
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.flags().join(","))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageError(pub String);

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown coverage flag {:?}", self.0)
    }
}

impl std::error::Error for CoverageError {}

/// Identity and completeness evidence of a projected block. Every validated
/// block yields one, including blocks without operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clock {
    pub slot: u64,
    /// Parent slot; lower than `slot - 1` after skipped slots.
    pub parent_slot: u64,
    pub blockhash: [u8; 32],
    pub previous_blockhash: [u8; 32],
    /// Unix seconds; `None` when the block carries no time (never synthesized).
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
    /// All transactions of the block, votes and failures included.
    pub transaction_count: u32,
    pub coverage: Coverage,
    pub spec_revision: u32,
}

/// One amount-moving token instruction with its original position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    /// Index of the transaction in the block, votes and failures included.
    pub transaction_index: u32,
    /// Index of the outer (top-level) instruction.
    pub instruction_index: u32,
    /// Index inside the inner instructions recorded for `instruction_index`;
    /// `None` for an outer instruction.
    pub inner_instruction_index: Option<u32>,
    /// Recorded invocation height of an inner instruction; `None` for outer
    /// instructions and for producers that did not record it.
    pub stack_height: Option<u32>,
    /// First transaction signature.
    pub signature: Signature,
    pub program: TokenProgram,
    pub kind: OperationKind,
    /// Exact raw amount in base units; zero amounts are kept.
    pub amount: u64,
    /// From the instruction for checked kinds; `None` otherwise.
    pub decimals: Option<u8>,
    /// From the instruction; `None` for unchecked `Transfer`.
    pub mint: Option<Pubkey>,
    /// Source token account; `None` for mints.
    pub source: Option<Pubkey>,
    /// Destination token account; `None` for burns. May equal `source`.
    pub destination: Option<Pubkey>,
    /// Owner, delegate or mint authority that authorized the instruction.
    pub authority: Pubkey,
    /// Accounts after the authority (multisig signers, Token-2022 hook accounts).
    pub extra_accounts: Vec<Pubkey>,
    /// Owner wallet of `source` recorded in the transaction's token balances
    /// (pre-transaction entry, else post-transaction entry); `None` when unrecorded.
    pub source_owner: Option<Pubkey>,
    /// Owner wallet of `destination`, resolved like `source_owner`.
    pub destination_owner: Option<Pubkey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    pub clock: Clock,
    /// Operations in execution order: by transaction, then outer instruction,
    /// with each outer instruction followed by its inner instructions.
    pub operations: Vec<Operation>,
}

/// Validates a complete block and projects its covered token operations.
///
/// The block must be complete and unfiltered so positions are original. No
/// Substreams runtime, store or network access is involved.
pub fn project(block: &Block, coverage: Coverage) -> Result<Projection, ProjectError> {
    let identity = validate(block)?;
    let mut operations = Vec::new();
    for (index, transaction) in block.transactions.iter().enumerate() {
        let transaction_index = index as u32;
        project_transaction(transaction, transaction_index, coverage, &mut operations)
            .map_err(|error| ProjectError::Transaction { transaction_index, error })?;
    }
    Ok(Projection {
        clock: Clock {
            slot: block.slot,
            parent_slot: block.parent_slot,
            blockhash: identity.blockhash,
            previous_blockhash: identity.previous_blockhash,
            block_time: block.block_time,
            block_height: block.block_height,
            transaction_count: block.transactions.len() as u32,
            coverage,
            spec_revision: SPEC_REVISION,
        },
        operations,
    })
}

fn project_transaction(transaction: &Transaction, transaction_index: u32, coverage: Coverage, out: &mut Vec<Operation>) -> Result<(), TransactionError> {
    let message = transaction.message.as_ref().ok_or(TransactionError::MissingMessage)?;
    let meta = transaction.meta.as_ref().ok_or(TransactionError::MissingMeta)?;
    if meta.failed {
        return Ok(());
    }
    let first = transaction.signatures.first().copied().unwrap_or_default();
    let signature: Signature = first
        .try_into()
        .map_err(|_| TransactionError::InvalidSignature { position: 0, len: first.len() })?;
    let projector = TransactionProjector {
        keys: Keys::new(message, meta),
        meta,
        transaction_index,
        signature,
        coverage,
    };

    let mut inner_by_outer: Vec<Option<&InnerInstructions>> = vec![None; message.instructions.len()];
    for inner in &meta.inner_instructions {
        if let Some(slot) = inner_by_outer.get_mut(inner.index as usize) {
            *slot = Some(inner);
        }
    }
    for (index, instruction) in message.instructions.iter().enumerate() {
        let instruction_index = index as u32;
        projector.visit(
            instruction,
            Position {
                instruction_index,
                inner_instruction_index: None,
            },
            out,
        )?;
        for (inner_index, inner) in inner_by_outer[index].iter().flat_map(|inner| inner.instructions.iter().enumerate()) {
            projector.visit(
                inner,
                Position {
                    instruction_index,
                    inner_instruction_index: Some(inner_index as u32),
                },
                out,
            )?;
        }
    }
    Ok(())
}

struct TransactionProjector<'t, 'a> {
    keys: Keys<'t, 'a>,
    meta: &'t Meta<'a>,
    transaction_index: u32,
    signature: Signature,
    coverage: Coverage,
}

impl TransactionProjector<'_, '_> {
    fn visit(&self, instruction: &Instruction, position: Position, out: &mut Vec<Operation>) -> Result<(), TransactionError> {
        let key_count = self.keys.len();
        let program_id = self
            .keys
            .get(instruction.program_id_index as usize)
            .ok_or(TransactionError::ProgramIndexOutOfBounds {
                position,
                program_id_index: instruction.program_id_index,
                key_count,
            })?;
        let Some(program) = TokenProgram::from_program_id(program_id) else {
            return Ok(());
        };
        // Accounts as (index, key) so token balance owners can be found by index.
        let accounts = instruction
            .accounts
            .iter()
            .enumerate()
            .map(|(account_position, &account_index)| {
                self.keys
                    .get(account_index as usize)
                    .and_then(|key| Pubkey::try_from(key).ok())
                    .map(|key| (account_index, key))
                    .ok_or(TransactionError::AccountIndexOutOfBounds {
                        position,
                        account_position,
                        account_index,
                        key_count,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let Some(decoded) = decode(instruction.data, &accounts).map_err(|error| TransactionError::Decode { position, error })? else {
            return Ok(());
        };
        if !self.coverage.includes(program, decoded.kind, position.inner_instruction_index.is_some()) {
            return Ok(());
        }
        let owner = |account: Option<(u8, Pubkey)>| account.map_or(Ok(None), |(index, _)| self.owner(index));
        out.push(Operation {
            transaction_index: self.transaction_index,
            instruction_index: position.instruction_index,
            inner_instruction_index: position.inner_instruction_index,
            stack_height: instruction.stack_height,
            signature: self.signature,
            program,
            kind: decoded.kind,
            amount: decoded.amount,
            decimals: decoded.decimals,
            mint: decoded.mint.map(|(_, key)| key),
            source: decoded.source.map(|(_, key)| key),
            destination: decoded.destination.map(|(_, key)| key),
            authority: decoded.authority.1,
            extra_accounts: decoded.extra_accounts.iter().map(|(_, key)| *key).collect(),
            source_owner: owner(decoded.source)?,
            destination_owner: owner(decoded.destination)?,
        });
        Ok(())
    }

    /// Owner recorded for the account in the pre-transaction token balances,
    /// else in the post-transaction ones.
    fn owner(&self, account_index: u8) -> Result<Option<Pubkey>, TransactionError> {
        fn recorded<'a>(balances: &[TokenBalance<'a>], account_index: u32) -> Option<&'a str> {
            balances
                .iter()
                .find(|balance| balance.account_index == account_index && !balance.owner.is_empty())
                .map(|balance| balance.owner)
        }
        let account_index = account_index as u32;
        match recorded(&self.meta.pre_token_balances, account_index).or_else(|| recorded(&self.meta.post_token_balances, account_index)) {
            None => Ok(None),
            Some(owner) => decode_key(owner).map(Some).ok_or(TransactionError::InvalidTokenBalanceOwner { account_index }),
        }
    }
}
