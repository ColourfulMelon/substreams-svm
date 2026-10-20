//! Numeraire spot swaps from its program-owned Anchor IDL. Quote-only handlers
//! are excluded; committed checked-token receipts and native event must agree.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::{token,token_2022};
use std::collections::HashSet;
use base64::Engine;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("NUMERUNsFCP3kuNmWZuXtm1AaQCPj9uw6Guv2Ekoi5P");
const CONFIG:[u8;32]=b58!("FS159v4b2jo3fjGBaUFmDzgx7k616XhpKhMwX2Q3HeeD");
const IN:[u8;8]=[104,104,131,86,161,189,180,216];
const IN_HINT:[u8;8]=[98,239,244,233,16,236,40,49];
const OUT:[u8;8]=[250,73,101,33,38,207,75,184];
const OUT_HINT:[u8;8]=[153,208,206,70,62,234,98,182];
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{
 let d=ix.data();let a=ix.accounts();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||d.len()<26||a.len()!=11{return None;}
 let exact=d[..8]==IN||d[..8]==IN_HINT;let hinted=d[..8]==IN_HINT||d[..8]==OUT_HINT;
 if (!exact&&d[..8]!=OUT&&d[..8]!=OUT_HINT)||d.len()!=if hinted{116}else{26}
  ||d[8]>=10||d[9]>=10||d[8]==d[9]||a[7].0.as_slice()!=CONFIG
  ||a[9].0.as_slice()!=token::PROGRAM_ID||a[10].0.as_slice()!=token_2022::PROGRAM_ID
  ||a[..9].iter().enumerate().any(|(i,k)|a[..i].iter().any(|other|k.0==other.0)){return None;}
 let amount=u64::from_le_bytes(d[10..18].try_into().ok()?);let limit=u64::from_le_bytes(d[18..26].try_into().ok()?);
 if amount==0||!exact&&limit==0{return None;}
 Some(Layout{protocol:Protocol::Numeraire,pool:0,authority:8,users:[3,4],vaults:[5,6],amount:if exact{amount}else{limit},minimum_output:if exact{limit}else{amount},exact_input:exact,input_limit:!exact,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>,net_output:u64)->bool{
 let Some(l)=layout(ix)else{return false;};let a=ix.accounts();let d=ix.data();let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0||calls.len()!=2||!successful.contains(&source)||net_output==0{return false;}
 let mut amounts=[0u64;2];
 let output_first=calls[0].accounts().first().is_some_and(|role|role.0==a[6].0);
 for (n,c)in calls.iter().enumerate(){let ca=c.accounts();let cd=c.data();let side=if output_first{1-n}else{n};let roles=if side==0{[3,1,5,8]}else{[6,2,4,0]};
  if c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+1+n as u32))
   ||![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
   ||ca.len()!=4||cd.len()!=10||cd[0]!=12||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
   ||mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[roles[1]].0.as_slice())
   ||mints.mint_for(a[roles[2]].0).as_deref()!=Some(a[roles[1]].0.as_slice()){return false;}
  amounts[side]=u64::from_le_bytes(cd[1..9].try_into().unwrap());
 }
 let requested=u64::from_le_bytes(d[10..18].try_into().unwrap());let limit=u64::from_le_bytes(d[18..26].try_into().unwrap());
 if amounts[0]==0||amounts[1]!=net_output||l.exact_input&&(amounts[0]!=requested||net_output<limit)
  ||!l.exact_input&&(amounts[0]>limit||net_output!=requested){return false;}
 // Map the complete runtime frames to instruction sources, so a sibling event
 // cannot validate this swap. Invocation commitment is checked above.
 let walk:Vec<_>=tx.walk_instructions().collect();let mut cursor=0;let mut stack=Vec::new();let mut event=None;
 for log in &tx.meta.as_option().unwrap().log_messages{
  let parts:Vec<_>=log.split_whitespace().collect();
  if parts.len()==4&&parts[0]=="Program"&&parts[2]=="invoke"{
   let Ok(program)=substreams_solana::base58::decode(parts[1])else{return false;};
   let Some(height)=parts[3].strip_prefix('[').and_then(|x|x.strip_suffix(']')).and_then(|x|x.parse::<u32>().ok())else{return false;};
   let Some(index)=walk.iter().enumerate().skip(cursor).find(|(_,x)|x.program_id().0==&program&&(if x.is_root(){1}else{x.stack_height()})==height).map(|(i,_)|i)else{return false;};
   if height as usize!=stack.len()+1{return false;}cursor=index+1;stack.push(index as u32);
  }else if parts.len()>=3&&parts[0]=="Program"&&matches!(parts[2],"success"|"failed:"){stack.pop();}
  else if stack.last()==Some(&source)&&log.starts_with("Program data: "){
   let Ok(e)=base64::engine::general_purpose::STANDARD.decode(&log[14..])else{return false;};
   if e.len()!=98||e[..8]!=if l.exact_input{[147,136,213,11,150,23,141,152]}else{[71,66,127,123,231,29,227,92]}{continue;}
   if event.is_some(){return false;}event=Some(e);
  }
 }
 let Some(e)=event else{return false;};
 u64::from_le_bytes(e[8..16].try_into().unwrap())==amounts[0]
  &&u64::from_le_bytes(e[16..24].try_into().unwrap())==net_output
  &&u64::from_le_bytes(e[24..32].try_into().unwrap())==limit
  &&e[32..64]==*a[8].0&&e[64..66]==d[8..10]&&e[66..98]==*a[0].0
}
#[cfg(test)]
mod tests{
 use super::*;use substreams_solana::base58;
 #[test]
 fn finalized_numeraire_checked_principals_bind_scoped_native_event(){
  let f:Vec<_>=include_str!("../fixtures/swap-coverage/custody-numeraire26/cases.tsv").lines().find(|l|l.ends_with("numeraire")).unwrap().split('\t').collect();
  let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/custody-numeraire26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let source=f[1].parse::<u32>().unwrap();
  let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let input=walk[source as usize+1].data().clone();let output=walk[source as usize+2].data().clone();
  let decoded=crate::process_transaction(tx.clone()).unwrap();let swaps:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(swaps.len(),1);let s=swaps[0];
  assert!(s.transfer_verified);assert_eq!(s.protocol,Protocol::Numeraire);assert_eq!(base58::encode(&s.amm_pool),f[2]);assert_eq!(base58::encode(&s.user),f[3]);assert_eq!(base58::encode(&s.input_mint),f[4]);assert_eq!(base58::encode(&s.output_mint),f[6]);assert_eq!(s.input_amount,f[5].parse::<u64>().unwrap());assert_eq!(s.output_amount,f[7].parse::<u64>().unwrap());assert_eq!(s.source_transfer_index,Some(f[8].parse::<u32>().unwrap()));
  for mode in 0..29{let mut wrong=tx.clone();let meta=wrong.meta.as_option_mut().unwrap();
   if mode<23{let target=if mode<13{&native}else if mode<18{&input}else{&output};let n=meta.inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==*target).unwrap();let a=n.accounts.clone();
    match mode{0=>n.data[0]^=1,1=>n.data.push(0),2=>n.data[8]=10,3=>n.data[9]=n.data[8],4=>n.data[10]^=1,5=>n.accounts[0]=a[8],6=>n.accounts[1]=a[2],7=>n.accounts[5]=a[6],8=>n.accounts[7]=a[0],9=>n.accounts[9]=a[0],10=>n.accounts[10]=a[0],11=>n.accounts[8]=a[0],12=>n.data[18..26].copy_from_slice(&(s.output_amount+1).to_le_bytes()),13|18=>n.data[1]^=1,14|19=>n.accounts[0]=a[2],15|20=>n.accounts[2]=a[0],16|21=>n.accounts[3]=a[0],_=>n.stack_height=Some(99)}
   }else if mode<27{let l=meta.log_messages.iter_mut().find(|l|l.starts_with("Program data: ")).unwrap();let mut e=base64::engine::general_purpose::STANDARD.decode(&l[14..]).unwrap();e[match mode{23=>8,24=>16,25=>32,_=>66}]^=1;*l=format!("Program data: {}",base64::engine::general_purpose::STANDARD.encode(e));}
   else if mode==27{meta.err=buffa::MessageField::some(Default::default());}else{meta.log_messages.clear();}
   assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"invalid Numeraire mode{mode}");
  }
 }
 #[test]
 fn archived_native_directions_preserve_exact_user_debits_and_physical_positions(){
  let cases=include_str!("../fixtures/swap-coverage/wide-numeraire26/cases.tsv");let mut reverse=0;
  for line in cases.lines(){let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
   let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/wide-numeraire26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
   let decoded=crate::process_transaction(tx.clone()).unwrap();let matches:Vec<_>=decoded.swaps.iter().filter(|row|row.source_index==Some(source)).collect();assert_eq!(matches.len(),1,"{}:{}",f[0],source);let row=matches[0];
   assert!(row.transfer_verified);assert_eq!(row.protocol,Protocol::Numeraire);assert_eq!(row.source_transfer_index,Some(f[8].parse().unwrap()));
   assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),(f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
   let walk:Vec<_>=tx.walk_instructions().collect();let native_accounts=walk[source as usize].accounts();let first=walk[source as usize+1].accounts();let output_first=first[0].0==native_accounts[6].0;
   if !output_first{continue;}reverse+=1;
   let mints=TokenMintLookup::new(&tx,tx.meta.as_option().unwrap());
   for mode in 0..9{let mut routed=(*row).clone();routed.protocol=Protocol::JupiterV6.into();routed.source_index=None;routed.source_transfer_index=None;
    match mode{0=>{},1=>routed.amm_pool=vec![0;32],2=>routed.user=vec![0;32],3=>routed.input_mint=vec![0;32],4=>routed.output_mint=vec![0;32],5=>routed.input_amount+=1,6=>routed.output_amount+=1,7=>{routed.source_index=Some(source);routed.source_transfer_index=Some(source);},_=>{routed.source_index=Some(source);routed.source_transfer_index=Some(source+2);}}
    let mut candidates=vec![routed];crate::quality::verify_with_fees(&tx,&mints,&mut candidates,false,|_,_|None);
    if mode==0{assert_eq!(candidates.len(),1,"routed native positive {}:{}",f[0],source);assert_eq!(candidates[0].protocol,Protocol::Numeraire);assert_eq!(candidates[0].source_transfer_index,row.source_transfer_index);}
    else{assert!(candidates.is_empty(),"{}:{} invalid routed mode{}",f[0],source,mode);}
   }

   let mut positions=Vec::new();for (root,_)in tx.transaction.as_option().unwrap().message.as_option().unwrap().instructions.iter().enumerate(){positions.push((root,None));if let Some((g,group))=tx.meta.as_option().unwrap().inner_instructions.iter().enumerate().find(|(_,g)|g.index==root as u32){for (n,_)in group.instructions.iter().enumerate(){positions.push((root,Some((g,n))));}}}
   for mode in 0..24 {let mut wrong=tx.clone();
    if mode==22{wrong.meta.as_option_mut().unwrap().err=buffa::MessageField::some(Default::default());}
    else if mode==23{wrong.meta.as_option_mut().unwrap().log_messages.clear();}
    else{let target=source as usize+if mode<6{0}else{1+(mode-6)%2};let(root,inner)=positions[target];
     if inner.is_none(){let call=&mut wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[root];match mode%6{0=>call.data[0]^=1,1=>call.data.push(0),2=>call.accounts[0]=call.accounts[1],3=>call.accounts[1]=call.accounts[0],4=>{let n=if mode<6{10}else{1};call.data[n]^=1;},_=>call.accounts[2]=call.accounts[0]};}
     else{let(g,n)=inner.unwrap();let call=&mut wrong.meta.as_option_mut().unwrap().inner_instructions[g].instructions[n];match mode%6{0=>call.data[0]^=1,1=>call.data.push(0),2=>call.accounts[0]=call.accounts[1],3=>call.accounts[1]=call.accounts[0],4=>{let n=if mode<6{10}else{1};call.data[n]^=1;},_=>call.stack_height=Some(99)};}
    }
    assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|row|row.source_index!=Some(source))),"{}:{} invalid mode{}",f[0],source,mode);
   }
  }
  assert!(reverse>0,"retained real output-first originals required");
 }

}
