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

const ownedPrograms=JSON.parse(fs.readFileSync(dir+'/owned-program-proof.json'));
for(const pin of ownedPrograms){
 const program=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+pin.program+'.program.json.gz'))),pd=JSON.parse(z.gunzipSync(fs.readFileSync(dir+'/'+pin.program+'.programdata.json.gz'))),loader='BPFLoaderUpgradeab1e11111111111111111111111';
 const p=Buffer.from(program.value.data[0],'base64'),d=Buffer.from(pd.value.data[0],'base64'),elf=fs.readFileSync(dir+'/'+pin.program+'.elf');
 assert(program.value.owner===loader&&program.value.executable&&p.length===36&&p.readUInt32LE()===2&&bs.encode(p.subarray(4))===pin.programData,'owned executable loader ProgramData binding');
 assert(pd.value.owner===loader&&!pd.value.executable&&d.readUInt32LE()===3&&u(d,4)===BigInt(pin.deployedSlot)&&d[12]===1&&d.subarray(45).equals(elf)&&hash(dir+'/'+pin.program+'.elf')===pin.elfSha256,'exact owned deployed ELF/slot/bytes');
 assert(rows.filter(r=>r.program_id===pin.program).every(r=>BigInt(r.block_num)>u(d,4)),'deployment predates finalized original');
}

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
   else if(b[0]===3||b[0]===12){const checked=b[0]===12,destination=a[checked?2:1],mint=checked?a[1]:mints.get(a[0]);assert(b.length===(checked?10:9)&&a.length>=(checked?4:3),'exact global token receipt');const n=u(b,1);assert(mints.get(a[0])===mint&&mints.get(destination)===mint,'both receipt mints');const charge=0n;add(tokens,a[0],-n);add(tokens,destination,n-charge);if(mint===sol){add(native,a[0],-n);add(native,destination,n);}}
   else if(b[0]===8){assert(b.length===9&&a.length===3&&mints.get(a[0])===a[1],'exact global typed burn');add(tokens,a[0],-u(b,1));}
   else if(b[0]===9){assert(b.length===1&&a.length===3&&mints.has(a[0])&&(mints.get(a[0])===sol||BigInt(tx.meta.preTokenBalances.find(t=>keys[t.accountIndex]===a[0])?.uiTokenAmount.amount||0)+(tokens.get(a[0])||0n)===0n),'exact token close');const n=balance(a[0],'preBalances')+(native.get(a[0])||0n);assert(n>=0n,'physical close remainder');add(native,a[0],-n);add(native,a[1],n);if(mints.get(a[0])===sol)add(tokens,a[0],-(BigInt(tx.meta.preTokenBalances.find(t=>keys[t.accountIndex]===a[0])?.uiTokenAmount.amount||0)+(tokens.get(a[0])||0n)));}
   else throw Error('unproved token op '+b[0]);
  }
  if([amm,curve].includes(x.program)&&b.toString('hex')==='f945a4da9667548a'){
   const descendants=ix.slice(x.index+1,ix.find(d=>d.index>x.index&&d.depth<=x.depth)?.index||ix.length);
   assert(a.length===4&&a[3]===x.program&&descendants.length===1&&descendants[0].depth===x.depth+1&&descendants[0].program===x.program&&descendants[0].accounts.length===1&&descendants[0].accounts[0]===a[2]&&descendants[0].data.length===88&&descendants[0].data.subarray(0,16).toString('hex')==='e445a52e51cb9a1d929fbdac925838f4'&&bs.encode(descendants[0].data.subarray(16,48))===a[0],'committed canonical close-volume event');
   assert(PublicKey.findProgramAddressSync([Buffer.from('user_volume_accumulator'),new PublicKey(a[0]).toBuffer()],new PublicKey(x.program))[0].toBase58()===a[1],'exact native owned volume PDA');
   const principal=balance(a[1],'preBalances')+(native.get(a[1])||0n);assert(principal>=0n,'exact physical rent remainder');add(native,a[1],-principal);add(native,a[0],principal);
  }

  if(x.index!==row.instruction_index)continue;
  const patch=x.program==='HVfvL7oT9k69GeZu5UwXJy7QjarqmFaVfw2H1oLyHfnF',program=x.program;
  assert(x.depth===1&&[9,48].includes(b.length),'exact root profile');
  assert(patch?b.toString('hex')==='be9f021766fb979f01'&&a.length===18:b.length===48&&b.subarray(0,8).toString('hex')==='220ff7488aa7099b'&&a.length===17&&u(b,32)===3n&&u(b,40)===0n,'primary bounded native arguments');
  const end=ix.find(c=>c.index>x.index&&c.depth<=x.depth)?.index||ix.length,calls=ix.slice(x.index+1,end);assert(calls.length===(patch?4:2)&&calls.every(c=>c.depth===2),'exact direct child frame');
  if(patch)for(const[n,c]of calls.slice(0,2).entries())assert(c.program===a[12]&&c.data.length===216&&c.data.subarray(0,8).toString('hex')==='9903b40f4934596c'&&c.data[8]===1&&c.data[9]===n+1&&JSON.stringify(c.accounts)===JSON.stringify([a[2],a[13],a[17]]),'typed pre/post hook frame');
  const q=calls[patch?2:0],o=calls[patch?3:1],input=u(q.data,1),output=u(o.data,1);
  assert(q.program===(patch?token:token22)&&q.data.length===10&&q.data[0]===12&&JSON.stringify(q.accounts)===JSON.stringify(patch?[a[8],a[4],a[6],a[3]]:[a[5],a[4],a[8],a[0]]),'actual input receipt roles');
  assert(o.program===(patch?token22:token)&&o.data.length===(patch?10:9)&&o.data[0]===(patch?12:3)&&JSON.stringify(o.accounts)===JSON.stringify(patch?[a[7],a[5],a[9],a[2]]:[a[9],a[6],a[7]]),'actual net output receipt roles');
  assert(input>0n&&output>0n&&(patch||input===u(b,8)&&output>=u(b,16)),'native requested input/minimum net receipt');
  const scoped=[];let cursor=0,stack=[];
  for(const l of tx.meta.logMessages){const w=l.split(' ');if(w[0]==='Program'&&w[2]==='invoke')stack.push(cursor++);else if(w[0]==='Program'&&['success','failed:'].includes(w[2]))stack.pop();else if(stack.at(-1)===x.index&&l.startsWith('Program data: '))scoped.push(Buffer.from(l.slice(14),'base64'));}
  const disc=patch?'3d3930844031ba7d':'452ff4ce7345aef6',events=scoped.filter(e=>e.length===(patch?120:329)&&e.subarray(0,8).toString('hex')===disc);assert(events.length===1,'source-scoped exact native event');const e=events[0];
  if(patch)assert(bs.encode(e.subarray(8,40))===a[2]&&bs.encode(e.subarray(40,72))===a[4]&&bs.encode(e.subarray(72,104))===a[5]&&u(e,104)===input&&u(e,112)===output,'primary buyback event roles and actual amounts');
  else assert(bs.encode(e.subarray(8,40))===program&&bs.encode(e.subarray(40,72))===a[4]&&bs.encode(e.subarray(72,104))===a[0]&&bs.encode(e.subarray(104,136))===a[1]&&e[136]===2&&u(e,153)===input&&u(e,137)-u(e,161)===output&&bs.encode(e.subarray(169,201))===a[11]&&bs.encode(e.subarray(249,281))===a[12],'primary sell event gross-minus-fee actual net source receipts');
  const roles=patch?[[8,sol,a[3]],[9,a[5],a[3]],[6,sol,a[2]],[7,a[5],a[2]]]:[[5,a[4],a[0]],[8,a[4],a[7]],[9,sol,a[7]]];
  for(const[role,mint,owner]of roles)for(const side of ['preTokenBalances','postTokenBalances']){const account=tx.meta[side].find(v=>keys[v.accountIndex]===a[role]);assert(account&&account.mint===mint&&account.owner===owner,'both original user/vault ownership/mint witnesses');}
  legs.push({signature:sig,slot:tx.slot,protocol:patch?84:85,source_index:x.index,first_transfer_index:q.index,transfer_verified:true,pool:a[patch?2:7],user:a[patch?3:0],input_mint:patch?sol:a[4],input_amount:String(input),output_mint:patch?a[5]:sol,output_amount:String(output),physicalInput:{source:q.index,program:q.program,accounts:q.accounts,amount:String(input)},physicalOutput:{source:o.index,program:o.program,accounts:o.accounts,amount:String(output)}});


 }
 const implicitSettlements=[];
 assert(legs.length===1&&legs[0].source_index===row.instruction_index,'exact missing native leg');
 const nativeLedgers=[...native].map(([account,expected])=>({account,expected:String(expected),actual:String(balance(account,'postBalances')-balance(account,'preBalances'))}));assert(nativeLedgers.every(l=>l.expected===l.actual),'every modeled native account '+JSON.stringify(nativeLedgers.filter(l=>l.expected!==l.actual)));
 const tokenLedgers=[...tokens].map(([account,expected])=>{const n=keys.indexOf(account),before=tx.meta.preTokenBalances.find(t=>t.accountIndex===n),after=tx.meta.postTokenBalances.find(t=>t.accountIndex===n);assert(before||after||expected===0n&&balance(account,'preBalances')===0n&&balance(account,'postBalances')===0n&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===18&&c.accounts[0]===account)&&ix.some(c=>[token,token22].includes(c.program)&&c.data[0]===9&&c.accounts[0]===account),'full principal balance witness or exact zero creation/closure');return{account,mint:mints.get(account),expected:String(expected),actual:String(BigInt(after?.uiTokenAmount.amount||0)-BigInt(before?.uiTokenAmount.amount||0))};});assert(tokenLedgers.every(l=>l.expected===l.actual),'all principal token balances: '+sig+' '+JSON.stringify(tokenLedgers.filter(l=>l.expected!==l.actual)));
 assert(keys.every((account,n)=>BigInt(tx.meta.postBalances[n])-BigInt(tx.meta.preBalances[n])===(native.get(account)||0n)),'all resolved global native account ledgers');
 proof.push({signature:sig,slot:tx.slot,rawPbSha256:hash(dir+'/'+sig+'.pb'),rpcGzipSha256:hash(dir+'/'+sig+'.rpc.json.gz'),completeRuntime:true,rpcLogsTruncated:tx.meta.logMessages.some(l=>l==='Log truncated'||/Log truncated/i.test(l)),rpcRuntimeLogCount:tx.meta.logMessages.length,originalRuntimeLogCount:original.meta.logMessages.length,legs,nativeLedgers,tokenLedgers,implicitSettlements});
}catch(e){failures.push({signature:row.signature,error:e.message,stack:e.stack});}}
fs.writeFileSync(dir+'/independent-proof.json',JSON.stringify(proof,null,2));fs.writeFileSync(dir+'/independent-failures.json',JSON.stringify(failures,null,2));console.log(JSON.stringify({failures:failures.length,errors:failures.slice(0,5),transactions:proof.length,legs:proof.flatMap(t=>t.legs).length,restored:proof.length,nativeLedgers:proof.flatMap(t=>t.nativeLedgers).length,tokenLedgers:proof.flatMap(t=>t.tokenLedgers).length}));
