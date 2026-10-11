//! JupiterZ Order Engine fills, pinned to its published production contract.
//! The market identity is the actual maker account, not an invented AMM pool.
use substreams_solana::{b58, block_view::InstructionView};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("61DFfeTKM7trxYcPQCM78bJ794ddZprZpAwAnLiwTpYH");

pub(crate) struct Fill {
    pub input_source: usize,
    pub input_destination: usize,
    pub output_source: usize,
    pub output_destination: usize,
    pub input_amount: u64,
    pub output_amount: u64,
    pub unwrap_input: bool,
    pub unwrap_output: bool,
}

pub(crate) fn layout(ix: &InstructionView) -> Option<Fill> {
    if ix.program_id().0.as_slice() != PROGRAM_ID {
        return None;
    }
    let data = ix.data();
    let accounts = ix.accounts();
    // Anchor consumes the 32-byte typed prefix. The contract deliberately
    // ignores appended off-chain fee/exact-out metadata (35-byte documented / 37-byte observed payloads).
    if data.len() < 32
        || data[..8] != [168, 96, 183, 163, 92, 10, 40, 160]
        || accounts.len() < 11
        || accounts[10].0.iter().any(|byte| *byte != 0)
        || ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&accounts[7].0.as_slice())
        || ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&accounts[9].0.as_slice())
    {
        return None;
    }
    let missing = [2, 3, 4, 5].map(|index| accounts[index].0.as_slice() == PROGRAM_ID);
    // Optional token accounts use the program ID as Anchor's None sentinel.
    // A missing token account is legal only for legacy native SOL settlement.
    if ((missing[0] || missing[1]) && (accounts[6].0.as_slice() != crate::SOL_MINT || accounts[7].0.as_slice() != token::PROGRAM_ID))
        || ((missing[2] || missing[3]) && (accounts[8].0.as_slice() != crate::SOL_MINT || accounts[9].0.as_slice() != token::PROGRAM_ID))
    {
        return None;
    }
    let unwrap_input = !missing[0] && missing[1];
    let unwrap_output = !missing[3] && missing[2];
    if (unwrap_input || unwrap_output) && accounts.len() < 12 {
        return None;
    }
    Some(Fill {
        input_source: if missing[0] { 0 } else { 2 },
        input_destination: if unwrap_input {
            11
        } else if missing[1] {
            1
        } else {
            3
        },
        output_source: if missing[3] || missing[2] { 1 } else { 5 },
        output_destination: if missing[2] { 0 } else { 4 },
        input_amount: u64::from_le_bytes(data[8..16].try_into().ok()?),
        output_amount: u64::from_le_bytes(data[16..24].try_into().ok()?),
        unwrap_input,
        unwrap_output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routed_pool::test_fixture::make_tx;
    use substreams_solana::{
        base58,
        pb::sf::solana::r#type::v1::{ConfirmedTransaction, InnerInstruction, InnerInstructions, TokenBalance, UiTokenAmount},
    };

    fn fill_transaction(present: [bool; 4], input_mint: [u8; 32], output_mint: [u8; 32], checked: [bool; 2]) -> ConfirmedTransaction {
        let mut accounts: Vec<_> = (1..=12).map(|i| [i; 32]).collect();
        for (position, has_token) in present.iter().enumerate() {
            if !has_token {
                accounts[position + 2] = PROGRAM_ID;
            }
        }
        accounts[6] = input_mint;
        accounts[7] = if checked[0] { token_2022::PROGRAM_ID } else { token::PROGRAM_ID };
        accounts[8] = output_mint;
        accounts[9] = if checked[1] { token_2022::PROGRAM_ID } else { token::PROGRAM_ID };
        accounts[10] = [0; 32];
        let mut data = vec![168, 96, 183, 163, 92, 10, 40, 160];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&200u64.to_le_bytes());
        data.extend_from_slice(&2_000_000_000i64.to_le_bytes());
        data.extend_from_slice(&[2, 0, 0, 0, 0]);
        let mut tx = make_tx(PROGRAM_ID, &accounts, data);
        let transfer = |amount: u64, source: u8, destination: u8, authority: u8, input: bool| {
            let checked = checked[usize::from(!input)];
            let mut data = vec![if checked { 12 } else { 3 }];
            data.extend_from_slice(&amount.to_le_bytes());
            let accounts = if checked {
                data.push(6);
                vec![source + 2, if input { 8 } else { 10 }, destination + 2, authority + 2]
            } else {
                vec![source + 2, destination + 2, authority + 2]
            };
            InnerInstruction {
                program_id_index: if input { 9 } else { 11 },
                accounts,
                data,
                stack_height: Some(2),
            }
        };
        let native = |amount: u64, source: u8, destination: u8| {
            let mut data = vec![2, 0, 0, 0];
            data.extend_from_slice(&amount.to_le_bytes());
            InnerInstruction {
                program_id_index: 12,
                accounts: vec![source + 2, destination + 2],
                data,
                stack_height: Some(2),
            }
        };
        let mut children = Vec::new();
        match (present[0], present[1]) {
            (true, true) => children.push(transfer(100, 2, 3, 0, true)),
            (false, false) => children.push(native(100, 0, 1)),
            (false, true) => {
                children.push(native(100, 0, 3));
                children.push(InnerInstruction {
                    program_id_index: 9,
                    accounts: vec![5],
                    data: vec![17],
                    stack_height: Some(2),
                });
            }
            (true, false) => {
                // Rent funding is paid by the maker and is not taker input.
                children.push(native(42, 1, 11));
                let mut data = vec![18];
                data.extend_from_slice(&accounts[1]);
                children.push(InnerInstruction {
                    program_id_index: 9,
                    accounts: vec![13, 8],
                    data,
                    stack_height: Some(2),
                });
                children.push(transfer(100, 2, 11, 0, true));
                children.push(InnerInstruction {
                    program_id_index: 9,
                    accounts: vec![13, 3, 3],
                    data: vec![9],
                    stack_height: Some(2),
                });
            }
        }
        match (present[2], present[3]) {
            (true, true) => children.push(transfer(200, 5, 4, 1, false)),
            (false, false) => children.push(native(200, 1, 0)),
            (true, false) => {
                children.push(native(200, 1, 4));
                children.push(InnerInstruction {
                    program_id_index: 11,
                    accounts: vec![6],
                    data: vec![17],
                    stack_height: Some(2),
                });
            }
            (false, true) => {
                children.push(native(42, 1, 11));
                let mut data = vec![18];
                data.extend_from_slice(&accounts[1]);
                children.push(InnerInstruction {
                    program_id_index: 11,
                    accounts: vec![13, 10],
                    data,
                    stack_height: Some(2),
                });
                children.push(transfer(200, 5, 11, 1, false));
                children.push(InnerInstruction {
                    program_id_index: 11,
                    accounts: vec![13, 3, 3],
                    data: vec![9],
                    stack_height: Some(2),
                });
                children.push(native(200, 1, 0));
            }
        }
        let meta = tx.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: children,
        }];
        for (balances, input_user, input_maker, output_user, output_maker) in
            [(&mut meta.pre_token_balances, 100, 0, 0, 200), (&mut meta.post_token_balances, 0, 100, 200, 0)]
        {
            for (position, amount, mint) in [
                (2, input_user, input_mint),
                (3, input_maker, input_mint),
                (4, output_user, output_mint),
                (5, output_maker, output_mint),
            ] {
                if present[position - 2] {
                    balances.push(TokenBalance {
                        account_index: position as u32 + 2,
                        mint: base58::encode(mint),
                        ui_token_amount: Some(UiTokenAmount {
                            amount: amount.to_string(),
                            decimals: 6,
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                }
            }
        }
        tx
    }

    #[test]
    fn all_contract_native_settlement_paths_preserve_user_amounts_and_maker_identity() {
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            for (present, input, output) in [([a, b, true, true], crate::SOL_MINT, [32; 32]), ([true, true, a, b], [31; 32], crate::SOL_MINT)] {
                let tx = fill_transaction(present, input, output, [false, false]);
                let decoded = crate::process_transaction(tx).unwrap();
                assert_eq!(decoded.swaps.len(), 1, "missing native path {present:?}");
                let swap = &decoded.swaps[0];
                assert_eq!(swap.protocol, 49);
                assert_eq!((swap.input_amount, swap.output_amount), (100, 200));
                assert_eq!(swap.amm_pool, vec![2; 32]);
                assert_eq!(swap.user, vec![1; 32]);
                assert!(swap.transfer_verified);
            }
        }
    }

    #[test]
    fn token2022_receipts_require_independent_net_balance_evidence() {
        for checked in [[true, false], [false, true], [true, true]] {
            let tx = fill_transaction([true; 4], [31; 32], [32; 32], checked);
            let swap = crate::process_transaction(tx.clone()).unwrap().swaps.pop().unwrap();
            assert_eq!((swap.input_amount, swap.output_amount), (100, 200));
            if checked[1] {
                let mut missing = tx.clone();
                missing.meta.as_mut().unwrap().pre_token_balances.clear();
                missing.meta.as_mut().unwrap().post_token_balances.clear();
                assert!(crate::process_transaction(missing).unwrap().swaps.is_empty());
                let mut fee = tx;
                fee.meta
                    .as_mut()
                    .unwrap()
                    .post_token_balances
                    .iter_mut()
                    .find(|balance| balance.account_index == 6)
                    .unwrap()
                    .ui_token_amount
                    .as_mut()
                    .unwrap()
                    .amount = "194".into();
                let swap = crate::process_transaction(fee).unwrap().swaps.pop().unwrap();
                assert_eq!((swap.input_amount, swap.output_amount), (100, 194));
            }
        }
    }

    #[test]
    fn incorrect_user_roles_and_native_lifecycles_never_publish_prices() {
        let plain = fill_transaction([true; 4], [31; 32], [32; 32], [false; 2]);
        let wrapped = fill_transaction([false, true, true, true], crate::SOL_MINT, [32; 32], [false; 2]);
        let unwrapped = fill_transaction([true, true, false, true], [31; 32], crate::SOL_MINT, [false; 2]);
        for mode in 0..8 {
            let mut tx = if mode < 4 {
                plain.clone()
            } else if mode == 4 {
                wrapped.clone()
            } else {
                unwrapped.clone()
            };
            match mode {
                0 => tx.meta.as_mut().unwrap().inner_instructions[0].instructions[0].data[1] = 99,
                1 => tx.meta.as_mut().unwrap().inner_instructions[0].instructions[1].accounts[2] = 2,
                2 => tx.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data.truncate(31),
                3 => tx.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[0] = 0,
                4 => {
                    tx.meta.as_mut().unwrap().inner_instructions[0].instructions.remove(1);
                }
                5 => {
                    tx.meta.as_mut().unwrap().inner_instructions[0].instructions.remove(4);
                }
                6 => tx.meta.as_mut().unwrap().inner_instructions[0].instructions[4].accounts[1] = 2,
                7 => tx.meta.as_mut().unwrap().inner_instructions[0].instructions[2].data[1] = 9,
                _ => unreachable!(),
            }
            assert!(
                crate::process_transaction(tx).map_or(true, |decoded| decoded.swaps.is_empty()),
                "unproved RFQ mode {mode}"
            );
        }
    }
}
