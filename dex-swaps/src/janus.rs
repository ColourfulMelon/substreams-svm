//! Janus outcome-to-cash exchange through its typed Predict merge contract.
use crate::{native_venues::Layout,token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{block_view::InstructionView,pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::token_2022;
pub(crate) const PROGRAM_ID:[u8;32]=b58!("JanusXpm3gsW3c9ErNoUgHppL8dGLvZKB7uekkJEYFP");
const PREDICT:[u8;32]=b58!("prediCtPZCttYMvm2W3PtxmMxLmT1dtN7riU6Cxh6tM");
pub(crate) fn layout(ix:&InstructionView)->Option<Layout>{let a=ix.accounts();let d=ix.data();
 if ix.program_id().0.as_slice()!=PROGRAM_ID||d.len()!=12||d[0..3]!=[7,0,1]||d[11]!=0||a.len()!=19
  ||a[14].0.as_slice()!=token_2022::PROGRAM_ID||a[16].0.as_slice()!=token_2022::PROGRAM_ID||a[17].0.as_slice()!=PREDICT
  ||a[5].0==a[6].0||a[7].0==a[15].0||a[11].0==a[13].0||a[12].0==a[5].0{return None;}
 let pda=solana_pubkey::Pubkey::find_program_address(&[a[1].0,a[4].0,b"market"],&solana_pubkey::Pubkey::new_from_array(PROGRAM_ID)).0;
 if pda.as_ref()!=a[2].0.as_slice(){return None;}let amount=u64::from_le_bytes(d[3..11].try_into().ok()?);if amount==0{return None;}
 Some(Layout{protocol:Protocol::Janus,pool:2,authority:0,users:[12,11],vaults:[5,7],amount,minimum_output:0,exact_input:true,input_limit:false,input_first:true,fixed_users:true})
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)>{
 let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};if depth==0||calls.len()!=7||!successful.contains(&source)||calls.iter().enumerate().any(|(i,_)|!successful.contains(&(source+i as u32+1))){return None;}
 let c=&calls[1];let ca=c.accounts();let cd=c.data();let roles=[1,4,8,9,10,7,15,5,6,14,16];
 if c.stack_height()!=depth+1||c.program_id().0.as_slice()!=PREDICT||cd.len()!=16||cd[..8]!=[148,141,236,47,174,126,69,111]||u64::from_le_bytes(cd[8..16].try_into().ok()?)!=l.amount||ca.len()!=11||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0){return None;}
 let mut amounts=[0u64;4];for (n,cidx,roles,height)in [(0,0,[12,9,5,0],depth+1),(1,4,[15,8,7,4],depth+2),(2,5,[7,8,11,1],depth+1),(3,6,[7,8,13,1],depth+1)]{
 let c=&calls[cidx];let d=c.data();let ca=c.accounts();if c.program_id().0.as_slice()!=token_2022::PROGRAM_ID||c.stack_height()!=height||d.len()!=10||d[0]!=12||ca.len()!=4||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
 ||mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[roles[1]].0.as_slice())||mints.mint_for(a[roles[2]].0).as_deref()!=Some(a[roles[1]].0.as_slice()){return None;}amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);}
 for (n,roles)in [(2,[5,9,1]),(3,[6,10,1])]{let c=&calls[n];let ca=c.accounts();let d=c.data();if c.program_id().0.as_slice()!=token_2022::PROGRAM_ID||c.stack_height()!=depth+2||ca.len()!=3||ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)||d.len()!=9||d[0]!=8||u64::from_le_bytes(d[1..9].try_into().ok()?)!=l.amount{return None;}}
 if amounts[0]!=l.amount||amounts[1]!=l.amount||amounts[2]==0||amounts[2].checked_add(amounts[3])?>l.amount{return None;}
    let meta=tx.meta.as_option()?;let resolved=tx.resolved_accounts();
    for role in [5,6,7] {
        let address=a[role].0;let position=resolved.iter().position(|key|*key==address)?;
        let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;
        let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
        if before.mint!=after.mint || before.owner!=substreams_solana::base58::encode(a[1].0) || after.owner!=before.owner { return None; }
        let mut change=0i128;
        for (position,c) in tx.walk_instructions().enumerate() {
            let ca=c.accounts();let d=c.data();
            if ![token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
                || !ca.iter().any(|key|key.0==address) { continue; }
            if !successful.contains(&(position as u32)) { return None; }
            if d.len()==9&&d[0]==8 {if ca.len()!=3||ca[0].0!=address||ca[2].0!=a[1].0||mints.mint_for(address).as_deref()!=Some(ca[1].0.as_slice()){return None;}change=change.checked_sub(i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?)))?;continue;}
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
 fn finalized_janus_merge_trade_binds_market_pda_inventory_burns_and_cash_fee() {
 let f:Vec<_>=include_str!("../fixtures/swap-coverage/janus-native26/cases.tsv").trim().split('\t').collect();let source=f[1].parse::<u32>().unwrap();let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/janus-native26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let ms=TokenMintLookup::new(&tx,tx.meta.as_option().unwrap());let success=(0..walk.len()as u32).collect();assert_eq!(settlement(source,&tx,&walk[source as usize],&walk[source as usize+1..source as usize+8],&ms,&success),Some((2330475,2276876)));
 let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(rows.len(),1);let row=rows[0];assert!(row.transfer_verified);assert_eq!((row.protocol,row.source_transfer_index),(Protocol::Janus.into(),Some(source+1)));assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),(f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
 for mode in 0..43 {let mut wrong=tx.clone();if mode<15 {let n=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();let a=n.accounts.clone();match mode {0=>n.data[0]=8,1=>n.data.push(0),2=>n.data[1]=1,3=>n.data[2]=0,4=>n.data[11]=1,5=>n.data[3..11].copy_from_slice(&(row.input_amount+1).to_le_bytes()),6=>n.data[3..11].fill(0),7=>n.accounts[2]=a[0],8=>n.accounts[1]=a[0],9=>n.accounts[4]=a[3],10=>n.accounts[7]=a[5],11=>n.accounts[5]=a[6],12=>n.accounts[17]=a[16],13=>n.accounts[11]=a[13],_=>{n.accounts.pop();}}}
 else if mode<36 {let target=source+1+(mode-15)/3;let root_at=walk[..target as usize].iter().rposition(|x|x.is_root()).unwrap();let root=walk[..=root_at].iter().filter(|x|x.is_root()).count()-1;let within=target as usize-root_at-1;let n=&mut wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().find(|g|g.index==root as u32).unwrap().instructions[within];match (mode-15)%3 {0=>{let off=if target==source+2{8}else{1};let amount=u64::from_le_bytes(n.data[off..off+8].try_into().unwrap())+1;n.data[off..off+8].copy_from_slice(&amount.to_le_bytes());},1=>n.accounts[0]=n.accounts[1],_=>n.stack_height=Some(99)}}
 else if mode==36 {wrong.meta.as_option_mut().unwrap().err=buffa::MessageField::some(Default::default());}else if mode==37 {wrong.meta.as_option_mut().unwrap().log_messages.clear();}else if mode==38 {wrong.meta.as_option_mut().unwrap().post_token_balances.clear();}else if mode==39 {wrong.meta.as_option_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}else {let a=walk[source as usize].accounts();let vault=a[(mode-40)as usize+5].0;let position=tx.resolved_accounts().iter().position(|x|*x==vault).unwrap();let n=wrong.meta.as_option_mut().unwrap().post_token_balances.iter_mut().find(|b|b.account_index as usize==position).unwrap();let amount=n.ui_token_amount.as_option().unwrap().amount.parse::<u64>().unwrap();n.ui_token_amount.as_option_mut().unwrap().amount=(amount+1).to_string();}
 assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"Janus mode{mode}");}
 }
}
