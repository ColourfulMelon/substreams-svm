# Native venue validation

The Pluto normalized decoder adds GoonFi V2, BisonFi, Manifest, HumidiFi,
classic Orca V2, AlphaQ, Kipseli, Flux, Scorch, Obsidian, TesseraV, Deriverse
and ZeroFi in `dex-swaps/src/native_venues.rs`. Protocol enum IDs 21–33 are
appended; existing IDs and the ClickHouse swap schema remain unchanged.

## Helius comparison (2026-10-08)

217 distinct successful, finalized mainnet transactions were fetched from
Helius using `getTransaction` with JSON encoding and
`maxSupportedTransactionVersion: 0`. They include native swaps, routed bridge
legs, pricing updates and other non-swap calls. An independent JSON/CPI audit
compared the decoder's source instruction, program, pool, user authority,
input/output mints and exact atomic amounts to the RPC transaction. Finalized
`getMultipleAccounts` responses independently checked market account owners.

The audit found **211 native legs across 64 pools, with zero mismatches**:

| Venue | Native legs | Recognized variants |
| --- | ---: | --- |
| GoonFi V2 | 17 | tag 1, 18/19 bytes |
| BisonFi | 33 | tag 2/7/19, 18/19/153 bytes |
| Manifest | 14 | swap tags 4/13, 19 bytes |
| HumidiFi | 12 | decoded selectors 4/15/20 (25 bytes), 28 (113), 50 (137) |
| Orca V2 | 9 | tag 1, 17 bytes |
| AlphaQ | 16 | tag 12, 18 bytes |
| Kipseli | 13 | swap discriminator, 25 bytes |
| Flux | 5 | tag 3, 18 bytes |
| Scorch | 52 | tags 1/2, 34 bytes |
| Obsidian | 3 | tag 1, 9 bytes |
| TesseraV | 15 | tag 16, 18 bytes |
| Deriverse | 4 | spot swap tag 26, 32 bytes |
| ZeroFi | 18 | tag 16, 17/18/57 bytes |

These are targeted coverage samples, not estimated traffic shares. They do
not establish support for every historical or future instruction variant.

66 representative protobuf transaction fixtures retain 86 native legs and
non-swap examples in `dex-swaps/fixtures/native-venues/`. `cases.tsv` stores
the exact expected source, pool, user, mints and atoms.
`helius-evidence.json` records signatures, slots, fixture SHA-256 hashes and
SPL/Token-2022 program evidence. The committed regression test checks every
expected native leg and rejects duplicate physical CPI identities across
native events and router wrappers.

To independently inspect any fixture, request its recorded signature from
Helius:

```json
{"jsonrpc":"2.0","id":1,"method":"getTransaction","params":["<signature>",{"commitment":"finalized","encoding":"json","maxSupportedTransactionVersion":0}]}
```

Resolve account keys as static keys followed by loaded writable and readonly
keys. Walk each outer instruction followed by its inner instructions, retaining
stack heights. For each recorded native source, compare only immediate token
CPIs inside that invocation: user-to-input-vault debit and output-vault-to-user
receipt. Amounts must match the recorded atoms, not a router total or instruction
minimum. For implicit Token-2022 fees, a unique incoming transfer, known recipient
balance delta and all outgoing debits must prove the receipt. Reject evidence
with other mint/burn/close operations. Fetch the recorded pool account at finalized
commitment and verify its owner against the README program ID (Scorch exception
below). Re-run the fixture suite with:

```sh
cargo +1.85.1 test -p dex-swaps -p svm-dex --locked
```

## Corrections and exclusions

- **Scorch:** Jupiter's `ojh19...` label points to the pricing program. Updates
  there are not trades. Swaps execute in `SCoRcH8c...`; the market account is
  owned by the separate `ojh19ojaKduoJZuaJADhcVGp4xt1TcdAvZmpVsCorch`
  pricing program.
