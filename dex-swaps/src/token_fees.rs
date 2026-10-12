//! Exact Token-2022 fees from owned mint metadata and completed native changes.
//! Rules pinned to solana-program/token-2022 b4724e6e7fecc8fc36295f861a769401bf92b270.
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::spl::token_2022;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferFee {
    pub basis_points: u16,
    pub maximum_fee: u64,
}

impl TransferFee {
    pub fn received(self, gross: u64) -> Option<u64> {
        if self.basis_points > 10_000 {
            return None;
        }
        let numerator = u128::from(gross) * u128::from(self.basis_points);
        let fee = ((numerator + 9_999) / 10_000).min(u128::from(self.maximum_fee));
        gross.checked_sub(u64::try_from(fee).ok()?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EpochFee {
    pub epoch: u64,
    pub fee: TransferFee,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub(crate) older: Option<EpochFee>,
    pub(crate) newer: Option<EpochFee>,
}

impl Schedule {
    pub fn at(self, epoch: u64) -> Option<TransferFee> {
        let applicable = match self.newer {
            Some(newer) if newer.epoch <= epoch => Some(newer),
            _ => self.older.filter(|fee| fee.epoch <= epoch),
        };
        applicable.map(|fee| fee.fee)
    }
}

/// Validate the owned mint, not token-account bytes or an untrusted event.
/// Unextended Token-2022 mints cannot contain a transfer fee configuration.
pub fn owned_mint(owner: &[u8], data: &[u8]) -> Option<(Schedule, bool)> {
    if owner != token_2022::PROGRAM_ID
        || data.len() < 82
        || data.len() == 355
        || !matches!(u32::from_le_bytes(data[..4].try_into().ok()?), 0 | 1)
        || !matches!(u32::from_le_bytes(data[46..50].try_into().ok()?), 0 | 1)
        || !matches!(data[45], 0 | 1)
    {
        return None;
    }
    let zero = Schedule {
        older: Some(EpochFee {
            epoch: 0,
            fee: TransferFee {
                basis_points: 0,
                maximum_fee: 0,
            },
        }),
        newer: None,
    };
    if data.len() == 82 {
        return Some((zero, data[45] == 1));
    }
    if data.len() < 166 || data[165] != data[45] || data[82..165].iter().any(|byte| *byte != 0) {
        return None;
    }
    let mut schedule = None;
    let mut offset = 166;
    while offset < data.len() {
        if data[offset..].iter().all(|byte| *byte == 0) {
            break;
        }
        let header = data.get(offset..offset + 4)?;
        let kind = u16::from_le_bytes(header[..2].try_into().ok()?);
        let length = u16::from_le_bytes(header[2..].try_into().ok()?) as usize;
        offset += 4;
        let extension = data.get(offset..offset.checked_add(length)?)?;
        if kind == 0 {
            return None;
        }
        if kind == 1 {
            if length != 108 || schedule.is_some() {
                return None;
            }
            let older = EpochFee::decode(&extension[72..90])?;
            let newer = EpochFee::decode(&extension[90..108])?;
            if older.epoch > newer.epoch {
                return None;
            }
            schedule = Some(Schedule {
                older: Some(older),
                newer: Some(newer),
            });
        }
        offset += length;
    }
    Some((schedule.unwrap_or(zero), data[45] == 1))
}

impl EpochFee {
    // Same 18-byte layout as Token-2022 TransferFee: epoch, maximum, bps.
    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 18 {
            return None;
        }
        let epoch = u64::from_le_bytes(bytes[..8].try_into().ok()?);
        let maximum_fee = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let basis_points = u16::from_le_bytes(bytes[16..].try_into().ok()?);
        (basis_points <= 10_000).then_some(Self {
            epoch,
            fee: TransferFee { basis_points, maximum_fee },
        })
    }

    fn encode(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.epoch.to_le_bytes());
        output.extend_from_slice(&self.fee.maximum_fee.to_le_bytes());
        output.extend_from_slice(&self.fee.basis_points.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Snapshot { schedule: Schedule, initialized: bool },
    InitializeFee(EpochFee),
    SetFee(EpochFee),
    InitializeMint,
    Close,
    UnknownInitializeFee,
    UnknownSetFee,
    UnknownInitializeMint,
    UnknownClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Absent,
    Known(Schedule),
    Unknown,
}

/// A known completed mint initialization activates its pending extensions.
/// A close/reinitialization never resurrects the old account's fee schedule.
#[derive(Clone, Copy, Debug)]
pub(crate) struct State {
    active: Option<Schedule>,
    pending: Pending,
}

impl Default for State {
    fn default() -> Self {
        Self {
            active: None,
            pending: Pending::Absent,
        }
    }
}

impl State {
    pub(crate) fn apply(&mut self, change: Change) {
        match change {
            Change::Snapshot { schedule, initialized } => {
                self.active = initialized.then_some(schedule);
                self.pending = if initialized { Pending::Absent } else { Pending::Known(schedule) };
            }
            Change::InitializeFee(fee) => {
                self.active = None;
                self.pending = Pending::Known(Schedule {
                    older: Some(fee),
                    newer: Some(fee),
                });
            }
            Change::SetFee(mut fee) => {
                let mut schedule = self.active.unwrap_or(Schedule { older: None, newer: None });
                if let Some(newer) = schedule.newer.filter(|newer| newer.epoch <= fee.epoch) {
                    schedule.older = Some(newer);
                }
                fee.epoch = fee.epoch.saturating_add(2);
                schedule.newer = Some(fee);
                self.active = Some(schedule);
                self.pending = Pending::Absent;
            }
            Change::InitializeMint => {
                self.active = match self.pending {
                    Pending::Known(schedule) => Some(schedule),
                    Pending::Unknown => None,
                    Pending::Absent => Some(Schedule {
                        older: Some(EpochFee {
                            epoch: 0,
                            fee: TransferFee {
                                basis_points: 0,
                                maximum_fee: 0,
                            },
                        }),
                        newer: None,
                    }),
                };
                self.pending = Pending::Absent;
            }
            Change::Close => *self = Self::default(),
            Change::UnknownInitializeFee | Change::UnknownSetFee | Change::UnknownClose => {
                self.active = None;
                self.pending = Pending::Unknown;
            }
            Change::UnknownInitializeMint => {
                // Mint initialization cannot modify the pending fee config.
                // Keep it available for a later proven initialization; it is
                // not yet active and never supplies an unproven transfer fee.
                self.active = None;
                if self.pending == Pending::Absent {
                    self.pending = Pending::Unknown;
                }
            }
        }
    }

    pub(crate) fn at(self, epoch: u64) -> Option<TransferFee> {
        self.active?.at(epoch)
    }
}

/// Native metadata mutations only. Caller must distinguish completed calls,
/// ancestor rollbacks and missing logs before applying any returned change.
pub(crate) fn instruction_change(ix: &InstructionView, epoch: u64) -> Option<(Vec<u8>, Change)> {
    if ix.program_id().0.as_slice() != token_2022::PROGRAM_ID {
        return None;
    }
    let accounts = ix.accounts();
    let mint = accounts.first()?.0;
    if mint.len() != 32 {
        return None;
    }
    let data = ix.data();
    let change = match data.as_slice() {
        [26, 0, rest @ ..] => {
            // Each authority is encoded as a one-byte option, followed by a
            // pubkey only for Some. Reject noncanonical/trailing encodings.
            let mut offset = 0;
            let valid = (0..2).all(|_| {
                let length = match rest.get(offset) {
                    Some(0) => 1,
                    Some(1) => 33,
                    _ => return false,
                };
                offset += length;
                offset <= rest.len()
            });
            let fee = valid.then(|| rest.get(offset..)).flatten().filter(|bytes| bytes.len() == 10).and_then(|bytes| {
                let basis_points = u16::from_le_bytes(bytes[..2].try_into().ok()?);
                let maximum_fee = u64::from_le_bytes(bytes[2..].try_into().ok()?);
                (basis_points <= 10_000).then_some(EpochFee {
                    epoch,
                    fee: TransferFee { basis_points, maximum_fee },
                })
            });
            fee.map(Change::InitializeFee).unwrap_or(Change::UnknownInitializeFee)
        }
        [26, 5, rest @ ..] => {
            let fee = (rest.len() == 10 && accounts.len() >= 2)
                .then(|| {
                    let basis_points = u16::from_le_bytes(rest[..2].try_into().unwrap());
                    let maximum_fee = u64::from_le_bytes(rest[2..].try_into().unwrap());
                    (basis_points <= 10_000).then_some(EpochFee {
                        epoch,
                        fee: TransferFee { basis_points, maximum_fee },
                    })
                })
                .flatten();
            fee.map(Change::SetFee).unwrap_or(Change::UnknownSetFee)
        }
        [0 | 20, rest @ ..] => {
            let valid = rest.len() >= 34
                && match rest[33] {
                    0 => rest.len() == 34,
                    1 => rest.len() == 66,
                    _ => false,
                }
                && (data[0] == 20 || accounts.len() >= 2);
            if valid {
                Change::InitializeMint
            } else {
                Change::UnknownInitializeMint
            }
        }
        [9] if accounts.len() >= 3 => Change::Close,
        [9, ..] => Change::UnknownClose,
        _ => return None,
    };
    Some((mint.clone(), change))
}

impl Change {
    pub(crate) fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            Self::Snapshot { schedule, initialized } => {
                bytes.extend_from_slice(&[0, u8::from(initialized)]);
                for fee in [schedule.older, schedule.newer] {
                    bytes.push(u8::from(fee.is_some()));
                    if let Some(fee) = fee {
                        fee.encode(&mut bytes);
                    }
                }
            }
            Self::InitializeFee(fee) | Self::SetFee(fee) => {
                bytes.push(if matches!(self, Self::InitializeFee(_)) { 1 } else { 2 });
                fee.encode(&mut bytes);
            }
            Self::InitializeMint => bytes.push(3),
            Self::Close => bytes.push(4),
            Self::UnknownInitializeFee => bytes.push(5),
            Self::UnknownSetFee => bytes.push(6),
            Self::UnknownInitializeMint => bytes.push(7),
            Self::UnknownClose => bytes.push(8),
        }
        bytes
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0, initialized @ (0 | 1), rest @ ..] => {
                let mut remaining = rest;
                let mut fees = [None, None];
                for fee in &mut fees {
                    let (flag, rest) = remaining.split_first()?;
                    remaining = match flag {
                        0 => rest,
                        1 => {
                            *fee = Some(EpochFee::decode(rest.get(..18)?)?);
                            rest.get(18..)?
                        }
                        _ => return None,
                    };
                }
                if !remaining.is_empty() || fees[0].zip(fees[1]).is_some_and(|(a, b)| a.epoch > b.epoch) {
                    return None;
                }
                Some(Self::Snapshot {
                    schedule: Schedule {
                        older: fees[0],
                        newer: fees[1],
                    },
                    initialized: *initialized == 1,
                })
            }
            [1, rest @ ..] => Some(Self::InitializeFee(EpochFee::decode(rest)?)),
            [2, rest @ ..] => Some(Self::SetFee(EpochFee::decode(rest)?)),
            [3] => Some(Self::InitializeMint),
            [4] => Some(Self::Close),
            [5] => Some(Self::UnknownInitializeFee),
            [6] => Some(Self::UnknownSetFee),
            [7] => Some(Self::UnknownInitializeMint),
            [8] => Some(Self::UnknownClose),
            _ => None,
        }
    }

    pub(crate) fn unproven(self) -> Self {
        match self {
            Self::InitializeFee(_) | Self::UnknownInitializeFee => Self::UnknownInitializeFee,
            Self::SetFee(_) | Self::UnknownSetFee => Self::UnknownSetFee,
            Self::InitializeMint | Self::UnknownInitializeMint => Self::UnknownInitializeMint,
            Self::Close | Self::UnknownClose => Self::UnknownClose,
            snapshot => snapshot,
        }
    }
}

