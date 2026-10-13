const resolvePackage=require('module').createRequire;
const packageMode=process.argv[2]?.endsWith('package.json');
const packageRequire=packageMode?resolvePackage(require('path').resolve(process.argv[2])):require;
const protobufRequire=packageMode&&process.argv[3]?resolvePackage(require('path').resolve(process.argv[3])):require;
const fs=require('fs'),crypto=require('crypto'),z=require('zlib');
const bs=packageRequire('bs58').default||packageRequire('bs58');
const pb=protobufRequire('protobufjs');
const T=pb.parse(fs.readFileSync(require('path').join(__dirname,'solana-type.proto'),'utf8')).root.lookupType('sf.solana.type.v1.ConfirmedTransaction');
const dir=packageMode?__dirname:process.argv[2]||__dirname,amm='pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA',token='TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',token22='TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',fee='pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ',sol='So11111111111111111111111111111111111111112',system='11111111111111111111111111111111';
const assert=(p,msg)=>{if(!p)throw Error(msg)},hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex'),u=(b,o)=>b.readBigUInt64LE(o);
const {PublicKey}=packageRequire('@solana/web3.js'); const curve='6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P';
const eqLogs=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
const rows=JSON.parse(fs.readFileSync(dir+'/sources.json'));const proof=[], failures=[];const partition=JSON.parse(fs.readFileSync(dir+'/partition.json'));assert(rows.length===244&&new Set(rows.map(r=>r.signature+'|'+r.instruction_index)).size===244,'complete unique fixed source corpus');for(const group of partition)assert(rows.filter(r=>r.program_id===group.program&&r.instruction_tag===group.tag).length===group.noIndexedWithin,'all assigned group sources retained');const payout=JSON.parse(fs.readFileSync(dir+'/satrush-payout-authority.json'));assert(payout.account.owner==='satRushGBRY2vgapeTAkoxz26vL2cYqyPi6CnBj7Tco'&&!payout.account.executable,'independently program-owned native settlement authority');

const ownedPrograms=JSON.parse(fs.readFileSync(dir+'/owned-program-proof.json'));
for(const pin of ownedPrograms){
 const program=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+pin.program+'.program.json.gz'))),pd=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+pin.program+'.programdata.json.gz'))),loader='BPFLoaderUpgradeab1e11111111111111111111111';
 const p=Buffer.from(program.value.data[0],'base64'),d=Buffer.from(pd.value.data[0],'base64'),elf=fs.readFileSync(dir+'/'+pin.program+'.elf');
 assert(program.value.owner===loader&&program.value.executable&&p.length===36&&p.readUInt32LE()===2&&bs.encode(p.subarray(4))===pin.programData,'owned executable loader ProgramData binding');
 assert(pd.value.owner===loader&&!pd.value.executable&&d.readUInt32LE()===3&&u(d,4)===BigInt(pin.deployedSlot)&&[0,1].includes(d[12])&&d.subarray(45).equals(elf)&&hash(dir+'/'+pin.program+'.elf')===pin.elfSha256,'exact owned deployed ELF/slot/bytes');
 assert(rows.filter(r=>r.program_id===pin.program).every(r=>BigInt(r.block_num)>u(d,4)),'deployment predates finalized original');
}

