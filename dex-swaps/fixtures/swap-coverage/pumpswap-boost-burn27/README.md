# PumpSwap boost buy and burn

72 finalized reserve-funded purchases followed by actual pool-base burns. Protocol 79 explicitly records quote debit and burned base: it does not claim trader base credit. Actual TransferChecked and Burn children, source/minimum bounds, native Buy/Boost events, PDA/ownership identities and complete native/token ledgers prove every tuple. Two transactions close an empty boost quote vault and refund exact residual lamports.

Scope: slots 456213447–456214372 from decoder .26 diagnostic originals. Original protobuf wire bytes and gzip-compressed raw finalized JSON-RPC responses are retained; gzip decompression recovers the exact response bytes. Requests used `getTransaction` with `commitment: finalized`, `encoding: json`, and `maxSupportedTransactionVersion: 0`. No operational settings are exported. Each oracle verifies original PB/RPC keys, complete walk/depth/data/account identities, signature, slot, all native and token balances, fee, complete runtime invocation ancestry and logs before deriving amounts. Explicit RPC truncation flags are derived from retained response logs; none in this scope were truncated.

Run with installed `bs58`, `@solana/web3.js`, and `protobufjs`, or supply package manifests:

```sh
node oracle.cjs /path/to/backend/package.json /path/to/protobufjs/package.json
```

`independent-proof.json` contains financial legs and whole ledger witnesses. `swap-candidates.json` pins exact source identities. `primary-pump-idl.json` is the primary Pump/PumpSwap ABI used by that oracle. Core tests use the original numbered PB fixtures and mutate native arguments, roles, amounts, runtime ancestry, events and final balances. New roundtrip/burn/v2 coverage is finite; no universal swap support claim.
