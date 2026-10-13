# Shared transaction log parsing

The transaction log loop constructs one `ParsedLog` per line and shares its
program ID, invocation depth and exit flags with the eleven scoped protocol
handlers and OKX. Previously each scoped handler decoded the same base58 header
and allocated its own program-ID vector. Protocol state transitions, event
parsing, handler order and committed-invocation filtering retain their existing
semantics. In particular, foreign headers remain data for an active handler;
this optimization does not reinterpret nested invocation scopes.

Both default-feature and pricing `--no-default-features` decoder test runs pass
all 218 tests. Two added scope tests cover nested foreign calls, failure exits,
truncation, trailing whitespace, malformed depths and invalid program IDs.

The release WASM was replayed locally against 200 captured finalized blocks
(slots 456489100–456489299), using their real metadata snapshots and ordinal-aware
store deltas. All event protobuf bytes and gzip-envelope protobuf bytes match
the deployed pluto.31 captures on all four passes for each binary. Each block
uses a fresh WASM instance, as in Substreams execution.

After one warmup pass, the median of three pass means was **59.22 ms/block** for
the deployed decoder and **31.99 ms/block** for this change: **46.0% less decoder
execution time**. These local V8 measurements include protobuf decoding and host
lookups, excluding filesystem reads, input allocation and instance creation.
They do not establish Pinax CPU cost or production catch-up throughput.
See [the replay proof](shared-log-parsing-proof.json) for hashes and measurements.

This is a source change, not a packaged release or deployment. Package version,
four-stage graph, financial schema and sink remain unchanged. A subsequent
release must retain the frozen metadata WASM to preserve metadata store hashes
and caches; rebuild only the default decoder binary for this optimization.
