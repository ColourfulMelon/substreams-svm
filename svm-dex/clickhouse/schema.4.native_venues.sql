-- Legacy OHLC keys identify a market by pool/mints; protocol is dependent on
-- that identity. Preserve this existing key while modern ClickHouse validates it.
ALTER TABLE state_ohlc_prices MODIFY SETTING allow_dimensions_outside_sorting_key = 1;
-- Run once with the writer stopped, or through the gated --init-native-venues
-- startup helper. Only enum-dependent projection definitions are refreshed.
-- Source rows, enum IDs and storage width remain unchanged. Old parts may use
-- raw scans for these projections; do not MATERIALIZE retained history here.
ALTER TABLE swaps DROP PROJECTION IF EXISTS prj_protocol_count;
ALTER TABLE swaps DROP PROJECTION IF EXISTS prj_protocol_by_minute;
ALTER TABLE state_pools_aggregating_by_pool DROP PROJECTION IF EXISTS prj_group_by_pool;
ALTER TABLE state_pools_aggregating_by_mint DROP PROJECTION IF EXISTS prj_group_by_pool;
-- ClickHouse 26.6+ metadata-only enum extension. Existing IDs and storage width
-- are preserved. Apply before starting a decoder with the new native venues.
-- Repeated startup application is safe; exact existing name/ID pairs merge.
ALTER TABLE swaps MODIFY COLUMN protocol ADD ENUM VALUES ('goonfi_v2' = 21, 'bisonfi' = 22, 'manifest' = 23, 'humidifi' = 24, 'orca_v2' = 25, 'alphaq' = 26, 'kipseli' = 27, 'flux' = 28, 'scorch' = 29, 'obsidian' = 30, 'tesserav' = 31, 'deriverse' = 32, 'zerofi' = 33);
ALTER TABLE state_ohlc_prices MODIFY COLUMN protocol ADD ENUM VALUES ('goonfi_v2' = 21, 'bisonfi' = 22, 'manifest' = 23, 'humidifi' = 24, 'orca_v2' = 25, 'alphaq' = 26, 'kipseli' = 27, 'flux' = 28, 'scorch' = 29, 'obsidian' = 30, 'tesserav' = 31, 'deriverse' = 32, 'zerofi' = 33);
ALTER TABLE state_pools_aggregating_by_pool MODIFY COLUMN protocol ADD ENUM VALUES ('goonfi_v2' = 21, 'bisonfi' = 22, 'manifest' = 23, 'humidifi' = 24, 'orca_v2' = 25, 'alphaq' = 26, 'kipseli' = 27, 'flux' = 28, 'scorch' = 29, 'obsidian' = 30, 'tesserav' = 31, 'deriverse' = 32, 'zerofi' = 33);
ALTER TABLE state_pools_aggregating_by_mint MODIFY COLUMN protocol ADD ENUM VALUES ('goonfi_v2' = 21, 'bisonfi' = 22, 'manifest' = 23, 'humidifi' = 24, 'orca_v2' = 25, 'alphaq' = 26, 'kipseli' = 27, 'flux' = 28, 'scorch' = 29, 'obsidian' = 30, 'tesserav' = 31, 'deriverse' = 32, 'zerofi' = 33);
ALTER TABLE swaps ADD PROJECTION IF NOT EXISTS prj_protocol_count (SELECT protocol, count(), min(block_num), max(block_num), min(timestamp), max(timestamp), min(minute), max(minute) GROUP BY protocol);
ALTER TABLE swaps ADD PROJECTION IF NOT EXISTS prj_protocol_by_minute (SELECT protocol, minute GROUP BY protocol, minute);
ALTER TABLE state_pools_aggregating_by_pool ADD PROJECTION IF NOT EXISTS prj_group_by_pool (SELECT min(min_timestamp), max(max_timestamp), min(min_block_num), max(max_block_num), protocol, program_id, amm, amm_pool, sum(transactions) GROUP BY amm_pool, protocol, program_id, amm);
ALTER TABLE state_pools_aggregating_by_mint ADD PROJECTION IF NOT EXISTS prj_group_by_pool (SELECT min(min_timestamp), max(max_timestamp), min(min_block_num), max(max_block_num), protocol, program_id, amm, amm_pool, arraySort(groupArrayDistinct(mint)), sum(transactions) GROUP BY protocol, program_id, amm, amm_pool);

