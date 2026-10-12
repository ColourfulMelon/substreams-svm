//! Oasis/MarcoPolo's observed Anchor swap format. Jupiter v4 declares these
//! account roles; finalized receipts and owned pool/vaults corroborate them.
//! Do not classify other instructions or infer trades from arbitrary transfers.
use substreams_solana::{b58, block_view::InstructionView};

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("9tKE7Mbmj4mxDjWatikzGAtkoWosiiZX9y6J4Hfm2R8H");

pub(crate) fn layout(ix: &InstructionView) -> Option<(u64, bool)> {
    let a = ix.accounts();
    let d = ix.data();
    if ix.program_id().0.as_slice() != PROGRAM_ID
        || a.len() != 17
        || d.len() != 33
        || d[..8] != [248, 198, 158, 145, 225, 117, 135, 200]
        || d[32] > 1
        || a[13].0.as_slice() != b58!("11111111111111111111111111111111")
        || a[14].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
        || a[15].0.as_slice() != b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
        || a[16].0.as_slice() != b58!("SysvarRent111111111111111111111111111111111")
        || a[0].0.as_slice() != b58!("BE5YRQ6N6LCw7UL3JwzVp317EWa4mzJY6JKDaudcXu7A")
        || a[12].0.as_slice() != b58!("JM78XNzeQRmZXDAP4DSq88ZdErbuSXSLE6fkRsVDKSu")
        || a[0].0 == a[1].0
        || a[1].0 == a[12].0
        || a[2].0 == a[3].0
        || a[4..8]
            .iter()
            .enumerate()
            .any(|(i, account)| account.0.iter().all(|byte| *byte == 0) || a[4..4 + i].iter().any(|other| other.0 == account.0))
    {
        return None;
    }
    let amount = u64::from_le_bytes(d[8..16].try_into().ok()?);
    (amount > 0).then_some((amount, d[32] == 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::pb::dex::swaps::v1::Protocol;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn finalized_oasis_receipts_keep_exact_pools_mints_amounts_and_commitment() {
        let expected: Vec<_> = include_str!("../fixtures/native-venues/oasis-legs.tsv").lines().collect();
        for (case, fixture) in [
            include_bytes!("../fixtures/native-venues/oasis-route-1.transaction.pb").as_slice(),
            include_bytes!("../fixtures/native-venues/oasis-route-2.transaction.pb").as_slice(),
            include_bytes!("../fixtures/native-venues/oasis-route-3.transaction.pb").as_slice(),
            include_bytes!("../fixtures/native-venues/oasis-route-4.transaction.pb").as_slice(),
        ]
        .into_iter()
        .enumerate()
        {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let row: Vec<_> = expected[case].split('\t').collect();
            let index = row[1].parse::<u32>().unwrap();
            let ix = tx.walk_instructions().nth(index as usize).unwrap();
            let native_data = ix.data().clone();
            let input = row[6].parse::<u64>().unwrap();
            let output = row[8].parse::<u64>().unwrap();
            let a = ix.accounts();
            let program = solana_program::pubkey::Pubkey::new_from_array(PROGRAM_ID);
            let pool = solana_program::pubkey::Pubkey::find_program_address(&[b"marcopoolv1", a[2].0, a[3].0], &program).0;
            assert_eq!(pool.to_bytes().as_slice(), a[1].0.as_slice());
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.amm.as_slice() == PROGRAM_ID).collect();
            assert_eq!(swaps.len(), 1, "one physical leg, including the Jupiter wrapper");
            let swap = swaps[0];
            assert_eq!(swap.protocol, Protocol::Oasis as i32);
            assert_eq!(swap.source_index, Some(index));
            assert_eq!(swap.source_transfer_index, Some(index + 1));
            assert_eq!(base58::encode(&swap.amm_pool), row[3]);
            assert_eq!(base58::encode(&swap.user), row[4]);
            assert_eq!(base58::encode(&swap.input_mint), row[5]);
            assert_eq!(base58::encode(&swap.output_mint), row[7]);
            assert_eq!((swap.input_amount, swap.output_amount), (input, output));
            assert!(swap.transfer_verified);
            for mode in 0..37 {
                let mut altered = tx.clone();
                let meta = altered.meta.as_mut().unwrap();
                let (group, position) = meta
                    .inner_instructions
                    .iter()
                    .enumerate()
                    .find_map(|(g, group)| group.instructions.iter().position(|ix| ix.data == native_data).map(|p| (g, p)))
                    .unwrap();
                let calls = &mut meta.inner_instructions[group].instructions;
                let accounts = calls[position].accounts.clone();
                match mode {
                    0 => calls[position].data[0] ^= 1,
                    1 => {
                        calls[position].data.pop();
                    }
                    2 => calls[position].data.push(0),
                    3 => calls[position].data[32] = 2,
                    4 => calls[position].data[32] ^= 1,
                    5 => calls[position].data[8..16].copy_from_slice(&(input - 1).to_le_bytes()),
                    6 => calls[position].data[8..16].copy_from_slice(&0u64.to_le_bytes()),
                    7 => calls[position].accounts[0] = accounts[1],
                    8 => calls[position].accounts[1] = accounts[0],
                    9 => calls[position].accounts[2] = accounts[3],
                    10 => calls[position].accounts[2] = accounts[0],
                    11 => calls[position].accounts[3] = accounts[0],
                    12 => calls[position].accounts[4] = accounts[5],
                    13 => calls[position].accounts[6] = accounts[7],
                    14 => calls[position].accounts[8] = accounts[0],
                    15 => calls[position].accounts[12] = accounts[0],
                    16 => calls[position].accounts[14] = accounts[0],
                    17 => calls[position].accounts[13] = accounts[0],
                    18 => calls[position].accounts[15] = accounts[0],
                    19 => calls[position].accounts[16] = accounts[0],
                    20 => calls[position + 1].accounts[2] = accounts[12],
                    21 => calls[position + 2].accounts[2] = accounts[8],
                    22 => calls[position + 1].accounts[1] = accounts[0],
                    23 => calls[position + 2].accounts[1] = accounts[0],
                    24 => calls[position + 3].accounts[1] = accounts[0],
                    25 => calls[position + 3].accounts[0] = accounts[0],
                    // This supported variant deliberately rejects nonzero referral receipts.
                    26 => calls[position + 3].data[1..9].copy_from_slice(&1u64.to_le_bytes()),
                    27 => {
                        calls.remove(position + 2);
                    }
                    28 => {
                        let extra = calls[position + 2].clone();
                        calls.insert(position + 3, extra);
                    }
                    29 => calls[position + 2].stack_height = Some(4),
                    30 => calls[position + 2].data[1..9].copy_from_slice(&0u64.to_le_bytes()),
                    31 => calls[position + 2].program_id_index = calls[position].program_id_index,
                    32 => calls.swap(position + 1, position + 2),
                    33..=35 => {
                        let own = format!("Program {} success", base58::encode(PROGRAM_ID));
                        let parent = format!("Program {} success", base58::encode(substreams_solana_idls::jupiter::v4::PROGRAM_ID));
                        let invoke = meta
                            .log_messages
                            .iter()
                            .position(|log| log == &format!("Program {} invoke [2]", base58::encode(PROGRAM_ID)))
                            .unwrap();
                        if mode == 34 {
                            let end = meta
                                .log_messages
                                .iter()
                                .enumerate()
                                .skip(invoke)
                                .find(|(_, log)| **log == parent)
                                .map(|(i, _)| i)
                                .unwrap();
                            meta.log_messages[end] = parent.replace(" success", " failed: custom program error: 1");
                        } else {
                            let end = meta
                                .log_messages
                                .iter()
                                .enumerate()
                                .skip(invoke)
                                .find(|(_, log)| {
                                    **log
                                        == if mode == 33 {
                                            own.clone()
                                        } else {
                                            format!("Program {} success", base58::encode(substreams_solana_idls::spl::token::PROGRAM_ID))
                                        }
                                })
                                .map(|(i, _)| i)
                                .unwrap();
                            meta.log_messages.truncate(end);
                        }
                    }
                    _ => calls[position + 3].accounts[2] = accounts[8],
                }
                assert!(
                    crate::process_transaction(altered).map_or(true, |decoded| { decoded.swaps.iter().all(|swap| swap.amm.as_slice() != PROGRAM_ID) }),
                    "Oasis case {case}, mutation {mode}"
                );
            }
        }
    }
}
