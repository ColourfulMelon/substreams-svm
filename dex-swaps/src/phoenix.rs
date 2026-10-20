//! Phoenix settlement evidence, pinned to Ellipsis-Labs/phoenix-v1 at
//! 6ae54af5f07f4d0ae8430b08cbe81d10432cd34c. A fill is an exchange-ledger
//! settlement, not necessarily an SPL transfer. Deposits and order sizes are
//! never substituted for the matched amounts in the signed audit log.
use substreams_solana::{pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;

pub const PROGRAM_ID: [u8; 32] = b58!("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY");
const LOG_AUTHORITY: [u8; 32] = b58!("7aDTsspkQNGKmrexAN7FLx9oxU3iPczSSvHNggyuqYkR");
const MARKET_DISCRIMINANT: u64 = 8167313896524341111;

/// Immutable conversion parameters from a Phoenix-owned market account.
/// The caller must supply the actual owner and account bytes from a trusted
/// finalized snapshot, or parameters reconstructed from a successful market
/// initialization. An RPC snapshot must not be assumed valid before its slot
/// if the market could have been closed and reinitialized in between.
#[derive(Clone, Debug)]
pub struct Market {
    address: [u8; 32],
    base_mint: [u8; 32],
    quote_mint: [u8; 32],
    base_vault: [u8; 32],
    quote_vault: [u8; 32],
    base_lot_atoms: u64,
    quote_lot_atoms: u64,
    base_lots_per_unit: u64,
    tick_quote_lots_per_unit: u64,
}

fn u64_at(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(data.get(offset..offset.checked_add(8)?)?.try_into().ok()?))
}

fn key_at(data: &[u8], offset: usize) -> Option<[u8; 32]> {
    data.get(offset..offset.checked_add(32)?)?.try_into().ok()
}

impl Market {
    pub(crate) fn from_definition(address: &[u8], data: &[u8], mut decimals: impl FnMut(&[u8]) -> Option<u32>) -> Option<Self> {
        if data.first() == Some(&0) {
            return Self::from_owned_header(address, &PROGRAM_ID, &data[1..]);
        }
        if data.first() != Some(&1) || !matches!(data.len(), 212 | 216) {
            return None;
        }
        let base_mint = &data[1..33];
        let quote_mint = &data[33..65];
        let params = &data[129..];
        let quote_lots_per_unit = u64_at(params, 24)?;
        let tick_lots_per_unit = u64_at(params, 32)?;
        let base_lots_per_unit = u64_at(params, 40)?;
        let raw_units = match (params.get(82)?, params.len()) {
            (0, 83) => 1,
            (1, 87) => u32::from_le_bytes(params[83..87].try_into().ok()?),
            _ => return None,
        };
        let base_decimals = decimals(base_mint)?;
        let quote_decimals = decimals(quote_mint)?;
        let base_unit = 10u64.checked_pow(base_decimals)?.checked_mul(u64::from(raw_units))?;
        let quote_unit = 10u64.checked_pow(quote_decimals)?;
        if base_lots_per_unit == 0
            || quote_lots_per_unit == 0
            || base_unit % base_lots_per_unit != 0
            || quote_unit % quote_lots_per_unit != 0
            || u16::from_le_bytes(params[48..50].try_into().ok()?) > 10000
        {
            return None;
        }
        let base_lot = base_unit / base_lots_per_unit;
        let quote_lot = quote_unit / quote_lots_per_unit;
        let tick_atoms = quote_lot.checked_mul(tick_lots_per_unit)?;
        let mut header = [0u8; 576];
        header[..8].copy_from_slice(&MARKET_DISCRIMINANT.to_le_bytes());
        header[40..44].copy_from_slice(&base_decimals.to_le_bytes());
        header[48..80].copy_from_slice(base_mint);
        header[80..112].copy_from_slice(&data[65..97]);
        header[112..120].copy_from_slice(&base_lot.to_le_bytes());
        header[120..124].copy_from_slice(&quote_decimals.to_le_bytes());
        header[128..160].copy_from_slice(quote_mint);
        header[160..192].copy_from_slice(&data[97..129]);
        header[192..200].copy_from_slice(&quote_lot.to_le_bytes());
        header[200..208].copy_from_slice(&tick_atoms.to_le_bytes());
        header[312..316].copy_from_slice(&raw_units.to_le_bytes());
        Self::from_owned_header(address, &PROGRAM_ID, &header)
    }

