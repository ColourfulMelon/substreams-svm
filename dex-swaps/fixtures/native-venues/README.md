# Finalized native venue fixtures

These public mainnet transactions were fetched through Helius at finalized
commitment. The files encode Substreams `ConfirmedTransaction` messages.

`cases.tsv` columns: protobuf filename, transaction signature, slot, and a
pipe-separated list of native legs. Each leg contains colon-separated protocol
ID, source instruction index, pool, user authority, input mint, input atoms,
output mint and output atoms. An empty list intentionally tests a non-swap call.

`helius-evidence.json` records the fixture checksums and original Helius evidence.
No provider credentials are included. See
[validation and reproduction notes](../../../docs/native-venue-validation.md).
