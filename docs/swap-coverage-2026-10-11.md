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

## `.9` additions

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

Raydium Launchpad now normalizes its four typed buy/sell exact-input/output
variants from scoped user/vault transfers even when its event version is unknown.
29 finalized BuyExactIn/SellExactIn fixtures independently prove transfers
across six program-owned pools, including 12 Token-2022 output receipts.
Seventeen existing input amounts are corrected from the pool's post-fee
receipt to the gross user debit. These legs use native protocol 14,
are deduplicated against Jupiter and contribute to mint pricing. Unknown
instruction opcodes remain excluded. See `launchpad-evidence.json`.

The core suite passes 98 tests. These additions do not resolve Phoenix or
OpenBook fills from existing exchange balances, DefiTuna two-hop swaps, 1DEX
exact-output, Jupiter v4 log attribution or unknown/new program variants.
Unsupported candidates remain diagnostic and never become guessed prices.

Primary RFQ reference:
https://github.com/jup-ag/rfq-webhook-toolkit/tree/3aa4fce515d4b5827ee2d48f08041ea706d911cd/programs/order-engine

The `.9` packaged 50-slot replay emits 6,591 unique swaps versus 6,574 live
`.8` rows. It preserves every existing event identity and all financial fields
except 17 independently proven Launchpad gross-input corrections. It adds
12 Launchpad, two 1DEX and three RFQ legs; 277 candidate instructions and
242 raw candidate transactions remain diagnostic. Historical live rows and
aggregates are not changed by deploying this release.

## Phoenix settlement accounting (pluto.10 candidate)

`dex-swaps/src/phoenix.rs` now decodes signed Phoenix self-CPI audit batches
against owner-verified 576-byte market headers. It supports IOC Swap /
SwapWithFreeFunds and marketable Limit / LimitWithFreeFunds packets. Each fill
requires a complete successful invocation chain, correct log authority, parent
instruction/market/trader identity, contiguous event indices, complete batches,
and a single matching FillSummary. The summary's client ID, base lots and quote
lots must agree with the typed order and maker fill events. Lot multiplication
uses checked arithmetic. Buy quote amounts include the taker fee; sell quote
receipts subtract it, as the official matching engine does. Deposits, posted
order sizes, zero-fill summaries and incomplete evidence never become trades.

