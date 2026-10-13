import argparse
import collections
import hashlib
import itertools
import json
import sys
from pathlib import Path

parser=argparse.ArgumentParser(description='Compare complete finalized baseline/final replays against pinned actual SPKG bytes; no database writes.')
parser.add_argument('--replay-root',type=Path,required=True);parser.add_argument('--baseline-package',type=Path,required=True);parser.add_argument('--final-package',type=Path,required=True);parser.add_argument('--expected-baseline-package-sha256',required=True);parser.add_argument('--expected-final-package-sha256',required=True);parser.add_argument('--baseline-runtime-source-commit');parser.add_argument('--final-runtime-source-commit');args=parser.parse_args()
BASE=args.replay_root.resolve()
package_paths={'27':args.baseline_package.resolve(),'28':args.final_package.resolve()};expected_package_hashes={'27':args.expected_baseline_package_sha256,'28':args.expected_final_package_sha256}
def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as stream:
        for chunk in iter(lambda:stream.read(1024*1024),b''):h.update(chunk)
    return h.hexdigest()
assert all(len(expected_package_hashes[v])==64 and sha(p)==expected_package_hashes[v] for v,p in package_paths.items()),'actual SPKG byte hash mismatch'
statuses={};status_hashes={}

def blocks(label):
    prefix = BASE / ('pluto-principal28-' + label)
    status = json.loads(Path(str(prefix) + '-status.json').read_text())
    assert status['exitCode'] == 0 and status['finalizedOnly'] and not status['databaseWrites'] and not status['productionMode'], status
    assert (status['firstSlot'],status['lastSlot'])==(first_slot,stop_slot-1),'wrong replay scope'
    assert status['packageSha256AtLaunch']==status['packageSha256AfterReplay']==expected_package_hashes[label[-2:]],'replay SPKG hash differs'
    assert Path(str(prefix)+'.jsonl').stat().st_size==status['outputBytes'],'incomplete replay byte count'
    statuses[label]=status;status_hashes[label]=sha(Path(str(prefix)+'-status.json'))
    with Path(str(prefix) + '.jsonl').open() as stream:
        for line in stream:
            if line.startswith('{'):
                yield json.loads(line)

def normalized(row):
    return {**row['compositePk']['keys'], **{f['name']: f.get('value', '') for f in row['fields'] if f['name'] != 'decoder_version'}}

old_label,new_label='baseline27','candidate28'
first_slot,stop_slot=456410752,456412316
seen = {label: collections.defaultdict(set) for label in [old_label, new_label]}
totals = {label: collections.Counter() for label in seen}
previous = {label: None for label in seen}
added, removed_diagnostics, new_diagnostics, changes = [], [], [], []
lost=[]
unchanged = 0
corrections = []
block_count = 0
sentinel = object()
for old_block, new_block in itertools.zip_longest(blocks(old_label), blocks(new_label), fillvalue=sentinel):
    assert old_block is not sentinel and new_block is not sentinel, 'different block counts'
    assert old_block['@block'] == new_block['@block']
    assert first_slot <= old_block['@block'] < stop_slot
    block_count += 1
    maps = {}
    for label, version, data in [(old_label, '27', old_block), (new_label, '28', new_block)]:
        rows = collections.defaultdict(dict)
        ordinals = []
        for row in data['@data']['tableChanges']:
            assert next(f['value'] for f in row['fields'] if f['name'] == 'decoder_version') == 'v0.5.2-pluto.' + version
            assert row['operation']=='OPERATION_CREATE','unexpected replay database operation'
            names=[f['name'] for f in row['fields']];assert len(names)==len(set(names)),'duplicate row field'
            table = row['table']
            assert table in ['blocks','swaps','swap_coverage','swap_coverage_transactions'],'unexpected replay table'
            key = json.dumps(row['compositePk'], sort_keys=True)
            assert key not in seen[label][table], (label, table, key)
            seen[label][table].add(key)
            rows[table][key] = normalized(row)
            totals[label][table] += 1
            ordinals.append(int(row.get('ordinal', 0)))
            if table == 'swaps':
                f = rows[table][key]
                assert (f['transfer_verified'] == '1' or f['orderbook_fill_verified'] == '1' or f.get('burn_output_verified','0') == '1') and int(f['input_amount']) > 0 and int(f['output_amount']) > 0
        assert sorted(ordinals) == list(range(len(ordinals)))
        assert len(rows['blocks']) == 1
        block = next(iter(rows['blocks'].values()))
        assert int(block['block_num'])==data['@block'] and int(block['parent_slot'])<int(block['block_num']),'invalid sealed block identity'
        if previous[label]:assert int(block['block_num'])>int(previous[label]['block_num']),'replay block order is not increasing'
        for counter, table in [('verified_swaps', 'swaps'), ('coverage_candidates', 'swap_coverage'), ('coverage_transactions', 'swap_coverage_transactions')]:
            assert int(block[counter]) == len(rows[table])
        if previous[label]:
            assert (block['parent_slot'], block['parent_hash']) == (previous[label]['block_num'], previous[label]['block_hash'])
        previous[label] = block
        maps[label] = rows
    old, new = maps[old_label], maps[new_label]
    lost.extend(old['swaps'][k] for k in old['swaps'].keys()-new['swaps'].keys())
    for key, financial in old['swaps'].items():
        if key not in new['swaps']:continue
        candidate = new['swaps'][key]
        assert financial == candidate, ('prior financial/provenance changed', key)
        unchanged += 1
    local_added = [row for key, row in new['swaps'].items() if key not in old['swaps']]
    added.extend(local_added)
    local_diag = {}
    for table in ['swap_coverage', 'swap_coverage_transactions']:
        for key in old[table].keys() & new[table].keys():
            if old[table][key]!=new[table][key]: changes.append({'diagnosticTable':table,'old':old[table][key],'new':new[table][key]})
        gone = [row for key, row in old[table].items() if key not in new[table]]
        fresh = [row for key, row in new[table].items() if key not in old[table]]
        local_diag[table] = len(fresh) - len(gone)
        removed_diagnostics.extend({'table': table, **row} for row in gone)
        new_diagnostics.extend({'table': table, **row} for row in fresh)
    assert old['blocks'].keys() == new['blocks'].keys()
    a, b = next(iter(old['blocks'].values())), next(iter(new['blocks'].values()))
    assert a.keys()==b.keys(),'sealed block schema changed'
    for field in ['block_hash', 'parent_hash', 'parent_slot', 'timestamp', 'block_num']:
        assert a[field] == b[field]
    delta = {key: int(b[key]) - int(a[key]) for key in a if a[key] != b[key]}
    assert delta.get('verified_swaps', 0) == len(local_added)
    for counter, table in [('coverage_candidates', 'swap_coverage'), ('coverage_transactions', 'swap_coverage_transactions')]:
        assert delta.get(counter, 0) == local_diag[table]
    assert set(delta) <= {'verified_swaps', 'coverage_candidates', 'coverage_transactions', 'corrected_events', 'quarantined_swaps', 'duplicate_wrappers'}, (a['block_num'], delta)
    if delta:
        changes.append({'slot': a['block_num'], 'deltas': delta})

