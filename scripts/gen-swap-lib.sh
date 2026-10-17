#!/usr/bin/env bash
# Generate a transaction wrapper; protocol extraction still needs implementation.
# Usage: ./scripts/gen-swap-lib.sh <crate_name> <idl_module> <instruction_enum> <swap_variant> <proto_path>
set -euo pipefail
set -o noclobber
if [[ $# -ne 5 ]]; then
    echo "Usage: $0 <crate_name> <idl_module> <instruction_enum> <swap_variant> <proto_path>" >&2
    exit 2
fi
CRATE=$1
IDL=$2
ENUM=$3
SWAP=$4
PROTO=$5
[[ "$CRATE" =~ ^[a-z][a-z0-9_-]*$ ]] || { echo "Invalid crate name" >&2; exit 2; }
[[ "$IDL" =~ ^[a-z_][a-z0-9_]*(::[a-z_][a-z0-9_]*)*$ ]] || { echo "Invalid IDL module" >&2; exit 2; }
[[ "$ENUM" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || { echo "Invalid instruction enum" >&2; exit 2; }
[[ "$PROTO" =~ ^[a-z_][a-z0-9_]*(::[a-z_][a-z0-9_]*)*$ ]] || { echo "Invalid protobuf module" >&2; exit 2; }
[[ -n "$SWAP" && "$SWAP" != *$'\n'* && "$SWAP" != *$'\r'* ]] || { echo "Invalid swap variant" >&2; exit 2; }
BASE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
SOURCE="$BASE/dex/$CRATE/src/lib.rs"
[[ ! -e "$SOURCE" && ! -L "$SOURCE" ]] || { echo "Refusing to overwrite $SOURCE" >&2; exit 1; }
[[ -f "$BASE/dex/$CRATE/Cargo.toml" ]] || { echo "Create the crate with gen-dex.sh first." >&2; exit 1; }
mkdir -p "$BASE/dex/$CRATE/src"

cat > "$SOURCE" << EOF
use common::solana::{get_fee_payer, get_signers};
use proto::pb::${PROTO} as pb;
use substreams::errors::Error;
use substreams_solana::{
    block_view::InstructionView,
    pb::sf::solana::r#type::v1::{Block, ConfirmedTransaction},
};
use substreams_solana_idls::${IDL};

#[substreams::handlers::map]
fn map_events(block: Block) -> Result<pb::Events, Error> {
    Ok(pb::Events {
        transactions: block.transactions_owned().filter_map(process_transaction).collect(),
    })
}

fn process_transaction(tx: ConfirmedTransaction) -> Option<pb::Transaction> {
    let tx_meta = tx.meta.as_option()?;
    let transaction = tx.transaction.as_option()?;
    transaction.message.as_option()?.header.as_option()?;
    let signature = transaction.signatures.first()?.clone();
    let fee_payer = get_fee_payer(&tx)?;
    let signers = get_signers(&tx)?;
    let instructions: Vec<pb::Instruction> = tx.walk_instructions().filter_map(|iview| process_instruction(&iview)).collect();
    if instructions.is_empty() { return None; }
    Some(pb::Transaction {
        fee: tx_meta.fee,
        compute_units_consumed: tx_meta.compute_units_consumed.unwrap_or_default(),
        signature,
        fee_payer,
        signers,
        instructions,
        logs: vec![],
    })
}
EOF

printf 'Generated %s; implement process_instruction with %s::%s and reviewed variant %s before building.\n' "$SOURCE" "$IDL" "$ENUM" "$SWAP"
