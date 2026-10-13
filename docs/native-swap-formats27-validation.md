# Combined native swap formats: .27

This release repairs actual missed spot exchanges identified in the retained finalized diagnostic corpus at slots 456213447–456214372. It adds observed Pump native sell/roundtrip settlements, Pump AMM quote buy-and-burn, Aldrin V1, Cropper, Picon DLMM, M0 full native orders, Patch buyback, Bxlw sells, Penguin SPL swaps, classic Pancakeswap swaps and Sanctum Infinity legacy exact-input swaps. It also admits the independently proved Tessera caller flag 2 and two DLMM host-fee aliases with exact net receipts.

Financial amounts come from committed source-owned transfers, native events, explicit fees, account closures and independently reconciled user/vault balances. Cropper includes its separate input fee in the user's gross debit. Protocol IDs 79–85 are additive; existing venue IDs and physical trade keys are unchanged.

Pump AMM buy-and-burn is an actual quote-funded pool buy that destroys its base output. It carries `burn_output_verified` and `source_burn_index`, with `transfer_verified=false`: the base amount is never represented as a wallet credit. The mapper requires consecutive source/quote/Burn provenance, matching format and nonzero quantities. Price views accept only that explicit verified burn format in addition to existing transfer/fill proofs. Tiny principals retain existing price-eligibility thresholds.

The existing core business tests include real finalized originals plus malformed native and routed candidates. The portable family verifiers under `dex-swaps/fixtures/swap-coverage/*27` retain raw originals, finalized RPC evidence, primary account/ABI contracts and exact financial tuples. Picon and Pancakeswap exact-output regression variants are ABI-derived from real exact-input originals; they are not claimed as separately observed real exact-output transactions. M0 currently accepts full Solana-chain native fills only; partial/foreign fills remain excluded.

Independent classification finds no fills in the sampled Manifest order placements/cancellations; liquidity operations, zero-proceeds swaps, loan creation, receipt redemption and same-direction wallet payouts remain diagnostics. Diagnostic count is not a count of missing swaps. Those exclusions retain original evidence and primary contracts in the respective family fixtures.

The combined package replay tools under `fixtures/swap-coverage/combined27` require immutable .26/.27 packages and completed clean finalized streams. They check existing keys and all 25 prior fields, new burn provenance, operation ordinals, table versions, block parents and sealed counters. Added rows must match independent venue oracles and original PB/finalized-RPC/containing-block metadata. Preparation or incomplete streams cannot attest a release.

The seven metadata module hashes and exact frozen metadata WASM remain unchanged. Release builds use `--no-default-features`. Historical recovery requires canonical replacement in an isolated replay workflow; normal deployment must preserve the exact durable cursor and never replay historical financial rows into existing aggregates.

This is a finite observed-format validation. It does not establish universal coverage of unobserved or future native formats. Unproved instructions and fee configurations remain retained diagnostics rather than fabricated prices.
