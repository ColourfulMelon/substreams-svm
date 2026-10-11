# DEX Swaps

`dex-swaps` emits a normalized protobuf stream of swap activity across supported Solana DEX protocols.

Each emitted swap includes:

- protocol enum
- protocol program ID (`amm`)
- pool or market identifier
- user wallet
- input token mint and amount
- output token mint and amount

Only protocols that can reliably provide the full normalized swap shape are included.

Notes:

- `protocol` is a protobuf enum with stable protocol IDs.
- Downstream packages such as `svm-dex` can materialize that enum into lowercase snake_case strings such as `jupiter_v6` and `raydium_amm_v4`.
- `amm` is the protocol program ID as raw bytes.
- `amm_pool` is the stable pool or market identifier when the protocol exposes one.
- Pricing rows require a resolved pool or market identifier and verified realized token flows. Unsupported exchange candidates are retained separately for replay.
- Jupiter currently uses `fee_payer` as the best available user-level identity from the decoded event stream.
- Native adapters resolve actual token mints from account balances or validated initialization instructions, then verify scoped user/vault transfers.

Current coverage:

- Boop
- Darklake
- DumpFun
- Jupiter v4
- Jupiter v6
- Orca Whirlpool `SwapV2`
- Pump.fun
- Pump.fun AMM
- Raydium AMM V4
- Raydium CLMM `V2`
- Raydium CPMM
- Raydium Launchpad
- Meteora AMM
- Meteora DAAM
- Meteora DLMM
- Saros native swaps
- OKX DEX V2
- SPL Token Swap-compatible programs

Currently excluded:

- Sanctum
  Current decode exposes only the swap amount and does not provide normalized pool, mint, or finalized output fields.
- DFlow
  Current decoder is a placeholder that treats any instruction as a swap and does not expose reliable normalized swap fields.

## 1DEX native swaps (source change after pluto.8)

`PROTOCOL_ONEDEX = 48` recognizes `swap_exact_amount_in` from the
1INTRO program `DEXYosS6oEGvk8uCDayvwEZz4qEyDJRf9nFgYCaqPMTm`.
The 24-byte payload must declare the actual user input and minimum output;
11 account positions identify the pool, authority, input/output vaults,
trader, token accounts, fee recipients and legacy Token program.
The published [FYEO assessment](https://files.1intro.com/public/SecurityCodeReview1DEX.pdf)
identifies the instruction and `PoolState` types in audited private source
revision `41a82354f27b49a1befcdddbb0f4628e3d167e8e`.
Actual amounts and account positions are independently checked against 40
unmodified finalized transactions and the program-owned pool state, rather
than relying on inferred router prices. The evidence and SHA-256 hashes are
in `fixtures/swap-coverage/onedex-evidence.json`.

Both directions of the sampled SOL/USDC pool match. In 37 samples the
pool pays two fees from the input vault after receiving the full user
input. These fees are already included in gross user input; the decoder
checks their declared recipients and authority without adding them again.
Three samples have no fee CPIs. Exact-output swaps, other pools and future
instruction variants still need independent sampling. This source change
is not included in the immutable pluto.8 release packages.
