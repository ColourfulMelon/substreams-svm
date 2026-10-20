use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpfun::bonding_curve as pumpfun;

use crate::SOL_MINT;

pub(crate) struct PendingTrade {
    bonding_curve: Vec<u8>,
    mint: Vec<u8>,
    quote_mint: Vec<u8>,
    user: Vec<u8>,
    is_buy: bool,
    source_index: u32,
    depth: u32,
    event_authority: Vec<u8>,
}

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView, source_index: u32) -> Option<pb::Swap> {
    // A sibling invocation cannot supply the event for an earlier native call.
    if pending_trade.as_ref().map_or(false, |trade| instruction.stack_height() <= trade.depth) {
        *pending_trade = None;
    }
    let program_id = instruction.program_id().0;
    if program_id != &pumpfun::PROGRAM_ID {
        return None;
    }

    if let Some(mut trade) = decode_trade_instruction(instruction) {
        trade.source_index = source_index;
        trade.depth = if instruction.is_root() { 1 } else { instruction.stack_height() };
        *pending_trade = Some(trade);
        return None;
    }

    // Permissive trade-event decoder lives in
    // `substreams_solana_idls::pumpfun::bonding_curve` — the on-chain
    // TradeEvent has grown past the IDL's V0..V3 fixed lengths, so the strict
    // decoder rejects every current event. The minimal helper reads the
    // stable leading layout at fixed offsets and ignores trailing fields.
    let event = pumpfun::events::unpack_trade_event_minimal(instruction.data()).ok()??;
    let trade = pending_trade.take()?;

    if instruction.stack_height() != trade.depth + 1
        || instruction.accounts().len() != 1
        || instruction.accounts()[0].0.as_slice() != trade.event_authority.as_slice()
    { return None; }
    if event.mint.as_slice() != trade.mint || event.user.as_slice() != trade.user || event.is_buy != trade.is_buy {
        return None;
    }
    let quote_amount = if trade.quote_mint == SOL_MINT {
        event.sol_amount
    } else {
        let amounts = pumpfun::events::unpack_quote_trade_amount(instruction.data()).ok()?;
        if amounts.mint.as_slice() != trade.quote_mint {
            return None;
        }
        amounts.amount
    };
    let mint = event.mint.to_vec();
    Some(pb::Swap {
        protocol: pb::Protocol::Pumpfun.into(),
        program_id: pumpfun::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        source_index: Some(trade.source_index),
        amm: pumpfun::PROGRAM_ID.to_vec(),
        amm_pool: trade.bonding_curve,
        user: event.user.to_vec(),
        input_mint: if event.is_buy { trade.quote_mint.clone() } else { mint.clone() },
        input_amount: if event.is_buy { quote_amount } else { event.token_amount },
        output_mint: if event.is_buy { mint } else { trade.quote_mint },
        output_amount: if event.is_buy { event.token_amount } else { quote_amount },
        ..Default::default()
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpfun::PROGRAM_ID {
        return None;
    }
    decode_trade_instruction(instruction).map(|t| t.bonding_curve)
}

// A finalized SellV2 caller adds one ignored bool to the primary two-u64
// prefix. Accept only that retained bounded profile, shared with receipt
// verification; all other unsupported suffixes retain the IDL rejection.
pub(crate) fn unpack_trade_instruction(data: &[u8]) -> Result<pumpfun::instructions::PumpFunInstruction, substreams_solana_idls::common::ParseError> {
    let normalized = if data.starts_with(&pumpfun::instructions::SELL_V2) && data.len() == 25 && data[24] <= 1 {
        &data[..24]
    } else { data };
    pumpfun::instructions::unpack(normalized)
}

// Exact quote input is a budget: fee and reserve rounding can leave atoms
// unspent, and partial fills can spend less. Bind the declared bound to the
// committed native event and physical whole-user quote debit, not equality.
pub(crate) fn exact_quote_budget_satisfies(
    instruction: &InstructionView, input: u64, output: u64, calls: &[InstructionView],
) -> bool {
    use pumpfun::instructions::PumpFunInstruction::*;
    let (budget, minimum, user, authority) = match unpack_trade_instruction(instruction.data()) {
        Ok(BuyExactQuoteInV2(a)) => (a.spendable_quote_in,a.min_tokens_out,13,25),
        Ok(BuyExactQuoteInV3(a)) => (a.spendable_quote_in,a.min_tokens_out,8,15),
        _ => return false,
    };
    let a=instruction.accounts();
    if a.len() <= authority || input==0 || input>budget || output<minimum { return false; }
    let depth=if instruction.is_root() {1} else {instruction.stack_height()};
    let mut events=calls.iter().filter(|ix| ix.program_id().0 == &pumpfun::PROGRAM_ID
        && ix.stack_height()==depth+1 && ix.data().starts_with(&[228,69,165,46,81,203,154,29,189,219,127,211,78,230,97,238]));
    let Some(event)=events.next() else { return false; };
    let e=event.accounts();let d=event.data();
    if events.next().is_some() || e.len()!=1 || e[0].0!=a[authority].0 { return false; }
    let Ok(Some(trade))=pumpfun::events::unpack_trade_event_minimal(d) else { return false; };
    let Ok(quote)=pumpfun::events::unpack_quote_trade_amount(d) else { return false; };
    if !trade.is_buy || trade.user.as_slice()!=a[user].0.as_slice() || trade.mint.as_slice()!=a[1].0.as_slice()
        || quote.mint.as_slice()!=a[2].0.as_slice() || trade.token_amount!=output
        || quote.amount.checked_add(quote.fee).and_then(|n|n.checked_add(quote.creator_fee))!=Some(input)
    { return false; }
    // Primary TradeEvent ABI: fee rates are fixed prefix fields; ix_name is
    // the bounded string after the volume prefix. Both are native source data.
    let Some(raw)=d.get(266..270) else { return false; };
    let length=u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    if length!=18 || d.get(270..288)!=Some(b"buy_exact_quote_in".as_slice()) { return false; }
    let rate=|offset|d.get(offset..offset+8).map(|b|u64::from_le_bytes(b.try_into().unwrap()));
    let (Some(protocol),Some(creator))=(rate(169),rate(217)) else { return false; };
    let ceil=|net:u128,bps:u64|(net*u128::from(bps)+9999)/10000;
    let amount=u128::from(quote.amount);
    // The primary SDK retains fees computed on the initial budget-derived
    // net when a ceiling-fee deficit then reduces the curve quote by atoms.
    // Do not recompute those retained fees on the smaller adjusted quote.
    let initial=u128::from(budget)*10000/(10000+u128::from(protocol)+u128::from(creator));
    let initial_protocol=ceil(initial,protocol);let initial_creator=ceil(initial,creator);
    let deficit=(initial+initial_protocol+initial_creator).saturating_sub(u128::from(budget));
    let budget_profile=initial.checked_sub(deficit)==Some(amount)
        && initial_protocol==u128::from(quote.fee) && initial_creator==u128::from(quote.creator_fee);
    let actual_net_profile=ceil(amount,protocol)==u128::from(quote.fee)
        && ceil(amount,creator)==u128::from(quote.creator_fee);
    budget_profile || actual_net_profile
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    use pumpfun::instructions::PumpFunInstruction::*;
    let (pool, mint, quote, user, is_buy, event_authority) = match unpack_trade_instruction(instruction.data()).ok()? {
        Buy(_) | BuyExactSolIn(_) => (3, 2, None, 6, true, 10),
        Sell(_) => (3, 2, None, 6, false, 10),
        BuyV2(_) | BuyExactQuoteInV2(_) => (10, 1, Some(2), 13, true, 25),
        SellV2(_) => (10, 1, Some(2), 13, false, 24),
        BuyV3(_) | BuyExactQuoteInV3(_) => (5, 1, Some(2), 8, true, 15),
        SellV3(_) => (5, 1, Some(2), 8, false, 15),
        _ => return None,
    };
    let accounts = instruction.accounts();
    Some(PendingTrade {
        bonding_curve: accounts.get(pool)?.0.clone(),
        mint: accounts.get(mint)?.0.clone(),
        quote_mint: match quote {
            Some(index) if accounts.get(index)?.0.as_slice() != [0; 32] => accounts.get(index)?.0.clone(),
            _ => SOL_MINT.to_vec(),
        },
        user: accounts.get(user)?.0.clone(),
        is_buy,
        source_index: 0,
        depth: 0,
        event_authority: accounts.get(event_authority)?.0.clone(),
    })
}

// Two committed V3 calls can share a curve. The earlier buy's canonical
// post-reserves independently seed the sell, so neither an aggregate wallet
// credit nor an ambiguous inverse reserve interval allocates its proceeds.
pub(crate) fn round_trip_native_sell(
    tx: &substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction,
    instructions: &[InstructionView], source: usize,
    successful: &std::collections::HashSet<u32>,
) -> Option<(u64, u32)> {
    use std::collections::HashMap;
    use substreams_solana_idls::spl::{token, token_2022};
    use pumpfun::instructions::PumpFunInstruction::*;
    let native=instructions.get(source)?;let a=native.accounts();
    let SellV3(sell)=unpack_trade_instruction(native.data()).ok()? else {return None};
    if sell.amount==0 || !native.is_root() || a.len()!=17 || a[2].0.as_slice()!=SOL_MINT
        || a[3].0.as_slice()!=token_2022::PROGRAM_ID || a[4].0.as_slice()!=token::PROGRAM_ID
        || a[14].0.iter().any(|v|*v!=0) || a[16].0.as_slice()!=pumpfun::PROGRAM_ID
        || [5,8,9,6,11,13].iter().enumerate().any(|(p,r)|[5,8,9,6,11,13][p+1..].iter().any(|q|a[*r].0==a[*q].0)) {return None;}
    let trades:Vec<_>=instructions.iter().enumerate().filter(|(_,ix)|ix.program_id().0.as_slice()==pumpfun::PROGRAM_ID
        && ix.accounts().iter().any(|key|key.0==a[5].0)).collect();
    if trades.len()!=2 || trades[1].0!=source {return None;}
    let (buy_index,buy_ix)=trades[0];let b=buy_ix.accounts();
    let BuyV3(buy)=unpack_trade_instruction(buy_ix.data()).ok()? else {return None};
    if !buy_ix.is_root() || b.len()!=17 || b.iter().zip(&a).any(|(x,y)|x.0!=y.0) || buy.amount!=sell.amount {return None;}
    let meta=tx.meta.as_option()?;let keys=tx.resolved_accounts();
    let value=|key:&Vec<u8>,side:&[u64]|keys.iter().position(|k|*k==key).and_then(|i|side.get(i)).copied();
    let mut native_changes=HashMap::<Vec<u8>,i128>::new();
    let mut token_changes=HashMap::<Vec<u8>,i128>::new();
    let mut add=|from:&Vec<u8>,to:&Vec<u8>,amount:u64| {
        *native_changes.entry(from.clone()).or_default()-=i128::from(amount);
        *native_changes.entry(to.clone()).or_default()+=i128::from(amount);
    };
    let mut claims=Vec::new();
    for (position,ix) in instructions.iter().enumerate() {
        if !successful.contains(&(position as u32)) {return None;}
        let d=ix.data();let c=ix.accounts();let program=ix.program_id().0;
        if program.iter().all(|v|*v==0) {
            if c.len()!=2 || !matches!(d.len(),12|52) || d[..4]!=[2,0,0,0] && d[..4]!=[0;4] {return None;}
            add(c[0].0,c[1].0,u64::from_le_bytes(d[4..12].try_into().ok()?));
        } else if program.as_slice()==token_2022::PROGRAM_ID {
            match d.first().copied()? {
                12 if d.len()==10 && c.len()==4 && c[1].0==a[1].0 && d[9]==6 => {
                    let amount=u64::from_le_bytes(d[1..9].try_into().ok()?);
                    *token_changes.entry(c[0].0.clone()).or_default()-=i128::from(amount);
                    *token_changes.entry(c[2].0.clone()).or_default()+=i128::from(amount);
                },
                18 if d.len()==33 && c.len()==2 && c[0].0==a[9].0 && c[1].0==a[1].0 && d[1..]==a[8].0[..] => {},
                21 if d.as_slice()==[21,7,0] && c.len()==1 && c[0].0==a[1].0 => {},
                22 if d.as_slice()==[22] && c.len()==1 && c[0].0==a[9].0 => {},
                _=>return None,
            }
        } else if program.as_slice()==pumpfun::PROGRAM_ID && ix.is_root() {
            if position==buy_index || position==source {
                let end=instructions.iter().enumerate().skip(position+1).find(|(_,ix)|ix.is_root()).map(|(i,_)|i).unwrap_or(instructions.len());
                let children=instructions.get(position+1..end)?;let count=children.len();let is_buy=position==buy_index;
                if (if is_buy {!matches!(count,3|4)}else{count!=2}) || children.iter().any(|x|x.is_root() || x.stack_height()!=2) {return None;}
                let offset=usize::from(is_buy && count==4);
                if offset==1 {
                    let create=&children[0];let ca=create.accounts();let cd=create.data();
                    let expected=solana_pubkey::Pubkey::find_program_address(&[b"user_volume_accumulator",a[8].0],&solana_pubkey::Pubkey::new_from_array(pumpfun::PROGRAM_ID)).0.to_bytes();
                    if create.program_id().0.iter().any(|v|*v!=0) || ca.len()!=2 || ca[0].0!=a[8].0 || ca[1].0!=a[11].0
                        || a[11].0.as_slice()!=expected || cd.len()!=52 || cd[..4]!=[0;4] || cd[20..52]!=pumpfun::PROGRAM_ID
                        || value(a[11].0,&meta.pre_balances)?!=0 {return None;}
                }
                let t=&children[offset];let ta=t.accounts();
                if t.program_id().0!=a[3].0 || t.data().len()!=10 || t.data()[0]!=12 || ta.len()!=4
                    || ta[0].0!=a[if is_buy {6}else{9}].0 || ta[1].0!=a[1].0
                    || ta[2].0!=a[if is_buy {9}else{6}].0 || ta[3].0!=a[if is_buy {5}else{8}].0
                    || u64::from_le_bytes(t.data()[1..9].try_into().ok()?)!=sell.amount {return None;}
                let event=&children[count-1];let ea=event.accounts();let ed=event.data();
                if event.program_id().0.as_slice()!=pumpfun::PROGRAM_ID || ea.len()!=1 || ea[0].0!=a[15].0 {return None;}
                let trade=pumpfun::events::unpack_trade_event_minimal(ed).ok()??;
                let quote=pumpfun::events::unpack_quote_trade_amount(ed).ok()?;
                if trade.is_buy!=is_buy || trade.mint.as_slice()!=a[1].0.as_slice() || trade.user.as_slice()!=a[8].0.as_slice()
                    || trade.token_amount!=sell.amount || quote.mint.iter().any(|v|*v!=0) || quote.amount!=trade.sol_amount {return None;}
                let read=|o|ed.get(o..o+8).and_then(|v|v.try_into().ok()).map(u64::from_le_bytes);
                let protocol=read(169)?;let creator=read(217)?;
                if protocol.checked_add(creator)?>10_000 || (u128::from(quote.amount)*u128::from(protocol)+9999)/10000!=u128::from(quote.fee)
                    || (u128::from(quote.amount)*u128::from(creator)+9999)/10000!=u128::from(quote.creator_fee) {return None;}
                let name_len=u32::from_le_bytes(ed.get(266..270)?.try_into().ok()?) as usize;
                let name_end=270usize.checked_add(name_len)?;
                if ed.get(270..name_end)?!=if is_buy {b"buy".as_slice()}else{b"sell".as_slice()}
                    || *ed.get(name_end)?>1 || read(name_end+1)?!=0 || read(name_end+9)?!=0
                    || read(name_end+17)?>10_000 || read(name_end+25)?!=quote.buyback_fee
                    || u128::from(quote.fee)*u128::from(read(name_end+17)?)/10000!=u128::from(quote.buyback_fee) {return None;}
                if is_buy {
                    let system=&children[offset+1];let sa=system.accounts();let amount=quote.amount.checked_add(quote.fee)?.checked_add(quote.creator_fee)?;
                    if amount>buy.max_sol_cost || system.program_id().0.iter().any(|v|*v!=0) || system.data().len()!=12 || system.data()[..4]!=[2,0,0,0]
                        || sa.len()!=2 || sa[0].0!=a[8].0 || sa[1].0!=a[5].0 || system.data()[4..12]!=amount.to_le_bytes() {return None;}
                    add(a[5].0,a[13].0,quote.buyback_fee);
                } else {
                    let net=quote.amount.checked_sub(quote.fee)?.checked_sub(quote.creator_fee)?;
                    if net==0 || net<sell.min_sol_output {return None;}
                    add(a[5].0,a[8].0,net);add(a[5].0,a[13].0,quote.buyback_fee);
                }
                claims.push((read(105)?,read(113)?,quote.amount,quote.fee,quote.creator_fee));
            } else if d.as_slice()==[249,69,164,218,150,103,84,138] {
                if c.len()!=4 || c[0].0!=a[8].0 || c[1].0!=a[11].0 || c[2].0!=a[15].0 || c[3].0.as_slice()!=pumpfun::PROGRAM_ID {return None;}
                let expected=solana_pubkey::Pubkey::find_program_address(&[b"user_volume_accumulator",a[8].0],&solana_pubkey::Pubkey::new_from_array(pumpfun::PROGRAM_ID)).0.to_bytes();
                let child=instructions.get(position+1)?;let cd=child.data();let ca=child.accounts();
                if c[1].0.as_slice()!=expected || child.program_id().0.as_slice()!=pumpfun::PROGRAM_ID || child.stack_height()!=2 || ca.len()!=1 || ca[0].0!=a[15].0
                    || cd.len()!=88 || cd[..16]!=[228,69,165,46,81,203,154,29,146,159,189,172,146,88,56,244] || cd[16..48]!=a[8].0[..]
                    || instructions.get(position+2).is_some_and(|x|!x.is_root()) || value(c[1].0,&meta.post_balances)?!=0 {return None;}
                let mut refund=value(c[1].0,&meta.pre_balances)?;
                for funding in instructions.iter().take(position) {
                    let f=funding.accounts();let data=funding.data();
                    if funding.program_id().0.iter().all(|v|*v==0) && f.len()==2 && f[1].0==c[1].0 {
                        if data.len()!=52 || data[..4]!=[0;4] || data[20..52]!=pumpfun::PROGRAM_ID || f[0].0!=a[8].0 {return None;}
                        refund=refund.checked_add(u64::from_le_bytes(data[4..12].try_into().ok()?))?;
                    }
                }
                add(c[1].0,c[0].0,refund);
            } else {return None;}
        } else if program.as_slice()==pumpfun::PROGRAM_ID && !ix.is_root() {
            if c.len()!=1 || c[0].0!=a[15].0 {return None;}
        } else if program.as_slice()==substreams_solana_macro::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL") {
            if !ix.is_root() || d.as_slice()!=[1] || c.len()!=6 || c[0].0!=a[8].0 || c[1].0!=a[9].0 || c[2].0!=a[8].0 || c[3].0!=a[1].0
                || c[4].0.iter().any(|v|*v!=0) || c[5].0!=a[3].0 {return None;}
        } else if program.as_slice()!=substreams_solana_macro::b58!("ComputeBudget111111111111111111111111111111") {return None;}
    }
    if claims.len()!=2 {return None;}
    let before=claims[0];let after=claims[1];
    if after.1==0 || before.1.checked_add(sell.amount)?!=after.1 || before.0.checked_sub(after.0)?!=after.2
        || u128::from(sell.amount)*u128::from(before.0)/u128::from(after.1)!=u128::from(after.2) {return None;}
    *native_changes.entry(keys.first()?.to_vec()).or_default()-=i128::from(meta.fee);
    if keys.iter().enumerate().any(|(i,key)|meta.post_balances.get(i).zip(meta.pre_balances.get(i)).map(|(after,before)|i128::from(*after)-i128::from(*before))!=Some(native_changes.get(*key).copied().unwrap_or(0))) {return None;}
    for key in [a[6].0,a[9].0] {
        let index=keys.iter().position(|x|*x==key)? as u32;
        let balance=|rows:&[substreams_solana::pb::sf::solana::r#type::v1::TokenBalance]|rows.iter().find(|x|x.account_index==index).map(|x|x.ui_token_amount.as_option()?.amount.parse::<u64>().ok()).unwrap_or(Some(0));
        if i128::from(balance(&meta.post_token_balances)?)-i128::from(balance(&meta.pre_token_balances)?)!=token_changes.get(key).copied().unwrap_or(0) {return None;}
    }
    Some((after.2.checked_sub(after.3)?.checked_sub(after.4)?,(source+1)as u32))
}

pub(crate) struct TitanNativeSettlement {
    pub parent: usize,
    pub close: usize,
    pub atlas: Vec<u8>,
    pub funder: Vec<u8>,
    pub quote: Vec<u8>,
    pub refund: u64,
}

// The pinned Titanium Pump adapter debits its canonical Atlas gain, credits the
// funding account, then wraps that identical gain. Only the retained single-leg
// V3 profile is admitted; opaque route tails never authorize guessed movements.
pub(crate) fn titan_native_settlement(
    tx: &substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction,
    instructions: &[InstructionView], source: usize, net: u64,
    successful: &std::collections::HashSet<u32>,
) -> Option<TitanNativeSettlement> {
    use substreams_solana_idls::spl::{token, token_2022};
    const TITAN: [u8;32]=substreams_solana_macro::b58!("T1TANpTeScyeqVzzgNViGDNrkQ6qHz9KrSBS4aNXvGT");
    let parent=instructions.iter().enumerate().take(source).filter(|(_,ix)|ix.is_root()).last()?.0;
    let root=instructions.get(parent)?; let r=root.accounts(); let d=root.data();
    let native=instructions.get(source)?;let a=native.accounts();
    if !root.is_root() || root.program_id().0.as_slice()!=TITAN || r.len()!=44 || d.len()!=37
        || d[..2]!=[42,2] || !(d[18..]==[50,0,2,164,31,0,1,0,0,0,37,0,0,1,0,202,154,59,27]
            || d[18..]==[50,0,2,64,31,0,1,0,0,0,37,0,0,1,0,202,154,59,27])
        || net==0 || native.is_root() || native.stack_height()!=2 || a.len()!=26
        || native.program_id().0.as_slice()!=pumpfun::PROGRAM_ID || a[2].0.as_slice()!=SOL_MINT
        || !matches!(unpack_trade_instruction(native.data()),Ok(pumpfun::instructions::PumpFunInstruction::SellV2(_)))
    {return None;}
    let atlas=solana_pubkey::Pubkey::find_program_address(&[b"atlas"],&solana_pubkey::Pubkey::new_from_array(TITAN)).0.to_bytes();
    if a[13].0.as_slice()!=atlas || r[2].0.as_slice()!=atlas || r[0].0!=r[1].0 || r[0].0==a[13].0
        || r[12].0!=a[14].0 || r[13].0!=a[15].0 || r[14].0!=a[1].0 || r[15].0.as_slice()!=SOL_MINT
        || r[5].0.as_slice()!=token::PROGRAM_ID || r[6].0.as_slice()!=token_2022::PROGRAM_ID
        || r[7].0.iter().any(|v|*v!=0) || r[8].0.as_slice()!=substreams_solana_macro::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
        || r[9].0.as_slice()!=TITAN {return None;}
    let prefix=instructions.get(parent+1..source)?;
    if !matches!(prefix.len(),1|11) {return None;}
    if prefix.len()==11 {
        for (offset,target,owner) in [(1,12,10),(6,17,16)] {
            let calls=&prefix[offset..offset+5];let ata=&calls[0];let aa=ata.accounts();
            let size=&calls[1];let create=&calls[2];let ca=create.accounts();let immutable=&calls[3];let initialize=&calls[4];let ia=initialize.accounts();
            let expected=solana_pubkey::Pubkey::find_program_address(&[a[owner].0,&token::PROGRAM_ID,&SOL_MINT],
                &solana_pubkey::Pubkey::new_from_array(substreams_solana_macro::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"))).0.to_bytes();
            if ata.program_id().0!=r[8].0 || ata.data().as_slice()!=[1] || aa.len()!=6 || a[target].0.as_slice()!=expected
                || aa[0].0!=r[0].0 || aa[1].0!=a[target].0 || aa[2].0!=a[owner].0 || aa[3].0.as_slice()!=SOL_MINT
                || aa[4].0.iter().any(|v|*v!=0) || aa[5].0.as_slice()!=token::PROGRAM_ID
                || size.program_id().0.as_slice()!=token::PROGRAM_ID || size.data().as_slice()!=[21,7,0] || size.accounts().len()!=1 || size.accounts()[0].0.as_slice()!=SOL_MINT
                || create.program_id().0.iter().any(|v|*v!=0) || create.data().len()!=52 || create.data()[..4]!=[0;4] || create.data()[12..20]!=165u64.to_le_bytes()
                || create.data()[20..52]!=token::PROGRAM_ID || ca.len()!=2 || ca[0].0!=r[0].0 || ca[1].0!=a[target].0
                || immutable.program_id().0.as_slice()!=token::PROGRAM_ID || immutable.data().as_slice()!=[22] || immutable.accounts().len()!=1 || immutable.accounts()[0].0!=a[target].0
                || initialize.program_id().0.as_slice()!=token::PROGRAM_ID || initialize.data().len()!=33 || initialize.data()[0]!=18 || initialize.data()[1..33]!=a[owner].0[..]
                || ia.len()!=2 || ia[0].0!=a[target].0 || ia[1].0.as_slice()!=SOL_MINT {return None;}
            let i=tx.resolved_accounts().iter().position(|key|*key==a[target].0)?;
            if tx.meta.as_option()?.pre_balances.get(i)!=Some(&0) {return None;}
            if calls.iter().enumerate().any(|(i,ix)|ix.is_root() || ix.stack_height()!=if i==0 {2}else{3}
                || !successful.contains(&((parent+1+offset+i)as u32))) {return None;}
        }
    }
    let mut calls=vec![&prefix[0],native];calls.extend(instructions.get(source+1..source+15)?.iter());
    if instructions.get(source+15).is_some_and(|ix|!ix.is_root()) || !successful.contains(&(parent as u32))
        || calls.iter().enumerate().any(|(n,ix)|ix.is_root() || ix.stack_height()!=[2,2,3,3,3,2,2,2,2,2,3,3,3,3,2,2][n]
            || !successful.contains(&(if n==0 {parent+1}else{source+n-1}as u32))) {return None;}
    let input=u64::from_le_bytes(d[2..10].try_into().ok()?);
    let transfer=&calls[0];let t=transfer.accounts();
    if transfer.program_id().0.as_slice()!=token_2022::PROGRAM_ID || transfer.data().len()!=10 || transfer.data()[0]!=12
        || t.len()!=4 || t[0].0!=r[3].0 || t[1].0!=a[1].0 || t[2].0!=a[14].0 || t[3].0!=r[0].0
        || u64::from_le_bytes(transfer.data()[1..9].try_into().ok()?)!=input
        || u64::from_le_bytes(native.data()[8..16].try_into().ok()?)!=input {return None;}
    let fee=&calls[2];let f=fee.accounts();
    let principal=&calls[3];let p=principal.accounts();let event=&calls[4];let e=event.accounts();
    if fee.program_id().0.as_slice()!=crate::pump_curve::FEE_PROGRAM || f.len()!=2 || f[0].0!=a[21].0 || f[1].0.as_slice()!=pumpfun::PROGRAM_ID
        || principal.program_id().0.as_slice()!=token_2022::PROGRAM_ID || principal.data()!=transfer.data() || p.len()!=4
        || p[0].0!=a[14].0 || p[1].0!=a[1].0 || p[2].0!=a[11].0 || p[3].0!=a[13].0
        || event.program_id().0.as_slice()!=pumpfun::PROGRAM_ID || e.len()!=1 || e[0].0!=a[24].0 {return None;}
    let wrap=&calls[5];let w=wrap.accounts();let sync=&calls[6];
    if wrap.program_id().0.iter().any(|v|*v!=0) || wrap.data().len()!=12 || wrap.data()[..4]!=[2,0,0,0]
        || w.len()!=3 || w[0].0!=r[0].0 || w[1].0!=a[15].0 || w[2].0!=a[13].0
        || u64::from_le_bytes(wrap.data()[4..12].try_into().ok()?)!=net
        || sync.program_id().0.as_slice()!=token::PROGRAM_ID || sync.data().as_slice()!=[17] || sync.accounts().len()!=1 || sync.accounts()[0].0!=a[15].0 {return None;}
    let route_fee=&calls[7];let f=route_fee.accounts();let close=&calls[8];let c=close.accounts();
    if route_fee.program_id().0.as_slice()!=token::PROGRAM_ID || route_fee.data().len()!=9 || route_fee.data()[0]!=3
        || f.len()!=3 || f[0].0!=a[15].0 || f[1].0!=r[10].0 || r[10].0!=r[11].0 || f[2].0!=a[13].0
        || close.program_id().0.as_slice()!=token::PROGRAM_ID || close.data().as_slice()!=[9] || c.len()!=3
        || c[0].0!=a[15].0 || c[1].0!=r[0].0 || c[2].0!=a[13].0 {return None;}
    let ata=&calls[9];let at=ata.accounts();
    if ata.program_id().0!=r[8].0 || ata.data().as_slice()!=[1] || at.len()!=6
        || at[0].0!=r[0].0 || at[1].0!=a[15].0 || at[2].0!=a[13].0 || at[3].0.as_slice()!=SOL_MINT
        || at[4].0.iter().any(|v|*v!=0) || at[5].0.as_slice()!=token::PROGRAM_ID {return None;}
    let size=&calls[10];let create=&calls[11];let ca=create.accounts();let immutable=&calls[12];let initialize=&calls[13];let ia=initialize.accounts();
    if size.program_id().0.as_slice()!=token::PROGRAM_ID || size.data().as_slice()!=[21,7,0] || size.accounts().len()!=1 || size.accounts()[0].0.as_slice()!=SOL_MINT
        || create.program_id().0.iter().any(|v|*v!=0) || create.data().len()!=52 || create.data()[..4]!=[0;4] || create.data()[12..20]!=165u64.to_le_bytes()
        || create.data()[20..52]!=token::PROGRAM_ID || ca.len()!=2 || ca[0].0!=r[0].0 || ca[1].0!=a[15].0
        || immutable.program_id().0.as_slice()!=token::PROGRAM_ID || immutable.data().as_slice()!=[22] || immutable.accounts().len()!=1 || immutable.accounts()[0].0!=a[15].0
        || initialize.program_id().0.as_slice()!=token::PROGRAM_ID || initialize.data().len()!=33 || initialize.data()[0]!=18 || initialize.data()[1..33]!=atlas
        || ia.len()!=2 || ia[0].0!=a[15].0 || ia[1].0.as_slice()!=SOL_MINT {return None;}
    let restore=&calls[14];let rs=restore.accounts();let sync=&calls[15];
    if restore.program_id().0.iter().any(|v|*v!=0) || restore.data().len()!=12 || restore.data()[..4]!=[2,0,0,0]
        || rs.len()!=2 || rs[0].0!=r[0].0 || rs[1].0!=a[15].0
        || sync.program_id().0.as_slice()!=token::PROGRAM_ID || sync.data().as_slice()!=[17] || sync.accounts().len()!=1 || sync.accounts()[0].0!=a[15].0 {return None;}
    let meta=tx.meta.as_option()?;let keys=tx.resolved_accounts();let quote=keys.iter().position(|k|*k==a[15].0)?;let owner=keys.iter().position(|k|*k==a[13].0)?;
    let before=*meta.pre_balances.get(quote)?;if meta.post_balances.get(quote)!=Some(&before) || meta.pre_balances.get(owner)!=meta.post_balances.get(owner) {return None;}
    let pre=meta.pre_token_balances.iter().find(|t|t.account_index==quote as u32)?;let post=meta.post_token_balances.iter().find(|t|t.account_index==quote as u32)?;
    if pre.mint!=substreams_solana::base58::encode(SOL_MINT) || post.mint!=pre.mint || pre.owner!=substreams_solana::base58::encode(&atlas)
        || post.owner!=pre.owner || pre.ui_token_amount.as_option()?.amount!=post.ui_token_amount.as_option()?.amount {return None;}
    let saved=pre.ui_token_amount.as_option()?.amount.parse::<u64>().ok()?;
    let rent=u64::from_le_bytes(create.data()[4..12].try_into().ok()?);let restore_amount=u64::from_le_bytes(restore.data()[4..12].try_into().ok()?);
    if restore_amount!=saved || rent.checked_add(saved)?!=before {return None;}
    let fee=u64::from_le_bytes(route_fee.data()[1..9].try_into().ok()?);if fee>net || net.checked_sub(fee)?<u64::from_le_bytes(d[10..18].try_into().ok()?) {return None;}
    Some(TitanNativeSettlement{parent,close:source+7,atlas:atlas.to_vec(),funder:r[0].0.clone(),quote:a[15].0.clone(),refund:before.checked_add(net)?.checked_sub(fee)?})
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, InnerInstruction, InnerInstructions, Message, MessageHeader, Transaction, TransactionStatusMeta,
    };

    #[test]
    fn finalized_v2_native_interface_and_titan_preparation_preserve_net() {
        let fixtures:&[(&[u8],usize,u32,u64)]=&[
(include_bytes!("../fixtures/swap-coverage/pump-native-sell27/0.transaction.pb").as_slice(),3,5,91348405u64),
(include_bytes!("../fixtures/swap-coverage/pump-native-sell27/1.transaction.pb").as_slice(),19,21,104877106u64),
(include_bytes!("../fixtures/swap-coverage/pump-native-sell27/2.transaction.pb").as_slice(),4,6,154427822u64),
        ];
        for &(bytes,source,first,net) in fixtures {
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let ix:Vec<_>=tx.walk_instructions().collect();let a=ix[source].accounts();let native=ix[source].data().clone();let event=ix[source+3].data().clone();let token=ix[source+2].data().clone();
            let result=crate::process_transaction(tx.clone()).unwrap();
            let row=result.swaps.iter().find(|s|s.source_index==Some(source as u32)).expect("native V2 original priced");
            assert_eq!((row.protocol,row.source_transfer_index,row.amm_pool.clone(),row.user.clone(),row.input_mint.clone(),row.input_amount,row.output_mint.clone(),row.output_amount),
                (pb::Protocol::Pumpfun.into(),Some(first),a[10].0.clone(),a[13].0.clone(),a[1].0.clone(),u64::from_le_bytes(native[8..16].try_into().unwrap()),SOL_MINT.to_vec(),net));
            for mode in 0..17 {
                let mut wrong=tx.clone();
                if mode==15 {let i=tx.resolved_accounts().iter().position(|k|*k==a[13].0).unwrap();wrong.meta.as_option_mut().unwrap().post_balances[i]+=1;}
                else if mode==16 {
                    let logs=&mut wrong.meta.as_option_mut().unwrap().log_messages;let p=logs.iter().rposition(|l|l==&format!("Program {} success",substreams_solana::base58::encode(&pumpfun::PROGRAM_ID))).unwrap();
                    logs[p]=format!("Program {} failed: custom program error: 1",substreams_solana::base58::encode(&pumpfun::PROGRAM_ID));
                } else {
                    let data=if mode<5 {&native}else if mode<11 {&event}else{&token};
                    if mode<5 && ix[source].is_root() {
                        let c=wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().find(|c|&c.data==data).unwrap();
                        match mode{0=>c.data[8]^=1,1=>c.data[16..24].copy_from_slice(&(net+1).to_le_bytes()),2=>c.data.extend_from_slice(&[0,0]),3=>c.accounts[13]=c.accounts[16],_=>c.accounts[24]=c.accounts[25]}
                    } else {
                        let c=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|c|&c.data==data).unwrap();
                        match mode{0=>c.data[8]^=1,1=>c.data[16..24].copy_from_slice(&(net+1).to_le_bytes()),2=>c.data.extend_from_slice(&[0,0]),3=>c.accounts[13]=c.accounts[16],4=>c.accounts[24]=c.accounts[25],
                            5=>c.data[16]^=1,6=>c.data[65]^=1,7=>c.data[64]=1,8=>c.data[48]^=1,9=>c.data[177]^=1,10=>c.data[225]^=1,
                            11=>c.data[1]^=1,12=>c.accounts[0]=c.accounts[2],13=>c.accounts[2]=c.accounts[0],_=>c.accounts[3]=c.accounts[2]}
                    }
                }
                assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source as u32))),"native source{source} mutation{mode}");
            }
        }
    }

    #[test]
    fn finalized_v3_roundtrips_bind_both_reserves_and_every_native_balance() {
        let fixtures:&[(&[u8],usize,u64)]=&[
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/0.transaction.pb").as_slice(),10,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/1.transaction.pb").as_slice(),7,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/2.transaction.pb").as_slice(),7,22554010u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/3.transaction.pb").as_slice(),7,22124386u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/4.transaction.pb").as_slice(),6,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/5.transaction.pb").as_slice(),6,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/6.transaction.pb").as_slice(),6,22177453u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/7.transaction.pb").as_slice(),6,22772932u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/8.transaction.pb").as_slice(),6,22925200u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/9.transaction.pb").as_slice(),5,23535313u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/10.transaction.pb").as_slice(),6,22270954u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/11.transaction.pb").as_slice(),6,23393206u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/12.transaction.pb").as_slice(),6,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/13.transaction.pb").as_slice(),6,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/14.transaction.pb").as_slice(),6,21010461u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/15.transaction.pb").as_slice(),6,22627900u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/16.transaction.pb").as_slice(),5,22920721u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/17.transaction.pb").as_slice(),6,22451633u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/18.transaction.pb").as_slice(),5,21510501u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/19.transaction.pb").as_slice(),6,22973733u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/20.transaction.pb").as_slice(),6,22432145u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/21.transaction.pb").as_slice(),6,22248287u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/22.transaction.pb").as_slice(),6,22432148u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/23.transaction.pb").as_slice(),6,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/24.transaction.pb").as_slice(),6,22432094u64),
(include_bytes!("../fixtures/swap-coverage/pump-roundtrip27/25.transaction.pb").as_slice(),6,22432094u64),
        ];
        for &(bytes,source,net) in fixtures {
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let ix:Vec<_>=tx.walk_instructions().collect();let committed=crate::logs::successful_invocations(&tx);
            assert_eq!(round_trip_native_sell(&tx,&ix,source,&committed),Some((net,(source+1)as u32)),"source{source}");
            let native=ix[source].data().clone();let event=ix[source+2].data().clone();
            let buy_event=ix.iter().find(|x|x.program_id().0.as_slice()==pumpfun::PROGRAM_ID && x.data().len()>100 && x.data().get(64)==Some(&1)).unwrap().data().clone();
            let system=ix.iter().find(|x|x.program_id().0.iter().all(|v|*v==0) && x.data().len()==12 && x.data()[..4]==[2,0,0,0]).unwrap().data().clone();
            let decoded=crate::process_transaction(tx.clone()).unwrap();
            let row=decoded.swaps.iter().find(|r|r.source_index==Some(source as u32)).expect("roundtrip sell priced by core");
            let a=ix[source].accounts();
            assert_eq!((row.protocol,row.source_transfer_index,row.amm_pool.clone(),row.user.clone(),row.input_mint.clone(),row.input_amount,row.output_mint.clone(),row.output_amount),
                (pb::Protocol::Pumpfun.into(),Some((source+1)as u32),a[5].0.clone(),a[8].0.clone(),a[1].0.clone(),u64::from_le_bytes(native[8..16].try_into().unwrap()),SOL_MINT.to_vec(),net));
            for mode in 0..22 {
                let mut wrong=tx.clone();
                if mode<3 || mode==15 || mode==20 {
                    let r=wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().find(|r|r.data==native).unwrap();
                    match mode {0=>r.data[8]^=1,1=>r.data[16..24].copy_from_slice(&(net+1).to_le_bytes()),2=>r.accounts[8]=r.accounts[13],15=>r.accounts[11]=r.accounts[12],_=>r.data.push(0)}
                } else if mode==16 {
                    let keys=tx.resolved_accounts();let i=keys.iter().position(|k|*k==a[8].0).unwrap();wrong.meta.as_option_mut().unwrap().post_balances[i]+=1;
                } else if mode==17 {
                    let keys=tx.resolved_accounts();let i=keys.iter().position(|k|*k==a[9].0).unwrap() as u32;
                    let r=wrong.meta.as_option_mut().unwrap().post_token_balances.iter_mut().find(|r|r.account_index==i).unwrap();
                    r.ui_token_amount.as_option_mut().unwrap().amount="1".into();
                } else if mode==18 || mode==19 {
                    let logs=&mut wrong.meta.as_option_mut().unwrap().log_messages;
                    let end=logs.iter().rposition(|l|l==&format!("Program {} success",substreams_solana::base58::encode(&pumpfun::PROGRAM_ID))).unwrap();
                    if mode==18 {logs[end]=format!("Program {} failed: custom program error: 1",substreams_solana::base58::encode(&pumpfun::PROGRAM_ID));}else{logs.truncate(end);}
                } else {
                    let target=if mode==8||mode==9 {&buy_event}else if mode==13||mode==14 {&system}else{&event};
                    let r=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|r|&r.data==target).unwrap();
                    match mode {3=>r.data[16]^=1,4=>r.data[65]^=1,5=>r.data[64]=1,6|8=>r.data[105]^=1,7|9=>r.data[113]^=1,
                        10=>r.data[177]^=1,11=>r.data[225]^=1,12=>r.data[303]^=1,13=>r.data[4]^=1,14=>r.accounts[1]=r.accounts[0],_=>r.accounts[0]^=1}
                }
                let w:Vec<_>=wrong.walk_instructions().collect();let committed=crate::logs::successful_invocations(&wrong);
                assert!(round_trip_native_sell(&wrong,&w,source,&committed).is_none(),"helper source{source} mutation{mode}");
                drop(w);
                assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|r|r.source_index!=Some(source as u32))),"fallback source{source} mutation{mode}");
            }
        }
    }

    #[test]
    fn finalized_curve_exact_spend_keeps_previous_financial_tuples() {
        let fixtures:&[(&[u8],u32,u32,&str,&str,&str,u64,&str,u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-curve-exact-spend26/0.transaction.pb").as_slice(), 19, 20, "J5SkNaYmJCdF9vJCBcVyujqixiaEAPXyFBZbg4LwuDbH", "CN3FYKeQik2wQwY8goDw6oHfagHkqY8MK9ZBjkk9ZABv", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", 2009760, "DJyHbwX5u6gHUQLf1fqyjr8reVSwfmgXVbMrE9wwpump", 195742223978),
            (include_bytes!("../fixtures/swap-coverage/pump-curve-exact-spend26/1.transaction.pb").as_slice(), 17, 18, "J5SkNaYmJCdF9vJCBcVyujqixiaEAPXyFBZbg4LwuDbH", "2KQKDA9RtgB8wfGMcnGsKMfFY2BkktG2ETMtRPSAzB8C", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", 10822028, "DJyHbwX5u6gHUQLf1fqyjr8reVSwfmgXVbMrE9wwpump", 522367153655),
            (include_bytes!("../fixtures/swap-coverage/pump-curve-exact-spend26/2.transaction.pb").as_slice(), 21, 22, "CTgr9yQNKuwasAy1WMzYzeCrEzr4ZBt1oZA9yewygVpk", "6KobcKGGX1aTvZyaXvTNcMv9GYt5oLhZnmcBboNwec37", "XPLTrL1ZH3tZJBdFWbbtrNjK9qoqZWfuhQJnXibbdfT", 116251108877, "BBSU4XLUq48jFr2rVV9vLcxfXYoG92oM37qTcuhFpump", 23826345255281),
            (include_bytes!("../fixtures/swap-coverage/pump-curve-exact-spend26/3.transaction.pb").as_slice(), 17, 19, "7BMih8fnu4AvAftqeheSnNMDTDmGHvqzyyqpVfJpYtBT", "ANhNhjH47k49GfVsRXWAB7ke9heNwyUNqNjJVo2fNL3g", "XspzcW1PRtgf6Wj92HCiZdjzKCyFekVD8P5Ueh3dRMX", 3572776, "6xQARVkkByCExiZmqSK6ritMA9o1E5sG12uca4JLpump", 4495045496687),
            (include_bytes!("../fixtures/swap-coverage/pump-curve-exact-spend26/4.transaction.pb").as_slice(), 14, 15, "7BMih8fnu4AvAftqeheSnNMDTDmGHvqzyyqpVfJpYtBT", "4kF9wfK4a4vXjvf5qXNkyiaa5QAS1BcHPNcSmGYwvQRT", "XspzcW1PRtgf6Wj92HCiZdjzKCyFekVD8P5Ueh3dRMX", 7826288, "6xQARVkkByCExiZmqSK6ritMA9o1E5sG12uca4JLpump", 8306694919241),
            (include_bytes!("../fixtures/swap-coverage/pump-curve-exact-spend26/5.transaction.pb").as_slice(), 20, 21, "CjJYeZZGLpwjpkhSb4yJtGetivAXdidUfV3LvmzrUhiL", "4k1U14CSSLyACLoTRGFRq8NffY6LrnFP5kDTLreKPMzL", "A7bdiYdS5GjqGFtxf17ppRHtDKPkkRqbKtR27dxvQXaS", 7764514, "6TGiuQdBEemiDEJ8WoGuXKECmpLh8gvnGQ2qD9kJpump", 12420170604595),
            (include_bytes!("../fixtures/swap-coverage/pump-curve-exact-spend26/6.transaction.pb").as_slice(), 24, 25, "BeUTqz3gYcVvX45Y4LvQd8qyE1LQBUY7SQUSwLLuNVSZ", "9Wj67iqcSPCuiZqAUsMCeYnYn2XwQa5uzSAQZ8nTqbh", "XspzcW1PRtgf6Wj92HCiZdjzKCyFekVD8P5Ueh3dRMX", 8125308, "5BYrGVrzt1o3FVnsSRJoS3KT8TSfzASRFBkjw7qGpump", 4623456534383),
        ];
        for &(bytes,source,first,pool,user,input_mint,input,output_mint,output)in fixtures {
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();
            assert_eq!(rows.len(),1,"source {source}");let row=rows[0];let decode=|s:&str|substreams_solana::base58::decode(s).unwrap();
            assert_eq!((row.source_transfer_index,row.amm_pool.clone(),row.user.clone(),row.input_mint.clone(),row.input_amount,row.output_mint.clone(),row.output_amount),
                (Some(first),decode(pool),decode(user),decode(input_mint),input,decode(output_mint),output));
            let walk:Vec<_>=tx.walk_instructions().collect();let end=walk.iter().enumerate().skip(source as usize+1)
                .find(|(_,i)|i.is_root() || i.stack_height()<=walk[source as usize].stack_height()).map(|(n,_)|n).unwrap_or(walk.len());
            assert!(exact_quote_budget_satisfies(&walk[source as usize],input,output,&walk[source as usize+1..end]),"primary retained pre-deficit fee basis source{source}");
            let native_data=walk[source as usize].data().clone();let event_data=walk[end-1].data().clone();
            for mode in 0..3 {
                let mut wrong=tx.clone();let call=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|i|i.data==native_data).unwrap();
                match mode{0=>call.data[8..16].copy_from_slice(&(input-1).to_le_bytes()),1=>call.data[16..24].copy_from_slice(&(output+1).to_le_bytes()),_=>call.data.extend_from_slice(&[2,2,2])}
                assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"existing curve source{source} malformed{mode}");
            }
            for offset in [177,225] {
                let mut wrong=tx.clone();let call=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|i|i.data==event_data).unwrap();call.data[offset]^=1;
                let walk:Vec<_>=wrong.walk_instructions().collect();assert!(!exact_quote_budget_satisfies(&walk[source as usize],input,output,&walk[source as usize+1..end]),"incorrect retained source fee{offset}");
            }
        }
    }

    #[test]
    fn finalized_curve_quote_budgets_preserve_existing_financial_tuples() {
        let fixtures: &[(&[u8],u32,u32,&str,&str,&str,u64,&str,u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-curve-budget-rounding26/0.transaction.pb").as_slice(), 25, 26, "95EuA9NEHHb4c8jMQ7bYZYCQxH52JLy5kBRHQhBKcd2v", "4bPAd6teDaz5NJ3WiqxKTwWKPeSbUbgtSFKRrnJPnry5", "85VBFQZC9TZkfaptBWjvUw7YbZjy52A6mjtPGjstQAmQ", 2732439825, "A51RN53Krd3Wpxhhb11GZ96TU5PbHwzq5vGcEfSqpump", 9250639363485),
            (include_bytes!("../fixtures/swap-coverage/pump-curve-budget-rounding26/1.transaction.pb").as_slice(), 11, 12, "6ndKPXCsmdWaPQz4ngjdX58qHKxiJmCFwuarXeZza2qT", "7GRssGJHusWaYMAvoWGqjAL3RQ2S1knn1FMM6E6bUWN", "85VBFQZC9TZkfaptBWjvUw7YbZjy52A6mjtPGjstQAmQ", 2619025650, "8CgM1hC3LaVHaSoWDGaGdSTfsmK6hk51PM4NdaKgpump", 6485323286047),
        ];
        for &(bytes,source,first,pool,user,input_mint,input,output_mint,output) in fixtures {
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let walk:Vec<_>=tx.walk_instructions().collect();let end=walk.iter().enumerate().skip(source as usize+1)
                .find(|(_,i)|i.is_root() || i.stack_height()<=walk[source as usize].stack_height()).map(|(n,_)|n).unwrap_or(walk.len());
            assert!(exact_quote_budget_satisfies(&walk[source as usize],input,output,&walk[source as usize+1..end]));
            let native_data=walk[source as usize].data().clone();let event_data=walk[end-1].data().clone();
            let decoded=crate::process_transaction(tx.clone()).unwrap();
            let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)).collect();
            assert_eq!(rows.len(),1,"source {source}");let row=rows[0];
            let decode=|s:&str|substreams_solana::base58::decode(s).unwrap();
            assert_eq!((row.source_transfer_index,row.amm_pool.clone(),row.user.clone(),row.input_mint.clone(),row.input_amount,row.output_mint.clone(),row.output_amount),
                (Some(first),decode(pool),decode(user),decode(input_mint),input,decode(output_mint),output));
            for mode in 0..12 {
                let mut wrong=tx.clone();
                let all=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions);
                let call=all.into_iter().find(|i|i.data==if mode<4{native_data.clone()}else{event_data.clone()}).unwrap();
                let a=call.accounts.clone();
                match mode {
                    0=>call.data[8..16].copy_from_slice(&(input-1).to_le_bytes()),
                    1=>call.data[16..24].copy_from_slice(&(output+1).to_le_bytes()),
                    2=>call.data.extend_from_slice(&[0,0]),3=>call.data[24]=2,
                    4=>call.data[65]^=1,5=>call.data[16]^=1,6=>call.data[64]=0,
                    7=>call.data[56]^=1,8=>call.data[177]^=1,9=>call.data[217]^=1,
                    10=>call.data[270]^=1,_=>call.accounts[0]=a[0]^1,
                }
                assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"curve source{source} mutation{mode}");
            }
            // Budget increments that keep the same rounded net and physical
            // cost are semantically valid; the event contains no budget field.
            let mut bounded=tx.clone();let call=bounded.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|i|i.data==native_data).unwrap();
            call.data[8..16].copy_from_slice(&(input+2).to_le_bytes());
            assert!(crate::process_transaction(bounded).unwrap().swaps.iter().any(|s|s.source_index==Some(source)&&s.input_amount==input&&s.output_amount==output));
        }
    }

    #[test]
    fn titanium_native_sell_binds_atlas_sweep_and_exact_wsol_recreation() {
        let tx=substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/swap-coverage/pump-live26/3T41BUa8j97bj7UbZnhBt6LPcw6BV8kuBqDsp1MZ6hMej8VrUoAj8iwAiZvAWe1rm5YeR5iAieiSccupXJ3w3fa2.pb").to_vec()).unwrap();
        let mint=substreams_solana::base58::decode("EQsUrUJuGDSmFjBGJ1Rsea4u1MyH1AfDkDnyM8W4pump").unwrap();
        let bytes=include_bytes!("../fixtures/swap-coverage/pump-live26/titan/EQsUrUJuGDSmFjBGJ1Rsea4u1MyH1AfDkDnyM8W4pump.mint");
        let (schedule,initialized)=crate::token_fees::owned_mint(&substreams_solana_idls::spl::token_2022::PROGRAM_ID,bytes).unwrap();let fee=schedule.at(0).unwrap();assert!(initialized&&fee.basis_points==0&&fee.maximum_fee==0);
        let lookup=|address:&[u8],_:u32|(address==mint.as_slice()).then_some(fee);
        let walk:Vec<_>=tx.walk_instructions().collect();let successful=crate::logs::successful_invocations(&tx);
        let proof=titan_native_settlement(&tx,&walk,4,1103177,&successful).expect("primary Atlas profile");assert_eq!(proof.refund,7721115981);
        let decoded=crate::process_transaction_with_metadata(tx.clone(),None,|_,_|None,|_,_|None,lookup).unwrap();
        let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(4)).collect();assert_eq!(rows.len(),1);
        assert_eq!((rows[0].source_transfer_index,rows[0].input_amount,rows[0].output_amount),(Some(6),39810570109,1103177));assert!(rows[0].transfer_verified);
        for mode in 0..20 {
            let mut wrong=tx.clone();
            if mode<5 {let ix=&mut wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[2];let a=ix.accounts.clone();match mode {
                0=>ix.data.push(0),1=>ix.data[18]^=1,2=>ix.data[2]^=1,3=>ix.accounts[2]=a[0],_=>ix.data[10..18].copy_from_slice(&1103178u64.to_le_bytes())
            }}else if mode<17 {let g=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().find(|g|g.index==2).unwrap();let (n,kind)=match mode{5=>(5,0),6=>(5,1),7=>(5,2),8=>(3,1),9=>(1,1),10=>(8,1),11=>(11,0),12=>(11,1),13=>(14,0),14=>(13,0),15=>(9,1),_=>(6,2)};let ix=&mut g.instructions[n];let a=ix.accounts.clone();match kind{
                0=>ix.data[if mode==14{1}else{4}]^=1,1=>ix.accounts[if mode==8{3}else if mode==9{13}else if mode==15{2}else{1}]=a[0],_=>ix.stack_height=Some(99)
            }}else if mode==17{wrong.meta.as_option_mut().unwrap().log_messages.clear();}else if mode==18{wrong.meta.as_option_mut().unwrap().post_balances[0]+=1;}else{wrong.meta.as_option_mut().unwrap().err=buffa::MessageField::some(Default::default());}
            assert!(crate::process_transaction_with_metadata(wrong,None,|_,_|None,|_,_|None,lookup).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(4))),"invalid Titanium mode{mode}");
        }
    }

    #[test]
    fn native_sells_bind_token_quote_siblings_and_prior_curve_extension() {
        let fixtures: &[(&[u8], u32, u32, u64, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-live-bounded/4.transaction.pb"), 11, 14, 583111108589, 16186670),
            (include_bytes!("../fixtures/swap-coverage/pump-live-bounded/3.transaction.pb"), 13, 15, 5288334904277, 151910719),
        ];
        for &(bytes, source, first, input, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let metadata = [
                (include_bytes!("../fixtures/swap-coverage/pump-live-bounded/legacy-base.mint").as_slice(), substreams_solana::base58::decode("6aXmESRXg6SDNf8Kwzy9WSF7FX6EpjW6XuYkQwu6pump").unwrap()),
                (include_bytes!("../fixtures/swap-coverage/pump-live-bounded/sibling-base.mint").as_slice(), substreams_solana::base58::decode("GGxMBLxCut9Po4vb1fvr2eCbgAzoBTWHUAoDBq6Xpump").unwrap()),
            ];
            let lookup = |address: &[u8], _: u32| metadata.iter().find(|(_, mint)| mint == address)
                .and_then(|(bytes, _)| crate::token_fees::owned_mint(&substreams_solana_idls::spl::token_2022::PROGRAM_ID, bytes))
                .and_then(|(schedule, initialized)| initialized.then(|| schedule.at(0)).flatten());
            let decoded = crate::process_transaction_with_metadata(tx.clone(), None, |_,_| None, |_,_| None, lookup).unwrap();
            let rows: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(source)).collect();
            assert_eq!(rows.len(), 1, "native source {source}");
            assert_eq!((rows[0].source_transfer_index,rows[0].input_amount,rows[0].output_amount),(Some(first),input,output));
            for mode in 0..4 {
                let mut altered = tx.clone();
                let original_data = tx.walk_instructions().nth(if source == 13 { 24 } else { 7 }).unwrap().data().clone();
                let call = altered.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
                    .find(|ix| ix.data == original_data).unwrap();
                match mode {
                    0 => call.data[0] ^= 1,
                    1 => call.accounts[if source == 13 { 2 } else { 0 }] ^= 1,
                    2 => call.stack_height = Some(1),
                    3 => if source == 13 { call.data[8] ^= 1 } else { call.accounts[1] ^= 1 },
                    _ => unreachable!(),
                }
                assert!(crate::process_transaction_with_metadata(altered, None, |_,_| None, |_,_| None, lookup).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),
                    "native sibling/extension witness mutation {mode}");
            }
        }
    }

    #[test]
    fn finalized_quote_v2_sell_boolean_uses_exact_net_receipt() {
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&include_bytes!("../fixtures/swap-coverage/pump-live-bounded/2.transaction.pb").to_vec()).unwrap();
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(8)).collect();
        assert_eq!(swaps.len(), 1);
        assert_eq!((swaps[0].source_transfer_index, swaps[0].input_amount, swaps[0].output_amount),
            (Some(10), 7006252285295, 59866866764));
        let native_data = tx.walk_instructions().nth(8).unwrap().data().clone();
        for mode in 0..5 {
            let mut altered = tx.clone();
            let data = &mut altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut()
                .find(|ix| ix.data == native_data).unwrap().data;
            assert_eq!(*data, native_data);
            match mode {
                0 => data[24] = 2,
                1 => data.push(0),
                2 => data.truncate(23),
                3 => data[8] ^= 1,
                4 => data[0] ^= 1,
                _ => unreachable!(),
            }
            assert!(crate::process_transaction(altered).map_or(true, |d| d.swaps.iter().all(|s| s.source_index != Some(8))),
                "native quote suffix mutation {mode}");
        }
    }

    #[test]
    fn emits_bonding_curve_buys_including_exact_sol_input() {
        for discriminator in [[102, 6, 61, 18, 1, 218, 235, 234], pumpfun::instructions::BUY_EXACT_SOL_IN] {
            let mut accounts: Vec<Vec<u8>> = (0..12).map(|i| vec![i; 32]).collect();
            accounts.push(pumpfun::PROGRAM_ID.to_vec());
            let mut buy = discriminator.to_vec();
            buy.extend_from_slice(&1_000u64.to_le_bytes());
            buy.extend_from_slice(&10_000u64.to_le_bytes());
            buy.push(1); // current track_volume flag

            let mut event = vec![0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
            event.extend_from_slice(&pumpfun::events::TRADE);
            event.extend_from_slice(&accounts[2]);
            event.extend_from_slice(&1_000u64.to_le_bytes());
            event.extend_from_slice(&10_000u64.to_le_bytes());
            event.push(1);
            event.extend_from_slice(&accounts[6]);

            let tx = ConfirmedTransaction {
                transaction: buffa::MessageField::some(Transaction {
                    message: buffa::MessageField::some(Message {
                        header: buffa::MessageField::some(MessageHeader {
                            num_required_signatures: 1,
                            ..Default::default()
                        }),
                        account_keys: accounts,
                        instructions: vec![
                            CompiledInstruction {
                                program_id_index: 12,
                                accounts: (0..12).collect(),
                                data: buy,
                            },
                        ],
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                meta: buffa::MessageField::some(TransactionStatusMeta { inner_instructions: vec![InnerInstructions { index: 0, instructions: vec![InnerInstruction { program_id_index: 12, accounts: vec![10], data: event, stack_height: Some(2) }] }], ..Default::default() }),
            };
            let mut pending = None;
            let swaps: Vec<_> = tx.walk_instructions().enumerate().filter_map(|(index, ix)| handle_instruction(&mut pending, &ix, index as u32)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!(swaps[0].amm_pool, vec![3; 32]);
            assert_eq!(swaps[0].input_mint, SOL_MINT.to_vec());
            assert_eq!(swaps[0].output_mint, vec![2; 32]);
            assert_eq!(swaps[0].input_amount, 1_000);
            assert_eq!(swaps[0].output_amount, 10_000);
        }
    }
}
