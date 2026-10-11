# Pluto DEX decoder release

`v0.5.2-pluto.9` extends `.8` with 1DEX exact-input swaps (48), JupiterZ
RFQ settlement (49), both Jupiter v6 singular/plural swap event formats and
event-independent settlement for existing typed Raydium Launchpad swaps.
Native transfers and independently proven recipient amounts determine prices.
For RFQ, `amm_pool` is the actual maker liquidity-source identity; asset-pair
columns keep its markets separate. Forty finalized 1DEX and forty RFQ fixtures
match independent contract/transfer audits. The 78-transaction Jupiter corpus
preserves all 213 existing financial legs. See [the coverage audit](swap-coverage-2026-10-11.md)
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
substreams pack dex-swaps/substreams.yaml -o spkg/dex-swaps-v0.5.2-pluto.9.spkg
substreams pack svm-dex/substreams.yaml -o spkg/svm-dex-v0.5.2-pluto.9.spkg
make -C svm-dex/clickhouse schema
substreams pack svm-dex/clickhouse/substreams.yaml -o spkg/svm-clickhouse-dex-v0.5.2-pluto.9.spkg
```

Stop the old writer gracefully before upgrading and preserve its exact durable
cursor. For this validated stateless upgrade, copy that cursor row under the
new output-module hash before starting the new writer. Keep strict module-hash
checking enabled: the pinned sink selects an exact hash before considering any
fallback. Check that the first actual new block is the old saved block plus
one, then verify cursor advancement, swap quality and coverage-table writes.
The adopted cursor row is a restart boundary, not evidence of newly processed
blocks. Do not use this migration for an unvalidated stateful or historical
replay change. Historical recovery needs
a separate versioned canonical replay and full replacements: appending old
swaps to live tables double-counts OHLC aggregates. The older history manifest
remains pinned to its original decoder.
