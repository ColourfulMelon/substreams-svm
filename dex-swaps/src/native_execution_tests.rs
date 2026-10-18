//! Execution-order and log-row attribution fixtures for the native API. Each
//! transaction is built from nested calls, so its instruction walk and runtime
//! invocation logs agree exactly unless a test deliberately breaks them.
use crate::{map_events, pb, project, InstructionPosition, InvocationOutcome, ProjectedSwap, Provenance, SwapKind};
use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::read::GzDecoder;
use solana_program::pubkey::Pubkey;
use std::io::Read;
use substreams_solana::base58;
use substreams_solana::pb::sf::solana::r#type::v1 as source;
use substreams_solana_idls::{jupiter, meteora, okx, orca, pumpfun::bonding_curve as pumpfun, raydium};

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
    fn before(mut self, line: String) -> Self {
        self.before.push(line);
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

fn pubkey(byte: u8) -> Pubkey {
    Pubkey::new_from_array(key(byte))
}

fn program_data(data: Vec<u8>) -> String {
    format!("Program data: {}", STANDARD.encode(data))
}

fn traded(pool: u8, a_to_b: bool, input_amount: u64, output_amount: u64) -> String {
    let event = orca::whirlpool::events::Traded {
        whirlpool: pubkey(pool),
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
    program_data(data)
}

/// A token-program transfer CPI, as every venue makes.
fn transfer() -> Call {
    Call::new(TOKEN_PROGRAM, vec![key(0x42), key(0x43)], vec![3])
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
    Call::new(orca::whirlpool::PROGRAM_ID, accounts, data).child(transfer())
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
    // Hop two's event names the wrong direction: it no longer matches its hop,
    // and hop one still verifies on its own.
    let block = block_with(vec![orca_two_hop(0x10, 0x50, 0x55)
        .after(traded(0x50, true, 100, 90))
        .after(traded(0x55, true, 90, 80))]);
    let rows = project_rows(&block);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].swap.amm_pool, key(0x50));
    assert_eq!(rows[0].invocation, Some(at(0, None)));
    assert_eq!(rows[0].verified_context, Some(at(0, None)));
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
    // The trace itself is exact, so the evidence line is still ranked.
    assert_eq!(rows[0].execution_ordinal, Some(0));
}

#[test]
fn a_leg_written_by_another_program_has_unknown_route_membership() {
    // Inside a proved Jupiter route, the leg's event line has a foreign writer:
    // no invocation, so its None route means "unknown", not "unrouted". Only
    // the summary, whose invocation is proved, joins the route.
    let mut venue = orca_swap(0x10, 0x50, true);
    venue.children[0].after.push(traded(0x50, true, 100, 90));
    let rows = project_rows(&block_with(vec![jupiter_route(venue)]));
    let orca = by_protocol(&rows, pb::Protocol::OrcaWhirlpool)[0];
    let summary = by_protocol(&rows, pb::Protocol::JupiterV6)[0];
    assert_eq!((orca.invocation, orca.aggregator_route), (None, None));
    assert!(orca.execution_ordinal.is_some());
    assert_eq!(summary.aggregator_route, Some(at(0, None)));
}

#[test]
fn forged_runtime_text_and_sibling_calls_keep_each_line_with_its_writer() {
    // A program can only write runtime-looking text behind `Program log:`.
    let whirlpool = base58::encode(orca::whirlpool::PROGRAM_ID);
    let mut venue = orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90));
    venue.children[0].after.push(format!("Program log: Program {whirlpool} invoke [3]"));
    venue.children[0].after.push(format!("Program log: Program {whirlpool} success"));
    let rows = project_rows(&block_with(vec![venue]));
    assert_eq!(rows[0].invocation, Some(at(0, None)));
    assert_eq!(rows[0].verified_context, Some(at(0, None)));
    // Two sibling calls to the same program at the same depth.
    let route = Call::new(jupiter::v6::PROGRAM_ID, vec![key(0x20)], vec![1, 2, 3])
        .child(orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90)))
        .child(orca_swap(0x10, 0x50, true).after(traded(0x50, true, 90, 80)));
    let rows = project_rows(&block_with(vec![route]));
    assert_eq!(rows[0].invocation, Some(at(0, Some(0))));
    assert_eq!(rows[1].invocation, Some(at(0, Some(2))));
    assert!(rows.iter().all(|row| row.verified_context == row.invocation));
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

// ---------------------------------------------------------------------------
// Log-row verifiers. Each legacy adapter below pairs events with instructions
// by sequence or FIFO, so a missing, extra or undecodable call shifts the
// pairing. The proved writer of the event line must then leave the row
// unverified while `invocation` still names that writer.
// ---------------------------------------------------------------------------

const MINT_IN: u8 = 0x91;
const MINT_OUT: u8 = 0x92;

