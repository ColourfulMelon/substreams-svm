//! Sanctum Infinity V1/V2 LST-to-LST spot swaps. INF liquidity issuance and
//! redemption use the same dispatch but are excluded by the u32::MAX index.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{base58,block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::token;
use std::collections::HashSet;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{
 let a=ix.accounts();let d=ix.data();let shift=usize::from(d.first()==Some(&1));
 if ix.program_id().0.as_slice()!=PROGRAM_ID || d.len()!=27 || !matches!(d[0],1|23) || d[1]==0 || d[2]==0
  || a.len()<11+shift+usize::from(d[1])+usize::from(d[2])+1
  || a[5+shift].0.as_slice()!=token::PROGRAM_ID || a[6+shift].0.as_slice()!=token::PROGRAM_ID
  || a[7+shift].0.as_slice()!=b58!("AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW")
  || a[8+shift].0.as_slice()!=b58!("Gb7m4daakbVbrFLR33FKMDVMHAprRZ66CSYt4bpFwUgS")
  || d[3..7]==[255;4] || d[7..11]==[255;4] || d[3..7]==d[7..11] || a[1].0==a[2].0 {return None;}
 if shift==1 {
  let fee=b58!("6U8Ve7NuTVq9pb3xEC2ZwxBhceWULUuJn1nSKCTraq5r");
  let ata=solana_pubkey::Pubkey::find_program_address(&[fee.as_slice(),token::PROGRAM_ID.as_slice(),a[2].0],
   &solana_pubkey::Pubkey::new_from_array(b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"))).0;
  if a[5].0.as_slice()!=ata.as_ref(){return None;}
 }
 let amount=u64::from_le_bytes(d[19..27].try_into().ok()?);if amount==0{return None;}
 Some(Layout{protocol:Protocol::SanctumInfinity,pool:7+shift,authority:0,users:[3,4],vaults:[9+shift,10+shift],amount,
  minimum_output:u64::from_le_bytes(d[11..19].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(tx:&ConfirmedTransaction,source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};let d=ix.data();let shift=usize::from(d[0]==1);
 if depth==0 || calls.len()!=9 || !successful.contains(&source)
  || calls.iter().enumerate().any(|(n,c)|c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+1+n as u32))){return None;}
 let inp=11+shift;let out=inp+usize::from(d[1]);let price=out+usize::from(d[2]);
 for (n,program,mint,tail,opcode) in [(0,inp,1,inp+1..out,0),(1,out,2,out+1..price,0),
  (2,inp,1,inp+1..out,0),(4,out,2,out+1..price,1),(7,inp,1,inp+1..out,0),(8,out,2,out+1..price,0)] {
  let c=&calls[n];let ca=c.accounts();let cd=c.data();
  if c.program_id().0!=a[program].0 || cd.len()!=9 || cd[0]!=opcode || ca.len()!=1+tail.len()
   || ca[0].0!=a[mint].0 || ca[1..].iter().zip(&a[tail]).any(|(x,y)|x.0!=y.0){return None;}
 }
 let p=&calls[3];let pa=p.accounts();let pd=p.data();
 if p.program_id().0!=a[price].0 || pd.len()!=17 || pd[0]!=0 || pd[1..9]!=d[19..27]
  || pa.len()!=2+a.len()-price-1 || pa[0].0!=a[1].0 || pa[1].0!=a[2].0
  || pa[2..].iter().zip(&a[price+1..]).any(|(x,y)|x.0!=y.0) || calls[2].data()[1..9]!=d[19..27]{return None;}
 let mut amounts=[0;2];let meta=tx.meta.as_option()?;let keys=tx.resolved_accounts();
 for (n,c) in calls[5..7].iter().enumerate(){let ca=c.accounts();let cd=c.data();let mint=1+n;
  let roles=if n==0{[3,1,l.vaults[0],0]}else{[l.vaults[1],2,4,l.pool]};
  if c.program_id().0!=a[5+shift+n].0 || cd.len()!=10 || cd[0]!=12 || ca.len()!=4
   || ca.iter().zip(roles).any(|(x,r)|x.0!=a[r].0)
   || [l.users[n],l.vaults[n]].iter().any(|r|mints.mint_for(a[*r].0).as_deref()!=Some(a[mint].0.as_slice())){return None;}
  amounts[n]=u64::from_le_bytes(cd[1..9].try_into().ok()?);
  let vault=a[l.vaults[n]].0;let pos=keys.iter().position(|key|*key==vault)?;
  let pre=meta.pre_token_balances.iter().find(|b|b.account_index as usize==pos)?;
  let post=meta.post_token_balances.iter().find(|b|b.account_index as usize==pos)?;
  for b in [pre,post]{if b.mint!=base58::encode(a[mint].0)||b.owner!=base58::encode(a[l.pool].0)
   || b.program_id!=base58::encode(token::PROGRAM_ID)||b.ui_token_amount.as_option()?.decimals!=u32::from(cd[9]){return None;}}
  let mut change=0i128;
  for (index,t) in tx.walk_instructions().enumerate(){let ta=t.accounts();let td=t.data();
   if t.program_id().0.as_slice()!=token::PROGRAM_ID || !ta.iter().any(|k|k.0==vault){continue;}
   if !successful.contains(&(index as u32)){return None;}
   let (to,from)=match (td.first(),td.len(),ta.len()) {
    (Some(12),10,4) if ta[1].0==a[mint].0 =>(ta[2].0,ta[0].0),
    (Some(3),9,3) if mints.mint_for(ta[0].0).as_deref()==Some(a[mint].0.as_slice())
      && mints.mint_for(ta[1].0).as_deref()==Some(a[mint].0.as_slice()) =>(ta[1].0,ta[0].0),
    _=>return None,
   };let atoms=i128::from(u64::from_le_bytes(td[1..9].try_into().ok()?));
   if from==vault{change-=atoms;}if to==vault{change+=atoms;}
  }
  if post.ui_token_amount.as_option()?.amount.parse::<i128>().ok()?-pre.ui_token_amount.as_option()?.amount.parse::<i128>().ok()?!=change{return None;}
 }
 (amounts[0]==l.amount&&amounts[1]>0&&amounts[1]>=l.minimum_output).then_some((amounts[0],amounts[1]))
}


#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn legacy_exact_input_swaps_bind_fee_account_calculators_and_receipts() {
  let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/sanctum-legacy27/cases.tsv").lines() {
   let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
   let tx=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(format!("{}/fixtures/swap-coverage/sanctum-legacy27/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap()).unwrap();
   let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let first=walk[source as usize+6].data().clone();let keys=tx.resolved_accounts();let first_accounts:Vec<u8>=walk[source as usize+6].accounts().iter().map(|a|keys.iter().position(|k|*k==a.0).unwrap() as u8).collect();
   let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)&&s.amm==PROGRAM_ID).collect();assert_eq!(rows.len(),1);let row=rows[0];
   assert!(row.transfer_verified);assert_eq!(row.protocol,Protocol::SanctumInfinity);assert_eq!(row.source_transfer_index,Some(source+6));assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),(f[3].into(),f[4].into(),f[5].into(),f[6].parse().unwrap(),f[7].into(),f[8].parse().unwrap()));
   let routed=|t:&ConfirmedTransaction|{let mut r=row.clone();r.protocol=Protocol::JupiterV4.into();let mut rows=vec![r];let mints=TokenMintLookup::new(t,t.meta.as_option().unwrap());crate::quality::verify_with_fees(t,&mints,&mut rows,false,|_,_|None);rows};assert_eq!(routed(&tx).len(),1);
   for mode in 0..30 {
    let mut wrong=tx.clone();
    if mode<20 {
     let (data,accounts)=wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().map(|x|(&mut x.data,&mut x.accounts)).chain(wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).map(|x|(&mut x.data,&mut x.accounts))).find(|(data,_)|**data==native).unwrap();
     let roles=accounts.clone();match mode {0=>data[0]=2,1=>data.push(0),2=>data[1]=0,3=>data[2]=0,4=>data[3..7].fill(255),5=>data[7..11].copy_from_slice(&native[3..7]),6=>data[19..27].fill(0),7=>data[19]^=1,8=>data[11..19].copy_from_slice(&(row.output_amount+1).to_le_bytes()),9=>{accounts.pop();},_=>{let role=[1,2,3,4,5,6,7,8,9,10][mode-10];accounts[role]=roles[0];}}
    } else if mode<27 {
     let n=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==first&&x.accounts==first_accounts).unwrap();let a=n.accounts.clone();match mode {20=>n.data[1]^=1,21=>n.data[1..9].fill(0),22=>n.accounts[0]=a[1],23=>n.accounts[2]=a[0],24=>n.accounts[3]=a[0],25=>n.data[9]^=1,_=>n.stack_height=Some(99)}
    } else if mode==27 {
     let b=wrong.meta.as_option_mut().unwrap().post_token_balances.iter_mut().find(|b|b.owner==f[3]&&b.mint==f[5]).unwrap();let a=b.ui_token_amount.as_option_mut().unwrap();a.amount=(a.amount.parse::<u64>().unwrap()+1).to_string();
    } else if mode==28 {wrong.meta.as_option_mut().unwrap().log_messages.truncate(4);}
    else {for l in &mut wrong.meta.as_option_mut().unwrap().log_messages {if *l==format!("Program {} success",base58::encode(PROGRAM_ID)){*l=l.replace("success","failed: mutation");}}}
    assert!(crate::process_transaction(wrong.clone()).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source)||s.amm!=PROGRAM_ID)),"native {mode}");assert!(routed(&wrong).is_empty(),"routed {mode}");
   }count+=1;
  }assert_eq!(count,3);
 }
}
