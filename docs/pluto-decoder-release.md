# Pluto DEX decoder release

`v0.5.2-pluto.27` adds verified native settlement formats for Aldrin V1,
Cropper, Picon DLMM, M0 native orders, Patch buybacks and Bxlw. Protocol IDs
79–85 append to existing IDs, including a dedicated PumpSwap buy-and-burn
format whose output is destroyed rather than credited to a trader. Typed burn
provenance is separate from transfer and orderbook verification.

The release also handles repeated and V2 Pump native settlement, Meteora DLMM
host-fee account aliasing, Tessera flag 2, Penguin's SPL Token Swap fork,
Sanctum legacy opcode 1 and Pancake's native classic-Token layout. Native
Pancake attribution fixes are bounded to exact committed original ancestry;
amounts, mint pairs, pools and physical receipt identities are preserved.

All 229 core tests pass: 215 decoder and 14 mapper tests. The locked Rust
1.85.1 WebAssembly release build uses `--no-default-features`. Exact package
replay compares .26 and .27 across 6,429 finalized blocks in three finite
windows. It preserves 876,427 existing rows unchanged, recovers 750 new legs
and corrects 1,221 bounded Pancake provenance rows. No existing swaps are
lost and no unexpected financial changes are accepted. All 1,971 recovered
or corrected rows match 27 financial/provenance fields against independent
original evidence from 1,969 transactions and 1,361 finalized containing
blocks. All four table versions, keys, ordinals, parent chains and sealed
counters pass. See the [package attestation](../dex-swaps/fixtures/swap-coverage/combined27/packaged-release-proof.json)
and [portable tools](../dex-swaps/fixtures/swap-coverage/combined27/README.md).

The retained live residual corpus additionally classifies 765 native sources
across 50 programs, distinguishing actual swaps from liquidity, lending,
fee claims, custody, game issuance and router calls whose child swaps already
exist. See [native format evidence](native-swap-formats27-validation.md).
These are finite observed scopes; unobserved or unproven formats remain
explicit diagnostic candidates. No universal future coverage claim is made.

IDL revision remains `785533d59d0b258500cbbce60890fe58b8c14405`. All seven
metadata module hashes and frozen metadata WASM bytes remain unchanged.
Before starting the writer, add the two raw burn columns and extend all eight
protocol enum columns through ID 85. Historical rows default both burn fields
to zero. Pricing accepts only verified transfer/orderbook rows or explicit
verified protocol-79 burn rows. Applying startup schema twice preserves old
financial rows and aggregate prices; malformed/unproven burn rows stay excluded.

Build with Rust 1.85.1 and Substreams CLI 1.18.2:

```sh
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked --no-default-features
cargo +1.85.1 build -p dex-swaps -p svm-dex --locked --no-default-features --target wasm32-unknown-unknown --release
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.27.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.27.spkg
make -C svm-dex/clickhouse schema
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.27.spkg
```

Warm the exact package's unchanged metadata registry with a read-only replay
near/before the intended restart boundary. Stop the old writer gracefully and
wait for its final data/cursor flush. Preserve its exact durable row and copy
that row under the verified new output-module hash before starting the new
writer. Keep strict module-hash checking enabled and retain the old hash row.
Check the first actual new block links to the saved old block and hash, then
verify advancing cursors, versions, sealed counters, keys and financial quality.
The adopted cursor is a restart boundary, not proof that the new decoder
processed the old block. Historical recovery requires canonical replacement;
appending old swaps or provenance corrections to live OHLC states double-counts.

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
