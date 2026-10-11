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
pub(crate) struct Schedule {
    pub older: Option<EpochFee>,
    pub newer: Option<EpochFee>,
}

impl Schedule {
    pub(crate) fn at(self, epoch: u64) -> Option<TransferFee> {
        let applicable = match self.newer {
            Some(newer) if newer.epoch <= epoch => Some(newer),
            _ => self.older.filter(|fee| fee.epoch <= epoch),
        };
        applicable.map(|fee| fee.fee)
    }
}

/// Validate the owned mint, not token-account bytes or an untrusted event.
/// Unextended Token-2022 mints cannot contain a transfer fee configuration.
pub(crate) fn owned_mint(owner: &[u8], data: &[u8]) -> Option<(Schedule, bool)> {
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
            .filter_map(|(index, ix)| instruction_change(&ix, epoch).map(|(mint, change)| (index as u32, mint, change)))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let outcomes = crate::logs::invocation_outcomes(tx);
        for (index, mint, change) in candidates {
            if outcomes.rolled_back.contains(&index) {
                continue;
            }
            let change = if outcomes.completed.contains(&index) { change } else { change.unproven() };
            changes.push((crate::phoenix_metadata::ordinal(transaction_index, index), mint, change));
        }
    }
    changes
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
    if history.is_empty() || !history.ends_with(';') {
        return None;
    }
    let mut state = State::default();
    for record in history[..history.len() - 1].split(';') {
        if record.len() % 2 != 0 || record.is_empty() {
            return None;
        }
        let bytes: Option<Vec<u8>> = record
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let text = std::str::from_utf8(pair).ok()?;
                u8::from_str_radix(text, 16).ok()
            })
            .collect();
        state.apply(Change::decode(&bytes?)?);
    }
    state.at(epoch)
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
        block.transactions[0].meta.as_mut().unwrap().log_messages[5] = format!("Program {program} failed: caught");
        assert_eq!(native_changes(&block, 100).len(), 3);
        block.transactions[0].meta.as_mut().unwrap().log_messages.truncate(5);
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
}
