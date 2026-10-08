-- Reporting evidence only. Execution must still verify an exact executable quote.
-- Ratios are anchor atoms per target atom. Quote-token decimals cancel across
-- bridge legs: consumers need only the target decimals and the anchor USD rate.
CREATE TABLE IF NOT EXISTS state_verified_pairs (
    minute DateTime('UTC'),
    amm_pool String,
    mint0 String,
    mint1 String,
    close0 AggregateFunction(argMax, Float64, UInt64),
    median0 AggregateFunction(quantileDeterministic, Float64, UInt64),
    last_amount0 AggregateFunction(argMax, UInt64, UInt64),
    max_timestamp SimpleAggregateFunction(max, DateTime('UTC')),
    observations SimpleAggregateFunction(sum, UInt64)
) ENGINE = AggregatingMergeTree
ORDER BY (minute, mint0, mint1, amm_pool)
TTL minute + INTERVAL 1 DAY;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_state_verified_pairs TO state_verified_pairs AS
WITH
    input_mint <= output_mint AS dir,
    if(dir, input_mint, output_mint) AS mint0,
    if(dir, output_mint, input_mint) AS mint1,
    if(dir, input_amount, output_amount) AS amount0,
    if(dir, output_amount, input_amount) AS amount1,
    ['EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
     'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
     'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB'] AS stable_mints,
    'So11111111111111111111111111111111111111112' AS wsol,
    toUInt64(block_num) * 4294967296 + toUInt64(transaction_index) * 65536 + toUInt64(instruction_index) AS position
SELECT
    toStartOfMinute(timestamp) AS minute, amm_pool, mint0, mint1,
    argMaxState(toFloat64(amount1) / amount0, position) AS close0,
    quantileDeterministicState(toFloat64(amount1) / amount0, position) AS median0,
    argMaxState(toUInt64(amount0), position) AS last_amount0,
    max(timestamp) AS max_timestamp,
    count() AS observations
FROM swaps
WHERE transfer_verified = 1 AND event_id != '' AND amm_pool != ''
  AND amount0 >= multiIf(mint0 IN stable_mints, 10000, mint0 = wsol, 100000, 1)
  AND amount1 >= multiIf(mint1 IN stable_mints, 10000, mint1 = wsol, 100000, 1)
GROUP BY minute, amm_pool, mint0, mint1;

CREATE OR REPLACE VIEW verified_recent_pairs AS
SELECT
    amm_pool, mint0, mint1,
    argMaxMerge(close0) AS raw_price,
    quantileDeterministicMerge(0.5)(median0) AS median_price,
    argMaxMerge(last_amount0) AS amount0,
    max(max_timestamp) AS as_of,
    sum(observations) AS observations
FROM state_verified_pairs
WHERE minute >= now() - INTERVAL 5 MINUTE
GROUP BY amm_pool, mint0, mint1
HAVING as_of >= now() - INTERVAL 60 SECOND AND as_of <= now() + INTERVAL 5 SECOND
   AND isFinite(raw_price) AND raw_price > 0 AND median_price > 0
   AND raw_price / median_price BETWEEN 0.5 AND 2;

-- Every conversion reaches a known anchor; arbitrary quote tokens never have
-- an assumed USD value. A two-hop path must use distinct pools/mints and both
-- legs must independently meet freshness and trade-size requirements.
CREATE OR REPLACE VIEW verified_anchor_paths AS
WITH
    ['So11111111111111111111111111111111111111112',
     'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
     'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
     'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB'] AS anchors,
    edges AS (
        SELECT amm_pool, mint0 AS mint, mint1 AS quote_mint,
            raw_price AS ratio, amount0 AS amount, as_of, observations
        FROM verified_recent_pairs
        UNION ALL
        SELECT amm_pool, mint1 AS mint, mint0 AS quote_mint,
            1 / raw_price AS ratio, amount0 * raw_price AS amount, as_of, observations
        FROM verified_recent_pairs
    ),
    direct AS (
        SELECT *, multiIf(quote_mint = anchors[1], 100000, 10000) AS anchor_dust
        FROM edges
        WHERE quote_mint IN anchors AND amount * ratio >= anchor_dust
    )
SELECT mint, quote_mint AS anchor_mint, amm_pool, ratio AS raw_anchor_per_atom,
    as_of, observations, [mint, quote_mint] AS quote_path
FROM direct
UNION ALL
SELECT e.mint, d.quote_mint AS anchor_mint, e.amm_pool,
    e.ratio * d.ratio AS raw_anchor_per_atom,
    least(e.as_of, d.as_of) AS as_of,
    least(e.observations, d.observations) AS observations,
    [e.mint, e.quote_mint, d.quote_mint] AS quote_path
FROM edges AS e INNER JOIN direct AS d ON e.quote_mint = d.mint
WHERE e.mint NOT IN anchors AND e.quote_mint NOT IN anchors
  AND e.mint != d.quote_mint AND e.amm_pool != d.amm_pool
  AND e.amount * e.ratio * d.ratio >= d.anchor_dust
  AND isFinite(raw_anchor_per_atom) AND raw_anchor_per_atom > 0;

-- SOL's USD conversion is itself derived from fresh, verified stablecoin
-- trades, never an old hourly candle or an assumed value for an unknown mint.
CREATE OR REPLACE VIEW verified_usd_price_candidates AS
WITH
    'So11111111111111111111111111111111111111112' AS wsol,
    ['EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
     'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
     'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB'] AS stables,
    sol AS (
        SELECT quantileExact(0.5)(raw_anchor_per_atom / 1000000) AS usd_per_sol_atom,
            min(as_of) AS as_of
        FROM verified_anchor_paths
        WHERE mint = wsol AND anchor_mint IN stables AND length(quote_path) = 2
    )
SELECT p.mint, p.amm_pool, p.quote_path, p.observations,
    p.raw_anchor_per_atom * if(p.anchor_mint = wsol, sol.usd_per_sol_atom, 0.000001) AS usd_per_atom,
    if(p.anchor_mint = wsol, least(p.as_of, sol.as_of), p.as_of) AS as_of
FROM verified_anchor_paths AS p CROSS JOIN sol
WHERE isFinite(usd_per_atom) AND usd_per_atom > 0;
