# Archived wide Gator originals

160 finalized native Gator legs, independently checked against original retained Firehose PBs and finalized getTransaction JSON. The oracle derives financial amounts from the two committed physical native receipts, never from packaged decoder amounts. Native input argument, payload shape, pool/mint/vault/authority roles, successful complete source ancestry, all token balances and every resolved lamport delta are checked. Primary executable/layout analysis is retained in ../gator-live26/README.md and that directory's owned executable/ProgramData assets.

The output has zero transfer fee for every retained leg: for non-SOL outputs, the original destination's full gross incoming-minus-outgoing token receipt ledger equals its observed token balance change. Since fees are nonnegative, this establishes each incoming fee is zero independently of later mint configuration. Owned mint fee schedules also reproduce the unrelated global token ledger; schedules with mutable authority are not described as immutable historical state. Active transfer hooks are rejected; the one64B TransferHook extension has zero programId and the original complete child frame confirms no hook invocation. Classic Token Batch255 supplies only strict bounded Transfer conservation entries, never fabricated principal instruction identities.

One original SkiEk69... has truncated finalized RPC logs while its retained original PB has all156 logs. The oracle matches the exact available RPC prefix, checks all instruction/role/amount/balance/signature fields, and uses complete original PB invocation ancestry. This limitation is explicitly marked rpcLogsTruncated in its proof. All other available runtime logs agree completely.

Run with optional dependency package paths:

```sh
node oracle.cjs /absolute/path/to/backend/package.json /absolute/path/to/protobufjs/package.json
```

Default resolution also works with Node dependencies available in the normal module path. The fixture directory defaults to the script directory in package-path mode. Raw PBs are retained original source wire bytes, not reconstructed from RPC. Source/sourceTransfer/log positions are explicit in the independent financial proof. Checksums in sha256.json cover all retained evidence.
