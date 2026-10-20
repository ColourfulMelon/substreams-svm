//! GatorSwap's deployed SBPF v2 contract (deployment slot 450446971).
//! The final byte selects swap; the first forty bytes are input atoms and mint.
//! The extended body contains an opaque signed quote, not an output minimum.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::{token, token_2022};
pub(crate) const PROGRAM_ID: [u8;32] = b58!("gatorLx9aC1e5ZWAXscv5QRKiLXnLPLXjftVc81h1Hr");
const CONFIG: [u8;32] = b58!("CKQdReQgzubcDznauYzDzJLmqU3NXPLPKjZh4RRDgKZN");
pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let d=ix.data(); let a=ix.accounts();
    if ix.program_id().0.as_slice()!=PROGRAM_ID || !matches!(d.len(),41|113) || d.last()!=Some(&1)
        || !matches!(a.len(),15|17|18|36) || a[6].0.as_slice()!=token::PROGRAM_ID
        || a[7].0.as_slice()!=token_2022::PROGRAM_ID || a[10].0.iter().any(|n|*n!=0)
        || a[11].0.as_slice()!=CONFIG || a[..12].iter().enumerate().any(|(i,k)| a[..i].iter().any(|other|other.0==k.0)) { return None; }
    let amount=u64::from_le_bytes(d[..8].try_into().ok()?); if amount==0 { return None; }
    Some(Layout{protocol:Protocol::Gator,pool:3,authority:0,users:[1,2],vaults:[4,5],amount,
        minimum_output:0,exact_input:true,input_limit:false,input_first:false,fixed_users:false})
}
pub(crate) fn settlement(tx:&ConfirmedTransaction, source:u32, ix:&InstructionView, calls:&[InstructionView], mints:&TokenMintLookup, successful:&HashSet<u32>) -> Option<(u64,u64)> {
    let l=layout(ix)?; let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
    if depth==0 || !successful.contains(&source) || !matches!(calls.len(),2|3)
        || calls.iter().enumerate().any(|(n,c)|c.is_root() || c.stack_height()!=depth+1 || !successful.contains(&(source+1+n as u32))) { return None; }
    let offset=calls.len()-2;
    if offset==1 {
        let c=&calls[0];let ca=c.accounts();let d=c.data();
        // A newly initialized quote-state account is funded separately from
        // both principal receipts. Rent is the actual SystemCreate amount.
        if c.program_id().0.iter().any(|n|*n!=0) || ca.len()!=2 || ca[0].0!=a[0].0 || ca[1].0!=a[8].0
            || d.len()!=52 || d[..4]!=[0;4] || u64::from_le_bytes(d[12..20].try_into().ok()?)!=512
            || d[20..52]!=PROGRAM_ID { return None; }
    }
    let input_mint=ix.data().get(8..40)?;
    let side=if mints.mint_for(a[4].0).as_deref()==Some(input_mint){0}
        else if mints.mint_for(a[5].0).as_deref()==Some(input_mint){1}else{return None;};
    let output_mint=mints.mint_for(a[5-side].0)?;
    if input_mint==output_mint || mints.mint_for(a[1+side].0).as_deref()!=Some(input_mint)
        || mints.mint_for(a[2-side].0).as_deref()!=Some(output_mint.as_slice()) { return None; }
    let mut atoms=[0u64;2];
    for (n,c) in calls[offset..].iter().enumerate() {
        let d=c.data();let ca=c.accounts();let roles=if n==0{[1+side,4+side,0]}else{[5-side,2-side,3]};
        // Plain successful Token-2022 Transfer also requires a mint without
        // transfer-fee configuration. The global receipt gate still proves net.
        if ![a[6].0.as_slice(),a[7].0.as_slice()].contains(&c.program_id().0.as_slice()) || d.len()!=9 || d[0]!=3 || ca.len()!=3
            || ca.iter().zip(roles).any(|(key,role)|key.0!=a[role].0) { return None; }
        atoms[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
    }
    // A temporary WSOL recipient can disappear before post-token balances.
    // Independently pin output against both persistent owned vault ledgers;
    // summing only this call would misattribute other swaps in the transaction.
    let meta=tx.meta.as_option()?;let resolved=tx.resolved_accounts();
    for role in [4,5] {
        let address=a[role].0;let position=resolved.iter().position(|key|*key==address)?;
        let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;
        let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
        if before.mint!=after.mint { return None; }
        let mut change=0i128;
        for (position,c) in tx.walk_instructions().enumerate() {
            let ca=c.accounts();let d=c.data();
            if ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
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
        let before=before.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?;
        let after=after.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?;
        if i128::from(after)-i128::from(before)!=change { return None; }
    }
    (atoms[0]==l.amount && atoms[1]>0).then_some((atoms[0],atoms[1]))
}

#[cfg(test)]
mod tests {
 use super::*;
 use substreams_solana::{base58,pb::sf::solana::r#type::v1::ConfirmedTransaction};
 #[test]
 fn finalized_gator_plain_and_signed_swaps_bind_both_pool_sides() {
  let fees:Vec<_>=include_str!("../fixtures/swap-coverage/gator-live26/fee-free-mints.tsv").lines().map(|address|{
   let mint=base58::decode(address).unwrap();let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/gator-live26/{}.mint",env!("CARGO_MANIFEST_DIR"),address)).unwrap();
   let (schedule,initialized)=crate::token_fees::owned_mint(&token_2022::PROGRAM_ID,&bytes).unwrap();let fee=schedule.at(0).unwrap();
   assert!(initialized&&fee.basis_points==0&&fee.maximum_fee==0);(mint,fee)
  }).collect();
  let lookup=|address:&[u8],_:u32|fees.iter().find(|(mint,_)|mint==address).map(|(_,fee)|*fee);
  let mut count=0;let mut shapes=HashSet::new();
  for line in include_str!("../fixtures/swap-coverage/gator-live26/cases.tsv").lines() {
   let f:Vec<_>=line.split('\t').collect();count+=1;
   let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/gator-live26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();
   let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let source=f[1].parse::<u32>().unwrap();
   let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let a=walk[source as usize].accounts();
   let first=f[8].parse::<u32>().unwrap();let input=walk[first as usize].data().clone();let output=walk[first as usize+1].data().clone();
   let decoded=crate::process_transaction_with_metadata(tx.clone(),None,|_,_|None,|_,_|None,lookup).unwrap();
   let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(rows.len(),1,"{}",f[0]);let s=rows[0];
   assert!(s.transfer_verified);assert_eq!(s.protocol,Protocol::Gator);assert_eq!(s.source_transfer_index,Some(first));
   assert_eq!((base58::encode(&s.amm_pool),base58::encode(&s.user),base58::encode(&s.input_mint),s.input_amount,base58::encode(&s.output_mint),s.output_amount),
    (f[2].to_owned(),f[3].to_owned(),f[4].to_owned(),f[5].parse().unwrap(),f[6].to_owned(),f[7].parse().unwrap()));
   if !shapes.insert((native.len(),a.len(),first-source,walk[first as usize].program_id().0==&token_2022::PROGRAM_ID,walk[first as usize+1].program_id().0==&token_2022::PROGRAM_ID)){continue;}
   for mode in 0..18 {
    let mut wrong=tx.clone();let meta=wrong.meta.as_option_mut().unwrap();
    if mode<9 {
     let n=meta.inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();let a=n.accounts.clone();
     match mode{0=>*n.data.last_mut().unwrap()=2,1=>n.data.push(0),2=>n.data[0]^=1,3=>n.data[8]^=1,4=>n.accounts[3]=a[0],5=>n.accounts[1]=a[2],6=>n.accounts[4]=a[5],7=>n.accounts[6]=a[3],_=>n.accounts[11]=a[0]}
    }else if mode<15 {
     let group=meta.inner_instructions.iter_mut().find(|g|g.instructions.windows(2).any(|w|w[0].data==input&&w[1].data==output)).unwrap();
     let offset=group.instructions.windows(2).position(|w|w[0].data==input&&w[1].data==output).unwrap();let n=&mut group.instructions[offset+usize::from(mode>=12)];let a=n.accounts.clone();
     match mode{9|12=>n.data[1]^=1,10|13=>n.accounts[2]=a[0],_=>n.stack_height=Some(99)}
    }else if mode==15{meta.err=buffa::MessageField::some(Default::default());}
    else if mode==16{meta.log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}
    else{meta.log_messages.clear();}
    assert!(crate::process_transaction_with_metadata(wrong,None,|_,_|None,|_,_|None,lookup).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"{} invalid Gator mode{mode}",f[0]);
   }
  }
  assert_eq!(count,57);assert!(shapes.len()>=8);
 }
}
