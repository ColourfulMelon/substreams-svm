const resolvePackage=require('module').createRequire;
const packageMode=process.argv[2]?.endsWith('package.json');
const packageRequire=packageMode?resolvePackage(require('path').resolve(process.argv[2])):require;
const protobufRequire=packageMode&&process.argv[3]?resolvePackage(require('path').resolve(process.argv[3])):require;
const fs=require('fs'),path=require('path'),z=require('zlib'),crypto=require('crypto'),bs=packageRequire('bs58').default||packageRequire('bs58'),pb=protobufRequire('protobufjs');
const dir=packageMode?__dirname:process.argv[2]||__dirname,T=pb.parse(fs.readFileSync(path.join(dir,'solana-type.proto'),'utf8')).root.lookupType('sf.solana.type.v1.ConfirmedTransaction');
const program='CQwWoJENUtKmwCMqnyGbEYkg41oxdat23kkNdJLvY7v9',token='TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',quest='QUESTP8xKMfot3ErcdfWXsHbG3kN9mutieAqrVNw74s';
const assert=(x,m)=>{if(!x)throw Error(m)},u=(d,o)=>d.readBigUInt64LE(o),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const originals=fs.readdirSync(dir).filter(n=>n.endsWith('.pb')).sort(),out=[],failures=[];let tokenLedgers=0,nativeLedgers=0;
for(const file of originals)try{
 const signature=file.slice(0,-3),wire=fs.readFileSync(path.join(dir,file)),tx=T.decode(wire),rpc=JSON.parse(z.gunzipSync(fs.readFileSync(path.join(dir,signature+'.rpc.json.gz')))),raw=rpc.response?.result||rpc.result||rpc;
 assert(!raw.meta.err&&tx.meta.err==null,'successful original');assert(raw.transaction.signatures[0]===signature,'exact signature');
 const keys=raw.transaction.message.accountKeys.concat(raw.meta.loadedAddresses.writable,raw.meta.loadedAddresses.readonly),pk=tx.transaction.message.accountKeys.concat(tx.meta.loadedWritableAddresses,tx.meta.loadedReadonlyAddresses).map(bs.encode);
 assert(JSON.stringify(keys)===JSON.stringify(pk),'complete raw PB finalized RPC account parity');
 const rg=new Map(raw.meta.innerInstructions.map(g=>[g.index,g.instructions])),pg=new Map(tx.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const walk=raw.transaction.message.instructions.flatMap((c,n)=>[{...c,stackHeight:1},...(rg.get(n)||[])]).map((c,index)=>({...c,index,program:keys[c.programIdIndex],accounts:c.accounts.map(n=>keys[n]),data:Buffer.from(bs.decode(c.data))}));
 const pw=tx.transaction.message.instructions.flatMap((c,n)=>[{...c,stackHeight:1},...(pg.get(n)||[])]);
 assert(walk.length===pw.length&&walk.every((c,n)=>c.program===pk[pw[n].programIdIndex]&&c.stackHeight===pw[n].stackHeight&&c.data.equals(Buffer.from(pw[n].data))&&JSON.stringify(c.accounts)===JSON.stringify(Array.from(pw[n].accounts||[]).map(i=>pk[i]))),'all native payloads, roles, source indices and depths');
 assert(JSON.stringify(raw.meta.logMessages)===JSON.stringify(tx.meta.logMessages),'complete source runtime parity');
 for(const side of ['pre','post']){
  assert(JSON.stringify(raw.meta[side+'Balances'].map(String))===JSON.stringify(tx.meta[side+'Balances'].map(String)),'native balance parity');
  assert(raw.meta[side+'TokenBalances'].length===tx.meta[side+'TokenBalances'].length&&raw.meta[side+'TokenBalances'].every((b,n)=>{const p=tx.meta[side+'TokenBalances'][n];return b.accountIndex===p.accountIndex&&b.mint===p.mint&&b.owner===p.owner&&b.programId===p.programId&&b.uiTokenAmount.amount===p.uiTokenAmount.amount&&b.uiTokenAmount.decimals===p.uiTokenAmount.decimals}),'token balance parity');
 }
 const events=new Map(),stack=[];let cursor=0;const committed=new Set();
 for(const [n,log]of raw.meta.logMessages.entries()){
  const w=log.split(' ');if(w[0]!=='Program')continue;
  if(w[2]==='invoke'){const depth=Number(w[3].slice(1,-1));assert(walk[cursor]?.program===w[1]&&walk[cursor].stackHeight===depth&&depth===stack.length+1,'exact full native invocation order');stack.push(cursor++);}
  else if(w[2]==='success'){const i=stack.pop();assert(i!==undefined&&walk[i].program===w[1],'committed successful ancestry');committed.add(i);}
  else if(w[2]==='failed:')throw Error('failed native frame');
  else if(w[1]==='data:'){const d=Buffer.from(w[2],'base64');if(d.subarray(0,8).toString('hex')!=='7597166370758b7d')continue;const i=stack.at(-1);assert(walk[i]?.program===program,'native source-bound event');assert(!events.has(i),'unique fill per native source');events.set(i,{log:n,data:d});}
 }
 assert(cursor===walk.length&&!stack.length,'complete runtime provenance');
 const changes=new Map();const add=(a,n)=>changes.set(a,(changes.get(a)||0n)+n);
 for(const c of walk){assert(committed.has(c.index),'all committed sources');if(c.program!==token)continue;const d=c.data,a=c.accounts;assert(a.length===4&&a[2]===a[3],'typed duplicated bank authority');
  if(d.length===9&&d[0]===3){const amount=u(d,1);add(a[0],-amount);add(a[1],amount);}
  else{assert(d.length===10&&d[0]===15&&d[9]===9&&a[1]===quest,'typed QUEST BurnChecked');add(a[0],-u(d,1));}
 }
 const before=new Map(raw.meta.preTokenBalances.map(b=>[keys[b.accountIndex],b])),after=new Map(raw.meta.postTokenBalances.map(b=>[keys[b.accountIndex],b]));
 assert(before.size===4&&after.size===4&&changes.size===4,'both asset pairs complete');
 for(const [a,b]of before){const p=after.get(a);assert(p&&b.mint===p.mint&&b.owner===p.owner&&b.programId===token&&p.programId===token&&BigInt(p.uiTokenAmount.amount)-BigInt(b.uiTokenAmount.amount)===changes.get(a),'whole persistent bank token ledger');tokenLedgers++;}
 const nativeChanges=new Map([[keys[0],-BigInt(raw.meta.fee)]]);const nativeAdd=(a,n)=>nativeChanges.set(a,(nativeChanges.get(a)||0n)+n);const closed=new Set();for(const [source]of events){const a=walk[source].accounts,p=keys.indexOf(a[3]);if(raw.meta.postBalances[p]===0){assert(!closed.has(a[3])&&BigInt(raw.meta.preBalances[p])>0n,'unique completed-order primary closure');closed.add(a[3]);const rent=BigInt(raw.meta.preBalances[p]);nativeAdd(a[3],-rent);nativeAdd(a[4],rent);}}for(let i=0;i<keys.length;i++){assert(BigInt(raw.meta.postBalances[i])-BigInt(raw.meta.preBalances[i])===(nativeChanges.get(keys[i])||0n),'whole native transaction ledger');nativeLedgers++;}
 const legs=[];
 for(const [source,e]of events){const c=walk[source],a=c.accounts,d=c.data,event=e.data;assert(d.length===9&&[218,221].includes(d[0])&&a.length===18,'pinned primary market ABI');const ask=d[0]===218;
  assert(event.length===201&&event[8]===Number(!ask),'201-byte primary side');for(const [offset,role]of [[9,3],[41,5],[73,9],[105,ask?1:4],[137,8]])assert(bs.encode(event.subarray(offset,offset+32))===a[role],'primary event role binding');
  const qty=u(event,169),unit=u(event,177),gross=u(event,185),burn=u(event,193),net=gross-burn,asset=qty*1000000n;assert(qty===u(d,1)&&qty>0n&&unit>0n&&gross===qty*unit*1000n&&burn>0n&&net>0n,'physical request/price/gross/burn arithmetic');
  const roles=[[12,14,10,10],[12,13,10,10],[7,11,6,6]],amounts=[burn,net,asset];let end=source+1;while(end<walk.length&&walk[end].stackHeight>c.stackHeight)end++;assert(end-source===4,'exact three immediate principal CPIs');
  for(let n=0;n<3;n++){const x=walk[source+n+1];assert(x.program===token&&x.stackHeight===c.stackHeight+1&&JSON.stringify(x.accounts)===JSON.stringify(roles[n].map(i=>a[i]))&&u(x.data,1)===amounts[n]&&x.data[0]===(n===0?15:3),'typed burn/net/asset committed witness');}
  for(const [role,mint,owner,decimals]of [[7,8,6,6],[11,8,10,6],[12,14,10,9],[13,14,6,9]]){const b=before.get(a[role]),p=after.get(a[role]);assert(b&&p&&b.mint===a[mint]&&b.owner===a[owner]&&b.uiTokenAmount.decimals===decimals&&p.uiTokenAmount.decimals===decimals,'native role bank ownership and mint');}
  legs.push({source_index:source,first_transfer_index:null,physical_quest_transfer_index:source+2,source_log_index:e.log,amm:program,pool:a[3],user:bs.encode(event.subarray(105,137)),input_mint:quest,input_amount:String(gross),output_mint:a[8],output_amount:String(asset),net_quest_transfer:String(net),quest_burn:String(burn),event_log_index:e.log,buyer_perspective:true});
 }
 out.push({signature,slot:raw.slot,raw_pb_sha256:sha(wire),finalized_rpc_sha256:sha(fs.readFileSync(path.join(dir,signature+'.rpc.json.gz'))),legs});
}catch(e){failures.push({original:file,error:e.message});}
const proof={scope:[Math.min(...out.map(t=>t.slot)),Math.max(...out.map(t=>t.slot))],transactions:out.length,financial_legs:out.reduce((n,t)=>n+t.legs.length,0),token_ledgers:tokenLedgers,native_ledgers:nativeLedgers,results:out,failures};
fs.writeFileSync(path.join(dir,'independent-proof.json'),JSON.stringify(proof,null,2)+'\n');console.log(JSON.stringify({transactions:proof.transactions,financial_legs:proof.financial_legs,token_ledgers:tokenLedgers,native_ledgers:nativeLedgers,failures}));if(failures.length)process.exitCode=1;
