//! Program-owned AnchorSwap's canonical Swap handler. The third transfer is
//! a separately declared output fee; price only the user's net receipt.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58,block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
pub(crate) const PROGRAM_ID:[u8;32]=b58!("9KuRi5CasX6XWCK846nhfRQmW32g6uuFdfaBkfxjgMpG");
const CONFIG:[u8;32]=b58!("6X3EXxG9sMNETi5A9pQXJzYm9PDLR8VjciNczPUrp2Cf");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||a.len()!=15||d.len()!=24||d[..8]!=[248,198,158,145,225,117,135,200]
  ||a[0].0.as_slice()!=CONFIG||a[11].0.as_slice()!=b58!("SysvarRent111111111111111111111111111111111")
  ||a[12].0.as_slice()!=b58!("11111111111111111111111111111111")
  ||a[13].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||a[14].0.as_slice()!=b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
  ||a[2].0==a[3].0||a[4].0==a[5].0||a[6].0==a[7].0||a[4].0==a[6].0||a[5].0==a[7].0||a[9].0==a[7].0{return None;}
 let pool=solana_pubkey::Pubkey::find_program_address(&[b"liquidity_pool",a[2].0,a[3].0],&solana_pubkey::Pubkey::new_from_array(PROGRAM_ID)).0;
 if a[1].0.as_slice()!=pool.as_ref(){return None;}
 for (role,owner,mint) in [(4,1,2),(5,1,3),(9,8,3)] {
  let ata=solana_pubkey::Pubkey::find_program_address(&[a[owner].0,&substreams_solana_idls::spl::token::PROGRAM_ID,a[mint].0],&solana_pubkey::Pubkey::new_from_array(b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"))).0;
  if a[role].0.as_slice()!=ata.as_ref(){return None;}
 }
 let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
 Some(Layout{protocol:Protocol::AnchorSwap,pool:1,authority:10,users:[6,7],vaults:[4,5],amount,minimum_output:u64::from_le_bytes(d[16..24].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
 if calls.len()!=3||depth==0||!successful.contains(&source){return None;}let mut amounts=[0u64;3];
 for(n,c)in calls.iter().enumerate(){let ca=c.accounts();let d=c.data();let roles=match n{0=>[6,4,10],1=>[5,7,1],_=>[5,9,1]};let mint=if n==0{2}else{3};
  if c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+n as u32+1))||c.program_id().0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID
   ||d.len()!=9||d[0]!=3||ca.len()!=3||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
   ||mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[mint].0.as_slice())||mints.mint_for(a[roles[1]].0).as_deref()!=Some(a[mint].0.as_slice()){return None;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
 }
    let meta=tx.meta.as_ref()?;let resolved=tx.resolved_accounts();
    for role in [4,5] {
        let address=a[role].0;let position=resolved.iter().position(|key|*key==address)?;
        let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;
        let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
        if before.mint!=after.mint || before.owner!=substreams_solana::base58::encode(a[1].0) || after.owner!=before.owner { return None; }
        let mut change=0i128;
        for (position,c) in tx.walk_instructions().enumerate() {
            let ca=c.accounts();let d=c.data();
            if ![substreams_solana_idls::spl::token::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
                || !ca.iter().any(|key|key.0==address) { continue; }
            if !successful.contains(&(position as u32)) { return None; }
            let checked=match (d.first(),d.len()) { (Some(3),9)=>false,(Some(12),10)=>true,_=>return None };
            if ca.len()<if checked{4}else{3} { return None; }
            let destination=ca[if checked{2}else{1}].0;
            if ca[0].0!=address && destination!=address { return None; }
            let mint=mints.mint_for(address)?;
            if mints.mint_for(ca[0].0).as_deref()!=Some(mint.as_slice())
                || mints.mint_for(destination).as_deref()!=Some(mint.as_slice())
                || checked && ca[1].0.as_slice()!=mint.as_slice() { return None; }
            let amount=i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?));
            if ca[0].0==address { change=change.checked_sub(amount)?; }
            if destination==address { change=change.checked_add(amount)?; }
        }
        let before=before.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?;
        let after=after.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?;
        if i128::from(after)-i128::from(before)!=change { return None; }
    }
 if amounts[0]!=l.amount||amounts[1]==0||amounts[1]<l.minimum_output{return None;}Some((amounts[0],amounts[1]))
}

#[cfg(test)]
mod tests{
 use super::*;use substreams_solana::base58;
 #[test]
 fn finalized_anchor_swap_binds_pool_fee_and_all_vault_receipts(){
  let f:Vec<_>=include_str!("../fixtures/swap-coverage/anchor-swap-native26/cases.tsv").lines().next().unwrap().split('\t').collect();let source=f[1].parse::<u32>().unwrap();
  let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/anchor-swap-native26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
  let d=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=d.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(rows.len(),1);let s=rows[0];assert!(s.transfer_verified);assert_eq!(s.protocol,Protocol::AnchorSwap as i32);assert_eq!(s.source_transfer_index,Some(source+1));
  assert_eq!((base58::encode(&s.amm_pool),base58::encode(&s.user),base58::encode(&s.input_mint),s.input_amount,base58::encode(&s.output_mint),s.output_amount),(f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
  for mode in 0..37 {let mut wrong=tx.clone();
   if mode<12 {let n=&mut wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0];let a=n.accounts.clone();match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>n.data[8..16].copy_from_slice(&(s.input_amount+1).to_le_bytes()),3=>n.data[16..24].copy_from_slice(&(s.output_amount+1).to_le_bytes()),4=>n.accounts[1]=a[10],5=>n.accounts[4]=a[5],6=>n.accounts[5]=a[4],7=>n.accounts[6]=a[7],8=>n.accounts[7]=a[6],9=>n.accounts[8]=a[10],10=>n.accounts[9]=a[7],_=>n.accounts[13]=a[1]}}
   else if mode<33 {let c=(mode-12)/7;let n=&mut wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[c];let a=n.accounts.clone();match(mode-12)%7{0=>{let amount=u64::from_le_bytes(n.data[1..9].try_into().unwrap())+1;n.data[1..9].copy_from_slice(&amount.to_le_bytes());},1=>n.data[1..9].fill(0),2=>n.accounts[0]=a[1],3=>n.accounts[1]=a[0],4=>n.accounts[2]=a[0],5=>n.stack_height=Some(99),_=>n.data.push(0)}}
   else if mode==33{wrong.meta.as_mut().unwrap().err=Some(Default::default());}
   else if mode==34{wrong.meta.as_mut().unwrap().log_messages.clear();}
   else if mode==35{wrong.meta.as_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}
   else{wrong.meta.as_mut().unwrap().post_token_balances.clear();}
   assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"AnchorSwap mode{mode}");
  }
 }
}
