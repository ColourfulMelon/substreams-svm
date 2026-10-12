//! Bounded inert suffixes accepted by the owned Anchor entrypoint.
//! Swap-only dispatch and exact native principal/event witnesses remain required.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58, block_view::InstructionView};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) fn applies(ix: &InstructionView) -> bool {
    ix.program_id().0.as_slice() == b58!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C") && ix.data().len() == 40
        || ix.program_id().0.as_slice() == b58!("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN") && ix.data().len() == 32
}

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    if !applies(ix) { return None; }
    let a = ix.accounts(); let d = ix.data();
    if d[24..].iter().any(|b| *b != 0) { return None; }
    let (protocol, pool, authority, users, vaults, input_first) = if ix.program_id().0.as_slice() == b58!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C") {
        if d[..8] != [143,190,90,218,196,30,51,222] || a.len() != 13
            || a[10].0 == a[11].0 || [8,9].iter().any(|p| ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&a[*p].0.as_slice())) { return None; }
        (Protocol::RaydiumCpmm, 3, 0, [4,5], [6,7], true)
    } else {
        if d[..8] != [248,198,158,145,225,117,135,200] || a.len() != 15 || a[7].0 == a[8].0
            || [10,11].iter().any(|p| ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&a[*p].0.as_slice()))
            || a[12].0 != ix.program_id().0 || a[14].0 != ix.program_id().0 { return None; }
        (Protocol::MeteoraDbc, 2, 9, [3,4], [5,6], false)
    };
    let amount=u64::from_le_bytes(d[8..16].try_into().ok()?);
    if amount == 0 || a[users[0]].0 == a[users[1]].0 || a[vaults[0]].0 == a[vaults[1]].0
        || users.iter().any(|u| vaults.iter().any(|v|a[*u].0==a[*v].0)) { return None; }
    Some(Layout{protocol,pool,authority,users,vaults,amount,minimum_output:u64::from_le_bytes(d[16..24].try_into().ok()?),
        exact_input:true,input_limit:false,input_first,fixed_users:true})
}

