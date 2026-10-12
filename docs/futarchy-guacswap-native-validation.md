# Futarchy and Guacswap native validation

The fixed finalized scope is slots 456163419–456164844. Public originals, full RPC responses, retained protobufs, program-owned Anchor IDLs, program/ProgramData records, raw/parsed owned account metadata and an independent oracle are under `dex-swaps/fixtures/swap-coverage/futarchy-guac26`. The oracle derives all 16 previously missing financial tuples and reconciles 32 complete original user ledgers. It compares full keys, instruction walks, lamport snapshots, token balances and runtime invocation logs between RPC and protobuf; every selected native invocation and descendant is committed.

Futarchy has nine canonical 25-byte `spotSwap` instructions, with exact 9-account roles. Two classic-token principals and an Anchor event CPI settle each trade. `SpotSwapEvent` binds the DAO, user, direction, exact input/output and minimum; its tagged post-AMM state binds base/quote mints and vaults. The event is 406 bytes for Spot state or 670 for Futarchy state. Program-owned `Dao` records bind the same immutable mints/vaults and owned token vaults use the DAO authority. Both Buy and Sell directions are covered. Conditional outcome conversion and liquidity instructions are excluded from this spot-only module.

Futarchy executable deployment 423005106 predates the originals, with ELF SHA256 `753daac67ed0393dc6b3678420ead88814205780eae13cacb5dbafdb179bf8d6`. The retained IDL file SHA256 is `1f262fa06490896ad6694212d9323868cfa2f33b348784f79dbe25f84386181e`; the oracle independently decompresses the program-owned IDL account and compares the exact parsed ABI.

Guacswap has seven canonical 33-byte `swap` instructions, with exact 17-account roles and three classic-token CPIs. The first two settle user input and output; the third pays the declared separate output-mint referral account. That referral is excluded from user proceeds. Pool PDA identity is derived from the primary IDL seeds `guacswappoolv1`, token X and token Y. Owned 417-byte pool state binds both mints/vaults; initialized owned vaults bind the canonical program authority. Both directions and native/root invocation forms are covered.

Guacswap uses only the actual positive principal debit, bounded by the typed delta-in request. A request with unused input never inflates the priced amount. All seven finalized originals exhaust their request; seven request-bound invariance checks exercise actual debit below the bound, without claiming finalized partial-execution evidence or inventing a minimum-output field from `priceLimit`. The program's successful native execution enforces its opaque fixed-point price limit. Output referral aliasing with the user's principal account is not present in these originals and is excluded by this strict layout.

Guacswap executable deployment 263699928 predates the originals, with ELF SHA256 `127f070b4c2b3ed764721b0946a0c33d926de62ba4b6b1872c5e8d315d202d48`. The retained IDL file SHA256 is `19308991e754739f3428a383dfe9cbcdb44546af34003a867bd29afd2ac2ffa3`; exact program-owned IDL decompression is verified by the oracle.

Core tests passed: nine Futarchy originals and 90 invalid mutations across the three observed direction/post-state shapes; seven Guacswap originals, 60 invalid mutations across both directions, plus seven bounded-request invariances. Pool substitutions, native widths/roles, zero principals, wrong transfer accounts/depths, event amounts/identities, failed invocations and missing commitment are rejected. These are source-level financial proofs, not universal venue coverage or historical reserve claims.

Run the portable independent oracle using Node with `bs58`, `@solana/web3.js` and `protobufjs` installed:

```sh
node dex-swaps/fixtures/swap-coverage/futarchy-guac26/independent-oracle.cjs [dependency-package.json] [protobufjs-package.json]
```
