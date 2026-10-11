# Pluto DEX decoder release

`v0.5.2-pluto.8` extends the deployed `.7` normalized decoder with DefiTuna
FusionAMM and Invariant swaps (protocol IDs 46–47). Exact native token flows,
optional referral debits and verified recipient amounts determine prices.
Phoenix swap/marketable-order intents remain replayable when funded from existing
exchange balances without wallet token transfers; these diagnostics are not
normalized fills or pricing evidence. See [the coverage audit](swap-coverage-2026-10-11.md)
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
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.8.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.8.spkg
make -C svm-dex/clickhouse schema
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.8.spkg
```

Stop the old writer before upgrading, preserve its cursor, and deliberately
adopt the new module hash. Verify cursor advancement, swap quality and new-table
writes before restoring strict module-hash checking. Historical recovery needs
a separate versioned canonical replay and full replacements: appending old
swaps to live tables double-counts OHLC aggregates. The older history manifest
remains pinned to its original decoder.
