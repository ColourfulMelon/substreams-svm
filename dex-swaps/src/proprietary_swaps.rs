//! Swap-only contracts derived from pinned program-owned deployed executables.
//! Unknown instruction variants stay diagnostic; committed typed receipts are
//! the amounts, and Token-2022 fees still require independent mint metadata.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58, block_view::InstructionView};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const QUANTUM: [u8; 32] = b58!("QuaNtZsgYRe5Z9Bk4LZ4cTD9tbkVoyCNf1R2BN9bBDv");
pub(crate) const BINARYFI: [u8; 32] = b58!("B72M6nyCLFgWiJtAN4naUTminMiTmyGcEqQHXwVeRdht");
const CLOCK: [u8; 32] = b58!("SysvarC1ock11111111111111111111111111111111");
const INSTRUCTIONS: [u8; 32] = b58!("Sysvar1nstructions1111111111111111111111111");
const BINARY_EXTRA: [u8; 32] = b58!("J1to1yufRnoWn81KYg1XkTWzmKjnYSnmE2VY8DGUJ9Qv");

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a = ix.accounts();
    let d = ix.data();
    let quantum = ix.program_id().0.as_slice() == QUANTUM;
    let (protocol, pool, users, vaults) = if quantum {
        // Quantum entrypoint11256 opcode7 ->5588 ->payload parser230:
        // exactly two u64s and a direction byte. Fixed A/B vaults and users;
        // direction1 spends B, direction0 spends A. Config is also authority.
        if a.len() != 10
            || d.len() != 18
            || d[0] != 7
            || d[17] > 1
            || a[7].0.as_slice() != token::PROGRAM_ID
            || a[8].0.as_slice() != CLOCK
            || a[9].0.as_slice() != INSTRUCTIONS
        {
            return None;
        }
        if d[17] == 1 {
            (Protocol::Quantum, 3, [2, 1], [5, 4])
        } else {
            (Protocol::Quantum, 3, [1, 2], [4, 5])
        }
    } else if ix.program_id().0.as_slice() == BINARYFI {
        // BinaryFi dispatcher9888 opcode8 ->8121: exactly14 accounts and
        // 16 payload bytes. Slots10/11 bind output/input token programs.
        if a.len() != 14
            || d.len() != 17
            || d[0] != 8
            || a[10..12]
                .iter()
                .any(|p| ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&p.0.as_slice()))
            || a[12].0.as_slice() != INSTRUCTIONS
            || a[13].0.as_slice() != BINARY_EXTRA
            || a[4].0 == a[5].0
        {
            return None;
        }
        (Protocol::Binaryfi, 2, [8, 9], [7, 6])
    } else {
        return None;
    };
    let amount = u64::from_le_bytes(d[1..9].try_into().ok()?);
    if amount == 0
        || a[users[0]].0 == a[users[1]].0
        || a[vaults[0]].0 == a[vaults[1]].0
        || a[pool].0 == a[0].0
        || a[0].0 == a[3].0
        || vaults.iter().any(|v| users.iter().any(|u| a[*v].0 == a[*u].0))
    {
        return None;
    }
    Some(Layout {
        protocol,
        pool,
        authority: 0,
        users,
        vaults,
        amount,
        exact_input: true,
        input_limit: false,
        input_first: true,
        fixed_users: true,
        minimum_output: u64::from_le_bytes(d[9..17].try_into().ok()?),
    })
}

pub(crate) fn bound_layout(ix: &InstructionView, mints: &TokenMintLookup) -> Option<Layout> {
    let mut l = layout(ix)?;
    if l.protocol == Protocol::Binaryfi {
        let a = ix.accounts();
        // Owned executable parser11021 validates fixed A/B mint4/5 and
        // vault6/7. Its direction branches11545/11589 select them from the
        // independently decoded user8 mint; user9 is always the destination.
        let input = mints.mint_for(a[8].0)?;
        let output = mints.mint_for(a[9].0)?;
        l.vaults = if input.as_slice() == a[4].0.as_slice() && output.as_slice() == a[5].0.as_slice() {
            [6, 7]
        } else if input.as_slice() == a[5].0.as_slice() && output.as_slice() == a[4].0.as_slice() {
            [7, 6]
        } else {
            return None;
        };
    }
    Some(l)
}

