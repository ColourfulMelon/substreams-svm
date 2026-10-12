# Pluto DEX decoder release

`v0.5.2-pluto.9` extends `.8` with 1DEX exact-input swaps (48), JupiterZ
RFQ settlement (49), both Jupiter v6 singular/plural swap event formats and
event-independent settlement for existing typed Raydium Launchpad swaps.
Native transfers and independently proven recipient amounts determine prices.
For RFQ, `amm_pool` is the actual maker liquidity-source identity; asset-pair
columns keep its markets separate. Forty finalized 1DEX and forty RFQ fixtures
match independent contract/transfer audits. The 78-transaction Jupiter corpus
preserves all 213 existing financial legs. See [the coverage audit](swap-coverage-2026-10-11.md)
for fixtures and remaining formats. This release does not establish universal
support for every current or future swap program.

IDL source is pinned to immutable revision
`785533d59d0b258500cbbce60890fe58b8c14405`. Existing protocol IDs remain unchanged.
The `.6` enum/projection migration is retained and extended; no history is
materialized or rewritten by it. Additive coverage tables retain original
candidate transactions and per-instruction identities for seven days. Candidate
records include non-swaps and never feed price views. Block counters persist.
Create all tables, columns and enum values before starting the new writer.

Build with Rust 1.85.1 and Substreams CLI 1.18.2, matching Pluto's SQL sink:

```sh
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked
cargo +1.85.1 build -p dex-swaps -p svm-dex --locked --target wasm32-unknown-unknown --release
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.9.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.9.spkg
make -C svm-dex/clickhouse schema
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.9.spkg
```

Stop the old writer gracefully before upgrading and preserve its exact durable
cursor. For this validated stateless upgrade, copy that cursor row under the
new output-module hash before starting the new writer. Keep strict module-hash
checking enabled: the pinned sink selects an exact hash before considering any
fallback. Check that the first actual new block is the old saved block plus
one, then verify cursor advancement, swap quality and coverage-table writes.
The adopted cursor row is a restart boundary, not evidence of newly processed
blocks. Do not use this migration for an unvalidated stateful or historical
replay change. Historical recovery needs
a separate versioned canonical replay and full replacements: appending old
swaps to live tables double-counts OHLC aggregates. The older history manifest
remains pinned to its original decoder.

## Candidate pluto.13 metadata and pricing binaries

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
