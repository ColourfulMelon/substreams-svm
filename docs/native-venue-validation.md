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

## Stream and rollout

No diagnostic rows or new stored tables are introduced. The output still contains
swaps with the existing 24 fields and compact block counters. Previously missing
trades can increase swap-row volume; native/router deduplication prevents counting
the same physical leg twice. The targeted sample does not predict billing growth.

See [`pluto-decoder-release.md`](pluto-decoder-release.md) for package builds and
cursor-safe deployment. Existing history is not rewritten by deploying this decoder.