/// Record each token account's mint, as a transaction's pre-balances do.
/// Accounts absent from the transaction are ignored.
fn with_mints(mut block: source::Block, accounts: &[(u8, u8)]) -> source::Block {
    let tx = &mut block.transactions[0];
    let keys = tx.transaction.as_option().unwrap().message.as_option().unwrap().account_keys.clone();
    let meta = tx.meta.as_option_mut().unwrap();
    for &(account, mint) in accounts {
        if let Some(index) = keys.iter().position(|existing| existing.as_slice() == key(account)) {
            meta.pre_token_balances.push(source::TokenBalance {
                account_index: index as u32,
                mint: base58::encode(key(mint)),
                ui_token_amount: Some(source::UiTokenAmount::default()).into(),
                ..Default::default()
            });
        }
    }
    block
}

/// The only row pairs `pool` with `input_amount` taken from evidence that
/// `writer` wrote for another call: attributed to the writer, never verified.
fn assert_stale(rows: &[ProjectedSwap], writer: InstructionPosition, pool: u8, input_amount: u64) {
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!((row.swap.amm_pool.as_slice(), row.swap.input_amount), (key(pool).as_slice(), input_amount));
    assert_eq!(row.invocation, Some(writer));
    assert_eq!(row.invocation_outcome, InvocationOutcome::Succeeded);
    assert_eq!(row.verified_context, None);
    assert_eq!(row.execution_ordinal, Some(0));
}

/// Every row is verified against the invocation that wrote its evidence.
fn assert_verified(rows: &[ProjectedSwap], writers: &[InstructionPosition]) {
    assert_eq!(rows.len(), writers.len());
    for (row, writer) in rows.iter().zip(writers) {
        assert_eq!(row.invocation, Some(*writer));
        assert_eq!(row.verified_context, Some(*writer));
    }
}

/// Raydium AMM v4 `swap_base_in_v2` by `owner` on `pool`, whose coin and pc
/// vaults are `pool + 1` and `pool + 2`. Add its `ray_log` with `before`: on
/// chain it precedes the swap's token transfers.
fn amm_v4_swap(pool: u8, owner: u8) -> Call {
    let accounts = vec![
        TOKEN_PROGRAM,
        key(pool),
        key(0x80),
        key(pool + 1),
        key(pool + 2),
        key(0x83),
        key(0x84),
        key(owner),
    ];
    let mut data = vec![16];
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    Call::new(raydium::amm::v4::PROGRAM_ID, accounts, data).child(transfer())
}

/// `ray_log` of a coin-to-pc `SwapBaseIn`.
fn ray_log(amount_in: u64, amount_out: u64) -> String {
    let mut data = vec![3];
    for value in [amount_in, 1, 2, 0, 0, 0, amount_out] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    format!("Program log: ray_log: {}", STANDARD.encode(data))
}

const VAULT_MINTS: [(u8, u8); 4] = [(0x71, MINT_IN), (0x72, MINT_OUT), (0x75, MINT_IN), (0x76, MINT_OUT)];

#[test]
fn raydium_amm_v4_verifies_only_the_swap_that_wrote_its_log() {
    let project = |calls| project_rows(&with_mints(block_with(calls), &VAULT_MINTS));
    let rows = project(vec![
        amm_v4_swap(0x70, 0x10).before(ray_log(100, 90)),
        amm_v4_swap(0x74, 0x11).before(ray_log(200, 180)),
    ]);
    assert_verified(&rows, &[at(0, None), at(1, None)]);
    assert_eq!(rows[1].swap.amm_pool, key(0x74));

    // The first call does not decode (seven accounts) but still writes its
    // log, which the sequence pairs with the second call's pool.
    let mut undecodable = amm_v4_swap(0x70, 0x10).before(ray_log(100, 90));
    undecodable.accounts.truncate(7);
    let rows = project(vec![undecodable, amm_v4_swap(0x74, 0x11).before(ray_log(200, 180))]);
    assert_stale(&rows, at(0, None), 0x74, 100);

    // An extra call writes no log: the next log pairs with its pool.
    let rows = project(vec![amm_v4_swap(0x70, 0x10), amm_v4_swap(0x74, 0x11).before(ray_log(200, 180))]);
    assert_stale(&rows, at(1, None), 0x70, 200);
}

/// Raydium CLMM `swap_v2` by `payer` on `pool` (vaults `pool + 1`/`pool + 2`).
fn clmm_swap(payer: u8, pool: u8) -> Call {
    let accounts = vec![
        key(payer),
        key(0xa0),
        key(pool),
        key(0xa1),
        key(0xa2),
        key(pool + 1),
        key(pool + 2),
        key(0xa3),
        TOKEN_PROGRAM,
        key(0xa4),
        key(0xa5),
        key(MINT_IN),
        key(MINT_OUT),
    ];
    let mut data = raydium::clmm::v3::instructions::SWAP_V2.to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.extend_from_slice(&0u128.to_le_bytes());
    data.push(1);
    Call::new(raydium::clmm::v3::PROGRAM_ID, accounts, data).child(transfer())
}

/// Raydium CLMM `swap_router_base_in` by `payer`. Its hop events name their
/// own pools and vaults; the vaults `0xb3..=0xb6` are its remaining accounts.
fn clmm_router(payer: u8) -> Call {
    let mut accounts = vec![key(payer), key(0xb0), key(MINT_IN), TOKEN_PROGRAM, key(0xb1), key(0xb2)];
    accounts.extend((0xb3..=0xb6).map(key));
    let mut data = raydium::clmm::v3::instructions::SWAP_ROUTER_BASE_IN.to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    Call::new(raydium::clmm::v3::PROGRAM_ID, accounts, data).child(transfer())
}

