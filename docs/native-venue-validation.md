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

## Stream and rollout

No diagnostic rows or new stored tables are introduced. The output still contains
swaps with the existing 24 fields and compact block counters. Previously missing
trades can increase swap-row volume; native/router deduplication prevents counting
the same physical leg twice. The targeted sample does not predict billing growth.

See [`pluto-decoder-release.md`](pluto-decoder-release.md) for package builds and
cursor-safe deployment. Existing history is not rewritten by deploying this decoder.
