# Verified indexing: Pluto release 0.5.2-pluto.5

This fork corrects Orca event-to-pool alignment and checks all emitted financial
swaps against their own AMM invocation and actual transaction transfers. An event
that cannot be proved is rejected before database output. Jupiter wrappers
sharing a native swap's physical transfer evidence are removed. Separate
identical invocations remain separate trades. Rejected and corrected payloads
are internal verification evidence only: live and archive outputs never stream
or store diagnostic rows. Compact per-block counters remain available.

Meteora DLMM/DAMM and Raydium CLMM/CPMM match events by their recorded pool,
rather than letting a skipped instruction shift the next pool's amounts.
Meteora AMM vault-share rounding and PumpSwap fee-exclusive events are normalized
to exact IDL user-account flows. SPL and Token-2022 transfers are supported. Transfer fees require exact net receipt
proof, not an assumed percentage. Native Pump.fun SOL sales require an exact
pool lamport debit and a single unambiguous pool invocation.

The mapper retains the original block transaction index and flattened transfer
position. Candle open/close uses `(slot, transaction, transfer)` order. Stable
`event_id` values identify physical events. Every block records its parent slot
and hash, decoder version, accepted/rejected/duplicate/corrected counts.

Thirteen additional native venues use explicit instruction layouts and immediate
SPL/Token-2022 CPIs. The [Helius comparison](../docs/native-venue-validation.md)
records finalized samples, supported variants and fail-closed exclusions.

## Fresh reporting evidence

`verified_usd_price_candidates` uses only recent verified non-dust swaps. Both
bridge legs must be at most 60 seconds old. Bridge ratios cancel intermediate
mint decimals, reach a known SOL/stable anchor, and avoid repeated pools/mints.
SOL conversion uses fresh stablecoin trades. Per-pool close must be within
0.5–2 times its recent median; reporting consumers additionally need consensus
across pools and must explicitly return unavailable when evidence expires.
These views are reporting evidence, never permission to execute an order.
Exact executable quotes, simulation and on-chain minimum proceeds remain required.

## Build and verify

Use Rust 1.85.1 and Substreams CLI 1.18.2:

```
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked
cargo +1.85.1 build -p dex-swaps -p svm-dex --locked --target wasm32-unknown-unknown --release
make -C svm-dex/clickhouse schema
```

Pack dex-swaps, svm-dex and ClickHouse manifests in dependency order. The older
history package retains its original decoder; version a new archive separately.
Run `svm-dex/tests/verified-prices.sql` after the schema in an isolated ClickHouse
instance. Replay a bounded finalized range with `db_out -o jsonl`, then run
`python3 svm-dex/backfill/quality-audit.py replay.jsonl`. This verifies immutable
identities, exact parent links (including skipped slots), amounts and counts.
Public fixtures cover the AQUA Orca multihop failure, native Pump.fun proceeds
and exact Token-2022 withheld fees.

## Rollout and retained history

Apply additive raw/block verification columns and reporting views before the
new writer. Update derived candle MVs without deleting aggregate tables. Stop the
old writer gracefully, verify its durable cursor is at least every retained raw
block and there are no duplicate event identities, then resume the same cursor.
Allow the reviewed module hash once; restore strict mismatch enforcement after
the new cursor is saved. Never run old and new writers concurrently.
After a diagnostic-free writer is confirmed, drop legacy diagnostic tables
synchronously to reclaim their retained rows and projections.

For live pricing, request `--final-blocks-only` so provider-confirmed chain
finality replaces the SDK's synthetic undo buffer. Use the supported SQL sink
`--live-drift-reconnect 1m` setting to force overdue live sessions back into
parallel catch-up. Monitor actual block timestamps: a healthy process or a
stream marked live does not establish freshness.

Existing history is not silently rewritten. The v0.3.0 canonical archive records
verified raw swaps and every block. Use a new versioned archive namespace
for a complete replay; do not mix old decoder archives or use additive repair
when a pool/amount changed or a wrapper was removed. Retain backups and use the
coordinator's bounded canonical day replacement before rebuilding historical
candles. `backfill/quality-reconciliation.sql` audits recent live data read-only.

A 50-slot public replay starting at 454558265 accepted all 481 Pump.fun swaps
and all 22 Raydium launchpad swaps. Four zero-output dust events and one ambiguous
multi-receipt CLMM event were quarantined. This is bounded regression coverage,
not a claim that every future protocol/account layout is supported.
