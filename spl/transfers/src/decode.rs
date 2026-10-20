//! Decoding of the SPL Token instructions that move token amounts.
//!
//! The data layouts are the classic Token program's, shared unchanged by
//! Token-2022: a one-byte tag, a little-endian `u64` amount and, for the
//! checked variants, a one-byte `decimals`. Trailing bytes are ignored, as the
//! programs ignore them.

use std::fmt;

/// Token program that executed an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TokenProgram {
    /// Classic SPL Token, `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`.
    Token,
    /// SPL Token-2022, `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`.
    Token2022,
}

/// `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
pub const TOKEN_PROGRAM_ID: [u8; 32] = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237, 95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];

/// `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`
pub const TOKEN_2022_PROGRAM_ID: [u8; 32] = [
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77, 131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
];

impl TokenProgram {
    pub fn from_program_id(program_id: &[u8]) -> Option<Self> {
        if program_id == TOKEN_PROGRAM_ID {
            Some(Self::Token)
        } else if program_id == TOKEN_2022_PROGRAM_ID {
            Some(Self::Token2022)
        } else {
            None
        }
    }

    pub fn program_id(self) -> [u8; 32] {
        match self {
            Self::Token => TOKEN_PROGRAM_ID,
            Self::Token2022 => TOKEN_2022_PROGRAM_ID,
        }
    }
}

/// Explicit kind of an amount-moving token instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OperationKind {
    /// `Transfer` (tag 3): the instruction does not name the mint.
    Transfer,
    /// `TransferChecked` (tag 12).
    TransferChecked,
    /// `MintTo` (tag 7).
    MintTo,
    /// `MintToChecked` (tag 14).
    MintToChecked,
    /// `Burn` (tag 8).
    Burn,
    /// `BurnChecked` (tag 15).
    BurnChecked,
}

impl OperationKind {
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            3 => Some(Self::Transfer),
            7 => Some(Self::MintTo),
            8 => Some(Self::Burn),
            12 => Some(Self::TransferChecked),
            14 => Some(Self::MintToChecked),
            15 => Some(Self::BurnChecked),
            _ => None,
        }
    }

    pub fn tag(self) -> u8 {
        match self {
            Self::Transfer => 3,
            Self::MintTo => 7,
            Self::Burn => 8,
            Self::TransferChecked => 12,
            Self::MintToChecked => 14,
            Self::BurnChecked => 15,
        }
    }

    /// Whether the instruction carries `decimals` (and, for transfers, the mint).
    pub fn is_checked(self) -> bool {
        matches!(self, Self::TransferChecked | Self::MintToChecked | Self::BurnChecked)
    }

    /// Minimum number of instruction accounts the layout requires.
    pub fn required_accounts(self) -> usize {
        match self {
            Self::TransferChecked => 4,
            _ => 3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transfer => "transfer",
            Self::TransferChecked => "transfer_checked",
            Self::MintTo => "mint_to",
            Self::MintToChecked => "mint_to_checked",
            Self::Burn => "burn",
            Self::BurnChecked => "burn_checked",
        }
    }
}

impl fmt::Display for OperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Account roles and amounts of a decoded instruction. `K` is whatever the
/// caller uses to reference instruction accounts (key bytes or key indexes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded<K> {
    pub kind: OperationKind,
    /// Exact raw amount in base units; zero is a valid amount.
    pub amount: u64,
    /// From the instruction for checked kinds; `None` otherwise.
    pub decimals: Option<u8>,
    /// Source token account; `None` for mints.
    pub source: Option<K>,
    /// Destination token account; `None` for burns.
    pub destination: Option<K>,
    /// Mint account; `None` for unchecked `Transfer`, which does not name it.
    pub mint: Option<K>,
    /// Owner, delegate or mint authority that authorized the instruction.
    pub authority: K,
    /// Accounts after the authority: the signing members when the authority is
    /// a multisig. Token-2022 transfer hooks also append their accounts here.
    pub extra_accounts: Vec<K>,
}

