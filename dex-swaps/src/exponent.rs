//! Exponent TradePt's native PT/SY exchange. BuyYt issuance wrappers do not
//! emit a duplicate price for the contained native trade.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("XPC1MM4dYACDfykNuXYZ5una2DsMDWL24CrYubCvarC");
const SY_PROGRAM:[u8;32]=b58!("XP1BRLn8eCYSygrd8er5P4GKdzqKbC3DLoSsS5UYVZy");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||a.len()!=29||d.len()!=20||d[0]!=3||d[9]!=0||d[10]!=1||d[19]!=0
  ||a[8].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||a[9].0.as_slice()!=SY_PROGRAM||a[13].0.as_slice()!=PROGRAM_ID
  ||a[3].0==a[4].0||a[5].0==a[6].0||a[3].0==a[5].0||a[4].0==a[6].0||a[3].0==a[10].0{return None;}
 let auth=solana_pubkey::Pubkey::find_program_address(&[b"__event_authority"],&solana_pubkey::Pubkey::new_from_array(PROGRAM_ID)).0;
 if a[12].0.as_slice()!=auth.as_ref(){return None;}let amount=u64::from_le_bytes(d[1..9].try_into().ok()?);if amount==0{return None;}
 Some(Layout{protocol:Protocol::Exponent,pool:1,authority:0,users:[4,3],vaults:[6,5],amount,minimum_output:u64::from_le_bytes(d[11..19].try_into().ok()?),exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
 if depth==0||calls.len()!=8||!successful.contains(&source)||calls.iter().enumerate().any(|(i,_)|!successful.contains(&(source+i as u32+1))){return None;}
 // Exact native SY setup and withdrawal CPIs bind the extra accounts. The
 // withdrawal's nested credit is inventory funding, separate from user proceeds.
 for (n,roles,opcode) in [(0,vec![14,15,16,17],7),(1,vec![18,14,15,16,19,17],10),(3,vec![1,14,15,18,16,5,8,19,17],6)]{
  let c=&calls[n];let ca=c.accounts();let d=c.data();
  if c.is_root()||c.stack_height()!=depth+1||c.program_id().0.as_slice()!=SY_PROGRAM||ca.len()!=roles.len()||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
   ||d.len()!=if n==3{9}else{1}||d[0]!=opcode{return None;}
 }
 let mut amounts=[0u64;4];for(n,cidx,roles,height)in [(0,2,[4,6,0],depth+1),(1,4,[16,5,14],depth+2),(2,5,[5,3,1],depth+1),(3,6,[5,10,1],depth+1)]{
  let c=&calls[cidx];let d=c.data();let ca=c.accounts();
  if c.is_root()||c.stack_height()!=height||c.program_id().0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID||d.len()!=9||d[0]!=3||ca.len()!=3||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0){return None;}
  let mint=if n==0{24}else{15};if mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[mint].0.as_slice())||mints.mint_for(a[roles[1]].0).as_deref()!=Some(a[mint].0.as_slice()){return None;}
  amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
 }
 let e=calls[7].data();let ea=calls[7].accounts();
 if calls[7].is_root()||calls[7].stack_height()!=depth+1||calls[7].program_id().0.as_slice()!=PROGRAM_ID||ea.len()!=1||ea[0].0!=a[12].0
  ||e.len()!=290||e[..16]!=[228,69,165,46,81,203,154,29,159,225,96,81,255,227,233,174]
  ||e[16..48]!=*a[0].0||e[48..80]!=*a[1].0||e[80..112]!=*a[3].0||e[112..144]!=*a[4].0||e[144]!=0||e[145]>1
  ||u64::from_le_bytes(e[146..154].try_into().ok()?)!=amounts[0]||u64::from_le_bytes(e[154..162].try_into().ok()?)!=amounts[2]
  ||u64::from_le_bytes(e[170..178].try_into().ok()?)!=amounts[3]||u64::from_le_bytes(calls[3].data()[1..9].try_into().ok()?)!=amounts[1]
  ||amounts[1]!=amounts[2].checked_add(amounts[3])?||amounts[0]!=l.amount||amounts[2]==0||amounts[2]<l.minimum_output{return None;}
    let meta=tx.meta.as_option()?;let resolved=tx.resolved_accounts();
    for role in [5,6] {
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
            if d.len()==9 && d[0]==7 {
                if ca.len()!=3 || ca[1].0!=address || mints.mint_for(address).as_deref()!=Some(ca[0].0.as_slice()) { return None; }
                change=change.checked_add(i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?)))?;
                continue;
            }
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
        let before=before.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?;
        let after=after.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?;
        if i128::from(after)-i128::from(before)!=change { return None; }
    }
 Some((amounts[0],amounts[2]))
}

