# Finite owned-instruction classification

This original transaction audit covers391 no-indexed-native-child instructions across17 program-owned IDLs in finalized slots456213447–456214372. All203 retained PBs match finalized RPC account keys, complete compiled instruction walk, balances, fees, gas and available runtime logs. Full PB runtime establishes committed ancestry. The current swap corpus contains no indexed source inside these native subtrees; indexed ancestors remain recorded.

The finite sources are liquidity-share deposits/redemptions, lending collateral issuance/redemption/debt repayment, pool/token initialization, issuer share redemption/minting, token wrappers, privacy custody deposits, accrued-fee withdrawals, bridge custody, escrow funding and order creation. Both Jupiter routes explicitly encode100%`JupiterLendRedeem`; their exact burn/output flows are retained. These391 diagnostics add zero DEX spot legs.

Complete protected token ledger proofs cover308 vault calls (616 vault/share ledgers),10 stock-facility redemptions (30 ledgers) and2 privacy deposits (4 ledgers). Other instructions retain full principal flows and native ABI roles; no whole-ledger claim is made for those.

FewR publishes an empty IDL. Its prior owned executable dispatch binds the exact discriminator to stock-facility`SupplyRedeemAndTransferExternal`, reads the requested u64, and the originals prove burned external receipts plus underlying internal-receipt payout. Umbra's prior executable consumes two128-bit fields and a64-bit amount, differing from its later owned IDL. Launchlab and Streamflow retain their two10-byte undeclared suffixes explicitly; known argument prefixes and initializer/escrow flows establish the finite business classification without assigning suffix semantics. No global coverage claim or historical configured exchange-rate assertion is made.

DBC four fee claims bind official source commit`f552f20aa3c1c7631427c3827aeea7c58b902813` (`MeteoraAg/dynamic-bonding-curve`): exact partner claim transfers and event, not reciprocal exchange inputs.

Run offline:

```sh
python3 verify-originals.py --node-dependency-package /path/to/backend/package.json --protobuf-package /path/to/protobufjs/package.json
```

Node must support JSON reviver source context for exact unsafe integers. Optional`--block-cache` accepts finalized containing-block gzip responses; without it no containing-block transaction ordering/hash claim is made. Regenerated classification-proof.json and source checks are fail-closed; raw originals and primary records are immutable inputs.
