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
 const sig=row.signature,raw=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+sig+'.rpc.json.gz')));const tx=raw.response.result;assert(raw.request.params[1].commitment==='finalized','finalized request');
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
   else if(b[0]===9){assert(b.length===1&&a.length===3&&mints.has(a[0])&&(mints.get(a[0])===sol||BigInt(tx.meta.preTokenBalances.find(t=>keys[t.accountIndex]===a[0])?.uiTokenAmount.amount||0)+(tokens.get(a[0])||0n)===0n),'exact token close');const n=balance(a[0],'preBalances')+(native.get(a[0])||0n);assert(n>=0n,'physical close remainder');add(native,a[0],-n);add(native,a[1],n);}
   else throw Error('unproved token op '+b[0]);
  }
  if([amm,curve].includes(x.program)&&b.toString('hex')==='f945a4da9667548a'){
   const descendants=ix.slice(x.index+1,ix.find(d=>d.index>x.index&&d.depth<=x.depth)?.index||ix.length);
   assert(a.length===4&&a[3]===x.program&&descendants.length===1&&descendants[0].depth===x.depth+1&&descendants[0].program===x.program&&descendants[0].accounts.length===1&&descendants[0].accounts[0]===a[2]&&descendants[0].data.length===88&&descendants[0].data.subarray(0,16).toString('hex')==='e445a52e51cb9a1d929fbdac925838f4'&&bs.encode(descendants[0].data.subarray(16,48))===a[0],'committed canonical close-volume event');
   assert(PublicKey.findProgramAddressSync([Buffer.from('user_volume_accumulator'),new PublicKey(a[0]).toBuffer()],new PublicKey(x.program))[0].toBase58()===a[1],'exact native owned volume PDA');
   const principal=balance(a[1],'preBalances')+(native.get(a[1])||0n);assert(principal>=0n,'exact physical rent remainder');add(native,a[1],-principal);add(native,a[0],principal);
  }
  if(x.program===curve&&b.subarray(0,8).toString('hex')==='1c92de7726c469d5'){
   assert(b.length===24&&a.length===17&&a[2]===sol&&a[3]===token22&&a[4]===token&&a[14]===system&&a[16]===curve,'exact V3 native SOL roles');
   const end=ix.find(c=>c.index>x.index&&c.depth<=x.depth)?.index||ix.length,calls=ix.slice(x.index+1,end);assert(calls.length===2&&calls.every(c=>c.depth===x.depth+1),'exact V3 direct token/event children');
   const [t,e]=calls;assert(t.program===token22&&t.data.length===10&&t.data[0]===12&&JSON.stringify(t.accounts)===JSON.stringify([a[9],a[1],a[6],a[8]])&&e.program===curve&&e.accounts.length===1&&e.accounts[0]===a[15],'canonical V3 native principal and event');
   const claim=eventParsed.find(v=>v.index===e.index)?.event;assert(claim&&claim.mint===a[1]&&claim.user===a[8]&&!claim.is_buy&&claim.quote_mint===system&&claim.quote_amount===claim.sol_amount,'canonical V3 event financial identity');
   const input=u(t.data,1),postT=u(e.data,113),postS=u(e.data,105),preT=postT-input;assert(input===u(b,8)&&input===BigInt(claim.token_amount)&&preT>0n,'exact physical V3 input');
   const low=(postS-1n)*postT/preT+1n,high=postS*postT/preT;assert(low===high,'unique independent V3 CP inverse');const gross=low-postS;assert(input*low/postT===gross&&gross===BigInt(claim.sol_amount),'computed gross state witness');
   const protocol=BigInt(claim.fee),creator=BigInt(claim.creator_fee),cb=BigInt(claim.cashback),bb=BigInt(claim.buyback_fee),net=gross-protocol-creator-cb;assert(cb===0n&&protocol===(gross*BigInt(claim.fee_basis_points)+9999n)/10000n&&creator===(gross*BigInt(claim.creator_fee_basis_points)+9999n)/10000n&&bb===protocol*BigInt(claim.buyback_fee_basis_points)/10000n&&net>=u(b,16)&&net>0n,'V3 native exact gross, retained fees and min net');
   add(native,a[5],-net-bb);add(native,a[8],net);add(native,a[13],bb);
   if(x.index===row.instruction_index)legs.push({signature:sig,slot:tx.slot,protocol:9,source_index:x.index,first_transfer_index:t.index,amm:curve,pool:a[5],user:a[8],input_mint:a[1],input_amount:String(input),output_mint:sol,output_amount:String(net),independentCP:{low:String(low),high:String(high),gross:String(gross),protocol:String(protocol),creator:String(creator),buyback:String(bb)}});
  }
  if(x.index!==row.instruction_index)continue;
  if(x.program===curve)continue;
  assert(x.program===amm&&['c62e1552b4d9e870','c2ab1c46684d5b2f','33e685a4017f83ad','5df6823ce7e940b2'].includes(b.subarray(0,8).toString('hex')),'exact AMM quote buy');
  const end=ix.find(c=>c.index>x.index&&c.depth<=x.depth)?.index||ix.length,calls=ix.slice(x.index+1,end);assert(calls.every(c=>c.depth===x.depth+1),'all direct AMM children');
  const buy=['c62e1552b4d9e870','c2ab1c46684d5b2f'].includes(b.subarray(0,8).toString('hex'));
  const receipts=calls.filter(c=>[token,token22].includes(c.program)&&c.data[0]===12),inputs=receipts.filter(c=>c.accounts[0]===a[buy?6:5]&&c.accounts[1]===a[buy?4:3]),outputs=receipts.filter(c=>c.accounts[0]===a[buy?7:8]&&c.accounts[1]===a[buy?3:4]&&c.accounts[2]===a[buy?5:6]),e=calls.at(-1);
  assert(inputs.length>0&&outputs.length===1&&inputs.every(c=>c.accounts[3]===a[1])&&outputs[0].accounts[3]===a[0]&&receipts.every(c=>inputs.includes(c)||c.accounts[3]===a[0]&&c.accounts[0]===a[8]&&c.accounts[1]===a[4]||outputs.includes(c)),'exact whole user quote debits and base receipt');
  assert(e.program===amm&&e.accounts.length===1&&e.accounts[0]===a[15]&&e.data.subarray(0,16).toString('hex')===(buy?'e445a52e51cb9a1d67f4521f2cf57777':'e445a52e51cb9a1d3e2f370aa503dc2a')&&bs.encode(e.data.subarray(128,160))===a[0]&&bs.encode(e.data.subarray(160,192))===a[1],'exact source-bound AMM event');
  const input=inputs.reduce((n,c)=>n+u(c.data,1),0n),output=u(outputs[0].data,1);assert(input>0n&&output>=u(b,16)&&(buy?input<=u(b,8)&&u(e.data,32)===u(b,8)&&u(e.data,401)===u(b,16)&&output===u(e.data,24):input===u(b,8)&&u(e.data,24)===input),'exact request budget/minimum and physical receipts');
  legs.push({signature:sig,slot:tx.slot,protocol:10,source_index:x.index,first_transfer_index:Math.min(inputs[0].index,outputs[0].index),amm,pool:a[0],user:a[1],input_mint:a[buy?4:3],input_amount:String(input),output_mint:a[buy?3:4],output_amount:String(output),physicalReceipts:receipts.map(c=>({source:c.index,program:c.program,accounts:c.accounts,amount:String(u(c.data,1))}))});

 }
 const implicitSettlements=[];
 if(ix[0].program==='FLASHX8DrLbgeR8FcfNV1F5krxYcYMUdBkrP1EPBtxB9'){
  const root=ix[0],a=root.accounts,b=root.data,program=root.program,source=ix[8];
  assert(tx.slot>455749398&&hash(dir+'/flashx/flashx.elf')==='5d879b6b0ba06b52bd80d3bec34d7878fe8450db8d40c68393810d2ca0b672db','pinned pre-original native FLASHX executable');
  assert(root.depth===1&&b.length===22&&b[0]===0&&b[17]===1&&b[18]===1&&b[19]===52&&a.length===29&&a[4]===program&&a[23]===program&&a[28]===program,'exact primary single-operation52 native profile');
  assert(ix[4].program===curve&&ix[4].data.subarray(0,8).toString('hex')==='1c92de7726c469d5'&&u(ix[4].data,8)===u(b,1)&&u(ix[4].data,16)===1n&&ix[4].accounts[8]===a[1],'exact requested native V3 settlement binding');
  const leaf=ix[7];assert(leaf.program===program&&leaf.depth===2&&leaf.data.length===82&&leaf.data[0]===7&&leaf.accounts.length===0,'committed primary leaf07 witness');
  assert(source.program===system&&source.depth===2&&source.data.length===12&&source.data.readUInt32LE()===2&&JSON.stringify(source.accounts)===JSON.stringify([a[1],a[24]]),'exact physical FLASHX fee funding');
  const amount=u(source.data,4),rate=BigInt(b[20]),referralRate=BigInt(b[21]);assert(rate<=200n&&referralRate<=120n&&balance(a[24],'preBalances')===balance(a[24],'postBalances'),'primary rate bounds and isolated fee source lifecycle');
  const product=amount*(200n-rate),base=product/200n;
  const recipients=[
    [a[25],a[25]===program?0n:base*referralRate/200n],
    [a[26],a[26]===program||product<6800n?0n:base*3n/100n],
    [a[27],a[27]===program||product<10000n?0n:product/10000n],
    [a[28],a[28]===program||product<4000n?0n:product/4000n],
  ];
  const allocated=recipients.reduce((n,r)=>n+r[1],0n),residual=amount-allocated;assert(residual>=0n,'bounded primary helper369 allocation');
  add(native,a[24],-amount);for(const [recipient,credit]of recipients)add(native,recipient,credit);add(native,a[2],residual);
  implicitSettlements.push({program,sourceRoot:0,sourceTransfer:8,source:a[24],primaryRecipient:a[2],amount:String(amount),rate:String(rate),referralRate:String(referralRate),product:String(product),base:String(base),recipients:recipients.map(([account,n])=>({account,amount:String(n)})),residual:String(residual),primaryHandler:369,primaryCaller:2746});
 }
 assert(legs.length===1&&legs[0].source_index===row.instruction_index,'exact missing native leg');
 const nativeLedgers=[...native].map(([account,expected])=>({account,expected:String(expected),actual:String(balance(account,'postBalances')-balance(account,'preBalances'))}));assert(nativeLedgers.every(l=>l.expected===l.actual),'every modeled native account '+JSON.stringify(nativeLedgers.filter(l=>l.expected!==l.actual)));
 const tokenLedgers=[...tokens].filter(([account])=>mints.get(account)!==sol).map(([account,expected])=>{const n=keys.indexOf(account),before=tx.meta.preTokenBalances.find(t=>t.accountIndex===n),after=tx.meta.postTokenBalances.find(t=>t.accountIndex===n);assert(before||after||expected===0n&&balance(account,'preBalances')===0n&&balance(account,'postBalances')===0n&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===18&&c.accounts[0]===account)&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===9&&c.accounts[0]===account),'full principal balance witness or exact zero creation/closure');return{account,mint:mints.get(account),expected:String(expected),actual:String(BigInt(after?.uiTokenAmount.amount||0)-BigInt(before?.uiTokenAmount.amount||0))};});assert(tokenLedgers.every(l=>l.expected===l.actual),'all principal token balances: '+sig+' '+JSON.stringify(tokenLedgers.filter(l=>l.expected!==l.actual)));
 assert(keys.every((account,n)=>BigInt(tx.meta.postBalances[n])-BigInt(tx.meta.preBalances[n])===(native.get(account)||0n)),'all resolved global native account ledgers');
 proof.push({signature:sig,slot:tx.slot,rawPbSha256:hash(dir+'/'+sig+'.pb'),rpcGzipSha256:hash(dir+'/'+sig+'.rpc.json.gz'),completeRuntime:true,legs,nativeLedgers,tokenLedgers,implicitSettlements});
}catch(e){failures.push({signature:row.signature,error:e.message,stack:e.stack});}}
fs.writeFileSync(dir+'/independent-proof.json',JSON.stringify(proof,null,2));fs.writeFileSync(dir+'/independent-failures.json',JSON.stringify(failures,null,2));console.log(JSON.stringify({failures:failures.length,errors:failures.slice(0,5),transactions:proof.length,legs:proof.flatMap(t=>t.legs).length,restored:proof.length,nativeLedgers:proof.flatMap(t=>t.nativeLedgers).length,tokenLedgers:proof.flatMap(t=>t.tokenLedgers).length}));
