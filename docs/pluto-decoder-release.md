# Pluto DEX decoder release

`v0.5.2-pluto.25` combines the independently verified missing native swap
formats from the finalized coverage audit: Saber, Dooar and Dexlab SPL swap
forks, Quantum/BinaryFi, Triangle, Archer, Riptide, Hadron and Quay; Jupiter
Lend oracle-backed settlement; SolFi u64 direction forms; bounded Meteora
swap2 and legacy suffixes; and uniquely proven repeated/native Pump sells.
New protocol IDs 55–62 are appended without renumbering existing protocols.
Pricing uses actual committed principal receipts, verified net token outputs
and exact native payout allocation. Fee/referral payments remain separate.

Native bindings apply to routed candidates too. Unknown payloads, uncommitted
ancestry, ambiguous repeated payouts and unsupported fee states remain
rejected. Legacy classic Token Batch255 operations can contribute separately
validated transaction ledgers; they never create synthetic swap principals.
See the per-format validation notes linked in [the repository map](repo-navigation.md).
All 189 core decoder/mapper tests pass. Two clean packaged replays cover 4,565
finalized blocks, preserve all 633,846 prior swap identities, add 3,726 swaps
and correct 825 existing Pump outputs from gross payouts to actual net user
receipts. No prior identity disappears and no other old financial field changes.
Every added/corrected row independently matches all 25 financial/provenance
fields from original source transactions and finalized RPC metadata. Four table
versions, sealed counters, parent chains and unique operation ordinals pass.
The [final attestation](../dex-swaps/fixtures/swap-coverage/combined25/packaged-release-proof.json)
links the immutable comparator manifests to the completed independent proof;
compressed complete evidence and checksums accompany the release.
Finite replay evidence does not establish universal support for all current,
historical or future Solana swap programs.

IDL source is pinned to immutable revision
`785533d59d0b258500cbbce60890fe58b8c14405`. Existing protocol IDs remain unchanged.
The `.6` enum/projection migration is retained and extended; no history is
materialized or rewritten by it. Additive coverage tables retain original
candidate transactions and per-instruction identities for seven days. Candidate
records include non-swaps and never feed price views. Block counters persist.
Create all tables, columns and enum values before starting the new writer.

Build with Rust 1.85.1 and Substreams CLI 1.18.2, matching Pluto's SQL sink:

```sh
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked --no-default-features
cargo +1.85.1 build -p dex-swaps -p svm-dex --locked --no-default-features --target wasm32-unknown-unknown --release
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.25.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.25.spkg
make -C svm-dex/clickhouse schema
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.25.spkg
```

Stop the old writer gracefully before upgrading and preserve its exact durable
cursor. For this validated pricing-only upgrade with unchanged metadata bytes and hashes, copy that cursor row under the
new output-module hash before starting the new writer. Keep strict module-hash
checking enabled: the pinned sink selects an exact hash before considering any
fallback. Check that the first actual new block links to the old saved block and block
ID (the next finalized block may follow skipped Solana slots), then verify cursor advancement, swap quality and coverage-table writes.
The adopted cursor row is a restart boundary, not evidence of newly processed
blocks. Do not use this migration for an unvalidated stateful or historical
replay change. Historical recovery needs
a separate versioned canonical replay and full replacements: appending old
swaps to live tables double-counts OHLC aggregates. The older history manifest
remains pinned to its original decoder.

## pluto.13 metadata and pricing binaries

The pricing module uses the current `dex_swaps.wasm`, built without default
features. All seven metadata modules use `dex_swaps_metadata.wasm`, built from
immutable source commit `5db51e774846de14bd4100e0243603161a98fa04` with Rust 1.85.1.
The separate binary is pinned by `dex-swaps/metadata-binary.json`. Its bytes and
all metadata module hashes match the registry that passed the complete fee
snapshot count/marker checks. Routine price-decoder changes must retain those
bytes so existing metadata stores and their native-change replay remain usable.
A change to metadata semantics requires a new pinned artifact and fresh replay.

For builds after `.13` is published, obtain the pinned metadata artifact from
that release before packing. It contains the same public owned-mint seed as
its source tree; no network requests execute inside WASM.

```sh
gh release download v0.5.2-pluto.13 --repo ColourfulMelon/substreams-svm \
  --pattern dex_swaps_metadata.wasm --dir target/wasm32-unknown-unknown/release
python3 - <<'PYCODE'
import hashlib, json
from pathlib import Path
pin = json.loads(Path('dex-swaps/metadata-binary.json').read_text())
blob = Path('target/wasm32-unknown-unknown/release/dex_swaps_metadata.wasm').read_bytes()
assert len(blob) == pin['sizeBytes'] and hashlib.sha256(blob).hexdigest() == pin['sha256']
PYCODE
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked --no-default-features
cargo +1.85.1 build -p dex-swaps -p svm-dex --locked --no-default-features \
  --target wasm32-unknown-unknown --release
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.13.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.13.spkg
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.13.spkg
```

The initial metadata artifact was built from that pinned source before the
pricing-only DLMM depth correction. The frozen binary's unused pricing export
is never referenced by the manifest. The corrected current pricing export is
the sole source of swap events. The package must pass a fresh finalized replay
and the coordinated single-writer cursor transition before live deployment.
