//! Swap-only layouts from the program-owned Ghosty and Memefun IDLs.
//! Direct, committed principal receipts bind the input request and payout;
//! deposit, fee collection, claim and curve creation never match these tags.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58, base58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const GHOSTY: [u8;32] = b58!("ghosty4ZU1Qk1HN7Ymz4pZ15QfspzJZgSYFkdKN6ZLK");
pub(crate) const MEMEFUN: [u8;32] = b58!("MEME4gfGe6itEpxgM1xaBVmiFF5XuCTc4oVXUE5w56F");
const SWAP: [u8;8] = [248,198,158,145,225,117,135,200];

pub(crate) fn layout(ix:&InstructionView)->Option<Layout> {
    let a=ix.accounts();let d=ix.data();let p=ix.program_id();
    if !d.starts_with(&SWAP) {return None;}
    let (protocol,pool,authority,users,vaults)=if p.0.as_slice()==GHOSTY {
        if a.len()!=16 || d.len()!=24 || a[12].0.as_slice()!=b58!("11111111111111111111111111111111")
            || a[13].0.as_slice()!=b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
            || ![GHOSTY.as_slice(),b58!("Sysvar1nstructions1111111111111111111111111").as_slice()].contains(&a[11].0.as_slice())
            || [14,15].iter().any(|i| ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&a[*i].0.as_slice()))
            || a[..11].iter().enumerate().any(|(i,k)|a[..i].iter().any(|v|v.0==k.0)) {return None;}
        (Protocol::Ghosty,1,0,[8,9],[6,7])
    }else if p.0.as_slice()==MEMEFUN {
        if a.len()!=10 || d.len()!=25 || d[24]>1 || a[7].0.as_slice()!=b58!("So11111111111111111111111111111111111111112")
            || a[8].0.as_slice()!=token_2022::PROGRAM_ID || a[9].0.as_slice()!=token::PROGRAM_ID
            || a[..8].iter().enumerate().any(|(i,k)|a[..i].iter().any(|v|v.0==k.0)) {return None;}
        (Protocol::Memefun,0,5,[3,4],if d[24]==1 {[2,1]}else{[1,2]})
    }else{return None;};
    let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);if amount==0{return None;}
    Some(Layout{protocol,pool,authority,users,vaults,amount,exact_input:true,input_limit:false,
        input_first:true,fixed_users:true,minimum_output:u64::from_le_bytes(d[16..24].try_into().ok()?)})
}

