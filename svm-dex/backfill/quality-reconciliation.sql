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
  AND b.decoder_version = 'v0.5.2-pluto.3'
  AND b.parent_slot >= (SELECT min(block_num) FROM blocks WHERE decoder_version = 'v0.5.2-pluto.3')
  AND b.parent_hash != p.block_hash;

SELECT protocol, verification_failure, count() AS events
FROM swap_diagnostics WHERE timestamp >= now() - INTERVAL 5 MINUTE
GROUP BY protocol, verification_failure ORDER BY events DESC;
