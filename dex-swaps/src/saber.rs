//! Saber StableSwap's canonical spot exchange. Deposit/withdraw instructions
//! are excluded even when they transfer both pool mints.
//! Source: saber-hq/stable-swap@39d5d4e0e844a009c83d37a3ce63d219fdba0bff.
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{block_view::InstructionView};
use substreams_solana_macro::b58;

pub(crate) const PROGRAM_ID: [u8; 32] = b58!("SSwpkEEcbUqx4vtoEByFjSkhKdCT862DNVb52nZg1UZ");

pub(crate) fn layout(ix: &InstructionView) -> Option<crate::native_venues::Layout> {
    let data = ix.data();
    let a = ix.accounts();
    if ix.program_id().0.as_slice() != PROGRAM_ID || a.len() != 9
        || data.len() != 17 || data[0] != 1
        || a[8].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
        || a[..8].iter().enumerate().any(|(i, key)| a[..i].iter().any(|other| other.0 == key.0))
    { return None; }
    let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
    (amount > 0).then_some(crate::native_venues::Layout {
        protocol: Protocol::Saber, pool: 0, authority: 2, users: [3, 6], vaults: [4, 5],
        amount, minimum_output: u64::from_le_bytes(data[9..17].try_into().ok()?),
        exact_input: true, input_limit: false, input_first: true, fixed_users: true,
    })
}

/// The processor sends exactly three classic-token CPIs, in this order:
/// user's input, user's principal output, and the pool's separate admin fee.
/// The admin fee can be zero; it is never added to the user's receipt.
/// Callers still require committed invocations and mint/net receipt evidence.
pub(crate) fn settlement(ix: &InstructionView, children: &[InstructionView]) -> Option<(u64, u64)> {
    let layout = layout(ix)?;
    let a = ix.accounts();
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if children.len() != 3 || depth == 0 { return None; }
    let mut amounts = [0; 3];
    for (n, child) in children.iter().enumerate() {
        let data = child.data();
        let accounts = child.accounts();
        let roles = match n { 0 => [3, 4, 2], 1 => [5, 6, 1], _ => [5, 7, 1] };
        if child.is_root() || child.stack_height() != depth + 1
            || child.program_id().0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
            || accounts.len() != 3 || data.len() != 9 || data[0] != 3
            || accounts.iter().zip(roles).any(|(key, role)| key.0 != a[role].0)
        { return None; }
        amounts[n] = u64::from_le_bytes(data[1..9].try_into().ok()?);
    }
    (amounts[0] == layout.amount && amounts[1] > 0 && amounts[1] >= layout.minimum_output)
        .then_some((amounts[0], amounts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

    #[test]
    fn finalized_saber_principal_receipts_bind_fee_roles_and_commitment() {
        for (raw, source, input, output) in [
            (include_bytes!("../fixtures/swap-coverage/saber-native/0.transaction.pb").as_slice(), 8, 488_556_414, 488_042_990),
            (include_bytes!("../fixtures/swap-coverage/saber-native/1.transaction.pb").as_slice(), 8, 1_178_879_500, 1_177_628_834),
            (include_bytes!("../fixtures/swap-coverage/saber-native/2.transaction.pb").as_slice(), 5, 655_747, 594_358),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&raw.to_vec()).unwrap();
            let walk: Vec<_> = tx.walk_instructions().collect();
            assert_eq!(settlement(&walk[source], &walk[source + 1..source + 4]), Some((input, output)));
            let native_data = walk[source].data().clone();
            let swaps = crate::process_transaction(tx.clone()).unwrap().swaps;
            let swaps: Vec<_> = swaps.iter().filter(|s| s.source_index == Some(source as u32)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!(swaps[0].protocol, Protocol::Saber);
            assert_eq!((swaps[0].input_amount, swaps[0].output_amount), (input, output));
            assert!(swaps[0].transfer_verified);
            for mode in 0..21 {
                let mut changed = tx.clone();
                let (group, position) = changed.meta.as_option().unwrap().inner_instructions.iter().enumerate()
                    .find_map(|(g, xs)| xs.instructions.iter().position(|x| x.data == native_data).map(|p| (g, p))).unwrap();
                let meta = changed.meta.as_option_mut().unwrap();
                let calls = &mut meta.inner_instructions[group].instructions;
                let a = calls[position].accounts.clone();
                match mode {
                    0 => calls[position].data[0] = 2,
                    1 => calls[position].data.push(0),
                    2 => calls[position].data[1] ^= 1,
                    3 => calls[position].data[9..17].copy_from_slice(&(output + 1).to_le_bytes()),
                    4 => calls[position].accounts[8] = a[0],
                    5 => calls[position].accounts[0] = a[1],
                    6 => calls[position].accounts[7] = a[6],
                    7 => calls[position + 1].accounts[0] = a[6],
                    8 => calls[position + 1].accounts[1] = a[5],
                    9 => calls[position + 1].accounts[2] = a[1],
                    10 => calls[position + 2].accounts[0] = a[4],
                    11 => calls[position + 2].accounts[1] = a[7],
                    12 => calls[position + 2].accounts[2] = a[2],
                    13 => calls[position + 3].accounts[0] = a[4],
                    14 => calls[position + 3].accounts[1] = a[6],
                    15 => calls[position + 3].accounts[2] = a[2],
                    16 => calls[position + 2].stack_height = Some(walk[source].stack_height() + 2),
                    17 => { let extra = calls[position + 1].clone(); calls.insert(position + 4, extra); }
                    18 => calls[position + 2].data[1..9].copy_from_slice(&0u64.to_le_bytes()),
                    19 => meta.err = buffa::MessageField::some(Default::default()),
                    _ => meta.log_messages.iter_mut().filter(|s| **s == format!("Program {} success", substreams_solana::base58::encode(PROGRAM_ID)))
                        .for_each(|s| *s = format!("Program {} failed: rejected swap", substreams_solana::base58::encode(PROGRAM_ID))),
                }
                assert!(crate::process_transaction(changed).map_or(true, |d| d.swaps.iter().all(|s| s.source_index != Some(source as u32))), "invalid settlement {mode}");
            }
        }
    }
}
