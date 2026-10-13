# Manifest no-fill classification, finalized .27 audit

Run the independent verifier with Node supporting exact JSON source contexts:

```sh
node verify-originals.cjs /path/to/backend/package.json /path/to/protobufjs/package.json
```

The dependency manifests provide `bs58` and `protobufjs`; the verifier also uses the pinned sibling `combined26/original-metadata.cjs` and `solana-type.proto`. No network access, database writes, runtime decoder changes, or package assertions are performed.

The finite finalized sample covers slots 456213447–456214372: 424 Manifest BatchUpdate diagnostics in 146 transactions, including all 851 Manifest invocations in those originals. Every source has a complete committed original protobuf runtime and fully parsed Borsh payload. All 424 source-bound return records match their placement counts: 1,800 placement requests, 1,798 resting order indices, two zero-sized non-resting requests, and 17 cancellations. No source-bound FillLog exists anywhere in these originals. These are unfilled batch operations, not missing swaps.

Finalized RPC/protobuf parity checks cover signatures, resolved account keys, complete compiled instruction data/depth, fee/gas, every token/native pre/post balance, and runtime ancestry. One RPC response truncates logs; its exact retained prefix/tail is checked against the complete original protobuf and explicitly recorded. Containing-block signature order is outside this classification proof.

Pinned primary source `Bonasa-Tech/manifest@5718d3eed80fa7472a826f346c0668644ff32138` shows the actual matching path updates maker/taker deposited balances, records volume, and emits one FillLog per match. Placement/cancellation return records are separate from fill events. The deployed owned executable predates the sample (slot 447525709); its retained instruction bytes construct the exact 232-byte FillLog, and its native syscall relocation resolves to `sol_log_data`. The source commit is a contract reference and does not attest a reproducible ELF build.

Eight program-owned MarketFixed accounts bind every declared market and its distinct mint/vault identities. These later finalized reads establish owned identity only; mutable orderbooks/reserves are not presented as historical snapshots or fill ledgers. The absent fills are established by mandatory fill emission and complete original runtime, not by token CPI counts or current market balances.

`sha256.json` pins originals, owned metadata/ELF, verifier dependencies, and primary source. `independent-proof.json` retains every typed source, return record and explicit retention limitation. This finite result does not establish universal historical or future Manifest coverage.
