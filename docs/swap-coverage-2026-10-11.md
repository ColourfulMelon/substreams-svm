# Swap coverage audit — 2026-10-11

The deployed `v0.5.2-pluto.5` decoder was replayed unchanged against 21
finalized mainnet blocks: 2,570 successful transactions with token balances.
RPC reads used `maxSupportedTransactionVersion: 127`: current transactions
include version 1, so a version-0-only audit is incomplete.

Helius parsed 1,550 transactions as swaps. Seventy-five were entirely absent
from both ClickHouse and the unchanged offline decoder. This establishes a
decoder gap rather than ingestion lag. Whole-transaction coverage alone is
insufficient: a later audit also checks uncovered immediate two-mint CPI legs
inside partially indexed transactions. Deposits, withdrawals and fee claims
can have two-mint transfers and must not be classified as swaps automatically.

The current fixes include:

- Pump.fun V2/V3 buys, sells and exact quote input; legacy/current boolean flags;
  current quote-mint events parsed through the string/shareholder fields.
- PumpSwap V2 swaps using the common first-nine-account trade prefix. V2 token
  programs replace legacy fee recipients at accounts 9/10.
- Exact native SOL buy debits and V3 sell proceeds. A V3 sell requires the
  curve debit and declared buyback recipient credit to prove net proceeds;
  unrelated native transfers or repeated fee-recipient calls invalidate it.
- Meteora DBC legacy swap, swap2 exact input/partial fill/exact output, and
  transfer-hook slice layout validation. User input/output positions are fixed
  while base/quote vault direction varies. Mint/transfer proof remains required.
- ByReal dynamic-fee swap_v3_dyn and PancakeSwap swap_v2 native token-flow paths.
- Obsidian vault reversal with fixed user input/output positions.
- FluxBeam's SPL-style swap prefix and native CPIs, distinct from FLUX.
- Poseidon V2, Denali, Aquifer bank exchanges, and Bison prediction cash/outcome
  buys and sells. The latter uses the actual prediction market identity and
  separate mint pairs, rather than fabricating a pool from token accounts.
- Orca zero-padding and DLMM referral transfer hooks in the pinned IDL.
- Native fallbacks for Orca, Raydium CLMM and DLMM when events are unavailable.
- Pump pool/curve multi-hop token transfers, including vault-to-vault legs.
- Token-2022 net receipts, including accounts created and closed in one
  transaction, explicit burns, repeated zero-fee receipts and transfer fees.
  Unproven net receipts are excluded; gross event amounts cannot override them.
- Physical CPI identity deduplication across native and router events.
- SolFi V2 and Saros native exchanges, plus native fallbacks for Raydium CPMM
  and Meteora DAMM v2 when emitted amounts do not reflect net receipts.
- Plasma and BonkSwap native exchanges, Heaven buys/sells, and Stabble weighted
  swap_v2/legacy layouts. Stabble's nested output must come from its declared
  vault program's exact withdrawal instruction and vault/user token accounts.
- Token-2022 self-harvest refunds prove the current transfer's gross amount is
  returned; older withheld fees collected simultaneously are excluded. The
  refund and all ancestors must return successfully. Caught invocation failures
  also exclude rolled-back transfers and swaps from pricing evidence.

Replay results: the original 1,677 legs in 1,481 transactions become 1,961 legs
in 1,598 transactions, with 117 additional transactions and no lost original
transaction. An independent raw-RPC check verifies 312 new or corrected legs
in 260 transactions with zero mismatches. Three additional finalized Bison
prediction transactions separately verify YES/NO buys and a YES sell. These
counts describe this sample, not global coverage.

A separate current-program inventory sampled 71 finalized transactions. The
new Plasma, BonkSwap, Heaven and Stabble adapters recover 42 legs in 40
transactions across 17 program-owned pools. Independent RPC checks match exact
accounts, mints, atom amounts and physical instruction identities with zero
mismatches; Heaven Token-2022 receipts additionally verify self-harvest ordering,
accounts and complete successful token-program logs. The original block corpus
remains at 1,961 legs after these additions and caught-failure guards. There are
87 passing core Rust tests and 121 positive raw transaction regression cases.
Phoenix samples include order placement, cancel/withdraw and immediate swaps;
they require event-specific handling rather than treating deposits as fills.
Lifinity, Obric, GoonFi v1 and SolFi v1 sampling did not yield recent executed
swaps, so account-address activity is not claimed as validated coverage.

The committed raw fixtures in `dex-swaps/fixtures/swap-coverage` are unmodified
public transactions. Expected restored legs were independently compared to
RPC instructions, actual user debits/receipts, mints, pools and integer atoms;
implicit native sells additionally use exact account lamport deltas. Fixture
signatures, slots and hashes are recorded in `evidence.json`.

Primary instruction references:

