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
  if(tag===1){assert(len===108&&value.subarray(0,32).every(x=>x===0),'immutable transfer fee authority');assert(value.subarray(72,90).equals(value.subarray(90,108)),'both fee schedules identical');config={maximum:u(value,80),bps:BigInt(value.readUInt16LE(88))};assert(config.bps<=10000n,'valid transfer fee bps');}
  assert(tag!==14,'no transfer hook');offset+=4+len;
 }
 feeConfigs.set(address,config);
});
const rows=JSON.parse(fs.readFileSync(dir+'/swap-candidates.json')).filter(x=>['3am2','57Z5','3GEa'].some(prefix=>x.signature.startsWith(prefix)));const proof=[];
for(const row of rows){
 const sig=row.signature,raw=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+sig+'.rpc.json.gz')));const tx=raw.response.result;assert(raw.request.params[1].commitment==='finalized','finalized request');
 assert(tx.slot===row.block_num&&tx.meta.err===null,'finalized original slot/success');
 const keys=tx.transaction.message.accountKeys.concat(tx.meta.loadedAddresses.writable,tx.meta.loadedAddresses.readonly),groups=new Map(tx.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const ix=tx.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(groups.get(i)||[])]).map((x,index)=>({index,depth:x.stackHeight,program:keys[x.programIdIndex],accounts:x.accounts.map(i=>keys[i]),data:Buffer.from(bs.decode(x.data))}));
 const original=T.decode(fs.readFileSync(dir+'/'+sig+'.pb')),pk=original.transaction.message.accountKeys.concat(original.meta.loadedWritableAddresses,original.meta.loadedReadonlyAddresses).map(bs.encode),pg=new Map(original.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const pIx=original.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(pg.get(i)||[])]);
 assert(JSON.stringify(keys)===JSON.stringify(pk)&&pIx.length===ix.length,'PB RPC full keys/walk parity');
 pIx.forEach((p,i)=>assert(pk[p.programIdIndex]===ix[i].program&&p.stackHeight===ix[i].depth&&Buffer.from(p.data).equals(ix[i].data)&&JSON.stringify(Array.from(p.accounts).map(n=>pk[n]))===JSON.stringify(ix[i].accounts),'PB RPC exact instruction parity'));
 assert(JSON.stringify(original.meta.preBalances.map(String))===JSON.stringify(tx.meta.preBalances.map(String))&&JSON.stringify(original.meta.postBalances.map(String))===JSON.stringify(tx.meta.postBalances.map(String)),'PB RPC all native balances');
 const stack=[],returns=new Map();let cursor=0;
 for(const l of tx.meta.logMessages){assert(l!=='Log truncated','complete logs');const w=l.split(' ');if(w[0]!=='Program')continue;
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
   else if([21,22,17].includes(b[0])){assert([1,3].includes(b.length),'exact non-transfer token operation');}
   else if(b[0]===3||b[0]===12){const checked=b[0]===12,destination=a[checked?2:1],mint=checked?a[1]:mints.get(a[0]);assert(b.length===(checked?10:9)&&a.length===(checked?4:3),'exact token receipt');const n=u(b,1);assert(mints.get(a[0])===mint&&mints.get(destination)===mint,'both receipt mints');const config=x.program===token22?feeConfigs.get(mint):null;assert(x.program!==token22||feeConfigs.has(mint),'every Token-2022 receipt owned mint');const charge=config?((n*config.bps+9999n)/10000n<config.maximum?(n*config.bps+9999n)/10000n:config.maximum):0n;add(tokens,a[0],-n);add(tokens,destination,n-charge);if(mint===sol){add(native,a[0],-n);add(native,destination,n);}}
   else if(b[0]===9){assert(b.length===1&&a.length===3&&mints.get(a[0])===sol,'exact WSOL close');const n=balance(a[0],'preBalances')+(native.get(a[0])||0n);assert(n>=0n,'physical close remainder');add(native,a[0],-n);add(native,a[1],n);}
   else throw Error('unproved token op '+b[0]);
  }
  if(x.index!==row.instruction_index)continue;
  const end=ix.find(c=>c.index>x.index&&c.depth<=x.depth)?.index||ix.length,calls=ix.slice(x.index+1,end),receipts=calls.filter(c=>[token,token22].includes(c.program)&&c.data[0]===12),e=calls.at(-1);
  assert(calls.every(c=>c.depth===x.depth+1),'exact immediate swap children');
  const curve=x.program!==amm,isBuy=b.subarray(0,8).toString('hex')==='c62e1552b4d9e870';
  const [pool,user,base,quote,userBase,userQuote,vaultBase,vaultQuote,authority]=curve?[10,13,1,2,14,15,11,12,24]:[0,1,3,4,5,6,7,8,15];
  assert(e.program===x.program&&e.accounts.length===1&&e.accounts[0]===a[authority]&&e.data.subarray(0,8).toString('hex')==='e445a52e51cb9a1d','source-bound event self-CPI');
  if(curve)assert(bs.encode(e.data.subarray(16,48))===a[base]&&bs.encode(e.data.subarray(65,97))===a[user]&&e.data[64]===0,'native curve mint/user/sell direction');
  else assert(bs.encode(e.data.subarray(128,160))===a[pool]&&bs.encode(e.data.subarray(160,192))===a[user]&&e.data.subarray(8,16).toString('hex')===(isBuy?'67f4521f2cf57777':'3e2f370aa503dc2a'),'AMM event pool/user/direction');
  const inputAccount=a[isBuy?userQuote:userBase],outputAccount=a[isBuy?userBase:userQuote],inputMint=a[isBuy?quote:base],outputMint=a[isBuy?base:quote];
  const inputs=receipts.filter(c=>c.accounts[0]===inputAccount&&c.accounts[1]===inputMint),outputs=receipts.filter(c=>c.accounts[2]===outputAccount&&c.accounts[1]===outputMint);
  assert(inputs.length>0&&outputs.length===1,'one exact principal credit and typed user debits');
  assert(inputs.every(c=>c.accounts[3]===a[user])&&outputs[0].accounts[0]===a[isBuy?vaultBase:vaultQuote]&&outputs[0].accounts[3]===a[pool],'principal authority/vault identities');
  const input=inputs.reduce((n,c)=>n+u(c.data,1),0n),output=u(outputs[0].data,1);
  assert(input===u(b,8)&&output>=u(b,16),'exact native input/minimum receipts');
  const first=Math.min(inputs[0].index,outputs[0].index);
  legs.push({signature:sig,slot:tx.slot,protocol:curve?9:10,source_index:x.index,first_transfer_index:first,amm:x.program,pool:a[pool],user:a[user],input_mint:inputMint,input_amount:String(input),output_mint:outputMint,output_amount:String(output),committedFeeReturns:calls.filter(c=>returns.has(c.index)).map(c=>({source:c.index,bps:returns.get(c.index).map(String)})),physicalReceipts:receipts.map(c=>({source:c.index,program:c.program,accounts:c.accounts,amount:String(u(c.data,1))}))});
 }
 assert(legs.length===1&&legs[0].source_index===row.instruction_index,'exact missing native leg');
 const nativeLedgers=[...native].map(([account,expected])=>({account,expected:String(expected),actual:String(balance(account,'postBalances')-balance(account,'preBalances'))}));assert(nativeLedgers.every(l=>l.expected===l.actual),'every modeled native account');
 const tokenLedgers=[...tokens].filter(([account])=>mints.get(account)!==sol).map(([account,expected])=>{const n=keys.indexOf(account),before=tx.meta.preTokenBalances.find(t=>t.accountIndex===n),after=tx.meta.postTokenBalances.find(t=>t.accountIndex===n);assert(before||after,'full principal balance witness');return{account,mint:mints.get(account),expected:String(expected),actual:String(BigInt(after?.uiTokenAmount.amount||0)-BigInt(before?.uiTokenAmount.amount||0))};});assert(tokenLedgers.every(l=>l.expected===l.actual),'all principal token balances: '+sig+' '+JSON.stringify(tokenLedgers.filter(l=>l.expected!==l.actual)));
 proof.push({signature:sig,slot:tx.slot,rawPbSha256:hash(dir+'/'+sig+'.pb'),rpcGzipSha256:hash(dir+'/'+sig+'.rpc.json.gz'),completeRuntime:true,ownedMintProofSha256:hash(dir+'/owned-mints.json'),legs,nativeLedgers,tokenLedgers});
}
fs.writeFileSync(dir+'/bounded-independent-proof.json',JSON.stringify(proof,null,2));console.log(JSON.stringify({transactions:proof.length,legs:proof.flatMap(t=>t.legs).length,restored:proof.length,nativeLedgers:proof.flatMap(t=>t.nativeLedgers).length,tokenLedgers:proof.flatMap(t=>t.tokenLedgers).length}));