/// CLMM `SwapEvent` on `pool`, token 0 (vault `vault_0`) in, token 1 out.
fn clmm_event(pool: u8, vault_0: u8, vault_1: u8, amount_in: u64, amount_out: u64) -> String {
    use raydium::clmm::v3::events::{SwapEvent, SWAP_EVENT};
    let event = SwapEvent {
        pool_state: pubkey(pool),
        sender: pubkey(0x01),
        token_account_0: pubkey(vault_0),
        token_account_1: pubkey(vault_1),
        amount_0: amount_in,
        transfer_fee_0: 0,
        amount_1: amount_out,
        transfer_fee_1: 0,
        zero_for_one: true,
        sqrt_price_x64: 0,
        liquidity: 0,
        tick: 0,
    };
    let mut data = SWAP_EVENT.to_vec();
    data.extend_from_slice(&borsh::to_vec(&event).unwrap());
    program_data(data)
}

#[test]
fn raydium_clmm_verifies_swaps_and_router_hops_only_against_their_writer() {
    let mints = [VAULT_MINTS.as_slice(), &[(0xb3, MINT_IN), (0xb4, 0x93), (0xb5, 0x93), (0xb6, MINT_OUT)]].concat();
    let project = |calls| project_rows(&with_mints(block_with(calls), &mints));
    let rows = project(vec![
        clmm_swap(0x10, 0x70).after(clmm_event(0x70, 0x71, 0x72, 100, 90)),
        clmm_router(0x11)
            .after(clmm_event(0xc0, 0xb3, 0xb4, 100, 80))
            .after(clmm_event(0xc1, 0xb5, 0xb6, 80, 70)),
    ]);
    assert_verified(&rows, &[at(0, None), at(1, None), at(1, None)]);
    // Router hops take their pools from their own events.
    assert_eq!(
        (rows[1].swap.amm_pool.clone(), rows[2].swap.amm_pool.clone()),
        (key(0xc0).to_vec(), key(0xc1).to_vec())
    );

    // The first swap's event does not decode, so the second swap's event
    // pairs with the first swap's pool.
    let rows = project(vec![
        clmm_swap(0x10, 0x70).after("Program data: AAAA".to_string()),
        clmm_swap(0x11, 0x74).after(clmm_event(0x74, 0x75, 0x76, 200, 180)),
    ]);
    assert_stale(&rows, at(1, None), 0x70, 200);

    // A router that writes no event keeps its placeholder, so the next
    // router's event takes the first router's payer.
    let rows = project(vec![clmm_router(0x10), clmm_router(0x11).after(clmm_event(0xc0, 0xb3, 0xb4, 100, 80))]);
    assert_eq!(rows[0].swap.user, key(0x10));
    assert_stale(&rows, at(1, None), 0xc0, 100);

    // Same payer, but the event's writer is a nested router: the row carries
    // the top-level router's stack height, not its writer's.
    let nested = Call::new(OTHER_PROGRAM, vec![key(0x2f)], vec![1]).child(clmm_router(0x10).after(clmm_event(0xc0, 0xb3, 0xb4, 100, 80)));
    let rows = project(vec![clmm_router(0x10), nested]);
    assert_eq!((rows[0].swap.user.as_slice(), rows[0].swap.stack_height), (key(0x10).as_slice(), 0));
    assert_stale(&rows, at(1, Some(0)), 0xc0, 100);
}

/// Raydium CPMM `swap_base_input` by `payer` on `pool`.
fn cpmm_swap(payer: u8, pool: u8) -> Call {
    let accounts = vec![
        key(payer),
        key(0xc8),
        key(0xc9),
        key(pool),
        key(0xca),
        key(0xcb),
        key(pool + 1),
        key(pool + 2),
        TOKEN_PROGRAM,
        TOKEN_PROGRAM,
        key(MINT_IN),
        key(MINT_OUT),
        key(0xcc),
    ];
    let mut data = raydium::cpmm::instructions::SWAP_BASE_INPUT.to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    Call::new(raydium::cpmm::PROGRAM_ID, accounts, data).child(transfer())
}

/// CPMM `SwapEvent` (v1 layout, no mints) on `pool`.
fn cpmm_event(pool: u8, input_amount: u64, output_amount: u64) -> String {
    use raydium::cpmm::events::{SwapEventV1, SWAP_EVENT};
    let event = SwapEventV1 {
        pool_id: pubkey(pool),
        input_vault_before: 0,
        output_vault_before: 0,
        input_amount,
        output_amount,
        input_transfer_fee: 0,
        output_transfer_fee: 0,
        base_input: true,
    };
    let mut data = SWAP_EVENT.to_vec();
    data.extend_from_slice(&borsh::to_vec(&event).unwrap());
    program_data(data)
}

