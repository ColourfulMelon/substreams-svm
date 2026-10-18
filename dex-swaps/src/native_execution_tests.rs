//! Execution-order and log-row attribution fixtures for the native API. Each
//! transaction is built from nested calls, so its instruction walk and runtime
//! invocation logs agree exactly unless a test deliberately breaks them.
use crate::{map_events, pb, project, InstructionPosition, InvocationOutcome, ProjectedSwap, Provenance, SwapKind};
use base64::Engine;
use flate2::read::GzDecoder;
use std::io::Read;
use substreams_solana::base58;
use substreams_solana::pb::sf::solana::r#type::v1 as source;
use substreams_solana_idls::{jupiter, orca, pumpfun::bonding_curve as pumpfun};

const PAYER: [u8; 32] = [0xfe; 32];
const TOKEN_PROGRAM: [u8; 32] = [0x77; 32];
const OTHER_PROGRAM: [u8; 32] = [0x78; 32];

struct Call {
    program: [u8; 32],
    accounts: Vec<[u8; 32]>,
    data: Vec<u8>,
    /// Lines written before the children run.
    before: Vec<String>,
    children: Vec<Call>,
    /// Lines written after the children return.
    after: Vec<String>,
    failure: Option<&'static str>,
}

impl Call {
    fn new(program: [u8; 32], accounts: Vec<[u8; 32]>, data: Vec<u8>) -> Self {
        Self {
            program,
            accounts,
            data,
            before: Vec::new(),
            children: Vec::new(),
            after: Vec::new(),
            failure: None,
        }
    }
    fn child(mut self, call: Call) -> Self {
        self.children.push(call);
        self
    }
    fn after(mut self, line: String) -> Self {
        self.after.push(line);
        self
    }
}

