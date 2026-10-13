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
const rows=JSON.parse(fs.readFileSync(dir+'/swap-candidates.json'));const proof=[], failures=[];
const idl=JSON.parse(fs.readFileSync(dir+'/primary-pump-idl.json')),defs=new Map(idl.types.map(t=>[t.name,t.type])),prefix='e445a52e51cb9a1dbddb7fd34ee661ee';
function parseEvents(raw){const keys=raw.transaction.message.accountKeys.concat(raw.meta.loadedAddresses.writable,raw.meta.loadedAddresses.readonly),groups=new Map(raw.meta.innerInstructions.map(g=>[g.index,g.instructions])),walk=raw.transaction.message.instructions.flatMap((x,n)=>[{...x,stackHeight:1},...groups.get(n)||[]]);const output=[];for(let i=0;i<walk.length;i++){const b=Buffer.from(bs.decode(walk[i].data));if(!b.toString('hex').startsWith(prefix))continue;let offset=16;function read(t){if(typeof t==='object'){if(t.vec){let n=read('u32');if(Number(n)>32)throw Error('bounded vector');return Array.from({length:Number(n)},()=>read(t.vec));}if(t.defined){const d=defs.get(t.defined.name),out={};if(!d||d.kind!=='struct')throw Error('primary structure');for(const f of d.fields)out[f.name]=read(f.type);return out;}throw Error('primary encoding');}if(t==='string'){const n=Number(read('u32'));if(n>4096||offset+n>b.length)throw Error('bounded string');const s=b.subarray(offset,offset+n).toString('utf8');offset+=n;return s;}const n={pubkey:32,bool:1,u8:1,u16:2,u32:4,u64:8,i64:8,u128:16}[t];if(!n||offset+n>b.length)throw Error('complete field');const d=b.subarray(offset,offset+n);offset+=n;if(t==='pubkey')return bs.encode(d);if(t==='bool'){if(d[0]>1)throw Error('boolean');return d[0]===1;}if(t==='u8')return d[0];if(t==='u16')return String(d.readUInt16LE());if(t==='u32')return String(d.readUInt32LE());if(t==='i64')return String(d.readBigInt64LE());if(t==='u64')return String(d.readBigUInt64LE());if(t==='u128')return String(d.readBigUInt64LE()+d.readBigUInt64LE(8)*2n**64n);}
const e={};for(const f of defs.get('TradeEvent').fields){if(offset===b.length){e[f.name]=typeof f.type==='object'?[]:f.type==='pubkey'?'11111111111111111111111111111111':f.type==='bool'?false:f.type==='string'?'':'0';}else e[f.name]=read(f.type);}if(offset!==b.length)throw Error('unknown trailing event format');if(e.quote_mint==='11111111111111111111111111111111'&&e.quote_amount==='0')e.quote_amount=e.sol_amount;output.push({index:i,event:e});}return output;}

