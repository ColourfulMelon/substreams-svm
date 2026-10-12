//! Futarchy DAO's native spot swaps. Conditional outcome conversions and
//! liquidity operations are separate instructions and are not emitted here.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58,block_view::InstructionView};
pub(crate) const PROGRAM_ID:[u8;32]=b58!("FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq");
const EVENT_AUTHORITY:[u8;32]=b58!("DGEympSS4qLvdr9r3uGHTfACdN8snShk4iGdJtZPxuBC");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||a.len()!=9||d.len()!=25||d[..8]!=[167,97,12,231,237,78,166,251]||d[16]>1
  ||a[6].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||a[7].0.as_slice()!=EVENT_AUTHORITY||a[8].0.as_slice()!=PROGRAM_ID
  ||a[..6].iter().enumerate().any(|(n,k)|a[..n].iter().any(|o|k.0==o.0)){return None;}
 let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
 let buy=d[16]==0;Some(Layout{protocol:Protocol::Futarchy,pool:0,authority:5,users:if buy{[2,1]}else{[1,2]},vaults:if buy{[4,3]}else{[3,4]},amount,minimum_output:u64::from_le_bytes(d[17..25].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0||calls.len()!=3||!successful.contains(&source)||calls.iter().enumerate().any(|(n,c)|c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+n as u32+1))){return None;}
 let mut amounts=[0u64;2];for(n,c)in calls[..2].iter().enumerate(){let d=c.data();let ca=c.accounts();let roles=if n==0{[l.users[0],l.vaults[0],5]}else{[l.vaults[1],l.users[1],0]};
  if c.program_id().0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||d.len()!=9||d[0]!=3||ca.len()!=3||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0){return None;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
 }
 if amounts[0]!=l.amount||amounts[1]==0||amounts[1]<l.minimum_output{return None;}
 let e=calls[2].data();let ea=calls[2].accounts();
 if calls[2].program_id().0.as_slice()!=PROGRAM_ID||ea.len()!=1||ea[0].0.as_slice()!=EVENT_AUTHORITY||!matches!(e.len(),406|670)
  ||e[..16]!=[228,69,165,46,81,203,154,29,29,253,121,255,82,60,148,195]
  ||e[40..72]!=*a[0].0||e[72..104]!=*a[5].0||e[104]!=ix.data()[16]||e[129]>1||e.len()!=if e[129]==0{406}else{670}
  ||u64::from_le_bytes(e[105..113].try_into().ok()?)!=amounts[0]||u64::from_le_bytes(e[113..121].try_into().ok()?)!=amounts[1]
  ||u64::from_le_bytes(e[121..129].try_into().ok()?)!=l.minimum_output{return None;}
 let tail=e.len()-128;
 if mints.mint_for(a[3].0).as_deref()!=Some(&e[tail..tail+32])||mints.mint_for(a[4].0).as_deref()!=Some(&e[tail+32..tail+64])
  ||e[tail+64..tail+96]!=*a[3].0||e[tail+96..]!=*a[4].0{return None;}
 Some((amounts[0],amounts[1]))
}

#[cfg(test)]
mod tests{
 use super::*;use substreams_solana::{base58,pb::sf::solana::r#type::v1::ConfirmedTransaction};
 #[test]
 fn finalized_futarchy_principals_bind_pool_and_separate_fees(){
  let mut shapes=HashSet::new();let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/futarchy-guac26/cases.tsv").lines(){let f:Vec<_>=line.split('\t').collect();if f[9]!="futarchy"{continue;}count+=1;
   let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/futarchy-guac26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let source=f[1].parse::<u32>().unwrap();let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let input=walk[source as usize+1].data().clone();let output=walk[source as usize+2].data().clone();let extra=walk[source as usize+3].data().clone();
   let decoded=crate::process_transaction(tx.clone()).unwrap();let swaps:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(swaps.len(),1,"{}",f[0]);let s=swaps[0];assert!(s.transfer_verified);assert_eq!(s.protocol,Protocol::Futarchy as i32);assert_eq!(base58::encode(&s.amm_pool),f[2]);assert_eq!(base58::encode(&s.user),f[3]);assert_eq!(base58::encode(&s.input_mint),f[4]);assert_eq!(base58::encode(&s.output_mint),f[6]);assert_eq!(s.input_amount,f[5].parse::<u64>().unwrap());assert_eq!(s.output_amount,f[7].parse::<u64>().unwrap());assert_eq!(s.source_transfer_index,Some(f[8].parse::<u32>().unwrap()));
   if !shapes.insert((native[16],extra.len())){continue;}
   for mode in 0..30{let mut wrong=tx.clone();
    if mode<12{
     let root=wrong.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions.iter().position(|x|x.data==native);
     if let Some(i)=root{let n=&mut wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[i];let a=n.accounts.clone();match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>n.data[8]^=1,3=>n.data[16]=2,4=>n.accounts[0]=a[5],5=>n.accounts[3]=a[4],6=>n.accounts[4]=a[3],7=>n.accounts[1]=a[2],8=>n.accounts[2]=a[1],9=>n.accounts[6]=a[0],10=>n.accounts[7]=a[5],_=>n.accounts[8]=a[0]}}
     else{let n=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();let a=n.accounts.clone();match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>n.data[8]^=1,3=>n.data[16]=2,4=>n.accounts[0]=a[5],5=>n.accounts[3]=a[4],6=>n.accounts[4]=a[3],7=>n.accounts[1]=a[2],8=>n.accounts[2]=a[1],9=>n.accounts[6]=a[0],10=>n.accounts[7]=a[5],_=>n.accounts[8]=a[0]}}
    }else if mode<27{let group=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g|g.instructions.windows(3).any(|w|w[0].data==input&&w[1].data==output&&w[2].data==extra)).unwrap();let start=group.instructions.windows(3).position(|w|w[0].data==input&&w[1].data==output&&w[2].data==extra).unwrap();let n=&mut group.instructions[start+if mode<17{0}else if mode<22{1}else{2}];let a=n.accounts.clone();match mode{12|17=>n.data[1]^=1,13|18=>n.accounts[0]=a[1],14|19=>n.accounts[1]=a[0],15|20=>n.accounts[2]=a[0],16|21=>n.stack_height=Some(99),22=>n.data[40]^=1,23=>n.data[72]^=1,24=>n.data[105]^=1,25=>n.data[113]^=1,_=>n.data[121]^=1}}
    else if mode==27{wrong.meta.as_mut().unwrap().err=Some(Default::default());}
    else if mode==28{wrong.meta.as_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}
    else{wrong.meta.as_mut().unwrap().log_messages.clear();}
    assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"futarchy {} mode{mode}",f[0]);
   }
  }assert_eq!(count,9);
 }
}
