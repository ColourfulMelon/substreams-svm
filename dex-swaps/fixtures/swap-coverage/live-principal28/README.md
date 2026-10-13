# Finalized live principal regression originals

See [validation and finite scope](../../../../docs/live-principal28-validation.md).

- `cases.tsv`: signature, native source, protocol, pool, user, input mint/atoms, output mint/atoms, physical input transfer index, positive-source flag.
- `preserved.tsv`: signature, native source, protocol, pool, input mint/atoms, output mint/atoms for previously indexed neighboring legs.
- `*.pb`: complete retained Firehose original transactions for all 40 signatures.
- `*.rpc.json`: exact four finalized JSON RPC request/response originals, including the v1 transaction.
- `sources.json`: finite original source/ledger audit, independent of the candidate decoder.
- `original-proof.json`: offline original financial parity and vault-ledger proof with explicit optional Firehose metadata omissions.
- `indexed27-swaps.json`: old four live financial legs, never candidate-derived expected amounts.

Run `python3 verify-originals.py` for the offline original oracle. The core test in `quality.rs` replays all 45 sources and rejects invalid financial/lifecycle/runtime evidence. No credential or operational endpoint is included.