#[test]
fn raydium_cpmm_verifies_only_the_swap_that_wrote_its_event() {
    let rows = project_rows(&block_with(vec![
        cpmm_swap(0x10, 0x70).after(cpmm_event(0x70, 100, 90)),
        cpmm_swap(0x11, 0x74).after(cpmm_event(0x74, 200, 180)),
    ]));
    assert_verified(&rows, &[at(0, None), at(1, None)]);
    let rows = project_rows(&block_with(vec![
        cpmm_swap(0x10, 0x70).after("Program data: AAAA".to_string()),
        cpmm_swap(0x11, 0x74).after(cpmm_event(0x74, 200, 180)),
    ]));
    assert_stale(&rows, at(1, None), 0x70, 200);
}

/// Meteora AMM `swap` by `user` on `pool`, from token account `0xd1` to `0xd2`.
fn meteora_amm_swap(user: u8, pool: u8) -> Call {
    let mut accounts = vec![key(pool), key(0xd1), key(0xd2)];
    accounts.extend((0xd3..0xdc).map(key));
    accounts.extend([key(user), key(0xdc), TOKEN_PROGRAM]);
    assert_eq!(accounts.len(), 15);
    let mut data = meteora::amm::instructions::SWAP.to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    Call::new(meteora::amm::PROGRAM_ID, accounts, data).child(transfer())
}

/// Meteora AMM `Swap` event.
fn meteora_amm_event(in_amount: u64, out_amount: u64) -> String {
    let mut data = vec![81, 108, 227, 190, 205, 208, 10, 196];
    for value in [in_amount, out_amount, 0, 0, 0] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    program_data(data)
}

#[test]
fn meteora_amm_verifies_only_the_swap_that_wrote_its_event() {
    let mints = [(0xd1, MINT_IN), (0xd2, MINT_OUT)];
    let project = |calls| project_rows(&with_mints(block_with(calls), &mints));
    let rows = project(vec![
        meteora_amm_swap(0x10, 0x70).after(meteora_amm_event(100, 90)),
        meteora_amm_swap(0x11, 0x74).after(meteora_amm_event(200, 180)),
    ]);
    assert_verified(&rows, &[at(0, None), at(1, None)]);
    let rows = project(vec![
        meteora_amm_swap(0x10, 0x70).after("Program data: AAAA".to_string()),
        meteora_amm_swap(0x11, 0x74).after(meteora_amm_event(200, 180)),
    ]);
    assert_stale(&rows, at(1, None), 0x70, 200);
}

// ---------------------------------------------------------------------------
// Aggregator summaries: route membership and order relative to their legs.
// ---------------------------------------------------------------------------

/// OKX `SwapV3` by payer `base` (source/destination mints `base + 3`/`base + 4`)
/// over `hops` single-dex Whirlpool hops.
fn okx_swap(base: u8, amount_in: u64, hops: usize) -> Call {
    let mut data = okx::v2::instructions::SWAP_V3.to_vec();
    for value in [amount_in, amount_in, amount_in] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&(hops as u32).to_le_bytes());
    for _ in 0..hops {
        // One dex (Whirlpool, tag 2) at weight 100.
        data.extend_from_slice(&1u32.to_le_bytes());
        data.push(2);
        data.extend_from_slice(&1u32.to_le_bytes());
        data.push(100);
    }
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    Call::new(okx::v2::PROGRAM_ID, (0..8).map(|offset| key(base + offset)).collect(), data)
}

fn okx_event(amount_in: u64, amount_out: u64) -> String {
    format!("Program log: SwapEvent {{ dex: Whirlpool, amount_in: {amount_in}, amount_out: {amount_out} }}")
}

/// The two-line route preamble OKX writes before a hop through `pool`.
fn okx_preamble(call: Call, amount_in: u64, pool: u8) -> Call {
    call.before(format!("Program log: Dex::Whirlpool amount_in: {amount_in}, offset: 0"))
        .before(format!("Program log: {}", base58::encode(key(pool))))
}

/// Jupiter v4 swap event, as its route writes after a leg.
fn jupiter_v4_event(input_amount: u64, output_amount: u64) -> String {
    let event = jupiter::v4::events::SwapEvent {
        amm: Pubkey::new_from_array(orca::whirlpool::PROGRAM_ID),
        input_mint: pubkey(0x51),
        input_amount,
        output_mint: pubkey(0x52),
        output_amount,
    };
    let mut data = vec![81, 108, 227, 190, 205, 208, 10, 196];
    data.extend_from_slice(&borsh::to_vec(&event).unwrap());
    program_data(data)
}

/// The summary's invocation is the aggregator invocation, it shares that
/// route with the venue leg it describes, and it is ordered after the leg.
fn assert_summary_follows_its_leg(rows: &[ProjectedSwap], protocol: pb::Protocol) -> &ProjectedSwap {
    let leg = by_protocol(rows, pb::Protocol::OrcaWhirlpool)[0];
    let summary = by_protocol(rows, protocol)[0];
    assert_eq!(summary.kind, SwapKind::AggregatorSummary);
    assert!(matches!(summary.provenance, Provenance::Log { .. }));
    assert_eq!(summary.invocation, Some(at(0, None)));
    assert_eq!(summary.invocation_outcome, InvocationOutcome::Succeeded);
    assert_eq!(summary.aggregator_route, Some(at(0, None)));
    assert_eq!(leg.invocation, Some(at(0, Some(0))));
    assert_eq!(leg.verified_context, Some(at(0, Some(0))));
    assert_eq!(leg.aggregator_route, summary.aggregator_route);
    assert_eq!((leg.execution_ordinal, summary.execution_ordinal), (Some(0), Some(1)));
    summary
}

