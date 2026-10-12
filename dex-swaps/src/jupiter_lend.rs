//! Jupiter Lend AMM's owned Anchor IDL, with liquidity settlement verified by
//! the quality pass. A router's receipt is not evidence of a native price.
use substreams_solana::{b58, block_view::InstructionView};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("jupZ4m2GqUCJ5iueMfzQf8khFfH31d4XAQt3RzCT9Vd");
pub(crate) const LIQUIDITY_PROGRAM: [u8; 32] = b58!("jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC");
pub(crate) const ORACLE_PROGRAM: [u8; 32] = b58!("jupnw4B6Eqs7ft6rxpzYLJZYSnrpRgPcr589n5Kv4oc");
const ASSOCIATED_TOKEN_PROGRAM: [u8; 32] = b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub(crate) const SWAP_IN: [u8; 8] = [141, 172, 10, 208, 69, 9, 56, 154];
pub(crate) const SWAP_OUT: [u8; 8] = [206, 36, 149, 14, 163, 132, 148, 1];
pub(crate) const GET_CENTER_PRICE: [u8; 8] = [60, 51, 11, 241, 151, 180, 192, 27];
pub(crate) const PRE_OPERATE: [u8; 8] = [129, 205, 158, 155, 198, 155, 72, 133];
pub(crate) const OPERATE: [u8; 8] = [217, 106, 208, 99, 116, 151, 42, 135];

pub(crate) struct Swap {
    pub input_token: usize,
    pub output_token: usize,
    pub recipient: usize,
    pub destination: usize,
    pub amount: u64,
    pub threshold: u64,
    pub exact_input: bool,
}