fn block_with(calls: Vec<Call>) -> source::Block {
    fn index(keys: &mut Vec<Vec<u8>>, key: [u8; 32]) -> u32 {
        if let Some(position) = keys.iter().position(|existing| existing.as_slice() == key) {
            return position as u32;
        }
        keys.push(key.to_vec());
        keys.len() as u32 - 1
    }
    fn walk(call: &Call, depth: u32, keys: &mut Vec<Vec<u8>>, inner: &mut Vec<source::InnerInstruction>, logs: &mut Vec<String>) {
        let program = base58::encode(call.program);
        logs.push(format!("Program {program} invoke [{depth}]"));
        logs.extend(call.before.iter().cloned());
        for child in &call.children {
            inner.push(source::InnerInstruction {
                program_id_index: index(keys, child.program),
                accounts: child.accounts.iter().map(|key| index(keys, *key) as u8).collect(),
                data: child.data.clone(),
                stack_height: Some(depth + 1),
            });
            walk(child, depth + 1, keys, inner, logs);
        }
        logs.extend(call.after.iter().cloned());
        logs.push(match call.failure {
            Some(message) => format!("Program {program} failed: {message}"),
            None => format!("Program {program} success"),
        });
    }

    let mut keys = vec![PAYER.to_vec()];
    let mut instructions = Vec::new();
    let mut inner_instructions = Vec::new();
    let mut logs = Vec::new();
    for (outer, call) in calls.iter().enumerate() {
        instructions.push(source::CompiledInstruction {
            program_id_index: index(&mut keys, call.program),
            accounts: call.accounts.iter().map(|key| index(&mut keys, *key) as u8).collect(),
            data: call.data.clone(),
        });
        let mut inner = Vec::new();
        walk(call, 1, &mut keys, &mut inner, &mut logs);
        if !inner.is_empty() {
            inner_instructions.push(source::InnerInstructions {
                index: outer as u32,
                instructions: inner,
            });
        }
    }
    let tx = source::ConfirmedTransaction {
        transaction: source::Transaction {
            signatures: vec![vec![7; 64]],
            message: source::Message {
                header: source::MessageHeader {
                    num_required_signatures: 1,
                    ..Default::default()
                }
                .into(),
                account_keys: keys,
                recent_blockhash: vec![0; 32],
                instructions,
                ..Default::default()
            }
            .into(),
        }
        .into(),
        meta: source::TransactionStatusMeta {
            inner_instructions,
            log_messages: logs,
            ..Default::default()
        }
        .into(),
    };
    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("../fixtures/solana-mainnet-350000026.pb.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    let mut block = <source::Block as buffa::Message>::decode_from_slice(bytes.as_slice()).unwrap();
    block.transactions = vec![tx];
    block
}

fn key(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn at(instruction_index: u32, inner_instruction_index: Option<u32>) -> InstructionPosition {
    InstructionPosition {
        instruction_index,
        inner_instruction_index,
    }
}

fn traded(pool: u8, a_to_b: bool, input_amount: u64, output_amount: u64) -> String {
    let event = orca::whirlpool::events::Traded {
        whirlpool: solana_program::pubkey::Pubkey::new_from_array(key(pool)),
        a_to_b,
        pre_sqrt_price: 0,
        post_sqrt_price: 0,
        input_amount,
        output_amount,
        input_transfer_fee: 0,
        output_transfer_fee: 0,
        lp_fee: 0,
        protocol_fee: 0,
    };
    let mut data = vec![225, 202, 73, 175, 147, 43, 160, 150];
    data.extend_from_slice(&borsh::to_vec(&event).unwrap());
    format!("Program data: {}", base64::engine::general_purpose::STANDARD.encode(data))
}

/// Orca `swap_v2` by `user` on `pool` (mints `0x51`/`0x52`), without its event.
fn orca_swap(user: u8, pool: u8, a_to_b: bool) -> Call {
    let accounts = vec![
        key(0x40),
        key(0x40),
        key(0x41),
        key(user),
        key(pool),
        key(0x51),
        key(0x52),
        key(0x42),
        key(0x43),
        key(0x44),
        key(0x45),
        key(0x46),
        key(0x47),
        key(0x48),
        key(0x49),
    ];
    let mut data = orca::whirlpool::instructions::SWAP_V2.to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&0u128.to_le_bytes());
    data.extend_from_slice(&[1, u8::from(a_to_b), 0]);
    Call::new(orca::whirlpool::PROGRAM_ID, accounts, data).child(Call::new(TOKEN_PROGRAM, vec![key(0x42), key(0x43)], vec![3]))
}

fn orca_two_hop(user: u8, pool_one: u8, pool_two: u8) -> Call {
    let mut accounts = vec![key(pool_one), key(pool_two), key(0x51), key(0x52), key(0x53)];
    accounts.extend((0x60..0x69).map(key));
    accounts.push(key(user));
    accounts.extend((0x69..0x71).map(key));
    accounts.push(key(0x41));
    assert_eq!(accounts.len(), 24);
    let mut data = orca::whirlpool::instructions::TWO_HOP_SWAP_V2.to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&[1, 1, 0]);
    data.extend_from_slice(&0u128.to_le_bytes());
    data.extend_from_slice(&0u128.to_le_bytes());
    data.push(0);
    Call::new(orca::whirlpool::PROGRAM_ID, accounts, data)
}

