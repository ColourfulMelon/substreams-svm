# Omnipair native leg in the Onedex aliased-fee regression original

The existing immutable finalized `onedex-aliased-fee-recipients` transaction contains an independently proven Omnipair swap at flattened native source index 8. This proof derives the financial tuple from the original protobuf/finalized RPC transaction and production program-owned IDL, not from decoder output.

The native request spends 10,120,621 `omfgRBnxHsNJh6YeGbGAmWenNkenzsXyBXm3WDhmeta` atoms and receives 1,753,261,255 `storenSbvkfzircixnaosc5CbzNZVrHJ6S3EKrS1yqR` atoms. The two physical principal receipts are flattened indices 10 and 11. Their enclosing native UpdatePairEvent and SwapEvent are indices 9 and 12. All four immediate children and the source commit successfully under the complete original runtime walk.

The portable verifier checks all original keys, signatures, fee, compute units, pre/post lamports, every token balance field, full instruction walk and complete runtime logs against finalized RPC. It independently decodes SwapArgs, Pair, UpdatePairEvent and SwapEvent with the retained program-owned Borsh ABI; proves current owned pair/reserve-vault PDA identities and mint decimals; binds original signer, pool/user token ownership, input direction, actual amounts, minimum output and input-fee allocation; and reconciles each of the four principal token accounts across every committed transfer, mint and burn in the whole transaction. Current owned state authenticates identities rather than historical reserve balances.

```sh
node verify-originals.cjs /path/to/apps/pluto-backend/package.json /path/to/protobufjs/package.json
```

Expected: one original, four complete principal token ledgers, source 8, input 10120621, output 1753261255. This finite proof supports the explicit additional Omnipair tuple in the legacy regression test; preservation of the other four emitted financial rows is checked separately by the runtime regression test.
