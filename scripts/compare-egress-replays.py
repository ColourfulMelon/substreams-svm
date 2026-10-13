"""Verify finalized SQL outputs and measure uncompressed DatabaseChanges protobuf bytes."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
for name in ['baseline', 'candidate', 'baseline-status', 'candidate-status', 'baseline-package', 'candidate-package', 'output']:
    p.add_argument('--' + name, type=Path, required=True)
p.add_argument('--start', type=int, required=True)
p.add_argument('--stop', type=int, required=True)
a = p.parse_args()

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''): h.update(chunk)
    return h.hexdigest()

def varint_size(n):
    return max(1, (n.bit_length() + 6) // 7)

def nested_size(n):
    return 1 + varint_size(n) + n

def string_size(s):
    return nested_size(len(s.encode())) if s else 0

def wire_size(row):
    # TableChange: table=1, ordinal=3, operation=4, fields=5, composite_pk=6.
    ordinal = int(row.get('ordinal', 0))
    n = string_size(row['table']) + (1 + varint_size(ordinal) if ordinal else 0) + 2
    key = sum(nested_size(string_size(k) + string_size(v)) for k, v in row['compositePk']['keys'].items())
    n += nested_size(key)
    for f in row['fields']:
        assert f.get('updateOp', 'UPDATE_OP_SET') == 'UPDATE_OP_SET'
        n += nested_size(string_size(f['name']) + string_size(f.get('value', '')) + 2)  # update_op=4: SET=1
    return nested_size(n)  # DatabaseChanges.table_changes=1.

def normalized(row):
    fields = {f['name']: f.get('value', '') for f in row['fields']}
    assert len(fields) == len(row['fields']), 'duplicate field names'
    keys = row['compositePk']['keys']
    assert all(fields[k] == v for k, v in keys.items() if k in fields), 'key/field conflict'
    return {**keys, **{k: v for k, v in fields.items() if k != 'decoder_version'}}

statuses = {}
blocks = {}
for label in ['baseline', 'candidate']:
    path, status_path, package = (getattr(a, label), getattr(a, label + '_status'), getattr(a, label + '_package'))
    status = json.loads(status_path.read_text())
    assert status['exitCode'] == 0 and status['finalizedOnly'] and not status['databaseWrites']
    assert not status['productionMode']
    assert status['packageSha256AtLaunch'] == status['packageSha256AfterReplay'] == sha(package)
    assert status['firstSlot'] <= a.start < a.stop <= status['lastSlot'] + 1
    assert path.stat().st_size == status['outputBytes']
    statuses[label] = status
    blocks[label] = {}
    with path.open() as stream:
        for line in stream:
            if not line.startswith('{'): continue
            b = json.loads(line)
            if a.start <= b['@block'] < a.stop:
                assert b['@block'] not in blocks[label], 'repeated block'
                blocks[label][b['@block']] = b
assert blocks['baseline'] and blocks['baseline'].keys() == blocks['candidate'].keys()
totals = {label: collections.Counter() for label in blocks}
bytes_by_table = {label: collections.Counter() for label in blocks}
previous = {label: None for label in blocks}
for slot in sorted(blocks['baseline']):
    rows = {}
    for label in blocks:
        grouped = collections.defaultdict(dict)
        changes = blocks[label][slot]['@data']['tableChanges']
        assert sorted(int(r.get('ordinal', 0)) for r in changes) == list(range(len(changes)))
        for r in changes:
            assert r['operation'] == 'OPERATION_CREATE'
            assert next(f['value'] for f in r['fields'] if f['name'] == 'decoder_version') == statuses[label]['release']
            key = json.dumps(r['compositePk'], sort_keys=True)
            assert key not in grouped[r['table']]
            grouped[r['table']][key] = normalized(r)
            totals[label][r['table']] += 1
            bytes_by_table[label][r['table']] += wire_size(r)
        assert len(grouped['blocks']) == 1
        sealed = next(iter(grouped['blocks'].values()))
        assert int(sealed['block_num']) == slot
        if previous[label]:
            assert (sealed['parent_slot'], sealed['parent_hash']) == (previous[label]['block_num'], previous[label]['block_hash'])
        previous[label] = sealed
        for counter, table in [('verified_swaps', 'swaps'), ('coverage_candidates', 'swap_coverage'), ('coverage_transactions', 'swap_coverage_transactions')]:
            assert int(sealed[counter]) == len(grouped[table])
        rows[label] = grouped
    old, new = rows['baseline'], rows['candidate']
    assert list(old['swaps']) == list(new['swaps']), 'financial keys or relative insertion order changed'
    assert old['swaps'] == new['swaps'], 'financial/provenance columns changed'
    assert not new['swap_coverage'] and not new['swap_coverage_transactions']
    for key, sealed in old['blocks'].items():
        expected = {**sealed, 'coverage_candidates': '0', 'coverage_transactions': '0'}
        assert expected == new['blocks'][key], 'sealed block changed beyond disabled diagnostics'
    assert set(new) <= {'blocks', 'swaps', 'swap_coverage', 'swap_coverage_transactions'}
old_bytes, new_bytes = (sum(bytes_by_table[label].values()) for label in ['baseline', 'candidate'])
proof = dict(firstSlot=a.start, lastSlot=a.stop - 1, finalizedBlocks=len(blocks['candidate']),
    rows=totals, databaseChangesProtobufBytes=bytes_by_table, baselineBytes=old_bytes,
    candidateBytes=new_bytes, reductionPercent=100 * (1 - new_bytes / old_bytes),
    packageSha256={label: statuses[label]['packageSha256AtLaunch'] for label in statuses},
    replaySha256={label: sha(getattr(a, label)) for label in statuses},
    financialRowsPreserved=totals['candidate']['swaps'], financialRowsChanged=0,
    financialRowsLost=0, relativeFinancialInsertionOrderPreserved=True,
    blockParentsAndFinancialCountersPreserved=True, databaseWrites=False)
a.output.write_text(json.dumps(proof, indent=2) + '\n')
print(json.dumps(proof, indent=2))