    pub fn from_owned_header(address: &[u8], owner: &[u8], data: &[u8]) -> Option<Self> {
        if owner != PROGRAM_ID || data.len() != 576 || u64_at(data, 0)? != MARKET_DISCRIMINANT {
            return None;
        }
        let base_decimals = u32::from_le_bytes(data[40..44].try_into().ok()?);
        let quote_decimals = u32::from_le_bytes(data[120..124].try_into().ok()?);
        let raw_base_units = u32::from_le_bytes(data[312..316].try_into().ok()?);
        let base_unit_atoms = 10u64.checked_pow(base_decimals)?.checked_mul(u64::from(raw_base_units))?;
        let quote_unit_atoms = 10u64.checked_pow(quote_decimals)?;
        let base_lot_atoms = u64_at(data, 112)?;
        let quote_lot_atoms = u64_at(data, 192)?;
        let tick_quote_atoms = u64_at(data, 200)?;
        if base_unit_atoms == 0
            || base_lot_atoms == 0
            || quote_lot_atoms == 0
            || tick_quote_atoms == 0
            || base_unit_atoms % base_lot_atoms != 0
            || quote_unit_atoms % quote_lot_atoms != 0
            || tick_quote_atoms % quote_lot_atoms != 0
        {
            return None;
        }
        let base_lots_per_unit = base_unit_atoms / base_lot_atoms;
        let tick_quote_lots_per_unit = tick_quote_atoms / quote_lot_atoms;
        // The on-chain initializer requires this divisibility. Without it,
        // rounding each maker fill separately could change the taker's price.
        if tick_quote_lots_per_unit % base_lots_per_unit != 0 {
            return None;
        }
        let market = Self {
            address: address.try_into().ok()?,
            base_mint: key_at(data, 48)?,
            quote_mint: key_at(data, 128)?,
            base_vault: key_at(data, 80)?,
            quote_vault: key_at(data, 160)?,
            base_lot_atoms,
            quote_lot_atoms,
            base_lots_per_unit,
            tick_quote_lots_per_unit,
        };
        if market.base_mint == market.quote_mint
            || market.base_vault == market.quote_vault
            || [market.address, market.base_mint, market.quote_mint, market.base_vault, market.quote_vault].contains(&[0; 32])
        {
            return None;
        }
        Some(market)
    }
}

struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Option<&'a [u8]> {
        let value = self.0.get(..size)?;
        self.0 = self.0.get(size..)?;
        Some(value)
    }
    fn byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn optional_u64(&mut self) -> Option<Option<u64>> {
        match self.byte()? {
            0 => Some(None),
            1 => Some(Some(self.u64()?)),
            _ => None,
        }
    }
}

struct Order {
    packet: u8,
    side: u8,
    price_ticks: Option<u64>,
    base_limit: u64,
    min_base: u64,
    min_quote: u64,
    client_id: [u8; 16],
    free_funds: bool,
}

// Mirror decode_order_packet's contiguous omitted-tail compatibility. A valid
// Borsh packet is still required; discriminators and two direction bytes alone
// cannot establish an order or bind its FillSummary client ID.
fn order_packet(data: &[u8]) -> Option<Order> {
    let mut bytes = data.get(1..)?.to_vec();
    for padding in 0..=3 {
        if padding > 0 {
            bytes.push(0);
        }
        let decoded = (|| {
            let mut reader = Cursor(&bytes);
            let packet = reader.byte()?;
            let side = reader.byte()?;
            if !matches!(packet, 1 | 2) || side > 1 {
                return None;
            }
            let price_ticks = if packet == 1 { Some(reader.u64()?) } else { reader.optional_u64()? };
            let base_limit = reader.u64()?;
            let (min_base, min_quote) = if packet == 2 {
                let quote_limit = reader.u64()?;
                if base_limit == 0 && quote_limit == 0 {
                    return None;
                }
                (reader.u64()?, reader.u64()?)
            } else {
                (0, 0)
            };
            if reader.byte()? > 2 {
                return None;
            } // SelfTradeBehavior
            reader.optional_u64()?; // match limit
            let client_id = reader.take(16)?.try_into().ok()?;
            let free_funds = match reader.byte()? {
                0 => false,
                1 => true,
                _ => return None,
            };
            reader.optional_u64()?; // last valid slot
            reader.optional_u64()?; // last valid unix timestamp
            if packet == 1 && reader.byte()? > 1 {
                return None;
            } // fail silently
            if !reader.0.is_empty() {
                return None;
            }
            Some(Order {
                packet,
                side,
                price_ticks,
                base_limit,
                min_base,
                min_quote,
                client_id,
                free_funds,
            })
        })();
        if decoded.is_some() {
            return decoded;
        }
    }
    None
}

