# Pluto decoder history repair

`history.yaml` archives the exact deployed `svm-clickhouse-dex-v0.5.2-pluto.1`
`db_out` into `canonical_swaps`. The wrapper preserves the complete canonical
payload and deterministic row key. The daily ReplacingMergeTree archive can
resume safely and supports complete aggregate rebuilds when old payloads were
incorrect as well as missing.

`substreams.yaml` is an additions-only diagnostic comparing the original
`svm-clickhouse-dex-v0.5.2` output with the repaired decoder. It preserves event
multiplicity and compares payloads independently of unstable output ordinals.
Jupiter pool enrichment is separated from newly counted swaps. A removed or
changed financial payload stops the diagnostic; it must use full canonical
history repair instead of a blind additive insert.

The production coordinator and recovery instructions live in the Pluto monorepo
at `apps/pluto-pricing-orchestrator/backfill/`. Archive cursors are isolated from
production. Repair/backup tables belong in a separate database because the SQL
sink discovers every table in its own database and requires a primary key.

Build the map with Rust 1.85 and pack with Substreams 1.18.2, which supports the
SQL service descriptors embedded in the pinned release packages:

```sh
cargo +1.85 test -p svm-dex
cargo +1.85 build -p svm-dex --target wasm32-unknown-unknown --release
substreams pack svm-dex/backfill/history.yaml -o pluto-decoder-history-v0.1.0.spkg
```

The deployed archive SPKG is checksum-pinned in the coordinator. Recompiling
changes its module hash; do not replace a running archive package or bypass the
cursor mismatch guard without validating and explicitly migrating its cursor.
