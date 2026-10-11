//! OpenBook market incarnations: a finalized owned-account snapshot followed
//! by completed CreateMarket/CloseMarket calls. Lot sizes are never inferred
//! from price, deposits, or an order's limits.
use crate::openbook::{Market, CLOSE, CREATE, PROGRAM_ID};
use std::collections::HashSet;
use substreams_solana::{base58, block_view::InstructionView, pb::sf::solana::r#type::v1::Block};
use substreams_solana_idls::spl::token;

pub(crate) const SNAPSHOT_SLOT: u64 = 455779697;
const SEED: &[u8; 283 * 880] = include_bytes!("../fixtures/openbook/market-registry-455779697.bin");
const EVENT_AUTHORITY: [u8; 32] = substreams_solana::b58!("8rDvL1qM41mYVNYS6oKLfGXyBGov3jLtbELToxjSwShj");
const EVENT_CPI: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];
const METADATA: [u8; 8] = [209, 87, 212, 236, 164, 58, 60, 117];

fn initialization_header(index: usize, instructions: &[InstructionView], successful: &HashSet<u32>) -> Option<Vec<u8>> {
    let ix = &instructions[index];
    let data = ix.data();
    let accounts = ix.accounts();
    if data.get(..8)? != CREATE
        || accounts.len() < 21
        || accounts[10].0.iter().any(|b| *b != 0)
        || accounts[11].0.as_slice() != token::PROGRAM_ID
        || accounts[12].0.as_slice() != substreams_solana::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
        || accounts[19].0.as_slice() != EVENT_AUTHORITY
        || accounts[20].0.as_slice() != PROGRAM_ID
    {
        return None;
    }
    let name_len = u32::from_le_bytes(data.get(8..12)?.try_into().ok()?) as usize;
    if name_len > 16 {
        return None;
    }
    let name = data.get(12..12 + name_len)?;
    std::str::from_utf8(name).ok()?;
    let mut cursor = 12 + name_len;
    let conf = f32::from_le_bytes(data.get(cursor..cursor + 4)?.try_into().ok()?);
    cursor += 4;
    let staleness = match data.get(cursor)? {
        0 => {
            cursor += 1;
            -1i64
        }
        1 => {
            cursor += 1;
            let value = u32::from_le_bytes(data.get(cursor..cursor + 4)?.try_into().ok()?);
            cursor += 4;
            i64::from(value)
        }
        _ => return None,
    };
    if data.len() != cursor + 40 {
        return None;
    }
    let quote_lot = &data[cursor..cursor + 8];
    let base_lot = &data[cursor + 8..cursor + 16];
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if depth == 0 {
        return None;
    }
    let mut metadata = None;
    for (child_index, child) in instructions.iter().enumerate().skip(index + 1) {
        if child.is_root() || child.stack_height() <= depth {
            break;
        }
        let event = child.data();
        if child.stack_height() != depth + 1
            || child.program_id().0.as_slice() != PROGRAM_ID
            || event.get(..8) != Some(EVENT_CPI.as_slice())
            || event.get(8..16) != Some(METADATA.as_slice())
        {
            continue;
        }
        let event_accounts = child.accounts();
        if !successful.contains(&(child_index as u32)) || event_accounts.len() != 1 || event_accounts[0].0.as_slice() != EVENT_AUTHORITY || metadata.is_some() {
            return None;
        }
        let event_name_len = u32::from_le_bytes(event.get(48..52)?.try_into().ok()?) as usize;
        if event_name_len != name_len || event.len() != 134 + name_len || event[16..48] != *accounts[0].0 || event[52..52 + name_len] != *name {
            return None;
        }
        let pos = 52 + name_len;
        if event[pos..pos + 32] != *accounts[8].0
            || event[pos + 32..pos + 64] != *accounts[9].0
            || event[pos + 66..pos + 74] != *base_lot
            || event[pos + 74..pos + 82] != *quote_lot
        {
            return None;
        }
        metadata = Some((event[pos + 64], event[pos + 65]));
    }
    let (base_decimals, quote_decimals) = metadata?;
    let mut header = vec![0; 848];
    header[..8].copy_from_slice(&[219, 190, 213, 55, 0, 227, 198, 154]);
    header[9] = base_decimals;
    header[10] = quote_decimals;
    for (role, offset) in [(1, 16), (2, 200), (3, 232), (4, 264), (8, 576), (9, 608), (6, 640), (7, 680)] {
        header[offset..offset + 32].copy_from_slice(accounts[role].0);
    }
    header[56..88].copy_from_slice(accounts[15].0);
    for (role, offset) in [(16, 88), (17, 120), (18, 152), (13, 296), (14, 328)] {
        if accounts[role].0.as_slice() != PROGRAM_ID {
            header[offset..offset + 32].copy_from_slice(accounts[role].0);
        }
    }
    header[184..184 + name_len].copy_from_slice(name);
    header[360..368].copy_from_slice(&f64::from(conf).to_le_bytes());
    header[368..376].copy_from_slice(&staleness.to_le_bytes());
    header[448..464].copy_from_slice(&data[cursor..cursor + 16]);
    header[480..496].copy_from_slice(&data[cursor + 16..cursor + 32]);
    header[48..56].copy_from_slice(&data[cursor + 32..cursor + 40]);
    Market::from_owned_header(accounts[0].0, &PROGRAM_ID, &header)?;
    Some(header)
}

