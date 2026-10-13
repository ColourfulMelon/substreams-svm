import json,hashlib,re,pathlib,collections,struct
BASE=pathlib.Path(__file__).resolve().parent
WORK=BASE
facts=json.loads((BASE/'facts.json').read_text())
meta={r['signature']:r for r in json.loads((BASE/'original-metadata.json').read_text())['transactions']}
def snake(s): return re.sub(r'(?<!^)(?=[A-Z])','_',s).lower()
def disc(ix): return bytes(ix['discriminator']) if 'discriminator' in ix else hashlib.sha256(('global:'+snake(ix['name'])).encode()).digest()[:8]
def flatten(a,p=''):
 out=[]
 for r in a:
  name=p+r['name']
  if 'accounts' in r: out+=flatten(r['accounts'],name+'.')
  else: out.append(name)
 return out
class Reader:
 def __init__(self,b,idl): self.b=b;self.n=8;self.types={x['name']:x['type'] for x in idl.get('types',[])}
 def take(self,n):
  assert n>=0 and self.n+n<=len(self.b),'argument buffer exhausted';r=self.b[self.n:self.n+n];self.n+=n;return r
 def read(self,t):
  if isinstance(t,str):
   if t in ['u8','i8','u16','i16','u32','i32','u64','i64','u128','i128']: return str(int.from_bytes(self.take(int(t[1:])//8),'little',signed=t.startswith('i')))
   if t=='bool':
    v=self.take(1)[0];assert v in [0,1],'invalid bool';return bool(v)
   if t in ['publicKey','pubkey']:return self.take(32).hex()
   if t in ['string','bytes']:
    n=int.from_bytes(self.take(4),'little');v=self.take(n);return v.decode() if t=='string' else v.hex()
   return self.read(self.types[t])
  if 'defined' in t:return self.read(self.types[t['defined'] if isinstance(t['defined'],str) else t['defined']['name']])
  if 'vec' in t:
   n=int.from_bytes(self.take(4),'little');assert n<=10000,'vector bound';return [self.read(t['vec']) for _ in range(n)]
  if 'array' in t:return [self.read(t['array'][0]) for _ in range(t['array'][1])]
  if 'option' in t:
   present=self.take(1)[0];assert present in [0,1],'invalid option';return self.read(t['option']) if present else None
  if t.get('kind')=='struct':return {f['name']:self.read(f['type']) for f in t['fields']}
  if t.get('kind')=='enum':
   n=self.take(1)[0];v=t['variants'][n];f=v.get('fields',[]);return {'variant':v['name'],'fields':{e['name']:self.read(e['type']) for e in f} if f and isinstance(f[0],dict) and 'name' in f[0] else [self.read(e) for e in f]}
  raise ValueError(str(t))
DBC='dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN'
classifications=[]
for f in facts:
 p=f['program_id'];b=bytes.fromhex(f['native']['data']);idl=json.loads((WORK/'primary'/(p+'.idl.json')).read_text());matches=[ix for ix in idl.get('instructions',[]) if disc(ix)==b[:8]];assert len(matches)<=1
 m=meta[f['signature']];s=f['instruction_index'];assert m['committed'][s] and all(m['committed'][c['index']] for c in f['children']);assert f['indexedWithin']==[]
 row={'signature':f['signature'],'slot':f['block_num'],'source_index':s,'program':p,'tag':b[:8].hex(),'nativeAccounts':f['native']['accounts'],'nativeData':f['native']['data'],'children':f['children'],'indexedAncestors':f['indexedAncestors'],'pbSha256':m['pbSha256'],'rpcSha256':m['rpcSha256'],'rpcLogsTruncated':not m['finalizedRpcRuntimeComplete'],'retainedPBRuntimeComplete':True,'fullInstructionAndBalanceParity':True,'existingIndexedNativeChildren':0}
 if not matches:
  row.update(status='unresolved_primary_abi',reason='Owned IDL empty or lacks this discriminator; runtime label does not establish business semantics')
  if p==DBC:
   row.update(status='primary_source_fee_claim',instruction='claim_trading_fee2',primaryCommit='f552f20aa3c1c7631427c3827aeea7c58b902813')
   assert len(b)==28 and b[24:]==b'\0'*4 and len(row['nativeAccounts'])==14
   a=row['nativeAccounts'];cs=f['children'];trans=[]
   for c in cs:
    cb=bytes.fromhex(c['data'])
    if c['program'] in a[10:12]:
     assert len(cb)==10 and cb[0]==12;expected=[a[5],a[7],a[3],a[0]] if c['program']==a[10] else [a[6],a[8],a[4],a[0]];assert c['accounts']==expected
     trans.append({'index':c['index'],'mint':c['accounts'][1],'source':c['accounts'][0],'destination':c['accounts'][2],'amount':str(int.from_bytes(cb[1:9],'little'))})
   assert len(trans)==2 and len(cs)==3 and cs[-1]['program']==p and cs[-1]['data'].startswith('e445a52e51cb9a1d')
   row['principals']=trans;row['classification']='accrued_partner_fee_withdrawal';row['spotSwap']=False
 else:
  ix=matches[0];reader=Reader(b,idl)
  try:
   args={a['name']:reader.read(a['type']) for a in ix.get('args',[])};assert reader.n==len(b),f'trailing bytes {len(b)-reader.n}'
   roles=flatten(ix['accounts']);assert len(row['nativeAccounts'])>=len(roles),'missing formal roles'
   row.update(status='typed_primary_abi',instruction=ix['name'],arguments=args,formalRoles=dict(zip(roles,row['nativeAccounts'])),remainingAccounts=row['nativeAccounts'][len(roles):],primaryIdlSha256=hashlib.sha256((WORK/'primary'/(p+'.idl.json')).read_bytes()).hexdigest())
  except Exception as e:row.update(status='unresolved_primary_shape',instruction=ix['name'],reason=str(e))
 classifications.append(row)
(BASE/'typed.json').write_text(json.dumps(classifications,indent=2)+'\n')
pass
for r in classifications:
 if False: pass
# Vault receipt issuance/redemption is bound to its underlying vault and LP-share token,
# not a secondary market exchange. Verify both complete token ledgers per source.
fmap={(f['signature'],f['instruction_index']):f for f in facts}
ledger_count=0
for row in classifications:
 if not row['program'].startswith('24Uq'):continue
 f=fmap[row['signature'],row['source_index']];a=row['nativeAccounts'];cs=f['children'];args=row['arguments'];assert len(a)==7 and len(cs)==2 and all(c['program']==a[6] and c['depth']==f['native']['depth']+1 for c in cs)
 def principal(c,opcode,roles):
  b=bytes.fromhex(c['data']);assert len(b)==9 and b[0]==opcode and c['accounts']==roles;return int.from_bytes(b[1:9],'little')
 if row['instruction']=='deposit':
  debit=principal(cs[0],3,[a[3],a[1],a[5]]);shares=principal(cs[1],7,[a[2],a[4],a[0]]);assert debit==int(args['tokenAmount']) and shares>=int(args['minimumLpTokenAmount']);kind='liquidity_share_issuance'
 else:
  debit=principal(cs[0],3,[a[1],a[3],a[0]]);shares=principal(cs[1],8,[a[4],a[2],a[5]]);assert shares==int(args['unmintAmount']) and debit>=int(args['minOutAmount']);kind='liquidity_share_redemption'
 row.update(classification=kind,spotSwap=False,underlyingAmount=str(debit),shareAmount=str(shares))
 balances={(v['side'],v['address']):v for v in f['balances']};m=meta[row['signature']];ledgers=[]
 for addr in [a[1],a[4]]:
  pre=balances.get(('pre',addr));post=balances.get(('post',addr));assert pre or post
  mint=(pre or post)['mint'];delta=0;flows=[]
  for ix in m['instructions']:
   if ix['program']!=a[6] or addr not in ix['accounts']:continue
   b=bytes.fromhex(ix['data']);op=b[0];roles=ix['accounts'];assert m['committed'][ix['index']]
   if op in [3,12]:
    assert len(b)==(9 if op==3 else 10);src,dst=(roles[0],roles[1]) if op==3 else(roles[0],roles[2]);amount=int.from_bytes(b[1:9],'little')
    if addr in [src,dst]:delta+=(amount if dst==addr else 0)-(amount if src==addr else 0);flows.append({'source_index':ix['index'],'debit':src,'credit':dst,'amount':str(amount)})
   elif op in [7,14] and roles[1]==addr:
    assert roles[0]==mint;amount=int.from_bytes(b[1:9],'little');delta+=amount;flows.append({'source_index':ix['index'],'mintTo':str(amount)})
   elif op in [8,15] and roles[0]==addr:
    assert roles[1]==mint;amount=int.from_bytes(b[1:9],'little');delta-=amount;flows.append({'source_index':ix['index'],'burn':str(amount)})
   else:assert op in [7,8,9,14,15,18,22],f'unsupported protected vault/share operation {op}'
  before=int(pre['amount']) if pre else 0;after=int(post['amount'])if post else 0;assert after-before==delta,f'whole receipt ledger {addr}: {after-before} vs {delta}'
  assert all(v['mint']==mint and v['program']==a[6] for v in [pre,post] if v)
  ledgers.append({'address':addr,'mint':mint,'before':str(before),'after':str(after),'delta':str(delta),'flows':flows});ledger_count+=1
 row['wholeTokenLedgers']=ledgers
(BASE/'typed.json').write_text(json.dumps(classifications,indent=2)+'\n')
print('vault sources',sum(r['program'].startswith('24Uq')for r in classifications),'whole protected vault/share token ledgers',ledger_count)
category={
'initialize_virtual_pool_with_token2022':'pool_and_token_initialization',
'initialize_with_token_2022':'pool_and_token_initialization',
'withdrawObligationCollateralAndRedeemReserveCollateralV2':'lending_collateral_redemption',
'redeemReserveCollateral':'lending_collateral_redemption',
'depositReserveLiquidity':'lending_collateral_issuance',
'depositReserveLiquidityAndObligationCollateralV2':'lending_collateral_issuance',
'depositReserveLiquidityAndObligationCollateral':'lending_collateral_issuance',
'repayAndWithdrawAndRedeem':'lending_debt_repayment_and_collateral_redemption',
'addLiquidity2':'liquidity_share_issuance','removeLiquidity2':'liquidity_share_redemption',
'shared_accounts_route_v2':'routed_lending_receipt_redemption',
'wrap':'token_wrapper_issuance','unwrap':'token_wrapper_redemption',
'deposit_user_vault':'vault_receipt_issuance','withdraw':'vault_receipt_redemption',
'bridge_spl':'cross_chain_token_custody','create_order_v2':'perpetual_order_creation',
'open_limit_order':'limit_order_nft_creation','mint_with_usdc':'attested_issuer_token_mint',
}
tokenPrograms={'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA','TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb'}
for row in classifications:
 if row['status']!='typed_primary_abi' or row['program'].startswith('24Uq'):continue
 assert row['instruction'] in category
 principals=[]
 for c in row['children']:
  if c['program'] not in tokenPrograms:continue
  b=bytes.fromhex(c['data']);op=b[0];a=c['accounts'];v=int.from_bytes(b[1:9],'little') if len(b)>=9 else 0
  if op in [3,12]:
   assert len(b)==(9 if op==3 else 10);principals.append({'index':c['index'],'operation':'transfer','source':a[0],'destination':a[1 if op==3 else 2],'mint':None if op==3 else a[1],'amount':str(v)})
  elif op in [7,14,8,15]:
   assert len(b)==(9 if op in [7,8]else 10);principals.append({'index':c['index'],'operation':'mint' if op in [7,14]else'burn','mint':a[0 if op in [7,14]else 1],'tokenAccount':a[1 if op in [7,14]else 0],'amount':str(v)})
 row.update(classification=category[row['instruction']],spotSwap=False,principals=principals)
 if row['instruction']=='shared_accounts_route_v2':
  plan=row['arguments']['route_plan'];assert len(plan)==1 and plan[0]['swap']=={'variant':'JupiterLendRedeem','fields':[]} and plan[0]['bps']=='10000'
  assert [v['operation']for v in principals]==['transfer','burn','transfer','transfer']
  assert principals[0]['amount']==principals[1]['amount']==row['arguments']['in_amount']
  assert principals[2]['amount']==principals[3]['amount']
  assert principals[1]['mint']==row['formalRoles']['source_mint'] and principals[2]['mint']==row['formalRoles']['destination_mint']
(BASE/'typed.json').write_text(json.dumps(classifications,indent=2)+'\n')
print('categories',collections.Counter(r.get('classification',r['status'])for r in classifications))
# Deployed stock-facility dispatch proves issuer external-share redemption despite its
# deliberately empty owned IDL. Do not interpret the internal receipt as a DEX quote.
for row in classifications:
 if not row['program'].startswith('FewR'):continue
 f=fmap[row['signature'],row['source_index']];a=row['nativeAccounts'];cs=row['children'];b=bytes.fromhex(row['nativeData']);assert len(b)==16 and len(a)==16 and len(cs)==4
 assert all(c['depth']==f['native']['depth']+1 for c in cs)
 burn,first,extension,second=cs;assert burn['program']==a[13]and burn['accounts']==[a[10],a[6],a[3]] and bytes.fromhex(burn['data'])[0]==15
 request=int.from_bytes(b[8:16],'little');assert int.from_bytes(bytes.fromhex(burn['data'])[1:9],'little')==request
 assert first['program']==second['program']==a[12] and first['accounts']==[a[8],a[5],a[9],a[8]] and second['accounts']==[a[9],a[5],a[11],a[3]]
 assert bytes.fromhex(first['data'])[0]==bytes.fromhex(second['data'])[0]==12 and first['data']==second['data']
 assert extension['program']==a[13]and extension['accounts']==[a[6],a[7]] and extension['data']=='2b01000000000000f03f0000000000000000'
 balances={(v['side'],v['address']):v for v in f['balances']};ledgers=[]
 for address in set([a[8],a[9],a[11],a[10]]):
  pre=balances.get(('pre',address));post=balances.get(('post',address));assert pre and post and pre['mint']==post['mint'] and pre['owner']==post['owner'] and pre['program']==post['program'];delta=0;flows=[]
  for c in meta[row['signature']]['instructions']:
   if c['program']not in tokenPrograms:continue
   d=bytes.fromhex(c['data']);op=d[0];r=c['accounts']
   if op==12 and address in[r[0],r[2]]:
    amount=int.from_bytes(d[1:9],'little');assert r[1]==pre['mint'];delta+=(amount if r[2]==address else 0)-(amount if r[0]==address else 0);flows.append({'index':c['index'],'debit':r[0],'credit':r[2],'amount':str(amount)})
   elif op==15 and r[0]==address:
    amount=int.from_bytes(d[1:9],'little');assert r[1]==pre['mint'];delta-=amount;flows.append({'index':c['index'],'burn':str(amount)})
   elif address in r:assert op in[12,15],f'unaccounted stock-token operation {op}'
  assert int(post['amount'])-int(pre['amount'])==delta
  ledgers.append({'address':address,'mint':pre['mint'],'before':pre['amount'],'after':post['amount'],'delta':str(delta),'flows':flows})
 row.update(status='primary_owned_executable_receipt_redemption',classification='issuer_external_share_redemption',spotSwap=False,request=str(request),internalReceiptAmount=str(int.from_bytes(bytes.fromhex(first['data'])[1:9],'little')),wholeTokenLedgers=ledgers,primaryDispatch={'elfSha256':'856c4a0379c78f343a0bfbc3b083a8b94dad4247c585373c1e1b038055d840ca','deployedSlot':425480485,'discriminatorComparePc':37219,'branchPc':37221,'handlerPc':40451,'argumentU64ReadPc':40463,'primaryLogVirtualAddress':'0x89b97','primaryLog':'Instruction: SupplyRedeemAndTransferExternal','sourcePath':'programs/stock_facility/src/state.rs','scopeLimitation':'No IDL account role names or historical configured exchange ratio claimed; exact original mint burn and receipt transfers bind observed roles and amounts.'})
(BASE/'typed.json').write_text(json.dumps(classifications,indent=2)+'\n')
print('final statuses',collections.Counter(r['status']for r in classifications))
for row in classifications:
 if row['status']!='unresolved_primary_shape':continue
 f=fmap[row['signature'],row['source_index']];a=row['nativeAccounts'];p=row['program'];idl=json.loads((WORK/'primary'/(p+'.idl.json')).read_text());ix=next(ix for ix in idl['instructions']if disc(ix)==bytes.fromhex(row['nativeData'])[:8]);roles=flatten(ix['accounts']);assert len(a)>=len(roles)
 row['formalRoles']=dict(zip(roles,a));row['primaryIdlSha256']=hashlib.sha256((WORK/'primary'/(p+'.idl.json')).read_bytes()).hexdigest()
 if p.startswith('UMBR'):
  assert len(a)==17 and len(bytes.fromhex(row['nativeData']))==48 and len(row['children'])==2
  sys,transfer=row['children'];assert sys['program']==a[15] and sys['accounts']==[a[0],a[7]]and len(bytes.fromhex(sys['data']))==12
  assert transfer['program']==a[13]and transfer['accounts']==[a[3],a[10],a[8],a[1]]and len(bytes.fromhex(transfer['data']))==10 and transfer['data'].startswith('0c')
  amount=int.from_bytes(bytes.fromhex(transfer['data'])[1:9],'little');assert amount==int.from_bytes(bytes.fromhex(row['nativeData'])[40:48],'little')
  ledgers=[];balances={(v['side'],v['address']):v for v in f['balances']}
  for addr,sign in[(a[3],-1),(a[8],1)]:
   pre=balances.get(('pre',addr));post=balances.get(('post',addr));assert pre and post and pre['mint']==post['mint']==a[10] and int(post['amount'])-int(pre['amount'])==sign*amount
   relevant=[c for c in meta[row['signature']]['instructions']if c['program']==a[13]and addr in c['accounts']];assert relevant==[transfer]
   ledgers.append({'address':addr,'mint':a[10],'before':pre['amount'],'after':post['amount'],'delta':str(sign*amount),'transferIndex':transfer['index']})
  row.update(status='primary_owned_executable_custody_deposit',classification='privacy_pool_single_asset_custody_deposit',spotSwap=False,amount=str(amount),wholeTokenLedgers=ledgers,primaryDispatch={'elfSha256':'44bc4af7c81ed47bf97bd39dab3d307f97c5e8589cb3d5fc6901bea5c6422df8','deployedSlot':451847033,'discriminatorComparePc':763920,'branchPc':763922,'handlerPc':802930,'argumentReadPc':[802971,802973,802974,802975,802976],'argumentBytes':40,'scopeLimitation':'Deployed binary consumes two128-bit fields and one64-bit amount; later owned IDL lists a different fee-proof argument form. No unavailable field semantics or fee-proof reconstruction claimed.'})
 else:
  reader=Reader(bytes.fromhex(row['nativeData']),idl);args={arg['name']:reader.read(arg['type']) for arg in ix['args']};suffix=bytes.fromhex(row['nativeData'])[reader.n:];assert len(suffix)==10
  row.update(status='typed_primary_prefix_with_retained_suffix',arguments=args,retainedUndeclaredSuffix=suffix.hex(),scopeLimitation='Owned formal instruction/arguments and account roles bind the business action; the10-byte suffix is retained verbatim and not assigned invented semantics.')
  if p.startswith('Lan'):
   mint=row['formalRoles']['base_mint'];vault=row['formalRoles']['base_vault'];mintto=[c for c in row['children']if c['program']==row['formalRoles']['base_token_program']and c['data'].startswith('07')]
   assert len(mintto)==1 and mintto[0]['accounts'][:2]==[mint,vault]
   assert not any(c['program']in tokenPrograms and c['data'].startswith(('03','0c','08','0f')) for c in row['children'])
   row.update(classification='pool_and_token_initialization',spotSwap=False,mintedInitialSupply=str(int.from_bytes(bytes.fromhex(mintto[0]['data'])[1:9],'little')))
  elif p.startswith('strm'):
   transfers=[c for c in row['children']if c['program']==row['formalRoles']['tokenProgram']and c['data'].startswith('0c')];assert len(transfers)==1 and transfers[0]['accounts']==[row['formalRoles']['senderTokens'],row['formalRoles']['mint'],row['formalRoles']['escrowTokens'],row['formalRoles']['sender']]
   amount=int.from_bytes(bytes.fromhex(transfers[0]['data'])[1:9],'little');assert amount>=int(args['netAmountDeposited']);row.update(classification='scheduled_stream_token_escrow_funding',spotSwap=False,grossEscrowFunding=str(amount),netStreamCommitment=args['netAmountDeposited'])
  else:raise ValueError(p)
(BASE/'typed.json').write_text(json.dumps(classifications,indent=2)+'\n')
print('completed classification',collections.Counter(r['status']for r in classifications));assert all(r.get('spotSwap')is False for r in classifications)

# Verify owned executable hashes and typed dispatch/read PCs from the actual text.
for prog,prefix,want,compare,handler,reads,logaddr,logtext in [
 ('FewRXXTzV2WwigvTQNsVanhEmCM9wzfNvoT1divNtbVH','FewR','856c4a0379c78f343a0bfbc3b083a8b94dad4247c585373c1e1b038055d840ca',37219,40451,[40463],0x89b97,b'Instruction: SupplyRedeemAndTransferExternal'),
 ('UMBRAD2ishebJTcgCLkTkNUx1v3GyoAgpTRPeWoLykh','UMBR','44bc4af7c81ed47bf97bd39dab3d307f97c5e8589cb3d5fc6901bea5c6422df8',763920,802930,[802971,802973,802974,802975,802976],0x69be5b,b'Instruction: DepositIntoStealthPoolFromPublicBalance')]:
 elf=(WORK/'primary'/(prog+'.elf')).read_bytes();assert hashlib.sha256(elf).hexdigest()==want
 h=struct.unpack_from('<16sHHIQQQIHHHHHH',elf);sections=[struct.unpack_from('<IIQQQQIIQQ',elf,h[6]+i*h[11])for i in range(h[12])];names=elf[sections[h[13]][4]:sections[h[13]][4]+sections[h[13]][5]];text=next(q for q in sections if names[q[0]:].split(b'\0')[0]==b'.text');code=elf[text[4]:text[4]+text[5]]
 tag=bytes.fromhex(next(r['tag']for r in classifications if r['program']==prog));opcode,reg,offset,lo=struct.unpack_from('<BBhi',code,compare*8);hi=struct.unpack_from('<BBhi',code,(compare+1)*8)[3];assert opcode==0x18 and struct.pack('<II',lo&0xffffffff,hi&0xffffffff)==tag
 section=next(q for q in sections if q[3]<=logaddr<q[3]+q[5]);fileoff=section[4]+logaddr-section[3];assert elf[fileoff:fileoff+len(logtext)]==logtext
 for pc in reads:assert struct.unpack_from('<BBhi',code,pc*8)[0]==0x79
