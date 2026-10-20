//! JupiterZ Order Engine fills, pinned to its published production contract.
//! The market identity is the actual maker account, not an invented AMM pool.
use substreams_solana::{block_view::InstructionView};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("61DFfeTKM7trxYcPQCM78bJ794ddZprZpAwAnLiwTpYH");

pub(crate) const UPDATED_PROGRAM_ID: [u8; 32] = b58!("2En5Y11SEAGLNmEezTuRUCwTyzyNReHaMbSnS5gjGsL1");
pub(crate) const V2_PROGRAM_ID: [u8; 32] = b58!("fd3nMFYTQjX1yr5ER8u7tPdHJB7qt8RpDpNtLQX2Br5");

pub(crate) struct Fill {
    pub input_mint: usize,
    pub output_mint: usize,
    pub input_token_program: usize,
    pub output_token_program: usize,
    pub input_accounts: [usize; 2],
    pub output_accounts: [usize; 2],
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
    let program = ix.program_id();
    let data = ix.data();
    let accounts = ix.accounts();
    if program.0.as_slice() == V2_PROGRAM_ID {
        // Program-owned IDL and official rfq-v2-sdk/fill-decoder f3f30ff:
        // Side, amount atoms, expiry, tick atoms/lot, base atoms/lot, Vec<Level>.
        // Price all exact physical principals, including exact lot rounding.
        if accounts.len() != 11 || data.len() < 45
            || data[..8] != [222, 208, 6, 209, 154, 163, 54, 94] || data[8] > 1
            || accounts[10].0.as_slice() != b58!("Sysvar1nstructions1111111111111111111111111")
            || ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&accounts[8].0.as_slice())
            || ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&accounts[9].0.as_slice())
            || accounts[6].0 == accounts[7].0 { return None; }
        let amount = u64::from_le_bytes(data[9..17].try_into().ok()?);
        let tick = u64::from_le_bytes(data[25..33].try_into().ok()?);
        let lot = u64::from_le_bytes(data[33..41].try_into().ok()?);
        let count = u32::from_le_bytes(data[41..45].try_into().ok()?) as usize;
        if tick == 0 || lot == 0 || count == 0
            || data.len() != 45usize.checked_add(count.checked_mul(16)?)? { return None; }
        let mut remaining = amount;
        let mut output = 0u64;
        for level in data[45..].chunks_exact(16) {
            if remaining == 0 { break; }
            let price = u64::from_le_bytes(level[..8].try_into().ok()?).checked_mul(tick)?;
            let quantity = u64::from_le_bytes(level[8..].try_into().ok()?);
            let (cost, proceeds) = if data[8] == 0 { (price, lot) } else { (lot, price) };
            if cost == 0 { continue; }
            let filled = quantity.min(remaining / cost);
            remaining = remaining.checked_sub(filled.checked_mul(cost)?)?;
            output = output.checked_add(filled.checked_mul(proceeds)?)?;
        }
        let input = amount.checked_sub(remaining)?;
        if input == 0 || output == 0 { return None; }
        let sell_base = data[8] == 1;
        return Some(Fill {
            input_mint: if sell_base { 6 } else { 7 }, output_mint: if sell_base { 7 } else { 6 },
            input_token_program: if sell_base { 8 } else { 9 }, output_token_program: if sell_base { 9 } else { 8 },
            input_accounts: if sell_base { [2, 4] } else { [3, 5] }, output_accounts: if sell_base { [3, 5] } else { [2, 4] },
            input_source: if sell_base { 2 } else { 3 }, input_destination: if sell_base { 4 } else { 5 },
            output_source: if sell_base { 5 } else { 4 }, output_destination: if sell_base { 3 } else { 2 },
            input_amount: input, output_amount: output, unwrap_input: false, unwrap_output: false,
        });
    }
    if ![PROGRAM_ID.as_slice(), UPDATED_PROGRAM_ID.as_slice()].contains(&program.0.as_slice()) { return None; }
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
    let missing = [2, 3, 4, 5].map(|index| accounts[index].0 == program.0);
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
        input_mint: 6, output_mint: 8, input_token_program: 7, output_token_program: 9,
        input_accounts: [2, 3], output_accounts: [4, 5],
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
        let meta = tx.meta.as_option_mut().unwrap();
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
                        ui_token_amount: buffa::MessageField::some(UiTokenAmount {
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
                assert_eq!(swap.protocol.to_i32(), 49);
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
                missing.meta.as_option_mut().unwrap().pre_token_balances.clear();
                missing.meta.as_option_mut().unwrap().post_token_balances.clear();
                assert!(crate::process_transaction(missing).unwrap().swaps.is_empty());
                let mut fee = tx;
                fee.meta
                    .as_option_mut()
                    .unwrap()
                    .post_token_balances
                    .iter_mut()
                    .find(|balance| balance.account_index == 6)
                    .unwrap()
                    .ui_token_amount
                    .as_option_mut()
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
                0 => tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[0].data[1] = 99,
                1 => tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[1].accounts[2] = 2,
                2 => tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].data.truncate(31),
                3 => tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0].data[0] = 0,
                4 => {
                    tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions.remove(1);
                }
                5 => {
                    tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions.remove(4);
                }
                6 => tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[4].accounts[1] = 2,
                7 => tx.meta.as_option_mut().unwrap().inner_instructions[0].instructions[2].data[1] = 9,
                _ => unreachable!(),
            }
            assert!(
                crate::process_transaction(tx).map_or(true, |decoded| decoded.swaps.is_empty()),
                "unproved RFQ mode {mode}"
            );
        }
    }

    #[test]
    fn current_production_fills_bind_finalized_principals_and_reject_routed_bypasses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage/rfq-current-production");
        let mut positives = 0;
        let mut rejected = 0;
        for line in include_str!("../fixtures/swap-coverage/rfq-current-production/cases.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(root.join(format!("{}.pb", f[0]))).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let source: u32 = f[1].parse().unwrap();
            let first: u32 = f[2].parse().unwrap();
            let source_ix = tx.walk_instructions().nth(source as usize).unwrap();
            let source_data = source_ix.data().clone();
            let a = source_ix.accounts();
            let parsed = layout(&source_ix).unwrap();
            let native = crate::process_transaction(tx.clone()).unwrap().swaps.into_iter()
                .find(|x| x.source_index == Some(source)).expect("finalized RFQ native price");
            assert_eq!(native.protocol.to_i32(), 49);
            assert_eq!(base58::encode(&native.program_id), f[3]);
            assert_eq!(base58::encode(&native.amm), f[3]);
            assert_eq!(base58::encode(&native.amm_pool), f[4]);
            assert_eq!(base58::encode(&native.user), f[5]);
            assert_eq!(base58::encode(&native.input_mint), f[6]);
            assert_eq!(native.input_amount.to_string(), f[7]);
            assert_eq!(base58::encode(&native.output_mint), f[8]);
            assert_eq!(native.output_amount.to_string(), f[9]);
            assert_eq!(native.source_transfer_index, Some(first));
            assert!(native.transfer_verified);
            positives += 1;
            let is_v2 = source_ix.program_id().0.as_slice() == V2_PROGRAM_ID;
            for mode in 0..12 {
                let mut wrong = tx.clone();
                // Matching the immutable native payload identifies both root
                // and routed inner instructions without modifying their index.
                if mode <= 3 || mode == 10 || mode == 11 {
                    let modify = |data: &mut Vec<u8>, accounts: &mut Vec<u8>| match mode {
                        0 => data[0] ^= 1,
                        1 => {
                            let offset = if is_v2 { 9 } else { 8 };
                            let n = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                            let step = if is_v2 { u64::from_le_bytes(data[33..41].try_into().unwrap()) } else { 1 };
                            data[offset..offset + 8].copy_from_slice(&(n + step).to_le_bytes());
                        }
                        2 => accounts.swap(0, 1),
                        3 => { accounts.swap(parsed.input_source, parsed.input_destination); }
                        10 => { data.truncate(if is_v2 { 44 } else { 31 }); }
                        11 => {
                            if is_v2 { data[33..41].fill(0); }
                            else { accounts.swap(6, 8); }
                        }
                        _ => unreachable!(),
                    };
                    let message = wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
                    if let Some(ix) = message.instructions.iter_mut().find(|x| x.data == source_data) {
                        modify(&mut ix.data, &mut ix.accounts);
                    } else {
                        let ix = wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut()
                            .flat_map(|g| &mut g.instructions).find(|x| x.data == source_data).unwrap();
                        modify(&mut ix.data, &mut ix.accounts);
                    }
                } else if mode == 4 || mode == 5 {
                    let calls = &mut wrong.meta.as_option_mut().unwrap().inner_instructions;
                    let i = calls.iter_mut().flat_map(|g| &mut g.instructions)
                        .find(|x| x.data.len() >= 9 && matches!(x.data[0], 3 | 12)
                            && u64::from_le_bytes(x.data[1..9].try_into().unwrap()) == native.output_amount).unwrap();
                    if mode == 4 { i.data[1] ^= 1; } else { i.accounts.swap(0, 1); }
                } else if mode == 6 {
                    wrong.meta.as_option_mut().unwrap().log_messages.clear();
                } else if mode == 7 {
                    let logs = &mut wrong.meta.as_option_mut().unwrap().log_messages;
                    let end = logs.iter().rposition(|x| x == &format!("Program {} success", f[3])).unwrap();
                    logs[end] = format!("Program {} failed: custom program error: 1", f[3]);
                } else if mode == 8 {
                    wrong.meta.as_option_mut().unwrap().post_token_balances.clear();
                } else {
                    let logs = &mut wrong.meta.as_option_mut().unwrap().log_messages;
                    let child_depth = if source_ix.is_root() { 2 } else { source_ix.stack_height() + 1 };
                    let end = logs.iter().rposition(|x| x.ends_with(&format!("invoke [{child_depth}]"))).unwrap();
                    logs.truncate(end + 1);
                    logs.push("Log truncated".into());
                }
                let rows = crate::process_transaction(wrong.clone()).unwrap().swaps;
                assert!(!rows.iter().any(|x| x.source_index == Some(source)), "RFQ {} native mutation {mode}", f[0]);
                // A Jupiter event with authentic amounts must never bypass a
                // malformed/failed native receipt, even without expansion.
                let mints = crate::token_mints::TokenMintLookup::new(&wrong, wrong.meta.as_option().unwrap());
                let mut routed = native.clone();
                routed.protocol = 5.into();
                routed.program_id = substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                let mut rows = vec![routed];
                crate::quality::verify_and_deduplicate(&wrong, &mints, &mut rows, false);
                assert!(rows.is_empty(), "RFQ {} routed mutation {mode}", f[0]);
                rejected += 2;
            }
            if f[0] == "3NSsTj15VzsyerzXesFxuaKr6n6ZRPVtXegdyLFpwhTmkSBskueEzmciNR5Pxy3ZWvaeSx1xMfW2b9dnFSy1dkmY" {
                for mode in 0..3 {
                    let mut wrong = tx.clone();
                    for child in wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions) {
                        if child.data.as_slice() == [17] {
                            if mode == 0 { child.data[0] = 21; }
                            if mode == 1 { child.data.push(0); }
                            if mode == 2 { child.accounts.clear(); }
                        }
                    }
                    assert!(!crate::process_transaction(wrong.clone()).unwrap().swaps.iter().any(|x| x.source_index == Some(source)), "unsynchronized native credit {mode}");
                    let lookup = crate::token_mints::TokenMintLookup::new(&wrong, wrong.meta.as_option().unwrap());
                    let mut routed = native.clone(); routed.protocol = 5.into();
                    routed.program_id = substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                    let mut rows = vec![routed];
                    crate::quality::verify_and_deduplicate(&wrong, &lookup, &mut rows, false);
                    assert!(rows.is_empty(), "unsynchronized routed credit {mode}");
                    rejected += 2;
                }
            }
            // Successful wrappers normalize to the same native maker/principal
            // identity, and cannot substitute a different user or maker.
            let mints = crate::token_mints::TokenMintLookup::new(&tx, tx.meta.as_option().unwrap());
            for mode in 0..3 {
                let mut routed = native.clone();
                routed.protocol = 5.into();
                routed.program_id = substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                if mode == 1 { routed.user = a[1].0.clone(); }
                if mode == 2 { routed.amm_pool = a[0].0.clone(); }
                let mut rows = vec![routed];
                crate::quality::verify_and_deduplicate(&tx, &mints, &mut rows, false);
                if mode == 0 {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].protocol.to_i32(), 49);
                    assert_eq!(rows[0].program_id, native.program_id);
                } else { assert!(rows.is_empty()); rejected += 1; }
            }
        }
        assert_eq!(positives, 22);
        assert_eq!(rejected, 578);
        eprintln!("RFQ finalized native positives={positives}; invalid native/routed cases={rejected}");
    }


    #[test]
    fn v2_lot_sweep_binds_both_directions_and_exact_framing() {
        for side in [0u8, 1] {
            let mut accounts: Vec<_> = (1..=11).map(|i| [i; 32]).collect();
            accounts[6] = [31; 32]; accounts[7] = [32; 32];
            accounts[8] = token::PROGRAM_ID; accounts[9] = token::PROGRAM_ID;
            accounts[10] = b58!("Sysvar1nstructions1111111111111111111111111");
            let mut data = vec![222, 208, 6, 209, 154, 163, 54, 94, side];
            for value in [103u64, 2_000_000_000, 3, 5] { data.extend_from_slice(&value.to_le_bytes()); }
            data.extend_from_slice(&2u32.to_le_bytes());
            for value in [2u64, 10, 3, 30] { data.extend_from_slice(&value.to_le_bytes()); }
            let tx = make_tx(V2_PROGRAM_ID, &accounts, data.clone());
            let ix = tx.walk_instructions().next().unwrap();
            let f = layout(&ix).unwrap();
            assert_eq!((f.input_source, f.input_destination, f.output_source, f.output_destination),
                if side == 0 { (3, 5, 4, 2) } else { (2, 4, 5, 3) });
            assert_eq!((f.input_amount, f.output_amount), if side == 0 { (96, 70) } else { (100, 150) });
            for mode in 0..8 {
                let mut bad = data.clone();
                match mode {
                    0 => bad[8] = 2,
                    1 => bad[25..33].fill(0),
                    2 => bad[33..41].fill(0),
                    3 => bad[41..45].copy_from_slice(&3u32.to_le_bytes()),
                    4 => { bad.pop(); }
                    5 => bad.push(0),
                    6 => bad[25..33].copy_from_slice(&u64::MAX.to_le_bytes()),
                    _ => bad[9..17].fill(0),
                }
                let wrong = make_tx(V2_PROGRAM_ID, &accounts, bad);
                assert!(layout(&wrong.walk_instructions().next().unwrap()).is_none(), "V2 side {side} invalid framing {mode}");
            }
        }
    }

}
