const fs=require('fs'),crypto=require('crypto'),z=require('zlib');
const bs=require('bs58').default||require('bs58');
const pb=require('protobufjs');
const T=pb.parse(fs.readFileSync(require('path').join(__dirname,'solana-type.proto'),'utf8')).root.lookupType('sf.solana.type.v1.ConfirmedTransaction');
const dir=process.argv[2]||__dirname,amm='pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA',token='TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',token22='TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',fee='pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ',sol='So11111111111111111111111111111111111111112',system='11111111111111111111111111111111';
const assert=(p,msg)=>{if(!p)throw Error(msg)},hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex'),u=(b,o)=>b.readBigUInt64LE(o);
const owned=JSON.parse(fs.readFileSync(dir+'/owned-mints.json')),feeConfigs=new Map();
owned.addresses.forEach((address,n)=>{
 const account=owned.value[n],b=Buffer.from(account.data[0],'base64');assert(account.owner===token22&&!account.executable&&b.length>=166&&b[45]===1&&b[165]===1,'owned initialized Token-2022 mint');
 let offset=166,config=null,seen=new Set();
 while(offset+4<=b.length){const tag=b.readUInt16LE(offset),len=b.readUInt16LE(offset+2);if(!tag&&!len)break;assert(!seen.has(tag)&&offset+4+len<=b.length,'exact bounded unique mint TLV');seen.add(tag);const value=b.subarray(offset+4,offset+4+len);
  if(tag===1){assert(len===108,'owned bounded transfer fee configuration');assert(value.subarray(72,90).equals(value.subarray(90,108)),'both fee schedules identical');config={maximum:u(value,80),bps:BigInt(value.readUInt16LE(88))};assert(config.bps<=10000n,'valid transfer fee bps');}
  assert(tag!==14,'no transfer hook');offset+=4+len;
 }
 feeConfigs.set(address,config);
});
const rows=JSON.parse(fs.readFileSync(dir+'/swap-candidates.json'));const proof=[], failures=[]; const gator='gatorLx9aC1e5ZWAXscv5QRKiLXnLPLXjftVc81h1Hr';const accounts=JSON.parse(fs.readFileSync(require('path').join(dir,'owned-accounts.json')));const ownedAccounts=new Map(accounts.addresses.map((a,n)=>[a,accounts.value[n]]));
for(const row of rows){try{
 const sig=row.signature,raw=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+sig+'.rpc.json.gz')));const tx=raw.response.result;assert(raw.request.params[1].commitment==='finalized','finalized request');
 assert(tx.slot===row.block_num&&tx.meta.err===null,'finalized original slot/success');
 const keys=tx.transaction.message.accountKeys.concat(tx.meta.loadedAddresses.writable,tx.meta.loadedAddresses.readonly),groups=new Map(tx.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const ix=tx.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(groups.get(i)||[])]).map((x,index)=>({index,depth:x.stackHeight,program:keys[x.programIdIndex],accounts:x.accounts.map(i=>keys[i]),data:Buffer.from(bs.decode(x.data))}));
 const original=T.decode(fs.readFileSync(dir+'/'+sig+'.pb')),pk=original.transaction.message.accountKeys.concat(original.meta.loadedWritableAddresses,original.meta.loadedReadonlyAddresses).map(bs.encode),pg=new Map(original.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const pIx=original.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(pg.get(i)||[])]);
 assert(JSON.stringify(keys)===JSON.stringify(pk)&&pIx.length===ix.length,'PB RPC full keys/walk parity');
 pIx.forEach((p,i)=>assert(pk[p.programIdIndex]===ix[i].program&&p.stackHeight===ix[i].depth&&Buffer.from(p.data).equals(ix[i].data)&&JSON.stringify(Array.from(p.accounts||[]).map(n=>pk[n]))===JSON.stringify(ix[i].accounts),'PB RPC exact instruction parity'));
 assert(JSON.stringify(original.meta.preBalances.map(String))===JSON.stringify(tx.meta.preBalances.map(String))&&JSON.stringify(original.meta.postBalances.map(String))===JSON.stringify(tx.meta.postBalances.map(String)),'PB RPC all native balances');
 const stack=[],returns=new Map();let cursor=0;
 for(const l of original.meta.logMessages){assert(l!=='Log truncated','complete logs');const w=l.split(' ');if(w[0]!=='Program')continue;
  if(w[2]==='invoke'){assert(ix[cursor].program===w[1]&&ix[cursor].depth===stack.length+1&&Number(w[3].slice(1,-1))===ix[cursor].depth,'native runtime source/depth');stack.push({source:cursor++,returned:[]});}
  else if(w[1]==='return:'){assert(stack.length&&ix[stack.at(-1).source].program===w[2],'scoped return provenance');stack.at(-1).returned.push(Buffer.from(w[3],'base64'));}
  else if(w[2]==='success'||w[2]==='failed:'){const f=stack.pop();assert(f&&ix[f.source].program===w[1]&&w[2]==='success','committed full ancestry');if(ix[f.source].program===fee){assert(stack.length&&[amm,'6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P'].includes(ix[stack.at(-1).source].program)&&f.returned.length===1&&f.returned[0].length===24,'native fee return');returns.set(f.source,[u(f.returned[0],0),u(f.returned[0],8),u(f.returned[0],16)]);}}
 }
 assert(cursor===ix.length&&!stack.length,'complete invocation witness');
 const native=new Map(),tokens=new Map(),mints=new Map(),add=(m,k,n)=>m.set(k,(m.get(k)||0n)+n),balance=(a,which)=>BigInt(tx.meta[which][keys.indexOf(a)]);
 add(native,keys[0],-BigInt(tx.meta.fee));
 const legs=[];
 for(const a of tx.meta.preTokenBalances.concat(tx.meta.postTokenBalances))mints.set(keys[a.accountIndex],a.mint);
 for(const x of ix){const b=x.data,a=x.accounts;
  if(x.program===system){const op=b.readUInt32LE();if(op===2||op===0){const n=u(b,4);add(native,a[0],-n);add(native,a[1],n);}else if(op===3){const len=Number(u(b,36)),o=44+len,n=u(b,o);assert(b.length===o+48,'exact CreateWithSeed layout');add(native,a[0],-n);add(native,a[1],n);}else if(op===4){assert(b.length===4&&a.length===3,'exact AdvanceNonceAccount');}else throw Error('unproved System op '+op);}
  if(x.program===token||x.program===token22){if(b[0]===1){assert(a.length>=3,'InitializeAccount');mints.set(a[0],a[1]);}
   else if(b[0]===18){assert(b.length===33&&a.length===2,'InitializeAccount3');mints.set(a[0],a[1]);}
   else if(b[0]===6){assert(b.length===35&&a.length===2,'bounded SetAuthority');}
   else if([21,22,17].includes(b[0])){assert([1,3].includes(b.length),'exact non-transfer token operation');}
   else if(b[0]===3||b[0]===12){const checked=b[0]===12,destination=a[checked?2:1],mint=checked?a[1]:mints.get(a[0]);assert(b.length===(checked?10:9)&&a.length>=(checked?4:3),'exact global token receipt');const n=u(b,1);assert(mints.get(a[0])===mint&&mints.get(destination)===mint,'both receipt mints');const config=x.program===token22?feeConfigs.get(mint):null;assert(x.program!==token22||feeConfigs.has(mint),'every Token-2022 receipt owned mint');const charge=config?((n*config.bps+9999n)/10000n<config.maximum?(n*config.bps+9999n)/10000n:config.maximum):0n;add(tokens,a[0],-n);add(tokens,destination,n-charge);if(mint===sol){add(native,a[0],-n);add(native,destination,n);}}
   else if(b[0]===9){assert(b.length===1&&a.length===3&&mints.has(a[0])&&(mints.get(a[0])===sol||BigInt(tx.meta.preTokenBalances.find(t=>keys[t.accountIndex]===a[0])?.uiTokenAmount.amount||0)+(tokens.get(a[0])||0n)===0n),'exact token close');const n=balance(a[0],'preBalances')+(native.get(a[0])||0n);assert(n>=0n,'physical close remainder');add(native,a[0],-n);add(native,a[1],n);}
   else throw Error('unproved token op '+b[0]);
  }
  if(x.program===amm&&b.toString('hex')==='f945a4da9667548a'){
   const descendants=ix.slice(x.index+1,ix.find(d=>d.index>x.index&&d.depth<=x.depth)?.index||ix.length);assert(a.length===4&&a[3]===amm&&descendants.length===1&&descendants[0].depth===x.depth+1&&descendants[0].program===amm&&descendants[0].accounts.length===1&&descendants[0].accounts[0]===a[2]&&descendants[0].data.length===88&&descendants[0].data.subarray(0,16).toString('hex')==='e445a52e51cb9a1d929fbdac925838f4'&&bs.encode(descendants[0].data.subarray(16,48))===a[0],'committed source-bound close-volume event');
   const creation=ix.filter(c=>c.index<x.index&&c.program===system&&c.data.length===52&&c.data.readUInt32LE()===0&&c.accounts[1]===a[1]).at(-1);
   assert(creation&&bs.encode(creation.data.subarray(20,52))===amm&&u(creation.data,12)===137n&&creation.accounts[0]===a[0],'independent exact owned state creation/funder');
   const principal=balance(a[1],'preBalances')+(native.get(a[1])||0n);assert(principal===u(creation.data,4),'known exact initialized owned rent remainder');add(native,a[1],-principal);add(native,a[0],principal);
  }
  if(x.index!==row.instruction_index)continue;
  const end=ix.find(c=>c.index>x.index&&c.depth<=x.depth)?.index||ix.length,calls=ix.slice(x.index+1,end),receipts=calls.filter(c=>[token,token22].includes(c.program)&&c.data[0]===12),e=calls.at(-1);
  assert(calls.every(c=>c.depth===x.depth+1),'exact immediate swap children');
  assert(x.program===gator&&[41,113].includes(b.length)&&b.at(-1)===1,'primary GatorSwap payload');
  assert([15,17,18,36].includes(a.length)&&a[6]===token&&a[7]===token22&&a[10]===system&&a[11]==='CKQdReQgzubcDznauYzDzJLmqU3NXPLPKjZh4RRDgKZN','typed Gator fixed roles');
  const callsReceipts=calls.filter(c=>[token,token22].includes(c.program)&&c.data[0]===3);
  assert(callsReceipts.length===2&&calls.length===(calls[0].program===system?3:2),'exact two Gator principals with optional state creation');
  if(calls[0].program===system){const c=calls[0];assert(c.data.length===52&&c.data.readUInt32LE()===0&&c.accounts.length===2&&c.accounts[0]===a[0]&&c.accounts[1]===a[8]&&u(c.data,12)===512n&&bs.encode(c.data.subarray(20,52))===gator,'exact owned quote-state create');}
  const inputMint=bs.encode(b.subarray(8,40)),side=mints.get(a[4])===inputMint?0:mints.get(a[5])===inputMint?1:-1;assert(side!==-1,'primary input mint binds owned pool side');
  const inputAccount=a[1+side],outputAccount=a[2-side],inputVault=a[4+side],outputVault=a[5-side],outputMint=mints.get(outputVault),[i,o]=callsReceipts;
  assert(inputMint!==outputMint&&mints.get(inputAccount)===inputMint&&mints.get(outputAccount)===outputMint,'all four principal mint identities');
  assert(i.accounts.length===3&&o.accounts.length===3&&i.data.length===9&&o.data.length===9&&i.accounts[0]===inputAccount&&i.accounts[1]===inputVault&&i.accounts[2]===a[0]&&o.accounts[0]===outputVault&&o.accounts[1]===outputAccount&&o.accounts[2]===a[3],'exact native user/pool authorities and receipts');
  const input=u(i.data,1),output=u(o.data,1);assert(input===u(b,0)&&input>0n&&output>0n,'native exact input and physical output');
  const pool=ownedAccounts.get(a[3]);assert(pool&&pool.owner===gator&&!pool.executable,'owned native pool');const pd=Buffer.from(pool.data[0],'base64');assert([1984,5120].includes(pd.length)&&[1,2].includes(pd[0])&&pd[1]===1&&bs.encode(pd.subarray(8,40))===mints.get(a[4])&&bs.encode(pd.subarray(40,72))===mints.get(a[5]),'owned initialized pool mint layout');
  for(const role of [4,5]){const v=ownedAccounts.get(a[role]);assert(v&&[token,token22].includes(v.owner)&&!v.executable,'owned native token vault');const vd=Buffer.from(v.data[0],'base64');assert(vd.length>=165&&bs.encode(vd.subarray(0,32))===mints.get(a[role])&&bs.encode(vd.subarray(32,64))===a[3]&&vd[108]===1,'owned initialized vault mint/authority');}
  const first=i.index;legs.push({signature:sig,slot:tx.slot,protocol:65,source_index:x.index,first_transfer_index:first,amm:gator,pool:a[3],user:a[0],input_mint:inputMint,input_amount:String(input),output_mint:outputMint,output_amount:String(output),nativeProfile:{payloadBytes:b.length,accounts:a.length,opaqueSignedQuote:b.length===113,noPlaintextMinimum:true},physicalReceipts:callsReceipts.map(c=>({source:c.index,program:c.program,accounts:c.accounts,amount:String(u(c.data,1))}))});

 }
 assert(legs.length===1&&legs[0].source_index===row.instruction_index,'exact missing native leg');
 const nativeLedgers=[...native].map(([account,expected])=>({account,expected:String(expected),actual:String(balance(account,'postBalances')-balance(account,'preBalances'))}));assert(nativeLedgers.every(l=>l.expected===l.actual),'every modeled native account '+JSON.stringify(nativeLedgers.filter(l=>l.expected!==l.actual)));
 const tokenLedgers=[...tokens].filter(([account])=>mints.get(account)!==sol).map(([account,expected])=>{const n=keys.indexOf(account),before=tx.meta.preTokenBalances.find(t=>t.accountIndex===n),after=tx.meta.postTokenBalances.find(t=>t.accountIndex===n);assert(before||after||expected===0n&&balance(account,'preBalances')===0n&&balance(account,'postBalances')===0n&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===18&&c.accounts[0]===account)&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===9&&c.accounts[0]===account),'full principal balance witness or exact zero creation/closure');return{account,mint:mints.get(account),expected:String(expected),actual:String(BigInt(after?.uiTokenAmount.amount||0)-BigInt(before?.uiTokenAmount.amount||0))};});assert(tokenLedgers.every(l=>l.expected===l.actual),'all principal token balances: '+sig+' '+JSON.stringify(tokenLedgers.filter(l=>l.expected!==l.actual)));
 assert(keys.every((account,n)=>BigInt(tx.meta.postBalances[n])-BigInt(tx.meta.preBalances[n])===(native.get(account)||0n)),'all resolved global native account ledgers');
 proof.push({signature:sig,slot:tx.slot,rawPbSha256:hash(dir+'/'+sig+'.pb'),rpcGzipSha256:hash(dir+'/'+sig+'.rpc.json.gz'),completeRuntime:true,ownedMintProofSha256:hash(dir+'/owned-mints.json'),legs,nativeLedgers,tokenLedgers});
}catch(e){failures.push({signature:row.signature,error:e.message,stack:e.stack});}}
fs.writeFileSync(dir+'/independent-proof.json',JSON.stringify(proof,null,2));fs.writeFileSync(dir+'/independent-failures.json',JSON.stringify(failures,null,2));console.log(JSON.stringify({failures:failures.length,errors:failures.slice(0,5),transactions:proof.length,legs:proof.flatMap(t=>t.legs).length,restored:proof.length,nativeLedgers:proof.flatMap(t=>t.nativeLedgers).length,tokenLedgers:proof.flatMap(t=>t.tokenLedgers).length}));
