//! Sanctum Infinity V2 LST-to-LST spot swaps. INF liquidity issuance and
//! redemption use the same dispatch but are excluded by the u32::MAX index.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{b58,base58,block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::token;
use std::collections::HashSet;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{
 let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID || d.len()!=27 || d[0]!=23 || d[1]==0 || d[2]==0
  || a.len()<11+usize::from(d[1])+usize::from(d[2])+1
  || a[5].0.as_slice()!=token::PROGRAM_ID || a[6].0.as_slice()!=token::PROGRAM_ID
  || a[7].0.as_slice()!=b58!("AYhux5gJzCoeoc1PoJ1VxwPDe22RwcvpHviLDD1oCGvW")
  || a[8].0.as_slice()!=b58!("Gb7m4daakbVbrFLR33FKMDVMHAprRZ66CSYt4bpFwUgS")
  || d[3..7]==[255;4] || d[7..11]==[255;4] || d[3..7]==d[7..11] || a[1].0==a[2].0 {return None;}
 let amount=u64::from_le_bytes(d[19..27].try_into().ok()?);if amount==0{return None;}
 Some(Layout{protocol:Protocol::SanctumInfinity,pool:7,authority:0,users:[3,4],vaults:[9,10],amount,
  minimum_output:u64::from_le_bytes(d[11..19].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(tx:&ConfirmedTransaction,source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};let d=ix.data();
 if depth==0 || calls.len()!=9 || !successful.contains(&source)
  || calls.iter().enumerate().any(|(n,c)|c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+1+n as u32))){return None;}
 let inp=11;let out=inp+usize::from(d[1]);let price=out+usize::from(d[2]);
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
 let mut amounts=[0;2];let meta=tx.meta.as_ref()?;let keys=tx.resolved_accounts();
 for (n,c) in calls[5..7].iter().enumerate(){let ca=c.accounts();let cd=c.data();let mint=1+n;
  let roles=if n==0{[3,1,9,0]}else{[10,2,4,7]};
  if c.program_id().0!=a[5+n].0 || cd.len()!=10 || cd[0]!=12 || ca.len()!=4
   || ca.iter().zip(roles).any(|(x,r)|x.0!=a[r].0)
   || [l.users[n],l.vaults[n]].iter().any(|r|mints.mint_for(a[*r].0).as_deref()!=Some(a[mint].0.as_slice())){return None;}
  amounts[n]=u64::from_le_bytes(cd[1..9].try_into().ok()?);
  let vault=a[l.vaults[n]].0;let pos=keys.iter().position(|key|*key==vault)?;
  let pre=meta.pre_token_balances.iter().find(|b|b.account_index as usize==pos)?;
  let post=meta.post_token_balances.iter().find(|b|b.account_index as usize==pos)?;
  for b in [pre,post]{if b.mint!=base58::encode(a[mint].0)||b.owner!=base58::encode(a[7].0)
   || b.program_id!=base58::encode(token::PROGRAM_ID)||b.ui_token_amount.as_ref()?.decimals!=u32::from(cd[9]){return None;}}
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
  if post.ui_token_amount.as_ref()?.amount.parse::<i128>().ok()?-pre.ui_token_amount.as_ref()?.amount.parse::<i128>().ok()?!=change{return None;}
 }
 (amounts[0]==l.amount&&amounts[1]>0&&amounts[1]>=l.minimum_output).then_some((amounts[0],amounts[1]))
}
