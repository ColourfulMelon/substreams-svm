//! Native Jupiter perpetual-pool spot exchanges, from its program-owned IDL.
//! Position changes are excluded. Token-ledger input is the actual committed debit.
use crate::native_venues::Layout;
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{b58, block_view::InstructionView};
use std::collections::HashSet;
pub(crate) const PROGRAM_ID: [u8;32] = b58!("PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu");
const TRANSFER_AUTHORITY: [u8;32] = b58!("AVzP2GeRmqGphJsMxWoqjpUifPpCret7LqWhD8NWQK49");
const EVENT_AUTHORITY: [u8;32] = b58!("37hJBDnntwqhGbK7L6M1bLyvccj4u55CCUiLPdYkiqBN");
const PRESWAP: [u8;8] = [197,38,86,165,199,23,38,234];
const LEDGER: [u8;8] = [139,141,238,197,41,211,172,19];
const SWAP2: [u8;8] = [65,75,63,76,235,91,91,136];
pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
 let d=ix.data(); let a=ix.accounts();
 if ix.program_id().0.as_slice()!=PROGRAM_ID || d.len()<8 {return None;}
 let ledger=d[..8]==LEDGER; let swap2=d[..8]==SWAP2;
 let token=if swap2 {14} else if ledger {13} else {12};
 let event=if swap2 {15} else if ledger {15} else {14};
 if (!ledger && !swap2 && d[..8]!=PRESWAP) || d.len()!=if ledger {16}else{24}
  || a.len()!=if ledger||swap2 {17}else{16}
  || a[3].0.as_slice()!=TRANSFER_AUTHORITY || a[token].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID
  || a[event].0.as_slice()!=EVENT_AUTHORITY || a[event+1].0.as_slice()!=PROGRAM_ID
  || !swap2 && a[token+1].0.as_slice()!=b58!("Sysvar1nstructions1111111111111111111111111")
  || a[1].0==a[2].0 || a[6].0==a[if swap2 {10}else{9}].0
 {return None;}
 let amount=if ledger {u64::MAX}else{u64::from_le_bytes(d[8..16].try_into().ok()?)};
 if amount==0{return None;}
 Some(Layout{protocol:Protocol::JupiterPerps,pool:5,authority:0,users:[1,2],vaults:if swap2 {[9,13]}else{[8,11]},amount,
 minimum_output:u64::from_le_bytes(d[if ledger {8}else{16}..].try_into().ok()?),exact_input:!ledger,input_limit:ledger,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,ix:&InstructionView,calls:&[InstructionView],successful:&HashSet<u32>)->Option<(u64,u64)> {
 let l=layout(ix)?; let a=ix.accounts();let swap2=ix.data()[..8]==SWAP2;
 let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0 || calls.len()!=3 || !successful.contains(&source)
  || calls.iter().enumerate().any(|(n,c)|c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+1+n as u32))){return None;}
 let mut amounts=[0u64;2];
 for (n,c) in calls[..2].iter().enumerate(){let ca=c.accounts();let d=c.data();let roles=if n==0 {[1,l.vaults[0],0]}else{[l.vaults[1],2,3]};
  if c.program_id().0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||ca.len()!=3||d.len()!=9||d[0]!=3
   ||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0){return None;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
 }
 if amounts[0]==0||amounts[1]==0||amounts[1]<l.minimum_output||l.exact_input&&amounts[0]!=l.amount{return None;}
 let event=&calls[2];let e=event.data();let ea=event.accounts();
 // Anchor emit_cpi + PoolSwapEvent: custody keys, pool, gross input/output,
 // USD value, actual output after fees, fee bps, owner and receiving account.
 if event.program_id().0.as_slice()!=PROGRAM_ID||ea.len()!=1||ea[0].0.as_slice()!=EVENT_AUTHORITY||e.len()!=216
  ||e[..16]!=[228,69,165,46,81,203,154,29,40,107,212,26,223,136,39,220]
  ||e[16..48]!=*a[6].0||e[48..80]!=*a[if swap2 {10}else{9}].0||e[80..112]!=*a[5].0
  ||u64::from_le_bytes(e[112..120].try_into().ok()?)!=amounts[0]
  ||u64::from_le_bytes(e[136..144].try_into().ok()?)!=amounts[1]
  ||u64::from_le_bytes(e[120..128].try_into().ok()?)<amounts[1]
  ||u64::from_le_bytes(e[144..152].try_into().ok()?)>10000
  ||e[152..184]!=*a[0].0||e[184..216]!=*a[2].0{return None;}
 Some((amounts[0],amounts[1]))
}
#[cfg(test)]
mod tests{
 use super::*;use substreams_solana::{base58,pb::sf::solana::r#type::v1::ConfirmedTransaction};
 #[test]
 fn finalized_custody_swaps_bind_native_events_and_ledger_principals(){
  let mut shapes=HashSet::new();let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/custody-numeraire26/cases.tsv").lines(){
   let f:Vec<_>=line.split('\t').collect();if f[9]!="jupiter_perps"{continue;}count+=1;
   let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/custody-numeraire26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();
   let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let source=f[1].parse::<u32>().unwrap();
   let walk:Vec<_>=tx.walk_instructions().collect();let ix=&walk[source as usize];let native=ix.data().clone();
   let expected=(f[5].parse::<u64>().unwrap(),f[7].parse::<u64>().unwrap());
   assert_eq!(settlement(source,ix,&walk[source as usize+1..source as usize+4],&crate::logs::successful_invocations(&tx)),Some(expected));
   let decoded=crate::process_transaction(tx.clone()).unwrap();let swaps:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();
   assert_eq!(swaps.len(),1,"{}",f[0]);let s=swaps[0];assert_eq!(s.protocol,Protocol::JupiterPerps as i32);assert!(s.transfer_verified);
   assert_eq!((s.input_amount,s.output_amount),expected);assert_eq!(base58::encode(&s.amm_pool),f[2]);assert_eq!(base58::encode(&s.user),f[3]);assert_eq!(base58::encode(&s.input_mint),f[4]);assert_eq!(base58::encode(&s.output_mint),f[6]);assert_eq!(s.source_transfer_index,Some(f[8].parse::<u32>().unwrap()));
   if !shapes.insert((native[..8].to_vec(),f[4].to_owned())){continue;}
   let input=walk[source as usize+1].data().clone();let output=walk[source as usize+2].data().clone();let event=walk[source as usize+3].data().clone();
   for mode in 0..30{
    let mut wrong=tx.clone();
    if mode<12{let n=wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions.iter_mut().find(|x|x.data==native).unwrap();let a=n.accounts.clone();
     match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>{n.data.pop();},3=>{if n.data.len()==16{n.data[8..16].copy_from_slice(&(expected.1+1).to_le_bytes());}else{n.data[8]^=1;}},4=>n.accounts[3]=a[0],5=>n.accounts[5]=a[0],6=>n.accounts[6]=a[9],7=>n.accounts[8]=a[11],8=>n.accounts[9]=a[6],9=>n.accounts[11]=a[8],10=>n.accounts[a.len()-2]=a[0],_=>n.accounts[a.len()-1]=a[0]}
    }else if mode<27{let target=if mode<17{&input}else if mode<22{&output}else{&event};let n=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g|g.instructions.iter().any(|x|x.data==event)).unwrap().instructions.iter_mut().find(|x|x.data==*target).unwrap();
     match mode{12|17=>n.data[1]^=1,13|18=>n.accounts[0]=n.accounts[1],14|19=>n.accounts[1]=n.accounts[0],15|20=>n.accounts[2]=n.accounts[0],16|21=>n.stack_height=Some(99),22=>n.data[80]^=1,23=>n.data[112]^=1,24=>n.data[136]^=1,25=>n.data[152]^=1,_=>n.data[184]^=1}
    }else if mode==27{wrong.meta.as_mut().unwrap().err=Some(Default::default());}
    else if mode==28{wrong.meta.as_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}
    else{wrong.meta.as_mut().unwrap().log_messages.clear();}
    assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"shape {:?} rejected mode{mode}",&native[..8]);
   }
  }
  assert_eq!(count,24);
 }
}
