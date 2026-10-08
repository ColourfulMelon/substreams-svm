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

    if clock.number > u32::MAX as u64 { return Err(Error::msg("block number exceeds order-key bounds")); }
    for (ordinal, transaction) in swaps.transactions.iter().enumerate() {
        let transaction_index = transaction.source_index.map(|index| index as usize).unwrap_or(ordinal);
        if transaction_index >= 65536 { return Err(Error::msg("transaction position exceeds order-key bounds")); }
        for (diagnostic, rows) in [(false, &transaction.swaps), (true, &transaction.rejected_swaps)] {
        for (ordinal, swap) in rows.iter().enumerate() {
            let instruction_index = if diagnostic { ordinal } else { swap.source_transfer_index.map(|index| index as usize).unwrap_or(ordinal) };
            if instruction_index >= 65536 { return Err(Error::msg("instruction position exceeds order-key bounds")); }
            let key = common_key_v2(&clock, transaction_index, instruction_index);
            let event_id = match (swap.source_index, swap.source_transfer_index) {
                (Some(instruction), Some(transfer)) => format!("{}:{}:{}", base58::encode(&transaction.signature), instruction, transfer),
                _ => String::new(),
            };
            let signers_raw = transaction.signers.iter().map(base58::encode).collect::<Vec<_>>().join(",");
            let row = tables
                .create_row(if diagnostic { "swap_diagnostics" } else { "swaps" }, key)
                // Transaction
                .set("signature", base58::encode(&transaction.signature))
                .set("fee_payer", base58::encode(&transaction.fee_payer))
                .set("signers_raw", signers_raw)
                .set("fee", transaction.fee)
                .set("compute_units_consumed", transaction.compute_units_consumed)
                .set("program_id", base58::encode(&swap.program_id))
                .set("stack_height", swap.stack_height)
                .set("event_id", event_id)
                .set("source_instruction_index", swap.source_index.unwrap_or_default())
                .set("transfer_verified", u32::from(swap.transfer_verified))
                .set("verification_failure", &swap.verification_failure)

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
    }

    if tables.all_row_count() > 0 {
        set_clock(&clock, tables.create_row("blocks", [("block_num", clock.number.to_string())]));
    }

    Ok(tables.to_database_changes())
}
