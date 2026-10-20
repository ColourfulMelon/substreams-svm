//! Swap-only contract from Quay's owned executable, deployed at442062012.
//! ELF06e35418a8bb498deecdb85ceeced6c69dccf3b5e7a759fda59b020450a365ec:
//! opcode32 dispatch10182->10264->5522, exact17-byte payload5524,
//! direction5527, signer5540, owned market813, minimum output6893..6898.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{base58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::token;

pub(crate) const PROGRAM_ID: [u8;32] = b58!("QUayE6nexQWYNZAEqfN8FxoNwQDSu3CAzT2qq9J1ArG");
const INSTRUCTIONS: [u8;32] = b58!("Sysvar1nstructions1111111111111111111111111");

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let a=ix.accounts(); let d=ix.data();
    if ix.program_id().0.as_slice()!=PROGRAM_ID || a.len()!=14 || d.len()!=18 || d[0]!=32 || d[17]>1
        || a[11].0.as_slice()!=INSTRUCTIONS || a[12].0.as_slice()!=token::PROGRAM_ID
        || a[..11].iter().enumerate().any(|(i,k)| a[..i].iter().any(|v|v.0==k.0))
    { return None; }
    let amount=u64::from_le_bytes(d[1..9].try_into().ok()?);
    if amount==0 { return None; }
    Some(Layout { protocol:Protocol::Quay,pool:1,authority:8,
        // Accounts are already input/output ordered in both price directions.
        users:[6,7],vaults:[4,5],
        amount,exact_input:true,input_limit:false,input_first:true,fixed_users:true,
        minimum_output:u64::from_le_bytes(d[9..17].try_into().ok()?), })
}

