# Native swap formats in pluto.27

The runtime is pinned to source `14826d2ced83d367af4afb6e46fb0bf79316e024`.
New native programs and settlement variants use source-bound immediate physical
receipts, exact primary layouts, owned immutable pool/authority roles, requested
input caps/minimum proceeds, committed ancestry and complete principal ledgers.
Unknown native layouts remain diagnostic; generic router totals never replace
native financial principals.

| Additional legs in the three final package replays | Count |
|---|---:|
| PumpSwap protocol-funded buy-and-burn | 613 |
| Aldrin V1 | 37 |
| Cropper | 19 |
| Picon DLMM | 11 |
| M0 native order | 5 |
| Bxlw | 3 |
| Patch buyback | 1 |
| Pump native repeated/V2 settlement | 29 |
| Penguin SPL Token Swap | 17 |
| Sanctum Infinity legacy | 7 |
| Pancake native legacy | 5 |
| Meteora DLMM host-fee alias | 2 |
| Tessera flag 2 | 1 |

Buy-and-burn output has a separate verified Burn identity and never invents
trader credit. Whole original quote/burn/event identities, PDAs and financial
ledgers bind that protocol's price. Protocol IDs 79–85 are appended without
reassigning existing IDs. Older rows have zero burn metadata.

The [combined package proof](../dex-swaps/fixtures/swap-coverage/combined27/README.md)
covers 6,429 finalized blocks: 876,427 unchanged rows, 750 added legs and 1,221
bounded Pancake provenance corrections, with zero lost or unexpectedly modified
rows. All 1,971 additions/corrections are independently matched across 27 fields,
including transaction/block metadata. Family originals and oracles are retained
under `dex-swaps/fixtures/swap-coverage/`; the combined family registry lists every
non-overlapping live and broader-window proof.

The remaining observed live native diagnostic groups are exhaustively classified
in [owned instruction classification](../dex-swaps/fixtures/swap-coverage/owned-instruction-classification27/README.md),
[Pump-related non-spot settlements](../dex-swaps/fixtures/swap-coverage/remaining-pump-null27/README.md)
and [remaining native non-spot operations](../dex-swaps/fixtures/swap-coverage/remaining-native-null27/README.md).
Their combined finite scope is 82 groups, 765 native sources and 50 programs.
LP operations, fee claims, lending/staking, bridge/custody transfers, game issuance
and child-swap router wrappers do not create spot prices. Explicit retained RPC
runtime limitations are checked against full original PB runtime, rather than
silently treated as complete RPC logs. Opaque primary ABIs are documented as such.

All 229 decoder/mapper core tests pass. The schema integration runs actual startup
SQL twice against isolated ClickHouse 26.7 and verifies historical row/price
preservation, readiness of all eight enums and strict burn-pricing gates. The
release does not replay old financial rows into live aggregates. Finite coverage
cannot guarantee all unobserved current or future swap types.
