//! Native market layouts, gated by program, swap opcode and payload length.
//! Amounts are taken from scoped user/vault CPIs in `quality`, never from a
//! quoted minimum, router total, oracle update or whole-transaction delta.
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{b58, block_view::InstructionView};

pub(crate) const VENUES: &[(Protocol, [u8; 32])] = &[
    (Protocol::GoonfiV2, b58!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE")),
    (Protocol::Bisonfi, b58!("BiSoNHVpsVZW2F7rx2eQ59yQwKxzU5NvBcmKshCSUypi")),
    (Protocol::Manifest, b58!("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms")),
    (Protocol::Humidifi, b58!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp")),
    (Protocol::OrcaV2, b58!("9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP")),
    (Protocol::Alphaq, b58!("ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA")),
    (Protocol::Kipseli, b58!("3TK9D8aoBFYjYZtKCjciPrVrRStsnvo7KmpcJqDavpaU")),
    (Protocol::Flux, b58!("FLUX6xBayGxLX9UcimVRxXFMHH6q43mAbRvDzSpCsvfK")),
    // Jupiter labels the separate pricing program ojh19... as Scorch. It
    // does not move tokens; native swaps execute in SCoRcH8c... instead.
    (Protocol::Scorch, b58!("SCoRcH8c2dpjvcJD6FiPbCSQyQgu3PcUAWj2Xxx3mqn")),
    (Protocol::Obsidian, b58!("HBVw6bZtcCaezhcBrmfyXBSBRWCdv72271xQ4GPvms2z")),
    (Protocol::Tesserav, b58!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH")),
    (Protocol::Deriverse, b58!("DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD")),
    (Protocol::Zerofi, b58!("ZERor4xhbUycZ6gb9ntrhqscUcZmAbQDjEAtCf4hbZY")),
];

pub(crate) struct Layout {
    pub protocol: Protocol,
    pub pool: usize,
    pub authority: usize,
    pub users: [usize; 2],
    pub vaults: [usize; 2],
    pub amount: u64,
    pub exact_input: bool,
    pub input_limit: bool,
    pub input_first: bool,
}

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let program = ix.program_id();
    let protocol = VENUES.iter().find(|(_, id)| id.as_slice() == program.0.as_slice())?.0;
    let raw = ix.data();
    let mut data = raw.clone();
    if protocol == Protocol::Humidifi {
        // XOR is symmetrical. Decode with byte operations (no alignment or
        // host-endianness assumptions in WASM).
        let key = [58, 255, 47, 255, 226, 186, 235, 195];
        for (i, byte) in data.iter_mut().enumerate() {
            let position = (i / 8) as u16;
            *byte ^= key[i % 8] ^ position.to_le_bytes()[i % 2];
        }
    }
    let tag = *data.first()?;
    // (pool, authority, users A/B, vaults A/B, amount offset, input-first)
    let (pool, authority, users, vaults, offset, input_first) = match protocol {
        Protocol::GoonfiV2 if tag == 1 && matches!(data.len(), 18 | 19) && data[1] <= 1 => (1, 0, [2, 3], [4, 5], 2, false),
        Protocol::Bisonfi if (tag == 2 && data.len() == 18) || (tag == 7 && data.len() == 19) || (tag == 19 && data.len() == 153) => {
            (1, 0, [4, 5], [2, 3], 1, false)
        }
        Protocol::Manifest if tag == 4 && data.len() == 19 && data[17] <= 1 && data[18] <= 1 => (1, 0, [3, 4], [5, 6], 1, false),
        Protocol::Manifest if tag == 13 && data.len() == 19 && data[17] <= 1 && data[18] <= 1 => (2, 1, [4, 5], [6, 7], 1, false),
        Protocol::Humidifi if data.len() == 25 && matches!(data[24], 4 | 15 | 20) && u64::from_le_bytes(data[16..24].try_into().ok()?) <= 1 => {
            (1, 0, [4, 5], [2, 3], 8, false)
        }
        Protocol::Humidifi if (data.len() == 113 && data[112] == 28) || (data.len() == 137 && data[136] == 50) => (1, 0, [4, 5], [2, 3], 0, false),
        Protocol::OrcaV2 if tag == 1 && data.len() == 17 => (0, 2, [3, 6], [4, 5], 1, true),
        Protocol::Alphaq if tag == 12 && data.len() == 18 && data[1] <= 1 => (1, 0, [3, 4], [5, 6], 2, false),
        Protocol::Kipseli if data.len() == 25 && data[..8] == [240, 224, 38, 33, 176, 31, 241, 175] && data[24] <= 1 => (1, 0, [6, 7], [2, 3], 8, false),
        Protocol::Flux if tag == 3 && data.len() == 18 && data[17] <= 1 => (1, 0, [4, 5], [2, 3], 9, false),
        Protocol::Scorch if tag == 2 && data.len() == 34 => (15, 1, [2, 3], [4, 5], 18, true),
        Protocol::Scorch if tag == 1 && data.len() == 34 => (11, 1, [2, 3], [4, 5], 18, true),
        Protocol::Obsidian if tag == 1 && data.len() == 9 => (1, 0, [4, 5], [3, 2], 1, true),
        // Account 0 is shared global state; account 1 is the individual market.
        Protocol::Tesserav if tag == 16 && data.len() == 18 && data[1] <= 1 => (1, 2, [5, 6], [3, 4], 2, false),
        // Tag 85 is a leveraged position change with lending CPIs, not a swap.
        Protocol::Deriverse if tag == 26 && data.len() == 32 && data[1] <= 1 && data[2..4] == [0, 0] => (5, 0, [11, 12], [3, 4], 16, false),
        Protocol::Zerofi if tag == 16 && matches!(data.len(), 17 | 18 | 57) => (0, 8, [6, 7], [3, 5], 1, true),
        _ => return None,
    };
    let count = ix.accounts().len();
    let minimum_accounts = match protocol {
        Protocol::GoonfiV2 => 13,
        Protocol::Bisonfi => 8,
        Protocol::Manifest => {
            if tag == 4 {
                8
            } else {
                9
            }
        }
        Protocol::Humidifi => {
            if data.len() == 25 {
                9
            } else {
                12
            }
        }
        Protocol::OrcaV2 => 10,
        Protocol::Alphaq => 12,
        Protocol::Kipseli => 15,
        Protocol::Flux => 11,
        Protocol::Scorch => {
            if tag == 2 {
                16
            } else {
                12
            }
        }
        Protocol::Obsidian => 8,
        Protocol::Tesserav => 12,
        Protocol::Deriverse => 15,
        Protocol::Zerofi => 14,
        _ => return None,
    };
    if count < minimum_accounts {
        return None;
    }
    if [pool, authority, users[0], users[1], vaults[0], vaults[1]].iter().any(|index| *index >= count) {
        return None;
    }
    let amount = u64::from_le_bytes(data.get(offset..offset + 8)?.try_into().ok()?);
    let exact_input = protocol != Protocol::Manifest || data[18] == 1;
    if amount == 0 && exact_input {
        return None;
    }
    Some(Layout {
        protocol,
        pool,
        authority,
        users,
        vaults,
        amount,
        exact_input,
        input_limit: matches!(protocol, Protocol::Manifest | Protocol::OrcaV2),
        input_first,
    })
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    let layout = layout(ix)?;
    Some(ix.accounts().get(layout.pool)?.0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn finalized_helius_swaps_match_exact_native_pools_and_user_flows() {
        let mut protocols = std::collections::HashSet::new();
        for line in include_str!("../fixtures/native-venues/cases.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/native-venues/{}", env!("CARGO_MANIFEST_DIR"), fields[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            assert_eq!(base58::encode(tx.hash()), fields[1]);
            let decoded = crate::process_transaction(tx);
            let mut actual = Vec::new();
            let mut identities = std::collections::HashSet::new();
            if let Some(decoded) = decoded {
                for swap in &decoded.swaps {
                    assert!(
                        identities.insert((swap.source_index, swap.source_transfer_index)),
                        "duplicate CPI in {}",
                        fields[0]
                    );
                    if !VENUES.iter().any(|(p, _)| *p as i32 == swap.protocol) {
                        continue;
                    }
                    assert!(swap.transfer_verified);
                    protocols.insert(swap.protocol);
                    actual.push(format!(
                        "{}:{}:{}:{}:{}:{}:{}:{}",
                        swap.protocol,
                        swap.source_index.unwrap(),
                        base58::encode(&swap.amm_pool),
                        base58::encode(&swap.user),
                        base58::encode(&swap.input_mint),
                        swap.input_amount,
                        base58::encode(&swap.output_mint),
                        swap.output_amount
                    ));
                }
            }
            assert_eq!(actual.join("|"), fields[3], "Helius mismatch in {}", fields[0]);
        }
        assert_eq!(protocols.len(), VENUES.len(), "every requested venue requires a real validated swap");
    }

    #[test]
    fn malformed_native_layouts_and_unknown_programs_are_not_swaps() {
        let (_, program) = VENUES[0];
        let accounts: Vec<_> = (1..=13).map(|i| [i; 32]).collect();
        let mut data = vec![1, 0];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        for (program, count, data) in [(program, 13, vec![1]), ([44; 32], 13, data.clone()), (program, 4, data.clone())] {
            let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts[..count], data);
            assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
        }
        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
        assert!(layout(&tx.walk_instructions().next().unwrap()).is_some());
        // A recognized swap without token CPIs can never publish a price.
        assert!(crate::process_transaction(tx).is_none());
    }

    fn fee_fixture(explicit_fee: bool) -> ConfirmedTransaction {
        use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions, TokenBalance, UiTokenAmount};
        use substreams_solana_idls::spl::{token, token_2022};
        let (_, program) = VENUES[0];
        let mut accounts: Vec<_> = (1..=13).map(|i| [i; 32]).collect();
        accounts[11] = token::PROGRAM_ID;
        accounts[12] = token_2022::PROGRAM_ID;
        let mut data = vec![1, 0];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        let mut tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
        let mut debit = vec![3];
        debit.extend_from_slice(&100u64.to_le_bytes());
        let mut receipt = if explicit_fee { vec![26, 1] } else { vec![12] };
        receipt.extend_from_slice(&200u64.to_le_bytes());
        receipt.push(6);
        if explicit_fee {
            receipt.extend_from_slice(&6u64.to_le_bytes());
        }
        let meta = tx.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![
                InnerInstruction {
                    program_id_index: 13,
                    accounts: vec![4, 6, 2],
                    data: debit,
                    stack_height: Some(2),
                },
                InnerInstruction {
                    program_id_index: 14,
                    accounts: vec![7, 9, 5, 3],
                    data: receipt,
                    stack_height: Some(2),
                },
            ],
        }];
        for (balances, source, vault_in, vault_out, output) in [
            (&mut meta.pre_token_balances, 100, 0, 1000, 0),
            (&mut meta.post_token_balances, 0, 100, 800, 194),
        ] {
            for (index, mint, amount) in [
                (4, accounts[6], source),
                (6, accounts[6], vault_in),
                (7, accounts[7], vault_out),
                (5, accounts[7], output),
            ] {
                balances.push(TokenBalance {
                    account_index: index,
                    mint: base58::encode(mint),
                    ui_token_amount: Some(UiTokenAmount {
                        amount: amount.to_string(),
                        ..Default::default()
                    }),
                    ..Default::default()
                });
            }
        }
        tx
    }

    #[test]
    fn fee_outputs_use_proven_receipts_and_reject_ambiguous_net_amounts() {
        for explicit_fee in [false, true] {
            let decoded = crate::process_transaction(fee_fixture(explicit_fee)).unwrap();
            assert_eq!(decoded.swaps.len(), 1);
            assert_eq!((decoded.swaps[0].input_amount, decoded.swaps[0].output_amount), (100, 194));
        }
        let mut unknown_receipt = fee_fixture(false);
        let meta = unknown_receipt.meta.as_mut().unwrap();
        meta.pre_token_balances.retain(|b| b.account_index != 5);
        meta.post_token_balances.retain(|b| b.account_index != 5);
        // Checked transfer proves the mint, but cannot prove an implicit fee
        // without the recipient balance. Do not publish the gross output.
        assert!(crate::process_transaction(unknown_receipt).is_none());
    }

    #[test]
    fn wrong_vault_authority_amount_depth_and_failed_transactions_emit_nothing() {
        for field in 0..9 {
            let mut tx = fee_fixture(true);
            let meta = tx.meta.as_mut().unwrap();
            match field {
                0 => meta.inner_instructions[0].instructions[0].accounts[1] = 7,
                1 => meta.inner_instructions[0].instructions[0].accounts[2] = 3,
                2 => meta.inner_instructions[0].instructions[0].data[1] += 1,
                3 => meta.inner_instructions[0].instructions[1].stack_height = None,
                4 => {
                    meta.pre_token_balances.clear();
                    meta.post_token_balances.clear();
                }
                5 => meta.inner_instructions[0].instructions[1].data[11..19].copy_from_slice(&201u64.to_le_bytes()),
                6 => meta.err = Some(Default::default()),
                7 => meta.inner_instructions[0].instructions[1].data[11..19].copy_from_slice(&200u64.to_le_bytes()),
                _ => {
                    let duplicate = meta.inner_instructions[0].instructions[0].clone();
                    meta.inner_instructions[0].instructions.insert(1, duplicate);
                }
            }
            assert!(crate::process_transaction(tx).is_none(), "invalid evidence {field}");
        }
    }
}
