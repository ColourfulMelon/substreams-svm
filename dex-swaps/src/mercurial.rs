//! Mercurial's documented N-coin Exchange ABI. Pool vaults form a declared
//! array; user source/destination follow it, rather than fixed two-coin roles.
use substreams_solana::{b58, block_view::InstructionView};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("MERLuDFBMmsHnsBPZw2sDQZHvXFMwp8EdjudcU2HKky");

pub(crate) struct Exchange {
    pub amount: u64,
    pub minimum_output: u64,
    pub source: usize,
    pub destination: usize,
}

pub(crate) fn layout(ix: &InstructionView) -> Option<Exchange> {
    let accounts = ix.accounts();
    let data = ix.data();
    if ix.program_id().0.as_slice() != PROGRAM_ID
        || data.len() != 17
        || data[0] != 4
        || !(8..=10).contains(&accounts.len())
        || accounts[1].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
    {
        return None;
    }
    let source = accounts.len() - 2;
    let destination = source + 1;
    if accounts[source].0 == accounts[destination].0
        || accounts[0].0 == accounts[2].0
        || accounts[4..source].iter().enumerate().any(|(i, vault)| {
            vault.0.iter().all(|byte| *byte == 0)
                || vault.0 == accounts[source].0
                || vault.0 == accounts[destination].0
                || accounts[4..4 + i].iter().any(|other| other.0 == vault.0)
        })
    {
        return None;
    }
    let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
    (amount > 0).then_some(Exchange {
        amount,
        minimum_output: u64::from_le_bytes(data[9..17].try_into().ok()?),
        source,
        destination,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::pb::dex::swaps::v1::Protocol;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn finalized_exchange_uses_exact_committed_transfers_and_rejects_incomplete_proof() {
        for (fixture, index, input, output) in [
            (
                include_bytes!("../fixtures/native-venues/mercurial-route-1.transaction.pb").as_slice(),
                14,
                20_072u64,
                20_105u64,
            ),
            (
                include_bytes!("../fixtures/native-venues/mercurial-route-2.transaction.pb").as_slice(),
                20,
                21_123u64,
                21_195u64,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let native_data = tx.walk_instructions().nth(index as usize).unwrap().data().clone();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(index)).collect();
            assert_eq!(swaps.len(), 1);
            let swap = swaps[0];
            assert_eq!(swap.protocol, Protocol::Mercurial as i32);
            assert_eq!(swap.amm, PROGRAM_ID);
            assert_eq!(base58::encode(&swap.amm_pool), "UST3iPxDFwUUiToMyLF7DYqSP9uaoz7Mzs2LxRYxVJG");
            assert_eq!(base58::encode(&swap.user), "BJcxXxr2mSr71SVJaqvHHmdxHUrYRvC4sct1wMNZxirK");
            assert_eq!(base58::encode(&swap.input_mint), "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
            assert_eq!(base58::encode(&swap.output_mint), "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
            assert_eq!((swap.input_amount, swap.output_amount), (input, output));
            assert_eq!(swap.source_transfer_index, Some(index + 1));
            assert!(swap.transfer_verified);
            for mode in 0..25 {
                let mut altered = tx.clone();
                let meta = altered.meta.as_mut().unwrap();
                let (group, position) = meta
                    .inner_instructions
                    .iter()
                    .enumerate()
                    .find_map(|(g, group)| group.instructions.iter().position(|ix| ix.data == native_data).map(|p| (g, p)))
                    .unwrap();
                let instructions = &mut meta.inner_instructions[group].instructions;
                let accounts = instructions[position].accounts.clone();
                match mode {
                    0 => instructions[position].data[1..9].copy_from_slice(&(input - 1).to_le_bytes()),
                    1 => instructions[position].data[1..9].copy_from_slice(&(input + 1).to_le_bytes()),
                    2 => instructions[position].data[9..17].copy_from_slice(&(output + 1).to_le_bytes()),
                    3 => instructions[position].data[0] = 1,
                    4 => instructions[position].data[0] = 3,
                    5 => {
                        instructions[position].data.pop();
                    }
                    6 => instructions[position].data.push(0),
                    7 => instructions[position].accounts[1] = accounts[0],
                    8 => instructions[position].accounts[2] = accounts[0],
                    9 => instructions[position].accounts[3] = accounts[0],
                    10 => instructions[position].accounts[7] = accounts[0],
                    11 => instructions[position].accounts[8] = accounts[0],
                    12 => instructions[position].accounts[6] = accounts[0],
                    13 => instructions[position].accounts[5] = accounts[6],
                    14 => instructions[position + 1].accounts[2] = accounts[0],
                    15 => instructions[position + 2].accounts[2] = accounts[0],
                    16 => {
                        instructions.remove(position + 2);
                    }
                    17 => {
                        let duplicate = instructions[position + 2].clone();
                        instructions.insert(position + 3, duplicate);
                    }
                    18 => instructions[position + 2].stack_height = Some(4),
                    19 => instructions[position + 2].program_id_index = instructions[position].program_id_index,
                    20 => instructions.swap(position + 1, position + 2),
                    21 => instructions[position + 1].data[1..9].copy_from_slice(&0u64.to_le_bytes()),
                    22 | 23 => {
                        let program = base58::encode(PROGRAM_ID);
                        let invoke = meta
                            .log_messages
                            .iter()
                            .position(|log| log == &format!("Program {program} invoke [2]"))
                            .unwrap();
                        let end = meta
                            .log_messages
                            .iter()
                            .enumerate()
                            .skip(invoke)
                            .find(|(_, log)| {
                                if mode == 22 {
                                    **log == format!("Program {} success", base58::encode(substreams_solana_idls::jupiter::v4::PROGRAM_ID))
                                } else {
                                    **log == format!("Program {program} success")
                                }
                            })
                            .map(|(i, _)| i)
                            .unwrap();
                        if mode == 22 {
                            meta.log_messages[end] = format!(
                                "Program {} failed: custom program error: 1",
                                base58::encode(substreams_solana_idls::jupiter::v4::PROGRAM_ID)
                            );
                        } else {
                            meta.log_messages.truncate(end);
                        }
                    }
                    _ => {
                        let mut unknown = instructions[position + 1].clone();
                        unknown.data[0] = 8;
                        instructions.insert(position + 3, unknown);
                    }
                }
                assert!(
                    crate::process_transaction(altered).map_or(true, |decoded| { decoded.swaps.iter().all(|swap| swap.source_index != Some(index)) }),
                    "Mercurial mode {mode}"
                );
            }
        }
    }

    #[test]
    fn documented_coin_array_layouts_keep_users_after_all_vaults() {
        for count in 2..=4 {
            let mut accounts: Vec<_> = (0..count + 6).map(|i| [i as u8 + 1; 32]).collect();
            accounts[1] = substreams_solana_idls::spl::token::PROGRAM_ID;
            let mut data = vec![4];
            data.extend_from_slice(&100u64.to_le_bytes());
            data.extend_from_slice(&90u64.to_le_bytes());
            let tx = crate::routed_pool::test_fixture::make_tx(PROGRAM_ID, &accounts, data.clone());
            let exchange = layout(&tx.walk_instructions().next().unwrap()).unwrap();
            assert_eq!((exchange.source, exchange.destination), (4 + count, 5 + count));
            assert_eq!((exchange.amount, exchange.minimum_output), (100, 90));
            accounts[4] = accounts[5];
            let tx = crate::routed_pool::test_fixture::make_tx(PROGRAM_ID, &accounts, data);
            assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
        }
    }
}