/// Complete changes retain exact transaction/instruction ordinals. Failed
/// ancestors roll back; absent or truncated logs invalidate the affected mint.
pub(crate) fn native_changes(block: &substreams_solana::pb::sf::solana::r#type::v1::Block, epoch: u64) -> Vec<(u64, Vec<u8>, Change)> {
    let mut changes = Vec::new();
    for (transaction_index, tx) in block.transactions.iter().enumerate() {
        if !tx.is_successful() {
            continue;
        }
        let candidates: Vec<_> = tx
            .walk_instructions()
            .enumerate()
            .filter_map(|(index, ix)| instruction_change(&ix, epoch).map(|(mint, change)| (index as u32, mint, change, ix.is_root())))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let outcomes = crate::logs::invocation_outcomes(tx);
        for (index, mint, change, root) in candidates {
            if outcomes.rolled_back.contains(&index) {
                continue;
            }
            // A successful Solana transaction proves every root instruction
            // committed, even if runtime logs are truncated. CPI mutations
            // still require a complete successful ancestor chain.
            let change = if root || outcomes.completed.contains(&index) {
                change
            } else {
                change.unproven()
            };
            changes.push((crate::phoenix_metadata::ordinal(transaction_index, index), mint, change));
        }
    }
    changes
}

/// Compact owned seeds are context:u64, mint:[u8;32], older/newer:[u8;18],
/// initialized:u8, reserved:[u8;3]. The generator validates all ownership/TLV
/// proofs, uniqueness and context ordering before producing this file.
/// Binary-search the read-context window to avoid scanning the entire registry
/// for every block. Context seeds describe end-of-block state and override any
/// earlier native changes in that same block.
pub(crate) fn updates(
    block: &substreams_solana::pb::sf::solana::r#type::v1::Block,
    seed: &[u8],
    anchor: u64,
    complete_at: u64,
) -> Result<proto::pb::dex::swaps::v1::TokenFeeUpdates, substreams::errors::Error> {
    use proto::pb::dex::swaps::v1::{TokenFeeUpdate, TokenFeeUpdates};
    use substreams::errors::Error;
    if seed.len() % 80 != 0 || anchor > complete_at || block.slot < anchor || block.parent_slot >= block.slot {
        return Err(Error::msg("invalid Token-2022 snapshot bounds"));
    }
    let mut bounds = [0, 0];
    for (position, boundary) in [block.parent_slot, block.slot].into_iter().enumerate() {
        let (mut lower, mut upper) = (0, seed.len() / 80);
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let context = u64::from_le_bytes(seed[middle * 80..middle * 80 + 8].try_into().unwrap());
            if context <= boundary {
                lower = middle + 1;
            } else {
                upper = middle;
            }
        }
        bounds[position] = lower;
    }
    if bounds[0] > bounds[1] {
        return Err(Error::msg("invalid Token-2022 parent slot"));
    }
    let mut changes: Vec<_> = native_changes(block, block.slot / 432_000)
        .into_iter()
        .map(|(ordinal, mint, change)| TokenFeeUpdate {
            ordinal,
            mint,
            change: change.encode(),
        })
        .collect();
    for row in seed[bounds[0] * 80..bounds[1] * 80].chunks_exact(80) {
        let context = u64::from_le_bytes(row[..8].try_into().unwrap());
        let older = EpochFee::decode(&row[40..58]).ok_or_else(|| Error::msg("invalid older mint fee"))?;
        let newer = EpochFee::decode(&row[58..76]).ok_or_else(|| Error::msg("invalid newer mint fee"))?;
        if context < anchor || context > complete_at || older.epoch > newer.epoch || !matches!(row[76], 0 | 1) || row[77..].iter().any(|byte| *byte != 0) {
            return Err(Error::msg("invalid owned Token-2022 seed"));
        }
        changes.push(TokenFeeUpdate {
            ordinal: i64::MAX as u64,
            mint: row[8..40].to_vec(),
            change: Change::Snapshot {
                schedule: Schedule {
                    older: Some(older),
                    newer: Some(newer),
                },
                initialized: row[76] == 1,
            }
            .encode(),
        });
    }
    if block.slot > complete_at && block.parent_slot <= complete_at {
        // Empty mint is exclusively a completion marker, never a mint key.
        changes.push(TokenFeeUpdate {
            ordinal: 0,
            mint: Vec::new(),
            change: complete_at.to_le_bytes().to_vec(),
        });
    }
    changes.sort_by_key(|update| update.ordinal);
    Ok(TokenFeeUpdates { changes })
}

