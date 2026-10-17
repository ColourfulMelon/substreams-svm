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

## Native complete-block API

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

`Projection::into_legacy()` strips only the added evidence. For a complete valid
block it yields the same row fields and order as `map_events`; the WASM wrapper
keeps its existing output and filtering behavior. Existing adapter order is not
global instruction execution order: instruction rows precede log-derived rows.

The native API does **not** repair or qualify existing decoder behavior. In
particular, the pending temporary-account/current-layout fixes in #223 and the
Orca event/pool association fix in #224 remain separate changes. SPL Token Swap's
current output amount is the instruction minimum, not proven realized output.
Coverage can be restricted to positioned venue events with proven invocation
success; the pinned full-block fixture includes five such Meteora DLMM rows.
That fixture is offline evidence, not broad protocol or live-provider coverage.

Tests compare the native and WASM-wrapper body, including an encoded-output hash
captured independently from unchanged main. They also cover original positions,
aggregator classification, caught CPI failures, ambiguous/truncated invocation
traces, malformed blocks, resource bounds, empty blocks, and missing time.
