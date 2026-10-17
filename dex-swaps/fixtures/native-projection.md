# Native projection fixture

`solana-mainnet-350000026.pb.gz` is the unchanged complete Firehose protobuf block
already committed in public [PR #225](https://github.com/pinax-network/substreams-svm/pull/225)
at `3de4050e4bd6febfeca526281181d1ec3e23307c`, copied for a self-contained test on
main. No additional provider request was made for this change.

- Slot: 350000026; 1444 original transactions.
- Decompressed SHA-256: `ec6e4f3c1805034babeee7e35498989f73224a8419903e164eaf0703fe760969`.
- Original capture date: 2026-10-11; source provenance and RPC cross-checks are in
  PR #225's `spl/transfers/tests/fixtures/README.md`.
- Existing `map_events` on unmodified main `e7af0dc108ea4cd0105dc16eda9782376141abb2`
  emits 156 transactions / 231 swaps. Encoded protobuf output SHA-256:
  `c1e611c718878724d851e446c0a13bec1b28e682ebf99e2ab7b4a2d221b78218`.
- The baseline hash was captured by running main's unchanged decoder/module body
  natively; only its WASM-export macro was disabled for the host build. The new
  native API and wrapper body both reproduce it.

These rows retain all existing decoder limitations. The five Meteora DLMM rows
with proven instruction-event invocation success and matching immediate-parent
context establish the evidence path on this block; they do not qualify all
Solana swap coverage. A separate mutation test keeps a caught failed swap's stale
FIFO row unchanged while leaving its context explicitly unverified.
