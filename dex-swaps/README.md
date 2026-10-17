# DEX Swaps

`dex-swaps` emits a normalized protobuf stream of swap activity across supported Solana DEX protocols.

Each emitted swap includes:

- protocol enum
- protocol program ID (`amm`)
- pool or market identifier
- user wallet
- input token mint and amount
- output token mint and amount

Only protocols that can reliably provide the full normalized swap shape are included.

Notes:

- `protocol` is a protobuf enum with stable protocol IDs.
- Downstream packages such as `svm-dex` can materialize that enum into lowercase snake_case strings such as `jupiter_v6` and `raydium_amm_v4`.
- `amm` is the protocol program ID as raw bytes.
- `amm_pool` is the stable pool or market identifier when the protocol exposes one.
- If a protocol does not expose a pool identifier, `amm_pool` falls back to `amm`.
- Jupiter currently uses `fee_payer` as the best available user-level identity from the decoded event stream.
- Meteora AMM is intentionally excluded for now because its current decoded swap shape does not expose canonical token mint identities for normalization.

Current coverage:

- Boop
- Darklake
- DumpFun
- Jupiter v4
- Jupiter v6
- Orca Whirlpool `SwapV2`
- Pump.fun
- Pump.fun AMM
- Raydium AMM V4
- Raydium CLMM `V2`
- Raydium CPMM
- Raydium Launchpad
- Meteora DAAM
- Meteora DLMM
- OKX DEX V2
- SPL Token Swap-compatible programs

Currently excluded:

- Meteora AMM
  Current decoded swap shape does not expose canonical token mint identities for normalization.
- Saros
  Current decode exposes intent fields like `amount_in` and `minimum_amount_out`, but not the user, pool, mints, or finalized output amount needed for normalized swaps.
- Sanctum
  Current decode exposes only the swap amount and does not provide normalized pool, mint, or finalized output fields.
- DFlow
  Current decoder is a placeholder that treats any instruction as a swap and does not expose reliable normalized swap fields.

## Solana 0.16 and Buffa migration

The Rust API now uses `substreams-solana-core` 0.16 and Buffa 0.9.2, with
Substreams 0.8 for the default WASM feature. This is a Rust source compatibility
change: older Prost-generated `Block` and output types are different Rust types,
even when they represent the same protobuf messages. Native callers must update
their chain model and protobuf runtime together. The SPL projector's separate
legacy Prost adapter remains available; it does not make an old `Block` directly
acceptable to `dex-swaps::project`.

Message fields use `buffa::MessageField<T>` instead of `Option<T>`; use
`as_option()` to preserve absence and `MessageField::some(value)` to construct a
present message. Enum fields use `buffa::EnumValue<E>` instead of `i32`; use
`as_known()` for a recognized variant or `to_i32()` when preserving an unknown
numeric value. Encoding and decoding use the `buffa::Message` trait. See
[protobuf development](../docs/protobuf.md) for generation and presence rules.

The `.proto` package names, field numbers, enum IDs and output meanings remain
unchanged. The existing normalized swap rows, ordering and WASM module output
schema remain the compatibility contract. Captured output-byte goldens and
native/wrapper parity tests check this migration; they do not qualify untested
protocol behavior or a live provider.

The native `project` and `project_with_limits` APIs cover legacy and v0 Solana
messages only. They return `ProjectError::UnsupportedMessage` for an explicit
version above zero, contradictory explicit v0 metadata, or any present inline
transaction configuration, including in failed transactions. An absent version
with `versioned: true` remains supported for older v0 producers. These checks
prevent newly added upstream fields from being silently discarded. They apply
to the validated native API; the legacy `map_events` wrapper retains its existing
filtering and extraction behavior.

The [Linux compatibility workflow](../.github/workflows/sdk-migration.yml) checks
formatting, clean regeneration with Buf 1.69.0, both native DEX feature modes,
native-token and svm-dex tests, and the complete release WASM workspace on Rust
1.85. Generated files are committed and checked for both modifications and new
untracked output; run `make -C proto protogen` to reproduce them locally.

## Native complete-block API

Native applications should depend on `dex-swaps` with `default-features = false`.
This uses `substreams-solana-core` (the same model and instruction views reexported
by `substreams-solana`) and avoids linking Substreams WASM allocator exports into
the host process. Native logging uses the standard `log` facade. The default
`substreams` feature retains the existing WASM entry point and Substreams logging;
`map_events` remains callable in both feature modes with the same error type and
output. `common` uses the same default feature for its database, `Clock` and scalar
helpers; its pure Solana helpers remain available without it. The SDK-free IDL
dependency is pinned to [substreams-solana-idls PR #148](https://github.com/pinax-network/substreams-solana-idls/pull/148);
review and resolve that dependency before release.

`project(&substreams_solana::pb::sf::solana::r#type::v1::Block)` is callable from
native Rust and returns a validated `Projection`. It borrows the existing decoded
block and calls the same protocol extraction as the WASM wrapper, with no store,
RPC request, or Substreams execution. `project_with_limits` additionally lets a
caller bound transactions, instructions, logs, log bytes, and output rows. The
structural validator is pinned to the pending native SPL projector in PR #225;
that dependency must be reviewed before release.

Every projection includes a clock, even for an empty block. Missing block time
remains `None`, and a parent slot may skip several slots. Transactions carry their
original index in the unfiltered block. Every row adds:

- `SwapKind::PoolExecution` or `SwapKind::AggregatorSummary` (Jupiter v4/v6 and
  OKX); summaries must not also count as venue executions.
- `Provenance::Instruction` with the exact outer/inner **event instruction**
  position and an event index, or `Provenance::Log` with the original log index.
  An event instruction may be a self-CPI emitted by an earlier swap invocation.
  A log-only row has no proven instruction identity; this API never invents it.
- `InvocationOutcome`: success is proven only when the recorded instructions,
  program invocation depths, and balanced outcome logs match exactly. A caught
  ancestor failure revokes an earlier successful event. Truncated, ambiguous, or
  missing traces remain `Unknown`. This proves the event invocation outcome, not
  correctness of the protocol adapter's pool/amount association.
- `verified_context`: for DLMM only, the exact immediate parent swap instruction
  when its pool, user and token accounts match the event's intrinsic pool/user
  and every decoded row field. This additionally requires proven event/ancestor
  success. Other protocols and unverified associations remain `None`. A stale
  FIFO row after a caught failed swap is retained but is never marked verified.

`Projection::into_legacy()` strips only the added evidence. For a complete valid
block it yields the same row fields and order as `map_events`; the WASM wrapper
keeps its existing output and filtering behavior. Existing adapter order is not
global instruction execution order: instruction rows precede log-derived rows.

The native API does **not** repair or qualify existing decoder behavior. In
particular, the pending temporary-account/current-layout fixes in #223 and the
Orca event/pool association fix in #224 remain separate changes. SPL Token Swap's
current output amount is the instruction minimum, not proven realized output.
Consumers requiring execution association must additionally require a verified
context, not just successful event invocation. The pinned full-block fixture
includes five Meteora DLMM rows with both forms of evidence.
That fixture is offline evidence, not broad protocol or live-provider coverage.

Tests compare the native and WASM-wrapper body, including an encoded-output hash
captured independently from unchanged main. They also cover original positions,
aggregator classification, caught CPI failures, ambiguous/truncated invocation
traces, exact runtime log grammar, failed-prior-swap FIFO association, malformed
blocks, resource bounds, empty blocks, and missing time.