pub(crate) fn settlement(tx:&ConfirmedTransaction,source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->bool {
    let Some(l)=layout(ix) else{return false;};let a=ix.accounts();
    let depth=if ix.is_root(){1}else{ix.stack_height()};
    if depth==0 || calls.len()!=2 || !successful.contains(&source){return false;}
    let ghost=l.protocol==Protocol::Ghosty;
    let mint_roles=if ghost{[2,3]}else if ix.data()[24]==1{[7,6]}else{[6,7]};
    let program_roles=if ghost{[14,15]}else if ix.data()[24]==1{[9,8]}else{[8,9]};
    let Some(meta)=tx.meta.as_ref()else{return false;};let resolved=tx.resolved_accounts();
    for(n,c)in calls.iter().enumerate(){
        let ca=c.accounts();let cd=c.data();let mint=mint_roles[n];
        let (from,to,authority)=if n==0{(l.users[0],l.vaults[0],l.authority)}else{(l.vaults[1],l.users[1],l.pool)};
        if c.is_root() || c.stack_height()!=depth+1 || !successful.contains(&(source+1+n as u32))
            || c.program_id().0!=a[program_roles[n]].0 || ca.len()!=4 || cd.len()!=10 || cd[0]!=12
            || ca.iter().zip([from,mint,to,authority]).any(|(v,i)|v.0!=a[i].0)
            || mints.mint_for(a[from].0).as_deref()!=Some(a[mint].0.as_slice())
            || mints.mint_for(a[to].0).as_deref()!=Some(a[mint].0.as_slice()){return false;}
        let Some(vault)=resolved.iter().position(|key|*key==a[l.vaults[n]].0)else{return false;};
        let snapshots:Vec<_>=meta.pre_token_balances.iter().chain(&meta.post_token_balances)
            .filter(|b|b.account_index as usize==vault).collect();
        if snapshots.is_empty() || snapshots.iter().any(|b|b.mint!=base58::encode(a[mint].0)
            || b.owner!=base58::encode(a[l.pool].0) || b.program_id!=base58::encode(a[program_roles[n]].0)
            || b.ui_token_amount.as_ref().is_none_or(|v|v.decimals!=u32::from(cd[9]))){return false;}
        let amount=u64::from_le_bytes(cd[1..9].try_into().unwrap());
        if amount==0 || n==0&&amount!=l.amount || n==1&&amount<l.minimum_output{return false;}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finalized_anchor_markets_bind_exact_receipts_and_reject_fabrications() {
        for (fixture,count) in [("anchor-markets26",7),("parent-spot26",22),("sanctum-infinity26",2)] {
        let dir=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage").join(fixture);
        let rows=std::fs::read_to_string(dir.join("cases.tsv")).unwrap();assert_eq!(rows.lines().count(),count);
        for row in rows.lines(){
            let f:Vec<_>=row.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(dir.join(format!("{}.pb",f[0]))).unwrap()).unwrap();
            for mode in 0..12 {
                let mut changed=tx.clone();
                let native=tx.walk_instructions().nth(source as usize).unwrap();
                let l=crate::native_venues::layout(&native).unwrap();
                let vault=native.accounts()[l.vaults[0]].0.clone();
                let output_index=tx.walk_instructions().enumerate().skip(source as usize+1).find(|(_,c)|c.data().first()==Some(&12) && c.accounts().len()>=4 && c.accounts()[0].0==native.accounts()[l.vaults[1]].0 && c.accounts()[2].0==native.accounts()[l.users[1]].0).unwrap().0 as u32;
                let minimum_offset=match l.protocol{Protocol::DrFraudsworth=>17,Protocol::SanctumInfinity=>11,_=>16};
                let amount_offset=if l.protocol==Protocol::SanctumInfinity{19}else{8};
                if mode==9 {let position=changed.resolved_accounts().iter().position(|k|**k==vault).unwrap();changed.meta.as_mut().unwrap().post_token_balances.retain(|b|b.account_index as usize!=position);}
                if mode==10 {changed.meta.as_mut().unwrap().log_messages.retain(|l|l!=&format!("Program {} success",base58::encode(native.program_id().0)));}
                if mode==11 {let index=changed.resolved_accounts().iter().position(|k|**k==vault).unwrap();for b in changed.meta.as_mut().unwrap().pre_token_balances.iter_mut(){if b.account_index as usize==index {b.owner=base58::encode(native.accounts()[l.authority].0);}}}
                let mut index=0u32;
                let meta=changed.meta.as_mut().unwrap();
                for(root,ix)in changed.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions.iter_mut().enumerate(){
                    if index==source {alter(&mut ix.data,&mut ix.accounts,mode,false,f[8],minimum_offset,amount_offset);}index+=1;
                    if let Some(group)=meta.inner_instructions.iter_mut().find(|g|g.index==root as u32){for child in &mut group.instructions{
                        if index==source||index==output_index {alter(&mut child.data,&mut child.accounts,mode,index==output_index,f[8],minimum_offset,amount_offset);}index+=1;
                    }}
                }
                let result=crate::process_transaction_with_metadata(changed,None,|_,_|None,|_,_|None,|mint,_|{
                    let data=std::fs::read(dir.join("fee-mints").join(format!("{}.bin",base58::encode(mint)))).ok()?;
                    let(schedule,initialized)=crate::token_fees::owned_mint(&token_2022::PROGRAM_ID,&data)?;
                    initialized.then(||schedule.at(1055)).flatten()
                });
                let rows:Vec<_>=result.as_ref().map_or(Vec::new(),|t|t.swaps.iter().filter(|s|s.source_index==Some(source)).collect());
                if mode==0 {
                    assert_eq!(rows.len(),1,"{}",f[0]);let s=rows[0];assert_eq!(s.protocol,f[2].parse::<i32>().unwrap());
                    assert_eq!(base58::encode(&s.amm_pool),f[3]);assert_eq!(base58::encode(&s.user),f[4]);
                    assert_eq!(base58::encode(&s.input_mint),f[5]);assert_eq!(s.input_amount,f[6].parse::<u64>().unwrap());
                    assert_eq!(base58::encode(&s.output_mint),f[7]);assert_eq!(s.output_amount,f[8].parse::<u64>().unwrap());assert!(s.transfer_verified);
                }else{assert!(rows.is_empty(),"{} mutation{mode}",f[0]);}
            }
        }
    }
    }
    fn alter(data:&mut Vec<u8>,accounts:&mut Vec<u8>,mode:u8,output:bool,proceeds:&str,minimum_offset:usize,amount_offset:usize){
        if output {match mode{5=>data[1..9].copy_from_slice(&0u64.to_le_bytes()),6=>accounts[3]=accounts[0],7=>data[9]=data[9].wrapping_add(1),8=>{data.pop();},_=>{}}}
        else {match mode{1=>data[0]^=1,2=>data.push(0),3=>{let amount=u64::from_le_bytes(data[amount_offset..amount_offset+8].try_into().unwrap());data[amount_offset..amount_offset+8].copy_from_slice(&(amount+1).to_le_bytes());},4=>data[minimum_offset..minimum_offset+8].copy_from_slice(&(proceeds.parse::<u64>().unwrap()+1).to_le_bytes()),_=>{}}}
    }
}
