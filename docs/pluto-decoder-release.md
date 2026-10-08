# Pluto DEX decoder release

`svm-dex-v0.5.2-pluto.5` carries the decoder fixes proposed in
[IDL #147](https://github.com/pinax-network/substreams-solana-idls/pull/147) and
[SVM #223](https://github.com/pinax-network/substreams-svm/pull/223).
It also carries the Orca pool/direction event matching correction proposed in
[SVM #224](https://github.com/pinax-network/substreams-svm/pull/224), with a
public three-hop AQUA transaction regression and fixed-prefix legacy Swap decoding.
Version .4 removed full diagnostic rows from streamed/database output while
retaining all transfer checks, rejection, deduplication and per-block counters.
It pins the IDL fork at `fb8fd5b75e4122534f118c9471315212abdca6bf` and
versions the normalized swaps, database adapter, and ClickHouse package together.
Version .5 adds thirteen native venues with invocation-scoped token-flow
verification. See [Helius validation](native-venue-validation.md): 217 finalized
transactions, 211 native legs, 64 pools, zero mismatches. Unknown variants and
unproven net receipts remain excluded. No diagnostic output or new stored tables
are introduced. The older v0.3.0 history manifest stays pinned to its original
decoder; a replay using .5 must use a fresh archive namespace and package version.

Build from this tag using Rust 1.85.1 and Substreams CLI 1.18.2. The CLI version
matches Pluto's SQL sink; newer packagers can inject conflicting legacy SQL
service descriptors into these manifests.

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.85.1
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked
cargo +1.85.1 build -p dex-swaps -p svm-dex --locked --target wasm32-unknown-unknown --release
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.5.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.5.spkg
make -C svm-dex/clickhouse schema
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.5.spkg
```

The public transaction regression fixtures are in `dex-swaps/fixtures`.
Pluto deploys the final ClickHouse SPKG with its existing sink schema and cursor.
A decoder upgrade must adopt the current cursor explicitly; historical recovery
uses a separate versioned canonical replay and full replacements, because the
OHLC materialized views aggregate every inserted row. Stop the old writer before
adopting the new module hash; remove legacy diagnostic tables only after the new
writer is confirmed to emit only swaps and blocks.
