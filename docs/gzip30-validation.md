# Per-block gzip output, pluto.30

The pricing SQL mapper now emits `pluto.sink.v1.GzipDatabaseChanges` version 1.
It contains the entire original `DatabaseChanges` as one fast gzip member,
plus its exact uncompressed length. Compression uses flate2 1.1.10's pure Rust
backend, with a zero modification time and no filename. All financial and
provenance fields, composite keys, relative insertion order, sealed counters
and parent links are retained. Only decoder_version advances from .29 to .30.
No financial decoder or SQL schema changes are included.

Deploy with Pluto's `sink-gzip-output.patch`, applied after its existing
shutdown and compact-key patches against pinned sink 4.12.4. It registers the
new protobuf output type, validates version and a 64 MiB uncompressed limit,
then checks gzip CRC, exact decoded length, one member, and no trailing data
before handing the original rows to the existing insertion/flush/cursor path.
Malformed/truncated/oversized input fails before rows are applied. The sink
also retains its current and legacy uncompressed formats. Protobuf Go bindings
are generated with protoc-gen-go 1.36.11 from `proto/v1/gzip-database.proto`.

All 231 existing/core Rust tests pass (216 decoder and 15 mapper), including
a gzip round trip retaining u64 maximum amounts and sealed blocks. WASM builds
with Rust 1.85.1 and locked dependencies. Pinned sink DB/handler tests pass for
both output formats, including unchanged SQL and cursor expectations, corrupt
checksums, truncation, lengths, unsupported types/versions, trailing members,
and bounded expansion. The native Rust example is consumed by the actual Go
sink decoder as a separate interoperability check.

The exact finalized 100-block .29/.30 packaged replay preserves every one of
9,643 swaps and every sealed block; fields differ only in decoder_version.
Actual encoded output falls from 12,061,668 bytes to 3,047,632 bytes including
the gzip envelope: **74.73%**. RPC cursors/envelopes are excluded, so this is not
a fixed daily bill reduction. See [package proof](gzip30-replay-proof.json).
Both finalized replay captures exited cleanly and made no production database
writes. The final capture uses `--bytes-encoding base64` for standard protobuf
JSON consumed by the actual sink decoder. The five-block warm replay
at 456449000..456449004 separately validates registry guards, complete block
parent links, positive financial amounts and zero diagnostics.

All seven frozen metadata module hashes, their 21,163,049-byte WASM artifact,
and map_events hash are unchanged. The five execution stages remain unchanged.
The release optimizes output bytes, not processing-stage billing.

The old .29 handwritten size estimator omitted nonzero Field.update_op tags.
The corrected script/proof included here agrees with the pinned Go protobuf
implementation: .28/.29 output was 20,392,093/12,061,668 bytes, or **40.85%**.
The older immutable release's reported 41.75% should use that correction.

Before rollout, warm the exact package, stop the sole writer gracefully,
confirm no rows beyond its durable cursor, copy that exact opaque cursor/slot/
block ID under the new verified module hash, and preserve the old hash row.
Start only the paired package and sink; check the first actual new block's
parent matches the saved boundary, then verify advancing cursors, row versions,
sealed counts, complete block hashes and zero diagnostics. Never append a
historical replay to financial tables.
