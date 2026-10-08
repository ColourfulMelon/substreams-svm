#!/usr/bin/env python3
"""Audit a bounded db_out JSONL replay without connecting to any live database."""
import argparse
import collections
import json

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('replay')
parser.add_argument('--decoder-version', default='v0.5.2-pluto.5')
args = parser.parse_args()
counts = collections.Counter()
events = set()
blocks = {}
for line in open(args.replay):
    try:
        output = json.loads(line)
    except ValueError:
        continue  # CLI progress/stream completion lines are not module output.
    for row in output.get('@data', {}).get('tableChanges', []):
        fields = {field['name']: field.get('value', '') for field in row['fields']}
        table = {'canonical_blocks': 'blocks', 'canonical_swaps': 'swaps'}.get(row['table'], row['table'])
        assert table in ('blocks', 'swaps'), f'unexpected streamed table {table}'
        counts[(table, fields.get('protocol', ''))] += 1
        if table == 'blocks':
            slot = int(fields['block_num'])
            assert slot not in blocks, f'duplicate block {slot}'
            blocks[slot] = fields
        elif table == 'swaps':
            assert fields['transfer_verified'] == '1', 'unverified financial event'
            assert fields['decoder_version'] == args.decoder_version, 'mixed decoder versions'
            assert fields['event_id'], 'missing immutable event identity'
            identity = (fields['block_hash'], fields['event_id'])
            assert identity not in events, f'duplicate financial event {identity}'
            events.add(identity)
            assert fields['input_mint'] != fields['output_mint'] and fields['amm_pool'], 'invalid pool or mints'
            assert 0 < int(fields['input_amount']) < 2**64 and 0 < int(fields['output_amount']) < 2**64, 'invalid amounts'
assert blocks, 'empty replay'
previous = None
for slot, block in sorted(blocks.items()):
    if previous:
        assert int(block['parent_slot']) == previous[0] and block['parent_hash'] == previous[1]['block_hash'], f'broken parent link at {slot}'
    previous = (slot, block)
assert sum(int(block['verified_swaps']) for block in blocks.values()) == len(events), 'quality count mismatch'
print(json.dumps({
    'first_slot': min(blocks), 'last_slot': max(blocks), 'blocks': len(blocks),
    'unique_verified_events': len(events),
    'counts': {':'.join(key): value for key, value in sorted(counts.items())},
    'quality_counters': {name: sum(int(block[name]) for block in blocks.values())
                         for name in ('quarantined_swaps', 'duplicate_wrappers', 'corrected_events')},
    'parent_links_and_event_identity': 'verified',
}, indent=2))