for(const row of rows){try{
 const sig=row.signature,raw=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+sig+'.rpc.json.gz')), (k,v,c)=>typeof v==='number'&&Number.isInteger(v)&&!Number.isSafeInteger(v)?c.source:v);const tx=raw.result;
 assert(tx.slot===row.block_num&&tx.meta.err===null,'finalized original slot/success');
 const keys=tx.transaction.message.accountKeys.concat(tx.meta.loadedAddresses.writable,tx.meta.loadedAddresses.readonly),groups=new Map(tx.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const ix=tx.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(groups.get(i)||[])]).map((x,index)=>({index,depth:x.stackHeight,program:keys[x.programIdIndex],accounts:x.accounts.map(i=>keys[i]),data:Buffer.from(bs.decode(x.data))}));
 const original=T.decode(fs.readFileSync(dir+'/'+sig+'.pb')),pk=original.transaction.message.accountKeys.concat(original.meta.loadedWritableAddresses,original.meta.loadedReadonlyAddresses).map(bs.encode),pg=new Map(original.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const pIx=original.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(pg.get(i)||[])]);
 assert(JSON.stringify(keys)===JSON.stringify(pk)&&pIx.length===ix.length,'PB RPC full keys/walk parity');
 pIx.forEach((p,i)=>assert(pk[p.programIdIndex]===ix[i].program&&p.stackHeight===ix[i].depth&&Buffer.from(p.data).equals(ix[i].data)&&JSON.stringify(Array.from(p.accounts||[]).map(n=>pk[n]))===JSON.stringify(ix[i].accounts),'PB RPC exact instruction parity'));
 assert(JSON.stringify(original.meta.preBalances.map(String))===JSON.stringify(tx.meta.preBalances.map(String))&&JSON.stringify(original.meta.postBalances.map(String))===JSON.stringify(tx.meta.postBalances.map(String)),'PB RPC all native balances');
 const trunc=tx.meta.logMessages.indexOf('Log truncated');assert(trunc<0?eqLogs(original.meta.logMessages,tx.meta.logMessages):trunc===tx.meta.logMessages.length-1&&eqLogs(original.meta.logMessages.slice(0,trunc),tx.meta.logMessages.slice(0,trunc)),'PB RPC complete runtime or exact retained truncation prefix');assert(String(original.meta.fee)===String(tx.meta.fee),'PB RPC exact fee parity');
 const tokenView=t=>({accountIndex:t.accountIndex,mint:t.mint,owner:t.owner,programId:t.programId,amount:String(t.uiTokenAmount.amount),decimals:t.uiTokenAmount.decimals});
 for(const side of ['preTokenBalances','postTokenBalances'])assert(JSON.stringify(original.meta[side].map(tokenView))===JSON.stringify(tx.meta[side].map(tokenView)),'PB RPC all token balance parity');
 assert(bs.encode(original.transaction.signatures[0])===sig&&tx.transaction.signatures[0]===sig,'original source and finalized RPC signature');

 const stack=[],returns=new Map();let cursor=0;
 for(const l of original.meta.logMessages){assert(l!=='Log truncated','complete logs');const w=l.split(' ');if(w[0]!=='Program')continue;
  if(w[2]==='invoke'){while(ix[cursor]&&ix[cursor].program!==w[1]&&['ComputeBudget111111111111111111111111111111','Ed25519SigVerify111111111111111111111111111','KeccakSecp256k11111111111111111111111111111'].includes(ix[cursor].program)&&ix[cursor].depth===1)cursor++;assert(ix[cursor].program===w[1]&&ix[cursor].depth===stack.length+1&&Number(w[3].slice(1,-1))===ix[cursor].depth,'native runtime source/depth');stack.push({source:cursor++,returned:[]});}
  else if(w[1]==='return:'){assert(stack.length&&ix[stack.at(-1).source].program===w[2],'scoped return provenance');stack.at(-1).returned.push(Buffer.from(w[3],'base64'));}
  else if(w[2]==='success'||w[2]==='failed:'){const f=stack.pop();assert(f&&ix[f.source].program===w[1]&&w[2]==='success','committed full ancestry');if(ix[f.source].program===fee){assert(stack.length&&[amm,'6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P'].includes(ix[stack.at(-1).source].program)&&f.returned.length===1&&f.returned[0].length===24,'native fee return');returns.set(f.source,[u(f.returned[0],0),u(f.returned[0],8),u(f.returned[0],16)]);}}
 }
 while(ix[cursor]&&['ComputeBudget111111111111111111111111111111','Ed25519SigVerify111111111111111111111111111','KeccakSecp256k11111111111111111111111111111'].includes(ix[cursor].program)&&ix[cursor].depth===1)cursor++;assert(cursor===ix.length&&!stack.length,'complete invocation witness');

 const source=ix[row.instruction_index],a=source.accounts,b=source.data,end=ix.find(c=>c.index>source.index&&c.depth<=source.depth)?.index||ix.length,calls=ix.slice(source.index+1,end);
 assert(source.program===row.program_id&&b.subarray(0,8).toString('hex')===row.instruction_tag,'exact original source identity');
 const isToken=c=>[token,token22].includes(c.program),typed=c=>({index:c.index,depth:c.depth,program:c.program,opcode:c.data[0],data:c.data.toString('hex'),roles:c.accounts.map(k=>a.indexOf(k)),accounts:c.accounts}),operations=calls.filter(c=>isToken(c)&&c.depth===source.depth+1),ops=operations.map(c=>c.data[0]),pre=new Map(tx.meta.preTokenBalances.map(t=>[keys[t.accountIndex],t])),post=new Map(tx.meta.postTokenBalances.map(t=>[keys[t.accountIndex],t]));
 const eq=(v,w)=>JSON.stringify(v)===JSON.stringify(w),role=(c,rs)=>assert(eq(c.accounts,rs.map(n=>a[n])),'bound exact principal roles '+c.index),shape=(c,n,op)=>assert(isToken(c)&&c.data[0]===op&&c.data.length===n,'exact token framing'),amount=c=>u(c.data,1),held=(key,mint,owner)=>{const t=pre.get(key)||post.get(key);assert(t&&t.mint===mint&&t.owner===owner,'source asset ownership');return t;};
 let classification;
 if(source.program.startsWith('satRush')){
  if(row.instruction_tag==='f96f7a675d564cc6'){
   assert(a[3]===payout.address,'owned settlement authority source binding');assert(b.length===8&&a.length===26&&calls.length>=3&&calls.every(c=>c.depth===source.depth+1),'settlement source/children framing');
   assert(operations.length>=2&&operations.length<=3&&ops.every(op=>op===12),'settlement only typed outgoing payouts');
   for(const c of operations){shape(c,10,12);assert(c.accounts[3]===a[3],'one program payout authority');held(c.accounts[0],c.accounts[1],a[3]);assert((pre.get(c.accounts[2])||post.get(c.accounts[2])).owner!==a[3],'distinct payout receiver');}
   assert(calls.at(-1).program===source.program&&calls.at(-1).data.subarray(0,16).toString('hex')==='e445a52e51cb9a1d7b158d226ccc68d6','source native settlement event');classification='one_way_game_settlement_payouts';
  }else{
   assert(b.length===8&&a.length===30&&operations.length===4&&ops.every(op=>op===12),'round rotation redistribution');
   assert(calls[0].program===system&&calls[0].data.readUInt32LE()===0&&bs.encode(calls[0].data.subarray(20))===source.program,'new native round state');
   assert(calls.some(c=>isToken(c)&&c.data[0]===7),'round reward issuance');for(const c of operations){shape(c,10,12);assert(c.accounts[3]===a[2],'one round redistribution authority');held(c.accounts[0],c.accounts[1],a[2]);}assert(calls[0].data.length===52&&eq(calls[0].accounts,[a[0],a[4]]),'caller creates native round state only');
   // MintTo is nested inside the owned sAtmiNt program; all direct token operations are payout transfers.
   classification='round_reward_issuance_and_redistribution';
  }
 }else if(source.program.startsWith('BURN')){
  assert(b.length===16&&operations.length%2===0&&operations.length>0,'incinerator bounded body/pairs');
  for(let n=0;n<operations.length;n+=2){const burn=operations[n],close=operations[n+1];shape(burn,9,8);shape(close,1,9);assert(burn.accounts[2]===a[0]&&eq(close.accounts,[burn.accounts[0],a[0],a[0]]),'owner burns then closes own account');assert(amount(burn)===BigInt(pre.get(burn.accounts[0]).uiTokenAmount.amount),'destroy complete original balance');}
  assert(calls.filter(c=>c.program===system).every(c=>c.data.length===12&&c.data.readUInt32LE()===2&&c.accounts[0]===a[0]),'only owner pays native fees');classification='token_destruction_and_rent_reclamation';
 }else if(source.program.startsWith('prediCt')){
  assert(b.length===16,'prediction amount ABI');
  if(row.instruction_tag==='7cbd1b2bd8289342'){
   assert([11,12].includes(a.length)&&eq(ops,[12,7,7]),'split checked collateral/two issued outcomes');role(operations[0],[5,2,6,0]);role(operations[1],[3,7,1]);role(operations[2],[4,8,1]);assert(operations.every(c=>amount(c)===u(b,8)),'equal collateral and binary issuance');held(a[5],a[2],a[0]);held(a[7],a[3],a[0]);held(a[8],a[4],a[0]);classification='binary_outcome_split_issuance';
  }else if(row.instruction_tag==='948dec2fae7e456f'){
   assert(a.length===11&&eq(ops,[8,8,12]),'merge two burnt outcomes into backing');role(operations[0],[7,3,0]);role(operations[1],[8,4,0]);role(operations[2],[6,2,5,1]);assert(operations.every(c=>amount(c)===u(b,8)),'equal binary burn and backing redemption');classification='binary_outcome_merge_redemption';
  }else{
   assert(row.instruction_tag==='84e76d14c5acb420'&&a.length===11&&eq(ops,[8,12]),'determined outcome redemption');role(operations[0],[4,3,0]);role(operations[1],[6,2,5,1]);assert(amount(operations[0])===u(b,8)&&amount(operations[1])<=amount(operations[0]),'burn outcome and bounded collateral redemption');classification='determined_outcome_redemption';
  }
 }else if(source.program.startsWith('EMBER')){
  const deposit=row.instruction_tag==='f223c68952e1f2b6';assert(a.length===8&&(deposit?b.length===16:(b.length===9&&b[8]===0)||(b.length===17&&b[8]===1)),'Ember bounded wrapper optional amount ABI');
  if(deposit){assert(eq(ops,[3,7]),'wrapper deposit underlying+issued receipt');role(operations[0],[4,6,0]);role(operations[1],[3,5,1]);assert(operations.every(c=>amount(c)===u(b,8)),'one-for-one wrapped issuance');classification='one_to_one_wrapper_deposit';}
  else{assert(eq(ops,[4,8,3]),'wrapper delegated burn and underlying return');role(operations[0],[5,1,0]);role(operations[1],[5,3,1]);role(operations[2],[6,4,1]);let live=BigInt(pre.get(a[5])?.uiTokenAmount.amount||0);for(const c of ix.slice(0,source.index)){if(c.program!==token||!c.accounts.includes(a[5]))continue;const op=c.data[0];if([3,12].includes(op)){const dst=c.accounts[op===12?2:1];if(c.accounts[0]===a[5])live-=amount(c);if(dst===a[5])live+=amount(c);}else if([7,14].includes(op)&&c.accounts[1]===a[5])live+=amount(c);else if([8,15].includes(op)&&c.accounts[0]===a[5])live-=amount(c);else assert([1,4,5,6,16,18,21,22].includes(op),'unproved preceding wrapper balance operation');}const withdrawn=b[8]===1?u(b,9):live;assert(operations.every(c=>amount(c)===withdrawn),'exact optional amount or full receipt balance and one-for-one redemption');classification='one_to_one_wrapper_redemption';}
 }else if(source.program.startsWith('Dbhg')){
  assert(b.length===16&&a.length===11&&eq(ops,[8,12]),'primary unstable vault Unwrap');role(operations[0],[8,4,0]);role(operations[1],[5,3,7,1]);assert(amount(operations[0])===u(b,8),'declared receipt shares burnt');classification='vault_receipt_unwrap_redemption';
 }else if(['SPMB','SP12','SPoo'].some(p=>source.program.startsWith(p))){
  assert(b.length===9&&b[0]===14&&a.length===10&&eq(ops,[7]),'SPL StakePool DepositSol receipt issuance');const funding=calls[0];assert(funding.program===system&&funding.data.length===12&&funding.data.readUInt32LE()===2&&u(funding.data,4)===u(b,1)&&eq(funding.accounts,[a[3],a[2]]),'native reserve deposit exact declared amount');role(operations[0],[7,4,1]);classification='native_stake_pool_deposit_issuance';
 }else if(source.program.startsWith('stkitr')){
  assert(b.length===9&&b[0]===0&&a.length===15,'bridge stake native amount');const nested=calls.find(c=>['SPMB','SP12','SPoo'].some(p=>c.program.startsWith(p)));assert(nested&&nested.data.length===9&&nested.data[0]===14&&u(nested.data,1)===u(b,1),'bounded nested StakePool DepositSol');const direct=operations.filter(c=>c.depth===source.depth+1);assert(eq(direct.map(c=>c.data[0]),[18,12,9,12]),'exact temporary WSOL unwrap and issued receipt delivery');role(direct[1],[1,7,3,0]);role(direct[3],[5,6,2,5]);assert(amount(direct[1])===u(b,1),'exact quote unwrap funding');classification='wrapped_sol_to_stake_pool_receipt_issuance';
 }else if(source.program.startsWith('StaKE')){
  assert(b.length===9&&b[0]===0&&a.length===8&&eq(ops,[3,7]),'primary Stake underlying/receipt');role(operations[0],[2,1,0]);role(operations[1],[4,3,5]);assert(amount(operations[0])===u(b,1),'declared ORCA stake input');classification='stake_receipt_issuance';
 }else if(source.program.startsWith('JrXL')){
  assert(b.length===25&&a.length===20&&eq(ops,[3,7]),'RLP Deposit receipt issuance');role(operations[0],[8,9,0]);role(operations[1],[4,5,3]);assert(amount(operations[0])===u(b,9),'declared reserve deposit');classification='reserve_lp_receipt_issuance';
 }else if(source.program.startsWith('STkEA')){
  assert(b.length===25&&b[0]===10&&a.length===12&&eq(ops,[3]),'ORE stake withdraw source');role(operations[0],[3,5,0,0]);assert(calls.filter(c=>c.program===system).every(c=>c.data.length===12&&u(c.data,4)===0n),'zero native movement in staking source');classification=amount(operations[0])===0n?'zero_principal_staking_source':'one_way_staking_principal_deposit';
 }else if(source.program.startsWith('LStw')){
  assert(b.length===9&&b[0]===3&&a.length===17,'ORE wrapped staking receipt');const direct=operations.filter(c=>c.depth===source.depth+1);assert(eq(direct.map(c=>c.data[0]),[3,7]),'ORE deposited underlying/minted wrapper');role(direct[0],[2,11,0,0]);role(direct[1],[5,3,10,10]);assert(amount(direct[0])===u(b,1),'declared ORE underlying deposit');classification='staked_ore_wrapper_issuance';
 }else if(source.program.startsWith('FqGg')){
  assert(b.length===10&&eq(ops,[12,12]),'two-token custody withdrawal');role(operations[0],[10,8,9,1,1]);role(operations[1],[14,12,13,1]);assert(operations.every(c=>(pre.get(c.accounts[0])||post.get(c.accounts[0])).owner===a[1]),'same source custody authority');classification='same_authority_custody_asset_withdrawal';
 }else if(source.program.startsWith('BotG')){
  assert(b.toString('hex')==='0b01020000'&&a.length===11&&eq(ops,[9,12,9]),'close empty token accounts and withdraw existing funds');role(operations[0],[5,1,0]);role(operations[1],[9,7,10,0]);role(operations[2],[9,1,0]);assert(BigInt(pre.get(a[5]).uiTokenAmount.amount)===0n&&BigInt(pre.get(a[9]).uiTokenAmount.amount)===amount(operations[1]),'only existing owner USDC withdrawn');classification='wallet_token_withdrawal_and_empty_account_closure';
 }else if(source.program.startsWith('4CcP')){
  assert(b.length===8&&a.length===16&&eq(ops,[7]),'attention round minted reward');role(operations[0],[7,12,8]);assert(calls.at(-1).program===system&&calls.at(-1).data.readUInt32LE()===2,'native round payout');classification='round_reward_issuance_and_native_payout';
 }else if(source.program.startsWith('bentoreo')){
  assert(b.length===9&&b[0]===30&&[42,52].includes(Number(u(b,1)))&&a.length===19&&eq(ops,[7]),'Bento bounded reward mint');role(operations[0],[7,8,1]);assert(calls.at(-1).program===system&&eq(calls.at(-1).accounts,[a[4],a[6]]),'SOL directed to native custody reserve');held(a[8],a[7],a[6]);classification='custody_token_issuance_and_native_deposit';
 }else throw Error('unclassified family');
 // Validate every operation affecting this source's token principals over the entire original transaction.
 const affected=new Set();for(const c of operations){if([3,12].includes(c.data[0])){affected.add(c.accounts[0]);affected.add(c.accounts[c.data[0]===12?2:1]);}else if([7,14].includes(c.data[0]))affected.add(c.accounts[1]);else if([8,15,9].includes(c.data[0]))affected.add(c.accounts[0]);}
 const delta=new Map(),nativeDelta=new Map(),mints=new Map([...pre,...post].map(([k,t])=>[k,t.mint])),rent=new Map(),add=(m,k,n)=>m.set(k,(m.get(k)||0n)+n);
 for(const c of ix){const d=c.data,ac=c.accounts;
  if(c.program===system&&d.length>=4){let op=d.readUInt32LE();if(op===2||op===0){let n=u(d,4);add(nativeDelta,ac[0],-n);add(nativeDelta,ac[1],n);if(op===0)rent.set(ac[1],n);}else if(op===3){const o=44+Number(u(d,36)),n=u(d,o);add(nativeDelta,ac[0],-n);add(nativeDelta,ac[1],n);rent.set(ac[1],n);}}
  if(!isToken(c))continue;
  if([1,16,18].includes(d[0])){mints.set(ac[0],ac[1]);continue;}
  if(!ac.some(k=>affected.has(k)))continue;
  if([3,12].includes(d[0])){assert(d.length===(d[0]===3?9:10),'global transfer exact ABI');const dst=ac[d[0]===12?2:1],n=u(d,1);assert(mints.get(ac[0])===mints.get(dst),'global same-mint transfer');add(delta,ac[0],-n);add(delta,dst,n);if(mints.get(ac[0])===sol){add(nativeDelta,ac[0],-n);add(nativeDelta,dst,n);}}
  else if([7,14].includes(d[0])){assert(d.length===(d[0]===7?9:10),'MintTo exact ABI');assert(mints.get(ac[1])===ac[0],'MintTo exact mint');add(delta,ac[1],u(d,1));}
  else if([8,15].includes(d[0])){assert(d.length===(d[0]===8?9:10)&&mints.get(ac[0])===ac[1],'Burn exact typed mint');add(delta,ac[0],-u(d,1));}
  else if(d[0]===9){assert(d.length===1,'Close exact framing');const remaining=BigInt(pre.get(ac[0])?.uiTokenAmount.amount||0)+(delta.get(ac[0])||0n);assert(remaining===0n||mints.get(ac[0])===sol,'close empty or native token');if(mints.get(ac[0])===sol)add(delta,ac[0],-remaining);const native=BigInt(tx.meta.preBalances[keys.indexOf(ac[0])])+(nativeDelta.get(ac[0])||0n);add(nativeDelta,ac[0],-native);add(nativeDelta,ac[1],native);}
  else if(d[0]===17){const key=ac[0],prior=BigInt(pre.get(key)?.uiTokenAmount.amount||0),reserve=rent.get(key)??(BigInt(tx.meta.preBalances[keys.indexOf(key)])-prior),actual=BigInt(tx.meta.preBalances[keys.indexOf(key)])+(nativeDelta.get(key)||0n)-reserve;assert(mints.get(key)===sol&&actual>=0n,'WSOL Sync exact funding/rent');delta.set(key,actual-prior);}
  else assert([4,5,6,10,11,13,21,22].includes(d[0]),'unproved affected token operation '+d[0]);
 }
 const ledgers=[];for(const key of affected){const before=pre.get(key),after=post.get(key);if(!before&&!after){const create=ix.find(c=>c.program===system&&c.accounts[1]===key&&[0,3].includes(c.data.readUInt32LE())),init=ix.find(c=>isToken(c)&&[1,16,18].includes(c.data[0])&&c.accounts[0]===key),close=ix.find(c=>isToken(c)&&c.data[0]===9&&c.accounts[0]===key);assert(create&&init&&close&&create.index<init.index&&init.index<close.index&&BigInt(tx.meta.preBalances[keys.indexOf(key)])===0n&&BigInt(tx.meta.postBalances[keys.indexOf(key)])===0n,'exact transient created/initialized/closed principal');}if(before&&after)assert(before.mint===after.mint&&before.owner===after.owner&&before.programId===after.programId,'persistent token identity');const observed=BigInt(after?.uiTokenAmount.amount||0)-BigInt(before?.uiTokenAmount.amount||0),expected=delta.get(key)||0n;assert(observed===expected,'whole original token ledger '+key+' '+observed+' != '+expected);ledgers.push({account:key,mint:(before||after)?.mint||mints.get(key),owner:(before||after)?.owner||null,program:(before||after)?.programId||ix.find(c=>isToken(c)&&[1,16,18].includes(c.data[0])&&c.accounts[0]===key)?.program,before:String(before?.uiTokenAmount.amount||0),after:String(after?.uiTokenAmount.amount||0),observedDelta:String(observed),expectedDelta:String(expected)});}
 proof.push({signature:sig,slot:row.block_num,sourceIndex:source.index,program:source.program,nativeData:b.toString('hex'),classification,originalPbSha256:hash(dir+'/'+sig+'.pb'),finalizedRpcSha256:hash(dir+'/'+sig+'.rpc.json.gz'),completeCommittedRuntime:true,rpcLogsTruncated:trunc>=0,retainedRpcLogPrefixLength:trunc>=0?trunc:tx.meta.logMessages.length,principals:operations.map(typed),wholeOriginalTokenLedgers:ledgers,spotLegs:[]});
 }catch(e){failures.push({signature:row.signature,source:row.instruction_index,program:row.program_id,error:e.message});}}
 const result={scope:'Exact finalized retained no-index-child source corpus; program names alone are never exclusion proof',sources:proof.length,transactions:new Set(proof.map(x=>x.signature)).size,spotLegs:0,completeTokenLedgers:proof.reduce((n,x)=>n+x.wholeOriginalTokenLedgers.length,0),proofs:proof,failures};fs.writeFileSync(dir+'/independent-proof.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({...result,proofs:undefined,failures:failures.slice(0,12),failureCount:failures.length}));if(failures.length)process.exitCode=1;
