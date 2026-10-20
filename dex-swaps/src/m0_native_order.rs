//! Complete native M0 orders from the program-owned Anchor IDL. Solver debit,
//! released order escrow, both native events and all principal ledgers agree.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::{token,token_2022};
pub(crate) const PROGRAM_ID:[u8;32]=b58!("MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK");
const FILL:[u8;8]=[219,161,238,151,122,148,159,211];
const CLOSE:[u8;8]=[242,180,199,62,212,67,188,136];
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{
 let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||d.len()!=306||a.len()!=16||d[..8]!=FILL||d[40..42]!=[1,0]
  ||d[82..86]!=1399811149u32.to_le_bytes()||d[86..90]!=d[82..86]||a[15].0.as_slice()!=PROGRAM_ID
  ||a[7].0.as_slice()!=b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")||a[8].0.iter().any(|b|*b!=0)
  ||[6,13].iter().any(|r|![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&a[*r].0.as_slice()))
  ||a[2].0==a[10].0||d[106..138]!=a[10].0[..]||d[138..170]!=a[2].0[..]||d[202..234]!=a[4].0[..]
  ||d[234..266]!=a[0].0[..]||d[274..306]!=a[0].0[..] {return None;}
 let program=solana_pubkey::Pubkey::new_from_array(PROGRAM_ID);
 for(role,seeds)in[(1,vec![b"global".as_slice()]),(9,vec![b"order".as_slice(),&d[8..40]]),(14,vec![b"__event_authority".as_slice()])]{
  if a[role].0.as_slice()!=solana_pubkey::Pubkey::find_program_address(&seeds,&program).0.as_ref(){return None;}
 }
 for(role,owner,mint,prog)in[(5,4,2,6),(12,9,10,13)]{
  let ata=solana_pubkey::Pubkey::find_program_address(&[a[owner].0,a[prog].0,a[mint].0],&solana_pubkey::Pubkey::new_from_array(b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"))).0;
  if a[role].0.as_slice()!=ata.as_ref(){return None;}
 }
 let roles=[3,5,11,12];if roles.iter().enumerate().any(|(n,r)|roles[n+1..].iter().any(|q|a[*r].0==a[*q].0)){return None;}
 let input=u64::from_le_bytes(d[266..274].try_into().ok()?);let output=u128::from_le_bytes(d[170..186].try_into().ok()?);
 if input==0||u128::from(input)!=u128::from_le_bytes(d[186..202].try_into().ok()?)||output==0||output>u128::from(u64::MAX){return None;}
 Some(Layout{protocol:Protocol::M0NativeOrder,pool:9,authority:0,users:[3,11],vaults:[5,12],amount:input,minimum_output:output as u64,exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let d=ix.data();let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0||calls.len()!=4||!successful.contains(&source)||calls.iter().enumerate().any(|(n,c)|c.is_root()||c.stack_height()!=depth+1||!successful.contains(&(source+n as u32+1))){return None;}
 for(n,roles,amount,prog)in[(0,[3,2,5,0],l.amount,6),(1,[12,10,11,9],l.minimum_output,13)]{
  let c=&calls[n];let ca=c.accounts();let b=c.data();
  if c.program_id().0!=a[prog].0||b.len()!=10||b[0]!=12||ca.len()!=4||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
   ||u64::from_le_bytes(b[1..9].try_into().ok()?)!=amount||mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[roles[1]].0.as_slice())
   ||mints.mint_for(a[roles[2]].0).as_deref()!=Some(a[roles[1]].0.as_slice()){return None;}
 }
 for(n,size,disc)in[(2,112,[120,124,109,66,249,116,174,30]),(3,48,[90,77,52,248,56,233,110,197])]{
  let c=&calls[n];let b=c.data();if c.program_id().0.as_slice()!=PROGRAM_ID||c.accounts().len()!=1||c.accounts()[0].0!=a[14].0
   ||b.len()!=size||b[..8]!=[228,69,165,46,81,203,154,29]||b[8..16]!=disc||b[16..48]!=d[8..40]{return None;}
 }
 let e=calls[2].data();if e[48..80]!=a[0].0[..]||u128::from_le_bytes(e[80..96].try_into().ok()?)!=u128::from(l.minimum_output)
  ||u128::from_le_bytes(e[96..112].try_into().ok()?)!=u128::from(l.amount){return None;}
 let meta=tx.meta.as_option()?;if meta.err.is_set(){return None;}let keys=tx.resolved_accounts();let walk:Vec<_>=tx.walk_instructions().collect();
 for(role,mint,owner,prog)in[(3,2,0,6),(5,2,4,6),(12,10,9,13),(11,10,0,13)]{
  let key=a[role].0;let pos=keys.iter().position(|k|*k==key)?;let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==pos)?;
  let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==pos);
  for b in std::iter::once(before).chain(after){if b.mint!=substreams_solana::base58::encode(a[mint].0)||b.owner!=substreams_solana::base58::encode(a[owner].0)||b.program_id!=substreams_solana::base58::encode(a[prog].0){return None;}}
  let mut change=0i128;let mut closed=false;
  for(index,c)in walk.iter().enumerate(){let ca=c.accounts();let b=c.data();
   if ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())||!ca.iter().any(|k|k.0==key){continue;}
   if c.program_id().0!=a[prog].0||!successful.contains(&(index as u32)){return None;}
   if b.as_slice()==[9]&&ca.len()==3&&role==12&&ca[0].0==key&&ca[1].0==a[4].0&&ca[2].0==a[9].0{
    let parent=walk.get(index.checked_sub(1)?)?;let pa=parent.accounts();let pd=parent.data();
    if closed||!parent.is_root()||parent.program_id().0.as_slice()!=PROGRAM_ID||pd.len()!=40||pd[..8]!=CLOSE||pd[8..40]!=d[8..40]
     ||pa.len()!=7||pa.iter().zip([4,4,9,10,12,15,13]).any(|(k,r)|k.0!=a[r].0)||c.stack_height()!=2
     ||!successful.contains(&((index-1)as u32))||index<=source as usize+4{ return None; }closed=true;continue;
   }
   if closed{return None;}let checked=match(b.first(),b.len(),ca.len()){(Some(3),9,3)=>false,(Some(12),10,4)=>true,_=>return None};
   let from=ca[0].0;let to=ca[if checked{2}else{1}].0;
   if mints.mint_for(from).as_deref()!=Some(a[mint].0.as_slice())||mints.mint_for(to).as_deref()!=Some(a[mint].0.as_slice())
    ||checked&&ca[1].0!=a[mint].0{return None;}
   let amount=i128::from(u64::from_le_bytes(b[1..9].try_into().ok()?));if from==key{change=change.checked_sub(amount)?;}if to==key{change=change.checked_add(amount)?;}
  }
  let post=if let Some(after)=after{if closed{return None;}after.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?}
   else{if !closed||*meta.post_balances.get(pos)?!=0{return None;}0};
  if i128::from(post)-i128::from(before.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?)!=change{return None;}
 }
 Some((l.amount,l.minimum_output))
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn complete_native_order_binds_release_receipts_events_and_closed_escrow(){
  let bytes=include_bytes!("../fixtures/swap-coverage/m0-native27/2RwPNAeJYJ16e75GgpxpZkBYP6jKgdxjgy27DoJeXj3ApHqPfcspyZuJHrgUL1aYJHD63sSA5WhuUDqnVEnjGW7y.pb");
  let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
  let decode=|tx|crate::process_transaction(tx).unwrap().swaps.into_iter().filter(|s|s.source_index==Some(0)).collect::<Vec<_>>();
  let original=decode(tx.clone());assert_eq!(original.len(),1);let swap=&original[0];
  assert_eq!(swap.protocol,Protocol::M0NativeOrder);assert!(swap.transfer_verified);
  assert_eq!(swap.source_transfer_index,Some(1));assert_eq!((swap.input_amount,swap.output_amount),(22_400_000,22_400_000));
  for(value,expected)in[(&swap.amm_pool,"7HQEka4bqKR9egKZnd26nLPQPButjCGAPXDNomcJojya"),(&swap.user,"8mZpjtSkD1FnkvHcue21hb4HB7oGTwK3vD2ntP1D5Z95"),
   (&swap.input_mint,"dawn7ZUF7h7anFuEsDdAU1Y3HYwikwqNMAENZsQJdNL"),(&swap.output_mint,"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")]{assert_eq!(substreams_solana::base58::encode(value),expected);}
  for mode in 0..27{
   let mut wrong=tx.clone();
   if mode<16{
    let ix=&mut wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[0];
    match mode{
     0=>ix.data[0]^=1,1=>ix.data[40]=2,2=>ix.data[82]^=1,3=>ix.data[266]^=1,
     4=>ix.data[170]^=1,5=>ix.data[186]^=1,6=>ix.data[234]^=1,7=>ix.data[274]^=1,
     8=>ix.accounts.swap(3,5),9=>ix.accounts.swap(12,11),10=>ix.accounts.swap(2,10),
     11=>ix.accounts.swap(9,1),12=>ix.accounts.swap(6,13),13=>ix.data.push(0),
     14=>{ix.data.pop();},_=>ix.accounts.swap(4,0),
    }
   }else if mode<=22{
    let inner=&mut wrong.meta.as_option_mut().unwrap().inner_instructions;
    let group=inner.iter_mut().find(|g|g.index==0).unwrap();
    match mode{
     16=>group.instructions[0].data[1]^=1,17=>group.instructions[1].data[1]^=1,
     18=>group.instructions[2].data[80]^=1,19=>group.instructions[2].data[48]^=1,
     20=>group.instructions[3].data[16]^=1,21=>group.instructions[0].accounts.swap(0,2),
     _=>group.instructions[2].accounts[0]=group.instructions[0].accounts[0],
    }
   }else if mode==23{wrong.meta.as_option_mut().unwrap().log_messages.clear();}
   else if mode==24{wrong.meta.as_option_mut().unwrap().post_token_balances[0].ui_token_amount.as_option_mut().unwrap().amount="1".into();}
   else if mode==25{wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[1].data[8]^=1;}
   else{let meta=wrong.meta.as_option_mut().unwrap();let keys=tx.resolved_accounts();let pos=keys.iter().position(|k|substreams_solana::base58::encode(k)=="HSxJu1BBWJtQMA7maAb5eB7EFaXZ2fPDZULc6RL8yhkW").unwrap();meta.post_balances[pos]=1;}
   assert!(decode(wrong.clone()).is_empty(),"malformed native order {mode}");
   let mints=TokenMintLookup::new(&wrong,wrong.meta.as_option().unwrap());let mut routed=swap.clone();routed.protocol=5.into();routed.program_id=substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();let mut rows=vec![routed];
   crate::quality::verify_and_deduplicate(&wrong,&mints,&mut rows,false);assert!(rows.is_empty(),"malformed routed order {mode}");
  }
  let lookup=TokenMintLookup::new(&tx,tx.meta.as_option().unwrap());let mut routed=swap.clone();routed.protocol=5.into();routed.program_id=substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();let mut rows=vec![routed];
  crate::quality::verify_and_deduplicate(&tx,&lookup,&mut rows,false);let mut expected=original;expected[0].program_id=substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();assert_eq!(rows,expected);
 }
}
