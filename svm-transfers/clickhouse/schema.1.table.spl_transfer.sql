-- SPL Token Transfers --
CREATE TABLE IF NOT EXISTS spl_transfer AS BASE_EVENTS
COMMENT 'SPL Token Transfer/Burn/Mint events';
ALTER TABLE spl_transfer
    -- ordering --
    -- 0 for an outer instruction, else 1 + the position among the inner
    -- instructions of `instruction_index` (original positions).
    ADD COLUMN IF NOT EXISTS inner_instruction_index UInt32,

    -- authority --
    ADD COLUMN IF NOT EXISTS authority               String,
    ADD COLUMN IF NOT EXISTS multisig_authority_raw  String,

    -- events --
    ADD COLUMN IF NOT EXISTS source                  String,
    ADD COLUMN IF NOT EXISTS destination             String,
    ADD COLUMN IF NOT EXISTS amount                  UInt64,
    ADD COLUMN IF NOT EXISTS mint                    LowCardinality(String),

    -- Optional
    ADD COLUMN IF NOT EXISTS decimals_raw            String;
