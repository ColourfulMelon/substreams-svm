#!/usr/bin/env bash
# Generate a new DEX crate scaffold; no protocol decoder or schema is inferred.
# Usage: ./scripts/gen-dex.sh <name> <desc> <program_id> <type>
# type: swap-only | swap-xy | buy-sell-events (documentation hint only)
set -euo pipefail
set -o noclobber
if [[ $# -ne 4 ]]; then
    echo "Usage: $0 <name> <desc> <program_id> <type>" >&2
    exit 2
fi
NAME=$1
DESC=$2
PROGRAM_ID=$3
TYPE=$4
[[ "$NAME" =~ ^[a-z][a-z0-9_-]*$ ]] || { echo "Invalid crate name" >&2; exit 2; }
[[ "$PROGRAM_ID" =~ ^[1-9A-HJ-NP-Za-km-z]{32,44}$ ]] || { echo "Invalid program ID spelling" >&2; exit 2; }
case "$TYPE" in swap-only|swap-xy|buy-sell-events) ;; *) echo "Invalid decoder shape" >&2; exit 2 ;; esac
[[ -n "$DESC" && "$DESC" != *$'\n'* && "$DESC" != *$'\r'* ]] || { echo "Description must be one line" >&2; exit 2; }
BASE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
[[ ! -e "$BASE/dex/$NAME" && ! -L "$BASE/dex/$NAME" ]] || { echo "Refusing to overwrite dex/$NAME" >&2; exit 1; }
MODULE=${NAME//-/_}
# JSON string quoting is valid in both TOML and YAML; do not interpolate raw prose.
QUOTED_DESC=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1], ensure_ascii=False))' "$DESC")
mkdir -p "$BASE/dex/$NAME/src"

# Cargo.toml
cat > "$BASE/dex/$NAME/Cargo.toml" << EOF
[package]
name = "$NAME"
description = $QUOTED_DESC
edition = { workspace = true }
version = { workspace = true }

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
substreams = { workspace = true }
substreams-solana = { workspace = true }
substreams-solana-idls = { workspace = true }
proto = { path = "../../proto" }
common = { path = "../../common" }
EOF

# substreams.yaml
cat > "$BASE/dex/$NAME/substreams.yaml" << EOF
specVersion: v0.1.0
package:
  name: $MODULE
  version: v0.1.0
  url: https://github.com/pinax-network/substreams-svm
  description: $QUOTED_DESC
  image: ../../image.png

imports:
  solana_common: ../../spkg/solana-common-v0.4.0.spkg

binaries:
  default:
    type: wasm/rust-v1+wasm-bindgen-shims
    file: ../../target/wasm32-unknown-unknown/release/$MODULE.wasm

protobuf:
  files:
    - v1/$NAME.proto
  importPaths:
    - ../../proto

modules:
  - name: map_events
    kind: map
    inputs:
      - map: solana_common:blocks_without_votes
    blockFilter:
      module: solana_common:program_ids_without_votes
      query:
        string: "program:$PROGRAM_ID"
    output:
      type: proto:$MODULE.v1.Events

network: solana
EOF

echo "Generated dex/$NAME"
