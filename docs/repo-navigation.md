# `substreams-svm` Repository Navigation Guide

This guide is a quick map for contributors and coding agents working in this monorepo.

## 1) What this repository is

`substreams-svm` is a Rust workspace of Solana/SVM Substreams packages for:

- protocol-specific decoders (DEXes, SPL/native, NFTs, staking)
- aggregate `DatabaseChanges` modules (`svm-*` crates)
- sink-facing packages for ClickHouse (`svm-*/clickhouse`)

## 2) Top-level layout

- `Cargo.toml`: workspace members and shared dependency versions.
- `proto/`: shared protobuf definitions in `proto/v1/*.proto`.
- `common/`: shared Rust helpers (`db.rs`, `solana.rs`).
- `dex/`: protocol-level DEX modules (one crate per DEX/program).
- `spl/`, `native/`, `nft/`, `staking/`: protocol-level non-DEX modules.
- `svm-*`: aggregate DB modules by data domain.
- `svm-*/clickhouse`: ClickHouse sink packages and schema DDL.
- `spkg/`: local SPKG dependencies used by aggregate manifests.
- `scripts/`: helper scripts for generating DEX boilerplate/proto glue.

## 3) Fast navigation by task

### Add or edit a DEX protocol decoder

For the Pluto normalized pricing stream, start in `dex-swaps/src/lib.rs`,
`native_venues.rs`, `quality.rs`, and `routed_pool.rs`. Jupiter Lend AMM
IDL layouts are in `jupiter_lend.rs`; its nested liquidity settlement is verified
in `quality.rs`. Additional native
market layouts do not need separate imported modules or SQL schema changes.
See [`native-venue-validation.md`](native-venue-validation.md) for verified
variants and fixture evidence.
The pluto.31 pricing graph removes the derived fee-state stage; see [four-stage validation](stage31-validation.md).
The pluto.33 SQL mapper uses gzip level 6; see [compression validation](gzip6-validation.md) and [package proof](stage33-package-proof.json). Decoder and metadata hashes remain unchanged.
The pluto.32 [package proof](stage32-package-proof.json) preserves the four-stage graph and metadata caches.
Transaction log headers are shared across protocol handlers; see [shared log parsing validation](shared-log-parsing-validation.md) for compatibility and local timing evidence.
The pluto.30 SQL output uses a versioned gzip envelope; see [gzip validation](gzip30-validation.md). Its paired sink decompresses before applying unchanged financial rows.
The pluto.29 live output omits coverage diagnostics and duplicate composite-key fields; see [egress validation](egress29-validation.md) and `scripts/compare-egress-replays.py`. Offline audits retain diagnostics with `--features coverage-diagnostics`.
The pluto.28 patch documents [live principal rounding and fresh/closed account evidence](live-principal28-validation.md).
The pluto.27 batch documents [additional native settlement and burn formats](native-swap-formats27-validation.md), the complete observed residual classifications and the [combined packaged replay tools](../dex-swaps/fixtures/swap-coverage/combined27/README.md).
The pluto.26 batch additionally documents [current proprietary formats](current-proprietary-native-validation.md), [bounded anchor suffixes](anchor-swap-suffix-native-validation.md), [custody/Numeraire swaps](custody-numeraire-native-validation.md), [Futarchy/Guacswap](futarchy-guacswap-native-validation.md), [additional native exchanges](additional-native-exchange-validation.md), and [typed non-spot classifications](residual-nonspot-native-validation.md). Combined package replay and deployment proofs determine which candidate decoders are released and live.
See [`pump-repeated-native-sell-validation.md`](pump-repeated-native-sell-validation.md)
for implicit Pump sell allocation from committed state witnesses, exact fee
returns, bounded integer rounding and physical ledgers.
See [`pump-native-cashback-validation.md`](pump-native-cashback-validation.md)
for native cashback, inert suffixes, same-wallet buy cycles and exact account
closure refunds proved against finalized originals. See
[`pump-gross-net-correction-validation.md`](pump-gross-net-correction-validation.md)
for independently proved corrections to existing gross SOL price outputs.
See [`riptide-native-validation.md`](riptide-native-validation.md) for native
exact-input settlement, token-program roles and original principal receipts.
See [`quay-native-validation.md`](quay-native-validation.md) for both Quay
directions and [`damm-legacy-suffix-validation.md`](damm-legacy-suffix-validation.md)
for bounded DAMM suffixes and canonical event binding.
See [`swap-coverage-2026-10-11.md`](swap-coverage-2026-10-11.md) for finalized
replay results, newer instruction variants, net-receipt verification and gaps
that still require investigation. See [`token-transfer-fee-registry.md`](token-transfer-fee-registry.md)
for the staged fee accounting core, finalized receipt fixtures and required
streaming integration. Standalone package availability does not
imply coverage in the normalized pricing stream.
Phoenix lot/fee accounting is in `dex-swaps/src/phoenix.rs`, with its bounded
current/future market registry in `phoenix_metadata.rs`. The pluto.10 candidate
adds signed ledger evidence separately from SPL transfer evidence; check release
and deployment state before treating it as live coverage.

