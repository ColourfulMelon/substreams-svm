//! Replay unmodified public transaction fixtures through the deployed decoder.
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

fn main() {
    let directory = std::env::args().nth(1).expect("fixture directory required");
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
        let decoded = dex_swaps::process_transaction(tx);
        println!(
            "TX\t{}\t{}\t{}",
            signature,
            decoded.as_ref().map_or(0, |tx| tx.swaps.len()),
            decoded.as_ref().map_or(0, |tx| tx.rejected_swaps.len())
        );
        if let Some(decoded) = decoded {
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
                    swap.source_transfer_index.unwrap(),
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
