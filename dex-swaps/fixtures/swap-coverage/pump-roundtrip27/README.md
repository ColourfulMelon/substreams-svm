# Pump native V3 same-curve round trips

26 finalized native sell legs; preceding committed buy reserve witnesses determine each sell’s constant-product gross proceeds. Exact ceiling fees, buyback debit, actual account creation/volume refund and complete native/token conservation prove net output. Ten inverse intervals were ambiguous before binding the preceding buy. No residual allocation or mutable global reserve guess.

Scope: slots 456213447–456214372 from decoder .26 diagnostic originals. Original protobuf wire bytes and gzip-compressed raw finalized JSON-RPC responses are retained; gzip decompression recovers the exact response bytes. Requests used `getTransaction` with `commitment: finalized`, `encoding: json`, and `maxSupportedTransactionVersion: 0`. No operational settings are exported. Each oracle verifies original PB/RPC keys, complete walk/depth/data/account identities, signature, slot, all native and token balances, fee, complete runtime invocation ancestry and logs before deriving amounts. Explicit RPC truncation flags are derived from retained response logs; none in this scope were truncated.

Run with installed `bs58`, `@solana/web3.js`, and `protobufjs`, or supply package manifests:

```sh
node oracle.cjs /path/to/backend/package.json /path/to/protobufjs/package.json
```

`independent-proof.json` contains financial legs and whole ledger witnesses. `swap-candidates.json` pins exact source identities. `primary-pump-idl.json` is the primary Pump/PumpSwap ABI used by that oracle. Core tests use the original numbered PB fixtures and mutate native arguments, roles, amounts, runtime ancestry, events and final balances. New roundtrip/burn/v2 coverage is finite; no universal swap support claim.
