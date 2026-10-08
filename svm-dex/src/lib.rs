mod backfill;
use common::db::{common_key_v2, set_clock};
use proto::pb::dex::swaps::v1 as pb;
use substreams::{errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_solana::base58;

fn protocol_slug(protocol: i32) -> &'static str {
    match pb::Protocol::try_from(protocol).unwrap_or(pb::Protocol::Unspecified) {
        pb::Protocol::Unspecified => "unspecified",
        pb::Protocol::Boop => "boop",
        pb::Protocol::Byreal => "byreal",
        pb::Protocol::Darklake => "darklake",
        pb::Protocol::Dumpfun => "dumpfun",
        pb::Protocol::JupiterV4 => "jupiter_v4",
        pb::Protocol::JupiterV6 => "jupiter_v6",
        pb::Protocol::MeteoraAmm => "meteora_amm",
        pb::Protocol::MeteoraDaam => "meteora_daam",
        pb::Protocol::MeteoraDlmm => "meteora_dlmm",
        pb::Protocol::Moonshot => "moonshot",
        pb::Protocol::OkxDex => "okx_dex",
        pb::Protocol::GoonfiV2 => "goonfi_v2",
        pb::Protocol::Bisonfi => "bisonfi",
        pb::Protocol::Manifest => "manifest",
        pb::Protocol::Humidifi => "humidifi",
        pb::Protocol::OrcaV2 => "orca_v2",
        pb::Protocol::Alphaq => "alphaq",
        pb::Protocol::Kipseli => "kipseli",
        pb::Protocol::Flux => "flux",
        pb::Protocol::Scorch => "scorch",
        pb::Protocol::Obsidian => "obsidian",
        pb::Protocol::Tesserav => "tesserav",
        pb::Protocol::Deriverse => "deriverse",
        pb::Protocol::Zerofi => "zerofi",
        pb::Protocol::OrcaWhirlpool => "orca_whirlpool",
        pb::Protocol::Pancakeswap => "pancakeswap",
        pb::Protocol::SplTokenSwap => "spl_token_swap",
        pb::Protocol::Pumpfun => "pumpfun",
        pb::Protocol::PumpfunAmm => "pumpfun_amm",
        pb::Protocol::RaydiumAmmV4 => "raydium_amm_v4",
        pb::Protocol::RaydiumClmm => "raydium_clmm",
        pb::Protocol::RaydiumCpmm => "raydium_cpmm",
        pb::Protocol::RaydiumLaunchpad => "raydium_launchpad",
    }
}

#[substreams::handlers::map]
pub fn db_out(clock: Clock, swaps: pb::Events) -> Result<DatabaseChanges, Error> {
    write_swaps(clock, swaps)
}

