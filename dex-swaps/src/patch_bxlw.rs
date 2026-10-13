//! Bounded deployed Patch BuybackSwap and Bxlw SellTokensV2 profiles.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{b58,block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token,token_2022};
use std::collections::HashSet;
use base64::Engine;
const SOL:[u8;32]=b58!("So11111111111111111111111111111111111111112");
pub(crate) const PATCH:[u8;32]=b58!("HVfvL7oT9k69GeZu5UwXJy7QjarqmFaVfw2H1oLyHfnF");
pub(crate) const BXLW:[u8;32]=b58!("BxLw4YQ65QdmeZ5znDZ75c22nAZw3uijrJbPmG8n1DST");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{
 let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()==PATCH {
  if d.len()!=9||d[..8]!=[190,159,2,23,102,251,151,159]||d[8]!=1||a.len()!=18||a[4].0.as_slice()!=SOL||a[10].0.as_slice()!=token::PROGRAM_ID||a[11].0.as_slice()!=token_2022::PROGRAM_ID||a[12].0.as_slice()!=b58!("AdKZDS3cpmSjys2GQNakgw8GmoYG9qPd3iC4hvbqoA4i")||a[14].0.as_slice()!=b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")||a[15].0.as_slice()!=b58!("11111111111111111111111111111111"){return None;}
  Some(Layout{protocol:Protocol::PatchBuyback,pool:2,authority:3,users:[8,9],vaults:[6,7],amount:0,minimum_output:0,exact_input:false,input_limit:false,input_first:true,fixed_users:true})
 }else if ix.program_id().0.as_slice()==BXLW {
  if d.len()!=48||d[..8]!=[34,15,247,72,138,167,9,155]||a.len()!=17||a[13].0.as_slice()!=b58!("11111111111111111111111111111111")||a[14].0.as_slice()!=token::PROGRAM_ID||a[15].0.as_slice()!=token_2022::PROGRAM_ID||a[16].0.as_slice()!=BXLW||d[32..40]!=3u64.to_le_bytes()||d[40..48]!=[0;8]{return None;}
  let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
  Some(Layout{protocol:Protocol::Bxlw,pool:7,authority:0,users:[5,6],vaults:[8,9],amount,minimum_output:u64::from_le_bytes(d[16..24].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
 }else{None}
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();
 if a[..if ix.program_id().0.as_slice()==PATCH{14}else{13}].iter().enumerate().any(|(i,k)|a[..i].iter().any(|old|old.0==k.0)){return None;}let patch=ix.program_id().0.as_slice()==PATCH;let height=if ix.is_root(){1}else{ix.stack_height()};
 if height==0||!successful.contains(&source)||calls.len()!=if patch{4}else{2}{return None;}
 let start=if patch{2}else{0};
 if patch {for(n,c)in calls[..2].iter().enumerate(){let d=c.data();let ca=c.accounts();if c.is_root()||c.stack_height()!=height+1||!successful.contains(&(source+1+n as u32))||c.program_id().0!=a[12].0||ca.len()!=3||ca.iter().zip([2,13,17]).any(|(k,r)|k.0!=a[r].0)||d.len()!=216||d[..8]!=[153,3,180,15,73,52,89,108]||d[8]!=1||d[9]!=(n+1)as u8{return None;}}}
 let mut amounts=[0u64;2];
 for(n,c)in calls[start..].iter().enumerate(){let ca=c.accounts();let d=c.data();let roles=if patch{if n==0{vec![8,4,6,3]}else{vec![7,5,9,2]}}else if n==0{vec![5,4,8,0]}else{vec![9,6,7]};let program=if patch{if n==0{token::PROGRAM_ID}else{token_2022::PROGRAM_ID}}else if n==0{token_2022::PROGRAM_ID}else{token::PROGRAM_ID};let checked=patch||n==0;
  if c.is_root()||c.stack_height()!=height+1||!successful.contains(&(source+1+start as u32+n as u32))||c.program_id().0.as_slice()!=program||ca.len()!=roles.len()||ca.iter().zip(&roles).any(|(k,r)|k.0!=a[*r].0)||d.len()!=if checked{10}else{9}||d[0]!=if checked{12}else{3}{return None;}
  let mint=if patch{a[if n==0{4}else{5}].0.as_slice()}else if n==0{a[4].0.as_slice()}else{SOL.as_slice()};
  if mints.mint_for(ca[0].0).as_deref()!=Some(mint)||mints.mint_for(ca[if checked{2}else{1}].0).as_deref()!=Some(mint){return None;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
 }
 if amounts.iter().any(|n|*n==0)||!patch&&(amounts[0]!=l.amount||amounts[1]<l.minimum_output){return None;}
 // All persistent user/vault receipts must conserve exactly; unknown token
 // operations or transfer-fee deductions cannot inherit the gross amount.
 let meta=tx.meta.as_ref()?;let keys=tx.resolved_accounts();
 for(role,mint,owner)in if patch{vec![(8,4,3),(9,5,3),(6,4,2),(7,5,2)]}else{vec![(5,4,0),(8,4,7),(9,usize::MAX,7)]}{
  let address=a[role].0;let mint=if mint==usize::MAX{SOL.as_slice()}else{a[mint].0.as_slice()};let position=keys.iter().position(|key|*key==address)?;
  let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
  if before.mint!=substreams_solana::base58::encode(mint)||after.mint!=before.mint||before.owner!=substreams_solana::base58::encode(a[owner].0)||after.owner!=before.owner||before.program_id!=substreams_solana::base58::encode(if mint==SOL.as_slice(){token::PROGRAM_ID}else{token_2022::PROGRAM_ID})||after.program_id!=before.program_id{return None;}
  let mut change=0i128;
  for(index,c)in tx.walk_instructions().enumerate(){if ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())||!c.accounts().iter().any(|k|k.0==address){continue;}let ca=c.accounts();let d=c.data();let checked=match(d.first(),d.len()){(Some(3),9)=>false,(Some(12),10)=>true,_=>return None};if ca.len()!=if checked{4}else{3}||!successful.contains(&(index as u32))||mints.mint_for(ca[0].0).as_deref()!=Some(mint)||mints.mint_for(ca[if checked{2}else{1}].0).as_deref()!=Some(mint){return None;}let n=i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?));if ca[0].0==address{change=change.checked_sub(n)?;}if ca[if checked{2}else{1}].0==address{change=change.checked_add(n)?;}}
  if i128::from(after.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?)-i128::from(before.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?)!=change{return None;}
 }
 let walk:Vec<_>=tx.walk_instructions().collect();let mut cursor=0;let mut stack=Vec::new();let mut event=None;
 for log in &meta.log_messages{let p:Vec<_>=log.split_whitespace().collect();if p.len()==4&&p[0]=="Program"&&p[2]=="invoke"{let key=substreams_solana::base58::decode(p[1]).ok()?;let depth=p[3].strip_prefix('[')?.strip_suffix(']')?.parse::<u32>().ok()?;let index=walk.iter().enumerate().skip(cursor).find(|(_,x)|x.program_id().0==&key&&(if x.is_root(){1}else{x.stack_height()})==depth)?.0;if depth as usize!=stack.len()+1{return None;}cursor=index+1;stack.push(index as u32);}else if p.len()>=3&&p[0]=="Program"&&matches!(p[2],"success"|"failed:"){let index=stack.pop()?;if p[2]!="success"||substreams_solana::base58::encode(walk[index as usize].program_id().0)!=p[1]{return None;}}else if stack.last()==Some(&source)&&log.starts_with("Program data: "){let e=base64::engine::general_purpose::STANDARD.decode(&log[14..]).ok()?;if e.len()==if patch{120}else{329}&&e[..8]==if patch{[61,57,48,132,64,49,186,125]}else{[69,47,244,206,115,69,174,246]}{if event.is_some(){return None;}event=Some(e);}}}
 if !stack.is_empty(){return None;}let e=event?;let read=|at:usize|u64::from_le_bytes(e[at..at+8].try_into().unwrap());
 if patch{if e[8..40]!=*a[2].0||e[40..72]!=*a[4].0||e[72..104]!=*a[5].0||read(104)!=amounts[0]||read(112)!=amounts[1]{return None;}}
 else if e[8..40]!=BXLW||e[40..72]!=*a[4].0||e[72..104]!=*a[0].0||e[104..136]!=*a[1].0||e[136]!=2||read(153)!=amounts[0]||read(137).checked_sub(read(161))?!=amounts[1]||e[169..201]!=*a[11].0||e[249..281]!=*a[12].0{return None;}
 Some((amounts[0],amounts[1]))
}

#[cfg(test)]
mod tests {
 use super::*;use substreams_solana::base58;
 #[test]
 fn finalized_patch_buyback_and_bxlw_sell_bind_request_and_net_receipts(){
  let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/patch-bxlw27/cases.tsv").lines(){
   let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();let patch=f[8]=="patch_buyback";
   let tx=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(format!("{}/fixtures/swap-coverage/patch-bxlw27/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap()).unwrap();count+=1;
   let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let input=walk[source as usize+if patch{3}else{1}].data().clone();let output=walk[source as usize+if patch{4}else{2}].data().clone();
   let result=crate::process_transaction(tx.clone()).expect("proven native exchange");let rows:Vec<_>=result.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(rows.len(),1,"{}",f[0]);let row=rows[0];assert!(row.transfer_verified);assert_eq!(row.protocol,if patch{Protocol::PatchBuyback}else{Protocol::Bxlw}as i32);assert_eq!(row.source_transfer_index,Some(source+if patch{3}else{1}));assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),(f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
   let verify_route=|transaction:&ConfirmedTransaction|{let mut candidate=row.clone();candidate.protocol=Protocol::JupiterV4 as i32;let mut candidates=vec![candidate];let mints=TokenMintLookup::new(transaction,transaction.meta.as_ref().unwrap());crate::quality::verify_with_fees(transaction,&mints,&mut candidates,false,|_,_|None);candidates};assert_eq!(verify_route(&tx).len(),1);
   for mode in 0..38 {
    let mut wrong=tx.clone();
    if mode<16 {let n=wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions.iter_mut().find(|x|x.data==native).unwrap();let a=n.accounts.clone();match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>{if patch{n.data[8]=2}else{n.data[8]^=1}},3=>{if patch{n.data[8]=0}else{n.data[16..24].copy_from_slice(&(row.output_amount+1).to_le_bytes())}},4=>{if patch{n.accounts.pop();}else{n.data[32]=4;}},5=>{if patch{n.accounts[12]=a[0]}else{n.data[40]=1}},_=>{let role=if patch{[2,3,4,5,6,7,8,9,10,11][mode-6]}else{[0,4,5,6,7,8,9,13,14,15][mode-6]};n.accounts[role]=a[if role==0{1}else{0}];}}}
    else if mode<28 {let checked=patch||mode<22;let c=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|c|c.data==if mode<22{input.clone()}else{output.clone()}).unwrap();let a=c.accounts.clone();match (mode-16)%6{0=>c.data[1]^=1,1=>c.data[1..9].fill(0),2=>c.accounts[0]=a[1],3=>c.accounts[if checked{2}else{1}]=a[0],4=>c.accounts[if checked{3}else{2}]=a[0],_=>c.stack_height=Some(99)}}
    else if mode<35 {let meta=wrong.meta.as_mut().unwrap();let disc=if patch{[61,57,48,132,64,49,186,125]}else{[69,47,244,206,115,69,174,246]};let l=meta.log_messages.iter_mut().find(|l|l.starts_with("Program data: ")&&base64::engine::general_purpose::STANDARD.decode(&l[14..]).is_ok_and(|e|e.starts_with(&disc))).unwrap();let mut e=base64::engine::general_purpose::STANDARD.decode(&l[14..]).unwrap();let at=if patch{[8,40,72,104,112,104,112][mode-28]}else{[8,40,72,104,137,153,161][mode-28]};e[at]^=1;*l=format!("Program data: {}",base64::engine::general_purpose::STANDARD.encode(e));}
    else if mode==35 {let b=wrong.meta.as_mut().unwrap().post_token_balances.iter_mut().find(|b|b.owner==f[2]&&b.mint==f[6]).unwrap();let a=b.ui_token_amount.as_mut().unwrap();a.amount=(a.amount.parse::<u64>().unwrap()+1).to_string();}
    else if mode==36{for l in &mut wrong.meta.as_mut().unwrap().log_messages{if *l==format!("Program {} success",if patch{base58::encode(PATCH)}else{base58::encode(BXLW)}){*l=l.replace("success","failed: mutation");}}}
    else if patch{let c=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|c|c.data.len()==216).unwrap();c.data[9]=9;}
    else{wrong.meta.as_mut().unwrap().log_messages.truncate(5);}
    assert!(crate::process_transaction(wrong.clone()).map_or(true,|p|p.swaps.iter().all(|s|s.source_index!=Some(source))),"native{} mode{mode}",f[8]);assert!(verify_route(&wrong).is_empty(),"routed{} mode{mode}",f[8]);
   }
  }assert_eq!(count,2);
 }
}