/// Pump.fun buy with its trade event self-CPI: an instruction-evidence row.
fn pumpfun_buy(user: u8) -> Call {
    let accounts: Vec<[u8; 32]> = (0x30..0x3c).map(key).collect();
    let mut buy = vec![102, 6, 61, 18, 1, 218, 235, 234];
    buy.extend_from_slice(&1_000u64.to_le_bytes());
    buy.extend_from_slice(&10_000u64.to_le_bytes());
    buy.push(1);
    let mut event = vec![0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
    event.extend_from_slice(&pumpfun::events::TRADE);
    event.extend_from_slice(&key(0x32));
    event.extend_from_slice(&1_000u64.to_le_bytes());
    event.extend_from_slice(&10_000u64.to_le_bytes());
    event.push(1);
    event.extend_from_slice(&key(user));
    Call::new(pumpfun::PROGRAM_ID, accounts, buy).child(Call::new(pumpfun::PROGRAM_ID, vec![key(0x3c)], event))
}

/// Jupiter v6 route whose swap event self-CPI summarizes `venue`'s leg.
fn jupiter_route(venue: Call) -> Call {
    let mut event = vec![228, 69, 165, 46, 81, 203, 154, 29];
    event.extend_from_slice(&[0; 8]);
    event.extend_from_slice(&orca::whirlpool::PROGRAM_ID);
    event.extend_from_slice(&key(0x51));
    event.extend_from_slice(&100u64.to_le_bytes());
    event.extend_from_slice(&key(0x52));
    event.extend_from_slice(&90u64.to_le_bytes());
    Call::new(jupiter::v6::PROGRAM_ID, vec![key(0x20)], vec![1, 2, 3])
        .child(venue)
        .child(Call::new(jupiter::v6::PROGRAM_ID, vec![key(0x21)], event))
}

fn project_rows(block: &source::Block) -> Vec<ProjectedSwap> {
    let projection = project(block).unwrap();
    assert_eq!(projection.clone().into_legacy(), map_events(block.clone()).unwrap());
    projection.transactions.into_iter().flat_map(|tx| tx.swaps).collect()
}

fn by_protocol(rows: &[ProjectedSwap], protocol: pb::Protocol) -> Vec<&ProjectedSwap> {
    rows.iter().filter(|row| row.swap.protocol == protocol).collect()
}

fn ordinals(rows: &[ProjectedSwap]) -> Vec<Option<u32>> {
    rows.iter().map(|row| row.execution_ordinal).collect()
}

#[test]
fn log_rows_attribute_to_their_emitter_and_interleave_with_instruction_rows() {
    // Orca's event line follows its own token CPI, inside a Jupiter route.
    let venue = orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90));
    let block = block_with(vec![jupiter_route(venue), pumpfun_buy(0x11)]);
    let rows = project_rows(&block);
    // Legacy adapter order: instruction rows, then log rows.
    let protocols: Vec<_> = rows.iter().map(|row| row.swap.protocol.as_known().unwrap()).collect();
    assert_eq!(protocols, [pb::Protocol::JupiterV6, pb::Protocol::Pumpfun, pb::Protocol::OrcaWhirlpool]);
    // Execution order: the Orca leg, then the Jupiter summary, then Pump.fun.
    assert_eq!(ordinals(&rows), [Some(1), Some(2), Some(0)]);

    let orca = by_protocol(&rows, pb::Protocol::OrcaWhirlpool)[0];
    assert!(matches!(orca.provenance, Provenance::Log { .. }));
    assert_eq!(orca.invocation, Some(at(0, Some(0))));
    assert_eq!(orca.invocation_outcome, InvocationOutcome::Succeeded);
    assert_eq!(orca.verified_context, Some(at(0, Some(0))));
    assert_eq!(orca.aggregator_route, Some(at(0, None)));

    let summary = by_protocol(&rows, pb::Protocol::JupiterV6)[0];
    assert_eq!(summary.kind, SwapKind::AggregatorSummary);
    assert_eq!(
        summary.provenance,
        Provenance::Instruction {
            position: at(0, Some(2)),
            event_index: 0
        }
    );
    assert_eq!(summary.invocation, Some(at(0, Some(2))));
    // The summary and the venue leg it describes share one route.
    assert_eq!(summary.aggregator_route, orca.aggregator_route);
    assert_eq!(summary.verified_context, None);

    let buy = by_protocol(&rows, pb::Protocol::Pumpfun)[0];
    assert_eq!(buy.invocation, Some(at(1, Some(0))));
    assert_eq!(buy.invocation_outcome, InvocationOutcome::Succeeded);
    assert_eq!(buy.aggregator_route, None);
}

#[test]
fn repeated_same_pool_calls_verify_only_the_matching_emitter() {
    let both = block_with(vec![
        orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90)),
        orca_swap(0x11, 0x50, true).after(traded(0x50, true, 200, 180)),
    ]);
    let rows = project_rows(&both);
    assert_eq!(rows.len(), 2);
    for (row, (outer, user, amount)) in rows.iter().zip([(0, 0x10, 100), (1, 0x11, 200)]) {
        assert_eq!(row.swap.user, key(user));
        assert_eq!(row.swap.input_amount, amount);
        assert_eq!(row.invocation, Some(at(outer, None)));
        assert_eq!(row.verified_context, Some(at(outer, None)));
    }
    assert_eq!(ordinals(&rows), [Some(0), Some(1)]);

    // The first call's event is not decodable, so the legacy FIFO pairs the
    // second event with the first call's user. The proved emitter is the
    // second call, whose user differs: the row stays, unverified.
    let stale = block_with(vec![
        orca_swap(0x10, 0x50, true).after("Program data: AAAAAAAAAAA=".to_string()),
        orca_swap(0x11, 0x50, true).after(traded(0x50, true, 200, 180)),
    ]);
    let rows = project_rows(&stale);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].swap.user, key(0x10));
    assert_eq!(rows[0].swap.input_amount, 200);
    assert_eq!(rows[0].invocation, Some(at(1, None)));
    assert_eq!(rows[0].invocation_outcome, InvocationOutcome::Succeeded);
    assert_eq!(rows[0].verified_context, None);
}

