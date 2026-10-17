mod system;

use common::solana::{get_fee_payer, get_signers};
use proto::pb::solana::native::token::v1 as pb;
use substreams::errors::Error;
use substreams_solana::pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction};

pub const SYSTEM_PROGRAM: [u8; 32] = [0; 32];

pub fn is_system_program(program_id: &[u8]) -> bool {
    program_id == &SYSTEM_PROGRAM
}

#[substreams::handlers::map]
fn map_events(block: Block) -> Result<pb::Events, Error> {
    Ok(pb::Events {
        transactions: block.transactions_owned().filter_map(process_transaction).collect(),
    })
}

fn process_transaction(tx: ConfirmedTransaction) -> Option<pb::Transaction> {
    let tx_meta = tx.meta.as_option()?;
    let resolved_accounts = tx.resolved_accounts();

    let create_balances = |balances: &[u64], tag: &str| -> Vec<pb::Balance> {
        if balances.len() != resolved_accounts.len() {
            substreams::log::info!(
                "Skipping {tag} balances update: {tag}_balances invalid length {} != {}",
                balances.len(),
                resolved_accounts.len()
            );
            return Vec::new();
        }
        balances
            .iter()
            .zip(resolved_accounts.iter())
            .map(|(&amount, account)| pb::Balance {
                account: account.to_vec(),
                amount,
            })
            .collect()
    };

    let instructions: Vec<_> = tx
        .walk_instructions()
        .filter_map(|iview| {
            let program_id = iview.program_id().0;
            if !is_system_program(&program_id) {
                return None;
            }

            system::unpack_transfers(&iview).map(|instruction| pb::Instruction {
                program_id: program_id.to_vec(),
                stack_height: iview.stack_height(),
                is_root: iview.is_root(),
                instruction: Some(instruction),
            })
        })
        .collect();

    let post_balances = create_balances(&tx_meta.post_balances, "post");
    let pre_balances = create_balances(&tx_meta.pre_balances, "pre");

    if instructions.is_empty() && post_balances.is_empty() && pre_balances.is_empty() {
        return None;
    }

    Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed.unwrap_or_default(),
        signature: tx.hash().to_vec(),
        fee_payer: get_fee_payer(&tx).unwrap_or_default(),
        signers: get_signers(&tx).unwrap_or_default(),
        instructions,
        post_balances,
        pre_balances,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams::buffa::{Message as _, MessageField};
    use substreams_solana::pb::sf::solana::r#type::v1 as sol;

    fn transfer_transaction() -> ConfirmedTransaction {
        ConfirmedTransaction {
            transaction: sol::Transaction {
                signatures: vec![vec![0x33; 64]],
                message: sol::Message {
                    header: sol::MessageHeader {
                        num_required_signatures: 1,
                        ..Default::default()
                    }
                    .into(),
                    account_keys: vec![vec![0x11; 32], vec![0x22; 32], SYSTEM_PROGRAM.to_vec()],
                    instructions: vec![sol::CompiledInstruction {
                        program_id_index: 2,
                        accounts: vec![0, 1],
                        // SystemInstruction::Transfer, fixed-width bincode, 42 lamports.
                        data: vec![2, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0],
                    }],
                    ..Default::default()
                }
                .into(),
            }
            .into(),
            meta: sol::TransactionStatusMeta {
                pre_balances: vec![100, 0, 0],
                post_balances: vec![58, 42, 0],
                ..Default::default()
            }
            .into(),
        }
    }

    #[test]
    fn native_transfer_preserves_legacy_oneof_wire_and_optional_scalar_default() {
        let decoded = process_transaction(transfer_transaction()).expect("decoded transfer");
        assert_eq!(decoded.compute_units_consumed, 0);
        assert_eq!(decoded.instructions.len(), 1);

        // Existing native-token.proto wire: program_id=1, is_root=3,
        // transfer=10 with source=1, destination=2, lamports=3.
        let mut expected = vec![0x0a, 32];
        expected.extend_from_slice(&SYSTEM_PROGRAM);
        expected.extend_from_slice(&[0x18, 1, 0x52, 70, 0x0a, 32]);
        expected.extend_from_slice(&[0x11; 32]);
        expected.extend_from_slice(&[0x12, 32]);
        expected.extend_from_slice(&[0x22; 32]);
        expected.extend_from_slice(&[0x18, 42]);
        assert_eq!(decoded.instructions[0].encode_to_vec(), expected);
        assert_eq!(pb::Instruction::decode_from_slice(&expected).unwrap(), decoded.instructions[0]);
    }

    #[test]
    fn missing_metadata_still_rejects_a_transaction_with_valid_transfer_bytes() {
        let mut tx = transfer_transaction();
        tx.meta = MessageField::none();
        assert!(process_transaction(tx).is_none());
    }
}
