use std::collections::HashMap;

use substreams_solana::{
    base58,
    pb::sf::solana::r#type::v1::{ConfirmedTransaction, TransactionStatusMeta},
};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) struct TokenMintLookup {
    mints: HashMap<Vec<u8>, Vec<u8>>,
}

impl TokenMintLookup {
    pub(crate) fn new(tx: &ConfirmedTransaction, tx_meta: &TransactionStatusMeta) -> Self {
        let accounts = tx.resolved_accounts();
        let mut mints = HashMap::new();

        for balance in tx_meta.pre_token_balances.iter().chain(tx_meta.post_token_balances.iter()) {
            let Some(account) = accounts.get(balance.account_index as usize) else {
                continue;
            };
            let Ok(mint) = base58::decode(&balance.mint) else {
                continue;
            };
            mints.insert((*account).clone(), mint);
        }

        // Accounts created and closed in this transaction are absent from both
        // balance snapshots. Their token initialization still records the mint.
        for instruction in tx.walk_instructions() {
            let program_id = instruction.program_id().0;
            if program_id != &token::PROGRAM_ID && program_id != &token_2022::PROGRAM_ID {
                continue;
            }

            let data = instruction.data();
            // InitializeAccount, InitializeAccount2, and InitializeAccount3
            // all place the token account and mint in the first two slots.
            if data.as_slice() != [1] && !(matches!(data.first(), Some(16 | 18)) && data.len() == 33) {
                continue;
            }
            let accounts = instruction.accounts();
            if let (Some(account), Some(mint)) = (accounts.first(), accounts.get(1)) {
                // Observed balances remain authoritative when they exist.
                mints.entry(account.0.clone()).or_insert_with(|| mint.0.clone());
            }
        }

        Self { mints }
    }

    pub(crate) fn mint_for(&self, token_account: &[u8]) -> Option<Vec<u8>> {
        self.mints.get(token_account).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{CompiledInstruction, Message, MessageHeader, TokenBalance, Transaction};

    fn initialization(program: [u8; 32], data: Vec<u8>) -> ConfirmedTransaction {
        ConfirmedTransaction {
            transaction: Some(Transaction {
                message: Some(Message {
                    header: Some(MessageHeader {
                        num_required_signatures: 1,
                        ..Default::default()
                    }),
                    account_keys: vec![vec![7; 32], vec![8; 32], program.to_vec()],
                    instructions: vec![CompiledInstruction {
                        program_id_index: 2,
                        accounts: vec![0, 1],
                        data,
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            meta: Some(TransactionStatusMeta::default()),
        }
    }

    #[test]
    fn resolves_temporary_accounts_initialized_by_either_token_program() {
        for program in [token::PROGRAM_ID, token_2022::PROGRAM_ID] {
            for tag in [1, 16, 18] {
                let mut data = vec![tag];
                if tag != 1 {
                    data.extend_from_slice(&[9; 32]);
                }
                let tx = initialization(program, data);
                let lookup = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
                assert_eq!(lookup.mint_for(&[7; 32]), Some(vec![8; 32]));
            }
        }
    }

    #[test]
    fn does_not_infer_mints_from_other_programs_or_malformed_initializations() {
        for (program, data) in [(token::PROGRAM_ID, vec![18]), ([3; 32], vec![1])] {
            let tx = initialization(program, data);
            let lookup = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            assert!(lookup.mint_for(&[7; 32]).is_none());
        }
    }

    #[test]
    fn preserves_mints_observed_in_balance_snapshots() {
        let mut tx = initialization(token::PROGRAM_ID, vec![1]);
        tx.meta.as_mut().unwrap().post_token_balances.push(TokenBalance {
            account_index: 0,
            mint: base58::encode(&[10; 32]),
            ..Default::default()
        });
        let lookup = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
        assert_eq!(lookup.mint_for(&[7; 32]), Some(vec![10; 32]));
    }
}