#[test]
fn multi_hop_instruction_verifies_each_hop_in_log_order() {
    let block = block_with(vec![orca_two_hop(0x10, 0x50, 0x55)
        .after(traded(0x50, true, 100, 90))
        .after(traded(0x55, false, 90, 80))]);
    let rows = project_rows(&block);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].swap.amm_pool, key(0x50));
    assert_eq!(rows[1].swap.amm_pool, key(0x55));
    for row in &rows {
        assert_eq!(row.invocation, Some(at(0, None)));
        assert_eq!(row.verified_context, Some(at(0, None)));
    }
    assert_eq!(ordinals(&rows), [Some(0), Some(1)]);
    // Hop two's event names the wrong direction: it no longer matches its hop.
    let block = block_with(vec![orca_two_hop(0x10, 0x50, 0x55)
        .after(traded(0x50, true, 100, 90))
        .after(traded(0x55, true, 90, 80))]);
    assert_eq!(project_rows(&block).len(), 1);
}

#[test]
fn failed_invocations_and_failed_ancestors_never_verify() {
    // Neither shape can occur in a successful transaction; the projection
    // must still stay conservative if a source reports one.
    for (venue_fails, route_fails) in [(true, false), (false, true)] {
        let mut venue = orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90));
        if venue_fails {
            venue.failure = Some("caught error");
        }
        let mut route = jupiter_route(venue);
        if route_fails {
            route.failure = Some("route error");
        }
        let rows = project_rows(&block_with(vec![route]));
        let orca = by_protocol(&rows, pb::Protocol::OrcaWhirlpool)[0];
        assert_eq!(orca.invocation, Some(at(0, Some(0))));
        assert_eq!(orca.invocation_outcome, InvocationOutcome::Failed);
        assert_eq!(orca.verified_context, None);
    }
}

#[test]
fn a_line_written_by_another_program_is_not_attributed() {
    // The adapter still reads the event while Orca is open, but the line was
    // written inside the token program's invocation.
    let mut venue = orca_swap(0x10, 0x50, true);
    venue.children[0].after.push(traded(0x50, true, 100, 90));
    let rows = project_rows(&block_with(vec![venue]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].invocation, None);
    assert_eq!(rows[0].invocation_outcome, InvocationOutcome::Unknown);
    assert_eq!(rows[0].verified_context, None);
    assert_eq!(rows[0].aggregator_route, None);
    // The trace itself is exact, so execution order is still proved.
    assert_eq!(rows[0].execution_ordinal, Some(0));
}

#[test]
fn truncated_ambiguous_or_misaligned_traces_leave_every_row_unattributed() {
    let base = || {
        block_with(vec![
            jupiter_route(orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90))),
            pumpfun_buy(0x11),
        ])
    };
    fn logs(block: &mut source::Block) -> &mut Vec<String> {
        &mut block.transactions[0].meta.as_option_mut().unwrap().log_messages
    }
    let mut truncated = base();
    logs(&mut truncated).push("Log truncated".to_string());
    let mut unbalanced = base();
    logs(&mut unbalanced).pop();
    let mut misaligned = base();
    // Retarget the Jupiter event self-CPI's invoke line; no adapter reads it.
    let event_cpi = format!("Program {} invoke [2]", base58::encode(jupiter::v6::PROGRAM_ID));
    let line = logs(&mut misaligned).iter().position(|line| line == &event_cpi).unwrap();
    logs(&mut misaligned)[line] = format!("Program {} invoke [2]", base58::encode(OTHER_PROGRAM));
    let mut missing_height = base();
    missing_height.transactions[0].meta.as_option_mut().unwrap().inner_instructions[0].instructions[0].stack_height = None;
    for block in [truncated, unbalanced, misaligned, missing_height] {
        let rows = project_rows(&block);
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert_eq!(row.invocation, None);
            assert_eq!(row.invocation_outcome, InvocationOutcome::Unknown);
            assert_eq!(row.verified_context, None);
            assert_eq!(row.aggregator_route, None);
            assert_eq!(row.execution_ordinal, None);
        }
    }
}

