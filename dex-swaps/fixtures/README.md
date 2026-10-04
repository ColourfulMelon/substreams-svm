# Swap regression fixtures

These protobuf-encoded `sf.solana.type.v1.ConfirmedTransaction` fixtures are derived from unmodified public Solana `getTransaction` results (JSON encoding). They retain signatures, resolved account metadata, instructions, inner instructions, logs, and token balances needed by the decoders. No diagnostic instruction edits or synthetic token balances are included.

- `privacy.transaction.pb`: slot 453196517, 2026-10-04 08:00:54 UTC. [Transaction](https://solscan.io/tx/36YvcbZZk5T78iCXMPrEsnTsKa1DY3v3qpLaf55VCirC9CFBMfnmrd7BbBKpXEFVUq6eZgrRpoV1s4kuXgVtL5Y4). Meteora DAMM v2 `swap2` includes the swap-mode byte. A separate Raydium leg must remain indexed.
- `knet.transaction.pb`: slot 452949660, 2026-10-03 13:40:22 UTC. [Transaction](https://solscan.io/tx/FdvAFiiouw5APC1CXXLpQjqcDPnYXKVVBCHAnbyBiFLXbosgpgQDjeC2PL3eJayPhpV89qpEiju2UYHJ3YHUfcj). The WSOL destination is initialized and closed within the transaction and absent from both balance snapshots.
- `oilinu.transaction.pb`: slot 452919823, 2026-10-03 11:27:34 UTC. [Transaction](https://solscan.io/tx/4jDUraxyc3j49eH6wQZDVSNG7KPDbDypAyuqmwFNbATA5LYkda1yBbgqmnvexFRC48gTay8oWbWPnc2BSYbPpAVs). PumpSwap `buy` includes the volume-tracking flag.
