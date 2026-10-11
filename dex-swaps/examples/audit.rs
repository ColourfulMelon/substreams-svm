//! Replay unmodified public transaction fixtures through the built decoder.
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

fn main() {
    let directory = std::env::args().nth(1).expect("fixture directory required");
    let phoenix_headers = std::env::args().nth(2).filter(|value| value != "-");
    let openbook_headers = std::env::args().nth(3).filter(|value| value != "-");
    // Fee files must come from an independently owner/epoch-verified read.
    // The audit never treats absent files as proof of zero transfer fees.
    let fee_mints = std::env::args().nth(4).filter(|value| value != "-");
    let fee_epoch = fee_mints.as_ref().map(|_| {
        std::env::args()
            .nth(5)
            .expect("explicit transaction epoch required for fee audit")
            .parse::<u64>()
            .expect("invalid fee audit epoch")
    });
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "pb"))
        .collect();
    files.sort();
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
        let signature = base58::encode(tx.hash());
        if let Some(directory) = &phoenix_headers {
            for (index, ix) in tx.walk_instructions().enumerate() {
                if ix.program_id().0.as_slice() != dex_swaps::phoenix::PROGRAM_ID {
                    continue;
                }
                let accounts = ix.accounts();
                if accounts.len() < 4 {
                    continue;
                }
                let path = std::path::Path::new(directory).join(format!("{}.bin", base58::encode(accounts[2].0)));
                let Ok(header) = std::fs::read(path) else {
                    continue;
                };
                let Some(market) = dex_swaps::phoenix::Market::from_owned_header(accounts[2].0, &dex_swaps::phoenix::PROGRAM_ID, &header) else {
                    continue;
                };
                if let Some(fill) = dex_swaps::phoenix::decode_fill(&tx, index, &market, None) {
                    println!(
                        "PHOENIX\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        signature,
                        fill.instruction_index,
                        fill.log_instruction_index,
                        fill.summary_event_index,
                        fill.slot,
                        base58::encode(fill.market),
                        base58::encode(fill.trader),
                        base58::encode(fill.input_mint),
                        fill.input_amount,
                        base58::encode(fill.output_mint),
                        fill.output_amount,
                        fill.fee_quote_atoms
                    );
                }
            }
        }
        let decoded = dex_swaps::process_transaction_with_metadata(
            tx,
            None,
            |address, _| {
                let directory = phoenix_headers.as_ref()?;
                let path = std::path::Path::new(directory).join(format!("{}.bin", base58::encode(address)));
                let header = std::fs::read(path).ok()?;
                dex_swaps::phoenix::Market::from_owned_header(address, &dex_swaps::phoenix::PROGRAM_ID, &header)
            },
            |address, _| {
                let directory = openbook_headers.as_ref()?;
                let path = std::path::Path::new(directory).join(format!("{}.bin", base58::encode(address)));
                let header = std::fs::read(path).ok()?;
                dex_swaps::openbook::Market::from_owned_header(address, &dex_swaps::openbook::PROGRAM_ID, &header)
            },
            |mint, _| {
                let directory = fee_mints.as_ref()?;
                let path = std::path::Path::new(directory).join(format!("{}.bin", base58::encode(mint)));
                let bytes = std::fs::read(path).ok()?;
                let (schedule, initialized) = dex_swaps::token_fees::owned_mint(&substreams_solana_idls::spl::token_2022::PROGRAM_ID, &bytes)?;
                if !initialized {
                    return None;
                }
                schedule.at(fee_epoch?)
            },
        );
        println!(
            "TX\t{}\t{}\t{}",
            signature,
            decoded.as_ref().map_or(0, |tx| tx.swaps.len()),
            decoded.as_ref().map_or(0, |tx| tx.rejected_swaps.len())
        );
        if let Some(decoded) = decoded {
            for candidate in &decoded.coverage_candidates {
                println!(
                    "COVERAGE\t{}\t{}\t{}\t{}\t{}",
                    signature,
                    candidate.source_index,
                    base58::encode(&candidate.program_id),
                    candidate.reason,
                    candidate.transfer_count
                );
            }
            for swap in &decoded.rejected_swaps {
                println!(
                    "REJECT\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    signature,
                    swap.protocol,
                    base58::encode(&swap.amm_pool),
                    base58::encode(&swap.input_mint),
                    swap.input_amount,
                    swap.output_amount,
                    swap.verification_failure
                );
            }
            for swap in decoded.swaps {
                println!(
                    "SWAP\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    signature,
                    swap.protocol,
                    swap.source_index.unwrap(),
                    swap.source_transfer_index.or(swap.source_event_index).or(swap.source_log_index).unwrap(),
                    base58::encode(swap.amm),
                    base58::encode(swap.amm_pool),
                    base58::encode(swap.user),
                    base58::encode(swap.input_mint),
                    swap.input_amount,
                    base58::encode(swap.output_mint),
                    swap.output_amount
                );
            }
        }
    }
}
