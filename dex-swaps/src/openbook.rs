//! OpenBook v1.7 completed taker fills. Order limits remain lots; only the
//! program's scoped, successful runtime events establish ledger token atoms.
use common::solana::parse_program_data;
use substreams_solana::{b58, base58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::token;

pub const PROGRAM_ID: [u8; 32] = b58!("opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb");
pub(crate) const PLACE: [u8; 8] = [51, 194, 155, 175, 109, 130, 96, 106];
pub(crate) const EDIT: [u8; 8] = [254, 208, 118, 29, 173, 248, 200, 70];
pub(crate) const PEG: [u8; 8] = [141, 185, 251, 63, 74, 85, 210, 145];
pub(crate) const EDIT_PEG: [u8; 8] = [62, 187, 125, 69, 26, 221, 157, 133];
pub(crate) const BATCH: [u8; 8] = [60, 63, 50, 123, 12, 197, 60, 190];
pub(crate) const CANCEL_BATCH: [u8; 8] = [128, 155, 222, 60, 186, 40, 225, 50];
pub(crate) const CREATE: [u8; 8] = [103, 226, 97, 235, 200, 188, 251, 254];
pub(crate) const CLOSE: [u8; 8] = [88, 154, 248, 186, 48, 14, 123, 244];
const TOTAL: [u8; 8] = [8, 235, 48, 58, 174, 76, 156, 105];

#[derive(Clone, Debug)]
pub struct Market {
    pub address: [u8; 32],
    pub base_mint: [u8; 32],
    pub quote_mint: [u8; 32],
    pub(crate) authority: [u8; 32],
    pub(crate) bids: [u8; 32],
    pub(crate) asks: [u8; 32],
    pub(crate) heap: [u8; 32],
    pub(crate) base_vault: [u8; 32],
    pub(crate) quote_vault: [u8; 32],
    base_lot: u64,
    quote_lot: u64,
    taker_fee: u64,
}
fn key(data: &[u8], offset: usize) -> Option<[u8; 32]> {
    data.get(offset..offset + 32)?.try_into().ok()
}
fn signed(data: &[u8], offset: usize) -> Option<i64> {
    Some(i64::from_le_bytes(data.get(offset..offset + 8)?.try_into().ok()?))
}
impl Market {
    /// The caller must establish the owner at the relevant market incarnation.
    pub fn from_owned_header(address: &[u8], owner: &[u8], data: &[u8]) -> Option<Self> {
        if owner != PROGRAM_ID || data.len() != 848 || data[..8] != [219, 190, 213, 55, 0, 227, 198, 154] {
            return None;
        }
        let base_lot = signed(data, 456)?;
        let quote_lot = signed(data, 448)?;
        let maker = signed(data, 480)?;
        let taker = signed(data, 488)?;
        if base_lot <= 0 || quote_lot <= 0 || !(0..=1_000_000).contains(&taker) || maker.unsigned_abs() > 1_000_000 || maker < -taker {
            return None;
        }
        let market = Self {
            address: address.try_into().ok()?,
            base_mint: key(data, 576)?,
            quote_mint: key(data, 608)?,
            authority: key(data, 16)?,
            bids: key(data, 200)?,
            asks: key(data, 232)?,
            heap: key(data, 264)?,
            base_vault: key(data, 640)?,
            quote_vault: key(data, 680)?,
            base_lot: base_lot as u64,
            quote_lot: quote_lot as u64,
            taker_fee: taker as u64,
        };
        if market.base_mint == market.quote_mint
            || market.base_vault == market.quote_vault
            || [
                market.address,
                market.base_mint,
                market.quote_mint,
                market.authority,
                market.bids,
                market.asks,
                market.heap,
                market.base_vault,
                market.quote_vault,
            ]
            .iter()
            .any(|k| k.iter().all(|b| *b == 0))
        {
            return None;
        }
        Some(market)
    }
}
struct Bytes<'a>(&'a [u8]);
impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let value = self.0.get(..n)?;
        self.0 = &self.0[n..];
        Some(value)
    }
    fn byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn count(&mut self) -> Option<usize> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize)
    }
}
#[derive(Clone, Debug)]
struct Order {
    side: u8,
    base_limit: u64,
    quote_limit: u64,
    price: Option<u64>,
    self_trade: u8,
    takes: bool,
}
impl Order {
    fn accepts(&self, market: &Market, side: u8, paid: u64, received: u64, fees: u64) -> bool {
        if self.side != side || !self.takes || paid == 0 || received == 0 {
            return false;
        }
        let base = if side == 0 { received } else { paid };
        let Some(quote) = (if side == 0 { paid.checked_sub(fees) } else { received.checked_add(fees) }) else {
            return false;
        };
        if quote == 0 || base % market.base_lot != 0 || quote % market.quote_lot != 0 {
            return false;
        }
        let base_lots = base / market.base_lot;
        let quote_lots = quote / market.quote_lot;
        if base_lots > self.base_limit || u128::from(if side == 0 { paid } else { quote }) > u128::from(self.quote_limit) * u128::from(market.quote_lot) {
            return false;
        }
        if let Some(price) = self.price {
            let limit = u128::from(base_lots) * u128::from(price);
            if (side == 0 && u128::from(quote_lots) > limit) || (side == 1 && u128::from(quote_lots) < limit) {
                return false;
            }
        }
        let fee_ceiling = (u128::from(quote) * u128::from(market.taker_fee) + 999_999) / 1_000_000;
        // DecrementTake is a normal book match without fees on its self-match
        // component. CancelProvide/Abort never include that component.
        if self.self_trade == 0 {
            u128::from(fees) <= fee_ceiling
        } else {
            u128::from(fees) == fee_ceiling
        }
    }
}
fn orders(data: &[u8]) -> Option<(Vec<Order>, bool)> {
    let tag: [u8; 8] = data.get(..8)?.try_into().ok()?;
    let mut bytes = Bytes(&data[8..]);
    let batch = tag == BATCH || tag == CANCEL_BATCH;
    if batch {
        let kind = bytes.byte()?;
        if kind > 4 {
            return None;
        }
        let mut orders = Vec::new();
        for side in [0, 1] {
            let count = bytes.count()?;
            if count > bytes.0.len() / 24 {
                return None;
            }
            for _ in 0..count {
                let price = bytes.i64()?;
                let quote = bytes.i64()?;
                let _expiry = bytes.u64()?;
                if price <= 0 || quote < 0 {
                    return None;
                }
                orders.push(Order {
                    side,
                    base_limit: i64::MAX as u64,
                    quote_limit: quote as u64,
                    price: if kind == 3 { None } else { Some(price as u64) },
                    self_trade: 1,
                    takes: matches!(kind, 0 | 1 | 3),
                });
            }
        }
        let limit = bytes.byte()?;
        for order in &mut orders {
            order.takes &= limit != 0;
        }
        return bytes.0.is_empty().then_some((orders, true));
    }
    if ![PLACE, EDIT, PEG, EDIT_PEG].contains(&tag) {
        return None;
    }
    let peg = tag == PEG || tag == EDIT_PEG;
    if tag == EDIT || tag == EDIT_PEG {
        bytes.u64()?; // cancelled client order id
        if bytes.i64()? < 0 {
            return None;
        }
    }
    let side = bytes.byte()?;
    let price = bytes.i64()?;
    if peg {
        if bytes.i64()? <= 0 {
            return None;
        }
    } else if price <= 0 {
        return None;
    }
    let base = bytes.i64()?;
    let quote = bytes.i64()?;
    bytes.u64()?; // client id (not present in TotalOrderFillEvent)
    let kind = bytes.byte()?;
    bytes.u64()?; // expiry
    let self_trade = bytes.byte()?;
    let limit = bytes.byte()?;
    if side > 1 || base < 0 || quote < 0 || kind > 4 || self_trade > 2 || !bytes.0.is_empty() || (peg && matches!(kind, 1 | 3)) {
        return None;
    }
    Some((
        vec![Order {
            side,
            base_limit: base as u64,
            quote_limit: quote as u64,
            price: if peg || kind == 3 { None } else { Some(price as u64) },
            self_trade,
            takes: limit != 0 && matches!(kind, 0 | 1 | 3),
        }],
        false,
    ))
}
pub(crate) fn market_index(ix: &InstructionView) -> Option<usize> {
    if ix.program_id().0.as_slice() != PROGRAM_ID {
        return None;
    }
    let (orders, batch) = orders(&ix.data())?;
    if !orders.iter().any(|order| order.takes) {
        return None;
    }
    Some(if batch { 5 } else { 4 })
}

