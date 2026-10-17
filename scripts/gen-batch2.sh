#!/usr/bin/env bash
# Historical one-off rewrite retired: it overwrote existing schemas and models.
set -euo pipefail
cat >&2 <<'EOF'
gen-batch2.sh is retired and does not modify files.
For a new schema use scripts/gen-swap-proto.sh; for an existing schema edit its
canonical .proto and run make -C proto protogen with the pinned Buffa generator.
Read docs/protobuf.md and docs/repo-navigation.md before changing decoder models.
EOF
exit 1
