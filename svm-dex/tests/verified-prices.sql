-- Run after the package schema in an isolated ClickHouse database.
-- No dependency on current chain/network prices.
INSERT INTO swaps (block_num, block_hash, timestamp, transaction_index, instruction_index,
    event_id, transfer_verified, amm_pool, input_mint, output_mint, input_amount, output_amount)
VALUES
    (454558265, 'block', now(), 2, 3, 'close', 1, 'ordered', 'TARGET', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 1800000),
    (454558265, 'block', now(), 1, 9, 'open', 1, 'ordered', 'TARGET', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 2100000),
    (454558265, 'block', now(), 2, 2, 'middle', 1, 'ordered', 'TARGET', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 1900000),
    (454558265, 'block', now(), 9, 9, 'dust', 1, 'ordered', 'TARGET', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 2, 2),
    (454558265, 'block', now(), 9, 10, 'unverified', 0, 'ordered', 'TARGET', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 999999000000),
    (454558265, 'block', now(), 3, 1, 'bridge', 1, 'bridge', 'BRIDGE', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000, 1000000),
    (454558265, 'block', now(), 3, 2, 'leaf', 1, 'leaf', 'LEAF', 'BRIDGE', 500, 100),
    (454558265, 'block', now() - INTERVAL 90 SECOND, 3, 3, 'stale', 1, 'stale', 'STALE', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 1000000),
    (454558265, 'block', now(), 3, 4, 'unknown', 1, 'unknown', 'UNKNOWN', 'UNPRICED', 1000000, 1000000),
    (454558265, 'block', now(), 4, 1, 'median1', 1, 'spike', 'SPIKE', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 1000000),
    (454558265, 'block', now(), 4, 2, 'median2', 1, 'spike', 'SPIKE', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 1000000),
    (454558265, 'block', now(), 4, 3, 'spike', 1, 'spike', 'SPIKE', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB', 1000000, 325000000000);

INSERT INTO swap_diagnostics (block_num, timestamp, amm_pool, input_mint, output_mint,
    input_amount, output_amount, verification_failure)
VALUES (454558265, now(), 'ordered', 'TARGET', 'USD1ttGY1N17NEEHLmELoaybftRBUSErhqYiQzvEmuB',
    1000000, 999999000000, 'duplicate_native_cpi');

SELECT 'deterministic open/close', throwIf(open != 2.1 OR close != 1.8)
FROM (SELECT argMinMerge(open0) AS open, argMaxMerge(close0) AS close
      FROM state_ohlc_prices WHERE amm_pool = 'ordered' AND interval_min = 1);
SELECT 'dust/unverified excluded from price evidence', throwIf(raw_price != 1.8 OR observations != 3)
FROM verified_recent_pairs WHERE amm_pool = 'ordered';
SELECT 'diagnostics never contribute to candles', throwIf(trades != 5)
FROM (SELECT sum(transactions) AS trades FROM state_ohlc_prices WHERE amm_pool = 'ordered' AND interval_min = 1);
SELECT 'exact bridge conversion without quote decimals', throwIf(count() != 1 OR any(raw_anchor_per_atom) != 200)
FROM verified_anchor_paths WHERE mint = 'LEAF' AND amm_pool = 'leaf';
SELECT 'absolute freshness and outlier rejection', throwIf(count() != 0)
FROM verified_anchor_paths WHERE mint IN ('STALE', 'UNKNOWN', 'SPIKE');
SELECT 'USD bridge does not assume quote value', throwIf(count() != 1 OR abs(any(usd_per_atom) - 0.0002) > 0.00000000001)
FROM verified_usd_price_candidates WHERE mint = 'LEAF';