assert not lost, 'prior swap lost'
original_path=Path(__file__).with_name('original-proof.json')
original=json.loads(original_path.read_text())
expected={(r['signature'],str(r['source'])):r for r in original['originals']}
assert len(added)==len(expected)==4
for row in added:
    r=expected.pop((row['signature'],row['source_instruction_index']))
    for field in ('input_mint','output_mint','input_amount','output_amount','user'):
        assert str(r[field])==row[field],(field,r,row)
    assert str(r['slot'])==row['block_num'] and r['pool']==row['amm']
    assert row['protocol']=={32:'deriverse',63:'ghosty',70:'omnipair'}[r['protocol']]
    assert row['transfer_verified']=='1'
assert not expected

hashes = {}
for label in seen:
    h = hashlib.sha256()
    with (BASE / ('pluto-principal28-' + label + '.jsonl')).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    hashes[label] = h.hexdigest()
proof = {'release': 'v0.5.2-pluto.28', 'firstSlot': first_slot, 'lastSlot': stop_slot - 1, 'blocks': block_count,
         'outputs': {label: {'sha256': hashes[label], 'tables': dict(totals[label]), 'cleanExit': True, 'packageSha256': json.loads((BASE / ('pluto-principal28-' + label + '-status.json')).read_text())['packageSha256AtLaunch']} for label in seen},
         'unchangedPriorSwaps': unchanged, 'preservedPriorSwapIdentities': unchanged + len(corrections), 'missingPriorSwaps':len(lost),'lostPriorSwaps':lost, 'modifiedPriorSwaps': len(corrections), 'priorFinancialModificationVerificationPending':bool(corrections),'unexpectedModifiedPriorSwaps':0, 'correctedPriorSwapCount': len(corrections), 'correctedPriorSwaps': corrections,
         'added': added, 'removedDiagnostics': removed_diagnostics, 'newDiagnostics': new_diagnostics,
         'sealedCounterChanges': changes, 'parentChainsAndSealedCountersValid': True,
         'allDatabaseOperationKeysAndOrdinalsUnique': True, 'allFourTableVersionsValidated': True,
         'originalRpcVerificationPending': False, 'databaseWrites': False,
         'actualPackageBytesVerified':True,'packageArtifacts':{version:{'path':str(p),'sha256':expected_package_hashes[version]} for version,p in package_paths.items()},
         'runtimeSourceCommitReferences':{'27':args.baseline_runtime_source_commit,'28':args.final_runtime_source_commit},
         'sourceCommitReferenceLimitation':'Captured build-provenance references; source commits alone do not cryptographically prove WASM build correspondence.',
         'statusArtifacts':{label:{'path':str(BASE/('pluto-principal28-'+label+'-status.json')),'sha256':status_hashes[label]} for label in seen},
         'verificationScriptSha256':sha(Path(__file__))}
assert all(sha(p)==expected_package_hashes[v] for v,p in package_paths.items()),'SPKG changed during comparison'
assert all(sha(BASE/('pluto-principal28-'+label+'-status.json'))==status_hashes[label] for label in seen),'status changed during comparison'
(BASE / 'pluto-principal28-packaged-replay-proof.json').write_text(json.dumps(proof, indent=2) + '\n')
print(json.dumps({'blocks': block_count, 'unchanged': unchanged, 'added': len(added), 'addedByProtocol': dict(collections.Counter(x['protocol'] for x in added)), 'missing':len(lost), 'modified': len(corrections), 'counterChanges': len(changes)}))

assert not lost,'Prior swap identities lost; retained exact lost rows in comparator proof. Bounded Pancake attribution changes still require independent final financial verification.'
