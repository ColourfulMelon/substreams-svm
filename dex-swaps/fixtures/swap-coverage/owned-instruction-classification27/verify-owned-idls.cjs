const fs=require('fs'),path=require('path'),zlib=require('zlib'),crypto=require('crypto'),{createRequire}=require('module');
const dir=process.argv[2],dependency=process.argv[3];if(!dir||!dependency)throw Error('Usage: node verify-owned-idls.cjs IDL_DIRECTORY NODE_PACKAGE_JSON');
const {PublicKey}=createRequire(path.resolve(dependency))('@solana/web3.js'),hash=b=>crypto.createHash('sha256').update(b).digest('hex'),assert=(v,s)=>{if(!v)throw Error(s)};
const manifest=JSON.parse(fs.readFileSync(path.join(dir,'manifest.json'))),proof=[];
for(const r of manifest.filter(x=>x.ownedIdl)){const b=fs.readFileSync(path.join(dir,r.program+'.account.json')),a=JSON.parse(b),p=new PublicKey(r.program),signer=PublicKey.findProgramAddressSync([],p)[0],expected=PublicKey.createWithSeedSync?PublicKey.createWithSeedSync(signer,'anchor:idl',p):null;
const [data,encoding]=a.value.data;assert(encoding==='base64'&&a.value.owner===r.program&&!a.value.executable,'owned primary IDL');
const seed=Buffer.from('anchor:idl'),address=new PublicKey(crypto.createHash('sha256').update(Buffer.concat([signer.toBuffer(),seed,p.toBuffer()])).digest()).toBase58();assert(a.address===address&&r.address===address,'owned IDL PDA+seed');
const raw=Buffer.from(data,'base64'),compressed=raw.subarray(44,44+raw.readUInt32LE(40)),json=zlib.inflateSync(compressed),saved=fs.readFileSync(path.join(dir,r.program+'.idl.json'));assert(hash(saved)===r.sha256,'retained manifest IDL hash');assert(JSON.stringify(JSON.parse(json))===JSON.stringify(JSON.parse(saved)),'owned compressed IDL semantic parity');
proof.push({program:r.program,address,readSlot:a.context.slot,owner:a.value.owner,accountSha256:hash(b),accountDataSha256:hash(raw),idlSha256:hash(saved),decompressedSha256:hash(json),instructionCount:JSON.parse(json).instructions.length});}
fs.writeFileSync(path.join(path.dirname(dir),'owned-idl-proof.json'),JSON.stringify(proof,null,2)+'\n');console.log(JSON.stringify({ownedIdlsVerified:proof.length}));
const bs=createRequire(path.resolve(dependency))('bs58').default||createRequire(path.resolve(dependency))('bs58');
for(const entry of JSON.parse(fs.readFileSync(path.join(dir,'owned-program-proof.json')))){
 const prog=JSON.parse(fs.readFileSync(path.join(dir,entry.program+'.program.json'))).value,data=JSON.parse(fs.readFileSync(path.join(dir,entry.program+'.programdata.json'))).value,pb=Buffer.from(prog.data[0],'base64'),db=Buffer.from(data.data[0],'base64'),elf=fs.readFileSync(path.join(dir,entry.program+'.elf'));
 assert(prog.executable&&prog.owner==='BPFLoaderUpgradeab1e11111111111111111111111'&&pb.readUInt32LE(0)===2&&bs.encode(pb.subarray(4,36))===entry.programData,'owned loader binding');
 assert(!data.executable&&data.owner===prog.owner&&db.readUInt32LE(0)===3&&db.readBigUInt64LE(4).toString()===entry.deployedSlot,'owned ProgramData binding');
 assert(hash(db.subarray(45))===hash(elf)&&hash(elf)===entry.elfSha256&&Number(entry.deployedSlot)<456213447,'prior deployed owned executable hash');
}