#[test]
fn jupiter_v4_summary_shares_its_route_with_its_leg() {
    let leg = orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90));
    let route = Call::new(jupiter::v4::PROGRAM_ID, vec![key(0x22)], vec![9, 9, 9])
        .child(leg)
        .after(jupiter_v4_event(100, 90));
    let rows = project_rows(&block_with(vec![route]));
    assert_eq!(rows.len(), 2);
    // Jupiter v4 summaries have no context verifier.
    assert_eq!(assert_summary_follows_its_leg(&rows, pb::Protocol::JupiterV4).verified_context, None);
}

#[test]
fn okx_summary_shares_its_route_with_its_leg_and_verifies_against_its_own_lines() {
    let leg = orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90));
    let route = okx_preamble(okx_swap(0x80, 100, 1), 100, 0x50).child(leg).after(okx_event(100, 90));
    let rows = project_rows(&block_with(vec![route]));
    assert_eq!(rows.len(), 2);
    let summary = assert_summary_follows_its_leg(&rows, pb::Protocol::OkxDex);
    assert_eq!(summary.verified_context, Some(at(0, None)));
    assert_eq!(
        (summary.swap.user.as_slice(), summary.swap.amm_pool.as_slice()),
        (key(0x80).as_slice(), key(0x50).as_slice())
    );

    // A two-hop route completes on its second event.
    let rows = project_rows(&block_with(vec![okx_swap(0x80, 100, 2).after(okx_event(100, 95)).after(okx_event(95, 90))]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].swap.output_amount, 90);
    assert_verified(&rows, &[at(0, None)]);
}

/// OKX `SwapV3` with an explicit route shape: per parallel chain, its amount
/// and the number of dexes in each hop (every dex Whirlpool, equal weights).
fn okx_swap_shaped(base: u8, amount_in: u64, chains: &[(u64, &[usize])]) -> Call {
    let mut data = okx::v2::instructions::SWAP_V3.to_vec();
    for value in [amount_in, amount_in, amount_in] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.extend_from_slice(&(chains.len() as u32).to_le_bytes());
    for (amount, _) in chains {
        data.extend_from_slice(&amount.to_le_bytes());
    }
    data.extend_from_slice(&(chains.len() as u32).to_le_bytes());
    for (_, hops) in chains {
        data.extend_from_slice(&(hops.len() as u32).to_le_bytes());
        for &dexes in hops.iter() {
            data.extend_from_slice(&(dexes as u32).to_le_bytes());
            data.extend(vec![2u8; dexes]);
            data.extend_from_slice(&(dexes as u32).to_le_bytes());
            data.extend(vec![(100 / dexes) as u8; dexes]);
        }
    }
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    Call::new(okx::v2::PROGRAM_ID, (0..8).map(|offset| key(base + offset)).collect(), data)
}

/// A verified OKX summary keeps the adapter's legacy field meanings: its
/// output is the last `SwapEvent`'s `amount_out`, which is the route's net
/// output only for a linear route. Verification proves the row was rebuilt
/// from the instruction's own lines, not that the output is the route's.
#[test]
fn verified_okx_split_and_parallel_routes_keep_the_last_event_output() {
    // One hop split 50/50 across two dexes: 100 in, 45 + 44 out.
    let split = okx_swap_shaped(0x80, 100, &[(100, &[2])]).after(okx_event(50, 45)).after(okx_event(50, 44));
    // Two parallel chains 60/40: 54 + 36 out.
    let parallel = okx_swap_shaped(0x80, 100, &[(60, &[1]), (40, &[1])])
        .after(okx_event(60, 54))
        .after(okx_event(40, 36));
    for (route, last_output) in [(split, 44), (parallel, 36)] {
        let rows = project_rows(&block_with(vec![route]));
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].swap.input_amount, rows[0].swap.output_amount), (100, last_output));
        // Not a single-hop route, so no pool either.
        assert!(rows[0].swap.amm_pool.is_empty());
        assert_verified(&rows, &[at(0, None)]);
    }
}

#[test]
fn nested_aggregators_share_the_outermost_route() {
    // Jupiter v6 -> OKX (not SwapV3, so no OKX summary) -> Orca.
    let leg = orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90));
    let okx = Call::new(okx::v2::PROGRAM_ID, vec![key(0x22)], vec![9, 9, 9]).child(leg);
    let rows = project_rows(&block_with(vec![jupiter_route(okx)]));
    let leg = by_protocol(&rows, pb::Protocol::OrcaWhirlpool)[0];
    let summary = by_protocol(&rows, pb::Protocol::JupiterV6)[0];
    assert_eq!(leg.invocation, Some(at(0, Some(1))));
    assert_eq!(leg.aggregator_route, Some(at(0, None)));
    assert_eq!(summary.aggregator_route, Some(at(0, None)));
}