- **TesseraV:** account 0 is shared global state; account 1 is the individual
  market. Using the global account would merge unrelated pools.
- **Orca/Manifest:** the requested input is a limit; rounding or partial matching
  can debit less. Publish the actual debit, never the requested amount.
- **Deriverse:** tag 85 leveraged position changes are excluded. Only verified
  spot swap tag 26 is recognized.
- **HumidiFi:** pricing update payloads are excluded. Swap XOR decoding uses
  byte operations and explicit little-endian offsets for WASM portability.

Unknown opcodes, missing stack depth, wrong authorities/vaults/mints, duplicate
flows, failed transactions and unproven net outputs emit no native swap. Synthetic
tests cover explicit and implicit 3% Token-2022 deductions, a fully withheld
output, invalid fees and unavailable receipt evidence; these are distinct from
the real finalized SPL/Token-2022 samples.

Layout references were cross-checked against the
[Manifest program](https://github.com/CKS-Systems/manifest),
[Deriverse SDK](https://github.com/deriverse/kit),
[Magnus router adapters](https://github.com/LimeChain/magnus/tree/extend-router-ixs/crates/router/src/adapters),
[PMM simulation account definitions](https://github.com/LimeChain/pmm-sim), and
[public native transaction samples](https://github.com/DefaultPerson/solana-dex-parser-go).
Finalized token flows determine the amounts; parser labels alone are insufficient.

## Manifest owner-layout correction (staged after pluto.12)

The official program at
[`5718d3eed80fa7472a826f346c0668644ff32138`](https://github.com/Bonasa-Tech/manifest/tree/5718d3eed80fa7472a826f346c0668644ff32138)
uses the same `process_swap` and `SwapContext::load` for opcodes 4 and 13.
Both accept either a shared payer/owner prefix or a separate-owner prefix.
The required System-program position distinguishes the two prefixes; the
opcode does not. The decoder binds the native direction, actual owner, token
accounts and market from that validated prefix. Its amount is an input limit,
and `out_atoms` is checked against the verified output receipt.

A finalized sample exposes five opcode-4 separate-owner calls and one opcode-13
shared-owner call that the earlier opcode-based layout missed. The correction
recovers all six while preserving all 131 previously decoded financial legs
in the 65-transaction sample. Together with owned fee metadata, that sample
restores 60 financial legs beyond its original 77; each added amount pair was
independently checked against the finalized raw token CPI bytes. These bounded
results do not establish complete swap coverage.

Two original protobuf fixtures and their finalized transfer/balance evidence
are retained as `manifest-swap-separate-owner.transaction.pb`,
`manifest-swapv2-shared-owner.transaction.pb` and
`manifest-layout-proof.json` in the native venue corpus. The existing exact
fixture test covers both; core layout tests exercise both opcodes, both owner
forms, both directions, exact-in/exact-out and invalid payload/account prefixes.

This source change has not yet been included in a live release.

## DAMM referral receipts (staged after pluto.12)

The official DAMM v2 program at
[`a85c926607433f23f0ea60f4ca7b1ae92f4156cb`](https://github.com/MeteoraAg/damm-v2/tree/a85c926607433f23f0ea60f4ca7b1ae92f4156cb)
sends the input, then the primary swap output, then an optional referral payment.
The referral account may be the same account as the user's output. The decoder
binds the first two actual direct token transfer instructions to the declared
user, vault, mint, authority and token-program roles, and separately validates
the referral's declared destination. The financial output is the primary net
receipt after its own transfer fee; the referral is excluded. Zero primary
output cannot be replaced by a positive referral, even if an event is present.

In the same 65 finalized transactions, this restores six additional DAMM legs
and preserves all 137 earlier full financial payloads. The resulting 143 legs
include 66 beyond the original no-fee-metadata baseline of 77; each additional
amount pair was independently checked against finalized raw token CPI bytes.
All 140 full financial payloads in the separate 38-transaction DLMM sample also
remain unchanged. These are bounded regression results, not a complete-coverage
claim.

Two original transaction fixtures, independently owned fee-mint bytes and
`damm-referral-proof.json` are retained in `fixtures/token-fees`. The existing
core fee test covers both swap formats, exact primary receipts, one row per
swap, recovery without the swap event, wrong declared or CPI account roles,
zero primary output and missing fee evidence/runtime logs.

This source change has not yet been included in a live release.

## Fluxbeam executed-input limits (staged after pluto.12)

Two original finalized Fluxbeam calls request 480,000,000 and 1,175,237,976
input atoms, but their native input CPIs debit 479,999,757 and 1,175,237,973.
Requiring equality drops both real swaps. The decoder now bounds the actual
debit by the instruction's requested input and checks the primary output
against the minimum. It binds the declared input/output mints and token
programs, user authority, pool authority and user/vault accounts in both native
normalization and final verification. Prices use the actual debit and verified
receipt; the requested amount never substitutes for the transferred amount.

`fluxbeam-rounded-input-1/2.transaction.pb` and `fluxbeam-input-proof.json`
retain the original finalized instructions, transfer order, balance evidence
and source hashes. The core regression test checks exact executed amounts,
duplicates, input/output bounds, malformed opcodes/payloads and changed declared
accounts. The upstream [SPL Token Swap processor](https://github.com/solana-labs/solana-program-library/blob/master/token-swap/program/src/processor.rs)
also recalculates the source transfer from the curve result. That source is
background evidence; these fixtures do not prove Fluxbeam is an identical build.

The correction recovers both sampled Fluxbeam trades and preserves all 143
earlier full financial payloads, producing 145 financial legs across the 65
transactions. All 140 full financial payloads in the separate DLMM sample stay
unchanged. Independent finalized CPI byte checks cover all 68 additional
financial legs beyond the original 77-leg baseline. Full swap coverage and a
live rollout remain outstanding.

## Receipts burned and closed after a swap (staged after pluto.12)

Two finalized DBC transfer-hook swaps pay positive base-token output, then burn
that output and close the previously initialized recipient account. The earlier
conservation check rejected every closed account with an observed balance.
The decoder now retains the observed starting balance and proves the missing
end balance from one committed close, with no reinitialization, post balance
or later token-account access. An omitted post balance without an actual close
cannot supply a zero end balance. Root closes use successful transaction
status; CPI closes require completed ancestors too.

The [pinned Token-2022 processor](https://github.com/solana-program/token-2022/blob/b4724e6e7fecc8fc36295f861a769401bf92b270/program/src/processor.rs)
requires a non-native account's spendable balance and withheld transfer fees to
be zero before closing. Unknown extension operations still invalidate
conservation. Unknown receipts into these closed accounts must conserve their
full gross amounts; a smaller burn cannot be interpreted as an invented fee.

`token-fees/dbc-burned-closed-output-1/2.transaction.pb` and
`closed-output-proof.json` retain the original finalized transactions and
balance evidence. Independent raw-byte inspection finds exactly a primary
receipt, equal burn and root close on each destination, a zero starting balance,
no post balance and no intervening unknown token operation. Core tests cover
both receipts, nonzero starting balances, inconsistent burns, missing balances,
missing/duplicate/wrong closes and completed, truncated, failed or rolled-back
CPI closes.

The correction preserves all 145 earlier full financial payloads and restores
two DBC financial legs, for 147 across the 65-transaction sample. All 140 full
financial payloads in the separate DLMM sample remain unchanged. Four other
sampled DBC calls have zero output and are excluded from pricing. This source
change has not yet been included in a live release.

## Shared Stabble and DLMM fee receipts

A finalized Stabble V2 withdrawal and DLMM Swap2 credit the same Token-2022
receiver in one transaction. Its total increase of 9,004,063,881 cannot establish
either swap's output by itself. Owner-verified mint bytes at finalized context
455848233 retain a stable 300-bps fee schedule from epoch 1029; both the sampled
transaction and metadata read are in epoch 1055. Applying that fee separately to
the actual output CPIs proves receipts of 90,210,921 for Stabble and 8,913,852,960
for DLMM. Stabble's separate fee payment remains excluded from its primary
output. The declared vault program/opcode bounds its nested transfer scope.

The retained transaction, mint bytes, raw instructions, balances and hash proof
are in `token-fees/stabble-shared-output-proof.json`. The existing decoder already
supports this layout; the additional evidence requires per-transfer fee metadata.
The core regression rejects missing or inconsistent metadata and an undeclared
vault program. Expanded fee coverage preserves all 147 prior full financial
payloads and recovers these two legs for 149 across the 65-transaction sample.
The separate DLMM sample preserves all 140 full payloads. This is bounded replay
evidence; the global fee registry must be complete and warmed before live use.

## DefiTuna two-hop swaps

The existing FusionAMM 1.1.4 on-chain IDL also defines `two_hop_swap`
(`c360ed6c44a2dbe6`). Its two pools, three directed mints, explicit vaults and
three token programs differ from the single-hop layout. The decoder consumes
its exact Borsh flags and optional transfer-hook/supplemental-tick slices and
binds exactly three immediate, committed transfers to the declared roles.
Both output receipts require proven net amounts. Request limits bound the
actual first debit and last receipt; neither limits nor whole-wallet deltas
create prices. The intermediate vault transfer supplies the first pool's output
and the second pool's input, with distinct physical leg identities.

The retained finalized transaction at slot 455838453 independently verifies both
program-owned pool states and all six affected account balance changes. It
restores WSOL 13,448,909 → USDC 1,495,460 and USDC 1,495,460 → TUNA 671,227,733.
These are two pool trades, not one aggregate WSOL/TUNA pool. Core tests reject
incomplete, duplicated or unauthorized intermediate transfers, invalid layouts,
failed limits and missing, truncated or rolled-back invocation evidence.
A diagnostic remains if either declared pool leg is unverified. Exact-output
bounds additionally use altered fixture arguments; this audit's observed route
is exact-input, with first direction A→B and second B→A. This staged source has
not yet been included in the live release.

## Manifest deposited-balance fills (staged)

Manifest `BatchUpdate` (opcode 6) can match deposited balances without user
Token CPIs. The pinned official program's `FillLog` is an exact 232-byte record:
market, maker, taker, both mints, fixed-point price, traded base/quote atoms,
order sequence numbers and direction/global flags. Its program source debits
and credits those exact ledger amounts before emitting the log. The decoder
publishes each match once as an orderbook fill, with its native instruction
and runtime log identity. It excludes swap opcodes 4/13, which already publish
aggregate user-transfer rows, so their individual fill logs are not duplicated.

The batch decoder fully consumes Borsh hints, cancels and placement records.
A cancel-only, post-only or global-only batch cannot create a taker intent.
Only a fill emitted directly by the declared market's Manifest invocation is
accepted, after that invocation and every ancestor committed. The taker must
match the declared payer; flags, padding, distinct mints, positive quantities
and the floor/ceiling price arithmetic must agree. Nested foreign program data,
failed ancestors and incomplete invocation logs cannot prove a financial fill.
These are deposited ledger amounts; Token-2022 fees on separate deposits or
withdrawals do not substitute for or reduce the matched ledger quantities.

Core tests construct exact buy/sell events, multiple fill identities and nested
success/failure traces against the pinned ABI. Two unmodified finalized empty
batch fixtures additionally prove that marketable intents with no fill publish
no price. A wider finalized audit inspected 837 successful transactions,
including 1,567 batch calls across 537 transactions, with **zero observed batch
fills**. It preserves all 402 existing full financial payloads in the other
validation samples and creates zero financial rows in these empty batches.
The positive tests are contract-derived; this bounded live sample does not
verify a real deposited-balance fill or establish complete chain coverage.

## Stream and rollout

The decoder uses the existing coverage tables for unverified marketable intents. The output still contains
swaps with the existing 24 fields and compact block counters. Previously missing
trades can increase swap-row volume; native/router deduplication prevents counting
the same physical leg twice. The targeted sample does not predict billing growth.

See [`pluto-decoder-release.md`](pluto-decoder-release.md) for package builds and
cursor-safe deployment. Existing history is not rewritten by deploying this decoder.

## Combined native format validation (2026-10-12)

The finalized `.24` replay at slots456081600–456085680 (4,081 blocks) independently proves **1,123 newly supported native financial tuples**:144 Saber,489 SolFi V2,119 Triangle,55 DLMM swap2,11 DAMM swap2 and305 Hadron. Across these families,2,246 complete user ledgers agree with original RPC/PB instructions, runtime, applicable mint fees and owned pool identities. Router wrappers and liquidity, fee claims, conversions and close_pool calls are classified separately; they are not additional physical swaps. The combined count covers these identified diagnostic sources, not universal chain coverage.

See [Saber/SolFi validation](saber-solfi-native-validation.md), [Triangle validation](triangle-native-validation.md), [Meteora swap2 validation](meteora-swap2-padding-validation.md) and [Hadron validation](hadron-native-validation.md) for exact source pins, receipt/fee semantics, mutation checks and exclusions. Their independent full tuples matched the combined host decoder before the final DAMM pool-binding guard; its targeted positives remain unchanged. Packaged replay and deployment evidence is recorded separately by the release process.

## Independent oracle dependencies and retained non-swap classifications

The Saber/SolFi, Triangle, Hadron and Meteora swap2 fixture oracles require Node.js with `bs58`, `@solana/web3.js` and `protobufjs`. Their evidence inputs resolve relative to each script's own directory. By default dependencies resolve with ordinary Node `require`. An optional first command-line argument selects a dependency provider's `package.json`; an optional second argument selects a different `package.json` provider for `protobufjs`:

```sh
node dex-swaps/fixtures/swap-coverage/hadron-native/independent-oracle.cjs ./proof-deps/package.json
node dex-swaps/fixtures/swap-coverage/hadron-native/independent-oracle.cjs ./app-deps/package.json ./protobuf-deps/package.json
```

The same arguments work for `saber-solfi-expanded`, `triangle-native`, `meteora-swap2-padding`, and their retained `repeat-native` copies. `repeat-native/verify-combined-originals.cjs` compares all 111 additional independently derived financial tuples with the packaged replay and verifies full original RPC/protobuf parity.

[`non-swap-classification`](../dex-swaps/fixtures/swap-coverage/non-swap-classification/summary.json) retains the source-specific Manifest and BLX proof for the finalized 4,081-block scope, slots 456081600–456085680. All 2,047 Manifest batch diagnostics are committed calls without an instruction-scoped FillLog. Three are unfilled siblings of a separate filled source already indexed; that source's exact mints and atoms reconcile to the original 232-byte FillLog. All 53 BLX router calls contain 142 already-indexed physical child swaps. Their exact native source, program, declared pool, bounded execution position and verified financial rows agree with the original committed instruction walk. None requires an additional router price row. This evidence classifies those original calls; it does not establish universal historical or future coverage.

## Live Meteora and Whirlpool exclusions (2026-10-12)

The deployed `.25` audit at slots 456163419–456164844 reported 19 DAMM `swap2` calls and 10 Whirlpool `swap_v2` calls without verified flows. All 19 DAMM calls are canonical exact-input attempts with one input atom, zero output and a zero referral payment. Their committed current `EvtSwap2` records the same exact request and receipts. All 10 Whirlpool calls have zero input and zero output; the committed `Traded` events also report zero amounts and fees. These calls produce no executable swap price.

[`live-meteora-whirlpool26`](../dex-swaps/fixtures/swap-coverage/live-meteora-whirlpool26) retains the 17 finalized original RPC/protobuf transactions, canonical pool/vault/mint ownership, complete runtime mapping and 58 full user-account ledgers. The core regression covers all 29 original source identities and 29 fabricated positive-event mutations, preserving the requirement for a positive committed recipient receipt.

The same live corpus contains 1,468 additional Meteora/Whirlpool liquidity, fee collection and position operations. All 1,497 diagnostic sources match primary instruction discriminators and source-specific original runtime names, have complete successful invocation ancestry and contain no nested native swap sources. None emitted a financial row under that native source. The two newer Whirlpool tags are `increase_liquidity_by_token_amounts_v2` (20 calls) and `reposition_liquidity_v2` (one call), pinned to official `orca-so/whirlpools` source commit `cc683ec92acb5e7606caa3d8e514d9383e9c7c11`. Existing DLMM and DAMM primary pins remain unchanged.

To rerun the 29 zero-output financial proofs, use the dependency provider arguments documented above. To rerun the complete classification, supply the original corpus containing `candidates.json`, `swaps.json` and each signature's `.pb` as the third command-line argument after those dependency providers:

```sh
node dex-swaps/fixtures/swap-coverage/live-meteora-whirlpool26/classify-originals.cjs ./app-deps/package.json ./protobuf-deps/package.json ./original-corpus
```

Full independent finalized RPC/ownership/ledger proof is asserted for the 29 zero-output attempts. Classification of the other 1,468 operations uses retained original protobuf runtime, immutable primary ABI and existing indexed source identities; it does not claim all their financial actions were independently reconstructed.


### Additional Anchor spot venues (combined 26)

Ghosty and Memefun are bound to their program-owned swap IDLs and exact two committed checked-transfer principals. Omnipair additionally binds its native PairUpdate and Swap events to the pool, user and physical amounts. Dr Fraudsworth permits only its declared Token-2022 hook account forwarding and the exact committed Execute child. The retained finalized corpus contains 29 source instructions; the combined core check verifies every exact financial tuple and rejects 319 malformed argument, receipt, authority, decimal, runtime or vault-owner variants.

The Dr Fraudsworth WSOL case `53Qh...`, source 15, starts with 577320 atoms, spends 516117 across its committed transfers and unwraps the remaining 61203 on CloseAccount. The remaining balance is a lifecycle debit and is never added to swap output. Its delegated close authority is established by the exact committed classic SetAuthority( CloseAccount, Some(authority) ) receipt; subsequent account accesses are prohibited. The independent original RPC/PB ledger includes that unwrap and all other token transfers, mints and burns. Fixture roots: `anchor-markets26` and `parent-spot26`.

These are finite finalized source proofs. They do not assert coverage of every future instruction or fee extension.


The v26 Sanctum Infinity native V2 profile additionally retains two finalized invocations and twelve complete token-account ledger equations. Its ABI is pinned to the primary igneous-labs/inf-1.5 source d4bfdd4d1d34985f8639433c0308941fc84bf1a2, with exact native pool/list/program roles, six calculator CPIs, pricing CPI and two checked principal transfers. The shared anchor core now checks 31 financial positives and 341 malformed variants including those two profiles. This proof is in `dex-swaps/fixtures/swap-coverage/sanctum-infinity26`.

SatRush payout classification in `dex-swaps/fixtures/swap-coverage/satrush-payout-classification26` independently verifies 304 finalized native invocations in 178 original transactions with 1,756 complete persistent token-account ledger equations. All retained SettleDeployPublic, RotateRound and ClaimSats legs flow out of program-owned authorities, with no opposing native asset exchange. This finite classification does not exclude other instructions from that program or claim network-wide swap coverage.