pub(crate) fn layout(ix: &InstructionView) -> Option<Swap> {
    let a = ix.accounts();
    let data = ix.data();
    if ix.program_id().0.as_slice() != PROGRAM_ID
        || data.len() != 25
        || ![24, 25, 26, 27].contains(&a.len())
        || data[8] > 1
        || a[20].0.as_slice() != LIQUIDITY_PROGRAM
        || a[23].0.as_slice() != ORACLE_PROGRAM
        || a.len() == 25 && a[24].0.as_slice() != ASSOCIATED_TOKEN_PROGRAM
        // The observed 26/27-account ABI omits the ATA program and passes
        // oracle state plus its sources. The committed oracle CPI must bind
        // this complete tail before settlement can produce a price.
        || a.len() > 25 && a[24..].iter().any(|tail| {
            tail.0.as_slice() == PROGRAM_ID || tail.0.as_slice() == ASSOCIATED_TOKEN_PROGRAM
                || tail.0.as_slice() == ORACLE_PROGRAM || tail.0.as_slice() == LIQUIDITY_PROGRAM
        })
        || a[21..23].iter().any(|program| {
            ![
                substreams_solana_idls::spl::token::PROGRAM_ID.as_slice(),
                substreams_solana_idls::spl::token_2022::PROGRAM_ID.as_slice(),
            ]
            .contains(&program.0.as_slice())
        })
        || a[7].0 == a[8].0
        || a[2].0 == a[3].0
        || a[13].0 == a[14].0
        || a[1].0 == a[0].0
        || a[13..15].iter().any(|vault| a[2..4].iter().any(|user| user.0 == vault.0) || vault.0 == a[1].0)
    {
        return None;
    }
    let exact_input = if data.starts_with(&SWAP_IN) {
        true
    } else if data.starts_with(&SWAP_OUT) {
        false
    } else {
        return None;
    };
    let amount = u64::from_le_bytes(data[9..17].try_into().ok()?);
    let threshold = u64::from_le_bytes(data[17..25].try_into().ok()?);
    if amount == 0 || !exact_input && threshold == 0 {
        return None;
    }
    let input_token = if data[8] == 1 { 0 } else { 1 };
    let output_token = 1 - input_token;
    // Anchor uses the called program's key for omitted optional accounts.
    let recipient = if a[4].0.as_slice() == PROGRAM_ID { 0 } else { 4 };
    let destination = if a[5 + output_token].0.as_slice() == PROGRAM_ID {
        2 + output_token
    } else {
        5 + output_token
    };
    if a[destination].0 == a[2 + input_token].0 || a[13..15].iter().any(|vault| vault.0 == a[destination].0) {
        return None;
    }
    Some(Swap {
        input_token,
        output_token,
        recipient,
        destination,
        amount,
        threshold,
        exact_input,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::pb::dex::swaps::v1::Protocol;
    use substreams_solana::{
        base58,
        pb::sf::solana::r#type::v1::{CompiledInstruction, ConfirmedTransaction, InnerInstructions},
    };

    #[test]
    fn finalized_both_directions_bind_native_debits_nested_receipts_and_liquidity_roles() {
        for (fixture, source, input, output) in [
            (
                include_bytes!("../fixtures/swap-coverage/dflow-native-gaps/jupz.pb").as_slice(),
                3u32,
                2_017_780u64,
                2_015_842u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend/3keUmDBNeVKq5eW7aHZ7oGBT5WEoZ8Z5QhWVBWP1TuyQDTkfnJxRGi1oPe2etrBjJCvfdRMcsktYrrszJbyLCf4a.pb"
                )
                .as_slice(),
                10,
                195_627_422,
                195_781_792,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend/3XRub1RmV2zmwDkSoaQrHnjZ4SUnxR6c4w1C7zTTappADTKexCejtcBZ8TRyqqC6sx4ZYm9xRBAfQsTnhiGV5T2f.pb"
                )
                .as_slice(),
                18,
                5_503_203,
                5_498_849,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let original = tx.walk_instructions().nth(source as usize).unwrap();
            let native_data = original.data().clone();
            let a: Vec<_> = original.accounts().iter().map(|a| a.0.clone()).collect();
            let side = if native_data[8] == 1 { 0 } else { 1 };
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.amm.as_slice() == PROGRAM_ID).collect();
            assert_eq!(swaps.len(), 1);
            let swap = swaps[0];
            assert_eq!(swap.protocol, Protocol::JupiterLend as i32);
            assert_eq!(swap.amm_pool, a[1]);
            assert_eq!(swap.user, a[0]);
            assert_eq!(swap.input_mint, a[7 + side]);
            assert_eq!(swap.output_mint, a[8 - side]);
            assert_eq!((swap.input_amount, swap.output_amount), (input, output));
            assert_eq!(swap.source_index, Some(source));
            assert_eq!(swap.source_transfer_index, Some(source + 2));
            assert!(swap.transfer_verified);
            assert!(decoded.coverage_candidates.iter().all(|c| c.source_index != source));

            // Exact-output/current-25/optional-account shapes follow the owned
            // IDL. These synthetic checks do not claim finalized positives.
            for mode in 0..4 {
                let mut shape = tx.clone();
                let resolved: Vec<_> = tx.resolved_accounts().iter().map(|a| (*a).clone()).collect();
                shape.transaction.as_mut().unwrap().message.as_mut().unwrap().account_keys = resolved;
                let meta = shape.meta.as_mut().unwrap();
                meta.loaded_writable_addresses.clear();
                meta.loaded_readonly_addresses.clear();
                let group = meta
                    .inner_instructions
                    .iter_mut()
                    .find(|g| g.instructions.iter().any(|ix| ix.data == native_data))
                    .unwrap();
                let position = group.instructions.iter().position(|ix| ix.data == native_data).unwrap();
                let native = group.instructions[position].clone();
                let liq = group.instructions[position + 1].program_id_index as u8;
                match mode {
                    0 => {
                        group.instructions[position].data[..8].copy_from_slice(&SWAP_OUT);
                        group.instructions[position].data[9..17].copy_from_slice(&output.to_le_bytes());
                        group.instructions[position].data[17..25].copy_from_slice(&input.to_le_bytes());
                    }
                    1 => {
                        let keys = &mut shape.transaction.as_mut().unwrap().message.as_mut().unwrap().account_keys;
                        let index = u8::try_from(keys.len()).unwrap();
                        keys.push(ASSOCIATED_TOKEN_PROGRAM.to_vec());
                        group.instructions[position].accounts.push(index);
                    }
                    2 => {
                        for role in 4..7 {
                            group.instructions[position].accounts[role] = native.program_id_index as u8;
                        }
                    }
                    _ => {
                        for role in 15..19 {
                            group.instructions[position].accounts[role] = native.program_id_index as u8;
                        }
                        group.instructions[position + 1].accounts[2..4].fill(liq);
                        for offset in [3, 4] {
                            group.instructions[position + offset].accounts[5..7].fill(liq);
                        }
                    }
                }
                let result = crate::process_transaction(shape).unwrap();
                let changed: Vec<_> = result.swaps.iter().filter(|s| s.amm.as_slice() == PROGRAM_ID).collect();
                assert_eq!(changed.len(), 1, "positive IDL shape {mode}");
                assert_eq!((changed[0].input_amount, changed[0].output_amount), (input, output));
            }

            // Classic-only standalone calls must also run the success scanner.
            let mut direct = tx.clone();
            let resolved: Vec<_> = tx.resolved_accounts().iter().map(|a| (*a).clone()).collect();
            let message = direct.transaction.as_mut().unwrap().message.as_mut().unwrap();
            message.account_keys = resolved;
            let group = tx
                .meta
                .as_ref()
                .unwrap()
                .inner_instructions
                .iter()
                .find(|g| g.instructions.iter().any(|ix| ix.data == native_data))
                .unwrap();
            let position = group.instructions.iter().position(|ix| ix.data == native_data).unwrap();
            let native = &group.instructions[position];
            message.instructions = vec![CompiledInstruction {
                program_id_index: native.program_id_index,
                accounts: native.accounts.clone(),
                data: native.data.clone(),
            }];
            let mut calls = group.instructions[position + 1..position + 6].to_vec();
            for (offset, child) in calls.iter_mut().enumerate() {
                child.stack_height = Some(if offset == 4 { 3 } else { 2 });
            }
            let meta = direct.meta.as_mut().unwrap();
            meta.loaded_writable_addresses.clear();
            meta.loaded_readonly_addresses.clear();
            meta.inner_instructions = vec![InnerInstructions { index: 0, instructions: calls }];
            let p = base58::encode(PROGRAM_ID);
            let l = base58::encode(LIQUIDITY_PROGRAM);
            let t = base58::encode(substreams_solana_idls::spl::token::PROGRAM_ID);
            meta.log_messages = vec![
                format!("Program {p} invoke [1]"),
                format!("Program {l} invoke [2]"),
                format!("Program {l} success"),
                format!("Program {t} invoke [2]"),
                format!("Program {t} success"),
                format!("Program {l} invoke [2]"),
                format!("Program {l} success"),
                format!("Program {l} invoke [2]"),
                format!("Program {t} invoke [3]"),
                format!("Program {t} success"),
                format!("Program {l} success"),
                format!("Program {p} success"),
            ];
            let result = crate::process_transaction(direct).unwrap();
            assert_eq!(result.swaps.len(), 1);
            assert_eq!((result.swaps[0].input_amount, result.swaps[0].output_amount), (input, output));
            assert_eq!(result.swaps[0].source_index, Some(0));

            for mode in 0..46 {
                let mut wrong = tx.clone();
                let meta = wrong.meta.as_mut().unwrap();
                let group = meta
                    .inner_instructions
                    .iter_mut()
                    .find(|g| g.instructions.iter().any(|ix| ix.data == native_data))
                    .unwrap();
                let position = group.instructions.iter().position(|ix| ix.data == native_data).unwrap();
                let calls = &mut group.instructions;
                let accounts = calls[position].accounts.clone();
                match mode {
                    0 => calls[position].data[0] ^= 1,
                    1 => {
                        calls[position].data.pop();
                    }
                    2 => calls[position].data.push(0),
                    3 => calls[position].data[8] = 2,
                    4 => calls[position].data[8] ^= 1,
                    5 => calls[position].data[9..17].copy_from_slice(&(input + 1).to_le_bytes()),
                    6 => calls[position].data[17..25].copy_from_slice(&(output + 1).to_le_bytes()),
                    7..=10 => calls[position].accounts[20 + mode - 7] = accounts[0],
                    11 => calls[position + 1].data[8..40].fill(0),
                    12 => calls[position + 1].accounts[0] = accounts[0],
                    13 => calls[position + 1].accounts[2] = accounts[0],
                    14 => calls[position + 1].accounts[4] = accounts[0],
                    15 => calls[position + 2].accounts[0] = accounts[1],
                    16 => calls[position + 2].accounts[2] = accounts[0],
                    17 => calls[position + 2].accounts[3] = accounts[1],
                    18 => calls[position + 2].data[1..9].copy_from_slice(&(input + 1).to_le_bytes()),
                    19 => calls[position + 3].data[24..40].copy_from_slice(&0i128.to_le_bytes()),
                    20 => calls[position + 3].data[24..40].copy_from_slice(&1i128.to_le_bytes()),
                    21 => calls[position + 3].data[24..40].copy_from_slice(&(-i128::from(input) - 1).to_le_bytes()),
                    22 => calls[position + 3].data[24..40].copy_from_slice(&i128::MIN.to_le_bytes()),
                    23 => calls[position + 3].data[8..24].copy_from_slice(&1i128.to_le_bytes()),
                    24..=29 => calls[position + 3].accounts[2 + mode - 24] = accounts[0],
                    30 => calls[position + 3].accounts[10] = accounts[0],
                    31 => calls[position + 3].accounts[8] = accounts[0],
                    32 => calls[position + 3].data[40..72].fill(0),
                    33 => calls[position + 4].data[24..40].copy_from_slice(&(i128::from(output) + 1).to_le_bytes()),
                    34 => calls[position + 4].accounts[8] = accounts[0],
                    35 => calls[position + 4].accounts[9] = accounts[0],
                    36 => calls[position + 4].data[104] = 2,
                    37 => calls[position + 5].accounts[0] = accounts[0],
                    38 => calls[position + 5].accounts[2] = accounts[0],
                    39 => calls[position + 5].accounts[3] = accounts[0],
                    40 => calls[position + 5].stack_height = Some(original.stack_height() + 1),
                    41 => {
                        calls.remove(position + 5);
                    }
                    42 => {
                        let extra = calls[position + 5].clone();
                        calls.insert(position + 6, extra);
                    }
                    43 => {
                        let own = format!("Program {} success", base58::encode(PROGRAM_ID));
                        let end = meta.log_messages.iter().position(|log| log == &own).unwrap();
                        meta.log_messages.truncate(end);
                    }
                    44 => {
                        let parent = tx
                            .walk_instructions()
                            .take(source as usize)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .find(|ix| {
                                if ix.is_root() {
                                    original.stack_height() == 2
                                } else {
                                    ix.stack_height() + 1 == original.stack_height()
                                }
                            })
                            .unwrap()
                            .program_id()
                            .0
                            .clone();
                        let success = format!("Program {} success", base58::encode(parent));
                        let end = meta.log_messages.iter().rposition(|log| log == &success).unwrap();
                        meta.log_messages[end] = success.replace(" success", " failed: custom program error: 1");
                    }
                    _ => calls[position + 4].data[..8].copy_from_slice(&PRE_OPERATE),
                }
                let result = crate::process_transaction(wrong).unwrap();
                assert!(
                    result.swaps.iter().all(|s| s.amm.as_slice() != PROGRAM_ID),
                    "source {source}, invalid settlement {mode}"
                );
                if matches!(mode, 1 | 2 | 3 | 7..=10) {
                    assert!(
                        result
                            .coverage_candidates
                            .iter()
                            .any(|c| c.source_index == source && c.program_id.as_slice() == PROGRAM_ID),
                        "retain unsupported Jupiter Lend shape {mode}"
                    );
                    assert!(!result.coverage_transaction.is_empty());
                }
            }
        }
    }

    #[test]
    fn finalized_oracle_tails_and_signed_supply_settlement_bind_exact_receipts() {
        for (fixture, source, input, output) in [
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/46bAEwM4U9uyYvqu1WFTLJy1GM4vFNvyCedEJ6ZtAQxx44CsuDDPA66jGXsUwwWkr8yUDsiVHXbvbkdziWEzGRbo.pb"
                )
                .as_slice(),
                16u32,
                184246890u64,
                141176134u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/423VH6ahp3STeimJwVUUZXq43zJnbzeysnJM5mCnJdYgGt54aNffHEF5RQErFChMYGCKWDsk6mdmjAZo6qRkphbi.pb"
                )
                .as_slice(),
                8u32,
                251712956u64,
                192871019u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/3vvmUkgpNhw5mg1HTH2wn9nfgsKFUUjD5U2tYZgX6yeLfSs3D43qcAikojiAaHU9yH3x7i8MQ9mx4pLZkhnsYhyf.pb"
                )
                .as_slice(),
                8u32,
                7634582u64,
                5849878u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/3CHRNKj9TneShPsrLrAcuoKSicN4zaAXzPyyY8EWpGxp3CEByVQ3hrMkx7wDcDvrJt6EjwLi1rbmKvBVrFztfaMV.pb"
                )
                .as_slice(),
                3u32,
                2400000000u64,
                2732273316u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/32GcCdtYNdtQATHs3yV4qQnabAirUd53xGnJss5JsMCogA1D1dGry8p2GNKaWunZwu4VKLMyP9tehgYGNHZS2rXf.pb"
                )
                .as_slice(),
                11u32,
                12343750u64,
                9458204u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/66daBVKn2dkJr2eK3m9rk8wVvzbNq3iaELURMpKKWc6zn5gPjf99NtzUBUL827t2P7cmgbnq4bQ4KP55xz42tp8r.pb"
                )
                .as_slice(),
                3u32,
                2048653738u64,
                2332279174u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/5ZwoLDAJfBtqWak5tAXRoNgGXkZ8mkJmFNFt8jxEwQ82B2vqGobEwNyg6J5wpgkSZAFet5xoBu3T63pcWQwRGWhB.pb"
                )
                .as_slice(),
                10u32,
                43481725u64,
                33317191u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/TWfm9d8HzN1UV1QV9Dvm9wCD7yJg1PchPZJfzkfAVxrbG9pubMVtKh3McTXLiwZNwtCctipFbXDf4KS7dcQrh7Q.pb"
                )
                .as_slice(),
                10u32,
                41066906u64,
                31466878u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/3bbqUuD2pjc2LJRbbMfxgDQGDoS5rC3rNW73m6kw4BjieS4gQoBus6ouG5RtVrGRe7FUtkYQsfcpfgAAr2w7MAu4.pb"
                )
                .as_slice(),
                8u32,
                7638026u64,
                5852519u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/38FPQwfKMdRCRCybx7aZe54d8GbSZExqaHf7bdDu8y9scjjtnkohvHU9qptPcqQuZcE53itP91szijywzhSjqJG.pb"
                )
                .as_slice(),
                13u32,
                490039698u64,
                557883781u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/YQd257hjLfV6kfq6c5gHigngKTQRkhPDfwA2zcuwAZN8HVCtt6WwufQs4BigjUPakQGq8TDNkn9tMrknfLe7Qax.pb"
                )
                .as_slice(),
                11u32,
                9110057u64,
                6980439u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/jupiter-lend-oracle/5Ezg6q4VkDRQvu8XyKySuvxtaPC3haVHkaUbC7ijnvjcjNbcVnQbfD8bBZ5rjXHabW95wpfC721MAzEs5vgpYEyV.pb"
                )
                .as_slice(),
                9u32,
                2332746735u64,
                2048653738u64,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let original = tx.walk_instructions().nth(source as usize).unwrap();
            let native_data = original.data().clone();
            let a: Vec<_> = original.accounts().iter().map(|a| a.0.clone()).collect();
            let side = if native_data[8] == 1 { 0 } else { 1 };
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.amm.as_slice() == PROGRAM_ID).collect();
            assert_eq!(swaps.len(), 1, "source {source}");
            let swap = swaps[0];
            assert_eq!(swap.protocol, Protocol::JupiterLend as i32);
            assert_eq!(swap.amm_pool, a[1]);
            assert_eq!(swap.user, a[0]);
            assert_eq!(swap.input_mint, a[7 + side]);
            assert_eq!(swap.output_mint, a[8 - side]);
            assert_eq!((swap.input_amount, swap.output_amount), (input, output));
            assert_eq!(swap.source_index, Some(source));
            assert_eq!(swap.source_transfer_index, Some(source + 3));
            assert!(swap.transfer_verified);
            assert!(decoded.coverage_candidates.iter().all(|c| c.source_index != source));
            for mode in 0..20 {
                let mut wrong = tx.clone();
                let meta = wrong.meta.as_mut().unwrap();
                let group = meta
                    .inner_instructions
                    .iter_mut()
                    .find(|g| g.instructions.iter().any(|ix| ix.data == native_data))
                    .unwrap();
                let pos = group.instructions.iter().position(|ix| ix.data == native_data).unwrap();
                let calls = &mut group.instructions;
                let native = calls[pos].clone();
                match mode {
                    0 => calls[pos + 1].data[0] ^= 1,
                    1 => {
                        calls[pos + 1].data.pop();
                    }
                    2 => calls[pos + 1].data.push(0),
                    3 => calls[pos + 1].accounts[0] = native.accounts[0],
                    4 => calls[pos + 1].accounts[1] = native.accounts[0],
                    5 => calls[pos + 1].program_id_index = native.program_id_index,
                    6 => calls[pos + 1].stack_height = Some(original.stack_height() + 2),
                    7 => {
                        calls.remove(pos + 1);
                    }
                    8 => calls[pos].accounts.push(native.accounts[0]),
                    9 => calls[pos + 4].data[8..24].copy_from_slice(&0i128.to_le_bytes()),
                    10 => calls[pos + 4].data[8..24].copy_from_slice(&(-1i128).to_le_bytes()),
                    11 => calls[pos + 4].data[8..24].copy_from_slice(&(i128::from(input) + 1).to_le_bytes()),
                    12 => calls[pos + 4].data[24..40].copy_from_slice(&(-1i128).to_le_bytes()),
                    13 => calls[pos + 5].data[8..24].copy_from_slice(&0i128.to_le_bytes()),
                    14 => calls[pos + 5].data[8..24].copy_from_slice(&i128::from(output).to_le_bytes()),
                    15 => calls[pos + 5].data[8..24].copy_from_slice(&(-i128::from(output) - 1).to_le_bytes()),
                    16 => calls[pos + 5].data[24..40].copy_from_slice(&1i128.to_le_bytes()),
                    17 => calls[pos + 6].accounts[2] = native.accounts[0],
                    18 => {
                        let own = format!("Program {} success", base58::encode(ORACLE_PROGRAM));
                        let end = meta.log_messages.iter().position(|l| l == &own).unwrap();
                        meta.log_messages[end] = own.replace(" success", " failed: custom program error: 1");
                    }
                    _ => {
                        let own = format!("Program {} success", base58::encode(PROGRAM_ID));
                        let end = meta.log_messages.iter().position(|l| l == &own).unwrap();
                        meta.log_messages[end] = own.replace(" success", " failed: custom program error: 1");
                    }
                }
                let result = crate::process_transaction(wrong).unwrap();
                assert!(
                    result.swaps.iter().all(|s| s.amm.as_slice() != PROGRAM_ID),
                    "source {source}, invalid oracle/supply settlement {mode}"
                );
            }
        }
    }
}