/// Proven taker settlement. This deliberately does not set transfer_verified:
/// free-funds fills settle balances inside Phoenix without moving SPL tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill {
    pub market: [u8; 32],
    pub trader: [u8; 32],
    pub input_mint: [u8; 32],
    pub input_amount: u64,
    pub output_mint: [u8; 32],
    pub output_amount: u64,
    pub fee_quote_atoms: u64,
    pub instruction_index: u32,
    pub log_instruction_index: u32,
    pub summary_event_index: u16,
    pub slot: u64,
}

/// Decode a single IOC or marketable limit order using authoritative market
/// parameters. The log must have returned successfully along with every
/// ancestor, even if the outer transaction caught another failed invocation.
/// Unknown, incomplete or inconsistent evidence returns None and remains
/// eligible for the existing replayable coverage diagnostic.
pub fn decode_fill(tx: &ConfirmedTransaction, index: usize, market: &Market, expected_slot: Option<u64>) -> Option<Fill> {
    if !tx.is_successful() {
        return None;
    }
    let instructions: Vec<_> = tx.walk_instructions().collect();
    let ix = instructions.get(index)?;
    if ix.program_id().0.as_slice() != PROGRAM_ID {
        return None;
    }
    let data = ix.data();
    let accounts = ix.accounts();
    // Swap / SwapWithFreeFunds accept IOC. PlaceLimitOrder variants accept
    // limit or post-only packets; only the former can have actual taker fills.
    let tag = *data.first()?;
    let order = order_packet(&data)?;
    let packet = order.packet;
    let side = order.side;
    if !matches!((tag, packet), (0 | 1, 2) | (2 | 3, 1))
        || side > 1
        || accounts.len() < 4
        || accounts[0].0.as_slice() != PROGRAM_ID
        || accounts[1].0.as_slice() != LOG_AUTHORITY
        || accounts[2].0.as_slice() != market.address
    {
        return None;
    }
    let trader: [u8; 32] = accounts[3].0.as_slice().try_into().ok()?;
    let free_funds = matches!(tag, 1 | 3);
    if order.free_funds != free_funds {
        return None;
    }
    if free_funds {
        if accounts.len() != 5 {
            return None;
        }
    } else {
        let vault_offset = if tag == 2 { 7 } else { 6 };
        if accounts.len() != vault_offset + 3
            || accounts[vault_offset].0.as_slice() != market.base_vault
            || accounts[vault_offset + 1].0.as_slice() != market.quote_vault
            || accounts[vault_offset + 2].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
        {
            return None;
        }
    }
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if depth == 0 {
        return None;
    }
    let successful = crate::logs::successful_invocations(tx);
    if !successful.contains(&(index as u32)) {
        return None;
    }
    let mut header = None::<Vec<u8>>;
    let mut event_index = 0u16;
    let mut total_base = 0u64;
    let mut notional_quote = 0u128;
    let mut summary = None;
    for (child_index, child) in instructions.iter().enumerate().skip(index + 1) {
        if child.is_root() || child.stack_height() <= depth {
            break;
        }
        if child.program_id().0.as_slice() != PROGRAM_ID {
            continue;
        }
        let event_data = child.data();
        let child_accounts = child.accounts();
        if child.stack_height() != depth + 1
            || !successful.contains(&(child_index as u32))
            || child_accounts.len() != 1
            || child_accounts[0].0.as_slice() != LOG_AUTHORITY
            || event_data.len() < 93
            || event_data[..3] != [15, 1, tag]
            || key_at(&event_data, 27)? != market.address
            || key_at(&event_data, 59)? != trader
            || expected_slot.is_some_and(|slot| u64_at(&event_data, 19) != Some(slot))
        {
            return None;
        }
        let identity = &event_data[2..91];
        if header.as_ref().is_some_and(|header| header != identity) {
            return None;
        }
        header = Some(identity.to_vec());
        let count = u16::from_le_bytes(event_data[91..93].try_into().ok()?);
        let mut offset = 93usize;
        if count == 0 {
            return None;
        }
        for _ in 0..count {
            let event_tag = *event_data.get(offset)?;
            let length = match event_tag {
                2 => 67,
                3 | 6 => 43,
                4 => 35,
                5 | 9 => 59,
                7 => 11,
                8 => 27,
                _ => return None,
            };
            let event = event_data.get(offset..offset.checked_add(length)?)?;
            if u16::from_le_bytes(event[1..3].try_into().ok()?) != event_index {
                return None;
            }
            if event_tag == 2 {
                let maker = key_at(event, 3)?;
                let maker_sequence = u64_at(event, 35)?;
                let price_ticks = u64_at(event, 43)?;
                let base_lots = u64_at(event, 51)?;
                // The high bit identifies a resting bid. A buy taker must
                // consume asks, and a sell taker must consume bids.
                if summary.is_some()
                    || maker == [0; 32]
                    || maker == trader
                    || price_ticks == 0
                    || base_lots == 0
                    || (maker_sequence >> 63) != u64::from(side)
                    || order
                        .price_ticks
                        .is_some_and(|limit| if side == 0 { price_ticks > limit } else { price_ticks < limit })
                {
                    return None;
                }
                total_base = total_base.checked_add(base_lots)?;
                let quote = u128::from(price_ticks)
                    .checked_mul(u128::from(base_lots))?
                    .checked_mul(u128::from(market.tick_quote_lots_per_unit))?
                    / u128::from(market.base_lots_per_unit);
                notional_quote = notional_quote.checked_add(quote)?;
            } else if event_tag == 6 {
                let base = u64_at(event, 19)?;
                let quote = u64_at(event, 27)?;
                let fee = u64_at(event, 35)?;
                let notional = u64::try_from(notional_quote).ok()?;
                let expected_quote = if side == 0 { notional.checked_add(fee)? } else { notional.checked_sub(fee)? };
                if summary.is_some()
                    || event[3..19] != order.client_id
                    || base != total_base
                    || quote != expected_quote
                    || fee > notional
                    || (order.base_limit != 0 && base > order.base_limit)
                    || base < order.min_base
                    || quote < order.min_quote
                {
                    return None;
                }
                summary = Some((base, quote, fee, child_index, event_index));
            }
            event_index = event_index.checked_add(1)?;
            offset += length;
        }
        if offset != event_data.len() {
            return None;
        }
    }
    let (base_lots, quote_lots, fee, log_index, summary_index) = summary?;
    let base_atoms = base_lots.checked_mul(market.base_lot_atoms)?;
    let quote_atoms = quote_lots.checked_mul(market.quote_lot_atoms)?;
    if base_atoms == 0 || quote_atoms == 0 {
        return None;
    }
    let (input_mint, input_amount, output_mint, output_amount) = if side == 0 {
        (market.quote_mint, quote_atoms, market.base_mint, base_atoms)
    } else {
        (market.base_mint, base_atoms, market.quote_mint, quote_atoms)
    };
    Some(Fill {
        market: market.address,
        trader,
        input_mint,
        input_amount,
        output_mint,
        output_amount,
        fee_quote_atoms: fee.checked_mul(market.quote_lot_atoms)?,
        instruction_index: u32::try_from(index).ok()?,
        log_instruction_index: u32::try_from(log_index).ok()?,
        summary_event_index: summary_index,
        slot: u64_at(&header?, 17)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routed_pool::test_fixture::make_tx;
    use substreams_solana::{
        base58,
        pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions},
    };

    #[test]
    fn original_wallet_fills_match_market_atoms_and_exact_token_settlement() {
        for (raw, header, index, expected) in [
            (
                include_bytes!("../fixtures/phoenix-immediate-1.transaction.pb").as_slice(),
                include_bytes!("../fixtures/phoenix-market-1.header.bin").as_slice(),
                7,
                (95_246_741, 99_890_000_000),
            ),
            (
                include_bytes!("../fixtures/phoenix-immediate-2.transaction.pb").as_slice(),
                include_bytes!("../fixtures/phoenix-market-2.header.bin").as_slice(),
                13,
                (15_273_853, 17_800),
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&raw.to_vec()).unwrap();
            let instructions: Vec<_> = tx.walk_instructions().collect();
            let accounts = instructions[index].accounts();
            let market = Market::from_owned_header(accounts[2].0, &PROGRAM_ID, header).unwrap();
            let fill = decode_fill(&tx, index, &market, None).unwrap();
            assert_eq!((fill.input_amount, fill.output_amount), expected);
            let children: Vec<_> = instructions
                .iter()
                .skip(index + 1)
                .take_while(|ix| !ix.is_root() && ix.stack_height() > 2)
                .collect();
            let amounts: Vec<_> = children
                .iter()
                .filter(|ix| ix.program_id().0.as_slice() == substreams_solana_idls::spl::token::PROGRAM_ID)
                .map(|ix| u64_at(&ix.data(), 1).unwrap())
                .collect();
            assert!(amounts.contains(&fill.input_amount));
            assert!(amounts.contains(&fill.output_amount));
            assert!(decode_fill(&tx, index, &market, Some(fill.slot)).is_some());
            assert!(decode_fill(&tx, index, &market, Some(fill.slot + 1)).is_none());
        }
    }

    #[test]
    fn finalized_fill_corpus_preserves_independently_verified_user_amounts() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/phoenix");
        for line in include_str!("../fixtures/phoenix/cases.tsv").lines() {
            let fields: Vec<_> = line.split('\t').collect();
            let raw = std::fs::read(directory.join(fields[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&raw).unwrap();
            let address = base58::decode(fields[2]).unwrap();
            let header = std::fs::read(directory.join(format!("{}.header.bin", fields[2]))).unwrap();
            let market = Market::from_owned_header(&address, &PROGRAM_ID, &header).unwrap();
            let index = fields[1].parse().unwrap();
            let fill = decode_fill(&tx, index, &market, None).unwrap();
            let normalized = crate::process_transaction_with_phoenix(tx.clone(), Some(fill.slot), |address, position| {
                if address == market.address && position == index as u32 {
                    Some(market.clone())
                } else {
                    None
                }
            })
            .unwrap();
            let rows: Vec<_> = normalized
                .swaps
                .iter()
                .filter(|swap| swap.protocol.to_i32() == 50 && swap.source_index == Some(index as u32))
                .collect();
            assert_eq!(rows.len(), 1);
            assert!(rows[0].orderbook_fill_verified);
            assert!(!rows[0].transfer_verified);
            assert!(rows[0].source_transfer_index.is_none());
            assert_eq!((rows[0].input_amount, rows[0].output_amount), (fill.input_amount, fill.output_amount));
            assert_eq!(base58::encode(fill.input_mint), fields[3]);
            assert_eq!(fill.input_amount.to_string(), fields[4]);
            assert_eq!(base58::encode(fill.output_mint), fields[5]);
            assert_eq!(fill.output_amount.to_string(), fields[6]);
            assert_eq!(fill.fee_quote_atoms.to_string(), fields[7]);
        }
        for line in include_str!("../fixtures/phoenix/unfilled.tsv").lines() {
            let fields: Vec<_> = line.split('\t').collect();
            let raw = std::fs::read(directory.join(fields[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&raw).unwrap();
            let address = base58::decode(fields[2]).unwrap();
            let header = std::fs::read(directory.join(format!("{}.header.bin", fields[2]))).unwrap();
            let market = Market::from_owned_header(&address, &PROGRAM_ID, &header).unwrap();
            assert!(
                decode_fill(&tx, fields[1].parse().unwrap(), &market, None).is_none(),
                "unfilled order {}",
                fields[0]
            );
        }
    }

    fn ledger_tx(side: u8, chunks: bool) -> (ConfirmedTransaction, Market) {
        let mut header = [0u8; 576];
        header[..8].copy_from_slice(&MARKET_DISCRIMINANT.to_le_bytes());
        for (offset, key) in [(48, [30; 32]), (128, [31; 32]), (80, [32; 32]), (160, [33; 32])] {
            header[offset..offset + 32].copy_from_slice(&key);
        }
        header[40..44].copy_from_slice(&6u32.to_le_bytes());
        header[120..124].copy_from_slice(&6u32.to_le_bytes());
        header[112..120].copy_from_slice(&100u64.to_le_bytes());
        header[192..200].copy_from_slice(&10u64.to_le_bytes());
        header[200..208].copy_from_slice(&100_000u64.to_le_bytes());
        header[312..316].copy_from_slice(&1u32.to_le_bytes());
        let market = Market::from_owned_header(&[20; 32], &PROGRAM_ID, &header).unwrap();
        let mut order = vec![1, 2, side, 0];
        for amount in [100u64, 0, 0, 0] {
            order.extend_from_slice(&amount.to_le_bytes());
        }
        order.extend_from_slice(&[0, 0]); // self-trade Abort, no match limit
        order.extend_from_slice(&0u128.to_le_bytes());
        order.extend_from_slice(&[1, 0, 0]); // free funds, no expiry
        let mut tx = make_tx(PROGRAM_ID, &[PROGRAM_ID, LOG_AUTHORITY, [20; 32], [21; 32], [22; 32]], order);
        let batch = |count: u16| {
            let mut data = vec![15, 1, 1];
            data.extend_from_slice(&42u64.to_le_bytes());
            data.extend_from_slice(&100i64.to_le_bytes());
            data.extend_from_slice(&200u64.to_le_bytes());
            data.extend_from_slice(&[20; 32]);
            data.extend_from_slice(&[21; 32]);
            data.extend_from_slice(&count.to_le_bytes());
            data
        };
        let mut fill = vec![2, 0, 0];
        fill.extend_from_slice(&[40; 32]);
        fill.extend_from_slice(&(if side == 0 { 1u64 } else { (1u64 << 63) + 1 }).to_le_bytes());
        fill.extend_from_slice(&10u64.to_le_bytes());
        fill.extend_from_slice(&20u64.to_le_bytes());
        fill.extend_from_slice(&0u64.to_le_bytes());
        let mut summary = vec![6, 1, 0];
        summary.extend_from_slice(&0u128.to_le_bytes());
        summary.extend_from_slice(&20u64.to_le_bytes());
        summary.extend_from_slice(&(if side == 0 { 203u64 } else { 197u64 }).to_le_bytes());
        summary.extend_from_slice(&3u64.to_le_bytes());
        let event = |data: Vec<u8>| InnerInstruction {
            program_id_index: 1,
            accounts: vec![3],
            data,
            stack_height: Some(2),
        };
        let children = if chunks {
            let mut first = batch(1);
            first.extend(fill);
            let mut second = batch(1);
            second.extend(summary);
            vec![event(first), event(second)]
        } else {
            let mut data = batch(2);
            data.extend(fill);
            data.extend(summary);
            vec![event(data)]
        };
        let meta = tx.meta.as_option_mut().unwrap();
        meta.log_messages = vec![format!("Program {} invoke [1]", base58::encode(PROGRAM_ID))];
        for _ in &children {
            meta.log_messages.push(format!("Program {} invoke [2]", base58::encode(PROGRAM_ID)));
            meta.log_messages.push(format!("Program {} success", base58::encode(PROGRAM_ID)));
        }
        meta.log_messages.push(format!("Program {} success", base58::encode(PROGRAM_ID)));
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: children,
        }];
        (tx, market)
    }

    #[test]
    fn exchange_balances_and_split_batches_use_actual_fills_in_both_directions() {
        for side in [0, 1] {
            for chunks in [false, true] {
                let (tx, market) = ledger_tx(side, chunks);
                assert!(tx.meta.as_option().unwrap().pre_token_balances.is_empty());
                let fill = decode_fill(&tx, 0, &market, Some(200)).unwrap();
                assert_eq!((fill.input_amount, fill.output_amount), if side == 0 { (2030, 2000) } else { (2000, 1970) });
                assert_eq!(fill.fee_quote_atoms, 30);
                assert_eq!(fill.summary_event_index, 1);
                assert_eq!(fill.log_instruction_index, if chunks { 2 } else { 1 });
                // A marketable limit order has the same settlement contract.
                let mut limit = tx;
                let mut order = vec![3, 1, side];
                order.extend_from_slice(&10u64.to_le_bytes());
                order.extend_from_slice(&100u64.to_le_bytes());
                order.extend_from_slice(&[0, 0]);
                order.extend_from_slice(&0u128.to_le_bytes());
                order.extend_from_slice(&[1, 0, 0, 0]);
                limit.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].data = order;
                for ix in &mut limit.meta.as_option_mut().unwrap().inner_instructions[0].instructions {
                    ix.data[2] = 3;
                }
                assert!(decode_fill(&limit, 0, &market, Some(200)).is_some());
            }
        }
    }

    #[test]
    fn invalid_or_rolled_back_audit_logs_never_become_settlements() {
        for mode in 0..18 {
            let (mut tx, mut market) = ledger_tx(0, true);
            let events = &mut tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions;
            match mode {
                0 => events[0].accounts[0] = 4,
                1 => events[0].data[2] = 3,
                2 => events[0].data[27] ^= 1,
                3 => events[0].data[59] ^= 1,
                4 => events[1].data[3] ^= 1,
                5 => events[1].data[94] = 0,
                6 => events[1].data[112] ^= 1,
                7 => events[1].data[120] ^= 1,
                8 => events[1].data.push(0),
                9 => {
                    events.pop();
                }
                10 => events[0].data[144] = 0,
                11 => events[0].data[135] |= 0x80,
                12 => market.base_lot_atoms = u64::MAX,
                13 => events[0].stack_height = Some(3),
                14 => tx.meta.as_option_mut().unwrap().log_messages.pop().map(|_| ()).unwrap(),
                15 => {
                    *tx.meta.as_option_mut().unwrap().log_messages.last_mut().unwrap() =
                        format!("Program {} failed: custom program error: 0x1", base58::encode(PROGRAM_ID))
                }
                16 => tx.meta.as_option_mut().unwrap().log_messages.clear(),
                17 => tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].data[1] = 0,
                _ => unreachable!(),
            }
            assert!(decode_fill(&tx, 0, &market, Some(200)).is_none(), "invalid mode {mode}");
        }
    }

    fn initialize_tx(raw_units: Option<u32>) -> ConfirmedTransaction {
        use substreams_solana::pb::sf::solana::r#type::v1::{TokenBalance, UiTokenAmount};
        let mut data = vec![100];
        for value in [1024u64, 1024, 1153, 100_000, 10_000, 10_000] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&150u16.to_le_bytes());
        data.extend_from_slice(&[50; 32]);
        match raw_units {
            None => data.push(0),
            Some(value) => {
                data.push(1);
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut tx = make_tx(
            PROGRAM_ID,
            &[
                PROGRAM_ID,
                LOG_AUTHORITY,
                [20; 32],
                [21; 32],
                [30; 32],
                [31; 32],
                [32; 32],
                [33; 32],
                [0; 32],
                substreams_solana_idls::spl::token::PROGRAM_ID,
            ],
            data,
        );
        let meta = tx.meta.as_option_mut().unwrap();
        meta.log_messages = vec![
            format!("Program {} invoke [1]", base58::encode(PROGRAM_ID)),
            format!("Program {} success", base58::encode(PROGRAM_ID)),
        ];
        for (index, mint) in [(8, [30; 32]), (9, [31; 32])] {
            meta.post_token_balances.push(TokenBalance {
                account_index: index,
                mint: base58::encode(mint),
                ui_token_amount: buffa::MessageField::some(UiTokenAmount {
                    decimals: 6,
                    amount: "0".into(),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        tx
    }

    #[test]
    fn snapshot_boundary_and_future_market_initialization_preserve_ordinal_metadata() {
        use crate::phoenix_metadata::{ordinal, updates, SNAPSHOT_SLOT};
        use substreams_solana::pb::sf::solana::r#type::v1::Block;
        let first = Block {
            slot: SNAPSHOT_SLOT + 5,
            parent_slot: SNAPSHOT_SLOT,
            ..Default::default()
        };
        let seeds = updates(&first);
        assert_eq!(seeds.iter().filter(|(_, key, _)| key.starts_with("m:")).count(), 255);
        assert_eq!(seeds.iter().filter(|(_, key, _)| key.starts_with("d:")).count(), 168);
        for (_, key, value) in seeds.iter().filter(|(_, key, _)| key.starts_with("m:")) {
            let address = base58::decode(&key[2..]).unwrap();
            assert!(Market::from_definition(&address, value, |_| None).is_some());
        }
        assert!(updates(&Block {
            slot: SNAPSHOT_SLOT + 6,
            parent_slot: SNAPSHOT_SLOT + 5,
            ..Default::default()
        })
        .is_empty());
        let slot = SNAPSHOT_SLOT + 10;
        let mut ledger = ledger_tx(0, false).0;
        for event in &mut ledger.meta.as_option_mut().unwrap().inner_instructions[0].instructions {
            event.data[19..27].copy_from_slice(&slot.to_le_bytes());
        }
        let mut block = Block {
            slot,
            parent_slot: SNAPSHOT_SLOT + 9,
            transactions: vec![initialize_tx(None), ledger, initialize_tx(Some(100))],
            ..Default::default()
        };
        let changes = updates(&block);
        let at = |ord: u64, key: &str| {
            changes
                .iter()
                .rev()
                .find(|(position, k, _)| *position <= ord && k == key)
                .map(|(_, _, value)| value.clone())
        };
        let key = format!("m:{}", base58::encode([20; 32]));
        let market_at = |ord| {
            Market::from_definition(&[20; 32], &at(ord, &key)?, |mint| {
                Some(u32::from_le_bytes(at(ord, &format!("d:{}", base58::encode(mint)))?.as_slice().try_into().ok()?))
            })
        };
        assert!(market_at(0).is_none());
        let current = market_at(ordinal(1, 0)).unwrap();
        let next = market_at(ordinal(2, 0)).unwrap();
        assert_eq!(current.base_lot_atoms, 100);
        assert_eq!(next.base_lot_atoms, 10_000);
        let decoded = crate::process_transaction_with_phoenix(block.transactions[1].clone(), Some(slot), |_, _| Some(current.clone())).unwrap();
        assert_eq!(decoded.swaps.len(), 1);
        assert_eq!((decoded.swaps[0].input_amount, decoded.swaps[0].output_amount), (2030, 2000));
        assert!(decoded.swaps[0].orderbook_fill_verified);
        assert!(!decoded.swaps[0].transfer_verified);
        assert!(decoded.coverage_candidates.is_empty());
        // A caught, rolled-back initialization never replaces a valid market.
        *block.transactions[2].meta.as_option_mut().unwrap().log_messages.last_mut().unwrap() =
            format!("Program {} failed: custom program error: 0x1", base58::encode(PROGRAM_ID));
        assert_eq!(updates(&block).iter().filter(|(_, key, _)| key.starts_with("m:")).count(), 1);
    }

    #[test]
    fn missing_mint_metadata_keeps_future_fills_replayable_until_a_deposit_supplies_it() {
        use crate::phoenix_metadata::{updates, SNAPSHOT_SLOT};
        use substreams_solana::pb::sf::solana::r#type::v1::Block;
        let mut init = initialize_tx(None);
        init.meta.as_option_mut().unwrap().post_token_balances.clear();
        let changes = updates(&Block {
            slot: SNAPSHOT_SLOT + 10,
            parent_slot: SNAPSHOT_SLOT + 9,
            transactions: vec![init],
            ..Default::default()
        });
        let definition = &changes.iter().find(|(_, key, _)| key.starts_with("m:")).unwrap().2;
        assert!(Market::from_definition(&[20; 32], definition, |_| None).is_none());
        let ledger = ledger_tx(0, true).0;
        let diagnostic = crate::process_transaction_with_phoenix(ledger.clone(), Some(200), |_, _| None).unwrap();
        assert!(diagnostic.swaps.is_empty());
        assert!(!diagnostic.coverage_transaction.is_empty());
        assert_eq!(diagnostic.coverage_candidates.len(), 1);
        let market = Market::from_definition(&[20; 32], definition, |_| Some(6)).unwrap();
        let decoded = crate::process_transaction_with_phoenix(ledger, Some(200), |_, _| Some(market.clone())).unwrap();
        assert_eq!(decoded.swaps.len(), 1);
        assert!(decoded.swaps[0].orderbook_fill_verified);
        assert!(decoded.swaps[0].source_transfer_index.is_none());
        assert!(decoded.swaps[0].source_event_index.is_some());
    }

    #[test]
    fn wrong_market_types_and_invalid_unit_conversions_are_rejected() {
        let header = include_bytes!("../fixtures/phoenix-market-1.header.bin");
        assert!(Market::from_owned_header(&[20; 32], &[0; 32], header).is_none());
        for (offset, value) in [(0, 0u64), (112, 0), (192, 0), (200, 0), (112, 7), (192, 7), (200, 7), (312, 0), (40, 255)] {
            let mut bad = header.to_vec();
            bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert!(Market::from_owned_header(&[20; 32], &PROGRAM_ID, &bad).is_none(), "bad offset {offset}");
        }
    }
}
