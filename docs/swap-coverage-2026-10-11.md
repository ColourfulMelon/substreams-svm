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

## Jupiter v4 route attribution candidate (pluto.13)

The v4 second-pass decoder used transaction-global latest-pool history and the
fee payer. It now accepts only the route program's own committed runtime events
and binds each event to an unconsumed, completed AMM call inside that route.
Repeated visits, later routes and preceding v6 routes cannot replace a pool.
A typed route or exact-output adapter supplies its declared transfer authority;
an absent authority or ambiguous pending calls leave the event unverified.
Failed ancestors and incomplete traces cannot establish financial settlement.

Three original finalized transactions retain twelve manually audited v4 events,
including repeated Whirlpool calls and a BonkSwap leg. Fixture proofs record
signatures, slots, byte hashes, declared pool positions and native coordinates.
Core mutations cover a sponsored fee payer, unknown authority, foreign program
data, ambiguous repeated calls and failed/truncated ancestors. These mutations
are constructed checks, not additional observed mainnet trade types.

The 60-transaction v4 sample contains 89 route calls and 270 v4 events. Full
financial payload comparison preserves its 288 existing native financial rows
and all 402 previously audited rows from the recognized, DLMM and DefiTuna
samples. This change does not demonstrate new financial legs in that sample;
native decoders already supplied them. All 144 core tests and the release WASM
build pass. The source candidate remains separate from the deployed immutable
pluto.12 package, pending complete fee metadata and controlled pluto.13 rollout.

## Legacy Orca V1 native swaps (pluto.13 candidate)

Two previously unclassified finalized Jupiter v4 legs execute in Orca V1
(`DjVE6JNiYqPL2QXyCUUh8rNjHrbz9hXHNYt99MQ59qw1`). Jupiter's public SPL
Token Swap implementation identifies this program; the legacy SPL v2.0.0
instruction source declares its ten-account swap prefix and optional host fee.
This differs from the later Token-2022 SPL swap account extension.

The decoder uses protocol `SPL_TOKEN_SWAP` with the actual Orca V1 program and
pool identity. Only opcode 1's complete swap payload and declared legacy Token
program are accepted. Executed, committed immediate token transfers establish
the user debit and receipt, including pool/user authority and vault/mint roles.
The debit must satisfy the instruction's input maximum and the receipt its
minimum output. Extra user transfers, ambiguous legs, caught failures or an
incomplete ancestor trace cannot provide a financial row. Router events share
these checks and cannot bypass them.

The independent oracle retains two original finalized transactions and later
program-owned, initialized pool headers. The headers corroborate immutable
vault/mint/PDA fields, not historical reserve amounts. Exact CPI atoms restore
5,914,841,313 KIN into 655,895 wrapped-SOL atoms and 46,517 wrapped-SOL into
5,175 USDT atoms. Thirty-eight core mutations across the two fixtures check
bounds, opcodes, payloads, roles, missing/duplicate CPIs and failed/truncated
ancestors. All 145 core tests pass. Full payload comparison preserves all 690
previous audited financial rows and adds precisely the two oracle-proven legs;
the two Orca V1 unclassified candidates disappear from that 60-transaction
sample. Other unclassified programs remain and are not covered by this result.

## Mercurial N-coin exchanges (pluto.13 candidate)

Two unclassified finalized legs execute in Mercurial's stable-swap program
(`MERLuDFBMmsHnsBPZw2sDQZHvXFMwp8EdjudcU2HKky`). Its published instruction
and state source at `bd691381904564135681d29f182adf5f29abcdf8` defines opcode 4,
an input amount/minimum output and a two-to-four-vault array. User source and
destination follow that entire array. The decoder recognizes that layout and
normalizes executed exchanges as the appended protocol `MERCURIAL` (52);
existing IDs remain unchanged and Rust bindings are regenerated from the schema.

Exact, committed immediate classic-Token CPIs establish the user debit and
receipt, declared user/pool authority, distinct declared vaults and vault/mint
roles. Both transfers must exist in order, the debit must equal the input
argument and the receipt must meet minimum output. Extra token operations,
ambiguous/missing legs, unrelated vaults, liquidity opcodes and failed or
incomplete ancestors cannot be priced. Routed events use the same receipts.