for(const row of rows){try{
 const sig=row.signature,raw=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+sig+'.rpc.json.gz')));const tx=raw.result;
 assert(tx.slot===row.block_num&&tx.meta.err===null,'finalized original slot/success');
 const keys=tx.transaction.message.accountKeys.concat(tx.meta.loadedAddresses.writable,tx.meta.loadedAddresses.readonly),groups=new Map(tx.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const ix=tx.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(groups.get(i)||[])]).map((x,index)=>({index,depth:x.stackHeight,program:keys[x.programIdIndex],accounts:x.accounts.map(i=>keys[i]),data:Buffer.from(bs.decode(x.data))}));
 const original=T.decode(fs.readFileSync(dir+'/'+sig+'.pb')),pk=original.transaction.message.accountKeys.concat(original.meta.loadedWritableAddresses,original.meta.loadedReadonlyAddresses).map(bs.encode),pg=new Map(original.meta.innerInstructions.map(g=>[g.index,g.instructions]));
 const pIx=original.transaction.message.instructions.flatMap((x,i)=>[{...x,stackHeight:1},...(pg.get(i)||[])]);
 assert(JSON.stringify(keys)===JSON.stringify(pk)&&pIx.length===ix.length,'PB RPC full keys/walk parity');
 pIx.forEach((p,i)=>assert(pk[p.programIdIndex]===ix[i].program&&p.stackHeight===ix[i].depth&&Buffer.from(p.data).equals(ix[i].data)&&JSON.stringify(Array.from(p.accounts||[]).map(n=>pk[n]))===JSON.stringify(ix[i].accounts),'PB RPC exact instruction parity'));
 assert(JSON.stringify(original.meta.preBalances.map(String))===JSON.stringify(tx.meta.preBalances.map(String))&&JSON.stringify(original.meta.postBalances.map(String))===JSON.stringify(tx.meta.postBalances.map(String)),'PB RPC all native balances');
 assert(JSON.stringify(original.meta.logMessages)===JSON.stringify(tx.meta.logMessages)&&String(original.meta.fee)===String(tx.meta.fee),'PB RPC complete runtime/fee parity');
 const tokenView=t=>({accountIndex:t.accountIndex,mint:t.mint,owner:t.owner,programId:t.programId,amount:String(t.uiTokenAmount.amount),decimals:t.uiTokenAmount.decimals});
 for(const side of ['preTokenBalances','postTokenBalances'])assert(JSON.stringify(original.meta[side].map(tokenView))===JSON.stringify(tx.meta[side].map(tokenView)),'PB RPC all token balance parity');
 assert(bs.encode(original.transaction.signatures[0])===sig&&tx.transaction.signatures[0]===sig,'original source and finalized RPC signature');

 const stack=[],returns=new Map();let cursor=0;
 for(const l of original.meta.logMessages){assert(l!=='Log truncated','complete logs');const w=l.split(' ');if(w[0]!=='Program')continue;
  if(w[2]==='invoke'){assert(ix[cursor].program===w[1]&&ix[cursor].depth===stack.length+1&&Number(w[3].slice(1,-1))===ix[cursor].depth,'native runtime source/depth');stack.push({source:cursor++,returned:[]});}
  else if(w[1]==='return:'){assert(stack.length&&ix[stack.at(-1).source].program===w[2],'scoped return provenance');stack.at(-1).returned.push(Buffer.from(w[3],'base64'));}
  else if(w[2]==='success'||w[2]==='failed:'){const f=stack.pop();assert(f&&ix[f.source].program===w[1]&&w[2]==='success','committed full ancestry');if(ix[f.source].program===fee){assert(stack.length&&[amm,'6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P'].includes(ix[stack.at(-1).source].program)&&f.returned.length===1&&f.returned[0].length===24,'native fee return');returns.set(f.source,[u(f.returned[0],0),u(f.returned[0],8),u(f.returned[0],16)]);}}
 }
 assert(cursor===ix.length&&!stack.length,'complete invocation witness'); const eventParsed=parseEvents(tx);
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
   else if(b[0]===3||b[0]===12){const checked=b[0]===12,destination=a[checked?2:1],mint=checked?a[1]:mints.get(a[0]);assert(b.length===(checked?10:9)&&a.length>=(checked?4:3),'exact global token receipt');const n=u(b,1);assert(mints.get(a[0])===mint&&mints.get(destination)===mint,'both receipt mints');const charge=0n;add(tokens,a[0],-n);add(tokens,destination,n-charge);if(mint===sol){add(native,a[0],-n);add(native,destination,n);}}
   else if(b[0]===8){assert(b.length===9&&a.length===3&&mints.get(a[0])===a[1],'exact global typed burn');add(tokens,a[0],-u(b,1));}
   else if(b[0]===9){assert(b.length===1&&a.length===3&&mints.has(a[0])&&(mints.get(a[0])===sol||BigInt(tx.meta.preTokenBalances.find(t=>keys[t.accountIndex]===a[0])?.uiTokenAmount.amount||0)+(tokens.get(a[0])||0n)===0n),'exact token close');const n=balance(a[0],'preBalances')+(native.get(a[0])||0n);assert(n>=0n,'physical close remainder');add(native,a[0],-n);add(native,a[1],n);}
   else throw Error('unproved token op '+b[0]);
  }
  if([amm,curve].includes(x.program)&&b.toString('hex')==='f945a4da9667548a'){
   const descendants=ix.slice(x.index+1,ix.find(d=>d.index>x.index&&d.depth<=x.depth)?.index||ix.length);
   assert(a.length===4&&a[3]===x.program&&descendants.length===1&&descendants[0].depth===x.depth+1&&descendants[0].program===x.program&&descendants[0].accounts.length===1&&descendants[0].accounts[0]===a[2]&&descendants[0].data.length===88&&descendants[0].data.subarray(0,16).toString('hex')==='e445a52e51cb9a1d929fbdac925838f4'&&bs.encode(descendants[0].data.subarray(16,48))===a[0],'committed canonical close-volume event');
   assert(PublicKey.findProgramAddressSync([Buffer.from('user_volume_accumulator'),new PublicKey(a[0]).toBuffer()],new PublicKey(x.program))[0].toBase58()===a[1],'exact native owned volume PDA');
   const principal=balance(a[1],'preBalances')+(native.get(a[1])||0n);assert(principal>=0n,'exact physical rent remainder');add(native,a[1],-principal);add(native,a[0],principal);
  }

  if(x.index!==row.instruction_index)continue;
  assert(x.program===amm&&x.depth===1&&b.length===24&&b.subarray(0,8).toString('hex')==='694406af000723a2'&&a.length===13&&a[4]===sol&&a[9]===token22&&a[10]===token&&a[12]===amm,'exact bounded boost source');
  const end=ix.find(c=>c.index>x.index&&c.depth<=x.depth)?.index||ix.length,calls=ix.slice(x.index+1,end);assert([4,5].includes(calls.length)&&calls.every(c=>c.depth===2),'bounded direct boost children');
  const [q,burn,buy,boost]=calls,ba=buy.data,e=boost.data;
  const authority=PublicKey.findProgramAddressSync([Buffer.from('boost_vault'),new PublicKey(a[0]).toBuffer()],new PublicKey(amm))[0].toBase58();
  const ata=PublicKey.findProgramAddressSync([new PublicKey(authority).toBuffer(),new PublicKey(token).toBuffer(),new PublicKey(sol).toBuffer()],new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'))[0].toBase58();
  const ea=PublicKey.findProgramAddressSync([Buffer.from('__event_authority')],new PublicKey(amm))[0].toBase58();assert(a[7]===authority&&a[8]===ata&&a[11]===ea,'native boost authority/ATA/event PDAs');
  assert(q.program===token&&q.data.length===10&&q.data[0]===12&&q.data[9]===9&&JSON.stringify(q.accounts)===JSON.stringify([a[8],a[4],a[6],a[7]])&&burn.program===token22&&burn.data.length===9&&burn.data[0]===8&&JSON.stringify(burn.accounts)===JSON.stringify([a[5],a[3],a[0]]),'actual quote debit and native base burn');
  assert(buy.program===amm&&boost.program===amm&&JSON.stringify(buy.accounts)===JSON.stringify([ea])&&JSON.stringify(boost.accounts)===JSON.stringify([ea])&&ba.length===512&&e.length===216&&ba.subarray(0,16).toString('hex')==='e445a52e51cb9a1d67f4521f2cf57777'&&e.subarray(0,16).toString('hex')==='e445a52e51cb9a1d3f451c16305cc2b9','two exact primary source events');
  for(const[o,role]of[[128,0],[160,7],[224,8]])assert(bs.encode(ba.subarray(o,o+32))===a[role],'underlying buy role');
  for(const[o,role]of[[24,3],[88,0],[120,1]])assert(bs.encode(e.subarray(o,o+32))===a[role],'primary boost role');
  const input=u(q.data,1),output=u(burn.data,1);assert(input>0n&&output>0n&&input<=u(b,8)&&output>=u(b,16)&&u(e,152)===u(b,8)&&u(e,160)===input&&u(e,168)===output&&u(ba,24)===output&&u(ba,32)===u(b,8)&&u(ba,72)===input&&u(ba,112)===input&&u(ba,120)===input&&u(ba,401)===u(b,16),'primary request/used/minimum/burn financial bounds');
  assert(ba.subarray(80,112).every(x=>x===0)&&ba.subarray(352,369).every(x=>x===0)&&ba.subarray(431,463).every(x=>x===0)&&ba.readUInt32LE(409)===18&&ba.subarray(413,431).toString()==='buy_exact_quote_in','zero native boost exchange fees and exact request name');
  assert(u(ba,56)-output===u(e,200)&&u(ba,64)+input===u(e,192)&&u(ba,48)-input===u(e,208)&&ba.subarray(463,479).equals(e.subarray(176,192)),'native reserve/vault balance state consistency');
  for(const[role,mint,owner]of[[5,3,0],[6,4,0],[8,4,7]])for(const balance of tx.meta.preTokenBalances.concat(tx.meta.postTokenBalances).filter(v=>keys[v.accountIndex]===a[role]))assert(balance.mint===a[mint]&&balance.owner===a[owner]&&balance.programId===a[mint===3?9:10],'all original token identity/ownership');
  if(calls.length===5)assert(calls[4].program===token&&calls[4].data.toString('hex')==='09'&&JSON.stringify(calls[4].accounts)===JSON.stringify([a[8],a[1],a[7]])&&u(e,208)===0n,'empty boost vault close and authority refund');
  legs.push({signature:sig,slot:tx.slot,protocol:79,source_index:x.index,first_transfer_index:q.index,source_burn_index:burn.index,burn_output_verified:true,transfer_verified:false,amm,pool:a[0],user:a[7],input_mint:a[4],input_amount:String(input),output_mint:a[3],output_amount:String(output),economicFormat:'boost_vault_quote_debit_and_pool_base_burn',physicalQuote:{source:q.index,program:q.program,accounts:q.accounts,amount:String(input)},physicalBurn:{source:burn.index,program:burn.program,accounts:burn.accounts,amount:String(output)}});


 }
 const implicitSettlements=[];
 assert(legs.length===1&&legs[0].source_index===row.instruction_index,'exact missing native leg');
 const nativeLedgers=[...native].map(([account,expected])=>({account,expected:String(expected),actual:String(balance(account,'postBalances')-balance(account,'preBalances'))}));assert(nativeLedgers.every(l=>l.expected===l.actual),'every modeled native account '+JSON.stringify(nativeLedgers.filter(l=>l.expected!==l.actual)));
 const tokenLedgers=[...tokens].map(([account,expected])=>{const n=keys.indexOf(account),before=tx.meta.preTokenBalances.find(t=>t.accountIndex===n),after=tx.meta.postTokenBalances.find(t=>t.accountIndex===n);assert(before||after||expected===0n&&balance(account,'preBalances')===0n&&balance(account,'postBalances')===0n&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===18&&c.accounts[0]===account)&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===9&&c.accounts[0]===account),'full principal balance witness or exact zero creation/closure');return{account,mint:mints.get(account),expected:String(expected),actual:String(BigInt(after?.uiTokenAmount.amount||0)-BigInt(before?.uiTokenAmount.amount||0))};});assert(tokenLedgers.every(l=>l.expected===l.actual),'all principal token balances: '+sig+' '+JSON.stringify(tokenLedgers.filter(l=>l.expected!==l.actual)));
 assert(keys.every((account,n)=>BigInt(tx.meta.postBalances[n])-BigInt(tx.meta.preBalances[n])===(native.get(account)||0n)),'all resolved global native account ledgers');
 proof.push({signature:sig,slot:tx.slot,rawPbSha256:hash(dir+'/'+sig+'.pb'),rpcGzipSha256:hash(dir+'/'+sig+'.rpc.json.gz'),completeRuntime:true,rpcLogsTruncated:tx.meta.logMessages.some(l=>l==='Log truncated'||/Log truncated/i.test(l)),rpcRuntimeLogCount:tx.meta.logMessages.length,originalRuntimeLogCount:original.meta.logMessages.length,legs,nativeLedgers,tokenLedgers,implicitSettlements});
}catch(e){failures.push({signature:row.signature,error:e.message,stack:e.stack});}}
fs.writeFileSync(dir+'/independent-proof.json',JSON.stringify(proof,null,2));fs.writeFileSync(dir+'/independent-failures.json',JSON.stringify(failures,null,2));console.log(JSON.stringify({failures:failures.length,errors:failures.slice(0,5),transactions:proof.length,legs:proof.flatMap(t=>t.legs).length,restored:proof.length,nativeLedgers:proof.flatMap(t=>t.nativeLedgers).length,tokenLedgers:proof.flatMap(t=>t.tokenLedgers).length}));
