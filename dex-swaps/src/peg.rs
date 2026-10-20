//! Peg's opcode-zero spot exchange, bound to its mint-derived quote PDA.
//! The deployed native handler admits exactly two classic-token principals.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
pub(crate) const PROGRAM_ID: [u8; 32] = b58!("pegVkBpfR9GFi5Jaa9YXAHACyM9CzCNvhc5bf8GWNyW");
const AUTHORITY: [u8; 32] = b58!("8Nvoi2pqCy143Bv9RbHsQzQVNBUWj5q4frHfwuudDPmL");
const SOL_VAULT: [u8; 32] = b58!("GWTv6VBbtcegGWvDxMQt1L13PcoL5AX789DbmMfLPZqp");
const WSOL: [u8; 32] = b58!("So11111111111111111111111111111111111111112");
pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a = ix.accounts(); let d = ix.data();
    if ix.program_id().0.as_slice() != PROGRAM_ID || a.len() != 8 || d.len() != 17 || d[0] != 0
        || a[6].0.as_slice() != AUTHORITY || a[7].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
        || a[0].0 == a[2].0 || a[1].0 == a[3].0 || a[0].0 == a[1].0 || a[2].0 == a[3].0 {
        return None;
    }
    let amount = u64::from_le_bytes(d[1..9].try_into().ok()?);
    if amount == 0 { return None; }
    Some(Layout { protocol: Protocol::Peg, pool: 4, authority: 5, users: [0, 2], vaults: [1, 3], amount,
        minimum_output: u64::from_le_bytes(d[9..17].try_into().ok()?), exact_input: true,
        input_limit: false, input_first: true, fixed_users: true })
}
pub(crate) fn settlement(source: u32, tx: &ConfirmedTransaction, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup,
    successful: &HashSet<u32>) -> Option<(u64, u64)> {
    let l = layout(ix)?; let a = ix.accounts(); let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if calls.len() != 2 || depth == 0 || !successful.contains(&source) { return None; }
    let input_mint = mints.mint_for(a[0].0)?; let output_mint = mints.mint_for(a[2].0)?;
    if input_mint == output_mint { return None; }
    let (lst, lst_vault, sol_vault) = if input_mint.as_slice() == WSOL { (&output_mint, 3, 1) }
        else if output_mint.as_slice() == WSOL { (&input_mint, 1, 3) } else { return None; };
    let pool = solana_pubkey::Pubkey::find_program_address(&[b"quote", lst],
        &solana_pubkey::Pubkey::new_from_array(PROGRAM_ID)).0;
    if a[4].0.as_slice() != pool.as_ref() || a[sol_vault].0.as_slice() != SOL_VAULT
        || mints.mint_for(a[lst_vault].0).as_deref() != Some(lst.as_slice()) { return None; }
    let mut amounts = [0u64; 2];
    for (n, c) in calls.iter().enumerate() {
        let d = c.data(); let ca = c.accounts(); let roles = if n == 0 { [0, 1, 5] } else { [3, 2, 6] };
        if c.is_root() || c.stack_height() != depth + 1 || !successful.contains(&(source + n as u32 + 1))
            || c.program_id().0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
            || d.len() != 9 || d[0] != 3 || ca.len() != 3 || ca.iter().zip(roles).any(|(k, r)|k.0 != a[r].0)
            || mints.mint_for(a[roles[0]].0) != mints.mint_for(a[roles[1]].0) { return None; }
        amounts[n] = u64::from_le_bytes(d[1..9].try_into().ok()?);
    }
    let meta=tx.meta.as_option()?;let resolved=tx.resolved_accounts();
    for role in [1,3] {
        let address=a[role].0;let position=resolved.iter().position(|key|*key==address)?;
        let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;
        let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
        if before.mint!=after.mint || before.owner!=substreams_solana::base58::encode(AUTHORITY) || after.owner!=before.owner { return None; }
        let mut change=0i128;
        for (position,c) in tx.walk_instructions().enumerate() {
            let ca=c.accounts();let d=c.data();
            if ![substreams_solana_idls::spl::token::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
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
    if amounts[0] != l.amount || amounts[1] == 0 || amounts[1] < l.minimum_output { return None; }
    Some((amounts[0], amounts[1]))
}

#[cfg(test)]
mod tests {
 use super::*; use substreams_solana::base58;
 #[test]
 fn finalized_peg_both_directions_bind_pda_and_all_vault_receipts() {
  let mut count=0;
  for line in include_str!("../fixtures/swap-coverage/peg-native26/cases.tsv").lines() {
   count+=1;let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
   let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/peg-native26/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();
   let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();let walk:Vec<_>=tx.walk_instructions().collect();
   let native=walk[source as usize].data().clone();let input=walk[source as usize+1].data().clone();let output=walk[source as usize+2].data().clone();
   let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();
   assert_eq!(rows.len(),1,"{}",f[0]);let s=rows[0];assert!(s.transfer_verified);assert_eq!(s.protocol,Protocol::Peg);assert_eq!(s.source_transfer_index,Some(source+1));
   assert_eq!((base58::encode(&s.amm_pool),base58::encode(&s.user),base58::encode(&s.input_mint),s.input_amount,base58::encode(&s.output_mint),s.output_amount),(f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
   for mode in 0..30 {let mut wrong=tx.clone();
    if mode<12 {let n=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|g.instructions.iter_mut()).find(|x|x.data==native).unwrap();let a=n.accounts.clone();match mode {0=>n.data[0]=1,1=>n.data.push(0),2=>n.data[1..9].copy_from_slice(&(s.input_amount+1).to_le_bytes()),3=>n.data[9..17].copy_from_slice(&(s.output_amount+1).to_le_bytes()),4=>n.accounts[4]=a[5],5=>n.accounts[1]=a[3],6=>n.accounts[3]=a[1],7=>n.accounts[0]=a[2],8=>n.accounts[2]=a[0],9=>n.accounts[6]=a[5],10=>n.accounts[7]=a[4],_=>{n.accounts.pop();}}}
    else if mode<26 {let g=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().find(|g|g.instructions.windows(2).any(|w|w[0].data==input&&w[1].data==output)).unwrap();let pos=g.instructions.windows(2).position(|w|w[0].data==input&&w[1].data==output).unwrap();let n=&mut g.instructions[pos+usize::from(mode>=19)];let a=n.accounts.clone();match mode {12|19=>{let amount=u64::from_le_bytes(n.data[1..9].try_into().unwrap())+1;n.data[1..9].copy_from_slice(&amount.to_le_bytes());},13|20=>n.data[1..9].fill(0),14|21=>n.accounts[0]=a[1],15|22=>n.accounts[1]=a[0],16|23=>n.accounts[2]=a[0],17|24=>n.stack_height=Some(99),_=>n.data.push(0)}}
    else if mode==26 {wrong.meta.as_option_mut().unwrap().err=buffa::MessageField::some(Default::default());}
    else if mode==27 {wrong.meta.as_option_mut().unwrap().log_messages.clear();}
    else if mode==28 {wrong.meta.as_option_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID)));}
    else {wrong.meta.as_option_mut().unwrap().post_token_balances.clear();}
    assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"Peg mode{mode}");
   }
  }assert_eq!(count,2);
 }
}
