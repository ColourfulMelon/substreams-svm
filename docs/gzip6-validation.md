# Gzip level 6 release validation

Release v0.5.2-pluto.33 changes the SQL mapper from fast gzip (level 1) to
`flate2::Compression::new(6)`. Its version-1 envelope and sink contract stay the
same. The default decoder and frozen metadata WASM are copied byte for byte from
release .32; all six metadata hashes and `map_events` remain unchanged, with
four execution stages. No sink or database schema changes are required.

The exact packaged mapper was run over 200 finalized blocks (456489100–456489299),
containing 24,436 swaps. Decompressed protobuf bytes match .32 after normalizing
only `decoder_version`; every financial field, provenance field, row order and
sealed block is preserved. Raw output remains 30,737,909 bytes. Actual protobuf
gzip envelopes shrink from 7,576,693 to 5,852,788 bytes, a **22.75%** additional
reduction. This sample does not establish whole-bill or daily savings.

Fresh WASM instances per block, one warmup and three measured passes give median
mean full mapper time of 2.79 ms at level 1 and 3.87 ms at level 6, about 1.09 ms
extra per block. This is local Node v24.16.0 V8 timing, not Pinax CPU timing or
production throughput. See [benchmark evidence](gzip6-benchmark.json).

All 15 mapper tests pass. The unchanged default decoder binary preserves the
previous release's decoder implementation and cache identity. Paired fresh finalized Pinax replays over slots 456577700–456577799 pass the
actual patched sink decoder, preserving all 11,976 financial rows and 100 sealed
blocks, with no changed or lost financial rows. Parent links, financial counters
and insertion order match; only decoder_version changes. The replay writes no
production financial data.
See [packaged release proof](stage33-package-proof.json).

The mapper module hash changes, so deployment must stop the previous writer,
verify its stable durable cursor and absence of rows beyond it, then adopt that
exact opaque cursor under the new hash. Keep the old cursor row; resume the
single writer at the next block and verify financial counters and parent links.
