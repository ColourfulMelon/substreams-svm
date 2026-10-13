use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pancakeswap;

use crate::{
    logs::{scoped_program_log, ParsedLog, ProgramLog},
    token_mints::TokenMintLookup,
};

pub(crate) struct State {
    pending: Vec<InstructionSwap>,
    next_index: usize,
    is_invoked: bool,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            pending: Vec::new(),
            next_index: 0,
            is_invoked: false,
        }
    }

    pub(crate) fn handle_instruction(&mut self, ix: &InstructionView, token_mints: &TokenMintLookup) {
        if let Some(swap) = decode_instruction(ix, token_mints) {
            self.pending.push(swap);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &ParsedLog<'_>) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) = scoped_program_log(log_message, &pancakeswap::PROGRAM_ID, &mut self.is_invoked)? else {
            return None;
        };

        let event = parse_log_data(log_message)?;
        let instruction = self.pending.get(self.next_index)?;
        self.next_index += 1;

        let (input_amount, output_amount) = if event.zero_for_one {
            (event.amount_0, event.amount_1)
        } else {
            (event.amount_1, event.amount_0)
        };

        Some(pb::Swap {
            protocol: pb::Protocol::Pancakeswap as i32,
            program_id: pancakeswap::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: pancakeswap::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool_state.clone(),
            user: instruction.user.clone(),
            input_mint: instruction.input_mint.clone(),
            input_amount,
            output_mint: instruction.output_mint.clone(),
            output_amount,
            ..Default::default()
    })
    }
}

struct InstructionSwap {
    stack_height: u32,
    user: Vec<u8>,
    pool_state: Vec<u8>,
    input_mint: Vec<u8>,
    output_mint: Vec<u8>,
}

struct SwapEvent {
    amount_0: u64,
    amount_1: u64,
    zero_for_one: bool,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_pool(ix)
}

fn decode_instruction(ix: &InstructionView, token_mints: &TokenMintLookup) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &pancakeswap::PROGRAM_ID {
        return None;
    }

    match pancakeswap::instructions::unpack(ix.data()) {
        Ok(pancakeswap::instructions::PancakeSwapInstruction::Swap(_)) => {
            let accounts = pancakeswap::accounts::get_swap_accounts(ix).ok()?;
            let input_token_account = accounts.input_token_account.to_bytes().to_vec();
            let output_token_account = accounts.output_token_account.to_bytes().to_vec();

            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                user: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_mint: token_mints.mint_for(&input_token_account)?,
                output_mint: token_mints.mint_for(&output_token_account)?,
            })
        }
        _ => None,
    }
}

