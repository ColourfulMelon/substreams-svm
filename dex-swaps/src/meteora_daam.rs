use std::collections::VecDeque;

use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::meteora::daam;

pub(crate) struct PendingSwap {
    stack_height: u32,
    user: Vec<u8>,
    pool: Vec<u8>,
    token_a_mint: Vec<u8>,
    token_b_mint: Vec<u8>,
}

struct SwapEvent {
    pool: Vec<u8>,
    amount_in: u64,
    output_amount: u64,
    trade_direction: u8,
}

/// Keep unresolved invocations and match events by their own pool address.
pub(crate) fn handle_instruction(pending: &mut VecDeque<PendingSwap>, instruction: &InstructionView) -> Option<pb::Swap> {
    let program_id = instruction.program_id().0;
    if program_id != &daam::PROGRAM_ID {
        return None;
    }

    if let Some(swap) = decode_swap_instruction(instruction) {
        pending.push_back(swap);
        return None;
    }

    let event = decode_swap_event(instruction)?;
    let position = pending.iter().position(|swap| swap.pool == event.pool)?;
    let swap = pending.remove(position)?;
    let (input_mint, output_mint) = if event.trade_direction == 0 {
        (swap.token_a_mint.clone(), swap.token_b_mint.clone())
    } else {
        (swap.token_b_mint.clone(), swap.token_a_mint.clone())
    };

    Some(pb::Swap {
        protocol: pb::Protocol::MeteoraDaam as i32,
        program_id: daam::PROGRAM_ID.to_vec(),
        stack_height: swap.stack_height,
        amm: daam::PROGRAM_ID.to_vec(),
        amm_pool: swap.pool,
        user: swap.user,
        input_mint,
        input_amount: event.amount_in,
        output_mint,
        output_amount: event.output_amount,
        ..Default::default()
    })
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_swap_instruction(ix).map(|s| s.pool)
}

