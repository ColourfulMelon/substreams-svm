use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::{
    spl::token_swap::{self, accounts},
    spl::token_swap::instructions::{self, TokenSwapInstruction},
};

use crate::token_mints::TokenMintLookup;

/// Canonical SPL Token Swap program plus known forks that reuse the exact
/// instruction layout. Adapter dispatch is gated to this allowlist because the
/// SPL Token Swap `Swap` discriminator (`0x01`) is single-byte and the payload
/// length matches many unrelated programs; without a gate, dispatch matched
/// any 17-byte instruction starting with `0x01` (cf. substreams-svm#208).
const ALLOWED_PROGRAM_IDS: &[[u8; 32]] = &[token_swap::PROGRAM_ID];

pub(crate) fn handle_instruction(ix: &InstructionView, token_mints: &TokenMintLookup) -> Option<pb::Swap> {
    let program_id = ix.program_id().0;
    if !ALLOWED_PROGRAM_IDS.iter().any(|allowed| allowed.as_slice() == program_id.as_slice()) {
        return None;
    }
    let TokenSwapInstruction::Swap {
        amount_in,
        minimum_amount_out,
    } = instructions::unpack(ix.data()).ok()? else {
        return None;
    };

    if amount_in == 0 {
        return None;
    }

    let accounts = accounts::get_swap_accounts(ix).ok()?;
    let input_mint = token_mints.mint_for(accounts.source.as_ref())?;
    let output_mint = token_mints.mint_for(accounts.destination.as_ref())?;

    if input_mint == output_mint {
        return None;
    }

    Some(pb::Swap {
        protocol: pb::Protocol::SplTokenSwap as i32,
        program_id: program_id.to_vec(),
        stack_height: ix.stack_height(),
        amm: program_id.to_vec(),
        amm_pool: accounts.swap_account.to_bytes().to_vec(),
        user: accounts.user_transfer_authority.to_bytes().to_vec(),
        input_mint,
        input_amount: amount_in,
        output_mint,
        output_amount: minimum_amount_out,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use substreams_solana::{
        base58,
        pb::sf::solana::r#type::v1::{TokenBalance, TransactionStatusMeta, UiTokenAmount},
    };
    use substreams_solana_idls::spl::token_swap;

    use super::*;
    use crate::routed_pool::test_fixture::make_tx;

    const PROGRAM: [u8; 32] = [0x99; 32];
    const SWAP_ACCOUNT: [u8; 32] = [1; 32];
    const AUTHORITY: [u8; 32] = [2; 32];
    const USER: [u8; 32] = [3; 32];
    const SOURCE: [u8; 32] = [4; 32];
    const SWAP_SOURCE: [u8; 32] = [5; 32];
    const SWAP_DESTINATION: [u8; 32] = [6; 32];
    const DESTINATION: [u8; 32] = [7; 32];
    const POOL_MINT: [u8; 32] = [8; 32];
    const FEE_ACCOUNT: [u8; 32] = [9; 32];
    const INPUT_MINT: [u8; 32] = [10; 32];
    const OUTPUT_MINT: [u8; 32] = [11; 32];

    fn swap_data(amount_in: u64, minimum_amount_out: u64) -> Vec<u8> {
        let mut data = vec![instructions::SWAP];
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&minimum_amount_out.to_le_bytes());
        data
    }

    fn token_balance(account_index: u32, mint: [u8; 32], amount: u64) -> TokenBalance {
        TokenBalance {
            account_index,
            mint: base58::encode(mint),
            ui_token_amount: Some(UiTokenAmount {
                amount: amount.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn token_mints() -> TokenMintLookup {
        let tx = make_tx(
            PROGRAM,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let mut meta = TransactionStatusMeta::default();
        meta.pre_token_balances = vec![
            token_balance(5, INPUT_MINT, 1_000),
            token_balance(8, OUTPUT_MINT, 100),
        ];
        meta.post_token_balances = vec![
            token_balance(5, INPUT_MINT, 500),
            token_balance(8, OUTPUT_MINT, 100),
        ];
        TokenMintLookup::new(&tx, &meta)
    }

    #[test]
    fn ignores_unrelated_program_with_matching_layout() {
        // Any 17-byte instruction starting with 0x01 used to match. Now gated to
        // the canonical SPL Token Swap program ID; an arbitrary program_id with
        // an identical-looking payload must not emit.
        let tx = make_tx(
            PROGRAM,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = token_mints();

        assert!(handle_instruction(&instruction, &token_mints).is_none());
    }

    #[test]
    fn ignores_bad_token_swap_payloads() {
        let tx = make_tx(token_swap::PROGRAM_ID, &[SWAP_ACCOUNT, AUTHORITY, USER], vec![instructions::SWAP]);
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = token_mints();

        assert!(handle_instruction(&instruction, &token_mints).is_none());
    }

    #[test]
    fn ignores_missing_token_mints() {
        let tx = make_tx(
            token_swap::PROGRAM_ID,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = TokenMintLookup::new(&tx, &TransactionStatusMeta::default());

        assert!(handle_instruction(&instruction, &token_mints).is_none());
    }

    #[test]
    fn emits_spl_token_swap_for_canonical_program() {
        let tx = make_tx(
            token_swap::PROGRAM_ID,
            &[
                SWAP_ACCOUNT,
                AUTHORITY,
                USER,
                SOURCE,
                SWAP_SOURCE,
                SWAP_DESTINATION,
                DESTINATION,
                POOL_MINT,
                FEE_ACCOUNT,
            ],
            swap_data(500, 450),
        );
        let instruction = tx.walk_instructions().next().unwrap();
        let token_mints = token_mints();

        let swap = handle_instruction(&instruction, &token_mints).expect("spl token swap");

        assert_eq!(swap.protocol, pb::Protocol::SplTokenSwap as i32);
        assert_eq!(swap.program_id, token_swap::PROGRAM_ID.to_vec());
        assert_eq!(swap.amm, token_swap::PROGRAM_ID.to_vec());
        assert_eq!(swap.amm_pool, SWAP_ACCOUNT.to_vec());
        assert_eq!(swap.user, USER.to_vec());
        assert_eq!(swap.input_mint, INPUT_MINT.to_vec());
        assert_eq!(swap.input_amount, 500);
        assert_eq!(swap.output_mint, OUTPUT_MINT.to_vec());
        assert_eq!(swap.output_amount, 450);
    }

    #[test]
    fn finalized_swaps_use_actual_user_receipts_and_only_declared_lp_fee_mints() {
        use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
        let expected: Vec<_> = include_str!("../fixtures/native-venues/spl-swap-legs.tsv").lines().collect();
        for (case, fixture) in [
            include_bytes!("../fixtures/native-venues/spl-swap-route-1.transaction.pb").as_slice(),
            include_bytes!("../fixtures/native-venues/spl-swap-route-2.transaction.pb").as_slice(),
            include_bytes!("../fixtures/native-venues/spl-swap-route-3.transaction.pb").as_slice(),
        ]
        .into_iter()
        .enumerate()
        {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let row: Vec<_> = expected[case].split('\t').collect();
            let index = row[1].parse::<u32>().unwrap();
            let native_data = tx.walk_instructions().nth(index as usize).unwrap().data().clone();
            let input = row[6].parse::<u64>().unwrap();
            let output = row[8].parse::<u64>().unwrap();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(index)).collect();
            assert_eq!(swaps.len(), 1);
            let swap = swaps[0];
            assert_eq!(swap.protocol, pb::Protocol::SplTokenSwap as i32);
            assert_eq!(swap.amm, token_swap::PROGRAM_ID);
            assert_eq!(swap.source_transfer_index, Some(index + 1));
            assert_eq!(base58::encode(&swap.amm_pool), row[3]);
            assert_eq!(base58::encode(&swap.user), row[4]);
            assert_eq!(base58::encode(&swap.input_mint), row[5]);
            assert_eq!(base58::encode(&swap.output_mint), row[7]);
            assert_eq!((swap.input_amount, swap.output_amount), (input, output));
            assert!(swap.transfer_verified);
            assert!(decoded.coverage_candidates.iter().all(|candidate| candidate.source_index != index));
            for mode in 0..33 {
                let mut altered = tx.clone();
                let meta = altered.meta.as_mut().unwrap();
                let (group, position) = meta
                    .inner_instructions
                    .iter()
                    .enumerate()
                    .find_map(|(g, group)| group.instructions.iter().position(|ix| ix.data == native_data).map(|p| (g, p)))
                    .unwrap();
                let calls = &mut meta.inner_instructions[group].instructions;
                let a = calls[position].accounts.clone();
                match mode {
                    0 => calls[position].data[1..9].copy_from_slice(&(input - 1).to_le_bytes()),
                    1 => calls[position].data[9..17].copy_from_slice(&(output + 1).to_le_bytes()),
                    2 => calls[position].data[0] = 2,
                    3 => {
                        calls[position].data.pop();
                    }
                    4 => calls[position].data.push(0),
                    5 => calls[position].accounts[9] = a[0],
                    6 => calls[position].accounts[1] = a[0],
                    7 => calls[position].accounts[4] = a[0],
                    8 => calls[position].accounts[2] = a[0],
                    9 => calls[position].accounts[3] = a[0],
                    10 => calls[position].accounts[6] = a[0],
                    11 => calls[position].accounts[7] = a[0],
                    12 => calls[position].accounts[8] = a[0],
                    13 => calls[position + 1].accounts[2] = a[0],
                    14 => calls[position + 3].accounts[2] = a[0],
                    15 => calls[position + 2].accounts[0] = a[0],
                    16 => calls[position + 2].accounts[1] = a[0],
                    17 => calls[position + 2].accounts[2] = a[0],
                    18 => calls[position + 2].data[0] = 8,
                    19 => calls[position + 2].program_id_index = calls[position].program_id_index,
                    20 => {
                        calls.remove(position + 3);
                    }
                    21 => {
                        let extra = calls[position + 3].clone();
                        calls.insert(position + 4, extra);
                    }
                    22 => calls[position + 3].stack_height = Some(4),
                    23 => calls[position + 3].data[1..9].copy_from_slice(&0u64.to_le_bytes()),
                    24 => {
                        let mut extra = calls[position + 2].clone();
                        extra.data[0] = 8;
                        calls.insert(position + 3, extra);
                    }
                    25 => calls.swap(position + 1, position + 3),
                    26..=29 => {
                        let program = base58::encode(token_swap::PROGRAM_ID);
                        let own = format!("Program {program} success");
                        let parent = format!("Program {} success", base58::encode(substreams_solana_idls::jupiter::v4::PROGRAM_ID));
                        let invoke = meta
                            .log_messages
                            .iter()
                            .position(|log| log == &format!("Program {program} invoke [2]"))
                            .unwrap();
                        if mode == 29 {
                            let success = format!("Program {} success", base58::encode(substreams_solana_idls::spl::token::PROGRAM_ID));
                            let fee_end = meta
                                .log_messages
                                .iter()
                                .enumerate()
                                .skip(invoke)
                                .filter(|(_, log)| **log == success)
                                .nth(1)
                                .map(|(i, _)| i)
                                .unwrap();
                            meta.log_messages[fee_end] = success.replace(" success", " failed: custom program error: 1");
                        } else {
                            let end = meta
                                .log_messages
                                .iter()
                                .enumerate()
                                .skip(invoke)
                                .find(|(_, log)| **log == if mode == 28 { parent.clone() } else { own.clone() })
                                .map(|(i, _)| i)
                                .unwrap();
                            if mode == 27 {
                                meta.log_messages.truncate(end);
                            } else {
                                meta.log_messages[end] = meta.log_messages[end].replace(" success", " failed: custom program error: 1");
                            }
                        }
                    }
                    30 => calls[position].accounts.extend_from_slice(&a[..2]),
                    31 => {
                        calls[position].accounts.pop();
                    }
                    _ => {
                        calls[position].accounts[8] = a[6];
                        calls[position + 2].accounts[1] = a[6];
                    }
                }
                assert!(
                    crate::process_transaction(altered).map_or(true, |decoded| {
                        decoded.swaps.iter().all(|swap| swap.amm.as_slice() != token_swap::PROGRAM_ID)
                    }),
                    "SPL case {case}, mutation {mode}"
                );
            }
        }
    }

    #[test]
    fn finalized_no_fee_and_closed_output_variants_keep_exact_receipts() {
        use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
        let rows: Vec<_> = include_str!("../fixtures/native-venues/spl-swap-recent-legs.tsv").lines().collect();
        for (row, fixture) in [
            (0, include_bytes!("../fixtures/native-venues/spl-swap-recent-1.transaction.pb").as_slice()),
            (14, include_bytes!("../fixtures/native-venues/spl-swap-recent-15.transaction.pb").as_slice()),
            (55, include_bytes!("../fixtures/native-venues/spl-swap-recent-56.transaction.pb").as_slice()),
        ] {
            let expected: Vec<_> = rows[row].split('\t').collect();
            let index = expected[1].parse::<u32>().unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            if row == 14 {
                let accounts = tx.resolved_accounts();
                let destination = tx.walk_instructions().nth(index as usize).unwrap().accounts()[6].0.clone();
                let destination_position = accounts.iter().position(|a| **a == destination).unwrap();
                let meta = tx.meta.as_ref().unwrap();
                assert!(!meta.pre_token_balances.iter().chain(&meta.post_token_balances).any(|b| b.account_index == destination_position as u32));
            }
            let decoded = crate::process_transaction(tx).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(index)).collect();
            assert_eq!(swaps.len(), 1);
            let swap = swaps[0];
            assert_eq!(swap.protocol, pb::Protocol::SplTokenSwap as i32);
            assert_eq!(base58::encode(&swap.amm_pool), expected[3]);
            assert_eq!(base58::encode(&swap.user), expected[4]);
            assert_eq!(base58::encode(&swap.input_mint), expected[5]);
            assert_eq!(base58::encode(&swap.output_mint), expected[7]);
            assert_eq!((swap.input_amount, swap.output_amount), (expected[6].parse().unwrap(), expected[8].parse().unwrap()));
            assert!(swap.transfer_verified);
        }
    }
}
