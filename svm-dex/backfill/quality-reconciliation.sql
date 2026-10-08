-- Read-only audits after rollout. Restrict to a bounded finalized block range.
-- A nonempty result requires a verified canonical replay; never delete/rewind
-- rows or append corrected OHLC aggregates directly to repair a discrepancy.
SELECT block_hash, event_id, count() AS copies
FROM swaps
WHERE timestamp >= now() - INTERVAL 5 MINUTE AND event_id != ''
GROUP BY block_hash, event_id HAVING copies > 1;

-- Parent links distinguish real missing blocks from skipped Solana slots.
SELECT b.block_num, b.parent_slot, b.parent_hash, p.block_hash AS indexed_parent_hash
FROM blocks AS b LEFT JOIN blocks AS p ON b.parent_slot = p.block_num
WHERE b.timestamp >= now() - INTERVAL 5 MINUTE
  AND b.decoder_version IN ('v0.5.2-pluto.3', 'v0.5.2-pluto.4', 'v0.5.2-pluto.5')
  AND b.parent_slot >= (SELECT min(block_num) FROM blocks WHERE decoder_version IN ('v0.5.2-pluto.3', 'v0.5.2-pluto.4', 'v0.5.2-pluto.5'))
  AND b.parent_hash != p.block_hash;

SELECT sum(quarantined_swaps) AS rejected, sum(duplicate_wrappers) AS duplicates,
       sum(corrected_events) AS corrected
FROM blocks WHERE timestamp >= now() - INTERVAL 5 MINUTE;