pub(crate) fn settlement(source:u32,tx:&substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&HashSet<u32>)->Option<(u64,u64)> {
    let l=layout(ix)?; let a=ix.accounts(); let depth=if ix.is_root(){1}else{ix.stack_height()};
    let dbc=l.protocol==Protocol::MeteoraDbc;
    if calls.len()!=if dbc{4}else{2} || !successful.contains(&source)
        || calls.iter().enumerate().any(|(n,c)|c.stack_height()!=depth+1||!successful.contains(&(source+n as u32+1))) {return None;}
    let input_mint=mints.mint_for(a[l.users[0]].0)?;
    let (input_vault,output_vault,input_mint_role,output_mint_role,input_program,output_program,output_authority,direction)=if dbc {
        let side=if input_mint==*a[7].0 {0}else if input_mint==*a[8].0 {1}else{return None};
        (5+side,6-side,7+side,8-side,10+side,11-side,0,side)
    }else{(6,7,10,11,8,9,1,0)};
    for (position,mint) in [(l.users[0],input_mint_role),(input_vault,input_mint_role),(l.users[1],output_mint_role),(output_vault,output_mint_role)] {
        if mints.mint_for(a[position].0).as_deref()!=Some(a[mint].0.as_slice()){return None;}
    }
    let mut amounts=[0;2];
    for (n,(from,to,authority,mint,program)) in [(l.users[0],input_vault,l.authority,input_mint_role,input_program),(output_vault,l.users[1],output_authority,output_mint_role,output_program)].into_iter().enumerate(){
        let c=&calls[n];let b=c.data();let roles=c.accounts();
        if c.program_id().0!=a[program].0 || b.len()!=10 || b[0]!=12 || roles.len()!=4
            || roles[0].0!=a[from].0||roles[1].0!=a[mint].0||roles[2].0!=a[to].0||roles[3].0!=a[authority].0{return None;}
        amounts[n]=u64::from_le_bytes(b[1..9].try_into().ok()?);
    }
    if amounts[0]!=l.amount || amounts[1]==0 || amounts[1]<l.minimum_output{return None;}
    if dbc {
        // Primary owner-derived EvtSwap and EvtSwap2 are separate authenticated
        // self-CPI events. Bind request, minimum, direction and both receipts.
        for (n,length,tag,request,minimum,spent,output) in [
            (2,170,[27,60,21,213,138,170,187,147],82,90,154,106),
            (3,195,[189,66,51,168,38,80,117,153],82,90,99,123)] {
            let c=&calls[n];let b=c.data();let roles=c.accounts();
            if c.program_id().0!=ix.program_id().0 || roles.len()!=1 || roles[0].0!=a[13].0 || b.len()!=length
                || b[..8]!=[228,69,165,46,81,203,154,29]||b[8..16]!=tag
                || b[16..48]!=a[l.pool].0.as_slice()[..]||b[48..80]!=a[1].0.as_slice()[..]
                || b[80]!=direction as u8||b[81]!=0
                || u64::from_le_bytes(b[request..request+8].try_into().ok()?)!=l.amount
                || u64::from_le_bytes(b[minimum..minimum+8].try_into().ok()?)!=l.minimum_output
                || u64::from_le_bytes(b[spent..spent+8].try_into().ok()?)!=amounts[0]
                || u64::from_le_bytes(b[output..output+8].try_into().ok()?)!=amounts[1]
                || n==2&&u64::from_le_bytes(b[98..106].try_into().ok()?)!=amounts[0]
                || n==3&&(b[98]!=0||b[115..123]!=[0;8]||u64::from_le_bytes(b[107..115].try_into().ok()?)!=amounts[0]) {return None;}
        }
    }
    if l.vaults.iter().any(|position| !crate::current_proprietary::vault_ledger(tx, a[*position].0, mints, successful)) { return None; }
    Some((amounts[0],amounts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::base58;
    use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

    #[test]
    fn finalized_zero_suffix_swap_formats_bind_principals_and_reject_routed_bypasses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage/anchor-swap-zero-suffix");
        let mut positives = 0;
        let mut invalid = 0;
        for line in include_str!("../fixtures/swap-coverage/anchor-swap-zero-suffix/cases.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(root.join(format!("{}.pb", f[0]))).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let source: u32 = f[1].parse().unwrap();
            let source_ix = tx.walk_instructions().nth(source as usize).unwrap();
            let source_data = source_ix.data().clone();
            let parsed = layout(&source_ix).unwrap();
            let lookup = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            let calls: Vec<_> = tx.walk_instructions().skip(source as usize+1).take(if parsed.protocol == Protocol::MeteoraDbc { 4 } else { 2 }).collect();
            let successful: HashSet<u32> = (0..tx.walk_instructions().count() as u32).collect();
            assert!(settlement(source, &tx, &source_ix, &calls, &lookup, &successful).is_some(), "original native settlement {}", f[0]);
            let native = crate::process_transaction(tx.clone()).unwrap().swaps.into_iter()
                .find(|x| x.source_index == Some(source)).unwrap_or_else(|| panic!("finalized padded Anchor swap swap {} source={source}", f[0]));
            assert_eq!(native.protocol, parsed.protocol as i32);
            for (actual, expected) in [(&native.program_id, f[3]), (&native.amm, f[3]), (&native.amm_pool, f[4]),
                (&native.user, f[5]), (&native.input_mint, f[6]), (&native.output_mint, f[8])] {
                assert_eq!(base58::encode(actual), expected);
            }
            assert_eq!(native.input_amount.to_string(), f[7]);
            assert_eq!(native.output_amount.to_string(), f[9]);
            assert_eq!(native.source_transfer_index, Some(f[2].parse().unwrap()));
            assert!(native.transfer_verified);
            positives += 1;
            for mode in 0..15 {
                let mut wrong = tx.clone();
                if mode < 6 || mode == 11 || mode == 12 {
                    let modify = |data: &mut Vec<u8>, accounts: &mut Vec<u8>| match mode {
                        0 => data[0] ^= 128,
                        1 => data[8] ^= 1,
                        2 => accounts.swap(parsed.users[0], parsed.vaults[0]),
                        3 => accounts.swap(parsed.pool, parsed.users[0]),
                        4 => data[1] ^= 1,
                        5 => { data.pop(); },
                        11 => data[24] = 1,
                        12 => data[16..24].fill(255),
                        _ => unreachable!(),
                    };
                    let message = wrong.transaction.as_mut().unwrap().message.as_mut().unwrap();
                    if let Some(ix) = message.instructions.iter_mut().find(|x| x.data == source_data) {
                        modify(&mut ix.data, &mut ix.accounts);
                    } else {
                        let ix = wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                            .find(|x| x.data == source_data).unwrap();
                        modify(&mut ix.data, &mut ix.accounts);
                    }
                } else if mode == 6 || mode == 7 || mode == 10 || mode == 13 {
                    let target = source + 1 + calls.iter().position(|child| {
                        let a = child.accounts();
                        a[0].0 == source_ix.accounts()[parsed.users[0]].0 && mode == 6 || child.accounts().get(2).is_some_and(|a| a.0 == source_ix.accounts()[parsed.users[1]].0) && mode != 6
                    }).unwrap() as u32;
                    let roots = wrong.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions.len();
                    let mut flat = 0;
                    let mut changed = false;
                    for root_position in 0..roots {
                        flat += 1;
                        if let Some(group) = wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g| g.index == root_position as u32) {
                            for i in &mut group.instructions {
                                if flat == target {
                                    if mode == 6 || mode == 13 { i.data[1] ^= 1; } else if mode == 7 { i.accounts.swap(0, 1); }
                                    else { let last = i.accounts.len()-1; i.accounts[last] = i.accounts[1]; }
                                    changed = true;
                                }
                                flat += 1;
                            }
                        }
                    }
                    assert!(changed);
                } else if mode == 14 {
                    wrong.meta.as_mut().unwrap().post_token_balances.clear();
                } else if mode == 8 {
                    wrong.meta.as_mut().unwrap().log_messages.clear();
                } else {
                    let logs = &mut wrong.meta.as_mut().unwrap().log_messages;
                    let depth = if source_ix.is_root() { 1 } else { source_ix.stack_height() };
                    let ordinal = tx.walk_instructions().take(source as usize + 1).filter(|ix| ix.program_id().0 == source_ix.program_id().0
                        && (if ix.is_root() { 1 } else { ix.stack_height() }) == depth).count()-1;
                    let begin = logs.iter().enumerate().filter(|(_,log)| *log == &format!("Program {} invoke [{depth}]", f[3]))
                        .nth(ordinal).unwrap().0;
                    let end = logs.iter().enumerate().skip(begin+1).find(|(_,log)| *log == &format!("Program {} success", f[3])).unwrap().0;
                    logs[end] = format!("Program {} failed: custom program error: 1", f[3]);
                }
                assert!(!crate::process_transaction(wrong.clone()).unwrap().swaps.iter().any(|x| x.source_index == Some(source)),
                    "{} native mutation {mode}", f[0]);
                let lookup = TokenMintLookup::new(&wrong, wrong.meta.as_ref().unwrap());
                let mut routed = native.clone(); routed.protocol = 5;
                routed.program_id = substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                let mut rows = vec![routed];
                crate::quality::verify_and_deduplicate(&wrong, &lookup, &mut rows, false);
                assert!(rows.is_empty(), "{} routed mutation {mode}", f[0]);
                invalid += 2;
            }
            if parsed.protocol == Protocol::MeteoraDbc {
                for event in 0..2 {
                    for mode in 0..8 {
                        let target = source + 3 + event;
                        let mut wrong = tx.clone();
                        let roots = wrong.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions.len();
                        let mut flat = 0;
                        for root_position in 0..roots {
                            flat += 1;
                            if let Some(group) = wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g| g.index == root_position as u32) {
                                for i in &mut group.instructions {
                                    if flat == target {
                                        let position = match mode { 0=>16,1=>82,2=>if event==0{106}else{123},3=>80,4=>8,5=>if event==0{154}else{99},6=>if event==0{98}else{107},_=>81 };
                                        i.data[position] ^= 1;
                                    }
                                    flat += 1;
                                }
                            }
                        }
                        assert!(!crate::process_transaction(wrong.clone()).unwrap().swaps.iter().any(|x|x.source_index==Some(source)), "native DBC event{event} mutation{mode}");
                        let lookup=TokenMintLookup::new(&wrong,wrong.meta.as_ref().unwrap());
                        let mut routed=native.clone();routed.protocol=5;routed.program_id=substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                        let mut rows=vec![routed];crate::quality::verify_and_deduplicate(&wrong,&lookup,&mut rows,false);
                        assert!(rows.is_empty(), "routed DBC event{event} mutation{mode}");invalid+=2;
                    }
                }
            }
            let lookup = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            for mode in 0..3 {
                let mut routed = native.clone(); routed.protocol = 5;
                routed.program_id = substreams_solana_idls::jupiter::v6::PROGRAM_ID.to_vec();
                if mode == 1 { routed.user = source_ix.accounts()[parsed.pool].0.clone(); }
                if mode == 2 { routed.amm_pool = source_ix.accounts()[parsed.users[0]].0.clone(); }
                let mut rows = vec![routed];
                crate::quality::verify_and_deduplicate(&tx, &lookup, &mut rows, false);
                if mode == 0 { assert_eq!(rows.len(), 1); assert_eq!(rows[0].protocol, parsed.protocol as i32); }
                else { assert!(rows.is_empty()); invalid += 1; }
            }
        }
        assert_eq!(positives, 6); assert_eq!(invalid, 288);
        eprintln!("Zero-suffix Anchor finalized positives={positives}; invalid native/routed cases={invalid}");
    }
}