The public contract is pinned to
[Ellipsis-Labs/phoenix-v1 at 6ae54af](https://github.com/Ellipsis-Labs/phoenix-v1/tree/6ae54af5f07f4d0ae8430b08cbe81d10432cd34c).
The 74-transaction finalized RPC sample contains 17 filled swaps and 57
unfilled marketable orders. All 17 decoded settlements match an independent
maker-fill/fee calculation and exact scoped Tokenkeg transfers, with both trade
directions represented across two markets. The committed subset contains all
17 fills and three unfilled orders; details, raw hashes and market-header
provenance are in `dex-swaps/fixtures/phoenix/evidence.json`. No finalized
ledger-only fill was observed. Deposited-fund fills, marketable limit fills,
both directions, split event batches and rolled-back/malformed evidence have
core synthetic tests; those tests are not on-chain sampling evidence.

The pluto.10 candidate wires this accounting into normalized swaps as protocol
`phoenix` (50), with `orderbook_fill_verified` and `source_event_index`. Ledger
fills keep `transfer_verified` false and use distinct `signature:instruction:ledger:log`
identities. Missing metadata preserves the original replayable diagnostic.
An already verified wrapper for the same native invocation is not counted twice.
Raw and aggregate price eligibility accepts either proven settlement type.

The bounded registry starts after finalized snapshot slot 455747960, with 255
owner/PDA-verified markets and 168 mint decimal records. The packed snapshot and
its public provenance are under `dex-swaps/fixtures/phoenix/market-registry-*`.
Successful InitializeMarket instructions update market definitions, including
new addresses or reinitialized markets. Legacy mint decimals are captured from
Phoenix transaction balance metadata; unresolved definitions wait for that
metadata instead of assuming decimals. Reads use transaction/instruction
ordinals, so a later initialization cannot alter an earlier fill in the same
block. A snapshot marker is mandatory before the map emits any block.

All 106 core Rust tests pass, including initialization ordering, rolled-back
initialization, missing metadata, truthful ledger flags and sink event identity.
An actual Substreams replay after the boundary passes the registry guard.
A second runtime replay of finalized slots 455754000–455754049 preserves all
5,134 previous financial rows and event identities exactly, with 261 candidates
and 219 raw candidate transactions. Isolated ClickHouse checks all eight enum
columns, 41,072 candle trades, 5,134 pool trades and 10,268 mint trades. Nine
price assertions include ledger-only prices and candles. All three Go readiness
checks and eleven read-only application price tests pass. All 388 existing raw
transaction audits remain byte-identical (962 financial legs, 185 diagnostics).

The registry does not establish historical market incarnations before the
snapshot. Historical wallet examples are independently bound to exact SPL CPIs;
ledger-only historical recovery needs initialization/account-state evidence.
The new release remains a tested candidate until its immutable artifacts are
published and the single writer is upgraded. Published pluto.9 assets remain
unchanged. This does not complete OpenBook, DefiTuna two-hop, 1DEX exact-output,
new/proprietary native programs or the other remaining coverage gaps.


## OpenBook immediate-fill candidate after pluto.10

The next source candidate recognizes OpenBook v2 `place_take_order` for IOC
and market orders using the deployed v1.7 snake_case Anchor discriminator
`032c47031ac7cb55`. The old OpenBook IDL instruction constants hash camelCase
names and are not evidence for the actual instruction format.

The native path validates the exact typed payload, base/quote direction, Token
and System program roles, immediate user/vault transfer scope, input authority
and the market's output authority. Prices use actual wallet token debits and
receipts, rather than the lot-denominated order limits or a fill summary that
can include referrer rebates. Protocol 51 identifies the real OpenBook market.

Independent finalized RPC evidence contains 47 fills (four bids, 43 asks)
across one market, nested at depths two and three. All 47 actual CPI identities,
pools, users, mints and atom amounts agree with the candidate; the sample's
127 previously normalized financial rows are unchanged. Fixtures cover both
v0 and v1 transactions and reject wrong order types, declared Token programs
and output authorities. There are no observed rebated wallet fills in this
sample. Core Rust validation now has 107 passing tests.

Primary contract: [OpenBook v1.7 at 796a470](https://github.com/openbook-dex/openbook-v2/tree/796a470033bc556c1d1a1f76709394fc50c63bf7).
Public transaction provenance is in `dex-swaps/fixtures/openbook/evidence.json`.

This native candidate was published as source commit f02de5f, before the
ledger integration below. Published pluto.10 assets remain unchanged.

## OpenBook ledger accounting after the immediate-fill candidate

The next source revision adds completed taker accounting for `place_order`,
`place_orders`, `cancel_all_and_place_orders`, `edit_order`,
`place_order_pegged` and `edit_order_pegged`. Runtime `TotalOrderFillEvent`
amounts represent atoms actually paid and received, including taker fees.
They are scoped to the exact native invocation and only admitted after that
call and its entire ancestor chain complete successfully. Batch summaries
remain individual fills with separate runtime-log identities; their positions
use the upper half of the existing instruction key to avoid collisions with
compiled Token CPIs and signed Phoenix event instructions. Existing financial
IDs are unchanged.

A finalized owner-verified snapshot at slot 455779697 supplies 283 market
headers. All market authority and both vault PDAs, positive lot sizes, fee
constraints and mint pairs were independently checked. The registry captures
future successful creations using their signed native metadata self-CPI,
and invalidates closed markets at their exact execution ordinal. Snapshot
records and reconstructed creation fields have separate type tags. Both the
Phoenix and OpenBook snapshot guards must pass before any financial output.

The 116 passing core tests cover actual-amount accounting in both directions for all six
ledger order families, malformed and foreign events, failed/truncated ancestor
chains, batch multiplicity, closed/recreated markets and non-colliding sink
identities. A further finalized sample of 161 successful transactions contains
157 immediate takes, two unfilled batch placements, one consume-events call
and one closure. The retained batch/closure fixtures publish no swap prices.
This sample does **not** contain a real ledger-only taker fill; positive ledger
accounting is validated against the primary contract and synthetic core cases.

Maker event consumption realizes a fill already executed and reported by the
original taker; it must not create a second financial swap. The primary contract
also defines DecrementTake self-matches as normal matches without fees on that
component. The decoder preserves those reported matches, but the summary does
not identify the self-matched component for a separate price-quality filter.

This revision remains unpublished as a package and undeployed until immutable
artifact validation and controlled cursor migration succeed. It does not close
DefiTuna two-hop, 1DEX exact-output, Jupiter v4 attribution, proprietary/new
programs or the other remaining coverage gaps.


## Chronological positions for mixed ledger/wallet transactions (pluto.12)

Final pre-deployment review found that the pluto.11 candidate's reserved log
identity keys sorted ledger fills after later wallet transfers in the same
transaction. That release was never deployed and is marked unsuitable for
rollout; its immutable assets remain unchanged.

The successor keeps the same unique financial row identities and adds an
independent `execution_position` on every new row. For transactions containing runtime fills,
it ranks all fills by their actual compiled native/transfer/event coordinate,
then by runtime log order within the same native call. Candles and recent-pair
states use that chronological position. Wallet-only fills retain their original
instruction position. Every new raw row supplies the position explicitly for the SQL sink, whose insert column list
is fixed by the live schema. Legacy rows default to their old instruction
position; existing financial amounts, pools and event IDs remain unchanged.
No aggregate type, historical aggregate state or financial amount is rewritten.
Mixed ledger/wallet and batch ordering are covered by core and real SQL tests
before the corrected package can be deployed.

The exact pluto.12 package passes 116 core Rust tests, three Go readiness tests
and twelve application price tests. Production-mode finalized replay of slots
455790670–455790719 preserves all 5,523 previous financial rows and their
chronology, adds the same independently verified OpenBook wallet fill, and
explicitly supplies positions on all 5,524 rows. Isolated ClickHouse validation
passes fifteen price assertions, including mixed ledger/wallet execution across
insert batches and all eight candle intervals. Its temporary database was removed.

## Existing-view upgrade companion

The production metadata check found that `IF NOT EXISTS` retained the previous
recent-pair insert-view definition on an existing installation. Fresh isolated
schemas had the correct execution-position definition, but the upgrade needed
to replace it explicitly. The startup schema now replaces that materialized
view while retaining its aggregate table, types and prior observations.
An isolated existing-view upgrade preserves a pre-upgrade price and passes
sixteen price assertions, including subsequent mixed wallet/ledger execution
across batches and all eight candle intervals. Production metadata confirms
both price insert views use execution positions. No reordered runtime-fill row
had reached the live view when its definition was refreshed atomically.
The published pluto.12 decoder package is unchanged; existing installations
must apply this companion migration before admitting mixed runtime fills.
