//! Guacswap canonical classic-token exchange. The third CPI is an output-side
//! referral payment, excluded from the user's receipt and quoted price.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58,block_view::InstructionView};
pub(crate) const PROGRAM_ID:[u8;32]=b58!("Gswppe6ERWKpUTXvRPfXdzHhiCyJvLadVvXGfdpBqcE1");
const STATE:[u8;32]=b58!("EzV4QKycvp4EDuAoTGjyY9HUS9Zn15vcmHkHJdNKHCnY");
const AUTHORITY:[u8;32]=b58!("FRwL3qJHuT5iYTNopCyhU1tyDn1HwtHbSa1ieYF6q2m9");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||a.len()!=17||d.len()!=33||d[..8]!=[248,198,158,145,225,117,135,200]||d[32]>1
  ||a[0].0.as_slice()!=STATE||a[12].0.as_slice()!=AUTHORITY||a[13].0.as_slice()!=b58!("11111111111111111111111111111111")
  ||a[14].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||a[15].0.as_slice()!=b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
  ||a[16].0.as_slice()!=b58!("SysvarRent111111111111111111111111111111111")
  ||a[2].0==a[3].0||a[4].0==a[5].0||a[6].0==a[7].0||a[4].0==a[6].0||a[5].0==a[7].0{return None;}
 let expected_pool=solana_pubkey::Pubkey::find_program_address(&[b"guacswappoolv1",a[2].0,a[3].0],&solana_pubkey::Pubkey::new_from_array(PROGRAM_ID)).0;
 if a[1].0.as_slice()!=expected_pool.as_ref(){return None;}
 let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
 let rev=usize::from(d[32]==0);Some(Layout{protocol:Protocol::Guacswap,pool:1,authority:8,users:[6+rev,7-rev],vaults:[4+rev,5-rev],amount,minimum_output:0,exact_input:true,input_limit:true,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let rev=usize::from(ix.data()[32]==0);let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0||calls.len()!=3||!successful.contains(&source){return None;}let mut amounts=[0u64;3];
 for(n,c)in calls.iter().enumerate(){let d=c.data();let ca=c.accounts();let roles=match n{0=>[l.users[0],l.vaults[0],8],1=>[l.vaults[1],l.users[1],12],_=>[l.vaults[1],10-rev,12]};
  if c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+n as u32+1))||c.program_id().0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID
   ||d.len()!=9||d[0]!=3||ca.len()!=3||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0){return None;}
  let mint=if n==0{2+rev}else{3-rev};if mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[mint].0.as_slice())||mints.mint_for(a[roles[1]].0).as_deref()!=Some(a[mint].0.as_slice()){return None;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
 }
 // Delta-in bounds the requested debit; price only the committed principal.
 // This also accepts bounded partial receipt shapes without using the request
 // as the swap amount. All current finalized fixtures exhaust their request.
 if amounts[0]==0||amounts[0]>l.amount||amounts[1]==0||a[10-rev].0==a[l.users[1]].0{return None;}
 Some((amounts[0],amounts[1]))
}

#[cfg(test)]
mod tests{
 use super::*;use substreams_solana::{base58,pb::sf::solana::r#type::v1::ConfirmedTransaction};
 #[test]
 fn finalized_guacswap_principals_bind_pool_and_separate_fees(){
  let mut shapes=HashSet::new();let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/futarchy-guac26/cases.tsv").lines(){let f:Vec<_>=line.split('\t').collect();if f[9]!="guacswap"{continue;}count+=1;
   let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/futarchy-guac26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let source=f[1].parse::<u32>().unwrap();let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let input=walk[source as usize+1].data().clone();let output=walk[source as usize+2].data().clone();let extra=walk[source as usize+3].data().clone();
   let decoded=crate::process_transaction(tx.clone()).unwrap();let swaps:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(swaps.len(),1,"{}",f[0]);let s=swaps[0];assert!(s.transfer_verified);assert_eq!(s.protocol,Protocol::Guacswap as i32);assert_eq!(base58::encode(&s.amm_pool),f[2]);assert_eq!(base58::encode(&s.user),f[3]);assert_eq!(base58::encode(&s.input_mint),f[4]);assert_eq!(base58::encode(&s.output_mint),f[6]);assert_eq!(s.input_amount,f[5].parse::<u64>().unwrap());assert_eq!(s.output_amount,f[7].parse::<u64>().unwrap());assert_eq!(s.source_transfer_index,Some(f[8].parse::<u32>().unwrap()));
   let mut bounded=tx.clone();if let Some(n)=bounded.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions.iter_mut().find(|x|x.data==native){n.data[8..16].copy_from_slice(&(s.input_amount+1).to_le_bytes());}else{let n=bounded.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();n.data[8..16].copy_from_slice(&(s.input_amount+1).to_le_bytes());}let partial=crate::process_transaction(bounded).unwrap();let restored=partial.swaps.iter().find(|s|s.source_index==Some(source)).unwrap();assert_eq!((restored.input_amount,restored.output_amount),(s.input_amount,s.output_amount));
   if !shapes.insert((native[32],extra.len())){continue;}
   for mode in 0..30{let mut wrong=tx.clone();
    if mode<12{
     let root=wrong.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions.iter().position(|x|x.data==native);
     if let Some(i)=root{let n=&mut wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[i];let a=n.accounts.clone();match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>n.data[8..16].copy_from_slice(&(s.input_amount-1).to_le_bytes()),3=>n.data[32]=2,4=>n.accounts[1]=a[8],5=>n.accounts[4]=a[5],6=>n.accounts[5]=a[4],7=>n.accounts[6]=a[7],8=>n.accounts[7]=a[6],9=>n.accounts[14]=a[1],10=>n.accounts[12]=a[8],_=>n.accounts[15]=a[1]}}
     else{let n=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();let a=n.accounts.clone();match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>n.data[8..16].copy_from_slice(&(s.input_amount-1).to_le_bytes()),3=>n.data[32]=2,4=>n.accounts[1]=a[8],5=>n.accounts[4]=a[5],6=>n.accounts[5]=a[4],7=>n.accounts[6]=a[7],8=>n.accounts[7]=a[6],9=>n.accounts[14]=a[1],10=>n.accounts[12]=a[8],_=>n.accounts[15]=a[1]}}
    }else if mode<27{let group=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g|g.instructions.windows(3).any(|w|w[0].data==input&&w[1].data==output&&w[2].data==extra)).unwrap();let start=group.instructions.windows(3).position(|w|w[0].data==input&&w[1].data==output&&w[2].data==extra).unwrap();let n=&mut group.instructions[start+if mode<17{0}else if mode<22{1}else{2}];let a=n.accounts.clone();match mode{12|17=>n.data[1..9].fill(0),13|18|22=>n.accounts[0]=a[1],14|19|23=>n.accounts[1]=a[0],15|20|24=>n.accounts[2]=a[0],16|21|25=>n.stack_height=Some(99),_=>n.data.push(0)}}
    else if mode==27{wrong.meta.as_mut().unwrap().err=Some(Default::default());}
    else if mode==28{wrong.meta.as_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}
    else{wrong.meta.as_mut().unwrap().log_messages.clear();}
    assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"guacswap mode{mode}");
   }
  }assert_eq!(count,7);
 }
}