fn decode_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    let program_id = ix.program_id().0;
    if program_id != &pancakeswap::PROGRAM_ID {
        return None;
    }

    match pancakeswap::instructions::unpack(ix.data()) {
        Ok(pancakeswap::instructions::PancakeSwapInstruction::Swap(_)) => {
            let accounts = pancakeswap::accounts::get_swap_accounts(ix).ok()?;
            Some(accounts.pool_state.to_bytes().to_vec())
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<SwapEvent> {
    let data = parse_program_data(log_message)?;
    match pancakeswap::events::unpack(data.as_slice()) {
        Ok(pancakeswap::events::PancakeSwapEvent::Swap(event)) => Some(SwapEvent {
            amount_0: event.amount_0,
            amount_1: event.amount_1,
            zero_for_one: event.zero_for_one,
        }),
        _ => None,
    }
}

pub(crate) const LEGACY_SWAP: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];

pub(crate) fn legacy_layout(ix: &InstructionView) -> Option<crate::native_venues::Layout> {
    let a=ix.accounts();let d=ix.data();
    if ix.program_id().0.as_slice()!=pancakeswap::PROGRAM_ID || d.len()!=41 || !d.starts_with(&LEGACY_SWAP)
        || d[40]>1 || a.len()<10 || a[8].0.as_slice()!=substreams_solana_idls::spl::token::PROGRAM_ID
        || [2,3,4,5,6].iter().enumerate().any(|(i,r)|[2,3,4,5,6][..i].iter().any(|s|a[*r].0==a[*s].0)) {return None;}
    let exact=d[40]==1;let specified=u64::from_le_bytes(d[8..16].try_into().ok()?);let threshold=u64::from_le_bytes(d[16..24].try_into().ok()?);
    if specified==0 || !exact&&threshold==0 {return None;}
    Some(crate::native_venues::Layout{protocol:pb::Protocol::Pancakeswap,pool:2,authority:0,users:[3,4],vaults:[5,6],
        amount:if exact{specified}else{threshold},minimum_output:if exact{threshold}else{specified},
        exact_input:exact,input_limit:exact,input_first:true,fixed_users:true})
}

pub(crate) fn legacy_settlement(source:u32,tx:&substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction,ix:&InstructionView,calls:&[InstructionView],mints:&TokenMintLookup,successful:&std::collections::HashSet<u32>)->Option<(u64,u64)> {
    use substreams_solana_idls::spl::{token,token_2022};
    use base64::Engine;
    let l=legacy_layout(ix)?;let a=ix.accounts();let depth=if ix.is_root(){1}else{ix.stack_height()};
    if calls.len()!=2 || !successful.contains(&source) {return None;}
    let input_mint=mints.mint_for(a[3].0)?;let output_mint=mints.mint_for(a[4].0)?;
    if input_mint==output_mint || mints.mint_for(a[5].0).as_deref()!=Some(input_mint.as_slice()) || mints.mint_for(a[6].0).as_deref()!=Some(output_mint.as_slice()) {return None;}
    let xy=input_mint<output_mint;let (mint0,mint1)=if xy{(&input_mint,&output_mint)}else{(&output_mint,&input_mint)};
    let program=solana_pubkey::Pubkey::new_from_array(pancakeswap::PROGRAM_ID);
    let pool=solana_pubkey::Pubkey::find_program_address(&[b"pool",a[1].0,mint0,mint1],&program).0;
    if a[2].0.as_slice()!=pool.as_ref() {return None;}
    for (role,mint) in [(5,&input_mint),(6,&output_mint)] {
        if a[role].0.as_slice()!=solana_pubkey::Pubkey::find_program_address(&[b"pool_vault",a[2].0,mint],&program).0.as_ref() {return None;}
    }
    if a[7].0.as_slice()!=solana_pubkey::Pubkey::find_program_address(&[b"observation",a[2].0],&program).0.as_ref() {return None;}
    let mut amounts=[0u64;2];
    for (n,c) in calls.iter().enumerate() {
        let roles=if n==0{[3,5,0]}else{[6,4,2]};let d=c.data();let ca=c.accounts();
        if !successful.contains(&(source+1+n as u32)) || c.stack_height()!=depth+1 || c.program_id().0.as_slice()!=token::PROGRAM_ID
            || d.len()!=9 || d[0]!=3 || ca.len()!=3 || ca.iter().zip(roles).any(|(key,r)|key.0!=a[r].0) {return None;}
        amounts[n]=u64::from_le_bytes(d[1..9].try_into().ok()?);
    }
    let specified=u64::from_le_bytes(ix.data()[8..16].try_into().ok()?);
    if amounts[0]==0 || amounts[1]==0 || amounts[0]>l.amount || amounts[1]<l.minimum_output
        || !l.exact_input&&amounts[1]!=specified || l.exact_input&&ix.data()[24..40].iter().all(|b|*b==0)&&amounts[0]!=specified {return None;}
    let meta=tx.meta.as_ref()?;let keys=tx.resolved_accounts();
    for (role,mint) in [(5,&input_mint),(6,&output_mint)] {
        let address=a[role].0;let position=keys.iter().position(|key|*key==address)?;
        let before=meta.pre_token_balances.iter().find(|b|b.account_index as usize==position)?;
        let after=meta.post_token_balances.iter().find(|b|b.account_index as usize==position)?;
        if before.mint!=substreams_solana::base58::encode(mint) || after.mint!=before.mint || before.owner!=substreams_solana::base58::encode(a[2].0) || after.owner!=before.owner
            || before.program_id!=substreams_solana::base58::encode(token::PROGRAM_ID) || after.program_id!=before.program_id {return None;}
        let mut change=0i128;
        for (index,c) in tx.walk_instructions().enumerate() {
            if ![token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
                || !c.accounts().iter().any(|key|key.0==address) || !successful.contains(&(index as u32)) {continue;}
            if c.program_id().0.as_slice()!=token::PROGRAM_ID {return None;}
            let d=c.data();let ca=c.accounts();let checked=match(d.first(),d.len()){(Some(3),9)=>false,(Some(12),10)=>true,_=>return None};
            if ca.len()!=if checked{4}else{3} {return None;}
            let from=ca[0].0;let to=ca[if checked{2}else{1}].0;
            if from!=address&&to!=address || mints.mint_for(from).as_deref()!=Some(mint.as_slice()) || mints.mint_for(to).as_deref()!=Some(mint.as_slice()) || checked&&ca[1].0.as_slice()!=mint.as_slice() {return None;}
            let amount=i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?));if from==address{change-=amount;}if to==address{change+=amount;}
        }
        if i128::from(after.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?)-i128::from(before.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?)!=change {return None;}
    }
    let walk:Vec<_>=tx.walk_instructions().collect();let mut cursor=0;let mut stack=Vec::new();let mut event=None;
    for log in &meta.log_messages {
        let parts:Vec<_>=log.split_whitespace().collect();
        if parts.len()==4&&parts[0]=="Program"&&parts[2]=="invoke" {
            let program=substreams_solana::base58::decode(parts[1]).ok()?;let height=parts[3].strip_prefix('[')?.strip_suffix(']')?.parse::<u32>().ok()?;
            let index=walk.iter().enumerate().skip(cursor).find(|(_,x)|x.program_id().0==&program&&(if x.is_root(){1}else{x.stack_height()})==height)?.0;
            if height as usize!=stack.len()+1{return None;}cursor=index+1;stack.push(index as u32);
        }else if parts.len()>=3&&parts[0]=="Program"&&matches!(parts[2],"success"|"failed:") {
            let frame=stack.pop()?;if substreams_solana::base58::encode(walk[frame as usize].program_id().0)!=parts[1]{return None;}
        }else if stack.last()==Some(&source)&&log.starts_with("Program data: ") {
            let b=base64::engine::general_purpose::STANDARD.decode(&log[14..]).ok()?;
            if !b.starts_with(&[64,198,205,232,38,8,113,226]){continue;}if b.len()!=205||event.is_some(){return None;}event=Some(b);
        }
    }
    if !stack.is_empty(){return None;}let e=event?;
    if e[8..40]!=*a[2].0 || e[40..72]!=*a[0].0 || e[72..104]!=*a[if xy{3}else{4}].0 || e[104..136]!=*a[if xy{4}else{3}].0 || e[168]!=u8::from(xy)
        || u64::from_le_bytes(e[136..144].try_into().ok()?)!=amounts[if xy{0}else{1}] || u64::from_le_bytes(e[152..160].try_into().ok()?)!=amounts[if xy{1}else{0}]
        || e[144..152].iter().chain(&e[160..168]).any(|b|*b!=0) {return None;}
    Some((amounts[0],amounts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use substreams_solana::{base58,pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn finalized_legacy_pancake_swaps_bind_pdas_events_and_full_vault_ledgers() {
        let mut originals=0;
        for line in include_str!("../fixtures/swap-coverage/pancake-legacy27/cases.tsv").lines() {
            let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
            let original=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(format!("{}/fixtures/swap-coverage/pancake-legacy27/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap()).unwrap();originals+=1;
            for derived_out in [false,true] {
                let mut tx=original.clone();let original_data=tx.walk_instructions().nth(source as usize).unwrap().data().clone();
                if derived_out {
                    // ABI-derived exact-output shape, not another finalized original.
                    let n=tx.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|n|n.data==original_data).unwrap();
                    n.data[8..16].copy_from_slice(&f[7].parse::<u64>().unwrap().to_le_bytes());n.data[16..24].copy_from_slice(&f[5].parse::<u64>().unwrap().to_le_bytes());n.data[40]=0;
                }
                let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)&&s.amm==pancakeswap::PROGRAM_ID).collect();
                assert_eq!(rows.len(),1,"{} derivedOut={derived_out}",f[0]);let row=rows[0];
                assert!(row.transfer_verified);assert_eq!(row.protocol,pb::Protocol::Pancakeswap as i32);assert_eq!(row.source_transfer_index,Some(source+1));
                assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),
                    (f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
                let verify_route=|transaction:&ConfirmedTransaction| {
                    let mut candidate=row.clone();candidate.protocol=pb::Protocol::JupiterV4 as i32;let mut candidates=vec![candidate];
                    let mints=TokenMintLookup::new(transaction,transaction.meta.as_ref().unwrap());
                    crate::quality::verify_with_fees(transaction,&mints,&mut candidates,false,|_,_|None);candidates
                };
                assert_eq!(verify_route(&tx).len(),1);
                let native=tx.walk_instructions().nth(source as usize).unwrap().data().clone();
                for mode in 0..37 {
                    let mut wrong=tx.clone();
                    if mode<24 || mode==36 {
                        let g=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g|g.instructions.iter().any(|n|n.data==native)).unwrap();
                        let p=g.instructions.iter().position(|n|n.data==native).unwrap();let a=g.instructions[p].accounts.clone();let n=&mut g.instructions;
                        match mode {
                            0=>n[p].data[0]^=1,1=>n[p].data.push(0),
                            2=>n[p].data[8..16].copy_from_slice(&(if derived_out{row.output_amount+1}else{row.input_amount-1}).to_le_bytes()),
                            3=>n[p].data[16..24].copy_from_slice(&(if derived_out{row.input_amount-1}else{row.output_amount+1}).to_le_bytes()),
                            4=>n[p].data[40]=2,5=>n[p].accounts[2]=a[0],6=>n[p].accounts[1]=a[0],7=>n[p].accounts[3]=a[4],
                            8=>n[p].accounts[5]=a[6],9=>n[p].accounts[6]=a[5],10=>n[p].accounts[7]=a[1],11=>n[p].accounts[8]=a[0],12=>n[p].accounts[0]=a[2],
                            13=>n[p+1].data[1..9].copy_from_slice(&(row.input_amount+1).to_le_bytes()),14=>n[p+1].data[1..9].fill(0),15=>n[p+1].accounts[0]=a[5],16=>n[p+1].accounts[1]=a[6],17=>n[p+1].accounts[2]=a[2],
                            18=>n[p+2].data[1..9].copy_from_slice(&(row.output_amount+1).to_le_bytes()),19=>n[p+2].data[1..9].fill(0),20=>n[p+2].accounts[0]=a[5],21=>n[p+2].accounts[1]=a[3],22=>n[p+2].accounts[2]=a[0],23=>n[p+2].stack_height=Some(99),
                            _=>n[p+2].program_id_index=u32::from(a[0]),
                        }
                    } else if mode<34 {
                        let log=wrong.meta.as_mut().unwrap().log_messages.iter_mut().find(|l|l.starts_with("Program data: ")&&base64::engine::general_purpose::STANDARD.decode(&l[14..]).is_ok_and(|e|e.len()==205&&e.starts_with(&[64,198,205,232,38,8,113,226])&&e[8..40]==row.amm_pool)).unwrap();
                        let mut e=base64::engine::general_purpose::STANDARD.decode(&log[14..]).unwrap();
                        if mode==33{e.push(0);}else{e[match mode{24=>8,25=>40,26=>72,27=>104,28=>136,29=>152,30=>144,31=>160,_=>168}]^=1;}
                        *log=format!("Program data: {}",base64::engine::general_purpose::STANDARD.encode(e));
                    } else if mode==34 {
                        let native_ix=tx.walk_instructions().nth(source as usize).unwrap();let a=native_ix.accounts();let k=tx.resolved_accounts();let position=k.iter().position(|key|*key==a[6].0).unwrap();
                        let b=wrong.meta.as_mut().unwrap().post_token_balances.iter_mut().find(|b|b.account_index as usize==position).unwrap().ui_token_amount.as_mut().unwrap();b.amount=(b.amount.parse::<u64>().unwrap()+1).to_string();
                    } else {
                        wrong.meta.as_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(pancakeswap::PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: mutation",base58::encode(pancakeswap::PROGRAM_ID)));
                    }
                    assert!(crate::process_transaction(wrong.clone()).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source)||s.amm!=pancakeswap::PROGRAM_ID)),"native Pancake mode{mode} derivedOut={derived_out}");
                    assert!(verify_route(&wrong).is_empty(),"routed Pancake mode{mode} derivedOut={derived_out}");
                }
            }
        }
        assert_eq!(originals,3);
    }
}