- https://github.com/pump-fun/pump-public-docs/tree/main/idl
- https://github.com/MeteoraAg/dynamic-bonding-curve-sdk/tree/main/packages/dynamic-bonding-curve/src/idl
- https://github.com/byreal-git/byreal-titan-integration
- https://github.com/pump-fun/pump-public-docs/blob/main/docs/instructions/MULTI_HOP_SWAP.md
- https://github.com/MeteoraAg/dlmm-sdk/blob/main/idls/dlmm.json
- https://github.com/solana-program/token-2022/blob/main/program/src/extension/transfer_fee/processor.rs

Run the core regressions and replay tool:

```sh
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked
cargo +1.85.1 run -p dex-swaps --example audit -- /path/to/raw/transaction/fixtures
```

This sample does not establish support for every swap type. A Jupiter lending
conversion and a zero-output Orca dust invocation are classified as swaps by
Helius but do not prove a positive two-sided spot trade. Other programs and
variants still require decoder coverage, including native SOL multi-hop curves
and some conversions and orderbook formats. The raw leg audit
also flags liquidity operations and router fee transfers: its candidates must
be classified rather than blindly indexed as prices. The audit must expand to
the current protocol inventory and retain unknown candidates for inspection.

Unclassified two-mint immediate token flows, mint/burn exchanges and recognized
swaps without verified receipts now emit `CoverageCandidate` diagnostics.
Multi-hop calls report any declared pool without a verified leg, even if another
hop in the same call succeeded. The original successful transaction protobuf
is retained once per candidate transaction in `swap_coverage_transactions`;
individual program/tag/source identities are in `swap_coverage`. Both tables
have a seven-day TTL; block-level counters persist after payload expiration.
These records are candidates, not proof that a swap was missed: liquidity,
router fees and lending conversions also appear. They never enter price views.
Pure orderbook fills without token transfers still need event-specific decoding.
Create the additive coverage tables and columns before starting this writer.

The `.7` package was deployed on 2026-10-11 and resumed at finalized slot
455702177, immediately after the previous writer's durable slot 455702176.
Strict module-hash checking was restored. By 15:59 UTC it was four seconds behind,
and retained candidate counts matched completed block counters exactly.
Production API service and disabled custom limit-order execution were preserved.

The `.8` decoder passes 90 core Rust tests and independently restores a further 78 legs: 38 DefiTuna
FusionAMM swaps across eight program-owned pools and 40 Invariant swaps across
two program-owned pools, with both directions represented. Actual token atoms,
accounts, user authorities, temporary WSOL mint initialization and Token-2022
net receipts were checked independently. Invariant referral inputs are additionally
covered by a financial test; no real referral was present in the 40-swap sample.
The raw evidence files preserve source revisions, hashes and exact identities.
Phoenix intents now remain diagnostic candidates even without token transfers,
while post-only placement, cancel, deposit and withdrawal are excluded from
that classification. They still require full fill and market-lot accounting.
DefiTuna two-hop swaps also require their own per-pool decoder and real fixtures.

Primary new references:

- https://github.com/DefiTuna/fusionamm-sdk/tree/683c52d6e9f29b7a696877f798c8bb2bfb439893
- https://github.com/invariant-labs/protocol/blob/a30e3b6d86a34400fae202299dba969daff36359/programs/invariant/src/instructions/swap.rs

 Historical recovery
must use a separate canonical archive/replacement; live OHLC tables cannot
safely receive appended historical replays. A live package upgrade must retain
the saved cursor and adopt its new module hash deliberately.

## Pending `.9` additions

1DEX exact-input swaps (protocol 48) have 40 finalized transaction fixtures
with both directions through one owner-verified pool. Gross taker input already
includes optional vault-paid fees. Exact-output remains disabled pending its
own contract and transaction evidence. See `onedex-evidence.json`.

Jupiter v6 now decodes the on-chain IDL's singular `SwapEvent` and plural
`SwapsEvent` formats. A 78-transaction sample contains 56 singular events and
53 vectors with 147 records, including 17 repeated-program vectors. Pool
attribution follows the exact parent route and native invocation occurrence.
All 213 previously normalized financial legs in this sample are unchanged;
these event records are not counted again as additional trades.

JupiterZ RFQ fills (protocol 49) use the actual maker identity and declared
taker accounts. Forty finalized transactions across seven makers independently
match exact token/SOL debits and receipts, including ten native input wraps,
four native output unwraps and two balance-proven Token-2022 outputs. Rent is
excluded from swap atoms. All eight public native settlement combinations have
core synthetic regressions; native input unwrap and native maker wallet paths
have no finalized sample in this corpus. Evidence, source revision, IDL hash
and raw transaction hashes are in `rfq-evidence.json`. The legacy `amm_pool`
field denotes the real RFQ maker for this protocol; pair columns distinguish
its separate asset markets. Consumers must not treat that address as AMM
reserve state.

The core suite passes 97 tests. These additions do not resolve Phoenix or
OpenBook fills from existing exchange balances, DefiTuna two-hop swaps, 1DEX
exact-output, Jupiter v4 log attribution or unknown/new program variants.
Unsupported candidates remain diagnostic and never become guessed prices.

Primary RFQ reference:
https://github.com/jup-ag/rfq-webhook-toolkit/tree/3aa4fce515d4b5827ee2d48f08041ea706d911cd/programs/order-engine
