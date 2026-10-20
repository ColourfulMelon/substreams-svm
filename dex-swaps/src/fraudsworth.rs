//! SwapSolPool from the program-owned Dr Fraudsworth AMM IDL. Remaining
//! accounts are forwarded only to the input Token-2022 transfer hook.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{base58,block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::{token,token_2022};
use std::collections::HashSet;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("5JsSAL3kJDUWD4ZveYXYZmgm1eVqueesTZVdAvtZg8cR");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{
 let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||!matches!(a.len(),11|15)||d.len()!=25||d[..8]!=[222,128,30,123,85,39,145,138]||d[16]>1
  ||[9,10].iter().any(|r|![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&a[*r].0.as_slice()))
  ||a[..9].iter().enumerate().any(|(n,k)|a[..n].iter().any(|v|v.0==k.0)){return None;}
 let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
 let b=usize::from(d[16]);
 Some(Layout{protocol:Protocol::DrFraudsworth,pool:1,authority:8,users:[6+b,7-b],vaults:[2+b,3-b],amount,
  minimum_output:u64::from_le_bytes(d[17..25].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(tx:&ConfirmedTransaction,source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->bool{
 let Some(l)=layout(ix)else{return false;};let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0||!matches!(calls.len(),2|3)||!successful.contains(&source)
  ||calls.iter().enumerate().any(|(n,c)|c.is_root()||!successful.contains(&(source+1+n as u32))){return false;}
 let hooked=calls.len()==3;let b=usize::from(ix.data()[16]);let mut amounts=[0u64;2];
 let positions:Vec<_>=calls.iter().enumerate().filter(|(_,c)|c.stack_height()==depth+1).map(|(n,_)|n).collect();
 if positions.len()!=2{return false;}
 for(n,position)in positions.iter().enumerate(){let c=&calls[*position];let ca=c.accounts();let d=c.data();let m=if n==0{4+b}else{5-b};let p=if n==0{9+b}else{10-b};
  let(from,to,auth)=if n==0{(l.users[0],l.vaults[0],8)}else{(l.vaults[1],l.users[1],1)};
  if c.stack_height()!=depth+1||c.program_id().0!=a[p].0||d.len()!=10||d[0]!=12||!matches!(ca.len(),4|8)
   ||ca[..4].iter().zip([from,m,to,auth]).any(|(k,r)|k.0!=a[r].0)
   ||mints.mint_for(a[from].0).as_deref()!=Some(a[m].0.as_slice())||mints.mint_for(a[to].0).as_deref()!=Some(a[m].0.as_slice()){return false;}
  if ca.len()==8 {
   if !hooked||a.len()!=15||a[p].0.as_slice()!=token_2022::PROGRAM_ID||ca[4..].iter().zip(&a[11..]).any(|(x,y)|x.0!=y.0){return false;}
   let Some(h)=calls.get(*position+1)else{return false;};let ha=h.accounts();let hd=h.data();
   if h.stack_height()!=depth+2||h.program_id().0!=a[13].0||ha.len()!=7||hd.len()!=16||hd[..8]!=[105,37,101,197,75,251,102,26]
    ||hd[8..16]!=d[1..9]||ha.iter().zip([from,m,to,auth,14,11,12]).any(|(k,r)|k.0!=a[r].0){return false;}
  }else if calls.get(*position+1).is_some_and(|c|c.stack_height()>depth+1){return false;}
  let Some(meta)=tx.meta.as_option()else{return false;};let keys=tx.resolved_accounts();let Some(vault)=keys.iter().position(|k|*k==a[l.vaults[n]].0)else{return false;};
  let snapshots:Vec<_>=meta.pre_token_balances.iter().chain(&meta.post_token_balances).filter(|v|v.account_index as usize==vault).collect();
  if snapshots.is_empty()||snapshots.iter().any(|v|v.mint!=base58::encode(a[m].0)||v.owner!=base58::encode(a[1].0)
   ||v.program_id!=base58::encode(a[p].0)||v.ui_token_amount.as_option().is_none_or(|t|t.decimals!=u32::from(d[9]))){return false;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().unwrap());
 }
 amounts[0]==l.amount&&amounts[1]>0&&amounts[1]>=l.minimum_output
}