The independent proof retains two original finalized transactions, owned SwapV2
pool state/PDA and three owned Token vaults. Current reads corroborate immutable
roles, not historical reserves or admin flags. The restored transfers are
20,072 USDT atoms into 20,105 USDC atoms and 21,123 into 21,195. Fifty mutations
across the two fixtures cover settlement bounds, roles, operation identity,
additional token calls and failure/trace completeness. Constructed ABI checks
cover two-, three- and four-vault positions; observed positive fixtures are
three-vault exchanges. All 147 core tests pass, and full payload comparison
preserves 692 prior financial rows while adding exactly those two verified legs.

The SQL sink maps protocol 52 to `mercurial`. Both standalone and startup schemas
extend all four financial enum columns before accepting new rows. An isolated
existing-schema upgrade preserves the prior aggregate and all 5,525 replayed
rows, verifies eight dependent enum columns and passes 21 price assertions,
including the Mercurial amounts in verified recent pairs, pool/mint counts and
all eight candle intervals. Only these isolated price checks assign controlled
current timestamps; historical source transaction timestamps are not refreshed.
The temporary database was removed. Live pluto.12 and its schema are unchanged;
apply the companion migration before the controlled pluto.13 rollout.

## Oasis / MarcoPolo native receipts (candidate .13)

Added protocol `oasis = 53` for the observed 33-byte Anchor `swap` format on
`9tKE7Mbmj4mxDjWatikzGAtkoWosiiZX9y6J4Hfm2R8H`. The primary account-role
source is the owner-verified Jupiter v4 on-chain IDL (`marcoPoloSwap`, context
455877687). Oasis has no standard on-chain IDL. Its owner-verified deployed
program data (deployment slot 232074629, read context 455876059) identifies
MarcoPolo instruction names and seeds; all three independently owned pools
match `marcopoolv1` plus the declared mint pair. State and authority match
`marcostatev1` and `MarcoPolo`. Later owned reads corroborate immutable pool,
vault, mint and authority roles, not historical reserves or fees.

The finalized 60-transaction Jupiter v4 sample contained ten previously missing
Oasis legs, on three pools and in both directions. Each has two positive classic
Token transfers followed by a **zero** referral transfer. Four original protobuf
transactions, ten independent amount-oracle rows, owned account bytes and the
primary IDL role definition are retained in `fixtures/native-venues/oasis-*`.
The decoder requires the exact program, opcode, payload length, boolean,
17-account role layout, declared mints, user and vault authorities, input limit,
and all three committed immediate Token calls. Failed or incomplete ancestors,
extra/nested operations, nonzero referrals and other formats stay unverified.
This is evidence for this observed format, not all Oasis variants.

The financial core test covers all four retained originals and 148 negative
mutations. All 148 decoder/sink tests pass; release WASM builds pass. Replay
preserves all 694 earlier complete financial payloads, plus 175 payloads from a
new 60-transaction BONKSwap sample, and adds exactly ten Oasis legs (879 total).
BONKSwap's third transfers in that sample are also zero; no BONKSwap decoder
change was needed. The ten new legs match the independent user/vault CPI oracle
exactly and are emitted once even when Jupiter also logs a wrapper event.

The companion pricing schema adds the Oasis enum to every financial aggregate;
coordinate it with the .13 rollout. The live .12 package has not been replaced.
The full Token-2022 registry, store warmup, remaining variants and historical
backfill still need completion before claiming exhaustive transaction coverage.


## Runtime log truncation and replay depth (candidate .13)

