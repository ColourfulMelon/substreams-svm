use crate::error::{Position, ProjectError, TransactionError};
use crate::model::{Block, Instruction, Message, Meta, Transaction};

/// Resolved account keys of a transaction: static keys, then loaded writable,
/// then loaded readonly addresses, which is the order instruction indexes use.
#[derive(Clone, Copy, Debug)]
pub struct Keys<'t, 'a> {
    static_keys: &'t [&'a [u8]],
    writable: &'t [&'a [u8]],
    readonly: &'t [&'a [u8]],
}

impl<'t, 'a> Keys<'t, 'a> {
    pub fn new(message: &'t Message<'a>, meta: &'t Meta<'a>) -> Self {
        Self {
            static_keys: &message.account_keys,
            writable: &meta.loaded_writable_addresses,
            readonly: &meta.loaded_readonly_addresses,
        }
    }

    pub fn len(&self) -> usize {
        self.static_keys.len() + self.writable.len() + self.readonly.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The key at a resolved index, or `None` when out of bounds.
    pub fn get(&self, index: usize) -> Option<&'a [u8]> {
        let mut index = index;
        for keys in [self.static_keys, self.writable, self.readonly] {
            if index < keys.len() {
                return Some(keys[index]);
            }
            index -= keys.len();
        }
        None
    }

    pub fn iter(&self) -> impl Iterator<Item = &'a [u8]> + 't {
        self.static_keys.iter().chain(self.writable).chain(self.readonly).copied()
    }
}

pub(crate) struct BlockIdentity {
    pub blockhash: [u8; 32],
    pub previous_blockhash: [u8; 32],
}

pub(crate) fn decode_key(value: &str) -> Option<[u8; 32]> {
    let mut key = [0u8; 32];
    match bs58::decode(value).onto(&mut key) {
        Ok(32) => Some(key),
        _ => None,
    }
}

/// Validates the block identity and every transaction's structure.
///
/// Every transaction must carry its message, header and meta, and 64-byte
/// signatures consistent with the header. Successful transactions must also
/// have 32-byte keys, loaded address counts matching their address table
/// lookups, and in-bounds program, account, inner-instruction and token-balance
/// indexes. Failed transactions are never projected, so their instruction-level
/// indexes are not checked.
pub fn validate_block(block: &Block) -> Result<(), ProjectError> {
    validate(block).map(|_| ())
}

pub(crate) fn validate(block: &Block) -> Result<BlockIdentity, ProjectError> {
    let hash = |field: &'static str, value: &str| {
        decode_key(value).ok_or_else(|| ProjectError::InvalidBlockHash {
            field,
            value: value.to_string(),
        })
    };
    let identity = BlockIdentity {
        blockhash: hash("blockhash", block.blockhash)?,
        previous_blockhash: hash("previous_blockhash", block.previous_blockhash)?,
    };
    if block.parent_slot >= block.slot && !(block.slot == 0 && block.parent_slot == 0) {
        return Err(ProjectError::InvalidParentSlot {
            slot: block.slot,
            parent_slot: block.parent_slot,
        });
    }
    if let Some(block_time) = block.block_time.filter(|time| *time < 0) {
        return Err(ProjectError::NegativeBlockTime { block_time });
    }
    for (index, transaction) in block.transactions.iter().enumerate() {
        validate_transaction(transaction).map_err(|error| ProjectError::Transaction {
            transaction_index: index as u32,
            error,
        })?;
    }
    Ok(identity)
}

/// Structural checks of one transaction; see [`validate_block`].
pub fn validate_transaction(transaction: &Transaction) -> Result<(), TransactionError> {
    let message = transaction.message.as_ref().ok_or(TransactionError::MissingMessage)?;
    let header = message.header.as_ref().ok_or(TransactionError::MissingHeader)?;
    let meta = transaction.meta.as_ref().ok_or(TransactionError::MissingMeta)?;

    let signatures = transaction.signatures.len();
    if signatures == 0 || signatures < header.num_required_signatures as usize || signatures > message.account_keys.len() {
        return Err(TransactionError::SignatureCount {
            signatures,
            required: header.num_required_signatures,
            account_keys: message.account_keys.len(),
        });
    }
    if let Some((position, signature)) = transaction.signatures.iter().enumerate().find(|(_, signature)| signature.len() != 64) {
        return Err(TransactionError::InvalidSignature {
            position,
            len: signature.len(),
        });
    }
    if meta.failed {
        return Ok(());
    }

    let keys = Keys::new(message, meta);
    if keys.len() > 256 {
        return Err(TransactionError::TooManyAccountKeys { count: keys.len() });
    }
    if let Some((position, key)) = keys.iter().enumerate().find(|(_, key)| key.len() != 32) {
        return Err(TransactionError::InvalidAccountKey { position, len: key.len() });
    }
    let lookup_writable: usize = message.address_table_lookups.iter().map(|lookup| lookup.writable_indexes.len()).sum();
    let lookup_readonly: usize = message.address_table_lookups.iter().map(|lookup| lookup.readonly_indexes.len()).sum();
    if lookup_writable != meta.loaded_writable_addresses.len() || lookup_readonly != meta.loaded_readonly_addresses.len() {
        return Err(TransactionError::LoadedAddressCount {
            writable: meta.loaded_writable_addresses.len(),
            readonly: meta.loaded_readonly_addresses.len(),
            lookup_writable,
            lookup_readonly,
        });
    }

    for (index, instruction) in message.instructions.iter().enumerate() {
        let position = Position {
            instruction_index: index as u32,
            inner_instruction_index: None,
        };
        check_instruction(instruction, position, &keys)?;
    }
    let mut recorded = vec![false; message.instructions.len()];
    for inner in &meta.inner_instructions {
        let parent = recorded.get_mut(inner.index as usize).ok_or(TransactionError::InnerInstructionsOutOfBounds {
            index: inner.index,
            instruction_count: message.instructions.len(),
        })?;
        if std::mem::replace(parent, true) {
            return Err(TransactionError::DuplicateInnerInstructions { index: inner.index });
        }
        for (index, instruction) in inner.instructions.iter().enumerate() {
            let position = Position {
                instruction_index: inner.index,
                inner_instruction_index: Some(index as u32),
            };
            check_instruction(instruction, position, &keys)?;
        }
    }
    for balance in meta.pre_token_balances.iter().chain(&meta.post_token_balances) {
        if balance.account_index as usize >= keys.len() {
            return Err(TransactionError::TokenBalanceAccountOutOfBounds {
                account_index: balance.account_index,
                key_count: keys.len(),
            });
        }
    }
    Ok(())
}

fn check_instruction(instruction: &Instruction, position: Position, keys: &Keys) -> Result<(), TransactionError> {
    if instruction.program_id_index as usize >= keys.len() {
        return Err(TransactionError::ProgramIndexOutOfBounds {
            position,
            program_id_index: instruction.program_id_index,
            key_count: keys.len(),
        });
    }
    if let Some((account_position, account_index)) = instruction.accounts.iter().enumerate().find(|(_, index)| **index as usize >= keys.len()) {
        return Err(TransactionError::AccountIndexOutOfBounds {
            position,
            account_position,
            account_index: *account_index,
            key_count: keys.len(),
        });
    }
    Ok(())
}