-- Recreate only insert-view metadata to infer the widened enum from swaps.
-- Target aggregate tables and all retained rows are preserved.
CREATE OR REPLACE MATERIALIZED VIEW mv_state_ohlc_prices
TO state_ohlc_prices
AS
WITH
    [1, 5, 10, 30, 60, 240, 1440, 10080] AS intervals,
    (input_mint <= output_mint) AS dir,
    if(dir, input_mint, output_mint) AS mint0,
    if(dir, output_mint, input_mint) AS mint1,
    if(dir, input_amount, output_amount) AS amount0,
    if(dir, output_amount, input_amount) AS amount1,
    [
        'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
        'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
        'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB'
    ] AS stable_mints,
    [
        'So11111111111111111111111111111111111111112',
        '7dHbWXmci3dT8UFYWYZweBLXgycu7Y3iL6trKn1Y7ARj'
    ] AS sol_mints,
    (
        transfer_verified = 1 AND
        amount0 >= multiIf(mint0 IN stable_mints, 10000, mint0 IN sol_mints, 100000, 1)
        AND amount1 >= multiIf(mint1 IN stable_mints, 10000, mint1 IN sol_mints, 100000, 1)
    ) AS price_eligible,
    toUInt64(block_num) * 4294967296 + toUInt64(transaction_index) * 65536 + toUInt64(instruction_index) AS trade_position,
    if(amount0 > 0, toFloat64(amount1) / amount0, 0.0) AS price,
    abs(amount0) AS gv0,
    abs(amount1) AS gv1,
    if(dir, toInt128(input_amount), -toInt128(output_amount)) AS nf0,
    if(dir, -toInt128(output_amount), toInt128(input_amount)) AS nf1
SELECT
    arrayJoin(intervals) AS interval_min,
    toDateTime(intDiv(toUInt32(s.timestamp), interval_min * 60) * interval_min * 60) AS timestamp,
    min(s.timestamp) AS min_timestamp,
    max(s.timestamp) AS max_timestamp,
    min(s.block_num) AS min_block_num,
    max(s.block_num) AS max_block_num,
    protocol,
    program_id,
    amm,
    amm_pool,
    mint0,
    mint1,
    argMinStateIf(price, trade_position, price_eligible) AS open0,
    min(if(price_eligible, toNullable(price), NULL)) AS min_price0,
    quantileDeterministicStateIf(price, trade_position, price_eligible) AS quantile0,
    max(if(price_eligible, toNullable(price), NULL)) AS max_price0,
    argMaxStateIf(price, trade_position, price_eligible) AS close0,
    sum(gv0) AS gross_volume0,
    sum(gv1) AS gross_volume1,
    sum(nf0) AS net_flow0,
    sum(nf1) AS net_flow1,
    count() AS transactions,
    uniqState(signer) AS uniq_signer,
    uniqState(fee_payer) AS uniq_fee_payer,
    uniqState(user) AS uniq_user
FROM swaps AS s
GROUP BY
    interval_min,
    amm_pool,
    protocol,
    program_id,
    amm,
    mint0,
    mint1,
    timestamp;

CREATE OR REPLACE MATERIALIZED VIEW mv_state_pools_aggregating_by_pool_swaps
TO state_pools_aggregating_by_pool
AS
SELECT
    -- timestamp & block number --
    min(timestamp) AS min_timestamp,
    max(timestamp) AS max_timestamp,
    min(block_num) AS min_block_num,
    max(block_num) AS max_block_num,

    -- DEX identity
    protocol, program_id, amm, amm_pool,

    -- universal --
    count() as transactions
FROM swaps
GROUP BY protocol, program_id, amm, amm_pool;

CREATE OR REPLACE MATERIALIZED VIEW mv_state_pools_aggregating_by_mint_input_mint
TO state_pools_aggregating_by_mint
AS
SELECT
    -- timestamp & block number --
    min(timestamp) AS min_timestamp,
    max(timestamp) AS max_timestamp,
    min(block_num) AS min_block_num,
    max(block_num) AS max_block_num,

    -- DEX identity
    protocol, program_id, amm, amm_pool,
    input_mint AS mint,

    -- universal --
    count() as transactions
FROM swaps
WHERE protocol NOT IN ('jupiter_v4', 'jupiter_v6')
GROUP BY mint, protocol, program_id, amm, amm_pool;

CREATE OR REPLACE MATERIALIZED VIEW mv_state_pools_aggregating_by_mint_output_mint
TO state_pools_aggregating_by_mint
AS
SELECT
    -- timestamp & block number --
    min(timestamp) AS min_timestamp,
    max(timestamp) AS max_timestamp,
    min(block_num) AS min_block_num,
    max(block_num) AS max_block_num,

    -- DEX identity
    protocol, program_id, amm, amm_pool,
    output_mint AS mint,

    -- universal --
    count() as transactions
FROM swaps
WHERE protocol NOT IN ('jupiter_v4', 'jupiter_v6')
GROUP BY mint, protocol, program_id, amm, amm_pool;