A finalized, successful transaction proves that its root committed. If the
runtime emits its exact `Log truncated` marker, completed child calls already
returned into that root remain committed even when its final success log is
missing. Unfinished inner frames and their children remain unproven: a later
failure may be caught by the root. Parsing stops at the first marker because
the runtime collector can retain shorter messages after dropping a larger one.
See the pinned [Solana log collector](https://github.com/solana-labs/solana/blob/49443406fd729c5818ecf58ae773592804d35d89/program-runtime/src/log_collector.rs)
and [transaction atomicity](https://solana.com/docs/core/transactions).

The original `spl-swap-recent-1` transaction reproduces a fully committed SPL
swap before a later unfinished DAMM frame. Core tests reject missing/spoofed
markers, failed status/ancestors, malformed root depth and pending inner work,
including suffix success logs after truncation. The separate `recent-56` original
retains an actual short log after the marker.

Three existing DLMM rows change only their descriptive stack height from 3 to
2, matching original compiled instructions. Program, pool, user, mints, amounts
and physical source identities remain identical. Replay ignores this metadata
field when comparing previous financial payloads, so it neither reinserts these
trades nor refreshes historical timestamps. It still refuses changed amounts,
missing trades and reassignment of a known pool.


## Canonical SPL Token Swap actual receipts (candidate .13)

The canonical `SwaPpA9LAaLfeLi3a68M4DjnLqgtticKg6CnyNwgAC8` program now
shares the strict native receipt path with legacy Orca V1 under existing
protocol `spl_token_swap = 19`. Its 17-byte opcode-1 instruction declares an
input maximum and output minimum; these bounds are not actual traded amounts.
The decoder requires exact immediate classic-Token input/output transfers,
user/vault authorities, mint roles, invocation commitment and bounds. Between
the transfers it permits only the declared pool-token owner fee `MintTo` and
optional host fee `MintTo`, with the exact pool mint, recipients and authority.
Extra/nested/foreign calls, wrong fee roles and liquidity instructions cannot
become financial rows. The sources are SPL's pinned
[instruction layout](https://github.com/solana-labs/solana-program-library/blob/token-swap-v2.0.0/token-swap/program/src/instruction.rs)
and [processor](https://github.com/solana-labs/solana-program-library/blob/token-swap-v2.0.0/token-swap/program/src/processor.rs).

Three original Jupiter v4 transactions prove previously missing owner-fee swaps,
including ECH input 694,921 atoms below its declared maximum of 694,937.
A separate finalized sample has 61 SPL legs in 60 transactions: nine no-fee
and 52 owner-fee variants, with native call depths 2 and 3. Ten transactions
have runtime-truncated logs; completed root children remain provable as above.
One original creates and closes its WSOL destination within the transaction,
so its mint is proven by explicit initialization rather than balance snapshots.
The owner-verified initialized SwapV1 pools and authority PDAs match every
sample's immutable vault/mint roles. Later owned state reads do not prove
historical reserves. Six representative original protobuf transactions are
retained; all 64 signatures, hashes and independent user/vault amount rows
are recorded in `fixtures/native-venues/spl-swap-*`.

All 151 decoder/sink core tests pass, including 99 malformed-call mutations,
root rollback/truncation checks and both transient-output and post-marker log
variants. Release WASM builds pass. Across seven transaction corpora, replay
preserves all 1,061 prior financial payloads (three corrected descriptive depths
aside) and adds exactly 64 canonical SPL legs, for 1,125 total. Each new leg
matches the independent actual-CPI oracle in program, pool, user, mint pair,
amounts and physical source positions. No enum migration is needed for this
existing protocol. Positive host-fee samples, other Token Swap forks, complete
Token-2022 metadata and historical backfill are still separate coverage work;
this sample does not establish exhaustive coverage.


## Native SOL Pump curve buys and funded V3 sells (candidate .13)

A finalized native SOL curve run at slot 455884188 had only its token-to-token
leg indexed. The documented multi-hop interface passes WSOL accounts as mint
witnesses while actually debiting the user's wallet into the curve account;
the curve's WSOL vault need not exist. The native receipt path now requires the
exact committed immediate wallet-to-curve System transfer and curve-authorized
base-token receipt, instruction amount, mint path and original physical source
positions. Additional wallet/curve SOL flows invalidate this proof. Delegated
PumpSwap wrappers still do not duplicate or relabel their child curve legs.
The restored first leg is 31,238,843 lamports to 896,182,574,050 token atoms.
Its previously indexed second leg remains unchanged.

A separate finalized V3 sell at slot 455884113 funds the curve with 76,200
lamports before transferring base tokens. The old proof rejected every explicit
pool-touching System transfer. The new proof accepts exactly one committed
wallet-to-curve funding CPI before the committed token input and requires the
original event CPI to commit too. Pool debit plus funding must exactly equal
net sale proceeds plus the declared buyback recipient's observed credit. Other
pool-bearing calls, repeated/outgoing/foreign/nested funding, failed event or
transfer frames, and mismatched pool/recipient deltas remain unverified. The
sale is 71,818,190,491 base atoms to 1,988,665 lamports; the funding is excluded
from sale proceeds. Independent original RPC/protobuf instruction/log parity,
user/vault atom deltas, pool/recipient lamport equations, derived curve/vault
PDAs and later owned immutable quote fields are retained in
`fixtures/swap-coverage/pump-native-sol-*`. Later state does not prove past
reserves or fees.

All 153 decoder/sink core tests pass, including 31 mutations of these originals.
Release WASM builds pass. Eight-corpus replay preserves all 1,200 earlier full
financial payloads, adding exactly these two native SOL legs (1,202 total), with
no other changes. Existing protocol 9 already feeds indexed pricing.
A wider 48-hour retained multi-hop sample has 90 original candidates in 59
transactions and still has 28 parent/child diagnostic rows after these fixes.
Those remaining routes, native SOL multi-hop sells, synthetic migrations and
PostCompleteBuy receipts still require validation and coverage work.

Primary references: [multi-hop account and native SOL roles](https://github.com/pump-fun/pump-public-docs/blob/main/docs/instructions/MULTI_HOP_SWAP.md),
[V3 account funding and fees](https://github.com/pump-fun/pump-public-docs/blob/main/docs/instructions/TRADE_V3.md)
and [Pump IDL](https://github.com/pump-fun/pump-public-docs/blob/main/idl/pump.json).


## Known Pump intents with missing settlement evidence (candidate .13)

Coverage diagnostics now recognize typed Pump curve trades and PumpSwap trades
through their exact existing instruction decoders and pool extractors. Native
SOL payouts can have only one Token mint and no output System CPI; they still
need a diagnostic row if no financial leg can be proved. The financial decoder
and pricing gate are unchanged by this recognition. An original V3 sell core
case with opaque funding/event evidence verifies that one visible mint produces
`recognized_swap_without_verified_flows` and no swap for that instruction.
All 153 core tests and release WASM builds pass. The original transaction is
retained by the existing diagnostic sink for later repair, without publishing
its intent as a price.


## Native SOL multi-hop sells (deployed .14)

Twenty-three retained original finalized transactions contain missing final curve
legs on six pools. Their Token-2022 gross input debits and reserve receipts,
native events, pool lamport debits and buyback lamport credits were independently
checked. The observed native buyback credit uses a committed empty self-CPI
and the recipient's canonical WSOL SyncNative; that helper is distinct from a
second pool swap. [Pump's multi-hop contract](https://github.com/pump-fun/pump-public-docs/blob/main/docs/instructions/MULTI_HOP_SWAP.md)
requires native SOL receipt in the user wallet on the final SOL curve sell,
with the output WSOL account serving as a mint witness and the fee WSOL account
receiving lamports before synchronization.

The decoder retains each physical curve leg. It requires the exact original
input amount, source authorities, canonical token programs, committed native
and token/event ancestry, a single matching native sell event and matching
pool/buyback deltas. Exact source-token conservation independently checks gross
debits; actual reserve receipts check net tokens, including transfer fees.
An extra pool-bearing call or explicit SOL flow cannot borrow the same balance
proof. Current reserves and current fee absence never stand in for historical
execution evidence. Remaining native settlement formats stay diagnostic.

All twenty-three recovered legs match the independent original-transaction oracle.
Two core tests cover these originals and 33 inconsistent amount/account/depth,
fee settlement, synchronization, balance and failed/truncated-call mutations.
All 157 core tests pass. Nine replay corpora preserve all 1,216 previous full
financial payloads exactly and add only these fourteen legs (1,230 total).
A separate retained 47-transaction corpus preserves all 73 previous full financial
payloads and recovers 23 final sell legs (96 total); fourteen overlap the earlier
corpus. Its finalized RPC/protobuf call and runtime-log parity, exact Token-2022
debit/receipt balances, native event and pool/buyback lamport equations, and later
owned immutable pool/vault roles are retained with the original fixtures.

A read-only finalized remote replay of blocks 455929225..455929274 exits cleanly
for both .13 and .14, preserves all 4,576 previous swap fields/keys (excluding only
the decoder-version label), and adds one independently verified sell to reach
4,577. Its exact input is 1,034,697,410,229 base atoms and net SOL output is
35,057,362 lamports. Only the corresponding verified/diagnostic block counters
change. Packaging passes. The frozen metadata binary and its seven module hashes
remain pinned to .13's artifact; only the pricing binary changes.

Release .14 is published and PR #18 is merged. The exact merged pricing image
was deployed at 2026-10-12 06:18:30 UTC with strict hash checks retained. A
graceful stop saved block 455944582; all four financial/diagnostic tables had
no rows beyond it. The exact opaque cursor was copied under the new output hash.
The first actual .14 block is 455944583 with that exact parent slot/hash.
Observed new swaps are unique, nonzero and verified; actual parent links and
sealed coverage counts match. Original auto-deploy settings are restored, and
the production API remains healthy with custom orders disabled. Pricing is
still catching up at this initial observation; freshness remains a separate
check. No replayed financial history is appended. The live and recent-registry
proofs are retained under `fixtures/native-venues/release14-*.json`.

A separate post-catchup observation at 2026-10-12 06:52:57 UTC shows actual
sink block 455954566 with 4.99 seconds lag, stream block 455954570 with
3.99 seconds lag and no stream errors. The sequential RPC observation is
not atomic with the sink read; its head can trail by a few slots. The initial
catchup proof is preserved unchanged, alongside `release14-freshness-proof.json`.


## 1DEX aliased fee recipients (candidate .15)

A retained finalized exact-input swap declares the same account in both fee
recipient positions. Its two committed vault-paid fee CPIs transfer 201 atoms
each to that account. The prior guard rejects equal actual fee destinations even
when both declared roles explicitly alias. The source repair allows that exact
alias while retaining the rejection of duplicate payments when the declared
recipients differ.

Original finalized RPC/protobuf calls and runtime logs match exactly. Isolated
pool-vault and fee-account balances and complete user-account conservation prove
the gross input of 20,157,333 WSOL atoms and net output of 2,254,971 USDC atoms.
The 402 fee atoms already belong to the input debit. Later owned PoolState mint,
vault and authority roles corroborate the declared accounts; current reserves
never prove past execution amounts.

All 158 core tests pass, including this original, an aliased-recipient synthetic
case and the existing rejection cases. Full-payload replay preserves all 1,042
previous published-fixture financial rows and adds exactly this one leg. The
recent native SOL sample preserves all 96 rows. All three .15 packages build,
and their frozen metadata binary and all seven metadata module hashes match
the pinned artifact. A clean-exit read-only finalized 50-block remote replay
preserves all 4,577 swaps, every financial and diagnostic row field/key/ordinal
except the decoder-version label, parent links and sealed counters. This range
does not contain the older aliased-recipient positive; its original finalized
RPC/PB proof and core replay provide that evidence separately.
Release .15 is published, and PR #19 is merged as
`108d18428540ad9c3aac41da0ed6018147056b4b`. The exact merged shared-pricing image
deployed at 2026-10-12 07:07:28 UTC. A graceful stop saved slot 455958071 with
no rows beyond it in the four financial/diagnostic tables. The exact opaque
cursor was adopted under the new output hash with strict checks retained.
The first actual .15 block is 455958072 with that exact parent slot/hash.
Initial observations show 373 unique linked blocks and 31,105 unique, nonzero,
verified swaps, with sealed coverage counts matching. Queries are sequential,
not atomic. Pricing remains about five minutes behind at this observation;
freshness is a separate pending check. The production API stays healthy with
custom orders disabled; its backend is not deployed. Original automatic-deploy
settings are restored after actual writes are verified. No historical financial
rows are replayed into the sink. The live and preflight evidence is retained
under `fixtures/native-venues/release15-*.json`. Retained
originals and independent proofs are in `fixtures/swap-coverage/onedex-aliased-*`.

A separate observation at 2026-10-12 07:25:40 UTC proves .15 has caught up:
actual sink slot 455963568 and stream slot 455963570 are both about 3.47 seconds
behind, with no stream errors. A subsequent finalized RPC head is 13 slots
ahead of the saved sink observation; the reads are sequential. The initial
catchup proof remains unchanged. This observation is retained separately in
`fixtures/native-venues/release15-freshness-proof.json`.

## Pump V3 shared buyback recipients (published and deployed .16)

A retained original finalized transaction at slot 455950129 contains a V3 sell
at instruction 11 and a V3 buy at instruction 23, on distinct curves, with the
same declared buyback recipient. Exact token CPIs and whole-transaction debit/
receipt balances independently prove both base legs. The sell's pool debit is
229,642,420 lamports: net user proceeds 228,543,099 plus buyback fee 1,099,321.
The buy's System CPI transfers gross user input 228,543,098 lamports, and its
pool receives 227,470,921 after the 1,072,177 buyback fee. The shared recipient
receives exactly 2,171,498, the sum of both fees; neither pool has another native
invocation, and no explicit System transfer touches that recipient.

Original RPC/protobuf instructions and runtime logs match. Later owner-verified
curve accounts and derived base vaults corroborate immutable roles only, never
past amounts or fee absence. The `.15` decoder verifies the buy but leaves the
sell diagnostic because its recipient guard requires one invocation and an
isolated single-leg recipient credit. The originals and per-leg/aggregate proof are retained
as `fixtures/swap-coverage/pump-v3-shared-buyback-recipient.*`.

A repair must verify every contributing native call's event, exact token
settlement and independent pool lamport equation before using an aggregate
recipient credit. Unknown contributions, overlapping pool activity, failed
ancestors or inconsistent credits must remain diagnostic.

Release .16 implements that proof for V3 buys/sells on distinct curves. Each
contribution requires canonical mint/token/program roles, one completed event,
an exact committed base-token CPI, independent source-token conservation,
proven net receipt and gross wallet funding where present. Native event amounts
and typed argument bounds must match. Its isolated pool lamport delta must equal
the fee-adjusted settlement. Aggregate recipient credit is accepted only when
the number of proven contributions equals every Pump invocation touching that
recipient, their checked fee sum equals its exact credit, and no explicit SOL
transfer touches it. Other formats remain diagnostic rather than supplying
unverified contributions.

All 159 core tests pass, including the original transaction and 37 inconsistent
settlement mutations. Thirteen replay corpora preserve all 2,405 prior full
financial-payload occurrences. Exactly one independently proven sell is added,
appearing in both the published fixture corpus and a recent recognized-swap
corpus; these are overlapping observations of the same trade. The financial
proof is retained beside the original. All three .16 packages build, with the
frozen metadata binary and all seven module hashes unchanged. Both versions
exit cleanly on a finalized 50-block remote replay containing the original.
All 4,138 old swaps retain their exact financial fields and keys, excluding
only the decoder-version label; the proven sell increases the total to 4,139.
Its one resolved candidate/raw diagnostic disappears. Only that block's
verified/coverage/corrected/quarantine counters change by the expected amounts.
Database-operation ordinals are checked against the exact two diagnostic
removals and one swap insertion: later ordinals shift by one in that block,
while stored execution positions and physical identities stay unchanged.
Parent links and sealed counters match. No replay writes to live financial
tables.

Release .16 is published to the existing fork and its package pin is merged
in monorepo PR #20. Deployment `5ko2ddhgcgwfvpkez0jpck1c` finished at
2026-10-12 08:25:31 UTC. Its source commit includes a later mobile release
workflow change; all pricing build inputs match the tested merge. The old
writer's durable slot 455979715 and hash were preserved exactly, with no old
rows beyond the cursor in any of the four tables. The first actual .16 block
455979716 has that exact parent. Strict module-hash checks remain enabled,
and original automatic-deploy settings are restored. The production API
remains healthy with custom order creation/execution disabled; this rollout
does not trigger a production backend deployment.

After initialization and catch-up, actual sink/stream observations at
09:02:24 UTC show 5.20/4.20 seconds of lag and no errors. A later read-only
database check verifies 1,114,657 unique nonzero verified swaps, 12,648 unique
linked blocks, zero invalid rows or broken parents, and matching sealed
coverage counters, at four seconds of pricing lag. Queries are sequential.
The evidence is retained in `fixtures/native-venues/release16-*.json`.
Read-only priming validated 9,024 blocks and 748,409 swap rows before its
planned bounded termination; its exit is not described as a clean replay.
The separate finite package replay above exits cleanly. Legacy/V2 settlement,
repeated same-curve swaps and other unverified variants still need work;
this rollout does not establish universal coverage.


## Legacy and V2 native sell settlement (published and deployed .17)

The decoder now isolates net SOL proceeds for canonical legacy and V2 Pump
sells. A committed event alone cannot establish a price: matching token CPIs,
original source conservation, aggregate curve/fee balances and the complete user
SOL ledger must agree. Rent funding and committed refunds, temporary wrapped SOL,
network fees and other known transfers are kept separate from native settlement.
Unknown or failed contributors remain diagnostic. Distinct users may sell into
one curve; multiple native payouts to the same user still require per-leg proof.

Original finalized RPC evidence proves fourteen previously rejected sells in
eleven transactions. Three previous golden sells used gross pool debits instead
of net proceeds; their output amounts alone change, from 416523623 to 411317077,
19970987 to 19721349, and 154093086 to 152166921 lamports. Every other financial
field and physical identity remains unchanged. Original proof files retain token
debits, fee allocation, wrapping, close-account refunds and native balance equations.

All 161 core tests pass, including 51 mutations of legacy/V2 settlement and
volume-account rent refunds. Fourteen retained corpora preserve 2405 of 2409 prior
full financial occurrences exactly, make only four proven output corrections,
and drop none. Thirty-nine added occurrences deduplicate to twenty-five unique
physical legs. Frozen metadata is unchanged. Package validation, publication and
a coordinated strict-cursor rollout must precede any claim of live .17 coverage.

A packaged 50-block comparison additionally exposed 84 gross-to-net sell-price
corrections and one temporary wrapper rent refund. Original finalized RPC evidence
proves all 85 complete native/user/source equations; the compressed originals and
owned immutable curve roles are retained. The wrapper creates and destroys one
account with 772160 lamports of rent. Its only other destination exactly matches
the committed service-fee transfer, so the destroyed remainder returns to the
original funder. Ten mutations reject inconsistent ownership, access, refund and
alternative-destination balances. The enlarged retained corpus adds that original
previously gross-priced sell as a fourth output-only correction.

The final .17 package exits cleanly on slots 455948150 through 455948199:
3409 prior swaps become 3410. All 3324 unaffected rows retain every field/key
except decoder_version; 85 original RPC-proven sells change output_amount alone.
One independently proven sell is added. Only its resolved raw/candidate diagnostics
are removed, corresponding counters change and database operation ordinals follow
the exact insertion/removal shift. Physical trade positions remain unchanged.
All parent links and sealed counts agree. The frozen metadata binary and all
seven module hashes are unchanged. The package is validated for publication;
.16 remains the active writer until a coordinated strict-cursor rollout.

Release .17 is published to the existing fork and PR #22 is merged as
`cfcf3b37a7009801e61796cd48373b994d75ac01`. Coolify deployment
`qewxn9ilmvjudol8dxzqsarc` finished at 2026-10-12 10:04:19 UTC using that
exact source. The old writer was actually exited before cursor adoption; its
saved block 456006564 and all four old-version tails are unchanged. The first
actual .17 block is 456006565 with the exact saved parent hash. At the retained
10:17 check, 189695 nonzero verified unique swaps and 2297 linked unique blocks
have no invalid rows, duplicates or broken parents; sealed counters agree.
Both original automatic deployment settings are restored. Production API remains
healthy with custom order creation and execution disabled. No historical financial
rows were appended. The separate read-only primer verified 5163 blocks and 429027
swaps before deliberate termination after live boundary validation; it is not a
clean finite replay.

Freshness recovered without restarting the advancing stream: at 10:37:45 UTC,
sink and stream drift were about 3.8 seconds, with no reported errors. A later
finalized RPC read put the saved cursor two slots behind. The retained live
proof contains the subsequent financial uniqueness, exact restart boundary,
parent-chain and sealed-counter validation. This establishes actual freshness
for this rollout, not universal swap coverage; the remaining independently
unverified formats still need work.

## Verified Tessera checked-transfer variant (.18 withdrawn)

A bounded 2901-slot .17 diagnostic range (456009160–456012060) retained 52
DFlow candidate transactions and 124 per-leg CPI receipts. Nine Tessera legs
were absent from stored swaps. Original finalized RPC instruction and token
balance parity, exact direct TransferChecked amounts/roles and owned market
accounts independently prove these omissions. DFlow wrapper fees and already
indexed native legs are not counted again. Other unmatched receipts remain
under investigation, including prediction-market legs and amount disagreements;
they are not all classified as missing trades.

The new Tessera opcode 17 uses nineteen bytes and fourteen accounts. Only the
observed zero trailing flag is supported. Direction, declared input/minimum,
market/user/vault/mint/program roles, output authority and completed invocations
must match its two immediate token transfers. Unknown flags, malformed payloads,
wrong roles and failed/incomplete calls remain excluded. Opcode 16 is unchanged.

Twelve original core fixtures (nine recent and three previously retained) pass
252 negative mutations. All 162 core Rust tests pass. Seventeen overlapping
corpora preserve every financial field of all 2787 prior occurrences, change
and drop none, and add twelve occurrences. Original RPC reads can truncate
logs; the proof compares their exact prefix before the first truncation marker
and all instructions/token balances, retaining the full original response.

The exact .18 package exits cleanly on finalized slots 456009180–456009229:
4298 prior swaps retain every field/key except decoder_version and fourteen
original RPC/owned-market-proven Tessera swaps are added. Fourteen resolved
candidate rows and twelve no-longer-needed raw rows are removed; all other
diagnostic fields/keys are unchanged. Parent links, sealed counters and unique
contiguous per-block database insertion ordinals agree. A second clean replay
has identical bytes. All seven metadata hashes and the frozen artifact remain
unchanged. Full proofs and original data are retained under
`dex-swaps/fixtures/native-venues/tessera-checked/`.

Preparation rejected .18 because all four sink tables still emitted the old
.17 decoder_version label. The previously retained financial replay excluded
that field and therefore did not validate provenance. The public .18 release
is marked withdrawn and was never deployed; immutable assets/tag remain for
audit. Its adoption PR was merged while automatic deployments were paused,
so the live writer continued using fresh .17 prices.

The .19 correction uses one shared version constant for all four sink tables.
Existing core tests now verify financial/block and diagnostic/raw row versions
against the price, aggregate and ClickHouse package manifests. Finite replay
also requires the expected version on every emitted row before financial
comparison. All seven metadata hashes remain unchanged. The Tessera decoder
and original financial evidence are unchanged. The exact .19 package exits
cleanly over the same fifty finalized blocks, validates the expected version
on every row and preserves all 4298 prior swaps while adding the same fourteen
original-verified legs. All parent links, sealed counters and operation ordering
agree. The source/release is published and deployment PR #24 is merged.
Deployment f06sqyxm071gwpplautiqgqr uses exact commit
d920ff2b31895691ef5e5b1d345011f2c1f9ce44. The old writer exited at saved block
456026634; all four tails were empty and the identical opaque cursor was backed
up/adopted with strict hash mode. The first actual .19 block 456026635 has the
exact saved parent hash. Live unique nonzero financial rows, linked blocks and
sealed financial/diagnostic counters pass. Both original automatic deployment
settings are restored, and production custom creation/execution remain disabled.
The owned read-only primer verified 5700 blocks and 553794 swaps before deliberate
termination/reaping. Wall-clock catch-up remains monitored. Complete coverage
of all swap types is still unproven.


## Independently retained next native gaps

Two unmatched DFlow receipts now have original finalized RPC instruction,
token-balance and exact log-prefix parity plus current program-owned market
corroboration. Retained originals are under
`dex-swaps/fixtures/swap-coverage/dflow-native-gaps/`. Current owned market reads
corroborate identity; they are not historical reserve evidence.

At slot 456009465 HumidiFi executes an eighteen-account call with the existing
25-byte XOR layout and decoded opcode 48. Its scoped direct SPL transfers
prove 8974777 wrapped SOL in and 1000030 USDC out. Opcode 48 is not included in
.19's known 4/15/20 branch; the independent native settlement remains missing.

At slot 456009179 Jupiter Lend AMM executes a 24-account swap_in call, with
2017780 USDT input and 2015842 USDC output. The input transfer is immediate;
the output transfer is one level deeper inside liquidity operate. The retained
program-owned AMM IDL and official Jupiter liquidity IDL at commit
7960a14a5a760abc41db3ed1395dc5cb56a1f8db pin these roles. Liquidity operate
has 105 bytes and thirteen accounts, not an inferred 89-byte layout. The
observed AMM call omits the current IDL's final associated-token-program account;
compatibility needs bounded shape validation before adding it. This protocol
is not yet decoded. Prediction-market receipts still need primary layout and
settlement evidence. None of these newly retained cases is a coverage claim or
an authorization to price router totals.


The next source change adds only the independently proven HumidiFi opcode-48
direction-zero shape: exactly eighteen accounts, canonical clock/classic token
programs, directed users/vaults/mints and exactly two committed immediate SPL
transfers with the pool as output authority. Other directions, lengths and
unproven layouts remain diagnostic. All 163 core Rust tests pass, including
the original swap and 25 negative cases. Eighteen overlapping corpora preserve
all 2801 prior full financial occurrences, modify/drop none, and add two
occurrences of the same independently proven HumidiFi leg. This source fix is
not in the live .19 package; versioned package replay and rollout remain pending.
Jupiter Lend and prediction-market normalization are still pending.


Nine retained Tri3 originals now match finalized RPC instructions, token
balances and exact log prefixes. Declared native inputs match their immediate
TransferChecked debits, and immediate inventory outputs match router output
receipts. All nine router input receipts also include outside-native fees;
those totals must not replace native input amounts. Original evidence is in
`dex-swaps/fixtures/swap-coverage/tri3-native/`. At context slot 456033192, all
selected current market/maker states and outcome mints are unavailable; this
read does not establish historical ownership or mint-fee state. Historical
market and net receipt evidence are still pending, and no Tri3 decoder was
added. Source/receipt differences alone are not classified as lost transactions.