fn write_swaps(clock: Clock, swaps: pb::Events) -> Result<DatabaseChanges, Error> {
    let mut tables = substreams_database_change::tables::Tables::new();

    if clock.number > u32::MAX as u64 {
        return Err(Error::msg("block number exceeds order-key bounds"));
    }
    for (ordinal, transaction) in swaps.transactions.iter().enumerate() {
        let transaction_index = transaction.source_index.map(|index| index as usize).unwrap_or(ordinal);
        if transaction_index >= 65536 {
            return Err(Error::msg("transaction position exceeds order-key bounds"));
        }
        for (ordinal, swap) in transaction.swaps.iter().enumerate() {
            let instruction_index = swap.source_transfer_index.map(|index| index as usize).unwrap_or(ordinal);
            if instruction_index >= 65536 {
                return Err(Error::msg("instruction position exceeds order-key bounds"));
            }
            let key = common_key_v2(&clock, transaction_index, instruction_index);
            let event_id = match (swap.source_index, swap.source_transfer_index) {
                (Some(instruction), Some(transfer)) => format!("{}:{}:{}", base58::encode(&transaction.signature), instruction, transfer),
                _ => String::new(),
            };
            let signers_raw = transaction.signers.iter().map(base58::encode).collect::<Vec<_>>().join(",");
            let row = tables
                .create_row("swaps", key)
                // Transaction
                .set("signature", base58::encode(&transaction.signature))
                .set("fee_payer", base58::encode(&transaction.fee_payer))
                .set("signers_raw", signers_raw)
                .set("fee", transaction.fee)
                .set("compute_units_consumed", transaction.compute_units_consumed)
                .set("program_id", base58::encode(&swap.program_id))
                .set("stack_height", swap.stack_height)
                .set("event_id", event_id)
                .set("decoder_version", "v0.5.2-pluto.4")
                .set("source_instruction_index", swap.source_index.unwrap_or_default())
                .set("transfer_verified", u32::from(swap.transfer_verified))
                // Swap
                .set("protocol", protocol_slug(swap.protocol))
                .set("amm", base58::encode(&swap.amm))
                .set("amm_pool", base58::encode(&swap.amm_pool))
                .set("user", base58::encode(&swap.user))
                .set("input_mint", base58::encode(&swap.input_mint))
                .set("input_amount", swap.input_amount)
                .set("output_mint", base58::encode(&swap.output_mint))
                .set("output_amount", swap.output_amount);

            set_clock(&clock, row);
        }
    }

    // Emit every observed block, including those without swaps. Parent links
    // distinguish skipped Solana slots from a real ingestion gap after restart.
    let all_swaps = swaps.transactions.iter().flat_map(|tx| &tx.swaps);
    let rejected: Vec<_> = swaps.transactions.iter().flat_map(|tx| &tx.rejected_swaps).collect();
    let row = tables.create_row("blocks", [("block_num", clock.number.to_string())]);
    row.set("parent_slot", swaps.parent_slot)
        .set("parent_hash", swaps.parent_hash)
        .set("decoder_version", "v0.5.2-pluto.4")
        .set("verified_swaps", all_swaps.filter(|swap| swap.transfer_verified).count() as u32)
        .set(
            "quarantined_swaps",
            rejected
                .iter()
                .filter(|swap| swap.verification_failure == "pool_mints_or_amounts_not_verified")
                .count() as u32,
        )
        .set(
            "duplicate_wrappers",
            rejected.iter().filter(|swap| swap.verification_failure == "duplicate_native_cpi").count() as u32,
        )
        .set(
            "corrected_events",
            rejected
                .iter()
                .filter(|swap| swap.verification_failure == "event_amounts_replaced_by_exact_user_flows")
                .count() as u32,
        );
    set_clock(&clock, row);

    Ok(tables.to_database_changes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_database_change::pb::sf::substreams::sink::database::v1::table_change::PrimaryKey;

    #[test]
    fn rejected_rows_are_not_streamed_and_source_positions_survive() {
        let clock = Clock { number: 100, id: "block".into(), timestamp: Some(Default::default()) };
        let swap = pb::Swap { source_index: Some(30), source_transfer_index: Some(31), transfer_verified: true, ..Default::default() };
        let changes = write_swaps(clock, pb::Events {
            parent_slot: 98, parent_hash: "parent".into(),
            transactions: vec![pb::Transaction {
                source_index: Some(17), signature: vec![7;64], swaps: vec![swap.clone()],
                rejected_swaps: vec![pb::Swap { transfer_verified: false, verification_failure: "duplicate_native_cpi".into(), ..swap }],
                ..Default::default()
            }],
        }).unwrap();
        let financial = changes.table_changes.iter().filter(|row| row.table == "swaps").collect::<Vec<_>>();
        assert_eq!(financial.len(), 1);
        let Some(PrimaryKey::CompositePk(key)) = &financial[0].primary_key else { panic!("source key missing") };
        assert_eq!(key.keys["transaction_index"], "17");
        assert_eq!(key.keys["instruction_index"], "31");
        assert!(changes.table_changes.iter().all(|row| matches!(row.table.as_str(), "swaps" | "blocks")));
        assert_eq!(changes.table_changes.len(), 2);
        let block = changes.table_changes.iter().find(|row| row.table == "blocks").unwrap();
        assert!(block.fields.iter().any(|field| field.name == "parent_slot" && field.value == "98"));
        assert!(block.fields.iter().any(|field| field.name == "duplicate_wrappers" && field.value == "1"));
    }

    #[test]
    fn empty_swap_block_keeps_parent_chain_and_oversized_order_keys_fail() {
        let clock = Clock { number: 100, id: "block".into(), timestamp: Some(Default::default()) };
        assert_eq!(write_swaps(clock.clone(), pb::Events::default()).unwrap().table_changes[0].table, "blocks");
        let too_large = pb::Events { transactions: vec![pb::Transaction { source_index: Some(65536), ..Default::default() }], ..Default::default() };
        assert!(write_swaps(clock, too_large).is_err());
    }
}
