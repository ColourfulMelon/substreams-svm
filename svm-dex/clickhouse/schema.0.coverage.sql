-- Diagnostic candidates include non-swaps. They never feed price views.
CREATE TABLE IF NOT EXISTS swap_coverage (
    block_num UInt32,
    block_hash String,
    timestamp DateTime('UTC'),
    transaction_index UInt32,
    instruction_index UInt32,
    signature String,
    program_id LowCardinality(String),
    stack_height UInt32,
    instruction_tag String,
    mints_raw String,
    transfer_count UInt32,
    reason LowCardinality(String),
    decoder_version LowCardinality(String)
) ENGINE = ReplacingMergeTree
PARTITION BY toDate(timestamp)
ORDER BY (timestamp, block_hash, transaction_index, instruction_index)
TTL timestamp + INTERVAL 7 DAY DELETE;

-- Keep one original Firehose transaction per candidate transaction, rather
-- than a separate large payload for every instruction. This enables replay
-- even when an RPC provider stops serving the signature. Block counters remain
-- available after this bounded diagnostic retention expires.
CREATE TABLE IF NOT EXISTS swap_coverage_transactions (
    block_num UInt32,
    block_hash String,
    timestamp DateTime('UTC'),
    transaction_index UInt32,
    instruction_index UInt32,
    signature String,
    decoder_version LowCardinality(String),
    transaction_proto_hex String CODEC(ZSTD(3))
) ENGINE = ReplacingMergeTree
PARTITION BY toDate(timestamp)
ORDER BY (timestamp, block_hash, transaction_index, instruction_index)
TTL timestamp + INTERVAL 7 DAY DELETE;
