#!/usr/bin/env bash
# Create a new swap-only schema, then use the workspace's pinned Buffa plugin.
# Usage: ./scripts/gen-swap-proto.sh <name> [package_name]
set -euo pipefail
set -o noclobber
if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "Usage: $0 <name> [package_name]" >&2
    exit 2
fi
NAME=$1
PKG=${2:-${NAME//-/_}}
[[ "$NAME" =~ ^[a-z][a-z0-9_-]*$ ]] || { echo "Invalid schema name" >&2; exit 2; }
[[ "$PKG" =~ ^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$ ]] || { echo "Invalid protobuf package" >&2; exit 2; }
BASE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
SCHEMA="$BASE/proto/v1/$NAME.proto"
[[ ! -e "$SCHEMA" && ! -L "$SCHEMA" ]] || { echo "Refusing to overwrite $SCHEMA" >&2; exit 1; }
# A package already generated elsewhere must not be replaced by a second schema.
PACKAGE_PATTERN=${PKG//./\\.}
if grep -REq "^[[:space:]]*package[[:space:]]+$PACKAGE_PATTERN\.v1[[:space:]]*;" "$BASE/proto/v1"; then
    echo "Package $PKG.v1 already exists; edit its canonical schema deliberately." >&2
    exit 1
fi
command -v buf >/dev/null
command -v python3 >/dev/null

cat > "$SCHEMA" << EOF
syntax = "proto3";

package $PKG.v1;

message Events {
  repeated Transaction transactions = 1;
}

message Transaction {
  bytes signature = 1;
  bytes fee_payer = 2;
  repeated bytes signers = 3;
  uint64 fee = 4;
  uint64 compute_units_consumed = 5;
  repeated Instruction instructions = 6;
  repeated Log logs = 7;
}

message Instruction {
  bytes program_id = 1;
  uint32 stack_height = 2;
  oneof instruction {
    SwapInstruction swap = 3;
  }
}

message SwapInstruction {
  uint64 amount_in = 1;
  uint64 minimum_amount_out = 2;
}

message Log {
  bytes program_id = 1;
  uint32 invoke_depth = 2;
}
EOF

make -C "$BASE/proto" protogen
printf 'Generated %s and shared Buffa bindings; review the complete generated diff.\n' "$SCHEMA"
