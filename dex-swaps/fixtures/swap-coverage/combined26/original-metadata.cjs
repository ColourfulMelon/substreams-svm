// Verify immutable original transaction PB against finalized RPC, and containing getBlock.
// This script decodes no venue ABI and obtains no financial amount from the indexer.
const fs=require('node:fs'),crypto=require('node:crypto'),zlib=require('node:zlib'),path=require('node:path');
const resource=__dirname,argv=process.argv.slice(2);
if(argv.includes('--help')){console.log('node original-metadata.cjs INPUT BLOCK_CACHE OUTPUT --node-dependency-package PACKAGE_JSON --protobuf-package PACKAGE_JSON [--prepare]');process.exit(0);}
const [inputFile,blockCache,outFile]=argv,flags={};for(let n=3;n<argv.length;n++){if(argv[n]==='--prepare')continue;if(!['--node-dependency-package','--protobuf-package'].includes(argv[n])||!argv[n+1])throw Error('Invalid argument '+argv[n]);flags[argv[n]]=path.resolve(argv[++n]);}
if(!flags['--node-dependency-package']||!flags['--protobuf-package'])throw Error('Both dependency package paths required');
const dependencyPackage=flags['--node-dependency-package'],protobufPackage=flags['--protobuf-package'],r=require('node:module').createRequire(dependencyPackage),bs=r('bs58').default||r('bs58'),pb=require('node:module').createRequire(protobufPackage)(path.dirname(protobufPackage));
const type=pb.parse(fs.readFileSync(path.join(resource,'solana-type.proto'),'utf8')).root.lookupType('sf.solana.type.v1.ConfirmedTransaction');
const phase=argv.includes('--prepare')?'--prepare':null,input=JSON.parse(fs.readFileSync(inputFile)),hash=b=>crypto.createHash('sha256').update(b).digest('hex'),eq=(a,b,m)=>{if(JSON.stringify(a)!==JSON.stringify(b))throw Error(m)},assert=(v,m)=>{if(!v)throw Error(m)},decodeJson=b=>JSON.parse(b.toString(),(k,v,c)=>typeof v==='number'&&!Number.isSafeInteger(v)&&c?.source?c.source:v),enc=a=>a.map(x=>bs.encode(x)),str=n=>String(n);
const precompiles=new Set(['Ed25519SigVerify111111111111111111111111111','KeccakSecp256k11111111111111111111111111111','Secp256r1SigVerify1111111111111111111111111']);
function runtime(walk,logs){
 let cursor=0;const frames=[],closed=[],committed=new Array(walk.length).fill(false),ancestors=new Array(walk.length).fill(null).map(()=>[]);
 for(const line of logs){
  const invoke=/^Program (\S+) invoke \[(\d+)\]$/.exec(line),success=/^Program (\S+) success$/.exec(line),failed=/^Program (\S+) failed:/.exec(line);
  if(invoke){
   const depth=Number(invoke[2]);assert(depth===frames.length+1,'runtime depth/ancestry mismatch');
   while(cursor<walk.length&&precompiles.has(walk[cursor].program)&&walk[cursor].depth===1)cursor++;
   const ix=walk[cursor];assert(ix&&ix.program===invoke[1]&&ix.depth===depth,'compiled invocation/runtime mismatch at '+cursor);
   ancestors[cursor]=frames.map(x=>x.index);frames.push({index:cursor,program:invoke[1],parents:[...frames]});cursor++;
  } else if(success||failed){
   const f=frames.pop();assert(f&&f.program===(success||failed)[1],'runtime return/ancestry mismatch');f.success=!!success;closed.push(f);
  }
 }
 assert(frames.length===0&&!logs.includes('Log truncated'),'retained original PB runtime incomplete');
 while(cursor<walk.length&&precompiles.has(walk[cursor].program)&&walk[cursor].depth===1)cursor++;
 assert(cursor===walk.length,'compiled calls absent from retained original runtime');
 for(const f of closed)committed[f.index]=f.success&&f.parents.every(x=>x.success);
 return {committed,ancestors};
}
const transactions=[],blocks=new Map();let completed=0;
for(const item of input.originals){
 try{
  const raw=fs.readFileSync(item.pbPath),rpcBytes=fs.readFileSync(item.rpcPath),tx=type.decode(raw),rpcRecord=decodeJson(item.rpcPath.endsWith('.gz')?zlib.gunzipSync(rpcBytes):rpcBytes),rpc=rpcRecord.response?.result||rpcRecord.result||rpcRecord,m=tx.transaction.message,s=tx.meta,msg=rpc.transaction.message,meta=rpc.meta;
  if(rpcRecord.request){assert(rpcRecord.request.method==='getTransaction'&&rpcRecord.request.params[0]===item.signature&&rpcRecord.request.params[1].commitment==='finalized'&&!rpcRecord.response.error,'transaction request was not finalized original identity');}
  if(item.sourceJsonPath){
   const sourceBytes=zlib.gunzipSync(fs.readFileSync(item.sourceJsonPath)),source=decodeJson(sourceBytes);assert(hash(sourceBytes)===item.sourceJsonSha256&&source.sourceBlock===item.slot&&source.sourceModule===item.originalSourceStream,'immutable original source JSON mismatch');
   const opts={bytes:String,longs:String,enums:Number,defaults:true};eq(type.toObject(tx,opts),type.toObject(type.fromObject(source.sourceTransaction),opts),'canonical original source JSON/PB semantic parity mismatch');
  }
  assert(hash(raw)===item.pbSha256&&hash(rpcBytes)===item.rpcSha256,'original hash changed');assert(!s.err&&!meta.err,'failed transaction');assert(rpc.slot===item.slot,'finalized transaction slot mismatch');
  const signatures=enc(tx.transaction.signatures),staticKeys=enc(m.accountKeys),keys=staticKeys.concat(enc(s.loadedWritableAddresses),enc(s.loadedReadonlyAddresses)),rpcKeys=msg.accountKeys.concat(meta.loadedAddresses?.writable||[],meta.loadedAddresses?.readonly||[]);
  eq(signatures,rpc.transaction.signatures,'original signatures mismatch');assert(signatures[0]===item.signature,'original signature mismatch');eq(keys,rpcKeys,'original resolved account keys mismatch');eq([m.header.numRequiredSignatures,m.header.numReadonlySignedAccounts,m.header.numReadonlyUnsignedAccounts],[msg.header.numRequiredSignatures,msg.header.numReadonlySignedAccounts,msg.header.numReadonlyUnsignedAccounts],'original header mismatch');assert(bs.encode(m.recentBlockhash)===msg.recentBlockhash,'original recent blockhash mismatch');
  assert(m.versioned===(typeof rpc.version==='number'),'original transaction versioned flag mismatch');if(m.version!=null)assert(m.version===rpc.version,'retained original transaction version number mismatch');eq(m.addressTableLookups.map(x=>({accountKey:bs.encode(x.accountKey),writableIndexes:[...x.writableIndexes],readonlyIndexes:[...x.readonlyIndexes]})),msg.addressTableLookups||[],'original address table lookup mismatch');
  const groups=new Map(s.innerInstructions.map(g=>[g.index,g.instructions])),rpcGroups=new Map(meta.innerInstructions.map(g=>[g.index,g.instructions])),walk=m.instructions.flatMap((ix,n)=>[{...ix,stackHeight:1},...(groups.get(n)||[])]).map((ix,index)=>({index,depth:ix.stackHeight,program:keys[ix.programIdIndex],accounts:[...(ix.accounts||[])].map(n=>keys[n]),data:Buffer.from(ix.data||[]).toString('hex')})),rpcWalk=msg.instructions.flatMap((ix,n)=>[{...ix,stackHeight:1},...(rpcGroups.get(n)||[])]).map((ix,index)=>({index,depth:ix.stackHeight,program:rpcKeys[ix.programIdIndex],accounts:ix.accounts.map(n=>rpcKeys[n]),data:Buffer.from(bs.decode(ix.data)).toString('hex')}));eq(walk,rpcWalk,'full original instruction/depth/data mismatch');
  eq(s.preBalances.map(str),meta.preBalances.map(str),'pre lamport mismatch');eq(s.postBalances.map(str),meta.postBalances.map(str),'post lamport mismatch');
  for(const side of ['pre','post']){
   const normalize=a=>a.map(x=>({index:x.accountIndex,mint:x.mint,amount:x.uiTokenAmount.amount,decimals:x.uiTokenAmount.decimals,owner:x.owner||'',program:x.programId||''}));eq(normalize(s[side+'TokenBalances']),normalize(meta[side+'TokenBalances']),'original '+side+' token balances mismatch');
  }
  assert(str(s.fee)===str(meta.fee)&&str(s.computeUnitsConsumed||0)===str(meta.computeUnitsConsumed||0),'original fee/gas mismatch');
  const cutoff=meta.logMessages.indexOf('Log truncated'),prefix=cutoff<0?meta.logMessages:meta.logMessages.slice(0,cutoff);eq(prefix,s.logMessages.slice(0,prefix.length),'finalized RPC runtime prefix mismatch');assert(cutoff>=0||prefix.length===s.logMessages.length,'full original runtime mismatch');let retainedTail=0;if(cutoff>=0){let after=prefix.length;for(const line of meta.logMessages.slice(cutoff+1)){assert(line!=='Log truncated','multiple truncation markers');const at=s.logMessages.indexOf(line,after);assert(at>=after,'retained RPC runtime tail not in complete original PB sequence');after=at+1;retainedTail++;}}const committed=runtime(walk,s.logMessages);
  if(!blocks.has(item.slot)){
   const file=path.join(blockCache,item.slot+'.json.gz'); if(!fs.existsSync(file)&&phase==='--prepare'){blocks.set(item.slot,{blockhash:null,blockTime:rpc.blockTime,signaturePositions:new Map([[item.signature,null]]),compressedResponseSha256:null,preparationOnly:true});} else {const bytes=fs.readFileSync(file),record=decodeJson(zlib.gunzipSync(bytes));assert(record.request.method==='getBlock'&&record.request.params[0]===item.slot&&record.request.params[1].commitment==='finalized'&&record.request.params[1].transactionDetails==='signatures','block request is not finalized signature order proof');assert(!record.response.error&&record.response.id===item.slot,'block RPC error');const b=record.response.result;assert(b.blockhash&&b.blockTime&&Array.isArray(b.signatures),'missing finalized block identity');assert(new Set(b.signatures).size===b.signatures.length,'duplicate finalized block signature');blocks.set(item.slot,{...b,signaturePositions:new Map(b.signatures.map((s,n)=>[s,n])),compressedResponseSha256:hash(bytes)});}
  }
  let b=blocks.get(item.slot);if(b.preparationOnly)b={...b,signaturePositions:new Map([[item.signature,null]])};assert(b.signaturePositions.has(item.signature),'original absent from finalized block');assert(b.blockTime===rpc.blockTime,'containing block and transaction finalized time mismatch');
  transactions.push({signature:item.signature,fee_payer:staticKeys[0],signers_raw:staticKeys.slice(0,m.header.numRequiredSignatures).join(','),fee:str(meta.fee),compute_units_consumed:str(meta.computeUnitsConsumed||0),block_num:item.slot,block_hash:b.blockhash,timestamp:b.blockTime,transaction_index:b.signaturePositions.get(item.signature),instructions:walk,...committed,originalLogs:s.logMessages,pbSha256:hash(raw),rpcSha256:hash(rpcBytes),blockRpcSha256:b.compressedResponseSha256,finalizedTransactionVersion:rpc.version,originalPbVersionNumberRetained:m.version!=null,finalizedRpcRuntimeComplete:cutoff<0,finalizedRpcRuntimeRetainedTailLines:retainedTail,finalizedRpcRuntimeRetainedTailVerified:true,retainedPBRuntimeComplete:true,fullInstructionAndBalanceParity:true});
  completed++;
 }catch(e){throw Error(item.signature+': '+e.message)}
}
fs.writeFileSync(outFile,JSON.stringify({phase:phase==='--prepare'?'PREPARATION ONLY':'finalized original metadata',scope:phase==='--prepare'?'Full original PB/finalized transaction RPC parity; containing block metadata is explicitly incomplete if cache is absent':'Full original PB/finalized transaction RPC and finalized containing block signature order/time/hash parity; all native ancestry derived from complete original runtime',missingFinalizedBlockSlots:[...blocks].filter(([slot,b])=>b.preparationOnly).map(([slot])=>slot),schemaSha256:hash(fs.readFileSync(path.join(resource,'solana-type.proto'))),finalizedBlocks:[...blocks.values()].filter(b=>!b.preparationOnly).length,transactions},null,2)+'\n');
console.log(JSON.stringify({originalMetadataVerified:completed,finalizedBlocks:[...blocks.values()].filter(b=>!b.preparationOnly).length,output:outFile}));
