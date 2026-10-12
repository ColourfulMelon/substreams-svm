//! Omnipair's program-owned SwapArgs, principal CPIs and SwapEvent.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{b58,base58,block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token,token_2022};
use std::collections::HashSet;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("omnixgS8fnqHfCcTGKWj6JtKjzpJZ1Y5y9pyFkQDkYE");
const EVENT_AUTHORITY:[u8;32]=b58!("FWdP9yTogKbuXvEqQNNHYw2TYm38MbinAZ2iTHeZWX8H");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{
 let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||a.len()!=14||d.len()!=24||d[..8]!=[248,198,158,145,225,117,135,200]
  ||a[10].0.as_slice()!=token::PROGRAM_ID||a[11].0.as_slice()!=token_2022::PROGRAM_ID
  ||a[12].0.as_slice()!=EVENT_AUTHORITY||a[13].0.as_slice()!=PROGRAM_ID
  ||a[..10].iter().enumerate().any(|(n,k)|a[..n].iter().any(|v|v.0==k.0)){return None;}
 let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
 Some(Layout{protocol:Protocol::Omnipair,pool:0,authority:9,users:[5,6],vaults:[3,4],amount,
  minimum_output:u64::from_le_bytes(d[16..24].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(tx:&ConfirmedTransaction,source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->bool{
 let Some(l)=layout(ix)else{return false;};let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0||calls.len()!=4||!successful.contains(&source)
  ||calls.iter().enumerate().any(|(n,c)|c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+1+n as u32))){return false;}
 for(n,c)in calls.iter().enumerate().filter(|(n,_)|*n==0||*n==3){let d=c.data();let ca=c.accounts();
  if c.program_id().0.as_slice()!=PROGRAM_ID||ca.len()!=1||ca[0].0.as_slice()!=EVENT_AUTHORITY
   ||d.len()!=if n==0{216}else{145}||d[..16]!=if n==0{[228,69,165,46,81,203,154,29,44,6,60,245,142,38,166,247]}else{[228,69,165,46,81,203,154,29,64,198,205,232,38,8,113,226]}{return false;}}
 let Some(meta)=tx.meta.as_ref()else{return false;};let keys=tx.resolved_accounts();let mut amounts=[0;2];
 for(n,c)in calls[1..3].iter().enumerate(){let ca=c.accounts();let d=c.data();let roles=if n==0{[5,7,3,9]}else{[4,8,6,0]};
  if ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
   ||ca.len()!=4||d.len()!=10||d[0]!=12||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
   ||mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[7+n].0.as_slice())||mints.mint_for(a[roles[2]].0).as_deref()!=Some(a[7+n].0.as_slice()){return false;}
  let Some(vault)=keys.iter().position(|k|*k==a[l.vaults[n]].0)else{return false;};
  let snapshots:Vec<_>=meta.pre_token_balances.iter().chain(&meta.post_token_balances).filter(|b|b.account_index as usize==vault).collect();
  if snapshots.is_empty()||snapshots.iter().any(|b|b.mint!=base58::encode(a[7+n].0)||b.owner!=base58::encode(a[0].0)
   ||b.program_id!=base58::encode(c.program_id().0)||b.ui_token_amount.as_ref().is_none_or(|v|v.decimals!=u32::from(d[9]))){return false;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().unwrap());
 }
 let e=calls[3].data();
 amounts[0]==l.amount&&amounts[1]>0&&amounts[1]>=l.minimum_output&&e[32]<=1
  &&u64::from_le_bytes(e[33..41].try_into().unwrap())==amounts[0]&&u64::from_le_bytes(e[41..49].try_into().unwrap())==amounts[1]
  &&e[73..105]==*a[9].0&&e[105..137]==*a[0].0
}
