use common::db::{instruction_key, set_authority, set_clock, set_spl_token_instruction_v2, set_spl_token_transaction_v2};
use proto::pb::solana::spl::token::v1 as pb;
use substreams::pb::substreams::Clock;
use substreams_solana::base58;

pub fn process_events(tables: &mut substreams_database_change::tables::Tables, clock: &Clock, events: &pb::Events) {
    for transaction in &events.transactions {
        for instruction in &transaction.instructions {
            match &instruction.instruction {
                // Transfers
                Some(pb::instruction::Instruction::Transfer(data)) => {
                    handle_transfer(tables, clock, transaction, instruction, data, &data.source, &data.destination);
                }
                // Table convention: a mint row has the mint as source...
                Some(pb::instruction::Instruction::Mint(data)) => {
                    handle_transfer(
                        tables,
                        clock,
                        transaction,
                        instruction,
                        data,
                        data.mint.as_deref().unwrap_or_default(),
                        &data.destination,
                    );
                }
                // ...and a burn row has it as destination.
                Some(pb::instruction::Instruction::Burn(data)) => {
                    handle_transfer(
                        tables,
                        clock,
                        transaction,
                        instruction,
                        data,
                        &data.source,
                        data.mint.as_deref().unwrap_or_default(),
                    );
                }
                _ => {}
            }
        }
    }
}

fn handle_transfer(
    tables: &mut substreams_database_change::tables::Tables,
    clock: &Clock,
    transaction: &pb::Transaction,
    instruction: &pb::Instruction,
    data: &pb::Transfer,
    source: &[u8],
    destination: &[u8],
) {
    // Self transfers and zero amounts are kept; positions are the original ones.
    let decimals_raw = data.decimals.map(|d| d.to_string()).unwrap_or_default();
    let key = instruction_key(
        clock,
        transaction.transaction_index,
        instruction.instruction_index,
        instruction.inner_instruction_index,
    );
    let row = tables
        .create_row("spl_transfer", key)
        .set("source", base58::encode(source))
        .set("destination", base58::encode(destination))
        .set("amount", data.amount)
        // empty when unknown (unchecked Transfer)
        .set("mint", data.mint.as_ref().map(base58::encode).unwrap_or_default())
        // -- SPL Token-2022 --
        .set("decimals_raw", decimals_raw);

    set_authority(&data.authority, &data.multisig_authority, row);
    set_spl_token_instruction_v2(instruction, row);
    set_spl_token_transaction_v2(transaction, row);
    set_clock(clock, row);
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use spl_token_common::{process_block, SOLANA_TOKEN_PROGRAM_KEG, SOLANA_TOKEN_PROGRAM_ZQB};
    use std::collections::HashMap;
    use std::io::Read;
    use substreams::buffa::{Message, MessageField};
    use substreams_database_change::pb::sf::substreams::sink::database::v1::table_change::PrimaryKey;
    use substreams_solana::pb::sf::solana::r#type::v1::Block;

    const BLOCK_350000026: &[u8] = include_bytes!("../../spl/transfers/tests/fixtures/solana-mainnet-350000026.pb.gz");

    fn is_spl_token_program(program_id: &[u8]) -> bool {
        program_id == SOLANA_TOKEN_PROGRAM_KEG || program_id == SOLANA_TOKEN_PROGRAM_ZQB
    }

    /// spl_transfer rows by (transaction_index, instruction_index, inner_instruction_index).
    fn rows(events: &pb::Events) -> HashMap<(u32, u32, u32), HashMap<String, String>> {
        let mut clock = Clock {
            id: "block".to_string(),
            number: 350000026,
            timestamp: MessageField::some(Default::default()),
        };
        clock.timestamp.get_or_insert_default().seconds = 1751210691;
        let mut tables = substreams_database_change::tables::Tables::new();
        process_events(&mut tables, &clock, events);
        let mut rows = HashMap::new();
        for change in tables.to_database_changes().table_changes {
            assert_eq!(change.table, "spl_transfer");
            let Some(PrimaryKey::CompositePk(pk)) = change.primary_key else {
                panic!("composite key expected")
            };
            let key = |name: &str| pk.keys[name].parse::<u32>().unwrap();
            let fields = change.fields.into_iter().map(|field| (field.name, field.value)).collect();
            rows.insert((key("transaction_index"), key("instruction_index"), key("inner_instruction_index")), fields);
        }
        rows
    }

    #[test]
    fn one_row_per_operation_at_its_original_position() {
        let mut bytes = Vec::new();
        GzDecoder::new(BLOCK_350000026).read_to_end(&mut bytes).unwrap();
        let block = Block::decode_from_slice(bytes.as_slice()).unwrap();
        let events = process_block(&block, is_spl_token_program).unwrap();

        let mut expected = Vec::new();
        for transaction in &events.transactions {
            for instruction in &transaction.instructions {
                if let Some(pb::instruction::Instruction::Transfer(_) | pb::instruction::Instruction::Mint(_) | pb::instruction::Instruction::Burn(_)) =
                    &instruction.instruction
                {
                    let inner = instruction.inner_instruction_index.map_or(0, |index| index + 1);
                    expected.push((transaction.transaction_index, instruction.instruction_index, inner));
                }
            }
        }
        let rows = rows(&events);
        // No two operations share a primary key (Tables would merge them).
        assert_eq!(rows.len(), expected.len());
        assert!(expected.iter().all(|key| rows.contains_key(key)));

        // Inner self transfer at 1335.0.6, kept.
        let row = &rows[&(1335, 0, 7)];
        assert_eq!(row["source"], row["destination"]);
        assert_eq!(row["amount"], "4051");

        // Top-level TransferChecked at 776.3, whose mint is a loaded address.
        let row = &rows[&(776, 3, 0)];
        assert_eq!(row["mint"], "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(row["destination"], "2am4n8dNv4HrVUVpL4SFqmdzfRznyxrBx3SXwp8WZ9pR");
        assert_eq!(row["authority"], "EsF1h2qZ84aWSPrWB2nVemqFxquYDi1RBa3aMqhqfcjJ");
        assert_eq!(row["decimals_raw"], "6");

        // Unchecked transfers: mint unknown, destination is not the authority.
        let unchecked: Vec<_> = rows.values().filter(|row| row["decimals_raw"].is_empty() && row["mint"].is_empty()).collect();
        assert!(!unchecked.is_empty());
        assert!(unchecked.iter().all(|row| row["destination"] != row["authority"]));
    }
}
