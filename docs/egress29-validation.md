# pluto.29 live egress validation

Production builds use Rust 1.85.1, `--locked --no-default-features` and the
unchanged pinned metadata artifact. Native audits can use
`--no-default-features --features coverage-diagnostics`. Core unit tests retain
the audit behavior. Live outputs contain financial swaps and sealed blocks;
coverage-only transactions need not be decoded into event output.

The SQL mapper ignores coverage candidates/originals even if they are supplied
by an older upstream decoder. Existing diagnostic tables/data and their TTLs
remain intact. Zero coverage counters now mean diagnostics are disabled.
Financial rejection counters remain available.

The stream no longer duplicates `swaps.block_hash` and `blocks.block_num` in
fields already carried by their composite key. **Deploy the patched Pluto SQL
sink with this package.** Its insert path restores missing insertable composite
columns even outside the ClickHouse sorting key. It retains explicitly supplied
columns and the existing primary-column behavior. Existing sink tests plus a
regression for omitted/explicit non-sorting key fields pass in both forms.
The unpatched v4.12.4 sink cannot consume the compact swap payload.

[Replay proof](egress29-replay-proof.json) compares exact .28/.29 packages on
finalized slots 456410752–456410851. Both streams exit cleanly and make no
writes to the database. All 9,643 financial rows match after restoring stream
keys and excluding only the release-version field. No rows are lost or added;
relative financial insert order, execution coordinates, physical identities,
amounts, parent links and financial block counters are unchanged. The new
output has 9,643 swaps and 100 blocks; 427 candidates and 330 raw transaction
rows are removed. Ordinals remain unique and contiguous within each block.

| Uncompressed DatabaseChanges protobuf bytes | .28 | .29 |
| --- | ---: | ---: |
| Swaps | 12,115,666 | 11,537,411 |
| Raw transaction diagnostics | 7,435,825 | 0 |
| Candidate diagnostics | 281,095 | 0 |
| Blocks | 42,317 | 39,907 |
| Total | 19,874,903 | 11,577,318 |

The 41.75% reduction is specific to this finite sample. It excludes RPC
response-envelope overhead; wire compression and ClickHouse ZSTD storage are
separate from billed uncompressed egress. The remaining five metadata/pricing
execution stages are unchanged, so this change does not reduce stage billing.
The seven metadata hashes and binary SHA-256 are identical to .28.

All 216 decoder and 14 mapper business tests pass. The mapper regression checks
that malformed/stale diagnostic data cannot change financial output and that
all key values remain recoverable. Patched pinned v4.12.4 `go test ./db ./sinker`
also passes. No new schema, protocol IDs or financial decoding behavior changes.

To reproduce, run `scripts/compare-egress-replays.py --help`, supplying the exact
packages, successful replay statuses and JSONL outputs from read-only,
final-only development-mode runs. The script verifies package hashes,
complete output byte counts, financial parity and protobuf encoded sizes.
The baseline can cover a larger range than the requested comparison window.
