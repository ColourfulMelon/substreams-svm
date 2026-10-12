//! Raydium-compatible CPMM fork. Canonical native request and scoped SwapEvent
//! must agree with both principal CPIs and both complete persistent vault ledgers.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58,block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use base64::Engine;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("HCrCy6bzHhZ1b6bXwQAucEFkKXyzYMh3hgAR8UPrYSEP");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||a.len()!=13||d.len()!=24||d[..8]!=[143,190,90,218,196,30,51,222]
  ||a[8].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||a[9].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID
  ||a[10].0==a[11].0||a[4].0==a[5].0||a[6].0==a[7].0||a[4].0==a[6].0||a[5].0==a[7].0{return None;}
 let authority=solana_pubkey::Pubkey::find_program_address(&[b"vault_and_lp_mint_auth_seed"],&solana_pubkey::Pubkey::new_from_array(PROGRAM_ID)).0;
 if a[1].0.as_slice()!=authority.as_ref(){return None;}
 let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
 Some(Layout{protocol:Protocol::RaydiumCpmmFork,pool:3,authority:0,users:[4,5],vaults:[6,7],amount,minimum_output:u64::from_le_bytes(d[16..24].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};if depth==0||calls.len()!=2||!successful.contains(&source){return None;}
 let mut amounts=[0u64;2];for(n,c)in calls.iter().enumerate(){let d=c.data();let ca=c.accounts();let roles=if n==0{[4,10,6,0]}else{[7,11,5,1]};
  if c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+n as u32+1))||c.program_id().0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID
   ||d.len()!=10||d[0]!=12||ca.len()!=4||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
   ||mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[10+n].0.as_slice())||mints.mint_for(a[roles[2]].0).as_deref()!=Some(a[10+n].0.as_slice()){return None;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
 }
    let meta=tx.meta.as_ref()?;let resolved=tx.resolved_accounts();
    for role in [6,7] {
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
 let walk:Vec<_>=tx.walk_instructions().collect();let mut cursor=0;let mut stack=Vec::new();let mut event=None;
 for log in &tx.meta.as_ref().unwrap().log_messages{
  let parts:Vec<_>=log.split_whitespace().collect();
  if parts.len()==4&&parts[0]=="Program"&&parts[2]=="invoke"{
   let Ok(program)=substreams_solana::base58::decode(parts[1])else{return None;};
   let Some(height)=parts[3].strip_prefix('[').and_then(|x|x.strip_suffix(']')).and_then(|x|x.parse::<u32>().ok())else{return None;};
   let Some(index)=walk.iter().enumerate().skip(cursor).find(|(_,x)|x.program_id().0==&program&&(if x.is_root(){1}else{x.stack_height()})==height).map(|(i,_)|i)else{return None;};
   if height as usize!=stack.len()+1{return None;}cursor=index+1;stack.push(index as u32);
  }else if parts.len()>=3&&parts[0]=="Program"&&matches!(parts[2],"success"|"failed:"){stack.pop();}
  else if stack.last()==Some(&source)&&log.starts_with("Program data: "){
   let Ok(e)=base64::engine::general_purpose::STANDARD.decode(&log[14..])else{return None;};
   if e.len()!=170||e[..8]!=[64,198,205,232,38,8,113,226]{continue;}
   if event.is_some(){return None;}event=Some(e);
  }
 }
 let e=event?;
 if e[8..40]!=*a[3].0||e[89..121]!=*a[10].0||e[121..153]!=*a[11].0||e[88]!=1
  ||u64::from_le_bytes(e[56..64].try_into().ok()?)!=amounts[0]||u64::from_le_bytes(e[64..72].try_into().ok()?)!=amounts[1]
  ||u64::from_le_bytes(e[72..80].try_into().ok()?)!=0||u64::from_le_bytes(e[80..88].try_into().ok()?)!=0
  ||amounts[0]!=l.amount||amounts[1]==0||amounts[1]<l.minimum_output{return None;}
 Some((amounts[0],amounts[1]))
}
#[cfg(test)]
mod tests {
 use super::*; use substreams_solana::base58;
 #[test]
 fn finalized_cpmm_fork_binds_request_scoped_event_and_all_vault_receipts() {
  let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/raydium-cpmm-fork-native26/cases.tsv").lines() {
   count+=1;let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
   let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/raydium-cpmm-fork-native26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();
   let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let walk:Vec<_>=tx.walk_instructions().collect();
   let native=walk[source as usize].data().clone();let input=walk[source as usize+1].data().clone();let output=walk[source as usize+2].data().clone();
   let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();
   assert_eq!(rows.len(),1,"{}",f[0]);let s=rows[0];assert!(s.transfer_verified);assert_eq!(s.protocol,Protocol::RaydiumCpmmFork as i32);assert_eq!(s.source_transfer_index,Some(source+1));
   assert_eq!((base58::encode(&s.amm_pool),base58::encode(&s.user),base58::encode(&s.input_mint),s.input_amount,base58::encode(&s.output_mint),s.output_amount),(f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
   for mode in 0..38 {let mut wrong=tx.clone();
    if mode<12 {let n=wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions.iter_mut().find(|x|x.data==native).unwrap();let a=n.accounts.clone();match mode {0=>n.data[0]^=1,1=>n.data.push(0),2=>n.data[8..16].copy_from_slice(&(s.input_amount+1).to_le_bytes()),3=>n.data[16..24].copy_from_slice(&(s.output_amount+1).to_le_bytes()),4=>n.accounts[3]=a[0],5=>n.accounts[6]=a[7],6=>n.accounts[7]=a[6],7=>n.accounts[4]=a[5],8=>n.accounts[5]=a[4],9=>n.accounts[1]=a[0],10=>n.accounts[8]=a[3],_=>{n.accounts.pop();}}}
    else if mode<26 {let g=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g|g.instructions.windows(2).any(|w|w[0].data==input&&w[1].data==output)).unwrap();let pos=g.instructions.windows(2).position(|w|w[0].data==input&&w[1].data==output).unwrap();let n=&mut g.instructions[pos+usize::from(mode>=19)];let a=n.accounts.clone();match mode {12|19=>{let amount=u64::from_le_bytes(n.data[1..9].try_into().unwrap())+1;n.data[1..9].copy_from_slice(&amount.to_le_bytes());},13|20=>n.data[1..9].fill(0),14|21=>n.accounts[0]=a[1],15|22=>n.accounts[1]=a[0],16|23=>n.accounts[3]=a[0],17|24=>n.stack_height=Some(99),_=>n.data.push(0)}}
    else if mode==26 {wrong.meta.as_mut().unwrap().err=Some(Default::default());}
    else if mode==27 {wrong.meta.as_mut().unwrap().log_messages.clear();}
    else if mode==28 {wrong.meta.as_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}
    else if mode==29 {wrong.meta.as_mut().unwrap().post_token_balances.clear();}
    else {let log=wrong.meta.as_mut().unwrap().log_messages.iter_mut().find(|l|l.starts_with("Program data: ")&&base64::engine::general_purpose::STANDARD.decode(&l[14..]).is_ok_and(|e|e.len()==170&&e[..8]==[64,198,205,232,38,8,113,226])).unwrap();let mut e=base64::engine::general_purpose::STANDARD.decode(&log[14..]).unwrap();let offset=[8,56,64,72,80,88,89,121][mode-30];e[offset]^=1;*log=format!("Program data: {}",base64::engine::general_purpose::STANDARD.encode(e));}
    assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"CPMM fork mode{mode}");
   }
  }assert_eq!(count,1);
 }
}