pub(crate) fn settlement(tx:&ConfirmedTransaction,source:u32,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->bool {
    let Some(l)=layout(ix) else {return false};let a=ix.accounts();
    let depth=if ix.is_root(){1}else{ix.stack_height()};
    if depth==0 || calls.len()!=2 || !successful.contains(&source) {return false;}
    for (n,c) in calls.iter().enumerate() {
        let ca=c.accounts();let cd=c.data();
        let (from,to,auth)=if n==0 {(l.users[0],l.vaults[0],8)}else{(l.vaults[1],l.users[1],2)};
        let mint=9+n;
        if c.is_root() || c.stack_height()!=depth+1 || !successful.contains(&(source+1+n as u32))
            || c.program_id().0.as_slice()!=token::PROGRAM_ID || ca.len()!=4 || cd.len()!=10 || cd[0]!=12
            || ca.iter().zip([from,mint,to,auth]).any(|(v,i)|v.0!=a[i].0)
            || mints.mint_for(a[from].0).as_deref()!=Some(a[mint].0.as_slice())
            || mints.mint_for(a[to].0).as_deref()!=Some(a[mint].0.as_slice())
        {return false;}
        // Physical vault snapshots independently bind mint, Token program,
        // vault authority and checked decimals to the principal receipt.
        let resolved=tx.resolved_accounts();
        let Some(meta)=tx.meta.as_option() else {return false;};
        let Some(vault)=resolved.iter().position(|key| *key==a[if n==0 {l.vaults[0]} else {l.vaults[1]}].0) else {return false;};
        let snapshots:Vec<_>=meta.pre_token_balances.iter().chain(&meta.post_token_balances)
            .filter(|b| b.account_index as usize==vault).collect();
        if snapshots.is_empty() || snapshots.iter().any(|b| b.mint!=base58::encode(a[mint].0)
            || b.owner!=base58::encode(a[2].0) || b.program_id!=base58::encode(token::PROGRAM_ID)
            || b.ui_token_amount.as_option().is_none_or(|v|v.decimals!=u32::from(cd[9])))
        {return false;}
        let amount=u64::from_le_bytes(cd[1..9].try_into().unwrap());
        if amount==0 || n==0 && amount!=l.amount || n==1 && amount<l.minimum_output {return false;}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_finalized_quay_swaps_match_independent_receipts_in_both_directions() {
        let directory=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage/quay-native");
        let cases=std::fs::read_to_string(directory.join("cases.tsv")).unwrap();
        assert_eq!(cases.lines().count(),96);
        let mut directions=HashSet::new();
        for row in cases.lines() {
            let f:Vec<_>=row.split('\t').collect();
            let bytes=std::fs::read(directory.join(format!("{}.transaction.pb",f[0]))).unwrap();
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let source=f[1].parse::<u32>().unwrap();
            let native=tx.walk_instructions().nth(source as usize).unwrap();
            directions.insert(native.data()[17]);
            let input=f[2].parse::<u64>().unwrap();let output=f[3].parse::<u64>().unwrap();
            let mut matched=0;
            for swap in crate::process_transaction(tx.clone()).unwrap().swaps.iter().filter(|s|s.source_index==Some(source)) {
                assert_eq!(swap.protocol,Protocol::Quay);
                assert_eq!((swap.input_amount,swap.output_amount),(input,output));
                assert_eq!(base58::encode(&swap.amm_pool),f[4]);assert_eq!(base58::encode(&swap.user),f[5]);
                assert_eq!(base58::encode(&swap.input_mint),f[6]);assert_eq!(base58::encode(&swap.output_mint),f[7]);
                assert_eq!(swap.source_transfer_index,Some(source+1));assert!(swap.transfer_verified);matched+=1;
            }
            assert_eq!(matched,1,"{} source {source}",f[0]);
        }
        assert_eq!(directions,HashSet::from([0,1]));
    }

    #[test]
    fn quay_receipts_reject_bad_roles_amounts_snapshots_and_incomplete_commitment() {
        let directory=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage/quay-native");
        let cases=std::fs::read_to_string(directory.join("cases.tsv")).unwrap();
        let mut tested=HashSet::new();
        for row in cases.lines() {
            let f:Vec<_>=row.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
            let original=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(directory.join(format!("{}.transaction.pb",f[0]))).unwrap()).unwrap();
            let native=original.walk_instructions().nth(source as usize).unwrap();
            if !tested.insert(native.data()[17]) {continue;}
            let native_accounts:Vec<_>=native.accounts().iter().map(|a|a.0.clone()).collect();
            for mode in 0..31 {
                let mut tx=original.clone();
                for (target,position) in [(source,0),(source+1,1),(source+2,2)] {
                    let mut index=0u32;let groups=&mut tx.meta.as_option_mut().unwrap().inner_instructions;
                    for (root,ix) in tx.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().enumerate() {
                        if index==target { mutate(&mut ix.data,&mut ix.accounts,position,mode,f[2],f[3]); }
                        index+=1;
                        if let Some(group)=groups.iter_mut().find(|g|g.index==root as u32) {
                            for child in &mut group.instructions {
                                if index==target {mutate(&mut child.data,&mut child.accounts,position,mode,f[2],f[3]);if mode==25 && position==2 {child.stack_height=Some(1);}}
                                index+=1;
                            }
                        }
                    }
                }
                if mode>=26 && mode<=29 {
                    let vault=tx.resolved_accounts().iter().position(|a|*a==&native_accounts[4]).unwrap();
                    let meta=tx.meta.as_option_mut().unwrap();
                    for snapshot in meta.pre_token_balances.iter_mut().chain(&mut meta.post_token_balances) {
                        if snapshot.account_index as usize!=vault {continue;}
                        match mode {26=>snapshot.mint=base58::encode(&native_accounts[10]),27=>snapshot.owner=base58::encode(&native_accounts[8]),28=>snapshot.program_id=base58::encode(PROGRAM_ID),_=>snapshot.ui_token_amount.as_option_mut().unwrap().decimals+=1,}
                    }
                }
                if mode==30 {tx.meta.as_option_mut().unwrap().log_messages.retain(|l|l!=&format!("Program {} success",base58::encode(PROGRAM_ID)));}
                assert!(crate::process_transaction(tx).map_or(true,|t|t.swaps.iter().all(|s|s.source_index!=Some(source))),"direction {} mutation {mode}",native.data()[17]);
            }
        }
        assert_eq!(tested.len(),2);
    }

    fn mutate(data:&mut Vec<u8>,accounts:&mut Vec<u8>,position:u8,mode:u8,input:&str,output:&str) {
        if position==0 {
            match mode {0=>data[0]=31,1=>{data.pop();},2=>data.push(0),3=>data[17]=2,4=>data[1..9].copy_from_slice(&0u64.to_le_bytes()),5=>data[1..9].copy_from_slice(&(input.parse::<u64>().unwrap()+1).to_le_bytes()),6=>data[9..17].copy_from_slice(&(output.parse::<u64>().unwrap()+1).to_le_bytes()),7..=13=>accounts[(mode-7) as usize+4]=accounts[0],_=>{}}
        } else {
            match mode {14 if position==1=>data[0]=3,15 if position==2=>{data.pop();},16 if position==1=>data.push(0),17 if position==1=>data[1..9].copy_from_slice(&0u64.to_le_bytes()),18 if position==2=>data[1..9].copy_from_slice(&0u64.to_le_bytes()),19 if position==1=>data[1..9].copy_from_slice(&(input.parse::<u64>().unwrap()-1).to_le_bytes()),20..=23 if position==2=>accounts[(mode-20)as usize]=accounts[3-(mode-20)as usize],24 if position==1=>data[9]=data[9].wrapping_add(1),_=>{}}
        }
    }
}