/// An amount-moving instruction whose data or account list does not fit its layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The data is shorter than the layout of `kind`.
    InvalidData { kind: OperationKind, len: usize },
    /// The instruction lists fewer accounts than `kind` requires.
    MissingAccounts { kind: OperationKind, required: usize, found: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData { kind, len } => write!(f, "{kind} instruction data has {len} bytes"),
            Self::MissingAccounts { kind, required, found } => {
                write!(f, "{kind} instruction lists {found} accounts, {required} required")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decodes a token program instruction.
///
/// Returns `Ok(None)` for instructions that do not move amounts (or carry no
/// data), and an error when an amount-moving instruction is truncated or lists
/// too few accounts. Only executed instructions of failed transactions can look
/// like that; the token programs reject them.
///
/// The caller must already know the instruction belongs to a token program.
pub fn decode<K: Copy>(data: &[u8], accounts: &[K]) -> Result<Option<Decoded<K>>, DecodeError> {
    let Some(kind) = data.first().copied().and_then(OperationKind::from_tag) else {
        return Ok(None);
    };
    let amount = data
        .get(1..9)
        .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("8 bytes")))
        .ok_or(DecodeError::InvalidData { kind, len: data.len() })?;
    let decimals = if kind.is_checked() {
        Some(*data.get(9).ok_or(DecodeError::InvalidData { kind, len: data.len() })?)
    } else {
        None
    };
    let required = kind.required_accounts();
    if accounts.len() < required {
        return Err(DecodeError::MissingAccounts {
            kind,
            required,
            found: accounts.len(),
        });
    }
    let account = |position: usize| accounts[position];
    let extra_accounts = accounts[required..].to_vec();
    let (source, destination, mint, authority) = match kind {
        // source, destination, authority
        OperationKind::Transfer => (Some(account(0)), Some(account(1)), None, account(2)),
        // source, mint, destination, authority
        OperationKind::TransferChecked => (Some(account(0)), Some(account(2)), Some(account(1)), account(3)),
        // mint, destination, mint authority
        OperationKind::MintTo | OperationKind::MintToChecked => (None, Some(account(1)), Some(account(0)), account(2)),
        // source, mint, owner or delegate
        OperationKind::Burn | OperationKind::BurnChecked => (Some(account(0)), None, Some(account(1)), account(2)),
    };
    Ok(Some(Decoded {
        kind,
        amount,
        decimals,
        source,
        destination,
        mint,
        authority,
        extra_accounts,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(kind: OperationKind, amount: u64, decimals: Option<u8>) -> Vec<u8> {
        let mut data = vec![kind.tag()];
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend(decimals);
        data
    }

    #[test]
    fn transfer_checked_roles() {
        let decoded = decode(&data(OperationKind::TransferChecked, 5, Some(6)), &[10, 11, 12, 13, 14, 15])
            .unwrap()
            .unwrap();
        assert_eq!(
            decoded,
            Decoded {
                kind: OperationKind::TransferChecked,
                amount: 5,
                decimals: Some(6),
                source: Some(10),
                destination: Some(12),
                mint: Some(11),
                authority: 13,
                extra_accounts: vec![14, 15],
            }
        );
    }

    #[test]
    fn unchecked_transfer_has_destination_and_no_mint() {
        let decoded = decode(&data(OperationKind::Transfer, 7, None), &[10, 11, 12]).unwrap().unwrap();
        assert_eq!(decoded.source, Some(10));
        assert_eq!(decoded.destination, Some(11));
        assert_eq!(decoded.authority, 12);
        assert_eq!(decoded.mint, None);
        assert_eq!(decoded.decimals, None);
        assert!(decoded.extra_accounts.is_empty());
    }

    #[test]
    fn mint_and_burn_roles() {
        let mint = decode(&data(OperationKind::MintToChecked, 9, Some(2)), &[1, 2, 3]).unwrap().unwrap();
        assert_eq!((mint.source, mint.destination, mint.mint, mint.authority), (None, Some(2), Some(1), 3));
        assert_eq!(mint.decimals, Some(2));
        let mint = decode(&data(OperationKind::MintTo, 9, None), &[1, 2, 3]).unwrap().unwrap();
        assert_eq!((mint.kind, mint.decimals), (OperationKind::MintTo, None));

        let burn = decode(&data(OperationKind::BurnChecked, 9, Some(2)), &[1, 2, 3, 4]).unwrap().unwrap();
        assert_eq!((burn.source, burn.destination, burn.mint, burn.authority), (Some(1), None, Some(2), 3));
        assert_eq!(burn.extra_accounts, vec![4]);
        let burn = decode(&data(OperationKind::Burn, 9, None), &[1, 2, 3]).unwrap().unwrap();
        assert_eq!((burn.kind, burn.decimals), (OperationKind::Burn, None));
    }

    #[test]
    fn zero_amount_is_kept() {
        let decoded = decode(&data(OperationKind::TransferChecked, 0, Some(0)), &[1, 2, 3, 4]).unwrap().unwrap();
        assert_eq!(decoded.amount, 0);
    }

    #[test]
    fn max_amount_is_exact() {
        let decoded = decode(&data(OperationKind::Transfer, u64::MAX, None), &[1, 2, 3]).unwrap().unwrap();
        assert_eq!(decoded.amount, u64::MAX);
    }

    #[test]
    fn trailing_bytes_are_ignored() {
        let mut bytes = data(OperationKind::TransferChecked, 3, Some(9));
        bytes.extend_from_slice(&[0xff, 0xff]);
        let decoded = decode(&bytes, &[1, 2, 3, 4]).unwrap().unwrap();
        assert_eq!((decoded.amount, decoded.decimals), (3, Some(9)));
    }

    #[test]
    fn other_instructions_are_not_operations() {
        assert_eq!(decode::<u8>(&[], &[]), Ok(None));
        // InitializeAccount, Approve, CloseAccount, TransferCheckedWithFee (extension 26)
        for tag in [1u8, 4, 9, 26] {
            assert_eq!(decode::<u8>(&[tag, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[1, 2, 3, 4]), Ok(None));
        }
    }

    #[test]
    fn truncated_data_is_an_error() {
        assert_eq!(
            decode::<u8>(&[3, 1, 2], &[1, 2, 3]),
            Err(DecodeError::InvalidData {
                kind: OperationKind::Transfer,
                len: 3
            })
        );
        // checked kinds need the decimals byte
        assert_eq!(
            decode::<u8>(&data(OperationKind::TransferChecked, 1, None), &[1, 2, 3, 4]),
            Err(DecodeError::InvalidData {
                kind: OperationKind::TransferChecked,
                len: 9
            })
        );
    }

    #[test]
    fn missing_accounts_are_an_error() {
        assert_eq!(
            decode(&data(OperationKind::TransferChecked, 1, Some(0)), &[1, 2, 3]),
            Err(DecodeError::MissingAccounts {
                kind: OperationKind::TransferChecked,
                required: 4,
                found: 3
            })
        );
        assert_eq!(
            decode(&data(OperationKind::Burn, 1, None), &[1]),
            Err(DecodeError::MissingAccounts {
                kind: OperationKind::Burn,
                required: 3,
                found: 1
            })
        );
    }

    #[test]
    fn program_ids() {
        assert_eq!(bs58::encode(TOKEN_PROGRAM_ID).into_string(), "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        assert_eq!(bs58::encode(TOKEN_2022_PROGRAM_ID).into_string(), "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
        assert_eq!(TokenProgram::from_program_id(&TOKEN_PROGRAM_ID), Some(TokenProgram::Token));
        assert_eq!(TokenProgram::from_program_id(&TOKEN_2022_PROGRAM_ID), Some(TokenProgram::Token2022));
        assert_eq!(TokenProgram::from_program_id(&[0; 32]), None);
    }
}
