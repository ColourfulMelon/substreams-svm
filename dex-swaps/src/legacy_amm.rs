//! Aldrin V1 and Cropper's classic Token AMM swap contracts. Requested input
//! is a cap: constant-product ceiling division can leave atoms unspent.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const ALDRIN: [u8; 32] = b58!("AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6");
pub(crate) const CROPPER: [u8; 32] = b58!("CTMAxxk34HjKWxQ3QLZK1HpaLXmBveao3ESePXbiyfzh");

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a = ix.accounts(); let d = ix.data();
    let (protocol, authority, users, vaults, offset, fee, lp) = if ix.program_id().0.as_slice() == ALDRIN {
        if a.len() != 10 || d.len() != 25 || d[..8] != [248,198,158,145,225,117,135,200] || d[24] > 1 || a[9].0.as_slice() != token::PROGRAM_ID { return None; }
        let bid = d[24] == 0;
        (Protocol::AldrinV1, 6, if bid { [8,7] } else { [7,8] }, if bid { [4,3] } else { [3,4] }, 8, 5, 2)
    } else if ix.program_id().0.as_slice() == CROPPER {
        if a.len() != 11 || d.len() != 17 || d[0] != 1 || a[10].0.as_slice() != token::PROGRAM_ID { return None; }
        (Protocol::Cropper, 2, [4,7], [5,6], 1, 9, 8)
    } else { return None; };
    let program=solana_pubkey::Pubkey::new_from_array(if protocol==Protocol::Cropper {CROPPER}else{ALDRIN});
    let signer=solana_pubkey::Pubkey::new_from_array(a[1].0.as_slice().try_into().ok()?);
    if protocol==Protocol::AldrinV1 {
        if solana_pubkey::Pubkey::find_program_address(&[a[0].0],&program).0!=signer {return None;}
    } else if !(0..=255u8).any(|bump|solana_pubkey::Pubkey::create_program_address(&[a[0].0,&[bump]],&program).is_ok_and(|p|p==signer)) {return None;}
    let roles = [users[0],users[1],vaults[0],vaults[1],fee];
    if roles.iter().enumerate().any(|(n,role)| roles[n+1..].iter().any(|other| a[*role].0 == a[*other].0))
        || a[0].0 == a[authority].0 || roles.iter().any(|role| a[*role].0 == a[lp].0) { return None; }
    let amount = u64::from_le_bytes(d[offset..offset+8].try_into().ok()?);
    if amount == 0 { return None; }
    Some(Layout { protocol, pool:0, authority, users, vaults, amount, minimum_output:u64::from_le_bytes(d[offset+8..offset+16].try_into().ok()?),
        exact_input:true, input_limit:true, input_first:true, fixed_users:true })
}