fn decode_swap_instruction(ix: &InstructionView) -> Option<PendingSwap> {
    let program_id = ix.program_id().0;
    if program_id != &daam::PROGRAM_ID {
        return None;
    }

    match daam::instructions::unpack(ix.data()) {
        Ok(daam::instructions::MeteoraDammInstruction::Swap(_)) | Ok(daam::instructions::MeteoraDammInstruction::Swap2(_)) => {
            // `swap` and `swap2` share the same 14-account list per the
            // canonical cp_amm IDL; only the args struct (dynamic-fee /
            // referral params) differs. Same anchor-CPI `EvtSwap` emit.
            let accounts = daam::accounts::get_swap_accounts(ix).ok()?;
            Some(PendingSwap {
                stack_height: ix.stack_height(),
                user: accounts.payer.to_bytes().to_vec(),
                pool: accounts.pool.to_bytes().to_vec(),
                token_a_mint: accounts.token_a_mint.to_bytes().to_vec(),
                token_b_mint: accounts.token_b_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

fn decode_swap_event(ix: &InstructionView) -> Option<SwapEvent> {
    match daam::anchor_cpi_event::unpack(ix.data()) {
        Ok(daam::anchor_cpi_event::MeteoraDammAnchorCpiEvent::EvtSwap(event)) => Some(SwapEvent {
            pool: event.pool.to_bytes().to_vec(),
            amount_in: event.actual_amount_in,
            output_amount: event.swap_result.output_amount,
            trade_direction: event.trade_direction,
        }),
        // Since the cp-amm upgrade on 2025-12-26 both `swap` and `swap2`
        // emit `EvtSwap2` instead of `EvtSwap`. `included_transfer_fee_amount_in`
        // mirrors what the user paid (pre-pool fees) and is the analogue of
        // the legacy `actual_amount_in`.
        Ok(daam::anchor_cpi_event::MeteoraDammAnchorCpiEvent::EvtSwap2(event)) => Some(SwapEvent {
            pool: event.pool.to_bytes().to_vec(),
            amount_in: event.included_transfer_fee_amount_in,
            output_amount: event.swap_result.output_amount,
            trade_direction: event.trade_direction,
        }),
        _ => None,
    }
}

/// Newly observed padded payloads require the current native event to bind the
/// pool as well as the two exact principal receipts. The original canonical
/// 24-byte legacy ABI retains its existing event/fallback behavior.
pub(crate) fn observed_settlement(
    source: u32, ix: &InstructionView, children: &[InstructionView],
    successful: &std::collections::HashSet<u32>,
    input_gross: u64, input_net: Option<u64>, output_gross: u64, output_net: u64,
) -> bool {
    use substreams_solana_idls::spl::{token, token_2022};
    let d = ix.data();
    let legacy = d.starts_with(&daam::instructions::SWAP) && matches!(d.len(), 25 | 32 | 41);
    let padded = d.starts_with(&daam::instructions::SWAP2) && d.len() == 32;
    if !legacy && !padded { return true; }
    let a = ix.accounts();
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if ix.program_id().0 != &daam::PROGRAM_ID || !matches!(a.len(), 14 | 15) || depth == 0
        || !successful.contains(&source) || a[13].0 != &daam::PROGRAM_ID
        || children.iter().enumerate().any(|(n,c)| !successful.contains(&(source + n as u32 + 1)) || c.is_root() || c.stack_height() != depth + 1)
        || padded && d[24] != 0
        || input_gross == 0 || output_net == 0 || output_net > output_gross
    { return false; }
    let events: Vec<_> = children.iter().enumerate().filter(|(_,c)| c.program_id().0 == &daam::PROGRAM_ID).collect();
    if events.len() != 1 { return false; }
    let (event_position,event) = events[0];
    let e = event.data();
    if e.len() != 196 || e[..16] != [0xe4,0x45,0xa5,0x2e,0x51,0xcb,0x9a,0x1d,0xbd,0x42,0x33,0xa8,0x26,0x50,0x75,0x99]
        || event.accounts().len() != 1 || event.accounts()[0].0 != a[12].0
    { return false; }
    let p = &e[16..];
    let atoms = |n: usize| u64::from_le_bytes(p[n..n+8].try_into().unwrap());
    let requested = u64::from_le_bytes(d[8..16].try_into().unwrap());
    let minimum = u64::from_le_bytes(d[16..24].try_into().unwrap());
    if p[..32] != a[1].0[..] || p[32] > 1 || p[33] > 2 || p[34] > 1 || p[51] != 0
        || atoms(35) != requested || atoms(43) != minimum || atoms(68) != 0
        || requested != input_gross || atoms(132) != input_gross || atoms(76) != output_gross
        || atoms(140) != output_gross || atoms(148) != output_net || output_net < minimum
        || atoms(52) > input_gross || input_net.is_some_and(|net| atoms(52) != net)
    { return false; }
    let calls: Vec<_> = children.iter().enumerate().filter(|(_,c)| [token::PROGRAM_ID.as_slice(),token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())).collect();
    if calls.len() != 2 + p[34] as usize || event_position != children.len() - 1 || children.len() != calls.len()+1 { return false; }
    let forward = p[32] == 0;
    let roles = if forward { [(2,6,4,8,9),(5,7,3,0,10)] } else { [(2,7,5,8,10),(4,6,3,0,9)] };
    for (n, &(from,mint,to,authority,program)) in roles.iter().enumerate() {
        let c = calls[n].1; let ca = c.accounts(); let cd = c.data();
        if calls[n].0 != n || c.program_id().0 != a[program].0 || cd.len()!=10 || cd[0]!=12 || ca.len()!=4
            || ca[0].0 != a[from].0 || ca[1].0 != a[mint].0 || ca[2].0 != a[to].0 || ca[3].0 != a[authority].0
            || u64::from_le_bytes(cd[1..9].try_into().unwrap()) != if n==0 {input_gross} else {output_gross}
        { return false; }
    }
    if calls.len()==3 {
        let c=calls[2].1; let ca=c.accounts();let cd=c.data();
        if calls[2].0 != 2 || cd.len()!=10 || cd[0]!=12 || ca.len()!=4 || ca[2].0 != a[11].0 || ca[3].0 != a[0].0
            || u64::from_le_bytes(cd[1..9].try_into().unwrap()) != atoms(124)
        { return false; }
        // CollectFeeMode can pay the referral from either declared vault.
        let role = if ca[0].0 == a[4].0 {(6,9)} else if ca[0].0 == a[5].0 {(7,10)} else {return false};
        if ca[1].0 != a[role.0].0 || c.program_id().0 != a[role.1].0 { return false; }
    } else if atoms(124) != 0 { return false; }
    true
}

#[cfg(test)]
mod tests {
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
    #[test]
    fn observed_damm_legacy_formats_bind_pool_and_current_event_receipts() {
        let dir=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/swap-coverage/damm-legacy-suffix");
        let cases=std::fs::read_to_string(dir.join("cases.tsv")).unwrap();
        let mut shapes=std::collections::HashSet::new();
        for line in cases.lines() {
            let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(dir.join(format!("{}.transaction.pb",f[0]))).unwrap()).unwrap();
            let walk:Vec<_>=tx.walk_instructions().collect();let native=&walk[source as usize];let data=native.data();
            if !shapes.insert((data.len(),data[24..].to_vec())) {continue;}
            let event=walk.iter().enumerate().skip(source as usize+1).take_while(|(_,ix)| !ix.is_root() && ix.stack_height()>if native.is_root(){1}else{native.stack_height()})
                .find(|(_,ix)|ix.program_id().0==native.program_id().0&&ix.data().len()==196).unwrap().0 as u32;
            for mode in 0..10 {
                let mut changed=tx.clone();let groups=&mut changed.meta.as_mut().unwrap().inner_instructions;let mut position=0u32;
                for (root,ix) in changed.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions.iter_mut().enumerate() {
                    if position==source {if mode==0{ix.accounts[1]=ix.accounts[8];}else if mode==1{ix.accounts[8]=ix.accounts[1];}}
                    position+=1;
                    if let Some(group)=groups.iter_mut().find(|g|g.index==root as u32) {
                        for ix in &mut group.instructions {
                            if position==source {if mode==0{ix.accounts[1]=ix.accounts[8];}else if mode==1{ix.accounts[8]=ix.accounts[1];}}
                            if position==event {
                                match mode {2=>ix.data[16]^=1,3=>ix.data[16+35]^=1,4=>ix.data[16+43]^=1,5=>ix.data[16+140]^=1,6=>ix.data[16+148]^=1,7=>ix.accounts[0]=tx.resolved_accounts().iter().position(|a| *a == native.accounts()[1].0).unwrap() as u8,8=>ix.stack_height=Some(99),9=>{ix.data.pop();},_=>{}}
                            }
                            position+=1;
                        }
                    }
                }
                let decoded=crate::process_transaction_with_metadata(changed,None,|_,_|None,|_,_|None,|mint,_|{
                    let raw=std::fs::read(dir.join("fee-mints").join(format!("{}.bin",base58::encode(mint)))).ok()?;
                    let(schedule,initialized)=crate::token_fees::owned_mint(&substreams_solana_idls::spl::token_2022::PROGRAM_ID,&raw)?;
                    initialized.then(||schedule.at(1055)).flatten()
                });
                assert!(decoded.map_or(true,|t|t.swaps.iter().all(|s|s.source_index!=Some(source))),"{} source{source} observed DAMM mutation{mode}",f[0]);
            }
        }
        assert_eq!(shapes.len(),4);
    }
}
