# Combined .26 verification

These tools compare the immutable .25 baseline and final .26 SPKG bytes and finalized replay streams, preserve every prior swap identity, and match every added or corrected row against independent original financial proofs. They validate all 25 financial/provenance fields, full original PB/finalized transaction RPC parity, committed instruction ancestry, and finalized containing-block hash/time/signature order. They perform no database writes or network calls.

The finite replay scopes are slots 456163419–456164844 and 456081600–456085680. Actual finalized block counts come from completed replays. These scopes and the independently proved original families do not establish global or future format coverage. A source commit is a captured build reference, not cryptographic proof that a WASM was built from that commit.

Set the shell variables below to available local artifacts. `source_repo` is this substreams repository, `replay_dir` contains the `pluto-combined26-{baseline25,candidate26,wide25,wide26}.jsonl` files and their `-status.json` files, and `blocks_dir` contains retained finalized `getBlock` responses named `<slot>.json.gz`. Dependency paths must point to package manifests providing `bs58` and `protobufjs`; no machine-specific path is embedded in these tools.

```sh
proof_tools="$source_repo/dex-swaps/fixtures/swap-coverage/combined26"
python3 "$proof_tools/compare-replays.py" --replay-root "$replay_dir" \
  --baseline-package "$baseline_spkg" --final-package "$final_spkg" \
  --expected-baseline-package-sha256 "$baseline_sha" \
  --expected-final-package-sha256 "$final_sha"
python3 "$proof_tools/compare-replays.py" --wide --replay-root "$replay_dir" \
  --baseline-package "$baseline_spkg" --final-package "$final_spkg" \
  --expected-baseline-package-sha256 "$baseline_sha" \
  --expected-final-package-sha256 "$final_sha"
python3 "$proof_tools/verify-financial.py" --repo-root "$source_repo" \
  --replay-root "$replay_dir" --block-cache "$blocks_dir" \
  --baseline-package "$baseline_spkg" --final-package "$final_spkg" \
  --expected-baseline-package-sha256 "$baseline_sha" \
  --expected-package-sha256 "$final_sha" \
  --node-dependency-package "$backend_package_json" \
  --protobuf-package "$protobuf_package_json"
```

Optional comparator arguments `--baseline-runtime-source-commit` and `--final-runtime-source-commit` retain build references. `--prepare` generates independent oracle inputs and required block slots; `--inventory` provides an assignment inventory. Neither preparation mode attests a final package. Normal verification requires completed clean finalized replays and explicit actual package paths/hashes; any missing oracle, lost prior identity, unproved prior-row correction, metadata mismatch, or incomplete evidence prevents success.

`extra-proof-families.json` refers only to public sibling fixture directories. Original amounts come from those portable venue oracles, never packaged rows. `solana-type.proto` retains the original protobuf schema; `protocol-ids.json` retains the protocol/slug mapping. Final results pin every proof, original PB/RPC, containing-block response, replay, package, and verifier resource hash. Truncated transaction RPC logs are explicitly recorded and require an exact retained prefix and ordered tail against complete original PB logs. Original PB numeric transaction-version retention limitations are recorded separately from the finalized RPC version.