pub(crate) fn settlement(source: u32, tx: &ConfirmedTransaction, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup, successful: &HashSet<u32>) -> Option<(u64,u64)> {
    let l = layout(ix)?; let a = ix.accounts(); let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    let cropper = l.protocol == Protocol::Cropper;
    if depth == 0 || !successful.contains(&source) || if cropper { calls.len()!=3 } else { ![2,3].contains(&calls.len()) }
        || calls.iter().enumerate().any(|(n,c)| c.program_id().0.as_slice()!=token::PROGRAM_ID || c.stack_height()!=depth+1 || !successful.contains(&(source+n as u32+1))) { return None; }
    let mut principal = [0u64;2];
    for (n,position,roles) in [(0,0,[l.users[0],l.vaults[0],l.authority]),(1,calls.len()-1,[l.vaults[1],l.users[1],1])] {
        let c=&calls[position]; let ca=c.accounts(); let d=c.data();
        if d.len()!=9 || d[0]!=3 || ca.len()!=3 || ca.iter().zip(roles).any(|(key,role)|key.0!=a[role].0) { return None; }
        principal[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
    }
    let input_mint=mints.mint_for(a[l.users[0]].0)?; let output_mint=mints.mint_for(a[l.users[1]].0)?;
    if input_mint==output_mint || mints.mint_for(a[l.vaults[0]].0).as_ref()!=Some(&input_mint) || mints.mint_for(a[l.vaults[1]].0).as_ref()!=Some(&output_mint)
        || principal[0]==0 || principal[1]==0 || principal[1]<l.minimum_output { return None; }
    let fee_role=if cropper {9}else{5}; let lp_role=if cropper {8}else{2};
    if input_mint.as_slice()==a[lp_role].0.as_slice() || output_mint.as_slice()==a[lp_role].0.as_slice() { return None; }
    if calls.len()==3 {
        let c=&calls[1]; let ca=c.accounts(); let d=c.data();
        let roles=if cropper {[l.users[0],fee_role,l.authority]}else{[lp_role,fee_role,1]};
        if d.len()!=9 || d[0]!=if cropper {3}else{7} || ca.len()!=3 || ca.iter().zip(roles).any(|(key,role)|key.0!=a[role].0) { return None; }
        let fee=u64::from_le_bytes(d[1..9].try_into().ok()?);
        if fee==0 || mints.mint_for(a[fee_role].0).as_deref()!=Some(if cropper { input_mint.as_slice() } else { a[lp_role].0.as_slice() }) { return None; }
        if cropper { principal[0]=principal[0].checked_add(fee)?; }
    }
    if principal[0]>l.amount { return None; }
    // Bind the physical receipts to all committed movements of the persistent
    // vaults/fee account, including native-output accounts the user later closes.
    let meta=tx.meta.as_ref()?; let resolved=tx.resolved_accounts();
    for role in [l.vaults[0],l.vaults[1],fee_role] {
        let address=a[role].0; let position=resolved.iter().position(|key|*key==address)?;
        let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;
        let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
        if before.mint!=after.mint || before.owner!=after.owner || before.program_id!=after.program_id
            || before.program_id!=substreams_solana::base58::encode(token::PROGRAM_ID)
            || role!=fee_role && before.owner!=substreams_solana::base58::encode(a[1].0) { return None; }
        let mut change=0i128;
        for (index,c) in tx.walk_instructions().enumerate() {
            let ca=c.accounts(); let d=c.data();
            if ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice()) || !ca.iter().any(|key|key.0==address) {continue;}
            if c.program_id().0.as_slice()!=token::PROGRAM_ID || !successful.contains(&(index as u32)) {return None;}
            if d.len()==9&&d[0]==7&&ca.len()==3&&ca[1].0==address&&role==fee_role&&!cropper&&ca[0].0==a[lp_role].0&&ca[2].0==a[1].0 {
                change=change.checked_add(i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?)))?; continue;
            }
            if d.len()!=9||d[0]!=3||ca.len()!=3||(ca[0].0!=address&&ca[1].0!=address) {return None;}
            let mint=mints.mint_for(address)?;
            if mints.mint_for(ca[0].0).as_deref()!=Some(mint.as_slice())||mints.mint_for(ca[1].0).as_deref()!=Some(mint.as_slice()) {return None;}
            let amount=i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?));
            if ca[0].0==address {change=change.checked_sub(amount)?;} if ca[1].0==address {change=change.checked_add(amount)?;}
        }
        if i128::from(after.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?)-i128::from(before.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?)!=change {return None;}
    }
    Some((principal[0],principal[1]))
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn finalized_legacy_amm_rounding_and_fee_principals() {
  let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/legacy-amm27/cases.tsv").lines() {
   let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();let tx=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(format!("{}/fixtures/swap-coverage/legacy-amm27/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap()).unwrap();
   let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let input=walk[source as usize+1].data().clone();let fee=walk[source as usize+2].data().clone();let output=walk[source as usize+3].data().clone();let crop=f[8]=="cropper";let amount=f[5].parse::<u64>().unwrap();let proceeds=f[7].parse::<u64>().unwrap();
   let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();assert_eq!(rows.len(),1,"{}",f[0]);let row=rows[0];assert!(row.transfer_verified);assert_eq!(row.protocol,if crop {Protocol::Cropper}else{Protocol::AldrinV1} as i32);assert_eq!(row.source_transfer_index,Some(source+1));assert_eq!((substreams_solana::base58::encode(&row.amm_pool),substreams_solana::base58::encode(&row.user),substreams_solana::base58::encode(&row.input_mint),row.input_amount,substreams_solana::base58::encode(&row.output_mint),row.output_amount),(f[2].into(),f[3].into(),f[4].into(),amount,f[6].into(),proceeds));
   for mode in 0..36 {let mut bad=tx.clone();
    if mode<14 {let ix=bad.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();let a=ix.accounts.clone();let offset=if crop {1}else{8};match mode {0=>ix.data[0]^=1,1=>ix.data.push(0),2=>ix.data[offset..offset+8].copy_from_slice(&(amount-1).to_le_bytes()),3=>ix.data[offset+8..offset+16].copy_from_slice(&(proceeds+1).to_le_bytes()),4=>ix.accounts[1]=a[0],5=>ix.accounts[0]=a[1],6=>ix.accounts[if crop {2}else{6}]=a[1],7=>ix.accounts[if crop {4}else{7}]=a[if crop {7}else{8}],8=>ix.accounts[if crop {5}else{3}]=a[if crop {6}else{4}],9=>ix.accounts[if crop {9}else{5}]=a[if crop {5}else{3}],10=>ix.accounts[if crop {8}else{2}]=a[if crop {4}else{7}],11=>ix.accounts[if crop {10}else{9}]=a[0],12=>{ix.accounts.pop();},_=>if crop {ix.data[0]=2}else{ix.data[24]=2}}}
    else if mode<32 {let position=(mode-14)/6;let target=[&input,&fee,&output][position];let ix=bad.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|&x.data==target).unwrap();let a=ix.accounts.clone();match (mode-14)%6 {0=>{let v=u64::from_le_bytes(ix.data[1..9].try_into().unwrap())+1;ix.data[1..9].copy_from_slice(&v.to_le_bytes());},1=>ix.data[1..9].fill(0),2=>ix.accounts[0]=a[1],3=>ix.accounts[1]=a[0],4=>ix.accounts[2]=a[0],_=>ix.stack_height=Some(99)}}
    else if mode==32 {bad.meta.as_mut().unwrap().log_messages.clear();}
    else if mode==33 {bad.meta.as_mut().unwrap().err=Some(Default::default());}
    else if mode==34 {bad.meta.as_mut().unwrap().post_token_balances.clear();}
    else {let output_vault=tx.resolved_accounts().iter().position(|a|*a==walk[source as usize+3].accounts()[0].0).unwrap() as u32;let b=bad.meta.as_mut().unwrap().post_token_balances.iter_mut().find(|x|x.account_index==output_vault).unwrap();let v=b.ui_token_amount.as_mut().unwrap();v.amount=(v.amount.parse::<u64>().unwrap()+1).to_string();}
    assert!(crate::process_transaction(bad).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"{} mode{}",f[0],mode);
   }
   count+=1;
  }assert_eq!(count,6);
 }
}