pub(crate) fn settlement(source: u32, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup, successful: &HashSet<u32>) -> bool {
    let Some(l) = bound_layout(ix, mints) else { return false };
    let a = ix.accounts();
    let quantum = l.protocol == Protocol::Quantum;
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if !successful.contains(&source)
        || calls.len() != 2
        || calls
            .iter()
            .enumerate()
            .any(|(n, c)| c.stack_height() != depth + 1 || !successful.contains(&(source + 1 + n as u32)))
    {
        return false;
    }
    let mut amounts = [0; 2];
    let mut mint_pair = [None, None];
    for (n, c) in calls.iter().enumerate() {
        let p = a[if quantum { 7 } else { 10 + l.vaults[n] - 6 }].0;
        let checked = p.as_slice() == token_2022::PROGRAM_ID;
        let ca = c.accounts();
        let data = c.data();
        if c.program_id().0 != p
            || data.len() != if checked { 10 } else { 9 }
            || data[0] != if checked { 12 } else { 3 }
            || ca.len() != if checked { 4 } else { 3 }
        {
            return false;
        }
        let (src, dst, auth) = if n == 0 { (l.users[0], l.vaults[0], 0) } else { (l.vaults[1], l.users[1], 3) };
        if ca[0].0 != a[src].0 || ca[if checked { 2 } else { 1 }].0 != a[dst].0 || ca[if checked { 3 } else { 2 }].0 != a[auth].0 {
            return false;
        }
        let mint = mints.mint_for(a[src].0);
        if mint.is_none()
            || mint != mints.mint_for(a[dst].0)
            || !quantum && (mint.as_deref() != Some(a[l.vaults[n] - 2].0.as_slice()) || checked && ca[1].0 != a[l.vaults[n] - 2].0)
        {
            return false;
        }
        amounts[n] = u64::from_le_bytes(data[1..9].try_into().unwrap());
        mint_pair[n] = mint;
    }
    amounts[0] == l.amount && amounts[1] > 0 && mint_pair[0] != mint_pair[1] && amounts[1] >= l.minimum_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

    #[test]
    fn finalized_swap_only_dispatch_binds_exact_principals_and_net_receipts() {
        for (fixture, source, input, output) in [
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/2Abm3jG45RQigSUnowRNba9qcEqjd3qK2YhmVWT7AhYK9n1XsdxWp4MbsPbtWH3hmvvXZuqgh2ZCaD9gqsqvzmzs.pb"
                )
                .as_slice(),
                7u32,
                300000000u64,
                2766520874u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/5tYiVyD4QQmKJy6Gfy3AFbKsN3TFE98DZR9eX2jtag2UDg91MoXNDbrXKDDk47wLJnKD37wzYiwkEAZx4kamb3yU.pb"
                )
                .as_slice(),
                7u32,
                300000000u64,
                2763381547u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/5kZZmtbho5hwXEsgkcwCHQ3b5q4bUX1d5yDDeroW6v4zCZJ8bUn1urZCHutKyoU3294uED1dB14cDXpb6Uu17pyA.pb"
                )
                .as_slice(),
                7u32,
                300000000u64,
                2762077353u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/PbrepLjR5YV53ZNkG2SvXa99w6WwVESayaXhq25D2DrJYkuqB7g66fS2rZbXtgHA7VkcqFCXCpFnMU3g2NDQ4Ft.pb"
                )
                .as_slice(),
                7u32,
                300000000u64,
                2765656793u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/5ooeU9x28F8p2QPpFeAVnKpWWVfqU8gBoeBEfVk82sVv4oRMTW2X3Z4o7qPBTQy37go7mCTN31ojgVSDyFoXLLwE.pb"
                )
                .as_slice(),
                7u32,
                300000000u64,
                2767539995u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/5awVQ52m8kajyweLbwdWEjT44vEnQvUnenjetyKX5aT1h33xfkkVNC9uNf55c3SUJymR31UeaPpxXXqWmqE69JXn.pb"
                )
                .as_slice(),
                5u32,
                3500000000u64,
                4245500u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/2auA3DMhyCH95hwFiewguBA5KHLLHe1dUeUvGNSGD1CcDjTvh2qQ1h9EtHhnc5qWWdqk2WgrQxsFZoZXMQ8rBcs2.pb"
                )
                .as_slice(),
                5u32,
                3500000000u64,
                4249000u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/4UpWkTY6ap7v4n2tJ3Mvi1iZ6hAD9N43D2H8v3Envd1hUFi6mMbepzcyghevoSZtyR4DF4nH82knM1tj6VouwWcJ.pb"
                )
                .as_slice(),
                13u32,
                541765u64,
                3274u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/xNWtkPwH25exbsqgq268f8BUqWwkPjLnk1Pud6jWJXKKh9ZfusucpaJuJYd3CMAaaWsJ9Sx41Yz81qCBsWqVmfy.pb"
                )
                .as_slice(),
                17u32,
                8095071u64,
                886913u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/proprietary-swaps/4SYmmVYkwhMm4WPnsgSyJUXVxHJwA23hVaaCLbdifahR4XZEaDQBtztKbyCCBxaADeDQdvGdKHtKKLrBAoPdv7q1.pb"
                )
                .as_slice(),
                16u32,
                34110031u64,
                129051340u64,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let views: Vec<_> = tx.walk_instructions().collect();
            let ix = &views[source as usize];
            let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            let l = bound_layout(ix, &mints).unwrap();
            let a: Vec<_> = ix.accounts().iter().map(|a| a.0.clone()).collect();
            let program = ix.program_id().0.clone();
            let native_data = ix.data().clone();
            let depth = ix.stack_height();
            let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            assert!(settlement(
                source,
                ix,
                &views[source as usize + 1..source as usize + 3],
                &mints,
                &crate::logs::successful_invocations(&tx)
            ));
            let decoded = crate::process_transaction_with_metadata(
                tx.clone(),
                None,
                |_, _| None,
                |_, _| None,
                |mint, _| {
                    let data = match substreams_solana::base58::encode(mint).as_str() {
                        "SPCXxcqXj6e5dJDVNovHN8744zkbhM2bYudU45BimGb" => {
                            include_bytes!("../fixtures/swap-coverage/proprietary-swaps/fee-mints/SPCXxcqXj6e5dJDVNovHN8744zkbhM2bYudU45BimGb.bin").as_slice()
                        }
                        "XsDoVfqeBukxuZHWhdvWHBhgEHjGNst4MLodqsJHzoB" => {
                            include_bytes!("../fixtures/swap-coverage/proprietary-swaps/fee-mints/XsDoVfqeBukxuZHWhdvWHBhgEHjGNst4MLodqsJHzoB.bin").as_slice()
                        }
                        _ => return None,
                    };
                    let (fees, initialized) = crate::token_fees::owned_mint(&token_2022::PROGRAM_ID, data)?;
                    assert!(initialized);
                    fees.at(1055)
                },
            )
            .unwrap();
            let found: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(source) && s.amm == program).collect();
            assert_eq!(found.len(), 1);
            let s = found[0];
            assert_eq!(s.protocol, l.protocol as i32);
            assert_eq!(s.amm_pool, a[l.pool]);
            assert_eq!(s.user, a[0]);
            assert_eq!(s.input_mint, mints.mint_for(&a[l.users[0]]).unwrap());
            assert_eq!(s.output_mint, mints.mint_for(&a[l.users[1]]).unwrap());
            assert_eq!((s.input_amount, s.output_amount), (input, output));
            assert_eq!(s.source_transfer_index, Some(source + 1));
            assert!(s.transfer_verified);
            assert!(decoded.coverage_candidates.iter().all(|c| c.source_index != source));
            for mode in 0..20 {
                let mut wrong = tx.clone();
                let group = wrong
                    .meta
                    .as_mut()
                    .unwrap()
                    .inner_instructions
                    .iter_mut()
                    .find(|g| g.instructions.iter().any(|ix| ix.data == native_data))
                    .unwrap();
                let position = group.instructions.iter().position(|ix| ix.data == native_data).unwrap();
                let calls = &mut group.instructions;
                let native = calls[position].clone();
                let checked = calls[position + 2].data[0] == 12;
                match mode {
                    0 => calls[position].data[0] = 0,
                    1 => calls[position].data.push(0),
                    2 => calls[position].data[1..9].copy_from_slice(&(input - 1).to_le_bytes()),
                    3 => calls[position].data[9..17].copy_from_slice(&(output + 1).to_le_bytes()),
                    4 => calls[position + 1].accounts[0] = native.accounts[l.pool],
                    5 => calls[position + 1].accounts[1] = native.accounts[l.pool],
                    6 => calls[position + 1].accounts[2] = native.accounts[l.pool],
                    7 => calls[position + 1].data[0] = 7,
                    8 => calls[position + 2].accounts[0] = native.accounts[l.vaults[0]],
                    9 => calls[position + 2].accounts[if checked { 2 } else { 1 }] = native.accounts[0],
                    10 => calls[position + 2].accounts[if checked { 3 } else { 2 }] = native.accounts[0],
                    11 => calls[position + 2].data[0] = 7,
                    12 => calls[position + 1].stack_height = Some(depth + 2),
                    13 => calls[position + 2].stack_height = Some(depth + 2),
                    14 => {
                        let extra = calls[position + 1].clone();
                        calls.insert(position + 2, extra);
                    }
                    15 => calls[position + 1].program_id_index = native.program_id_index,
                    16 => calls[position].accounts[l.pool] = native.accounts[0],
                    17 => {
                        let success = format!("Program {} success", substreams_solana::base58::encode(&program));
                        let logs = &mut wrong.meta.as_mut().unwrap().log_messages;
                        let ordinal = views.iter().take(source as usize).filter(|prior| prior.program_id().0 == &program).count();
                        let end = logs.iter().enumerate().filter(|(_, line)| *line == &success).nth(ordinal).unwrap().0;
                        logs[end] = success.replace(" success", " failed: custom program error: 1");
                    }
                    18 => calls[position + 2].data[1..9].fill(0),
                    _ => {
                        if l.protocol == Protocol::Quantum {
                            calls[position].data[17] = 2;
                        } else {
                            calls[position].accounts[13] = native.accounts[0];
                        }
                    }
                }
                let bad: Vec<_> = wrong.walk_instructions().collect();
                let end = bad
                    .iter()
                    .enumerate()
                    .skip(source as usize + 1)
                    .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
                    .map_or(bad.len(), |(end, _)| end);
                let mints = TokenMintLookup::new(&wrong, wrong.meta.as_ref().unwrap());
                assert!(
                    !settlement(
                        source,
                        &bad[source as usize],
                        &bad[source as usize + 1..end],
                        &mints,
                        &crate::logs::successful_invocations(&wrong)
                    ),
                    "source{source} mutation{mode}"
                );
            }
        }
    }
}
