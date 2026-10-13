# Penguin and legacy PancakeSwap native swaps

The `.27` decoder adds two observed native instruction families. It does not claim universal Solana swap coverage.

## Penguin SPL Token Swap

Four finalized transactions in slots 456214188–456214217 execute Penguin opcode 1. The 17-byte instruction has the canonical SPL Token Swap account prefix: pool, pool authority, user transfer authority, user input, input vault, output vault, user output, LP mint, LP fee recipient, classic Token program. The input is a maximum. Canonical constant-product rounding leaves zero, three or four atoms unspent in these originals; the emitted input is the actual principal debit, with no tolerance or substitution of the requested amount. Minted LP fees are separate from swap proceeds.

The portable proof in `dex-swaps/fixtures/swap-coverage/penguin-native27` checks all four complete original protobufs against finalized RPC instructions, balances and runtime logs. It independently derives the exact financial tuples and reconciles 20 whole user/vault/fee-recipient ledgers. The owned 324-byte pool state binds the token program, two vaults and mints, LP mint and fee recipient. The stored bump derives the native pool authority. Classic vault and LP mint ownership are checked. Current reserves and mint supply are not represented as historical snapshots.

Primary contract evidence is retained in `spl-swap-forks/primary`: Jupiter's official source identifies Penguin as an SPL Token Swap fork; the SPL processor and instruction contract define the amount cap, authority seeds and LP fee CPIs. Penguin's upgradeable program deployment slot is 161376048 and its retained ELF SHA-256 is `93622e51c0582563f50e4e16e9912c3768026fc51c9f741ae2b791dbc33614f6`, predating all originals. This is an executable pin, not a claim of a reproducible source build.

The existing core test covers four original native positives and four routed verification positives, plus 132 native and 132 routed malformed assertions. Mutations cover calldata, amount cap/minimum, pool/authority, account roles, principal receipts, LP fee minting, vault/fee balances and failed runtime. The previous eight Dooar/Dexlab positives and 160 helper mutations still pass. Existing Dooar/Dexlab behavior is unchanged; the stronger PDA and persistent ledger checks apply to Penguin.

## PancakeSwap legacy Swap

Three finalized originals use the older classic-token `Swap` discriminator `f8c69e91e17587c8`. Previously only the newer `SwapV2` native layout was accepted. The program-owned IDL establishes the 41-byte arguments, ten fixed accounts, optional remaining tick/bitmap accounts, and canonical 205-byte SwapEvent. The official `@pancakeswap/solana-core-sdk` 0.2.1 supplies pool, vault, observation, tick and bitmap PDA contracts and owned state layouts.

The portable proof in `pancake-legacy27` establishes three exact financial tuples and 12 complete user/vault ledgers. It checks complete original protobuf/finalized RPC/runtime parity, the program-owned IDL, owned pools/mints/vaults/ticks/bitmap, all canonical PDAs, two immediate committed classic principal transfers and one native-scoped event. Pool identity, sender, ordered user accounts, direction, exact amounts and zero classic transfer fees must agree. Current pool state is used for immutable identities, not historical pricing.

The program deployment slot is 432564695 and the retained ELF SHA-256 is `5227d6ae48e15c1703aa2fc341b8184a56499ed9695edcd5d2550ee6a61da84f`, predating the originals. The SDK tarball and owned IDL hashes are retained with the evidence. The decoder preserves existing SwapV2 behavior.

The core test covers three real exact-input positives, three explicitly ABI-derived exact-output variants and six routed verification positives. It rejects 222 native and 222 routed malformed assertions, including changed opcode, width, amounts, PDAs, accounts, CPIs, event fields, vault balances and runtime completion. Derived exact-output variants are not counted as additional finalized observations. A malformed native layout cannot fall through routed generic verification. Legacy Token-2022 swaps are excluded by the classic-token ABI.