pub(crate) fn updates(block: &Block) -> Vec<(u64, String, Vec<u8>)> {
    let mut result = Vec::new();
    if block.slot > SNAPSHOT_SLOT && block.parent_slot <= SNAPSHOT_SLOT {
        for row in SEED.chunks_exact(880) {
            result.push((0, format!("m:{}", base58::encode(&row[..32])), [vec![0], row[32..].to_vec()].concat()));
        }
        result.push((0, "__openbook_snapshot".into(), SNAPSHOT_SLOT.to_le_bytes().to_vec()));
    }
    for (transaction_index, tx) in block.transactions.iter().enumerate() {
        if !tx.is_successful() {
            continue;
        }
        let instructions: Vec<_> = tx.walk_instructions().collect();
        if !instructions
            .iter()
            .any(|ix| ix.program_id().0.as_slice() == PROGRAM_ID && (ix.data().starts_with(&CREATE) || ix.data().starts_with(&CLOSE)))
        {
            continue;
        }
        let successful = crate::logs::successful_invocations(tx);
        for (index, ix) in instructions.iter().enumerate() {
            if ix.program_id().0.as_slice() != PROGRAM_ID || !successful.contains(&(index as u32)) {
                continue;
            }
            let accounts = ix.accounts();
            let update = if ix.data().starts_with(&CREATE) {
                initialization_header(index, &instructions, &successful).map(|header| (accounts[0].0, [vec![1], header].concat()))
            } else if ix.data().as_slice() == CLOSE && accounts.len() >= 7 && accounts[6].0.as_slice() == token::PROGRAM_ID {
                // A closed market must not keep its old lot sizes if reused.
                Some((accounts[1].0, Vec::new()))
            } else {
                None
            };
            if let Some((address, value)) = update {
                result.push((
                    crate::phoenix_metadata::ordinal(transaction_index, index as u32),
                    format!("m:{}", base58::encode(address)),
                    value,
                ));
            }
        }
    }
    result.sort_by_key(|row| row.0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{ConfirmedTransaction, InnerInstruction, InnerInstructions};
    fn initialization() -> ConfirmedTransaction {
        let h = include_bytes!("../fixtures/openbook/sol-usdc-market-455779697.bin");
        let key = |offset| <[u8; 32]>::try_from(&h[offset..offset + 32]).unwrap();
        let row = SEED.chunks_exact(880).find(|row| &row[32..] == h.as_slice()).unwrap();
        let address = <[u8; 32]>::try_from(&row[..32]).unwrap();
        let accounts = [
            address,
            key(16),
            key(200),
            key(232),
            key(264),
            [31; 32],
            key(640),
            key(680),
            key(576),
            key(608),
            [0; 32],
            token::PROGRAM_ID,
            substreams_solana::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
            PROGRAM_ID,
            PROGRAM_ID,
            [32; 32],
            PROGRAM_ID,
            PROGRAM_ID,
            PROGRAM_ID,
            EVENT_AUTHORITY,
            PROGRAM_ID,
        ];
        let mut data = CREATE.to_vec();
        data.extend(6u32.to_le_bytes());
        data.extend(b"future");
        data.extend(0.1f32.to_le_bytes());
        data.push(1);
        data.extend(100u32.to_le_bytes());
        for value in [10i64, 100, 0, 1000, 0] {
            data.extend(value.to_le_bytes());
        }
        let mut tx = crate::routed_pool::test_fixture::make_tx(PROGRAM_ID, &accounts, data);
        let mut event = EVENT_CPI.to_vec();
        event.extend(METADATA);
        event.extend(address);
        event.extend(6u32.to_le_bytes());
        event.extend(b"future");
        event.extend(accounts[8]);
        event.extend(accounts[9]);
        event.extend([9, 6]);
        event.extend(100i64.to_le_bytes());
        event.extend(10i64.to_le_bytes());
        let meta = tx.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![InnerInstruction {
                program_id_index: 1,
                accounts: vec![21],
                data: event,
                stack_height: Some(2),
            }],
        }];
        let program = base58::encode(PROGRAM_ID);
        meta.log_messages = vec![
            format!("Program {program} invoke [1]"),
            format!("Program {program} invoke [2]"),
            format!("Program {program} success"),
            format!("Program {program} success"),
        ];
        tx
    }
    #[test]
    fn snapshot_seeds_owned_market_headers_at_the_first_produced_slot() {
        use solana_program::pubkey::Pubkey;
        let block = Block {
            slot: SNAPSHOT_SLOT + 3,
            parent_slot: SNAPSHOT_SLOT,
            ..Default::default()
        };
        let rows = updates(&block);
        assert_eq!(rows.len(), 284);
        let program = Pubkey::new_from_array(PROGRAM_ID);
        let token = Pubkey::new_from_array(token::PROGRAM_ID);
        let ata = Pubkey::new_from_array(substreams_solana::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));
        for row in SEED.chunks_exact(880) {
            let address = Pubkey::try_from(&row[..32]).unwrap();
            let header = &row[32..];
            let market = Market::from_owned_header(address.as_ref(), &PROGRAM_ID, header).unwrap();
            let (authority, bump) = Pubkey::find_program_address(&[b"Market", address.as_ref()], &program);
            assert_eq!(market.authority, authority.to_bytes());
            assert_eq!(header[8], bump);
            for (mint, vault) in [(market.base_mint, market.base_vault), (market.quote_mint, market.quote_vault)] {
                assert_eq!(
                    Pubkey::find_program_address(&[authority.as_ref(), token.as_ref(), &mint], &ata).0.to_bytes(),
                    vault
                );
            }
        }
        assert!(updates(&Block {
            slot: SNAPSHOT_SLOT + 4,
            parent_slot: SNAPSHOT_SLOT + 3,
            ..Default::default()
        })
        .is_empty());
        assert!(updates(&Block {
            slot: SNAPSHOT_SLOT,
            parent_slot: SNAPSHOT_SLOT - 1,
            ..Default::default()
        })
        .is_empty());
    }
    #[test]
    fn future_initializations_require_signed_metadata_matching_the_created_market() {
        let tx = initialization();
        let block = Block {
            slot: SNAPSHOT_SLOT + 20,
            parent_slot: SNAPSHOT_SLOT + 19,
            transactions: vec![tx.clone()],
            ..Default::default()
        };
        let rows = updates(&block);
        assert_eq!(rows.len(), 1);
        let address = base58::decode(rows[0].1.strip_prefix("m:").unwrap()).unwrap();
        assert!(Market::from_owned_header(&address, &PROGRAM_ID, &rows[0].2[1..]).is_some());
        assert_eq!(rows[0].2[10..12], [9, 6]);
        assert_eq!(i64::from_le_bytes(rows[0].2[457..465].try_into().unwrap()), 100);
        assert_eq!(rows[0].0, crate::phoenix_metadata::ordinal(0, 0));
        for variant in 0..9 {
            let mut wrong = tx.clone();
            match variant {
                0 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions.clear(),
                1 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[0].accounts[0] = 2,
                2 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[0].data[16] ^= 1,
                3 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[0].data[118] = 20,
                4 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[0].data[126] ^= 1,
                5 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].accounts[11] = 2,
                6 => *wrong.meta.as_mut().unwrap().log_messages.last_mut().unwrap() = format!("Program {} failed: error", base58::encode(PROGRAM_ID)),
                7 => {
                    wrong.meta.as_mut().unwrap().log_messages.pop();
                }
                _ => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data.push(0),
            }
            let mut invalid = block.clone();
            invalid.transactions = vec![wrong];
            assert!(updates(&invalid).is_empty(), "variant {variant}");
        }
    }
    #[test]
    fn closures_and_reinitializations_replace_metadata_at_exact_execution_ordinals() {
        let first = initialization();
        let a = first.transaction.as_ref().unwrap().message.as_ref().unwrap().account_keys[2].clone();
        let address: [u8; 32] = a.try_into().unwrap();
        let mut close = crate::routed_pool::test_fixture::make_tx(
            PROGRAM_ID,
            &[[31; 32], address, [32; 32], [33; 32], [34; 32], [35; 32], token::PROGRAM_ID],
            CLOSE.to_vec(),
        );
        let program = base58::encode(PROGRAM_ID);
        close.meta.as_mut().unwrap().log_messages = vec![format!("Program {program} invoke [1]"), format!("Program {program} success")];
        let mut second = initialization();
        let data = &mut second.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data;
        data[27..35].copy_from_slice(&20i64.to_le_bytes());
        second.meta.as_mut().unwrap().inner_instructions[0].instructions[0].data[132..140].copy_from_slice(&20i64.to_le_bytes());
        let rows = updates(&Block {
            slot: SNAPSHOT_SLOT + 20,
            parent_slot: SNAPSHOT_SLOT + 19,
            transactions: vec![first, close, second],
            ..Default::default()
        });
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter().map(|row| row.0).collect::<Vec<_>>(),
            vec![
                crate::phoenix_metadata::ordinal(0, 0),
                crate::phoenix_metadata::ordinal(1, 0),
                crate::phoenix_metadata::ordinal(2, 0)
            ]
        );
        assert!(rows[1].2.is_empty());
        assert_eq!(i64::from_le_bytes(rows[0].2[449..457].try_into().unwrap()), 10);
        assert_eq!(i64::from_le_bytes(rows[2].2[449..457].try_into().unwrap()), 20);
    }
}
