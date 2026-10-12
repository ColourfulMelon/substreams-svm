const fs=require('fs'),crypto=require('crypto'),z=require('zlib');
const bs=require('bs58').default||require('bs58');
const pb=require('protobufjs');
const T=pb.parse(fs.readFileSync(require('path').join(__dirname,'solana-type.proto'),'utf8')).root.lookupType('sf.solana.type.v1.ConfirmedTransaction');
const dir=process.argv[2]||__dirname,amm='pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA',token='TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',token22='TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',fee='pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ',sol='So11111111111111111111111111111111111111112',system='11111111111111111111111111111111';
const assert=(p,msg)=>{if(!p)throw Error(msg)},hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex'),u=(b,o)=>b.readBigUInt64LE(o);
const rows=JSON.parse(fs.readFileSync(dir+'/swap-candidates.json')).filter(x=>x.program_id===amm&&x.reason==='recognized_swap_without_verified_flows');const proof=[];
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
  else if(w[2]==='success'||w[2]==='failed:'){const f=stack.pop();assert(f&&ix[f.source].program===w[1]&&w[2]==='success','committed full ancestry');if(ix[f.source].program===fee){assert(stack.length&&ix[stack.at(-1).source].program===amm&&f.returned.length===1&&f.returned[0].length===24,'native fee return');returns.set(f.source,[u(f.returned[0],0),u(f.returned[0],8),u(f.returned[0],16)]);}}
 }
 assert(cursor===ix.length&&!stack.length,'complete invocation witness');
 const native=new Map(),tokens=new Map(),mints=new Map(),add=(m,k,n)=>m.set(k,(m.get(k)||0n)+n),balance=(a,which)=>BigInt(tx.meta[which][keys.indexOf(a)]);
 add(native,keys[0],-BigInt(tx.meta.fee));
 const legs=[];
 for(const a of tx.meta.preTokenBalances.concat(tx.meta.postTokenBalances))mints.set(keys[a.accountIndex],a.mint);
 for(const x of ix){const b=x.data,a=x.accounts;
  if(x.program===system){const op=b.readUInt32LE();if(op===2||op===0){const n=u(b,4);add(native,a[0],-n);add(native,a[1],n);}else if(op===3){const len=Number(u(b,36)),o=44+len,n=u(b,o);assert(b.length===o+48,'exact CreateWithSeed layout');add(native,a[0],-n);add(native,a[1],n);}else throw Error('unproved System op '+op);}
  if(x.program===token||x.program===token22){if(b[0]===1){assert(a.length>=3,'InitializeAccount');mints.set(a[0],a[1]);}
   else if(b[0]===12){assert(b.length===10&&a.length===4,'exact checked receipt');const n=u(b,1);assert(mints.get(a[0])===a[1]&&mints.get(a[2])===a[1],'both receipt mints');add(tokens,a[0],-n);add(tokens,a[2],n);if(a[1]===sol){add(native,a[0],-n);add(native,a[2],n);}}
   else if(b[0]===9){assert(b.length===1&&a.length===3&&mints.get(a[0])===sol,'exact WSOL close');const n=balance(a[0],'preBalances')+(native.get(a[0])||0n);assert(n>=0n,'physical close remainder');add(native,a[0],-n);add(native,a[1],n);}
   else throw Error('unproved token op '+b[0]);
  }
  if(x.program!==amm||b.subarray(0,8).toString('hex')!=='c62e1552b4d9e870')continue;
  assert(b.length===25&&b[24]===1&&a.length===26,'exact repeated native ABI');const calls=ix.slice(x.index+1,x.index+8);assert(calls.length===7&&calls.every(c=>c.depth===x.depth+1),'exact immediate native children');
  const f=calls[0],out=calls[1],quote=calls.slice(2,6),e=calls[6];assert(f.program===fee&&f.data.length===57&&returns.has(f.index),'committed exact fee CPI');assert(out.program===token22&&out.accounts[0]===a[7]&&out.accounts[1]===a[3]&&out.accounts[2]===a[5]&&out.accounts[3]===a[0],'base principal identities');
  assert(quote.every(c=>c.program===token&&c.data[0]===12&&c.accounts[0]===a[6]&&c.accounts[1]===a[4]&&c.accounts[3]===a[1]),'all quote debit authority identities');assert(JSON.stringify(quote.map(c=>c.accounts[2]))===JSON.stringify([a[8],a[10],a[17],a[25]]),'exact quote vault/fee roles');
  assert(e.program===amm&&e.depth===x.depth+1&&e.accounts.length===1&&e.accounts[0]===a[15]&&e.data.subarray(0,16).toString('hex')==='e445a52e51cb9a1d67f4521f2cf57777','source-bound native event');assert(bs.encode(e.data.subarray(128,160))===a[0]&&bs.encode(e.data.subarray(160,192))===a[1],'event pool/user identities');
  const input=quote.reduce((n,c)=>n+u(c.data,1),0n),output=u(out.data,1);assert(input===u(b,8)&&output>=u(b,16),'native input and minimum');assert(input===u(e.data,72)&&output===u(e.data,24),'event matched by independent physical receipts');
  const bps=returns.get(f.index), beforeFee=u(e.data,120),ceil=(n,d)=>(n+d-1n)/d;
  assert(u(e.data,80)===bps[0]&&u(e.data,96)===bps[1]&&u(e.data,352)===bps[2],'native return versus source event fee rates');
  assert(u(e.data,88)===ceil(beforeFee*bps[0],10000n)&&u(e.data,104)===ceil(beforeFee*bps[1],10000n)&&u(e.data,360)===ceil(beforeFee*bps[2],10000n),'exact primary ceil fee math');
  assert(u(quote[0].data,1)===beforeFee+u(e.data,88)&&u(quote[1].data,1)+u(quote[3].data,1)===u(e.data,104)&&u(quote[2].data,1)===u(e.data,360),'full LP/protocol-buyback/creator physical fee settlement');
  legs.push({signature:sig,slot:tx.slot,protocol:10,source_index:x.index,first_transfer_index:out.index,amm,pool:a[0],user:a[1],input_mint:a[4],input_amount:String(input),output_mint:a[3],output_amount:String(output),feeReturnBps:returns.get(f.index).map(String),feeReceipts:quote.slice(1).map(c=>({source:c.index,recipient:c.accounts[2],amount:String(u(c.data,1))}))});
 }
 assert(legs.length===2&&legs[1].source_index===row.instruction_index,'two exact native legs');
 const nativeLedgers=[...native].map(([account,expected])=>({account,expected:String(expected),actual:String(balance(account,'postBalances')-balance(account,'preBalances'))}));assert(nativeLedgers.every(l=>l.expected===l.actual),'every modeled native account');
 const tokenLedgers=[...tokens].filter(([account])=>mints.get(account)!==sol).map(([account,expected])=>{const n=keys.indexOf(account),before=tx.meta.preTokenBalances.find(t=>t.accountIndex===n),after=tx.meta.postTokenBalances.find(t=>t.accountIndex===n);assert(before&&after,'full principal balance witness');return{account,mint:mints.get(account),expected:String(expected),actual:String(BigInt(after.uiTokenAmount.amount)-BigInt(before.uiTokenAmount.amount))};});assert(tokenLedgers.every(l=>l.expected===l.actual),'all principal token balances including both outputs');
 proof.push({signature:sig,slot:tx.slot,rawPbSha256:hash(dir+'/'+sig+'.pb'),rpcGzipSha256:hash(dir+'/'+sig+'.rpc.json.gz'),completeRuntime:true,legs,nativeLedgers,tokenLedgers});
}
fs.writeFileSync(dir+'/repeated-independent-proof.json',JSON.stringify(proof,null,2));console.log(JSON.stringify({transactions:proof.length,legs:proof.flatMap(t=>t.legs).length,restored:proof.length,nativeLedgers:proof.flatMap(t=>t.nativeLedgers).length,tokenLedgers:proof.flatMap(t=>t.tokenLedgers).length}));