/// The README's count-once rule for one route, from the published fields
/// only: its route summaries and its venue legs. The route summaries are the
/// Jupiter v4 or OKX summaries whose `invocation` is the route itself, or,
/// when there are none, the route's Jupiter v6 summaries.
fn count_once(rows: &[ProjectedSwap], route: InstructionPosition) -> (Vec<&ProjectedSwap>, Vec<&ProjectedSwap>) {
    let in_route: Vec<_> = rows
        .iter()
        .filter(|row| row.invocation.is_some() && row.aggregator_route == Some(route))
        .collect();
    let summaries = |keep: &dyn Fn(&ProjectedSwap) -> bool| -> Vec<&ProjectedSwap> {
        in_route
            .iter()
            .copied()
            .filter(|row| row.kind == SwapKind::AggregatorSummary && keep(row))
            .collect()
    };
    let mut route_summaries =
        summaries(&|row| matches!(row.swap.protocol.as_known(), Some(pb::Protocol::JupiterV4 | pb::Protocol::OkxDex)) && row.invocation == Some(route));
    if route_summaries.is_empty() {
        route_summaries = summaries(&|row| row.swap.protocol == pb::Protocol::JupiterV6);
    }
    let legs = in_route.into_iter().filter(|row| row.kind == SwapKind::PoolExecution).collect();
    (route_summaries, legs)
}

#[test]
fn nested_aggregator_summaries_count_once_per_route() {
    let leg = || orca_swap(0x10, 0x50, true).after(traded(0x50, true, 100, 90));
    let okx_route = |child: Call| okx_preamble(okx_swap(0x80, 100, 1), 100, 0x50).child(child).after(okx_event(100, 90));
    let protocols = |rows: &[&ProjectedSwap]| rows.iter().map(|row| row.swap.protocol.as_known().unwrap()).collect::<Vec<_>>();
    let amounts = |rows: &[&ProjectedSwap]| rows.iter().map(|row| (row.swap.input_amount, row.swap.output_amount)).collect::<Vec<_>>();

    // Jupiter v6 -> OKX SwapV3 -> Orca: one 100-unit trade, but two proved
    // summaries (one per aggregator) in the one route.
    let rows = project_rows(&block_with(vec![jupiter_route(okx_route(leg()))]));
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.aggregator_route == Some(at(0, None))));
    let nested = by_protocol(&rows, pb::Protocol::OkxDex)[0];
    // The nested summary is even verified: against the nested OKX call, which
    // wrote it, and which is not the route.
    assert_eq!((nested.invocation, nested.verified_context), (Some(at(0, Some(0))), Some(at(0, Some(0)))));
    let (summaries, legs) = count_once(&rows, at(0, None));
    assert_eq!(protocols(&summaries), [pb::Protocol::JupiterV6]);
    assert_eq!(protocols(&legs), [pb::Protocol::OrcaWhirlpool]);
    assert_eq!((amounts(&summaries), amounts(&legs)), (vec![(100, 90)], vec![(100, 90)]));

    // OKX SwapV3 -> Jupiter v6 -> Orca: the OKX summary is the route's own,
    // and the nested Jupiter v6 summary is left out.
    let rows = project_rows(&block_with(vec![okx_route(jupiter_route(leg()))]));
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.aggregator_route == Some(at(0, None))));
    let (summaries, legs) = count_once(&rows, at(0, None));
    assert_eq!(protocols(&summaries), [pb::Protocol::OkxDex]);
    assert_eq!((summaries[0].invocation, summaries[0].verified_context), (Some(at(0, None)), Some(at(0, None))));
    assert_eq!(protocols(&legs), [pb::Protocol::OrcaWhirlpool]);
    assert_eq!((amounts(&summaries), amounts(&legs)), (vec![(100, 90)], vec![(100, 90)]));
}

