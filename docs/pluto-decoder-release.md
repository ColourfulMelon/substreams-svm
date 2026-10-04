# Pluto DEX decoder release

`svm-dex-v0.5.2-pluto.1` carries the decoder fixes proposed in
[IDL #147](https://github.com/pinax-network/substreams-solana-idls/pull/147) and
[SVM #223](https://github.com/pinax-network/substreams-svm/pull/223).
It pins the IDL fork at `fb8fd5b75e4122534f118c9471315212abdca6bf` and
versions the normalized swaps, database adapter, and ClickHouse package together.

Build from this tag using Rust 1.85.1 and Substreams CLI 1.18.2. The CLI version
matches Pluto's SQL sink; newer packagers can inject conflicting legacy SQL
service descriptors into these manifests.

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.85
cargo +1.85 test -p dex-swaps --locked
cargo +1.85 build -p dex-swaps -p svm-dex --locked --target wasm32-unknown-unknown --release
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.1.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.1.spkg
make -C svm-dex/clickhouse schema
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.1.spkg
```

The public transaction regression fixtures are in `dex-swaps/fixtures`.
Pluto deploys the final ClickHouse SPKG with its existing sink schema and cursor.
A decoder upgrade must adopt the current cursor explicitly; historical recovery
needs a separate process that inserts only missing swap legs because the OHLC
materialized views aggregate every inserted row.
