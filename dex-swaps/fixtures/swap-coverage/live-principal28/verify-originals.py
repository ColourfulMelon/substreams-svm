"""Offline financial/RPC parity for the four retained finalized missing swaps."""
import collections, hashlib, json
from pathlib import Path
def varint(data,p):
    n=shift=0
    while True:
        v=data[p];p+=1;n|=(v&127)<<shift
        if not v&128:return n,p
        shift+=7
def message(data):
    result=collections.defaultdict(list);p=0
    while p<len(data):
        tag,p=varint(data,p);field=tag>>3;wire=tag&7
        if wire==0:value,p=varint(data,p)
        elif wire==2:
            length,p=varint(data,p);value=data[p:p+length];p+=length
        elif wire in (1,5):
            length=8 if wire==1 else 4;value=data[p:p+length];p+=length
        else:raise RuntimeError('Unexpected protobuf wire type')
        result[field].append(value)
    return result
def b58(data):
    alphabet='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';n=int.from_bytes(data,'big');s=''
    while n:n,r=divmod(n,58);s=alphabet[r]+s
    return '1'*(len(data)-len(data.lstrip(b'\0')))+s

def decode58(value):
    n=0
    for character in value:n=n*58+'123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'.index(character)
    return b'\0'*(len(value)-len(value.lstrip('1')))+(n.to_bytes((n.bit_length()+7)//8,'big') if n else b'')
def uints(values):
    result=[]
    for v in values:
        if isinstance(v,int):result.append(v)
        else:
            p=0
            while p<len(v):n,p=varint(v,p);result.append(n)
    return result
root=Path(__file__).resolve().parent
proof=[]
for row in (root/'cases.tsv').read_text().splitlines():
    f=row.split('\t')
    if f[10]!='1':continue
    raw=(root/(f[0]+'.pb')).read_bytes();original=message(raw);transaction=message(original[1][0]);m=message(transaction[2][0]);meta=message(original[2][0])
    record=json.loads((root/(f[0]+'.rpc.json')).read_text());rpc=record['response']['result'];rm=rpc['transaction']['message'];rmeta=rpc['meta']
    assert record['request']['params'][1]['commitment']=='finalized' and rmeta['err'] is None and not meta[1]
    assert [b58(x) for x in transaction[1]]==rpc['transaction']['signatures'] and f[0]==rpc['transaction']['signatures'][0]
    assert [b58(x) for x in m[2]]==rm['accountKeys'] and b58(m[3][0])==rm['recentBlockhash']
    header=message(m[1][0]);assert [header.get(n,[0])[0] for n in [1,2,3]]==[rm['header'][k] for k in ['numRequiredSignatures','numReadonlySignedAccounts','numReadonlyUnsignedAccounts']]
    assert [b58(x) for x in meta[12]]==rmeta['loadedAddresses']['writable'] and [b58(x) for x in meta[13]]==rmeta['loadedAddresses']['readonly']
    assert meta.get(2,[0])[0]==rmeta['fee'] and uints(meta[3])==rmeta['preBalances'] and uints(meta[4])==rmeta['postBalances']
    assert [x.decode() for x in meta[6]]==rmeta['logMessages'] and 'Log truncated' not in rmeta['logMessages']
    assert meta.get(16,[None])[0]==rmeta.get('computeUnitsConsumed')
    # Firehose currently omits optional cost_units; RPC supplies it. Its absence
    # is retained explicitly rather than inventing an original protobuf value.
    if meta[17]:assert meta[17][0]==rmeta.get('costUnits')
    if rpc['version']==1:
        assert m[5]==[1]
        if m[7]:assert m[7]==[1]
        if m[8]:
            config=message(m[8][0])
            for n,k in [(1,'priorityFee'),(2,'computeUnitLimit'),(3,'loadedAccountsDataSizeLimit'),(4,'heapSize')]:assert config.get(n,[None])[0]==rm['transactionConfig'][k]
    keys=rm['accountKeys']+rmeta['loadedAddresses']['writable']+rmeta['loadedAddresses']['readonly'];walk=[]
    pi={message(x).get(1,[0])[0]:message(x)[2] for x in meta[5]};ri={x['index']:x['instructions'] for x in rmeta['innerInstructions']};assert set(pi)==set(ri)
    assert len(m[4])==len(rm['instructions'])
    for index,(encoded,ix) in enumerate(zip(m[4],rm['instructions'])):
        natives=[(encoded,ix,1)]+[(a,b,b['stackHeight']) for a,b in zip(pi.get(index,[]),ri.get(index,[]))];assert len(pi.get(index,[]))==len(ri.get(index,[]))
        for encoded,ix,depth in natives:
            p=message(encoded);assert p.get(1,[0])[0]==ix['programIdIndex'] and list(p.get(2,[b''])[0])==ix['accounts'] and p.get(3,[b''])[0]==decode58(ix['data'])
            if depth!=1:assert p.get(4,[None])[0]==depth
            walk.append(dict(index=len(walk),depth=depth,program=keys[ix['programIdIndex']],accounts=[keys[x] for x in ix['accounts']],data=decode58(ix['data'])))
    mint_map={}
    for field,label in [(7,'preTokenBalances'),(8,'postTokenBalances')]:
        values=[]
        for encoded in meta[field]:
            p=message(encoded);amount=message(p[3][0]);values.append((p.get(1,[0])[0],p[2][0].decode(),p.get(4,[b''])[0].decode(),p.get(5,[b''])[0].decode(),amount.get(2,[0])[0],amount.get(3,[b''])[0].decode()))
        expected=[(v['accountIndex'],v['mint'],v.get('owner',''),v.get('programId',''),v['uiTokenAmount']['decimals'],v['uiTokenAmount']['amount']) for v in rmeta[label]]
        assert values==expected
        mint_map.update({keys[v['accountIndex']]:v['mint'] for v in rmeta[label]})
    for ix in walk:
        if ix['program'].startswith('Token') and ix['data'][:1]==b'\x12' and len(ix['data'])==33 and len(ix['accounts'])==2:mint_map.setdefault(ix['accounts'][0],ix['accounts'][1])
    source=int(f[1]);native=walk[source];a=native['accounts'];protocol=int(f[2]);input_role,output_role=(11,12) if protocol==32 else (5,6) if protocol==70 else (8,9);vaults=(3,4) if protocol in [32,70] else (6,7)
    expected_program={32:'DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD',70:'omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE',63:'ghosty4ZU1Qk1HN7Ymz4pZ15QfspzJZgSYFkdKN6ZLK'}[protocol]
    assert native['program']==expected_program and a[5 if protocol==32 else 0 if protocol==70 else 1]==f[3] and a[9 if protocol==70 else 0]==f[4]
    end=next((ix['index'] for ix in walk[source+1:] if ix['depth']<=native['depth']),len(walk));flows=[];ledger=collections.defaultdict(int)
    for ix in walk:
        d=ix['data'];ca=ix['accounts']
        if not ix['program'].startswith('Token'):continue
        if d[:1]==b'\x03' and len(d)==9:from_account,to_account,mint=ca[0],ca[1],mint_map[ca[0]]
        elif d[:1]==b'\x0c' and len(d)==10:from_account,to_account,mint=ca[0],ca[2],ca[1]
        else:continue
        assert mint_map[from_account]==mint_map[to_account]==mint
        amount=int.from_bytes(d[1:9],'little');ledger[from_account]-=amount;ledger[to_account]+=amount
        if source<ix['index']<end and ix['depth']==native['depth']+1:flows.append((ix,from_account,to_account,mint,amount))
    assert len(flows)==2
    first,second=flows
    assert first[0]['index']==int(f[9]) and first[1:]==(a[input_role],a[vaults[0]],f[5],int(f[6])) and second[1:]==(a[vaults[1]],a[output_role],f[7],int(f[8]))
    for role in vaults:
        address=a[role];before=next(v for v in rmeta['preTokenBalances'] if keys[v['accountIndex']]==address);after=next(v for v in rmeta['postTokenBalances'] if keys[v['accountIndex']]==address)
        assert int(after['uiTokenAmount']['amount'])-int(before['uiTokenAmount']['amount'])==ledger[address]
    block_file=root/(str(rpc['slot'])+'.block.rpc.json');block_record=json.loads(block_file.read_text());block=block_record['response']['result']
    assert block_record['request']['params'][1]['commitment']=='finalized' and block_record['request']['params'][0]==rpc['slot']
    assert block['blockTime']==rpc['blockTime'] and block['signatures'].count(f[0])==1
    expected_financial={
        'signature':f[0], 'fee_payer':keys[0], 'signers_raw':','.join(rm['accountKeys'][:rm['header']['numRequiredSignatures']]),
        'fee':str(rmeta['fee']), 'compute_units_consumed':str(rmeta['computeUnitsConsumed']),
        'program_id':expected_program, 'stack_height':str(native['depth']), 'event_id':f'{f[0]}:{source}:{first[0]["index"]}',
        'source_instruction_index':str(source), 'execution_position':str(first[0]['index']),
        'transfer_verified':'1', 'orderbook_fill_verified':'0', 'burn_output_verified':'0', 'source_burn_index':'0',
        'protocol':{32:'deriverse',63:'ghosty',70:'omnipair'}[protocol], 'amm':expected_program, 'amm_pool':f[3], 'user':f[4],
        'input_mint':f[5], 'input_amount':f[6], 'output_mint':f[7], 'output_amount':f[8],
        'instruction_index':str(first[0]['index']), 'block_num':str(rpc['slot']), 'timestamp':str(block['blockTime']),
        'block_hash':block['blockhash'], 'transaction_index':str(block['signatures'].index(f[0])),
    }
    assert len(expected_financial)==27
    proof.append(dict(signature=f[0],slot=rpc['slot'],protocol=protocol,source=source,input_mint=f[5],input_amount=int(f[6]),output_mint=f[7],output_amount=int(f[8]),pool=f[3],user=f[4],pb_sha256=hashlib.sha256(raw).hexdigest(),rpc_sha256=hashlib.sha256((root/(f[0]+'.rpc.json')).read_bytes()).hexdigest(),finalized_rpc_financial_metadata_parity=True,pb_optional_cost_units_missing=not bool(meta[17]),rpc_cost_units=rmeta.get('costUnits'),pb_v1_version_and_config_missing=rpc['version']==1 and not m[7] and not m[8],persistent_vault_ledgers_verified=True,block_rpc_sha256=hashlib.sha256(block_file.read_bytes()).hexdigest(),expected_financial=expected_financial))
assert len(proof)==4
result=dict(originals=proof,confirmed_swaps=4,remaining_zero_output_sources=41,scope='Four native positive sources, forty original transactions and forty-five recognized diagnostics in finalized slots 456410496..456413279; no global coverage claim')
(root/'original-proof.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='originals'}))