/// The OKX adapter completes the oldest queued `SwapV3` context on any
/// OKX-shaped `SwapEvent` line. Mispaired summaries keep the writer of their
/// line as `invocation` and route, and are never verified.
#[test]
fn okx_summaries_paired_with_another_instruction_are_never_verified() {
    let forged = |amount_in, amount_out| Call::new(OTHER_PROGRAM, vec![key(0x2f)], vec![9]).after(okx_event(amount_in, amount_out));
    let row = |rows: &[ProjectedSwap], user: u8| rows.iter().find(|row| row.swap.user == key(user)).unwrap().clone();

    // A top-level forger completes OKX#1's context, so OKX#1's own event
    // completes OKX#2's: the row has OKX#2's accounts and OKX#1's output.
    let rows = project_rows(&block_with(vec![
        forged(1, 2),
        okx_swap(0x80, 100, 1).after(okx_event(100, 200)),
        okx_swap(0x90, 300, 1).after(okx_event(300, 400)),
    ]));
    let (forgery, leaked) = (row(&rows, 0x80), row(&rows, 0x90));
    assert_eq!((forgery.swap.output_amount, forgery.invocation, forgery.verified_context), (2, None, None));
    assert_eq!((leaked.swap.input_amount, leaked.swap.output_amount), (300, 200));
    assert_eq!((leaked.invocation, leaked.aggregator_route), (Some(at(1, None)), Some(at(1, None))));
    assert_eq!(leaked.invocation_outcome, InvocationOutcome::Succeeded);
    assert_eq!(leaked.verified_context, None);
    assert!(rows.iter().all(|row| row.aggregator_route != Some(at(2, None))));

    // The forged line comes from a child of OKX#0 (a venue or transfer hook).
    let rows = project_rows(&block_with(vec![
        okx_swap(0x80, 100, 1).child(forged(1, 2)).after(okx_event(100, 200)),
        okx_swap(0x90, 300, 1).child(transfer()).after(okx_event(300, 400)),
    ]));
    let leaked = row(&rows, 0x90);
    assert_eq!((leaked.invocation, leaked.verified_context), (Some(at(0, None)), None));

    // Within one two-hop route, a child's forged event counts as hop one, so
    // OKX's own first event completes the summary with an interim output.
    let rows = project_rows(&block_with(vec![okx_swap(0x80, 100, 2)
        .child(forged(1, 2))
        .after(okx_event(100, 95))
        .after(okx_event(95, 90))]));
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].swap.output_amount, rows[0].invocation, rows[0].verified_context),
        (95, Some(at(0, None)), None)
    );

    // No forgery: OKX#0 writes one more event than its route has hops.
    let rows = project_rows(&block_with(vec![
        okx_swap(0x80, 100, 1).after(okx_event(100, 150)).after(okx_event(150, 200)),
        okx_swap(0x90, 300, 1).after(okx_event(300, 400)),
    ]));
    assert_verified(&[row(&rows, 0x80)], &[at(0, None)]);
    let leaked = row(&rows, 0x90);
    assert_eq!(
        (leaked.swap.output_amount, leaked.invocation, leaked.verified_context),
        (200, Some(at(0, None)), None)
    );

    // An OKX instruction that is not SwapV3 has no context but writes an event.
    let other = Call::new(
        okx::v2::PROGRAM_ID,
        (0..8).map(|offset| key(0xa0 + offset)).collect(),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 0],
    )
    .after(okx_event(50, 60));
    let rows = project_rows(&block_with(vec![other, okx_swap(0x90, 300, 1).after(okx_event(300, 400))]));
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].swap.output_amount, rows[0].invocation, rows[0].verified_context),
        (60, Some(at(0, None)), None)
    );
    assert_eq!(rows[0].aggregator_route, Some(at(0, None)));
}

// ---------------------------------------------------------------------------
// One-field stale pairings: a verifier must compare every field it rebuilds,
// so each stale row below differs from its writer's own swap in one field.
// ---------------------------------------------------------------------------

fn nested(call: Call) -> Call {
    Call::new(OTHER_PROGRAM, vec![key(0x2f)], vec![1]).child(call)
}

/// For a sequence-paired venue: the first call writes `junk` (no or an
/// undecodable event), so the second call's event (200 in) pairs with the
/// first call. The writer then differs from the row in its pool only, its
/// user only, or its stack height only.
fn assert_each_instruction_field_is_compared(
    project: impl Fn(Vec<Call>) -> Vec<ProjectedSwap>,
    swap: impl Fn(u8, u8) -> Call,
    junk: impl Fn(Call) -> Call,
    event: impl Fn(Call, u8) -> Call,
) {
    let rows = project(vec![junk(swap(0x10, 0x70)), event(swap(0x11, 0x70), 0x70)]);
    assert_stale(&rows, at(1, None), 0x70, 200);
    let rows = project(vec![junk(swap(0x10, 0x70)), event(swap(0x10, 0x74), 0x74)]);
    assert_stale(&rows, at(1, None), 0x70, 200);
    let rows = project(vec![junk(swap(0x10, 0x70)), nested(event(swap(0x10, 0x70), 0x70))]);
    assert_stale(&rows, at(1, Some(0)), 0x70, 200);
}

#[test]
fn log_verifiers_compare_pool_user_and_stack_height_separately() {
    let undecodable = |call: Call| call.after("Program data: AAAA".to_string());
    assert_each_instruction_field_is_compared(
        |calls| project_rows(&with_mints(block_with(calls), &VAULT_MINTS)),
        |user, pool| amm_v4_swap(pool, user),
        |call| call,
        |call, _| call.before(ray_log(200, 180)),
    );
    assert_each_instruction_field_is_compared(
        |calls| project_rows(&with_mints(block_with(calls), &VAULT_MINTS)),
        clmm_swap,
        undecodable,
        |call, pool| call.after(clmm_event(pool, pool + 1, pool + 2, 200, 180)),
    );
    assert_each_instruction_field_is_compared(
        |calls| project_rows(&block_with(calls)),
        cpmm_swap,
        undecodable,
        |call, pool| call.after(cpmm_event(pool, 200, 180)),
    );
    assert_each_instruction_field_is_compared(
        |calls| project_rows(&with_mints(block_with(calls), &[(0xd1, MINT_IN), (0xd2, MINT_OUT)])),
        meteora_amm_swap,
        undecodable,
        |call, _| call.after(meteora_amm_event(200, 180)),
    );
    // Orca pairs by pool and direction, so only the user (pinned above) or the
    // stack height can differ.
    let rows = project_rows(&block_with(vec![
        orca_swap(0x10, 0x50, true).after("Program data: AAAAAAAAAAA=".to_string()),
        nested(orca_swap(0x10, 0x50, true).after(traded(0x50, true, 200, 180))),
    ]));
    assert_stale(&rows, at(1, Some(0)), 0x50, 200);
}

