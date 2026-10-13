#!/usr/bin/env python3
"""Strict packaged financial/provenance matcher. Amounts come only from pinned independent venue oracles.
--prepare retains oracle inputs/required finalized block slots. --inventory permits incomplete replay
input for assignment only; neither mode claims final verification. Normal mode requires sealed comparator proofs.
"""
import argparse, collections, datetime, gzip, hashlib, json, pathlib, re, subprocess, sys
RESOURCE=pathlib.Path(__file__).resolve().parent
BASE=RESOURCE
HERE=BASE/'combined27-financial'
FIX=RESOURCE.parent
NATIVE=['live-dlmm-host-fees27','picon-native27','legacy-amm27','m0-native27','tessera-flag27']
CASES=[]
LEGS=['pump-roundtrip27','pump-native-sell27','pumpswap-boost-burn27','patch-bxlw27']
TOKEN={'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA','TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb'}
def read(p):return json.loads(p.read_text())
def dump(p,a):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(a,indent=2)+'\n')
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def b58(s):
 n=0
 for c in s:n=n*58+'123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'.index(c)
 return b'\0'*(len(s)-len(s.lstrip('1')))+(n.to_bytes((n.bit_length()+7)//8,'big') if n else b'')
def rpc_read(p):
 data=p.read_bytes(); obj=json.loads(gzip.decompress(data) if p.suffix=='.gz' else data)
 return obj.get('response',obj).get('result',obj)
def walk_rpc(r):
 keys=r['transaction']['message']['accountKeys']+r['meta'].get('loadedAddresses',{}).get('writable',[])+r['meta'].get('loadedAddresses',{}).get('readonly',[])
 groups={g['index']:g['instructions'] for g in r['meta']['innerInstructions']}
 walk=[]
 for n,x in enumerate(r['transaction']['message']['instructions']):
  for y in [dict(x,stackHeight=1)]+groups.get(n,[]):walk.append(dict(program=keys[y['programIdIndex']],depth=y['stackHeight'],accounts=[keys[z] for z in y['accounts']],data=b58(y['data'])))
 return walk

def prepare():
 ids={int(k):v for k,v in read(RESOURCE/'protocol-ids.json').items()}
 proofs=[]; rows=[];txs={}; exclusions=[]
 extras=read(RESOURCE/'extra-proof-families.json') if (RESOURCE/'extra-proof-families.json').exists() else []
 for entry in NATIVE+CASES+LEGS+extras:
  name=entry['family'] if isinstance(entry,dict) else entry
  directory=pathlib.Path(entry['raw']) if isinstance(entry,dict) and 'raw' in entry else FIX/name.split('/')[0]
  p=pathlib.Path(entry['proof']) if isinstance(entry,dict) and 'proof' in entry else directory/((name.split('/')[1]+'-independent-proof.json') if '/' in name else 'independent-proof.json')
  a=read(p);ph=sha(p); proofs.append(dict(family=name,path=str(p),sha256=ph))
  if isinstance(a,dict):
   assert not a.get('failures',[]),(name,'independent oracle failures')
   for flag in ['allFinalizedOriginalRpcPbParity','allPrincipalTokenAndNativeLedgers','allFourPrincipalLedgers']:
    if flag in a:assert a[flag] is True,(name,flag,'independent proof incomplete')
  failures=directory/'independent-failures.json'
  if failures.exists():assert read(failures)==[],(name,'retained independent failures')
  collection=a if isinstance(a,list) else a.get('originals',a.get('sources',a.get('results',[])))
  cases={}
  if name in CASES or isinstance(a,dict) and 'sources' in a:
   cp=directory/'cases.tsv';proofs.append(dict(family=name+'-independent-cases',path=str(cp),sha256=sha(cp)))
   for line in cp.read_text().splitlines():
    if not line or line.startswith('#'):continue
    sig,src,protocol,pool,user,im,ia,om,oa=line.split('\t');cases[(sig,int(src))]=dict(protocol=ids[int(protocol)],pool=pool,user=user,inputMint=im,input=ia,outputMint=om,output=oa)
  for t in collection:
   sig=t['signature']; slot=t.get('slot',t.get('block_num'));pb=directory/(sig+'.pb');rpc=directory/(sig+'.rpc.json')
   if not rpc.exists():rpc=pathlib.Path(str(rpc)+'.gz')
   assert pb.exists() and rpc.exists(),(name,sig,'missing originals')
   pbhash=sha(pb);rpchash=sha(rpc)
   retainedpb=t.get('pbSha256',t.get('rawPbSha256',t.get('raw_pb_sha256')))
   assert retainedpb is None or retainedpb==pbhash,(name,sig,'PB hash differs')
   retainedrpc=t.get('rpcGzipSha256',t.get('rpcSha256',t.get('finalized_rpc_sha256')))
   uncompressed=hashlib.sha256(gzip.decompress(rpc.read_bytes())).hexdigest() if rpc.suffix=='.gz' else rpchash
   assert retainedrpc in [None,rpchash,uncompressed],(name,sig,'RPC hash differs')
   original=dict(signature=sig,slot=slot,pbPath=str(pb),rpcPath=str(rpc),pbSha256=pbhash,rpcSha256=rpchash,oracleOriginalHashRetained=retainedpb is not None,explicitRpcRuntimeTruncation=t.get('rpcRuntimeTruncated',t.get('rpcLogsTruncated',False)))
   retention=[t[k] for k in ['rpcRuntimeTruncated','rpcLogsTruncated'] if k in t]
   assert all(isinstance(v,bool) for v in retention) and len(set(retention))<=1,(name,sig,'conflicting/nonboolean RPC retention flags')
   retained=rpc_read(rpc)
   assert ('Log truncated' in retained['meta']['logMessages'])==original['explicitRpcRuntimeTruncation'],(name,sig,'explicit RPC retention limitation missing or incorrect')
   w=walk_rpc(retained); txs[(sig,pbhash,rpchash)]=original
   legs=[t['financialRow']] if 'financialRow' in t else t.get('legs',[t]) if name in LEGS or isinstance(entry,dict) and entry.get('style')=='legs' else [t]
   for x in legs:
    if not x.get('priceable',True):exclusions.append(dict(family=name,signature=sig,source=x['source_index'],reason='independently proved zero proceeds'));continue
    source=x.get('source_index',x.get('sourceIndex',x.get('instruction_index')));tr=x.get('first_transfer_index',x.get('sourceTransfer')); log=x.get('source_log_index');src=w[source]; program=x.get('amm',x.get('program_id',src['program']))
    if name in CASES or isinstance(a,dict) and 'sources' in a:
     c=cases[(sig,source)]; assert (c['input'],c['output'])==(t['exactInput'],t['exactOutput']),(name,sig,'independent oracle/cases disagree')
     assert all(l['expected']==l['actual'] for l in t['ledgers']),(name,sig,'whole ledger failure')
     x={**x,**c};desc=[]
     a0=src['accounts'];data=src['data']
     if c['protocol']=='ghosty':roles=[1,0,2,3];assert src['program']=='ghosty4ZU1Qk1HN7Ymz4pZ15QfspzJZgSYFkdKN6ZLK' and len(a0)==16 and len(data)==24 and data[:8].hex()=='f8c69e91e17587c8'
     elif c['protocol']=='memefun':roles=[0,5,7 if data[24] else 6,6 if data[24] else 7];assert src['program']=='MEME4gfGe6itEpxgM1xaBVmiFF5XuCTc4oVXUE5w56F' and len(a0)==10 and len(data)==25 and data[:8].hex()=='f8c69e91e17587c8' and data[24]<=1
     elif c['protocol']=='omnipair':roles=[0,9,7,8];assert src['program']=='omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE' and len(a0)==14 and len(data)==24 and data[:8].hex()=='f8c69e91e17587c8'
     elif c['protocol']=='dr_fraudsworth':roles=[1,8,4+data[16],5-data[16]];assert len(a0)==15 and len(data)==25 and data[:8].hex()=='de801e7b5527918a' and data[16]<=1
     elif c['protocol']=='sanctum_infinity':roles=[7+int(data[0]==1),0,1,2];assert src['program']=='5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx' and len(data)==27 and data[0] in [1,23]
     else:raise AssertionError('unmapped independent case roles')
     assert [a0[n] for n in roles]==[c['pool'],c['user'],c['inputMint'],c['outputMint']],(name,sig,'primary source account role/cases mismatch')
     requested=int.from_bytes(data[19:27] if c['protocol']=='sanctum_infinity' else data[8:16],'little');assert requested==int(t['exactInput']),(name,sig,'primary source request/oracle mismatch')
     for n in range(source+1,len(w)):
      if w[n]['depth']<=src['depth']:break
      if w[n]['program'] in TOKEN and w[n]['data'] and w[n]['data'][0] in [3,12,26]:desc.append(n)
     assert desc,(name,sig,'no native physical receipt');tr=desc[0]
    protocol=x.get('protocol','cryptoquest');protocol=ids[protocol] if isinstance(protocol,int) else protocol
    financial=dict(signature=sig,protocol=protocol,amm=program,amm_pool=x['pool'],user=x['user'],input_mint=x.get('input_mint',x.get('inputMint')),input_amount=x.get('input_amount',x.get('input')),output_mint=x.get('output_mint',x.get('outputMint')),output_amount=x.get('output_amount',x.get('output')))
    assert source is not None and (tr is not None or log is not None) and all(v is not None for v in financial.values()),(name,x)
    if x.get('burn_output_verified',False):
     burn=x.get('source_burn_index');assert isinstance(burn,int) and burn>source and burn<len(w),(name,sig,'missing typed burn source')
     b=w[burn];assert b['program'] in TOKEN and b['data'][0] in [8,15] and len(b['data'])==(9 if b['data'][0]==8 else 10) and b['accounts'][1]==financial['output_mint'] and int.from_bytes(b['data'][1:9],'little')==int(financial['output_amount']),(name,sig,'independent output burn receipt differs')
    assert program==src['program'],(name,sig,'native source program differs')
    stack=source
    # Pump event-decoded rows preserve their committed native event CPI depth.
    if protocol in ['pumpfun','pumpfun_amm']:
     events=[n for n in range(source+1,len(w)) if w[n]['program']==program and w[n]['data'].hex().startswith('e445a52e51cb9a1d') and w[n]['depth']==src['depth']+1]
     nextnative=next((n for n in range(source+1,len(w)) if w[n]['depth']<=src['depth']),len(w));events=[n for n in events if n<nextnative]
     assert len(events)==1,(name,sig,source,'native event CPI identity');stack=events[0]
    ix=32768+log if log is not None else tr
    financial.update(program_id=program,source_instruction_index=source,instruction_index=ix,event_id=f'{sig}:{source}:log:{log}' if log is not None else f'{sig}:{source}:{tr}',transfer_verified='0' if log is not None or x.get('transfer_verified') is False else '1',orderbook_fill_verified='1' if log is not None else '0',burn_output_verified='1' if x.get('burn_output_verified',False) else '0',source_burn_index=str(x.get('source_burn_index') or 0))
    rows.append(dict(family=name,slot=slot,stackInstruction=stack,sourceTransfer=tr,sourceLog=log,physicalTransfer=x.get('physical_quest_transfer_index',tr),financial={k:str(v) for k,v in financial.items()},original=original,proofPath=str(p),proofSha256=ph,oracleRecord=x))
 keys=[(x['financial']['signature'],x['financial']['source_instruction_index'],x['financial']['instruction_index']) for x in rows];assert len(keys)==len(set(keys)),'duplicate independent proof identity'
 result=dict(scope='Pinned independent original financial oracles; no packaged verification yet',rows=rows,originals=list(txs.values()),proofs=proofs,exclusions=exclusions,slots=sorted({x['slot'] for x in txs.values()}))
 dump(HERE/'oracle-inputs.json',result);dump(HERE/'required-finalized-block-slots.json',result['slots']);print(json.dumps(dict(independentRows=len(rows),originals=len(txs),slots=len(result['slots']),byFamily=dict(collections.Counter(x['family'] for x in rows)))))
 return result

def normalized(r):return {**r['compositePk']['keys'],**{x['name']:x.get('value','') for x in r['fields'] if x['name']!='decoder_version'}}
def inventory(args):
 expected=prepare();keys={(r['financial']['signature'],r['financial']['source_instruction_index'],r['financial']['instruction_index']):r for r in expected['rows']};report=[]
 for old,new in [('baseline26','candidate27'),('live26','live27'),('wide26','wide27')]:
  records=[];newp=pathlib.Path(args.inventory_root)/('pluto-combined27-'+new+'.jsonl');oldp=BASE/('pluto-combined27-'+old+'.jsonl')
  if not oldp.exists() or not newp.exists():
   report.append(dict(label=new,phase='REPLAY INPUT NOT AVAILABLE',rows=[],missingPaths=[str(p) for p in [oldp,newp] if not p.exists()]));continue
  with oldp.open() as a,newp.open() as b:
   for oldline,newline in zip((x for x in a if x.startswith('{')),(x for x in b if x.startswith('{'))):
    try:o,n=json.loads(oldline),json.loads(newline)
    except json.JSONDecodeError:break
    assert o['@block']==n['@block'];prior={json.dumps(r['compositePk'],sort_keys=True):normalized(r) for r in o['@data']['tableChanges'] if r['table']=='swaps'}
    for r in n['@data']['tableChanges']:
     if r['table']!='swaps':continue
     x=normalized(r);key=json.dumps(r['compositePk'],sort_keys=True);y=prior.get(key)
     if y=={k:v for k,v in x.items() if k not in ['burn_output_verified','source_burn_index']}:continue
     identity=(x['signature'],x['source_instruction_index'],x['instruction_index']); oracle=keys.get(identity)
     differences={k:dict(expected=v,actual=x.get(k)) for k,v in oracle['financial'].items() if x.get(k)!=v} if oracle else None
     records.append(dict(kind='added' if y is None else 'correction',row=x,old=y,oracleFamily=oracle['family'] if oracle else None,differences=differences))
  report.append(dict(label=new,phase='PARTIAL INVENTORY ONLY',rows=records,byProtocol=dict(collections.Counter(x['row']['protocol'] for x in records)),unmatched=sum(x['oracleFamily'] is None for x in records),mismatches=sum(bool(x['differences']) for x in records)))
 dump(HERE/'partial-inventory.json',report);print(json.dumps([{k:v for k,v in x.items() if k!='rows'}|dict(rows=len(x['rows'])) for x in report]))

def verify(args):
 assert args.expected_package_sha256 and re.fullmatch(r'[0-9a-f]{64}',args.expected_package_sha256),'explicit final package SHA256 required'
 assert args.baseline_package and args.final_package and args.expected_baseline_package_sha256,'actual baseline/final SPKG paths and expected SHA256 required'
 package_paths={'26':pathlib.Path(args.baseline_package),'27':pathlib.Path(args.final_package)};package_hashes={'26':args.expected_baseline_package_sha256,'27':args.expected_package_sha256}
 assert all(sha(p)==package_hashes[v] for v,p in package_paths.items()),'actual SPKG byte hash mismatch'
 inputs=prepare();scopes=[read(pathlib.Path(p)) for p in args.replay_proof]
 inputs={**inputs,'rows':[r for r in inputs['rows'] if any(p['firstSlot']<=r['slot']<=p['lastSlot'] for p in scopes)],'originals':[r for r in inputs['originals'] if any(p['firstSlot']<=r['slot']<=p['lastSlot'] for p in scopes)]}
 dump(HERE/'verification-oracle-inputs.json',inputs)
 subprocess.run(['node',str(RESOURCE/'original-metadata.cjs'),str(HERE/'verification-oracle-inputs.json'),args.block_cache,str(HERE/'original-metadata.json'),'--node-dependency-package',args.node_dependency_package,'--protobuf-package',args.protobuf_package],check=True)
 meta=read(HERE/'original-metadata.json');txmeta={(t['signature'],t['pbSha256'],t['rpcSha256']):t for t in meta['transactions']}
 expected={};evidence=[]
 for r in inputs['rows']:
  f=r['financial'];s=int(f['source_instruction_index']);tx=txmeta[(f['signature'],r['original']['pbSha256'],r['original']['rpcSha256'])];tr=r['physicalTransfer'];log=r['sourceLog'];w=tx['instructions']
  assert tx['committed'][s] and w[s]['program']==f['program_id'],'uncommitted native source'
  assert tr is not None and tx['committed'][tr] and s in tx['ancestors'][tr],'physical receipts outside committed source'
  burn=int(f['source_burn_index']);
  if f['burn_output_verified']=='1':assert burn>0 and tx['committed'][burn] and s in tx['ancestors'][burn] and tx['committed'][tr],'burn/quote receipt outside committed native source'
  else:assert burn==0,'non-burn row carries burn provenance'
  stack=r['stackInstruction'];assert tx['committed'][stack] and w[stack]['program']==f['amm'] and (stack==s or s in tx['ancestors'][stack]),'unbound event stack'
  if not tx['finalizedRpcRuntimeComplete']:assert r['original']['explicitRpcRuntimeTruncation'],'unrecorded RPC runtime retention limitation'
  if log is not None:
   active=[];cursor=0;logsource=None
   for n,line in enumerate(tx['originalLogs']):
    inv=re.match(r'^Program (\S+) invoke \[(\d+)\]$',line)
    if inv:
     while cursor<len(w) and w[cursor]['program']!=inv[1]:cursor+=1
     assert cursor<len(w);active.append(cursor);cursor+=1
    if n==log:logsource=list(active);break
    if re.match(r'^Program \S+ (success|failed:)',line):active.pop()
   assert logsource and logsource[-1]==s,'native financial log outside own committed source'
  fields={**f,**{k:str(tx[k]) for k in ['fee_payer','signers_raw','fee','compute_units_consumed','block_num','block_hash','transaction_index']},'timestamp':str(tx['timestamp']),'stack_height':str(w[stack]['depth'])}
  expected[(f['signature'],f['source_instruction_index'],f['instruction_index'])]=dict(record=r,fields=fields)
 matched=[];missing=[];mismatches=[];per=[];seen=set()
 for p in [pathlib.Path(x) for x in args.replay_proof]:
  replay=read(p);assert replay['release']=='v0.5.2-pluto.27' and replay['actualPackageBytesVerified'] and replay['allFourTableVersionsValidated'] and replay['verificationScriptSha256']==sha(RESOURCE/'compare-replays.py'),'unbound comparator contract'
  assert replay['missingPriorSwaps']==0 and replay['parentChainsAndSealedCountersValid'] and replay['allDatabaseOperationKeysAndOrdinalsUnique'] and not replay['databaseWrites'];assert replay['modifiedPriorSwaps']==len(replay['correctedPriorSwaps'])==replay['correctedPriorSwapCount'] and replay['unexpectedModifiedPriorSwaps']==0,'unexpected prior financial/provenance modification'
  for label,out in replay['outputs'].items():
   statuspath=BASE/('pluto-combined27-'+label+'-status.json');status=read(statuspath);assert sha(statuspath)==replay['statusArtifacts'][label]['sha256'],'status artifact changed after sealed comparison';assert status['exitCode']==0 and status['finalizedOnly'] and not status['databaseWrites'] and not status['productionMode'];assert (BASE/('pluto-combined27-'+label+'.jsonl')).stat().st_size==status['outputBytes'],'replay bytes no longer sealed';assert sha(BASE/('pluto-combined27-'+label+'.jsonl'))==out['sha256'];assert status['packageSha256AtLaunch']==out['packageSha256'];
   assert out['packageSha256']==package_hashes[label[-2:]],'replay package hash mismatch'
   assert status['packageSha256AfterReplay']==out['packageSha256'],'package changed during replay'
  label=next(x for x in replay['outputs'] if x.endswith('27'));full=collections.defaultdict(list)
  with (BASE/('pluto-combined27-'+label+'.jsonl')).open() as stream:
   for line in stream:
    if not line.startswith('{'):continue
    for row in json.loads(line)['@data']['tableChanges']:
     if row['table']=='swaps':x=normalized(row);full[x['signature']].append(x)
  assert all(out['sha256'] and out['cleanExit'] for out in replay['outputs'].values()),'incomplete comparator outputs'
  nmatched=0;nunmatched=0;ndiff=0
  for kind,a,old in [('added',x,None) for x in replay['added']]+[('correction',x['new'],x['old']) for x in replay['correctedPriorSwaps']]:
   key=(a['signature'],a['source_instruction_index'],a['instruction_index']);assert key not in seen,'duplicate replay scope identity';seen.add(key)
   if key not in expected:missing.append(dict(replay=label,kind=kind,row=a,old=old,reason='no independent original financial oracle'));nunmatched+=1;continue
   item=expected[key];r=item['record'];fields=dict(item['fields']);transaction=full[a['signature']]
   if any(':log:' in x['event_id'] for x in transaction):
    ordered=sorted(transaction,key=lambda x:(int(x['source_instruction_index']) if ':log:' in x['event_id'] else int(x['instruction_index']),int(x['event_id'].split(':log:')[1]) if ':log:' in x['event_id'] else -1));fields['execution_position']=str(next(i for i,x in enumerate(ordered) if (x['source_instruction_index'],x['instruction_index'])==key[1:]))
   else:fields['execution_position']=fields['instruction_index']
   diff={k:dict(expected=v,actual=a.get(k)) for k,v in fields.items() if a.get(k)!=v};unknown=set(a)-set(fields)
   if unknown:diff['unmapped_fields']=sorted(unknown)
   if kind=='correction':
    changed={k for k in set(old)|set(a) if k not in ['burn_output_verified','source_burn_index'] and old.get(k)!=a.get(k)}
    native='HpNfyc2Saw7RKkQd8nEL4khUcuPhQ7WwY1B2qjx8jxFq';assert a['protocol']=='pancakeswap' and a['program_id']==a['amm']==native and fields['protocol']=='pancakeswap','unproved Pancake correction identity'
    depth_fix=changed=={'stack_height'} and old['protocol']=='pancakeswap' and old['program_id']==native and old['stack_height']=='0' and 1<=int(fields['stack_height'])<=5
    router_fix=changed <= {'protocol','program_id','user','stack_height'} and {'protocol','program_id'} <= changed and old['protocol']=='jupiter_v6' and old['program_id']=='JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4' and old['amm']==native
    assert depth_fix or router_fix,'correction exceeds independently bounded Pancake provenance profile'
    assert all(old.get(k,'0')==v for k,v in fields.items() if k not in changed),'old financial/provenance fields disagree with independent original'
    assert all(a.get(k,'0')=='0' for k in ['burn_output_verified','source_burn_index']),'Pancake correction gained burn witness'
    tx=txmeta[(a['signature'],r['original']['pbSha256'],r['original']['rpcSha256'])];source=int(a['source_instruction_index'])
    assert tx['instructions'][source]['program']==native and tx['instructions'][source]['depth']==int(fields['stack_height']),'corrected Pancake depth not native original'
    if router_fix:
     ancestors=[tx['instructions'][n] for n in tx['ancestors'][source] if tx['instructions'][n]['program']==old['program_id']];assert ancestors,'old Jupiter attribution lacks original committed ancestor'
     if 'user'in changed:assert any(old['user']in ix['accounts'] for ix in ancestors),'old router user absent from original ancestor accounts'
    elif 'user'in changed:raise AssertionError('nonrouter economic user correction')
   result=dict(replay=label,kind=kind,family=r['family'],independentProofPath=r['proofPath'],independentProofSha256=r['proofSha256'],original=r['original'],expected=fields,actual=a,old=old,matched=not diff,priorCorrectionChangedFields=sorted(changed) if kind=='correction' else [],priorCorrectionOldFieldsIndependentlyChecked=kind=='correction')
   matched.append(result)
   if diff:mismatches.append(dict(replay=label,identity=key,family=r['family'],differences=diff));ndiff+=1
   else:nmatched+=1
  per.append(dict(label=label,comparatorPath=str(p),comparatorSha256=sha(p),packageSha256=package_hashes['27'],outputs=replay['outputs'],added=len(replay['added']),corrected=len(replay['correctedPriorSwaps']),matched=nmatched,unmatched=nunmatched,mismatches=ndiff))
 report=dict(release='v0.5.2-pluto.27',scope='Strict immutable packaged replay vs independent finalized original financial and transaction/block provenance',matchedRows=sum(x['matched'] for x in matched),checkedRows=len(matched)+len(missing),mismatches=mismatches,unverifiedRows=missing,allAddedRowsVerified=not any(x['kind']=='added' for x in missing) and not any(x['kind']=='added' and not x['matched'] for x in matched),allCorrectedRowsVerified=not any(x['kind']=='correction' for x in missing) and not any(x['kind']=='correction' and not x['matched'] for x in matched),perReplay=per,oracleProofs=inputs['proofs'],originalMetadataPath=str(HERE/'original-metadata.json'),originalMetadataSha256=sha(HERE/'original-metadata.json'),comparedFields=sorted(next(iter(matched))['expected']) if matched else [],rows=matched,databaseWrites=False,globalCoverageProven=False)
 assert all(sha(p)==package_hashes[v] for v,p in package_paths.items()),'SPKG bytes changed during verification'
 assert all(sha(pathlib.Path(p['comparatorPath']))==p['comparatorSha256'] for p in per),'sealed comparator changed during verification'
 assert all(sha(BASE/('pluto-combined27-'+label+'.jsonl'))==out['sha256'] for replay in scopes for label,out in replay['outputs'].items()),'complete replay output changed during verification'
 assert all(sha(pathlib.Path(p['path']))==p['sha256'] for p in inputs['proofs']),'independent proof changed during verification'
 report.update(packageArtifacts={v:dict(path=str(p),sha256=package_hashes[v]) for v,p in package_paths.items()},actualPackageBytesVerified=True,verificationScripts={p.name:sha(p) for p in [RESOURCE/'verify-financial.py',RESOURCE/'compare-replays.py',RESOURCE/'original-metadata.cjs',RESOURCE/'solana-type.proto',RESOURCE/'protocol-ids.json',RESOURCE/'extra-proof-families.json']})
 dump(pathlib.Path(args.output),report);print(json.dumps({k:report[k] for k in ['matchedRows','checkedRows','allAddedRowsVerified','allCorrectedRowsVerified']}|dict(unverifiedRows=len(missing),mismatches=len(mismatches),perReplay=per)))
 return 1 if missing or mismatches else 0

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--repo-root',type=pathlib.Path,default=RESOURCE.parents[3]);parser.add_argument('--replay-root',type=pathlib.Path,required=True);parser.add_argument('--node-dependency-package');parser.add_argument('--protobuf-package');parser.add_argument('--baseline-package');parser.add_argument('--final-package');parser.add_argument('--expected-baseline-package-sha256');parser.add_argument('--prepare',action='store_true');parser.add_argument('--inventory',action='store_true');parser.add_argument('--inventory-root',default=None);parser.add_argument('--replay-proof',action='append',default=[]);parser.add_argument('--expected-package-sha256',required=False,default=None);parser.add_argument('--block-cache',default=None);parser.add_argument('--output',default=None);args=parser.parse_args()
 BASE=args.replay_root.resolve();HERE=BASE/'combined27-financial';FIX=args.repo_root.resolve()/'dex-swaps/fixtures/swap-coverage'
 args.inventory_root=args.inventory_root or str(BASE);args.block_cache=args.block_cache or str(HERE/'finalized-blocks');args.output=args.output or str(BASE/'combined27-financial-proof.json')
 if not args.prepare and not args.inventory:assert args.node_dependency_package and args.protobuf_package,'Node dependency/protobuf package paths required'
 if args.prepare:prepare()
 elif args.inventory:inventory(args)
 else:
  if not args.replay_proof:args.replay_proof=[str(BASE/'pluto-combined27-packaged-replay-proof.json'),str(BASE/'pluto-combined27-wide-packaged-replay-proof.json'),str(BASE/'pluto-combined27-live-packaged-replay-proof.json')]
  sys.exit(verify(args))
