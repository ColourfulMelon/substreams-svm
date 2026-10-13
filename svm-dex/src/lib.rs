mod backfill;
use common::db::common_key_v2;
use proto::pb::dex::swaps::v1 as pb;
use substreams::{errors::Error, pb::substreams::Clock};
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_solana::base58;

const DECODER_VERSION: &str = "v0.5.2-pluto.29";

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
        pb::Protocol::MeteoraDbc => "meteora_dbc",
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
        pb::Protocol::Fluxbeam => "fluxbeam",
        pb::Protocol::PoseidonV2 => "poseidon_v2",
        pb::Protocol::Denali => "denali",
        pb::Protocol::Aquifer => "aquifer",
        pb::Protocol::BisonfiPredict => "bisonfi_predict",
        pb::Protocol::SolfiV2 => "solfi_v2",
        pb::Protocol::Saros => "saros",
        pb::Protocol::Plasma => "plasma",
        pb::Protocol::Bonkswap => "bonkswap",
        pb::Protocol::Heaven => "heaven",
        pb::Protocol::Stabble => "stabble",
        pb::Protocol::Defituna => "defituna",
        pb::Protocol::Invariant => "invariant",
        pb::Protocol::Onedex => "onedex",
        pb::Protocol::JupiterRfq => "jupiter_rfq",
        pb::Protocol::Phoenix => "phoenix",
        pb::Protocol::OpenbookV2 => "openbook_v2",
        pb::Protocol::Mercurial => "mercurial",
        pb::Protocol::Oasis => "oasis",
        pb::Protocol::JupiterLend => "jupiter_lend",
        pb::Protocol::Saber => "saber",
        pb::Protocol::Quantum => "quantum",
        pb::Protocol::Binaryfi => "binaryfi",
        pb::Protocol::Triangle => "triangle",
        pb::Protocol::Archer => "archer",
        pb::Protocol::Riptide => "riptide",
        pb::Protocol::Hadron => "hadron",
        pb::Protocol::Quay => "quay",
        pb::Protocol::Ghosty => "ghosty",
        pb::Protocol::Memefun => "memefun",
        pb::Protocol::Gator => "gator",
        pb::Protocol::JupiterPerps => "jupiter_perps",
        pb::Protocol::Numeraire => "numeraire",
        pb::Protocol::Futarchy => "futarchy",
        pb::Protocol::Guacswap => "guacswap",
        pb::Protocol::Omnipair => "omnipair",
        pb::Protocol::DrFraudsworth => "dr_fraudsworth",
        pb::Protocol::SanctumInfinity => "sanctum_infinity",
        pb::Protocol::Peg => "peg",
        pb::Protocol::AnchorSwap => "anchor_swap",
        pb::Protocol::RaydiumCpmmFork => "raydium_cpmm_fork",
        pb::Protocol::Janus => "janus",
        pb::Protocol::Exponent => "exponent",
        pb::Protocol::Cryptoquest => "cryptoquest",
        pb::Protocol::PumpfunAmmBuyBurn => "pumpfun_amm_buy_burn",
        pb::Protocol::AldrinV1 => "aldrin_v1",
        pb::Protocol::Cropper => "cropper",
        pb::Protocol::PiconDlmm => "picon_dlmm",
        pb::Protocol::M0NativeOrder => "m0_native_order",
        pb::Protocol::PatchBuyback => "patch_buyback",
        pb::Protocol::Bxlw => "bxlw",
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
        // Row identity positions live in two coordinate domains. For a
        // transaction containing runtime fills, rank all financial events by
        // their actual compiled invocation/transfer coordinate, then log order.
        // Wallet-only transactions retain their original instruction positions.
        let execution_positions = if transaction.swaps.iter().any(|swap| swap.source_log_index.is_some()) {
            if transaction.swaps.len() > 65536 {
                return Err(Error::msg("financial event count exceeds order-key bounds"));
            }
            let mut ordered: Vec<_> = transaction.swaps.iter().enumerate().collect();
            ordered.sort_by_key(|(ordinal, swap)| {
                (
                    swap.source_transfer_index
                        .or(swap.source_event_index)
                        .or(swap.source_index)
                        .unwrap_or(*ordinal as u32),
                    swap.source_log_index,
                )
            });
            let mut positions = vec![0u32; ordered.len()];
            for (position, (ordinal, _)) in ordered.into_iter().enumerate() {
                positions[ordinal] = position as u32;
            }
            Some(positions)
        } else {
            None
        };
        for (ordinal, swap) in transaction.swaps.iter().enumerate() {
            let burn_format = swap.protocol == pb::Protocol::PumpfunAmmBuyBurn as i32;
            if burn_format != swap.burn_output_verified
                || burn_format != swap.source_burn_index.is_some()
                || burn_format && (swap.transfer_verified || swap.orderbook_fill_verified
                    || swap.source_index.is_none() || swap.source_transfer_index.is_none()
                    || swap.source_event_index.is_some() || swap.source_log_index.is_some()
                    || swap.source_burn_index.is_some_and(|burn| burn >= 32768
                        || swap.source_transfer_index.unwrap().checked_add(1) != Some(burn))
                    || swap.source_index.unwrap().checked_add(1) != swap.source_transfer_index
                    || swap.input_amount == 0 || swap.output_amount == 0)
            {
                return Err(Error::msg("invalid buy-and-burn evidence"));
            }
            // Runtime log positions and compiled instruction positions are
            // different coordinate systems. Reserve the upper half of the
            // existing UInt16 key for logs; legacy transfer/event IDs stay exact.
            let instruction_index = if let Some(log) = swap.source_log_index {
                if log >= 32768
                    || !swap.orderbook_fill_verified
                    || swap.source_index.is_none()
                    || swap.source_transfer_index.is_some()
                    || swap.source_event_index.is_some()
                {
                    return Err(Error::msg("invalid runtime fill proof or log position"));
                }
                32768 + log as usize
            } else {
                let compiled = swap
                    .source_transfer_index
                    .or(swap.source_event_index)
                    .map(|index| index as usize)
                    .unwrap_or(ordinal);
                if compiled >= 32768 {
                    return Err(Error::msg("instruction position exceeds reserved order-key bounds"));
                }
                compiled
            };
            let key = common_key_v2(&clock, transaction_index, instruction_index);
            let event_id = match (swap.source_index, swap.source_transfer_index, swap.source_event_index, swap.source_log_index) {
                (Some(instruction), Some(transfer), _, None) => format!("{}:{}:{}", base58::encode(&transaction.signature), instruction, transfer),
                (Some(instruction), None, Some(event), None) if swap.orderbook_fill_verified => {
                    format!("{}:{}:ledger:{}", base58::encode(&transaction.signature), instruction, event)
                }
                (Some(instruction), None, None, Some(log)) if swap.orderbook_fill_verified => {
                    format!("{}:{}:log:{}", base58::encode(&transaction.signature), instruction, log)
                }
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
                .set("decoder_version", DECODER_VERSION)
                .set("source_instruction_index", swap.source_index.unwrap_or_default())
                .set(
                    "execution_position",
                    execution_positions.as_ref().map_or(instruction_index as u32, |positions| positions[ordinal]),
                )
                .set("transfer_verified", u32::from(swap.transfer_verified))
                .set("orderbook_fill_verified", u32::from(swap.orderbook_fill_verified))
                .set("burn_output_verified", u32::from(swap.burn_output_verified))
                .set("source_burn_index", swap.source_burn_index.unwrap_or_default())
                // Swap
                .set("protocol", protocol_slug(swap.protocol))
                .set("amm", base58::encode(&swap.amm))
                .set("amm_pool", base58::encode(&swap.amm_pool))
                .set("user", base58::encode(&swap.user))
                .set("input_mint", base58::encode(&swap.input_mint))
                .set("input_amount", swap.input_amount)
                .set("output_mint", base58::encode(&swap.output_mint))
                .set("output_amount", swap.output_amount);

            // Pluto's patched ClickHouse sink restores block_hash from the
            // composite key, including keys outside the SQL sorting key.
            row.set("block_num", clock.number)
                .set("timestamp", clock.timestamp.as_ref().expect("missing timestamp").seconds.to_string());
        }
    }

    // Emit every observed block, including those without swaps. Parent links
    // distinguish skipped Solana slots from a real ingestion gap after restart.
    let all_swaps = swaps.transactions.iter().flat_map(|tx| &tx.swaps);
    let rejected: Vec<_> = swaps.transactions.iter().flat_map(|tx| &tx.rejected_swaps).collect();
    let row = tables.create_row("blocks", [("block_num", clock.number.to_string())]);
    row.set("parent_slot", swaps.parent_slot)
        .set("parent_hash", swaps.parent_hash)
        .set("decoder_version", DECODER_VERSION)
        .set(
            "verified_swaps",
            all_swaps.filter(|swap| swap.transfer_verified || swap.orderbook_fill_verified || swap.burn_output_verified).count() as u32,
        )
        // Zero marks disabled live diagnostics, not an exhaustive coverage audit.
        .set("coverage_candidates", 0u32)
        .set("coverage_transactions", 0u32)
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
    // block_num is already present in the composite primary key.
    row.set("block_hash", &clock.id)
        .set("timestamp", clock.timestamp.as_ref().expect("missing timestamp").seconds.to_string());

    Ok(tables.to_database_changes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_database_change::pb::sf::substreams::sink::database::v1::table_change::PrimaryKey;

    #[test]
    fn buy_and_burn_preserves_destruction_provenance_and_rejects_mixed_receipts() {
        let clock = Clock { number: 100, id: "block".into(), timestamp: Some(Default::default()) };
        let swap = pb::Swap {
            protocol: pb::Protocol::PumpfunAmmBuyBurn as i32, source_index: Some(30),
            source_transfer_index: Some(31), source_burn_index: Some(32), burn_output_verified: true,
            input_amount: 100, output_amount: 200, ..Default::default()
        };
        let events = pb::Events { transactions: vec![pb::Transaction {
            signature: vec![7;64], swaps: vec![swap], ..Default::default()
        }], ..Default::default() };
        let changes = write_swaps(clock.clone(), events.clone()).unwrap();
        let row = changes.table_changes.iter().find(|row| row.table == "swaps").unwrap();
        for (name,value) in [("protocol","pumpfun_amm_buy_burn"),("transfer_verified","0"),
            ("burn_output_verified","1"),("source_burn_index","32")] {
            assert!(row.fields.iter().any(|field| field.name == name && field.value == value));
        }
        let block = changes.table_changes.iter().find(|row| row.table == "blocks").unwrap();
        assert!(block.fields.iter().any(|field| field.name == "verified_swaps" && field.value == "1"));
        for mode in 0..14 {
            let mut wrong = events.clone(); let swap = &mut wrong.transactions[0].swaps[0];
            match mode {
                0 => swap.burn_output_verified = false,
                1 => swap.source_burn_index = None,
                2 => swap.source_burn_index = Some(33),
                3 => swap.protocol = pb::Protocol::PumpfunAmm as i32,
                4 => swap.transfer_verified = true,
                5 => swap.orderbook_fill_verified = true,
                6 => swap.source_index = None,
                7 => swap.source_transfer_index = None,
                8 => swap.source_event_index = Some(34),
                9 => swap.source_log_index = Some(34),
                10 => swap.input_amount = 0,
                11 => swap.output_amount = 0,
                12 => swap.source_transfer_index = Some(u32::MAX),
                _ => swap.source_index = Some(u32::MAX),
            }
            assert!(write_swaps(clock.clone(), wrong).is_err(), "invalid burn receipt {mode}");
        }
    }

    #[test]
    fn rejected_rows_are_not_streamed_and_source_positions_survive() {
        let clock = Clock {
            number: 100,
            id: "block".into(),
            timestamp: Some(Default::default()),
        };
        let swap = pb::Swap {
            source_index: Some(30),
            source_transfer_index: Some(31),
            transfer_verified: true,
            ..Default::default()
        };
        let changes = write_swaps(
            clock,
            pb::Events {
                parent_slot: 98,
                parent_hash: "parent".into(),
                transactions: vec![pb::Transaction {
                    source_index: Some(17),
                    signature: vec![7; 64],
                    swaps: vec![swap.clone()],
                    rejected_swaps: vec![pb::Swap {
                        transfer_verified: false,
                        verification_failure: "duplicate_native_cpi".into(),
                        ..swap
                    }],
                    ..Default::default()
                }],
            },
        )
        .unwrap();
        let version = include_str!("../substreams.yaml").lines().find_map(|line| line.strip_prefix("  version: ")).unwrap();
        for manifest in [include_str!("../clickhouse/substreams.yaml"), include_str!("../../dex-swaps/substreams.yaml")] {
            assert!(manifest.lines().any(|line| line == format!("  version: {version}")));
        }
        assert!(changes.table_changes.iter().all(|row| row.fields.iter().any(|field| field.name == "decoder_version" && field.value == version)));
        let financial = changes.table_changes.iter().filter(|row| row.table == "swaps").collect::<Vec<_>>();
        assert_eq!(financial.len(), 1);
        let Some(PrimaryKey::CompositePk(key)) = &financial[0].primary_key else {
            panic!("source key missing")
        };
        assert_eq!(key.keys["transaction_index"], "17");
        assert_eq!(key.keys["instruction_index"], "31");
        assert!(changes.table_changes.iter().all(|row| matches!(row.table.as_str(), "swaps" | "blocks")));
        assert_eq!(changes.table_changes.len(), 2);
        let block = changes.table_changes.iter().find(|row| row.table == "blocks").unwrap();
        assert!(block.fields.iter().any(|field| field.name == "parent_slot" && field.value == "98"));
        assert!(block.fields.iter().any(|field| field.name == "duplicate_wrappers" && field.value == "1"));
    }

    #[test]
    fn ledger_fills_keep_distinct_event_identity_and_count_as_verified_without_token_transfers() {
        let clock = Clock {
            number: 100,
            id: "block".into(),
            timestamp: Some(Default::default()),
        };
        let signature = vec![7; 64];
        let changes = write_swaps(
            clock,
            pb::Events {
                transactions: vec![pb::Transaction {
                    signature: signature.clone(),
                    source_index: Some(17),
                    swaps: vec![pb::Swap {
                        protocol: 50,
                        source_index: Some(30),
                        source_event_index: Some(34),
                        orderbook_fill_verified: true,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let row = changes.table_changes.iter().find(|row| row.table == "swaps").unwrap();
        let Some(PrimaryKey::CompositePk(key)) = &row.primary_key else {
            panic!("missing key")
        };
        assert_eq!(key.keys["instruction_index"], "34");
        let event_id = format!("{}:30:ledger:34", base58::encode(signature));
        for (name, value) in [
            ("event_id", event_id.as_str()),
            ("transfer_verified", "0"),
            ("orderbook_fill_verified", "1"),
            ("protocol", "phoenix"),
        ] {
            assert!(row.fields.iter().any(|field| field.name == name && field.value == value));
        }
        let block = changes.table_changes.iter().find(|row| row.table == "blocks").unwrap();
        assert!(block.fields.iter().any(|field| field.name == "verified_swaps" && field.value == "1"));
    }

    #[test]
    fn runtime_fill_keys_do_not_collide_with_compiled_instruction_positions() {
        let clock = Clock {
            number: 100,
            id: "block".into(),
            timestamp: Some(Default::default()),
        };
        let runtime = pb::Swap {
            protocol: 51,
            source_index: Some(30),
            source_log_index: Some(34),
            orderbook_fill_verified: true,
            ..Default::default()
        };
        let events = pb::Events {
            transactions: vec![pb::Transaction {
                signature: vec![7; 64],
                source_index: Some(17),
                swaps: vec![
                    pb::Swap {
                        source_index: Some(30),
                        source_transfer_index: Some(34),
                        transfer_verified: true,
                        ..Default::default()
                    },
                    runtime.clone(),
                    pb::Swap {
                        source_log_index: Some(35),
                        ..runtime.clone()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let changes = write_swaps(clock.clone(), events.clone()).unwrap();
        let rows: Vec<_> = changes.table_changes.iter().filter(|row| row.table == "swaps").collect();
        let keys: Vec<_> = rows
            .iter()
            .map(|row| {
                let Some(PrimaryKey::CompositePk(key)) = &row.primary_key else {
                    panic!("key")
                };
                key.keys["instruction_index"].clone()
            })
            .collect();
        assert_eq!(keys, vec!["34", "32802", "32803"]);
        for (row, position) in rows.iter().zip(["2", "0", "1"]) {
            assert!(row.fields.iter().any(|field| field.name == "execution_position" && field.value == position));
        }
        for (row, suffix) in rows.iter().zip([":30:34", ":30:log:34", ":30:log:35"]) {
            assert!(row.fields.iter().any(|field| field.name == "event_id" && field.value.ends_with(suffix)));
        }
        for variant in 0..6 {
            let mut invalid = events.clone();
            let swap = &mut invalid.transactions[0].swaps[1];
            match variant {
                0 => swap.source_log_index = Some(32768),
                1 => swap.source_transfer_index = Some(34),
                2 => swap.source_event_index = Some(34),
                3 => swap.source_index = None,
                4 => swap.orderbook_fill_verified = false,
                _ => {
                    swap.source_log_index = None;
                    swap.source_transfer_index = Some(32768);
                }
            }
            assert!(write_swaps(clock.clone(), invalid).is_err(), "variant {variant}");
        }
    }

    #[test]
    fn empty_swap_block_keeps_parent_chain_and_oversized_order_keys_fail() {
        let clock = Clock {
            number: 100,
            id: "block".into(),
            timestamp: Some(Default::default()),
        };
        assert_eq!(write_swaps(clock.clone(), pb::Events::default()).unwrap().table_changes[0].table, "blocks");
        let too_large = pb::Events {
            transactions: vec![pb::Transaction {
                source_index: Some(65536),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(write_swaps(clock, too_large).is_err());
    }

    #[test]
    fn live_mapper_ignores_diagnostics_and_preserves_financial_rows() {
        let clock = Clock { number: 100, id: "block".into(), timestamp: Some(Default::default()) };
        let mut tx = pb::Transaction {
            source_index: Some(17), signature: vec![7; 64],
            swaps: vec![pb::Swap { input_amount: 100, output_amount: 200, ..Default::default() }],
            ..Default::default()
        };
        let mut clean = write_swaps(clock.clone(), pb::Events { transactions: vec![tx.clone()], ..Default::default() }).unwrap();
        // A stale upstream diagnostic must neither be streamed nor affect pricing.
        tx.coverage_transaction = vec![0; 32_768];
        tx.coverage_candidates = vec![pb::CoverageCandidate { source_index: 65536, ..Default::default() }];
        let mut diagnostic = write_swaps(clock.clone(), pb::Events { transactions: vec![tx.clone()], ..Default::default() }).unwrap();
        for row in clean.table_changes.iter_mut().chain(&mut diagnostic.table_changes) {
            row.fields.sort_by(|a, b| a.name.cmp(&b.name));
        }
        assert_eq!(clean, diagnostic);
        tx.coverage_transaction.clear();
        let mut missing_original = write_swaps(clock, pb::Events { transactions: vec![tx], ..Default::default() }).unwrap();
        for row in &mut missing_original.table_changes { row.fields.sort_by(|a, b| a.name.cmp(&b.name)); }
        assert_eq!(clean, missing_original);
        assert_eq!(clean.table_changes.len(), 2);
        for row in &clean.table_changes {
            let Some(PrimaryKey::CompositePk(key)) = &row.primary_key else { panic!("missing composite key") };
            assert!(row.fields.iter().all(|field| !key.keys.contains_key(&field.name)));
            let mut restored: std::collections::HashMap<_, _> = row.fields.iter().map(|f| (f.name.as_str(), f.value.as_str())).collect();
            restored.extend(key.keys.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            assert_eq!(restored["block_hash"], "block");
            assert_eq!(restored["block_num"], "100");
            assert_eq!(restored["decoder_version"], DECODER_VERSION);
            if row.table == "swaps" {
                assert_eq!(restored["input_amount"], "100");
                assert_eq!(restored["output_amount"], "200");
                assert_eq!(restored["transaction_index"], "17");
            } else {
                assert_eq!(row.table, "blocks");
                assert_eq!(restored["coverage_candidates"], "0");
                assert_eq!(restored["coverage_transactions"], "0");
            }
        }
    }

    #[test]
    fn finalized_cryptoquest_fills_preserve_runtime_log_database_identity() {
        use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
        let originals: &[(&[u8], u64, usize)] = &[
            (include_bytes!("../../dex-swaps/fixtures/swap-coverage/cryptoquest-live26/5q78v5Ri7c63U9dGgLnH7WEVTZVRinYGPPvtRteQaEekPQefdpNzVzZpEmxCkWPsfPGQMZ8ig5AiJ8Y4m8F9eYpz.pb"), 456163753, 2),
            (include_bytes!("../../dex-swaps/fixtures/swap-coverage/cryptoquest-live26/nWY5ryPesc9dY7jQn3ZvnpMN1zb4PipXteY6vuCojP3B8Mvck1n65TPRtbjZX9WtGjnt7erdQNxGQBYpwA9iv1H.pb"), 456164118, 1),
        ];
        for (wire, slot, count) in originals {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&wire.to_vec()).unwrap();
            let mut transaction = dex_swaps::process_transaction(tx).unwrap();
            transaction.source_index = Some(17);
            let fills: Vec<_> = transaction.swaps.iter().filter(|s| s.protocol == pb::Protocol::Cryptoquest as i32).cloned().collect();
            assert_eq!(fills.len(), *count);
            let signature = base58::encode(&transaction.signature);
            let changes = write_swaps(Clock { number: *slot, id: "retained-finalized-block".into(), timestamp: Some(Default::default()) },
                pb::Events { transactions: vec![transaction], ..Default::default() }).unwrap();
            let financial: Vec<_> = changes.table_changes.iter().filter(|row| row.table == "swaps").collect();
            assert_eq!(financial.len(), *count);
            for fill in fills {
                assert!(fill.orderbook_fill_verified && fill.source_transfer_index.is_none() && fill.source_event_index.is_none());
                let log = fill.source_log_index.unwrap();
                let id = format!("{}:{}:log:{}", signature, fill.source_index.unwrap(), log);
                let rows: Vec<_> = financial.iter().filter(|row| row.fields.iter().any(|f| f.name == "event_id" && f.value == id)).collect();
                assert_eq!(rows.len(), 1);
                let Some(PrimaryKey::CompositePk(key)) = &rows[0].primary_key else { panic!("missing runtime fill key") };
                assert_eq!(key.keys["instruction_index"], (32768 + log).to_string());
                assert!(rows[0].fields.iter().any(|f| f.name == "protocol" && f.value == "cryptoquest"));
                assert!(rows[0].fields.iter().any(|f| f.name == "input_amount" && f.value == fill.input_amount.to_string()));
                assert!(rows[0].fields.iter().any(|f| f.name == "output_amount" && f.value == fill.output_amount.to_string()));
            }
        }
    }
}
