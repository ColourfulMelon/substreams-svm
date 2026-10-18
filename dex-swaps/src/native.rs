//! Native complete-block API with source evidence separate from legacy rows.
use crate::{pb, process_transaction_with_provenance};
use std::collections::BTreeMap;
use std::fmt;
use substreams_solana::block_view::InstructionView;
use substreams_solana::pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction};
use substreams_solana_idls::{jupiter, okx};

spl_transfers::block_view_buffa!(fn view_block, substreams_solana::pb::sf::solana::r#type::v1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstructionPosition {
    pub instruction_index: u32,
    pub inner_instruction_index: Option<u32>,
}

/// Location of the evidence that emitted a decoded row, not a guessed location
/// of an earlier swap invocation. Self-CPI events carry their event instruction's
/// position. Log rows keep the original transaction log index; the invocation
/// that wrote the line is reported separately in [`ProjectedSwap::invocation`]
/// only when the complete invocation trace proves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    Instruction { position: InstructionPosition, event_index: u32 },
    Log { log_index: u32, event_index: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwapKind {
    PoolExecution,
    /// Jupiter and OKX summaries can duplicate separately decoded venue rows.
    AggregatorSummary,
}

/// Success requires an exact instruction/log invocation trace, including every
/// enclosing call. Missing, truncated, or ambiguous logs leave it unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationOutcome {
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedSwap {
    pub swap: pb::Swap,
    pub kind: SwapKind,
    pub provenance: Provenance,
    pub invocation_outcome: InvocationOutcome,
    /// The swap instruction whose account context and event fields
    /// independently rebuild this row. For DLMM instruction rows it is the
    /// immediate enclosing swap of the event self-CPI; for Orca Whirlpool and
    /// Raydium AMM v4/CLMM/CPMM log rows it is the proved invocation that wrote
    /// the event line. Requires proven success of that invocation and every
    /// ancestor. None means unverified, even when the event invocation succeeded.
    pub verified_context: Option<InstructionPosition>,
    /// The invocation that emitted this row's evidence, proved by an exact
    /// alignment of the instruction walk with the runtime invocation logs:
    /// the event instruction for instruction rows, or the innermost invocation
    /// executing when a log row's line was written. A log row is attributed
    /// only when that invocation's program is the row's `program_id`. None when
    /// the trace is missing, truncated, ambiguous or inconsistent.
    pub invocation: Option<InstructionPosition>,
    /// The outermost Jupiter v4, Jupiter v6 or OKX invocation enclosing (or
    /// equal to) `invocation`. Aggregator summaries and the venue legs they
    /// describe share it, so one routed trade can be counted once. None for
    /// unattributed rows and rows outside every aggregator invocation.
    pub aggregator_route: Option<InstructionPosition>,
    /// Position of this row in the transaction's execution order, counting
    /// from 0 across instruction and log rows. Rows are ordered by where their
    /// evidence appears in the proved invocation trace: an instruction row at
    /// its event instruction's `invoke` line, a log row at its own line, then
    /// by event index. Present for every row of a transaction or for none:
    /// None when the transaction's trace cannot be proved.
    pub execution_ordinal: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectedTransaction {
    /// Original index in the complete block, including votes and failures.
    pub transaction_index: u32,
    pub signature: Vec<u8>,
    pub fee_payer: Vec<u8>,
    pub signers: Vec<Vec<u8>>,
    pub fee: u64,
    pub compute_units_consumed: u64,
    /// Existing adapter output order (instruction rows, then log rows), kept
    /// identical to the legacy wrapper. Sort by
    /// [`ProjectedSwap::execution_ordinal`] for proved execution order.
    pub swaps: Vec<ProjectedSwap>,
}

impl ProjectedTransaction {
    pub(crate) fn into_legacy(self) -> pb::Transaction {
        pb::Transaction {
            signature: self.signature,
            fee_payer: self.fee_payer,
            signers: self.signers,
            fee: self.fee,
            compute_units_consumed: self.compute_units_consumed,
            swaps: self.swaps.into_iter().map(|row| row.swap).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clock {
    pub slot: u64,
    pub parent_slot: u64,
    pub blockhash: String,
    pub previous_blockhash: String,
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
    pub transaction_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    pub clock: Clock,
    pub transactions: Vec<ProjectedTransaction>,
}

impl Projection {
    /// Remove added provenance without changing any legacy row field or order.
    pub fn into_legacy(self) -> pb::Events {
        pb::Events {
            transactions: self.transactions.into_iter().map(ProjectedTransaction::into_legacy).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_transactions: usize,
    pub max_instructions: usize,
    pub max_log_messages: usize,
    pub max_log_bytes: usize,
    pub max_swaps: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_transactions: 100_000,
            max_instructions: 100_000,
            max_log_messages: 1_000_000,
            max_log_bytes: 64 * 1024 * 1024,
            max_swaps: 100_000,
        }
    }
}

#[derive(Debug)]
pub enum ProjectError {
    Capacity,
    InvalidBlock(spl_transfers::ProjectError),
    UnsupportedMessage(spl_transfers::UnsupportedMessage),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity => f.write_str("native swap projection capacity exceeded"),
            Self::InvalidBlock(_) => f.write_str("invalid complete Solana block"),
            Self::UnsupportedMessage(_) => f.write_str("unsupported Solana transaction version or inline configuration"),
        }
    }
}
impl std::error::Error for ProjectError {}

/// Validate and project one complete block without executing Substreams. This
/// exposes existing adapter behavior; it does not repair missing variants,
/// pool/event association defects or prove the success of every inner call.
pub fn project(block: &Block) -> Result<Projection, ProjectError> {
    project_with_limits(block, Limits::default())
}

pub fn project_with_limits(block: &Block, limits: Limits) -> Result<Projection, ProjectError> {
    if block.transactions.len() > limits.max_transactions || block.transactions.len() > u32::MAX as usize {
        return Err(ProjectError::Capacity);
    }
    let (mut instructions, mut logs, mut log_bytes) = (0usize, 0usize, 0usize);
    for tx in &block.transactions {
        if let Some(message) = tx.transaction.as_option().and_then(|tx| tx.message.as_option()) {
            instructions = instructions.saturating_add(message.instructions.len());
        }
        if let Some(meta) = tx.meta.as_option() {
            for group in &meta.inner_instructions {
                instructions = instructions.saturating_add(group.instructions.len());
            }
            logs = logs.saturating_add(meta.log_messages.len());
            for log in &meta.log_messages {
                log_bytes = log_bytes.saturating_add(log.len());
            }
        }
        if instructions > limits.max_instructions
            || instructions > u32::MAX as usize
            || logs > limits.max_log_messages
            || logs > u32::MAX as usize
            || log_bytes > limits.max_log_bytes
        {
            return Err(ProjectError::Capacity);
        }
    }
    let view = view_block(block).map_err(ProjectError::UnsupportedMessage)?;
    spl_transfers::validate_block(&view).map_err(ProjectError::InvalidBlock)?;
    let mut transactions = Vec::new();
    let mut swaps = 0usize;
    for (index, tx) in block.transactions.iter().enumerate() {
        if tx.meta.as_option().is_some_and(|meta| meta.err.is_set()) {
            continue;
        }
        if let Some(mut transaction) = process_transaction_with_provenance(tx, limits.max_swaps.saturating_sub(swaps))? {
            transaction.transaction_index = index as u32;
            if let Some(trace) = crate::native_outcome::prove(tx) {
                annotate(tx, &trace, &mut transaction.swaps);
            }
            swaps += transaction.swaps.len();
            transactions.push(transaction);
        }
    }
    Ok(Projection {
        clock: Clock {
            slot: block.slot,
            parent_slot: block.parent_slot,
            blockhash: block.blockhash.clone(),
            previous_blockhash: block.previous_blockhash.clone(),
            block_time: block.block_time.as_option().map(|time| time.timestamp),
            block_height: block.block_height.as_option().map(|height| height.block_height),
            transaction_count: block.transactions.len() as u32,
        },
        transactions,
    })
}

/// Add proved invocation evidence to rows already extracted from `tx`. Row
/// order and every legacy field stay unchanged.
fn annotate(tx: &ConfirmedTransaction, trace: &crate::native_outcome::Trace, rows: &mut [ProjectedSwap]) {
    let Some(meta) = tx.meta.as_option() else { return };
    let instructions: BTreeMap<_, _> = instruction_positions(tx).into_iter().zip(tx.walk_instructions()).collect();
    let token_mints = crate::token_mints::TokenMintLookup::new(tx, meta);
    let mut order_keys = Vec::with_capacity(rows.len());
    for row in rows.iter_mut() {
        let order_key = match row.provenance {
            Provenance::Instruction { position, event_index } => {
                let evidence = trace.evidence.get(&position);
                if let Some(evidence) = evidence {
                    row.invocation_outcome = evidence.outcome;
                    if evidence.invoke_log_index.is_some() {
                        row.invocation = Some(position);
                    }
                    if evidence.outcome == InvocationOutcome::Succeeded {
                        if let Some(parent) = evidence.parent {
                            if let (Some(event), Some(context)) = (instructions.get(&position), instructions.get(&parent)) {
                                if crate::meteora_dlmm::matches_context(context, event, &row.swap) {
                                    row.verified_context = Some(parent);
                                }
                            }
                        }
                    }
                }
                evidence.and_then(|evidence| evidence.invoke_log_index).map(|line| (line, event_index))
            }
            Provenance::Log { log_index, event_index } => {
                let emitter = trace.emitters.get(log_index as usize).copied().flatten();
                if let Some((emitter, view, evidence)) = emitter.and_then(|emitter| Some((emitter, instructions.get(&emitter)?, trace.evidence.get(&emitter)?)))
                {
                    if view.program_id().0.as_slice() == row.swap.program_id.as_slice() {
                        row.invocation = Some(emitter);
                        row.invocation_outcome = evidence.outcome;
                        let line = &meta.log_messages[log_index as usize];
                        if evidence.outcome == InvocationOutcome::Succeeded && verify_log_context(view, line, &token_mints, &row.swap) {
                            row.verified_context = Some(emitter);
                        }
                    }
                }
                Some((log_index, event_index))
            }
        };
        row.aggregator_route = row.invocation.and_then(|invocation| aggregator_route(invocation, trace, &instructions));
        order_keys.push(order_key);
    }
    // Every key is a distinct (trace line, event index) pair: an instruction
    // row's `invoke` line is never a log row's evidence line.
    if let Some(keys) = order_keys.into_iter().collect::<Option<Vec<_>>>() {
        let mut order: Vec<usize> = (0..rows.len()).collect();
        order.sort_by_key(|&index| (keys[index], index));
        for (ordinal, index) in order.into_iter().enumerate() {
            rows[index].execution_ordinal = Some(ordinal as u32);
        }
    }
}

fn verify_log_context(emitter: &InstructionView, line: &str, token_mints: &crate::token_mints::TokenMintLookup, row: &pb::Swap) -> bool {
    match row.protocol.as_known() {
        Some(pb::Protocol::OrcaWhirlpool) => crate::orca_whirlpool::verify_log_row(emitter, line, token_mints, row),
        Some(pb::Protocol::RaydiumAmmV4) => crate::raydium_amm_v4::verify_log_row(emitter, line, token_mints, row),
        Some(pb::Protocol::RaydiumClmm) => crate::raydium_clmm::verify_log_row(emitter, line, token_mints, row),
        Some(pb::Protocol::RaydiumCpmm) => crate::raydium_cpmm::verify_log_row(emitter, line, row),
        _ => false,
    }
}

fn aggregator_route(
    invocation: InstructionPosition,
    trace: &crate::native_outcome::Trace,
    instructions: &BTreeMap<InstructionPosition, InstructionView>,
) -> Option<InstructionPosition> {
    let mut route = None;
    let mut current = Some(invocation);
    while let Some(position) = current {
        let program = instructions.get(&position)?.program_id().0;
        if program == &jupiter::v4::PROGRAM_ID || program == &jupiter::v6::PROGRAM_ID || program == &okx::v2::PROGRAM_ID {
            route = Some(position);
        }
        current = trace.evidence.get(&position)?.parent;
    }
    route
}

pub(crate) fn instruction_positions(tx: &ConfirmedTransaction) -> Vec<InstructionPosition> {
    let Some(message) = tx.transaction.as_option().and_then(|tx| tx.message.as_option()) else {
        return Vec::new();
    };
    // Match the SDK walk's last-entry behavior without a repeated linear scan.
    // The validated native API rejects duplicate parent entries beforehand.
    let inner_by_outer: std::collections::HashMap<_, _> = tx
        .meta
        .as_option()
        .into_iter()
        .flat_map(|meta| &meta.inner_instructions)
        .map(|group| (group.index, group))
        .collect();
    let mut positions = Vec::new();
    for index in 0..message.instructions.len() {
        positions.push(InstructionPosition {
            instruction_index: index as u32,
            inner_instruction_index: None,
        });
        if let Some(group) = inner_by_outer.get(&(index as u32)) {
            for inner in 0..group.instructions.len() {
                positions.push(InstructionPosition {
                    instruction_index: index as u32,
                    inner_instruction_index: Some(inner as u32),
                });
            }
        }
    }
    positions
}

pub(crate) struct Collector {
    rows: Vec<ProjectedSwap>,
    current: Provenance,
    max_swaps: usize,
    overflow: bool,
}

impl Collector {
    pub fn new(max_swaps: usize) -> Self {
        Self {
            rows: Vec::new(),
            current: Provenance::Log { log_index: 0, event_index: 0 },
            max_swaps,
            overflow: false,
        }
    }
    pub fn at_instruction(&mut self, position: InstructionPosition) {
        self.current = Provenance::Instruction { position, event_index: 0 };
    }
    pub fn at_log(&mut self, log_index: u32) {
        self.current = Provenance::Log { log_index, event_index: 0 };
    }
    pub fn push(&mut self, swap: pb::Swap) {
        if self.rows.len() >= self.max_swaps {
            self.overflow = true;
            return;
        }
        let kind = if matches!(
            swap.protocol.as_known(),
            Some(pb::Protocol::JupiterV4 | pb::Protocol::JupiterV6 | pb::Protocol::OkxDex)
        ) {
            SwapKind::AggregatorSummary
        } else {
            SwapKind::PoolExecution
        };
        self.rows.push(ProjectedSwap {
            swap,
            kind,
            provenance: self.current,
            invocation_outcome: InvocationOutcome::Unknown,
            verified_context: None,
            invocation: None,
            aggregator_route: None,
            execution_ordinal: None,
        });
        match &mut self.current {
            Provenance::Instruction { event_index, .. } | Provenance::Log { event_index, .. } => *event_index += 1,
        }
    }
    pub fn finish(self) -> Result<Vec<ProjectedSwap>, ProjectError> {
        if self.overflow {
            Err(ProjectError::Capacity)
        } else {
            Ok(self.rows)
        }
    }
}