#[derive(Clone, Debug)]
pub struct Fill {
    pub instruction_index: u32,
    pub log_index: u32,
    pub market: [u8; 32],
    pub trader: [u8; 32],
    pub input_mint: [u8; 32],
    pub output_mint: [u8; 32],
    pub input_amount: u64,
    pub output_amount: u64,
}
/// Financial events require both a completed native invocation and the success
/// of every ancestor. Data emitted by nested programs never belongs to this call.
pub fn decode_fills(tx: &ConfirmedTransaction, index: usize, market: &Market) -> Option<Vec<Fill>> {
    if !tx.is_successful() || !crate::logs::successful_invocations(tx).contains(&(index as u32)) {
        return None;
    }
    let instructions: Vec<_> = tx.walk_instructions().collect();
    let ix = instructions.get(index)?;
    if ix.program_id().0.as_slice() != PROGRAM_ID {
        return None;
    }
    let (orders, batch) = orders(&ix.data())?;
    let accounts = ix.accounts();
    let pool = if batch { 5 } else { 4 };
    if accounts.len() < if batch { 14 } else { 12 }
        || accounts[pool].0.as_slice() != market.address
        || accounts[pool + 1].0.as_slice() != market.bids
        || accounts[pool + 2].0.as_slice() != market.asks
        || accounts[pool + 3].0.as_slice() != market.heap
        || accounts[if batch { 13 } else { 11 }].0.as_slice() != token::PROGRAM_ID
    {
        return None;
    }
    if batch {
        if accounts[9].0.as_slice() != market.quote_vault || accounts[10].0.as_slice() != market.base_vault {
            return None;
        }
    } else {
        let side = orders.first()?.side;
        if accounts[8].0.as_slice() != if side == 0 { market.quote_vault } else { market.base_vault } {
            return None;
        }
    }
    let target = base58::encode(PROGRAM_ID);
    let mut cursor = 0;
    let mut stack = Vec::<(String, Option<usize>)>::new();
    let mut summaries = Vec::new();
    let mut closed = false;
    for (log_index, log) in tx.meta.as_ref()?.log_messages.iter().enumerate() {
        let words: Vec<_> = log.split_whitespace().collect();
        if words.first() == Some(&"Program") && words.get(2) == Some(&"invoke") && words.len() == 4 {
            let depth: u32 = words[3].strip_prefix('[')?.strip_suffix(']')?.parse().ok()?;
            let program = base58::decode(words[1]).ok()?;
            let matched = instructions
                .iter()
                .enumerate()
                .skip(cursor)
                .find(|(_, ix)| ix.program_id().0 == &program && (if ix.is_root() { 1 } else { ix.stack_height() }) == depth)
                .map(|(i, _)| i);
            if depth as usize != stack.len() + 1 {
                return None;
            }
            if let Some(i) = matched {
                cursor = i + 1;
            } else {
                return None;
            }
            stack.push((words[1].to_owned(), matched));
        } else if words.first() == Some(&"Program") && matches!(words.get(2), Some(&"success") | Some(&"failed:")) {
            let frame = stack.pop()?;
            if frame.0 != words[1] {
                return None;
            }
            if frame.1 == Some(index) {
                closed = true;
                break;
            }
        } else if stack.last().is_some_and(|frame| frame.0 == target && frame.1 == Some(index)) {
            if let Some(data) = parse_program_data(log) {
                if data.is_empty() {
                    return None;
                }
                if data.starts_with(&TOTAL) {
                    if data.len() != 65 {
                        return None;
                    }
                    summaries.push((log_index, data));
                }
            }
        }
    }
    if !closed {
        return None;
    }
    let mut next = 0;
    let mut result = Vec::new();
    for (log_index, data) in summaries {
        let side = data[8];
        let taker = key(&data, 9)?;
        if taker.as_slice() != accounts[1].0 {
            return None;
        }
        let paid = u64::from_le_bytes(data[41..49].try_into().ok()?);
        let received = u64::from_le_bytes(data[49..57].try_into().ok()?);
        let fees = u64::from_le_bytes(data[57..65].try_into().ok()?);
        let accepted = orders
            .iter()
            .enumerate()
            .skip(next)
            .find(|(_, order)| order.accepts(market, side, paid, received, fees))?
            .0;
        next = accepted + 1;
        result.push(Fill {
            instruction_index: index as u32,
            log_index: log_index.try_into().ok()?,
            market: market.address,
            trader: accounts[0].0.as_slice().try_into().ok()?,
            input_mint: if side == 0 { market.quote_mint } else { market.base_mint },
            output_mint: if side == 0 { market.base_mint } else { market.quote_mint },
            input_amount: paid,
            output_amount: received,
        });
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    fn market() -> Market {
        let mut header = *include_bytes!("../fixtures/openbook/sol-usdc-market-455779697.bin");
        header[448..456].copy_from_slice(&10i64.to_le_bytes());
        header[456..464].copy_from_slice(&100i64.to_le_bytes());
        header[480..488].copy_from_slice(&0i64.to_le_bytes());
        header[488..496].copy_from_slice(&1000i64.to_le_bytes());
        Market::from_owned_header(&[21; 32], &PROGRAM_ID, &header).unwrap()
    }
    fn packet(tag: [u8; 8], side: u8) -> Vec<u8> {
        let mut data = tag.to_vec();
        if tag == EDIT || tag == EDIT_PEG {
            data.extend(7u64.to_le_bytes());
            data.extend(5i64.to_le_bytes());
        }
        data.push(side);
        data.extend(30i64.to_le_bytes());
        if tag == PEG || tag == EDIT_PEG {
            data.extend(40i64.to_le_bytes());
        }
        data.extend(100i64.to_le_bytes());
        data.extend(10000i64.to_le_bytes());
        data.extend(9u64.to_le_bytes());
        data.push(if tag == PEG || tag == EDIT_PEG { 0 } else { 1 });
        data.extend(0u64.to_le_bytes());
        data.extend([0, 10]);
        data
    }
    fn transaction(m: &Market, data: Vec<u8>, batch: bool, fills: &[(u8, u64, u64, u64)]) -> ConfirmedTransaction {
        let mut a = vec![
            [31; 32],
            [32; 32],
            [33; 32],
            [34; 32],
            m.address,
            m.bids,
            m.asks,
            m.heap,
            m.quote_vault,
            [35; 32],
            [36; 32],
            token::PROGRAM_ID,
        ];
        if !batch && data[if data[..8] == EDIT || data[..8] == EDIT_PEG { 24 } else { 8 }] == 1 {
            a[8] = m.base_vault;
        }
        if batch {
            a = vec![
                [31; 32],
                [32; 32],
                [33; 32],
                [34; 32],
                [35; 32],
                m.address,
                m.bids,
                m.asks,
                m.heap,
                m.quote_vault,
                m.base_vault,
                [36; 32],
                [37; 32],
                token::PROGRAM_ID,
            ];
        }
        let mut tx = crate::routed_pool::test_fixture::make_tx(PROGRAM_ID, &a, data);
        let logs = &mut tx.meta.as_mut().unwrap().log_messages;
        logs.push(format!("Program {} invoke [1]", base58::encode(PROGRAM_ID)));
        for (side, paid, received, fees) in fills {
            let mut event = TOTAL.to_vec();
            event.push(*side);
            event.extend(a[1]);
            event.extend(paid.to_le_bytes());
            event.extend(received.to_le_bytes());
            event.extend(fees.to_le_bytes());
            logs.push(format!("Program data: {}", STANDARD.encode(event)));
        }
        logs.push(format!("Program {} success", base58::encode(PROGRAM_ID)));
        tx
    }
    #[test]
    fn deposited_balances_edits_and_pegged_orders_use_actual_atom_totals() {
        let m = market();
        for tag in [PLACE, EDIT, PEG, EDIT_PEG] {
            for side in [0, 1] {
                let (paid, received) = if side == 0 { (601, 200) } else { (200, 599) };
                let tx = transaction(&m, packet(tag, side), false, &[(side, paid, received, 1)]);
                let fills = decode_fills(&tx, 0, &m).unwrap();
                assert_eq!(fills.len(), 1);
                assert_eq!((fills[0].input_amount, fills[0].output_amount), (paid, received));
                assert_eq!(fills[0].input_mint, if side == 0 { m.quote_mint } else { m.base_mint });
                assert!(tx.meta.as_ref().unwrap().inner_instructions.is_empty());
                let missing = crate::process_transaction(tx.clone()).unwrap();
                assert!(missing.swaps.is_empty());
                assert_eq!(missing.coverage_candidates.len(), 1);
                assert!(!missing.coverage_transaction.is_empty());
                let normalized = crate::process_transaction_with_markets(
                    tx,
                    None,
                    |_, _| None,
                    |address, position| {
                        assert_eq!(address, m.address);
                        assert_eq!(position, 0);
                        Some(m.clone())
                    },
                )
                .unwrap();
                assert_eq!(normalized.swaps.len(), 1);
                let swap = &normalized.swaps[0];
                assert_eq!(swap.protocol, 51);
                assert!(swap.orderbook_fill_verified && !swap.transfer_verified);
                assert_eq!(swap.source_log_index, Some(1));
                assert!(swap.source_transfer_index.is_none() && swap.source_event_index.is_none());
                assert!(normalized.coverage_candidates.is_empty());
            }
        }
    }
    #[test]
    fn batches_preserve_each_fill_and_skip_unfilled_orders_in_order() {
        let m = market();
        for tag in [BATCH, CANCEL_BATCH] {
            let mut data = tag.to_vec();
            data.push(1);
            for count in [3u32, 1] {
                data.extend(count.to_le_bytes());
                for _ in 0..count {
                    data.extend(30i64.to_le_bytes());
                    data.extend(10000i64.to_le_bytes());
                    data.extend(0u64.to_le_bytes());
                }
            }
            data.push(10);
            let tx = transaction(&m, data, true, &[(0, 601, 200, 1), (0, 301, 100, 1), (1, 100, 299, 1)]);
            let fills = decode_fills(&tx, 0, &m).unwrap();
            assert_eq!(fills.len(), 3);
            assert_eq!(fills.iter().map(|x| x.log_index).collect::<Vec<_>>(), vec![1, 2, 3]);
            let mut malformed = tx.clone();
            malformed.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[9..13].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(decode_fills(&malformed, 0, &m).is_none());
        }
    }

    #[test]
    fn finalized_unfilled_batches_and_market_closures_do_not_publish_deposit_prices() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/openbook/ledger");
        for line in include_str!("../fixtures/openbook/ledger/unfilled.tsv").lines() {
            let fields: Vec<_> = line.split('\t').collect();
            let raw = std::fs::read(directory.join(format!("{}.transaction.pb", fields[0]))).unwrap();
            let tx: ConfirmedTransaction = substreams::proto::decode(&raw).unwrap();
            let snapshot = include_bytes!("../fixtures/openbook/market-registry-455779697.bin");
            let normalized = crate::process_transaction_with_markets(
                tx.clone(),
                None,
                |_, _| None,
                |address, _| {
                    let row = snapshot.chunks_exact(880).find(|row| &row[..32] == address)?;
                    Market::from_owned_header(address, &PROGRAM_ID, &row[32..])
                },
            );
            assert!(normalized.map_or(true, |value| value.swaps.is_empty()), "{}", fields[0]);
            if fields[2] == "close_market" {
                let rows = crate::openbook_metadata::updates(&substreams_solana::pb::sf::solana::r#type::v1::Block {
                    slot: fields[1].parse().unwrap(),
                    parent_slot: fields[1].parse::<u64>().unwrap() - 1,
                    transactions: vec![tx],
                    ..Default::default()
                });
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].1, format!("m:{}", fields[4]));
                assert!(rows[0].2.is_empty());
            }
        }
    }

    #[test]
    fn runtime_events_require_the_native_scope_and_successful_ancestors() {
        use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions};
        let m = market();
        let mut tx = transaction(&m, packet(PLACE, 0), false, &[(0, 601, 200, 1)]);
        let message = tx.transaction.as_mut().unwrap().message.as_mut().unwrap();
        let native = message.instructions[0].clone();
        let wrapper_index = message.account_keys.len() as u32;
        message.account_keys.push(vec![90; 32]);
        message.instructions[0].program_id_index = wrapper_index;
        message.instructions[0].data.clear();
        let wrapper = base58::encode([90; 32]);
        let program = base58::encode(PROGRAM_ID);
        let meta = tx.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![InnerInstruction {
                program_id_index: native.program_id_index,
                accounts: native.accounts,
                data: native.data,
                stack_height: Some(2),
            }],
        }];
        meta.log_messages[0] = format!("Program {program} invoke [2]");
        meta.log_messages.insert(0, format!("Program {wrapper} invoke [1]"));
        meta.log_messages.push(format!("Program {wrapper} success"));
        assert_eq!(decode_fills(&tx, 1, &m).unwrap()[0].log_index, 2);
        // A completed descendant is rolled back when its ancestor fails.
        let mut failed = tx.clone();
        *failed.meta.as_mut().unwrap().log_messages.last_mut().unwrap() = format!("Program {wrapper} failed: error");
        assert!(decode_fills(&failed, 1, &m).is_none());
        let mut truncated = tx.clone();
        truncated.meta.as_mut().unwrap().log_messages.pop();
        assert!(decode_fills(&truncated, 1, &m).is_none());
        // Truncation of a later independent call cannot invalidate this fill.
        let mut later = tx.clone();
        let message = later.transaction.as_mut().unwrap().message.as_mut().unwrap();
        message.instructions.push(message.instructions[0].clone());
        later.meta.as_mut().unwrap().log_messages.push(format!("Program {wrapper} invoke [1]"));
        assert_eq!(decode_fills(&later, 1, &m).unwrap().len(), 1);
        // A child can print identical event bytes, but it is not OpenBook.
        let mut foreign = tx;
        let message = foreign.transaction.as_mut().unwrap().message.as_mut().unwrap();
        let foreign_index = message.account_keys.len() as u32;
        message.account_keys.push(vec![91; 32]);
        let meta = foreign.meta.as_mut().unwrap();
        meta.inner_instructions[0].instructions.push(InnerInstruction {
            program_id_index: foreign_index,
            stack_height: Some(3),
            ..Default::default()
        });
        let child = base58::encode([91; 32]);
        meta.log_messages.insert(2, format!("Program {child} invoke [3]"));
        meta.log_messages.insert(4, format!("Program {child} success"));
        assert!(decode_fills(&foreign, 1, &m).unwrap().is_empty());
    }

    #[test]
    fn malformed_failed_and_foreign_events_never_become_ledger_prices() {
        let m = market();
        let tx = transaction(&m, packet(PLACE, 0), false, &[(0, 601, 200, 1)]);
        for variant in 0..8 {
            let mut wrong = tx.clone();
            match variant {
                0 => {
                    wrong.meta.as_mut().unwrap().log_messages.pop();
                }
                1 => {
                    *wrong.meta.as_mut().unwrap().log_messages.last_mut().unwrap() =
                        format!("Program {} failed: custom program error", base58::encode(PROGRAM_ID));
                }
                2 => {
                    wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].accounts[4] = 2;
                }
                3 => {
                    wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data.push(0);
                }
                4 => {
                    wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[41] = 2;
                }
                5 => {
                    wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[51] = 0;
                }
                6 => {
                    let logs = &mut wrong.meta.as_mut().unwrap().log_messages;
                    logs[1].push_str("AA==");
                }
                _ => {
                    wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[17..25].copy_from_slice(&1i64.to_le_bytes());
                }
            }
            assert!(decode_fills(&wrong, 0, &m).is_none(), "variant {variant}");
        }
        assert!(decode_fills(&transaction(&m, packet(PLACE, 0), false, &[(0, 602, 200, 1)]), 0, &m).is_none());
        // Self-match decrements remain normal matches, with no taker fee on
        // that component. CancelProvide requires the market's full fee.
        assert!(decode_fills(&transaction(&m, packet(PLACE, 0), false, &[(0, 600, 200, 0)]), 0, &m).is_some());
        let mut cancel = packet(PLACE, 0);
        cancel[50] = 1;
        assert!(decode_fills(&transaction(&m, cancel, false, &[(0, 600, 200, 0)]), 0, &m).is_none());
    }
}
