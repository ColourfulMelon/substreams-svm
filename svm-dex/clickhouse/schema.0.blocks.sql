CREATE TABLE IF NOT EXISTS blocks (
    block_num                   UInt32,
    block_hash                  String,
    timestamp                   DateTime(0, 'UTC'),
    minute                      UInt32 MATERIALIZED toRelativeMinuteNum(timestamp),

    -- PROJECTIONS --
    PROJECTION prj_block_hash ( SELECT * ORDER BY block_hash ),
    PROJECTION prj_timestamp ( SELECT * ORDER BY timestamp )
)
ENGINE = MergeTree
ORDER BY ( block_num )
COMMENT 'Blocks';
ALTER TABLE blocks
    ADD COLUMN IF NOT EXISTS parent_slot UInt64 DEFAULT 0,
    ADD COLUMN IF NOT EXISTS parent_hash String DEFAULT '',
    ADD COLUMN IF NOT EXISTS decoder_version LowCardinality(String) DEFAULT '',
    ADD COLUMN IF NOT EXISTS verified_swaps UInt32 DEFAULT 0,
    ADD COLUMN IF NOT EXISTS quarantined_swaps UInt32 DEFAULT 0,
    ADD COLUMN IF NOT EXISTS duplicate_wrappers UInt32 DEFAULT 0,
    ADD COLUMN IF NOT EXISTS corrected_events UInt32 DEFAULT 0;