#[cfg(test)]
mod tests {
 use super::*;use substreams_solana::base58;
 #[test]
 fn finalized_exponent_trade_binds_native_principals_inventory_and_fee() {
  let f:Vec<_>=include_str!("../fixtures/swap-coverage/exponent-native26/cases.tsv").trim().split('\t').collect();let source=f[1].parse::<u32>().unwrap();
  let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/exponent-native26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
  let ws:Vec<_>=tx.walk_instructions().collect();let ms=TokenMintLookup::new(&tx,tx.meta.as_option().unwrap());let success=(0..ws.len() as u32).collect();assert_eq!(settlement(source,&tx,&ws[source as usize],&ws[source as usize+1..source as usize+9],&ms,&success),Some((3131244803017,2632427523953)),"direct native helper");let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(rows.len(),1);let row=rows[0];assert!(row.transfer_verified);assert_eq!(row.protocol,Protocol::Exponent);assert_eq!(row.source_transfer_index,Some(source+3));
  assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),(f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));assert!(decoded.swaps.iter().all(|s|s.source_index!=Some(11)));
  let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();
  for mode in 0..42 {let mut wrong=tx.clone();
   if mode<15 {let n=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();let a=n.accounts.clone();match mode {0=>n.data[0]=4,1=>n.data.push(0),2=>n.data[9]=1,3=>n.data[10]=0,4=>n.data[19]=1,5=>n.data[1..9].copy_from_slice(&(row.input_amount+1).to_le_bytes()),6=>n.data[11..19].copy_from_slice(&(row.output_amount+1).to_le_bytes()),7=>n.accounts[1]=a[0],8=>n.accounts[3]=a[4],9=>n.accounts[5]=a[6],10=>n.accounts[12]=a[0],11=>n.accounts[9]=a[8],12=>n.accounts[15]=a[24],13=>n.accounts[24]=a[15],_=>{n.accounts.pop();}}}
   else if mode<37 {let target=match mode {15..=17=>source+1,18..=20=>source+2,21..=24=>source+3,25..=27=>source+4,28..=30=>source+5,31..=33=>source+6,34..=36=>source+7,_=>unreachable!()};let root_at=walk[..target as usize].iter().rposition(|x|x.is_root()).unwrap();let root=walk[..=root_at].iter().filter(|x|x.is_root()).count()-1;let within=target as usize-root_at-1;let n=&mut wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().find(|g|g.index==root as u32).unwrap().instructions[within];let kind=match mode {15..=17=>mode-15,18..=20=>mode-18,21..=24=>mode-21,25..=27=>mode-25,28..=30=>mode-28,31..=33=>mode-31,_=>mode-34};match kind {0=>{if n.data.len()==9{let amount=u64::from_le_bytes(n.data[1..9].try_into().unwrap())+1;n.data[1..9].copy_from_slice(&amount.to_le_bytes());}else{n.data[0]^=1;}},1=>n.accounts[0]=n.accounts[1],2=>n.stack_height=Some(99),_=>n.data.push(0)}}
   else if mode<40 {let target=source+8;let root_at=walk[..target as usize].iter().rposition(|x|x.is_root()).unwrap();let root=walk[..=root_at].iter().filter(|x|x.is_root()).count()-1;let within=target as usize-root_at-1;let n=&mut wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().find(|g|g.index==root as u32).unwrap().instructions[within];let pos=[48,154,170][(mode-37)as usize];n.data[pos]^=1;}
   else if mode==40 {wrong.meta.as_option_mut().unwrap().log_messages.clear();}else{wrong.meta.as_option_mut().unwrap().post_token_balances.clear();}
   assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"Exponent mode{mode}");
  }
 }
}