For standalone protocol packages:

1. Start in `dex/<protocol>/` and update `src/lib.rs`.
2. Confirm `substreams.yaml` block filter (`program:<PROGRAM_ID>`).
3. Ensure matching protobuf in `proto/v1/*.proto`.
4. Wire output into `svm-dex/substreams.yaml` imports + `db_out` inputs.
5. Update ClickHouse sink schemas in `svm-dex/clickhouse/`.

### Add or edit aggregate database mapping logic

- Domain crates: `svm-balances`, `svm-transfers`, `svm-accounts`, `svm-metadata`, `svm-native`, `svm-spl`, `svm-nfts`, `svm-staking`, `svm-dex`.
- Entry point is typically `src/lib.rs` plus per-protocol files in `src/*.rs`.

### Work on sink schemas

- ClickHouse: `svm-*/clickhouse/schema.*.sql` and `Makefile`.
- Canonical decoder-history archives and additive diagnostics: `svm-dex/backfill/`.

## 4) Workspace module inventory

### Aggregate domain crates (`svm-*`)

- `svm-accounts`, `svm-balances`, `svm-transfers`
- `svm-metadata`, `svm-native`, `svm-spl`
- `svm-nfts`, `svm-staking`, `svm-dex`

### DEX crates (`dex/*`)

- Core/high-volume: Raydium (`amm-v4`, `clmm`, `cpmm`, `launchpad`), Jupiter (`v4`, `v6`), Pump.fun (`bonding_curve`, `amm`), Meteora (`amm`, `daam`, `dlmm`), Orca (`whirlpool`).
- Additional protocols: BonkSwap, Lifinity, Phoenix, OpenBook, PancakeSwap, Moonshot, Stabble, Darklake, DumpFun, GoonFi, Heaven, Plasma, Saros, Aldrin, Boop, ByReal, DFlow, Drift, Obric (`v2`, `v3`), OKX DEX, Sanctum, Serum, SolFi (`v1`, `v2`), PumpSwap.

### Non-DEX protocol crates

- `spl/token`, `spl/token-2022`, `spl/token-swap`, `spl/token-lending`
- `native/system`, `native/stake`, `native/vote`
- `nft/magiceden/m2`, `nft/magiceden/m3`, `nft/tensor`
- `staking/marinade`
- `metaplex`

## 5) Source-of-truth files

- Program ID filters: each module's `substreams.yaml` under `blockFilter.query.string`.
- Aggregate DEX wiring: `svm-dex/substreams.yaml`.
- SQL schema outputs: `svm-*/clickhouse/schema.*.sql`.
- Shared protobuf contracts: `proto/v1/*.proto`.
- Shared transform helpers: `common/src/solana.rs`, `common/src/db.rs`.

## 6) Build/test entry points

- Pluto fork release: [`pluto-decoder-release.md`](pluto-decoder-release.md).

- Build workspace WASM: `cargo build --target wasm32-unknown-unknown --release`
- Build/pack per package: `make build`, `make pack` (where Makefile exists)
- Local stream checks: `make noop` / `make gui` in module directories

## 7) Notes discovered during repo scan

- Many historical docs still mention old `db-svm-*` folder names; current structure is `svm-*`.
- Root `README.md` previously under-described current module coverage.
- `scripts/gen-dex.sh` uses a hard-coded base path (`/data/workspace/substreams-svm`), so it may require adaptation before use in other environments.
