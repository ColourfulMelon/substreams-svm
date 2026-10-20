//! Picon DLMM classic-token swaps from the official SDK's canonical ABI.
//! Native events, exact principal roles and full vault ledgers must agree.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::token;
use std::collections::HashSet;
use base64::Engine;
pub(crate) const PROGRAM_ID: [u8; 32] = b58!("pDLMMk3KW3CUSxE1Wgjhx3jSmm2spe6Ry7Mq2HT26rC");
const IN: [u8; 8] = [104,104,131,86,161,189,180,216];
const OUT: [u8; 8] = [250,73,101,33,38,207,75,184];
pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a=ix.accounts(); let d=ix.data();
    if ix.program_id().0.as_slice()!=PROGRAM_ID || d.len()!=31 || a.len()<11
        || (d[..8]!=IN && d[..8]!=OUT) || d[24]>1 || d[25..29]!=[1,0,0,0] || d[29]!=0
        || d[30]==0 || a.len()!=10+usize::from(d[30]) || a[8].0.as_slice()!=token::PROGRAM_ID || a[9].0.as_slice()!=token::PROGRAM_ID
        || a[..8].iter().enumerate().any(|(i,k)|a[..i].iter().any(|o|k.0==o.0)) { return None; }
    for (vault,mint) in [(6,2),(7,3)] {
        let ata=solana_pubkey::Pubkey::find_program_address(&[a[1].0,&token::PROGRAM_ID,a[mint].0],
            &solana_pubkey::Pubkey::new_from_array(b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"))).0;
        if a[vault].0.as_slice()!=ata.as_ref() { return None; }
    }
    let exact=d[..8]==IN; let amount=u64::from_le_bytes(d[8..16].try_into().ok()?); let limit=u64::from_le_bytes(d[16..24].try_into().ok()?);
    if amount==0 || !exact&&limit==0 { return None; }
    let xy=d[24]==1;
    Some(Layout { protocol:Protocol::PiconDlmm,pool:1,authority:0,users:if xy{[4,5]}else{[5,4]},vaults:if xy{[6,7]}else{[7,6]},
        amount:if exact{amount}else{limit},minimum_output:if exact{limit}else{amount},exact_input:exact,input_limit:!exact,input_first:true,fixed_users:true })
}
pub(crate) fn settlement(source:u32,tx:&ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>) -> Option<(u64,u64)> {
    let l=layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
    if depth==0 || calls.len()!=2 || !successful.contains(&source) { return None; }
    let xy=ix.data()[24]==1; let mut amounts=[0u64;2];
    for(n,c)in calls.iter().enumerate() {
        let roles=if n==0{[l.users[0],if xy{2}else{3},l.vaults[0],0]}else{[l.vaults[1],if xy{3}else{2},l.users[1],1]};
        let d=c.data();let ca=c.accounts();
        if c.is_root() || c.stack_height()!=depth+1 || !successful.contains(&(source+1+n as u32)) || c.program_id().0.as_slice()!=token::PROGRAM_ID
            || d.len()!=10 || d[0]!=12 || ca.len()!=4 || ca.iter().zip(roles).any(|(k,r)|k.0!=a[r].0)
            || mints.mint_for(a[roles[0]].0).as_deref()!=Some(a[roles[1]].0.as_slice()) || mints.mint_for(a[roles[2]].0).as_deref()!=Some(a[roles[1]].0.as_slice()) { return None; }
        amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
    }
    if amounts[0]==0 || amounts[1]==0 || l.exact_input&&(amounts[0]!=l.amount||amounts[1]<l.minimum_output)
        || !l.exact_input&&(amounts[0]>l.amount||amounts[1]!=l.minimum_output) { return None; }
    let meta=tx.meta.as_option()?;let keys=tx.resolved_accounts();
    for (role,mint) in [(6,2),(7,3)] {
        let address=a[role].0;let position=keys.iter().position(|key|*key==address)?;
        let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;
        let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
        if before.mint!=substreams_solana::base58::encode(a[mint].0) || before.mint!=after.mint || before.owner!=substreams_solana::base58::encode(a[1].0) || after.owner!=before.owner { return None; }
        let mut change=0i128;
        for (position,c) in tx.walk_instructions().enumerate() {
            if ![token::PROGRAM_ID.as_slice(),substreams_solana_idls::spl::token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice()) || !c.accounts().iter().any(|k|k.0==address) { continue; }
            if c.program_id().0.as_slice()!=token::PROGRAM_ID { return None; }
            let ca=c.accounts();let d=c.data();let checked=match(d.first(),d.len()){(Some(3),9)=>false,(Some(12),10)=>true,_=>return None};
            if !successful.contains(&(position as u32)) || ca.len()<if checked{4}else{3} { return None; }
            let from=ca[0].0;let to=ca[if checked{2}else{1}].0;
            if from!=address&&to!=address || mints.mint_for(from).as_deref()!=Some(a[mint].0.as_slice()) || mints.mint_for(to).as_deref()!=Some(a[mint].0.as_slice())
                || checked&&ca[1].0!=a[mint].0 { return None; }
            let amount=i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?));
            if from==address { change=change.checked_sub(amount)?; }if to==address { change=change.checked_add(amount)?; }
        }
        if i128::from(after.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?)-i128::from(before.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?)!=change { return None; }
    }
    // Bind the log event to this exact native invocation, never a sibling.
    let walk:Vec<_>=tx.walk_instructions().collect();let mut cursor=0;let mut stack=Vec::new();let mut event=None;
    for log in &meta.log_messages {
        let parts:Vec<_>=log.split_whitespace().collect();
        if parts.len()==4 && parts[0]=="Program" && parts[2]=="invoke" {
            let program=substreams_solana::base58::decode(parts[1]).ok()?;
            let height=parts[3].strip_prefix('[')?.strip_suffix(']')?.parse::<u32>().ok()?;
            let index=walk.iter().enumerate().skip(cursor).find(|(_,x)|x.program_id().0==&program&&(if x.is_root(){1}else{x.stack_height()})==height)?.0;
            if height as usize!=stack.len()+1 { return None; }cursor=index+1;stack.push(index as u32);
        } else if parts.len()>=3 && parts[0]=="Program" && matches!(parts[2],"success"|"failed:") {
            let frame=stack.pop()?;if substreams_solana::base58::encode(walk[frame as usize].program_id().0)!=parts[1] { return None; }
        } else if stack.last()==Some(&source)&&log.starts_with("Program data: ") {
            let bytes=base64::engine::general_purpose::STANDARD.decode(&log[14..]).ok()?;
            if bytes.len()!=97 || bytes[..8]!=[64,198,205,232,38,8,113,226] { continue; }
            if event.is_some() { return None; }event=Some(bytes);
        }
    }
    if !stack.is_empty() { return None; }let e=event?;
    if e[8..40]!=*a[1].0 || e[40]!=ix.data()[24] || u64::from_le_bytes(e[49..57].try_into().ok()?)!=amounts[0]
        || u64::from_le_bytes(e[57..65].try_into().ok()?)!=amounts[1] || e[65..81].iter().any(|b|*b!=0) { return None; }
    Some((amounts[0],amounts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::base58;
    #[test]
    fn finalized_picon_principals_and_derived_exact_out_require_full_evidence() {
        let mut original_count=0;
        for line in include_str!("../fixtures/swap-coverage/picon-native27/cases.tsv").lines() {
            let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
            let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/picon-native27/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap();
            let original=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();original_count+=1;
            for exact_out in [false,true] {
                let mut tx=original.clone();
                let original_native=tx.walk_instructions().nth(source as usize).unwrap().data().clone();
                if exact_out {
                    // ABI-derived variant, not an additional finalized original.
                    let n=tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().find(|n|n.data==original_native).unwrap();
                    n.data[..8].copy_from_slice(&OUT);n.data[8..16].copy_from_slice(&f[7].parse::<u64>().unwrap().to_le_bytes());n.data[16..24].copy_from_slice(&f[5].parse::<u64>().unwrap().to_le_bytes());
                    for l in &mut tx.meta.as_option_mut().unwrap().log_messages { if *l=="Program log: Instruction: SwapExactIn" {*l="Program log: Instruction: SwapExactOut".into();} }
                }
                let native=tx.walk_instructions().nth(source as usize).unwrap().data().clone();
                let input=tx.walk_instructions().nth(source as usize+1).unwrap().data().clone();
                let output=tx.walk_instructions().nth(source as usize+2).unwrap().data().clone();
                let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();
                assert_eq!(rows.len(),1,"{} derivedExactOut={exact_out}",f[0]);let row=rows[0];
                assert!(row.transfer_verified);assert_eq!(row.protocol,Protocol::PiconDlmm);assert_eq!(row.source_transfer_index,Some(source+1));
                assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),
                    (f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
                let verify_route=|transaction:&ConfirmedTransaction| {
                    let mut candidate=row.clone();candidate.protocol=Protocol::JupiterV4.into();let mut candidates=vec![candidate];
                    let mints=TokenMintLookup::new(transaction,transaction.meta.as_option().unwrap());
                    crate::quality::verify_with_fees(transaction,&mints,&mut candidates,false,|_,_|None);candidates
                };
                assert_eq!(verify_route(&tx).len(),1);
                for mode in 0..37 {
                    let mut wrong=tx.clone();
                    if mode<15 {
                        let n=wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().find(|n|n.data==native).unwrap();let a=n.accounts.clone();
                        match mode {
                            0=>n.data[0]^=1,1=>n.data.push(0),
                            2=>n.data[8..16].copy_from_slice(&(if exact_out{row.output_amount+1}else{row.input_amount+1}).to_le_bytes()),
                            3=>n.data[16..24].copy_from_slice(&(if exact_out{row.input_amount-1}else{row.output_amount+1}).to_le_bytes()),
                            4=>n.data[24]^=1,5=>n.data[25]=0,6=>n.data[29]=1,7=>n.data[30]=2,
                            8=>n.accounts[1]=a[0],9=>n.accounts[2]=a[3],10=>n.accounts[4]=a[5],11=>n.accounts[6]=a[7],12=>n.accounts[7]=a[6],13=>n.accounts[8]=a[1],_=>n.accounts[9]=a[1],
                        }
                    } else if mode<29 {
                        let n=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|n|n.data==if mode<22{input.clone()}else{output.clone()}).unwrap();let a=n.accounts.clone();
                        match (mode-15)%7 {0=>n.data[1]^=1,1=>n.data[1..9].fill(0),2=>n.accounts[0]=a[1],3=>n.accounts[1]=a[0],4=>n.accounts[2]=a[0],5=>n.accounts[3]=a[2],_=>n.stack_height=Some(99)}
                    } else if mode<35 {
                        let meta=wrong.meta.as_option_mut().unwrap();let log=meta.log_messages.iter_mut().find(|l|l.starts_with("Program data: ")&&base64::engine::general_purpose::STANDARD.decode(&l[14..]).is_ok_and(|e|e.starts_with(&[64,198,205,232,38,8,113,226]))).unwrap();
                        let mut e=base64::engine::general_purpose::STANDARD.decode(&log[14..]).unwrap();e[match mode{29=>8,30=>40,31=>49,32=>57,33=>65,_=>73}]^=1;*log=format!("Program data: {}",base64::engine::general_purpose::STANDARD.encode(e));
                    } else if mode==35 {
                        let meta=wrong.meta.as_option_mut().unwrap();let pool=f[2];let b=meta.post_token_balances.iter_mut().find(|b|b.owner==pool&&b.mint==f[6]).unwrap();let amount=b.ui_token_amount.as_option_mut().unwrap();amount.amount=(amount.amount.parse::<u64>().unwrap()+1).to_string();
                    } else {
                        wrong.meta.as_option_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: mutation",base58::encode(PROGRAM_ID)));
                    }
                    assert!(crate::process_transaction(wrong.clone()).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"native Picon mode{mode} derivedExactOut={exact_out}");
                    assert!(verify_route(&wrong).is_empty(),"routed Picon mode{mode} derivedExactOut={exact_out}");
                }
            }
        }
        assert_eq!(original_count,2);
    }
}
