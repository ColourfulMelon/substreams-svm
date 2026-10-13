# Finalized live principal regression originals

See [validation and finite scope](../../../../docs/live-principal28-validation.md).

- `cases.tsv`: signature, native source, protocol, pool, user, input mint/atoms, output mint/atoms, physical input transfer index, positive-source flag.
- `preserved.tsv`: signature, native source, protocol, pool, input mint/atoms, output mint/atoms for previously indexed neighboring legs.
- `*.pb`: complete retained Firehose original transactions for all 40 signatures.
- `*.rpc.json`: exact four finalized JSON RPC request/response originals, including the v1 transaction.
- `*.block.rpc.json`: finalized containing-block hashes, timestamps and complete signature ordering.
- `compare-replays.py`: exact paired package comparison, all prior financial/provenance fields unchanged and all 27 new financial/provenance fields matched to independently verified originals.
- `sources.json`: finite original source/ledger audit, independent of the candidate decoder.
- `original-proof.json`: offline original financial parity and vault-ledger proof with explicit optional Firehose metadata omissions.
- `indexed27-swaps.json`: old four live financial legs, never candidate-derived expected amounts.

Run `python3 verify-originals.py` for the offline original oracle. The core test in `quality.rs` replays all 45 sources and rejects invalid financial/lifecycle/runtime evidence. No credential or operational endpoint is included.

`packaged-release-proof.json` pins the actual `.28` package and metadata hashes, successful replay statuses, all 27 independently matched new fields and all 142,127 unchanged prior financial rows. The paired comparator uses retained replay JSONL/status files from finalized slots 456410752–456412315:

```sh
python3 compare-replays.py --replay-root REPLAY_DIRECTORY \
  --baseline-package svm-clickhouse-dex-v0.5.2-pluto.27.spkg \
  --final-package svm-clickhouse-dex-v0.5.2-pluto.28.spkg \
  --expected-baseline-package-sha256 cdd2d7dbcb4ec3213682f98274832d3c44beb57b33839b62805eafb8bf236fc2 \
  --expected-final-package-sha256 31c9041c5b1e05cc7fc67c850257c6bc05ecc3780b5aff85de8f7aa6cc750512
```
