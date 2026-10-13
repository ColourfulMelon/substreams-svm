# Combined .27 packaged replay proof

These read-only tools compare exact .26/.27 SPKG bytes and finalized replay output, then match every new financial row against independent venue oracles and original finalized transactions. They do not query application pricing or write a database. Source commit references are captured build provenance; they do not cryptographically establish a reproducible WASM build.

Three finite windows are supported:

| Window | Baseline / candidate labels | Slots, inclusive |
|---|---|---|
| baseline | baseline26 / candidate27 | 456163419–456164844 |
| live | live26 / live27 | 456213447–456214372 |
| wide | wide26 / wide27 | 456081600–456085680 |

Replay artifacts must be named `pluto-combined27-LABEL.jsonl` and `pluto-combined27-LABEL-status.json`. Each status must attest clean completion, finalized-only execution, no production/database writes, exact scope and byte count, and identical actual SPKG SHA256 at launch and completion. All four table versions, create operations, unique keys/fields/ordinals, block parents and sealed counters are checked. Every old financial/provenance field must remain identical except decoder version. Introduced burn columns must equal zero for old rows. Lost or unexpectedly modified old rows fail closed and are retained explicitly. The comparator permits only the two bounded Pancake provenance profiles; all 1,221 accepted corrections are independently matched to committed native ancestry and unchanged financial principals.

Run each comparator window, substituting the actual immutable packages, hashes and captured source references:

```sh
python3 compare-replays.py --replay-root /artifacts --window live \
  --baseline-package /packages/release26.spkg --final-package /packages/release27.spkg \
  --expected-baseline-package-sha256 BASELINE_SHA256 \
  --expected-final-package-sha256 FINAL_SHA256 \
  --baseline-runtime-source-commit BASELINE_COMMIT --final-runtime-source-commit FINAL_COMMIT
```

Repeat with `--window baseline` and `--window wide`. Comparator reports contain full output/status/package byte hashes and every added row; diagnostic changes are separate from missing financial swaps.

Prepare independent oracle input identities and required finalized block slots:

```sh
python3 verify-financial.py --repo-root /source --replay-root /artifacts --prepare
```

`--inventory` compares available partial prefixes only to identify additions lacking proofs. Missing replay files and partial output are explicitly preparation states, never passing release results. The fixture descriptors and `extra-proof-families.json` support additional independently verified original families. They must contain exact native financial principals and raw PB/finalized RPC provenance; candidate output cannot supply oracle amounts.

For final verification, provide all comparator reports (default paths cover all three windows), the actual package bytes/hashes, dependency packages, and finalized containing-block cache:

```sh
python3 verify-financial.py --repo-root /source --replay-root /artifacts \
  --baseline-package /packages/release26.spkg --final-package /packages/release27.spkg \
  --expected-baseline-package-sha256 BASELINE_SHA256 --expected-package-sha256 FINAL_SHA256 \
  --node-dependency-package /backend/package.json --protobuf-package /protobufjs/package.json \
  --block-cache /cache/finalized-blocks --output /artifacts/combined27-financial-proof.json
```

The dependency package must resolve `bs58`; the protobuf package must resolve `protobufjs`. Cached blocks are gzip JSON containing exact finalized `getBlock` request/response records with signature order, time and blockhash. `original-metadata.cjs` checks original PB/finalized RPC account keys, complete instruction bytes/depths, metadata, token/lamport balances, fee/gas and retained runtime, then binds each transaction to the containing block. Explicit truncated-RPC flags must match retained logs; complete original PB commitment/ancestry remains mandatory.

All 25 prior financial/provenance fields and the two introduced burn fields are checked for added rows, including physical quote/burn/log identity, native/event depth, execution position, transaction index, signers, fee/gas, timestamp and blockhash. Burn rows additionally bind a committed source-owned physical Burn and exact burned mint/amount. Unmatched originals, field differences or old-row modifications fail verification and remain in the output. Complete packages, replay outputs, comparator bytes and original proof hashes are checked again before writing final attestation. No global future-format coverage claim is made.

Final result: 6,429 finalized blocks, 876,427 unchanged prior rows, 750 added legs, 1,221 bounded Pancake provenance corrections, zero losses and zero unexpected modifications. All 1,971 added/corrected rows match 27 fields against 1,969 finalized originals and 1,361 containing blocks. See `packaged-release-proof.json`; compressed comparator, financial and original metadata evidence is published with the release. All seven metadata hashes and frozen metadata bytes are unchanged.