/// Invariants and per-protocol attribution counts on the pinned mainnet blocks.
#[test]
fn real_blocks_attribute_log_rows_without_changing_legacy_output() {
    use std::collections::BTreeMap;
    let manifest = env!("CARGO_MANIFEST_DIR");
    let fixtures = [
        format!("{manifest}/fixtures/solana-mainnet-350000026.pb.gz"),
        format!("{manifest}/../spl/transfers/tests/fixtures/solana-mainnet-349019252.pb.gz"),
        format!("{manifest}/../spl/transfers/tests/fixtures/solana-mainnet-350000009.pb.gz"),
        format!("{manifest}/../spl/transfers/tests/fixtures/solana-mainnet-350000010.pb.gz"),
    ];
    // protocol -> [rows, log rows, attributed log rows, verified rows, ordered rows, routed rows]
    let mut counts: BTreeMap<String, [usize; 6]> = BTreeMap::new();
    let mut unordered_transactions = 0;
    for path in &fixtures {
        let mut bytes = Vec::new();
        GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_end(&mut bytes).unwrap();
        let block = <source::Block as buffa::Message>::decode_from_slice(bytes.as_slice()).unwrap();
        let projection = project(&block).unwrap();
        assert_eq!(projection.clone().into_legacy(), map_events(block.clone()).unwrap());
        for transaction in &projection.transactions {
            let original = &block.transactions[transaction.transaction_index as usize];
            let instructions: BTreeMap<_, _> = crate::native::instruction_positions(original)
                .into_iter()
                .zip(original.walk_instructions())
                .collect();
            let ordered: Vec<_> = transaction.swaps.iter().filter_map(|row| row.execution_ordinal).collect();
            assert!(ordered.is_empty() || ordered.len() == transaction.swaps.len(), "ordinals are all-or-none");
            if ordered.is_empty() {
                unordered_transactions += 1;
            } else {
                let mut sorted = ordered.clone();
                sorted.sort_unstable();
                assert_eq!(sorted, (0..ordered.len() as u32).collect::<Vec<_>>());
                // Each evidence kind keeps its own legacy order.
                for log_rows in [false, true] {
                    let kind: Vec<_> = transaction
                        .swaps
                        .iter()
                        .filter(|row| matches!(row.provenance, Provenance::Log { .. }) == log_rows)
                        .map(|row| row.execution_ordinal.unwrap())
                        .collect();
                    assert!(kind.windows(2).all(|pair| pair[0] < pair[1]));
                }
            }
            for row in &transaction.swaps {
                let protocol = format!("{:?}", row.swap.protocol.as_known().unwrap());
                let entry = counts.entry(protocol).or_default();
                entry[0] += 1;
                entry[4] += usize::from(row.execution_ordinal.is_some());
                entry[5] += usize::from(row.aggregator_route.is_some());
                if row.verified_context.is_some() {
                    entry[3] += 1;
                    assert_eq!(row.invocation_outcome, InvocationOutcome::Succeeded);
                }
                if row.aggregator_route.is_some() {
                    assert!(row.invocation.is_some());
                }
                match row.provenance {
                    Provenance::Instruction { position, .. } => {
                        assert!(row.invocation.is_none() || row.invocation == Some(position));
                    }
                    Provenance::Log { .. } => {
                        entry[1] += 1;
                        if let Some(invocation) = row.invocation {
                            entry[2] += 1;
                            assert_eq!(instructions[&invocation].program_id().0.as_slice(), row.swap.program_id.as_slice());
                        }
                        if let Some(context) = row.verified_context {
                            assert_eq!(Some(context), row.invocation);
                        }
                    }
                }
                if row.kind == SwapKind::AggregatorSummary && row.invocation.is_some() {
                    assert!(row.aggregator_route.is_some());
                }
            }
        }
    }
    let summary: Vec<String> = counts
        .iter()
        .map(|(protocol, [rows, logs, attributed, verified, ordered, routed])| {
            format!("{protocol}: rows={rows} log={logs} attributed={attributed} verified={verified} ordered={ordered} routed={routed}")
        })
        .collect();
    eprintln!("{}\nunordered transactions={unordered_transactions}", summary.join("\n"));
    let golden = include_str!("../fixtures/native-execution-coverage.txt");
    assert_eq!(format!("{}\nunordered transactions={unordered_transactions}\n", summary.join("\n")), golden);
}