/// Append-store records are hex-encoded native changes, separated by `;` by
/// the Substreams SDK. An absent key proves zero fees only after the complete
/// owned snapshot and all changes since its anchor have been incorporated.
pub(crate) fn fee_at(history: Option<&str>, snapshot_complete: bool, epoch: u64) -> Option<TransferFee> {
    if !snapshot_complete {
        return None;
    }
    let Some(history) = history else {
        return Some(TransferFee {
            basis_points: 0,
            maximum_fee: 0,
        });
    };
    State::from_history(history)?.at(epoch)
}

impl State {
    pub(crate) fn from_history(history: &str) -> Option<Self> {
        if history.is_empty() || !history.ends_with(';') {
            return None;
        }
        let mut state = Self::default();
        for record in history[..history.len() - 1].split(';') {
            if record.len() % 2 != 0 || record.is_empty() {
                return None;
            }
            let bytes: Option<Vec<u8>> = record
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
                .collect();
            state.apply(Change::decode(&bytes?)?);
        }
        Some(state)
    }

    // Active and pending-known state reuse the canonical snapshot encoding.
    // Distinct unknown states preserve whether a later initialization may
    // prove a fee-free re-creation or must retain an unknown pending config.
    pub(crate) fn encode(self) -> Vec<u8> {
        if let Some(schedule) = self.active {
            Change::Snapshot { schedule, initialized: true }.encode()
        } else {
            match self.pending {
                Pending::Known(schedule) => Change::Snapshot { schedule, initialized: false }.encode(),
                Pending::Absent => vec![9],
                Pending::Unknown => vec![10],
            }
        }
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [9] => Some(Self::default()),
            [10] => Some(Self {
                active: None,
                pending: Pending::Unknown,
            }),
            _ => {
                let change = Change::decode(bytes)?;
                if !matches!(change, Change::Snapshot { .. }) {
                    return None;
                }
                let mut state = Self::default();
                state.apply(change);
                Some(state)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_caps_and_maximum_amounts_match_native_fee_math() {
        let fee = TransferFee {
            basis_points: 100,
            maximum_fee: 1_000_000_000_000_000,
        };
        for (gross, net) in [
            (98_424_324_865, 97_440_081_616),
            (62_357_111_407, 61_733_540_292),
            (15_879_366_251, 15_720_572_588),
            (2_372_764_271, 2_349_036_628),
            (2_969_123_294, 2_939_432_061),
            (0, 0),
            (1, 0),
            (100, 99),
            (101, 99),
        ] {
            assert_eq!(fee.received(gross), Some(net));
        }
        assert_eq!(
            TransferFee {
                basis_points: 0,
                maximum_fee: u64::MAX
            }
            .received(u64::MAX),
            Some(u64::MAX)
        );
        assert_eq!(
            TransferFee {
                basis_points: 10_000,
                maximum_fee: u64::MAX
            }
            .received(u64::MAX),
            Some(0)
        );
        assert_eq!(
            TransferFee {
                basis_points: 10_000,
                maximum_fee: 3
            }
            .received(u64::MAX),
            Some(u64::MAX - 3)
        );
        assert_eq!(
            TransferFee {
                basis_points: 10_001,
                maximum_fee: 0
            }
            .received(1),
            None
        );
    }

    #[test]
    fn fee_changes_follow_two_epoch_activation_and_replace_pending_changes() {
        let older = EpochFee {
            epoch: 100,
            fee: TransferFee {
                basis_points: 100,
                maximum_fee: 10,
            },
        };
        let next = EpochFee {
            epoch: 101,
            fee: TransferFee {
                basis_points: 200,
                maximum_fee: 20,
            },
        };
        let mut state = State::default();
        state.apply(Change::InitializeFee(older));
        assert_eq!(state.at(100), None);
        state.apply(Change::InitializeMint);
        assert_eq!(state.at(100), Some(older.fee));
        state.apply(Change::SetFee(next));
        assert_eq!(state.at(102), Some(older.fee));
        assert_eq!(state.at(103), Some(next.fee));
        let replacement = EpochFee {
            epoch: 102,
            fee: TransferFee {
                basis_points: 300,
                maximum_fee: 30,
            },
        };
        state.apply(Change::SetFee(replacement));
        assert_eq!(state.at(103), Some(older.fee));
        assert_eq!(state.at(104), Some(replacement.fee));
        state.apply(Change::SetFee(EpochFee { epoch: 104, fee: older.fee }));
        assert_eq!(state.at(105), Some(replacement.fee));
        assert_eq!(state.at(106), Some(older.fee));
    }

    #[test]
    fn unproven_mutations_never_default_to_zero_or_resurrect_closed_fees() {
        let fee = EpochFee {
            epoch: 100,
            fee: TransferFee {
                basis_points: 100,
                maximum_fee: 10,
            },
        };
        for unknown in [Change::UnknownInitializeFee, Change::UnknownClose, Change::UnknownSetFee] {
            let mut state = State::default();
            state.apply(Change::InitializeFee(fee));
            state.apply(Change::InitializeMint);
            state.apply(unknown);
            assert_eq!(state.at(100), None);
            state.apply(Change::InitializeMint);
            assert_eq!(state.at(100), None);
        }
        let mut state = State::default();
        state.apply(Change::InitializeFee(fee));
        state.apply(Change::UnknownInitializeMint);
        assert_eq!(state.at(100), None);
        state.apply(Change::InitializeMint);
        assert_eq!(state.at(100), Some(fee.fee));
        state.apply(Change::Close);
        assert_eq!(state.at(100), None);
        state.apply(Change::InitializeMint);
        assert_eq!(state.at(100).unwrap().basis_points, 0);
    }

    #[test]
    fn a_completed_fee_change_recovers_only_when_its_new_schedule_activates() {
        let mut state = State::default();
        let fee = EpochFee {
            epoch: 100,
            fee: TransferFee {
                basis_points: 200,
                maximum_fee: 20,
            },
        };
        state.apply(Change::UnknownSetFee);
        state.apply(Change::SetFee(fee));
        assert_eq!(state.at(101), None);
        assert_eq!(state.at(102), Some(fee.fee));
    }

    #[test]
    fn owned_mints_require_the_program_base_layout_and_canonical_fee_tlv() {
        let data = include_bytes!("../fixtures/token-fees/dew.bin");
        let (schedule, initialized) = owned_mint(&token_2022::PROGRAM_ID, data).unwrap();
        assert!(initialized);
        assert_eq!(schedule.older.unwrap().epoch, 1039);
        assert_eq!(schedule.newer.unwrap().epoch, 1039);
        assert_eq!(schedule.at(1055).unwrap().basis_points, 100);
        let (no_fee, _) = owned_mint(&token_2022::PROGRAM_ID, include_bytes!("../fixtures/token-fees/pump.bin")).unwrap();
        assert_eq!(no_fee.at(1055).unwrap().basis_points, 0);
        assert!(owned_mint(&[0; 32], data).is_none());
        let mut reserved = vec![0; 355];
        reserved[45] = 1;
        reserved[165] = 1;
        reserved[166..168].copy_from_slice(&1u16.to_le_bytes());
        reserved[168..170].copy_from_slice(&108u16.to_le_bytes());
        assert!(owned_mint(&token_2022::PROGRAM_ID, &reserved).is_none());
        for position in [0, 45, 46, 82, 165] {
            let mut changed = data.to_vec();
            changed[position] = 9;
            assert!(owned_mint(&token_2022::PROGRAM_ID, &changed).is_none());
        }
        let mut changed = data.to_vec();
        changed[45] = 0;
        changed[165] = 0;
        let (schedule, initialized) = owned_mint(&token_2022::PROGRAM_ID, &changed).unwrap();
        let mut state = State::default();
        state.apply(Change::Snapshot { schedule, initialized });
        assert_eq!(state.at(1055), None);
        state.apply(Change::InitializeMint);
        assert_eq!(state.at(1055).unwrap().basis_points, 100);
        // The first extension is metadataPointer, followed by transferFeeConfig.
        // Locate TLV bounds rather than assuming the fee extension is first.
        let mut offset = 166;
        while u16::from_le_bytes(changed[offset..offset + 2].try_into().unwrap()) != 1 {
            offset += 4 + u16::from_le_bytes(changed[offset + 2..offset + 4].try_into().unwrap()) as usize;
        }
        changed[offset + 2] = 107;
        assert!(owned_mint(&token_2022::PROGRAM_ID, &changed).is_none());
        assert!(owned_mint(&token_2022::PROGRAM_ID, &data[..offset + 4 + 107]).is_none());
    }

    #[test]
    fn repeated_dlmm_receipts_match_independent_finalized_settlement() {
        use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
        let dew = base58::decode("DEW9dSN6QpWyNthphCpMmAbZP1Q4cEKR9xQXAri98WDP").unwrap();
        let pump = base58::decode("pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn").unwrap();
        let schedule = owned_mint(&token_2022::PROGRAM_ID, include_bytes!("../fixtures/token-fees/dew.bin")).unwrap().0;
        let pump_schedule = owned_mint(&token_2022::PROGRAM_ID, include_bytes!("../fixtures/token-fees/pump.bin"))
            .unwrap()
            .0;
        for (fixture, expected, baseline_count) in [
            (
                include_bytes!("../fixtures/token-fees/two-receipts.transaction.pb").as_slice(),
                vec![(17, 97_440_081_616), (23, 61_733_540_292)],
                1,
            ),
            (
                include_bytes!("../fixtures/token-fees/three-receipts.transaction.pb").as_slice(),
                vec![(13, 15_720_572_588), (19, 2_349_036_628), (25, 2_939_432_061)],
                2,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let baseline = crate::process_transaction(tx.clone()).unwrap();
            assert_eq!(baseline.swaps.len(), baseline_count);
            let decoded = crate::process_transaction_with_metadata(
                tx.clone(),
                None,
                |_, _| None,
                |_, _| None,
                |mint, _| {
                    if mint == dew {
                        schedule.at(1055)
                    } else if mint == pump {
                        pump_schedule.at(1055)
                    } else {
                        None
                    }
                },
            )
            .unwrap();
            assert_eq!(decoded.swaps.len(), baseline_count + expected.len());
            for (index, net) in expected {
                let swap = decoded.swaps.iter().find(|swap| swap.source_index == Some(index)).unwrap();
                assert_eq!(swap.protocol, proto::pb::dex::swaps::v1::Protocol::MeteoraDlmm as i32);
                assert_eq!(swap.output_mint, dew);
                assert_eq!(swap.output_amount, net);
                assert!(swap.transfer_verified);
            }
            for old in baseline.swaps {
                assert!(decoded.swaps.contains(&old), "existing financial row changed");
            }
            let wrong_fee = crate::process_transaction_with_metadata(
                tx.clone(),
                None,
                |_, _| None,
                |_, _| None,
                |mint, _| {
                    if mint == dew {
                        Some(TransferFee {
                            basis_points: 101,
                            maximum_fee: 1_000_000_000_000_000,
                        })
                    } else if mint == pump {
                        pump_schedule.at(1055)
                    } else {
                        None
                    }
                },
            )
            .unwrap();
            assert_eq!(
                wrong_fee.swaps.len(),
                baseline_count,
                "incorrect metadata must disagree with the finalized receipt ledger"
            );
            let mut truncated = tx.clone();
            truncated.meta.as_mut().unwrap().log_messages.clear();
            let unproven = crate::process_transaction_with_metadata(
                truncated,
                None,
                |_, _| None,
                |_, _| None,
                |mint, _| (mint == dew).then(|| schedule.at(1055)).flatten(),
            )
            .unwrap();
            assert!(unproven.swaps.iter().all(|swap| swap.output_mint != dew));
        }
    }

    #[test]
    fn fee_history_requires_completeness_and_rejects_corrupt_or_unproven_records() {
        let fee = EpochFee {
            epoch: 100,
            fee: TransferFee {
                basis_points: 100,
                maximum_fee: 10,
            },
        };
        let changes = [
            Change::InitializeFee(fee),
            Change::InitializeMint,
            Change::SetFee(fee),
            Change::UnknownSetFee,
            Change::UnknownClose,
            Change::Close,
            Change::UnknownInitializeFee,
            Change::UnknownInitializeMint,
            Change::Snapshot {
                schedule: Schedule {
                    older: Some(fee),
                    newer: Some(fee),
                },
                initialized: true,
            },
        ];
        for change in changes {
            assert_eq!(Change::decode(&change.encode()), Some(change));
        }
        let history: String = changes[..2]
            .iter()
            .map(|change| change.encode().iter().map(|byte| format!("{byte:02x}")).collect::<String>() + ";")
            .collect();
        assert_eq!(fee_at(Some(&history), false, 100), None);
        assert_eq!(fee_at(None, false, 100), None);
        assert_eq!(fee_at(None, true, 100).unwrap().basis_points, 0);
        assert_eq!(fee_at(Some(&history), true, 100), Some(fee.fee));
        assert_eq!(fee_at(Some(&(history.clone() + "06;")), true, 100), None);
        for malformed in ["", "xyz;", "00;", "03", "03;;", "0300;"] {
            assert_eq!(fee_at(Some(malformed), true, 100), None);
        }
    }

    #[test]
    fn native_changes_require_valid_encodings_and_committed_runtime_calls() {
        use substreams_solana::{
            base58,
            pb::sf::solana::r#type::v1::{Block, CompiledInstruction, ConfirmedTransaction, Message, Transaction, TransactionStatusMeta},
        };
        let mut init = vec![26, 0, 0, 0];
        init.extend_from_slice(&100u16.to_le_bytes());
        init.extend_from_slice(&10u64.to_le_bytes());
        let mut set = vec![26, 5];
        set.extend_from_slice(&200u16.to_le_bytes());
        set.extend_from_slice(&20u64.to_le_bytes());
        let mut initialize = vec![20, 6];
        initialize.extend_from_slice(&[8; 32]);
        initialize.push(0);
        let program = base58::encode(token_2022::PROGRAM_ID);
        let data = vec![init, initialize, set, vec![9]];
        let mut tx = ConfirmedTransaction {
            transaction: Some(Transaction {
                message: Some(Message {
                    account_keys: vec![vec![7; 32], vec![8; 32], token_2022::PROGRAM_ID.to_vec()],
                    instructions: data
                        .into_iter()
                        .map(|data| CompiledInstruction {
                            program_id_index: 2,
                            accounts: vec![0, 1, 1],
                            data,
                        })
                        .collect(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            meta: Some(TransactionStatusMeta {
                log_messages: (0..4)
                    .flat_map(|_| [format!("Program {program} invoke [1]"), format!("Program {program} success")])
                    .collect(),
                ..Default::default()
            }),
        };
        let mut block = Block {
            transactions: vec![tx.clone()],
            ..Default::default()
        };
        let changes = native_changes(&block, 100);
        assert_eq!(changes.len(), 4);
        assert!(matches!(changes[0].2, Change::InitializeFee(_)));
        assert_eq!(changes[1].2, Change::InitializeMint);
        assert!(matches!(changes[2].2, Change::SetFee(_)));
        assert_eq!(changes[3].2, Change::Close);
        assert!(changes.windows(2).all(|pair| pair[0].0 < pair[1].0));
        block.transactions[0].meta.as_mut().unwrap().log_messages.clear();
        assert_eq!(
            native_changes(&block, 100),
            changes,
            "confirmed transaction success proves root metadata changes"
        );
        // A wrapper can catch a failed CPI; move the same calls under it so
        // rollback and truncated-log tests reflect actual Solana semantics.
        let message = block.transactions[0].transaction.as_mut().unwrap().message.as_mut().unwrap();
        let originals = std::mem::take(&mut message.instructions);
        let wrapper_index = message.account_keys.len() as u32;
        message.account_keys.push(vec![9; 32]);
        message.instructions.push(CompiledInstruction {
            program_id_index: wrapper_index,
            accounts: vec![],
            data: vec![],
        });
        let meta = block.transactions[0].meta.as_mut().unwrap();
        meta.inner_instructions = vec![substreams_solana::pb::sf::solana::r#type::v1::InnerInstructions {
            index: 0,
            instructions: originals
                .into_iter()
                .map(|ix| substreams_solana::pb::sf::solana::r#type::v1::InnerInstruction {
                    program_id_index: ix.program_id_index,
                    accounts: ix.accounts,
                    data: ix.data,
                    stack_height: Some(2),
                })
                .collect(),
        }];
        let wrapper = base58::encode([9; 32]);
        meta.log_messages = vec![format!("Program {wrapper} invoke [1]")];
        for _ in 0..4 {
            meta.log_messages
                .extend([format!("Program {program} invoke [2]"), format!("Program {program} success")]);
        }
        meta.log_messages.push(format!("Program {wrapper} success"));
        meta.log_messages[6] = format!("Program {program} failed: caught");
        assert_eq!(native_changes(&block, 100).len(), 3);
        block.transactions[0].meta.as_mut().unwrap().log_messages.truncate(6);
        let changes = native_changes(&block, 100);
        assert_eq!(changes[2].2, Change::UnknownSetFee);
        assert_eq!(changes[3].2, Change::UnknownClose);
        tx.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data.push(0);
        assert_eq!(
            instruction_change(&tx.walk_instructions().next().unwrap(), 100).unwrap().1,
            Change::UnknownInitializeFee
        );
    }

    #[test]
    fn finalized_mainnet_epoch_schedule_matches_registry_epoch_arithmetic() {
        let data = include_bytes!("../fixtures/token-fees/epoch-schedule.bin");
        assert_eq!(u64::from_le_bytes(data[..8].try_into().unwrap()), 432_000);
        assert_eq!(u64::from_le_bytes(data[8..16].try_into().unwrap()), 432_000);
        assert_eq!(data[16], 0);
        assert_eq!(u64::from_le_bytes(data[17..25].try_into().unwrap()), 0);
        assert_eq!(u64::from_le_bytes(data[25..33].try_into().unwrap()), 0);
        assert_eq!(455_808_433u64 / 432_000, 1055);
    }

    #[test]
    fn compact_state_retains_pending_unknowns_and_epoch_activation() {
        let fee = EpochFee {
            epoch: 0,
            fee: TransferFee {
                basis_points: 100,
                maximum_fee: 100,
            },
        };
        let mut state = State::default();
        for change in [
            Change::InitializeFee(fee),
            Change::UnknownInitializeMint,
            Change::InitializeMint,
            Change::SetFee(EpochFee {
                epoch: 1,
                fee: TransferFee {
                    basis_points: 200,
                    maximum_fee: 200,
                },
            }),
            Change::UnknownClose,
            Change::InitializeMint,
            Change::Close,
            Change::InitializeMint,
        ] {
            state.apply(change);
            let encoded = state.encode();
            assert!(encoded.len() <= 40);
            let restored = State::decode(&encoded).unwrap();
            assert_eq!(restored.pending, state.pending);
            assert_eq!(restored.active, state.active);
            for epoch in [0, 1, 2, 3, u64::MAX] {
                assert_eq!(restored.at(epoch), state.at(epoch));
            }
        }
        for bytes in [vec![], vec![1], vec![3], vec![9, 0], vec![10, 0]] {
            assert!(State::decode(&bytes).is_none());
        }
    }

    #[test]
    fn complete_owned_registry_seeds_every_mint_once_and_preserves_pending_state() {
        use crate::token_fee_snapshot::{ANCHOR, COMPLETE_AT, COUNT, SEED};
        use substreams_solana::pb::sf::solana::r#type::v1::Block;
        let mut contexts = Vec::new();
        let mut mints = std::collections::HashSet::new();
        for row in SEED.chunks_exact(80) {
            let context = u64::from_le_bytes(row[..8].try_into().unwrap());
            assert!((ANCHOR..=COMPLETE_AT).contains(&context));
            assert!(contexts.last().map_or(true, |last| *last <= context));
            if contexts.last() != Some(&context) {
                contexts.push(context);
            }
            assert!(mints.insert(row[8..40].to_vec()));
        }
        assert_eq!(mints.len(), COUNT as usize);
        let mut parent_slot = ANCHOR - 1;
        let mut seeded = std::collections::HashSet::new();
        let mut pending = 0;
        for slot in contexts {
            let result = updates(&Block { slot, parent_slot, ..Default::default() }, SEED, ANCHOR, COMPLETE_AT).unwrap();
            for update in result.changes {
                assert_eq!(update.ordinal, i64::MAX as u64);
                assert!(seeded.insert(update.mint));
                let change = Change::decode(&update.change).unwrap();
                assert!(matches!(change, Change::Snapshot { .. }));
                let mut state = State::default();
                state.apply(change);
                let restored = State::decode(&state.encode()).unwrap();
                assert_eq!(state.at(slot / 432_000), restored.at(slot / 432_000));
                if state.at(slot / 432_000).is_none() {
                    pending += 1;
                    state.apply(Change::InitializeMint);
                    assert!(state.at(slot / 432_000).is_some());
                }
            }
            parent_slot = slot;
        }
        assert_eq!(seeded, mints);
        assert_eq!(pending, 38);
        let completion = updates(&Block { slot: COMPLETE_AT + 3, parent_slot, ..Default::default() }, SEED, ANCHOR, COMPLETE_AT).unwrap();
        assert_eq!(completion.changes.len(), 1);
        assert!(completion.changes[0].mint.is_empty());
        assert_eq!(completion.changes[0].ordinal, 0);
        assert_eq!(completion.changes[0].change, COMPLETE_AT.to_le_bytes());
        assert!(updates(&Block { slot: COMPLETE_AT + 4, parent_slot: COMPLETE_AT + 3, ..Default::default() }, SEED, ANCHOR, COMPLETE_AT).unwrap().changes.is_empty());
    }

    #[test]
    fn context_seeds_follow_native_changes_and_complete_after_all_read_slots() {
        use substreams_solana::{
            base58,
            pb::sf::solana::r#type::v1::{Block, CompiledInstruction, ConfirmedTransaction, Message, Transaction, TransactionStatusMeta},
        };
        let older = EpochFee {
            epoch: 0,
            fee: TransferFee {
                basis_points: 100,
                maximum_fee: 100,
            },
        };
        let newer = EpochFee {
            epoch: 2,
            fee: TransferFee {
                basis_points: 200,
                maximum_fee: 200,
            },
        };
        let mut seed = Vec::new();
        for (slot, mint, initialized) in [(100u64, 7u8, true), (102, 8, false), (104, 9, true)] {
            seed.extend_from_slice(&slot.to_le_bytes());
            seed.extend_from_slice(&[mint; 32]);
            older.encode(&mut seed);
            newer.encode(&mut seed);
            seed.extend_from_slice(&[u8::from(initialized), 0, 0, 0]);
        }
        let mut data = vec![26, 5];
        data.extend_from_slice(&200u16.to_le_bytes());
        data.extend_from_slice(&200u64.to_le_bytes());
        let program = base58::encode(token_2022::PROGRAM_ID);
        let tx = ConfirmedTransaction {
            transaction: Some(Transaction {
                message: Some(Message {
                    account_keys: vec![vec![7; 32], vec![8; 32], token_2022::PROGRAM_ID.to_vec()],
                    instructions: vec![CompiledInstruction {
                        program_id_index: 2,
                        accounts: vec![0, 1],
                        data,
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            meta: Some(TransactionStatusMeta {
                log_messages: vec![format!("Program {program} invoke [1]"), format!("Program {program} success")],
                ..Default::default()
            }),
        };
        let first = updates(
            &Block {
                slot: 100,
                parent_slot: 99,
                transactions: vec![tx],
                ..Default::default()
            },
            &seed,
            100,
            104,
        )
        .unwrap();
        assert_eq!(first.changes.len(), 2);
        assert_eq!(first.changes[1].ordinal, i64::MAX as u64);
        assert!(first.changes.iter().all(|item| !item.mint.is_empty()));
        let mut state = State::default();
        for update in first.changes {
            state.apply(Change::decode(&update.change).unwrap());
        }
        assert_eq!(state.at(1), Some(older.fee));
        assert_eq!(state.at(2), Some(newer.fee));
        let second = updates(
            &Block {
                slot: 102,
                parent_slot: 100,
                ..Default::default()
            },
            &seed,
            100,
            104,
        )
        .unwrap();
        assert_eq!(second.changes.len(), 1);
        assert_eq!(second.changes[0].mint, vec![8; 32]);
        let mut pending = State::default();
        pending.apply(Change::decode(&second.changes[0].change).unwrap());
        assert_eq!(pending.at(1), None);
        pending.apply(Change::InitializeMint);
        assert_eq!(pending.at(1), Some(older.fee));
        let third = updates(
            &Block {
                slot: 104,
                parent_slot: 102,
                ..Default::default()
            },
            &seed,
            100,
            104,
        )
        .unwrap();
        assert_eq!(third.changes.len(), 1);
        assert_eq!(third.changes[0].mint, vec![9; 32]);
        let marker = updates(
            &Block {
                slot: 107,
                parent_slot: 104,
                ..Default::default()
            },
            &seed,
            100,
            104,
        )
        .unwrap();
        assert_eq!(marker.changes.len(), 1);
        assert_eq!(marker.changes[0].ordinal, 0);
        assert!(marker.changes[0].mint.is_empty());
        assert_eq!(marker.changes[0].change, 104u64.to_le_bytes());
        assert!(updates(
            &Block {
                slot: 108,
                parent_slot: 107,
                ..Default::default()
            },
            &seed,
            100,
            104
        )
        .unwrap()
        .changes
        .is_empty());
        for position in [56, 76, 77] {
            let mut invalid = seed.clone();
            invalid[position] = if position == 56 { 0xff } else { 2 };
            if position == 56 {
                invalid[57] = 0xff;
            }
            assert!(updates(
                &Block {
                    slot: 100,
                    parent_slot: 99,
                    ..Default::default()
                },
                &invalid,
                100,
                104
            )
            .is_err());
        }
        assert!(updates(
            &Block {
                slot: 100,
                parent_slot: 99,
                ..Default::default()
            },
            &seed[..seed.len() - 1],
            100,
            104
        )
        .is_err());
        assert!(updates(
            &Block {
                slot: 100,
                parent_slot: 100,
                ..Default::default()
            },
            &seed,
            100,
            104
        )
        .is_err());
    }

    #[test]
    fn damm_referrals_do_not_replace_or_inflate_the_main_swap_receipt() {
        use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
        for (fixture, mint_data, mint, native_index, input_amount, output_amount) in [
            (
                include_bytes!("../fixtures/token-fees/damm-swap2-referral.transaction.pb").as_slice(),
                include_bytes!("../fixtures/token-fees/damm-swap2-referral.mint.bin").as_slice(),
                "Cy5TzDyJ4mo6MBAbpwUcAgdfNiXYKrYUPrvhPGVU6XMR",
                8,
                529_168_408,
                10_544_991_067,
            ),
            (
                include_bytes!("../fixtures/token-fees/damm-swap-referral.transaction.pb").as_slice(),
                include_bytes!("../fixtures/token-fees/damm-swap-referral.mint.bin").as_slice(),
                "44sHXMkPeciUpqhecfCysVs7RcaxeM24VPMauQouBREV",
                9,
                30_620_368_359,
                13_766_846_830_718,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let mint = base58::decode(mint).unwrap();
            let schedule = owned_mint(&token_2022::PROGRAM_ID, mint_data).unwrap().0;
            let decode = |transaction| {
                crate::process_transaction_with_metadata(
                    transaction,
                    None,
                    |_, _| None,
                    |_, _| None,
                    |key, _| (key == mint).then(|| schedule.at(1055)).flatten(),
                )
                .unwrap()
            };
            let decoded = decode(tx.clone());
            let rows: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(native_index)).collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].protocol, proto::pb::dex::swaps::v1::Protocol::MeteoraDaam as i32);
            assert_eq!((rows[0].input_amount, rows[0].output_amount), (input_amount, output_amount));
            assert_eq!(rows[0].source_transfer_index, Some(native_index + 1));
            assert!(rows[0].transfer_verified);
            let no_fee = crate::process_transaction_with_metadata(tx.clone(), None, |_, _| None, |_, _| None, |_, _| None).unwrap();
            assert!(no_fee.swaps.iter().all(|swap| swap.source_index != Some(native_index)));

            let native_data = tx.walk_instructions().nth(native_index as usize).unwrap().data().to_vec();
            let output_data = tx.walk_instructions().nth(native_index as usize + 2).unwrap().data().to_vec();
            let referral_data = tx.walk_instructions().nth(native_index as usize + 3).unwrap().data().to_vec();
            for role in [0, 2, 3, 8, 9, 10, 11] {
                let mut altered = tx.clone();
                let mut changed = 0;
                for group in &mut altered.meta.as_mut().unwrap().inner_instructions {
                    for instruction in &mut group.instructions {
                        if instruction.data == native_data {
                            instruction.accounts[role] = instruction.accounts[1];
                            changed += 1;
                        }
                    }
                }
                assert_eq!(changed, 1);
                assert!(decode(altered).swaps.iter().all(|swap| swap.source_index != Some(native_index)), "role {role}");
            }
            for (data, role) in [(&output_data, 0), (&output_data, 3), (&referral_data, 0), (&referral_data, 3)] {
                let mut altered = tx.clone();
                let mut changed = 0;
                for group in &mut altered.meta.as_mut().unwrap().inner_instructions {
                    for instruction in &mut group.instructions {
                        if &instruction.data == data {
                            instruction.accounts[role] = instruction.accounts[2];
                            changed += 1;
                        }
                    }
                }
                assert_eq!(changed, 1);
                assert!(decode(altered).swaps.iter().all(|swap| swap.source_index != Some(native_index)));
            }
            let mut hidden_event = tx.clone();
            let mut hidden = 0;
            for group in &mut hidden_event.meta.as_mut().unwrap().inner_instructions {
                for instruction in &mut group.instructions {
                    if matches!(
                        substreams_solana_idls::meteora::daam::anchor_cpi_event::unpack(&instruction.data),
                        Ok(substreams_solana_idls::meteora::daam::anchor_cpi_event::MeteoraDammAnchorCpiEvent::EvtSwap2(_))
                    ) {
                        instruction.data[8..16].fill(0);
                        hidden += 1;
                    }
                }
            }
            assert!(hidden > 0);
            let independent = decode(hidden_event);
            let rows: Vec<_> = independent.swaps.iter().filter(|swap| swap.source_index == Some(native_index)).collect();
            assert_eq!(rows.len(), 1);
            assert_eq!((rows[0].input_amount, rows[0].output_amount), (input_amount, output_amount));
            // The actual primary transfer remains first, even when zero; the
            // positive referral cannot be reclassified as the swap output.
            let mut zero_output = tx.clone();
            let mut changed = 0;
            for group in &mut zero_output.meta.as_mut().unwrap().inner_instructions {
                for instruction in &mut group.instructions {
                    if instruction.data == output_data {
                        instruction.data[1..9].fill(0);
                        changed += 1;
                    }
                }
            }
            assert_eq!(changed, 1);
            assert!(decode(zero_output).swaps.iter().all(|swap| swap.source_index != Some(native_index)));
            let mut missing_logs = tx;
            missing_logs.meta.as_mut().unwrap().log_messages.clear();
            assert!(decode(missing_logs).swaps.iter().all(|swap| swap.source_index != Some(native_index)));
        }
    }

    #[test]
    fn typed_dlmm_swaps_survive_missing_legacy_events_without_duplicate_rows() {
        use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
        let dew = base58::decode("DEW9dSN6QpWyNthphCpMmAbZP1Q4cEKR9xQXAri98WDP").unwrap();
        let schedule = owned_mint(&token_2022::PROGRAM_ID, include_bytes!("../fixtures/token-fees/dew.bin")).unwrap().0;
        for (fixture, expected) in [
            (
                include_bytes!("../fixtures/token-fees/two-receipts.transaction.pb").as_slice(),
                vec![(17, 97_440_081_616), (23, 61_733_540_292)],
            ),
            (
                include_bytes!("../fixtures/token-fees/three-receipts.transaction.pb").as_slice(),
                vec![(13, 15_720_572_588), (19, 2_349_036_628), (25, 2_939_432_061)],
            ),
        ] {
            let mut tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let mut hidden = 0;
            for group in &mut tx.meta.as_mut().unwrap().inner_instructions {
                for ix in &mut group.instructions {
                    if ix.data.len() == 145 && ix.data[8..16] == [81, 108, 227, 190, 205, 208, 10, 196] {
                        // Keep the actual CPI instruction position/runtime logs
                        // and the modern Swap2Evt intact; hide only legacy data.
                        ix.data[8..16].fill(0);
                        hidden += 1;
                    }
                }
            }
            assert!(hidden >= expected.len());
            let decoded = crate::process_transaction_with_metadata(
                tx.clone(),
                None,
                |_, _| None,
                |_, _| None,
                |mint, _| (mint == dew).then(|| schedule.at(1055)).flatten(),
            )
            .unwrap();
            for (index, amount) in &expected {
                let rows: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(*index)).collect();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].output_amount, *amount);
                assert_eq!(rows[0].protocol, proto::pb::dex::swaps::v1::Protocol::MeteoraDlmm as i32);
                assert!(rows[0].transfer_verified);
            }
            tx.meta.as_mut().unwrap().log_messages.clear();
            let unproven =
                crate::process_transaction_with_metadata(tx, None, |_, _| None, |_, _| None, |mint, _| (mint == dew).then(|| schedule.at(1055)).flatten())
                    .unwrap();
            assert!(unproven
                .swaps
                .iter()
                .all(|swap| !expected.iter().any(|(index, _)| swap.source_index == Some(*index))));
        }
    }
}