#[test]
fn okx_verifier_compares_every_field_at_the_completing_line() {
    let forged = |amount_in, amount_out| Call::new(OTHER_PROGRAM, vec![key(0x2f)], vec![9]).after(okx_event(amount_in, amount_out));
    // Same payer and mints: the leaked summary differs from its writer's own
    // summary at that line in the input amount only.
    let rows = project_rows(&block_with(vec![
        forged(1, 2),
        okx_swap(0x80, 100, 1).after(okx_event(100, 200)),
        okx_swap(0x80, 300, 1).after(okx_event(300, 400)),
    ]));
    let leaked = rows.iter().find(|row| row.swap.input_amount == 300).unwrap();
    assert_eq!(
        (leaked.swap.output_amount, leaked.invocation, leaked.verified_context),
        (200, Some(at(1, None)), None)
    );

    // The leaked summary equals OKX#1's own summary, which completes at
    // another line: a summary verifies only at the line that completes it.
    let rows = project_rows(&block_with(vec![
        okx_swap(0x80, 100, 1).after(okx_event(100, 150)).after(okx_event(150, 200)),
        okx_swap(0x90, 300, 1).after(okx_event(300, 200)),
    ]));
    let leaked = rows.iter().find(|row| row.swap.user == key(0x90)).unwrap();
    assert_eq!(
        (leaked.swap.output_amount, leaked.invocation, leaked.verified_context),
        (200, Some(at(0, None)), None)
    );
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "distinct"))]
fn colliding_order_keys_withhold_every_ordinal() {
    assert_eq!(crate::native::execution_ordinals(&[(4, 0), (1, 0), (2, 1)]), Some(vec![2, 0, 1]));
    // Unreachable from the current adapters; release builds withhold.
    assert_eq!(crate::native::execution_ordinals(&[(4, 0), (1, 0), (4, 0)]), None);
}

/// Invariants and per-protocol attribution counts on the pinned mainnet blocks.
#[test]
fn real_blocks_attribute_log_rows_without_changing_legacy_output() {
    use sha2::Digest;
    use std::collections::BTreeMap;
    let manifest = env!("CARGO_MANIFEST_DIR");
    // Encoded `map_events` output SHA-256 captured from unchanged main
    // (3760f70), which predates log-row attribution.
    let fixtures = [
        (
            format!("{manifest}/fixtures/solana-mainnet-350000026.pb.gz"),
            "c1e611c718878724d851e446c0a13bec1b28e682ebf99e2ab7b4a2d221b78218",
        ),
        (
            format!("{manifest}/../spl/transfers/tests/fixtures/solana-mainnet-349019252.pb.gz"),
            "812b832df23541624027634e8fc815be656087a98501b0aebd7873805f00e5b2",
        ),
        (
            format!("{manifest}/../spl/transfers/tests/fixtures/solana-mainnet-350000009.pb.gz"),
            "2787054499c4c0b78825a46dc8750d503705c19129cb465b6a00ae2fa8f09e26",
        ),
        (
            format!("{manifest}/../spl/transfers/tests/fixtures/solana-mainnet-350000010.pb.gz"),
            "e1a0255c46f4cf9bdfee21e24afac8e392582ecf02eaa166b31c169342da40a0",
        ),
    ];
    // protocol -> [rows, rows with an invocation, log rows, log rows with an
    // invocation, verified rows, ordered rows, routed rows]
    let mut counts: BTreeMap<String, [usize; 7]> = BTreeMap::new();
    let mut unordered_transactions = 0;
    for (path, legacy_sha256) in &fixtures {
        let mut bytes = Vec::new();
        GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_end(&mut bytes).unwrap();
        let block = <source::Block as buffa::Message>::decode_from_slice(bytes.as_slice()).unwrap();
        let projection = project(&block).unwrap();
        let legacy = map_events(block.clone()).unwrap();
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(buffa::Message::encode_to_vec(&legacy))),
            *legacy_sha256,
            "{path}"
        );
        assert_eq!(projection.clone().into_legacy(), legacy);
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
                entry[1] += usize::from(row.invocation.is_some());
                entry[5] += usize::from(row.execution_ordinal.is_some());
                entry[6] += usize::from(row.aggregator_route.is_some());
                if row.verified_context.is_some() {
                    entry[4] += 1;
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
                        entry[2] += 1;
                        if let Some(invocation) = row.invocation {
                            entry[3] += 1;
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
        .map(|(protocol, [rows, attributed, logs, log_attributed, verified, ordered, routed])| {
            format!("{protocol}: rows={rows} attributed={attributed} log={logs} log_attributed={log_attributed} verified={verified} ordered={ordered} routed={routed}")
        })
        .collect();
    eprintln!("{}\nunordered transactions={unordered_transactions}", summary.join("\n"));
    // The golden's `#` lines document its columns.
    let golden: String = include_str!("../fixtures/native-execution-coverage.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(format!("{}\nunordered transactions={unordered_transactions}\n", summary.join("\n")), golden);
}
