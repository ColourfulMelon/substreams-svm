# Four-stage pricing decoder, pluto.31

The pricing graph removes the derived `store_token_fee_state` stage. Swap decoding
reconstructs the exact existing `token_fees::State` from the retained fee history
at each transfer's instruction ordinal. A block-local cache reuses a State only
when that ordinal-specific history is byte-identical. Earlier lookups cannot
reuse future fee updates. Native fee activation (epoch + 2), pending settings,
closures, recreation and unknown mutations retain the existing state codec.

The map also consumes the unchanged fee-update output and eagerly validates
history for every updated tracked mint, including mints without swaps. The exact
snapshot completion marker and all 238,409 snapshot seeds remain mandatory;
missing metadata is never silently accepted. Absence means zero fees only after
these guards pass. All six retained metadata module hashes and the frozen
metadata WASM bytes remain unchanged. The removed materialized store is no
longer a module input; its immutable binary export remains available.

The final package has four execution stages instead of five. Gzip envelope,
all SQL schemas, protocol IDs and supported financial formats remain unchanged.
The only expected row difference is decoder_version advancing from .30 to .31.
Deploy with the existing .30 bounded gzip and compact-key sink patches.

## Validation

All 231 core Rust tests pass (216 decoder, 15 mapper), with locked Rust 1.85.1
and no default features. The strict four-stage prototype retained all 9,643
financial rows, ordering, parent links and sealed counters in a finalized
100-block replay. Ordinal cache checks cover future prevalidation, earlier
lookups, closures, unknowns, pending configurations and recreation.
The final .31 package also preserves 12,949 swaps across 100 fresh blocks and
5,962 swaps across 46 finalized blocks in a 50-slot range containing two native
fee initializations. Skipped slots are not counted as finalized blocks.
See [release package proof](stage31-package-proof.json) for the final .31
package's fresh paired replay and retained module hashes.

A preceding 100-block prototype replay counted 3,365 processed blocks against
4,174 for .30 (19.4% fewer); provider cache availability was not held constant.
These are processed-stage savings, not total daily bill savings. Replay elapsed
time includes initialization and delivery and was essentially unchanged.
No live latency improvement is assumed.

History parsing moves into swap decoding. Synthetic local V8/WASM timing measured
~10 microseconds to reconstruct 64 records and ~632 microseconds for 4,096;
repeated cached lookups still copy/compare history (~0.9 / 54 microseconds).
These exclude host store calls and do not describe the live mint distribution.
Monitor processing rate, memory failures and cursor lag after rollout.

## Single-writer handoff

Warm the exact .31 package read-only while .30 runs. Stop the sole old writer
gracefully; verify a stable final cursor and no rows beyond it. Back up the old
opaque cursor, block number and block ID. Copy that exact row under .31's output
module hash, retaining the .30 row and strict hash checks. Start only .31,
verify its first actual parent matches the saved boundary, and verify advancing
cursors, financial counts, unique keys and zero diagnostics. Never append the
validation replay to live financial tables. Keep the paired .30 package and
its saved cursor for a verified reverse handoff if rollback becomes necessary.
