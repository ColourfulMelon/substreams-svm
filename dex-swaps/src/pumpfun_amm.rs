use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::pumpswap;
use substreams_solana_idls::pumpswap::accounts::TradeTokenAccounts;
use substreams_solana_idls::pumpswap::events::TradeDirection;

// Boost trades acquire base inventory by burning the pool's base reserve.
// Keep the burn receipt separate from a trader token-account credit.
pub(crate) struct BoostBurn { pub swap: pb::Swap, pub burn_index: u32 }

pub(crate) fn decode_boost_burns(tx:&substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction)->Vec<BoostBurn> {
    use std::collections::HashMap;
    use substreams_solana_idls::spl::{token,token_2022};
    let prove=||->Option<Vec<BoostBurn>> {
        let meta=tx.meta.as_option()?;if meta.err.is_set(){return None;}
        let walk:Vec<_>=tx.walk_instructions().collect();let keys=tx.resolved_accounts();
        let committed=crate::logs::successful_invocations(tx);
        let mut native=HashMap::<Vec<u8>,i128>::new();let mut token_delta=HashMap::<Vec<u8>,i128>::new();
        let mut mints=HashMap::<Vec<u8>,Vec<u8>>::new();
        for b in meta.pre_token_balances.iter().chain(&meta.post_token_balances) {
            let key=keys.get(b.account_index as usize)?;let mint=substreams_solana::base58::decode(&b.mint).ok()?;
            if mints.insert((*key).clone(),mint.clone()).is_some_and(|other|other!=mint){return None;}
        }
        let mut result=Vec::new();
        for (source,ix) in walk.iter().enumerate() {
            let a=ix.accounts();let d=ix.data();let program=ix.program_id().0;
            if !committed.contains(&(source as u32)) {return None;}
            if program.as_slice()==pumpswap::PROGRAM_ID && d.starts_with(&[105,68,6,175,0,7,35,162]) {
                if !ix.is_root() || d.len()!=24 || a.len()!=13 || a[4].0.as_slice()!=crate::SOL_MINT || a[9].0.as_slice()!=token_2022::PROGRAM_ID
                    || a[10].0.as_slice()!=token::PROGRAM_ID || a[12].0.as_slice()!=pumpswap::PROGRAM_ID
                    || [0,1,3,4,5,6,7,8].iter().enumerate().any(|(p,r)|[0,1,3,4,5,6,7,8][p+1..].iter().any(|q|a[*r].0==a[*q].0)) {return None;}
                let authority=solana_pubkey::Pubkey::find_program_address(&[b"boost_vault",a[0].0],&solana_pubkey::Pubkey::new_from_array(pumpswap::PROGRAM_ID)).0.to_bytes();
                let ata=solana_pubkey::Pubkey::find_program_address(&[&authority,&token::PROGRAM_ID,a[4].0],
                    &solana_pubkey::Pubkey::new_from_array(substreams_solana_macro::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"))).0.to_bytes();
                let event_authority=solana_pubkey::Pubkey::find_program_address(&[b"__event_authority"],&solana_pubkey::Pubkey::new_from_array(pumpswap::PROGRAM_ID)).0.to_bytes();
                if a[7].0.as_slice()!=authority || a[8].0.as_slice()!=ata || a[11].0.as_slice()!=event_authority {return None;}
                let end=walk.iter().enumerate().skip(source+1).find(|(_,x)|x.is_root()).map(|(i,_)|i).unwrap_or(walk.len());
                let c=walk.get(source+1..end)?;
                if !matches!(c.len(),4|5) || c.iter().any(|x|x.is_root() || x.stack_height()!=2) {return None;}
                let q=&c[0];let qa=q.accounts();let burn=&c[1];let ba=burn.accounts();let buy=&c[2];let event=&c[3];let e=event.data();let b=buy.data();
                if q.program_id().0!=a[10].0 || q.data().len()!=10 || q.data()[0]!=12 || q.data()[9]!=9 || qa.len()!=4
                    || qa[0].0!=a[8].0 || qa[1].0!=a[4].0 || qa[2].0!=a[6].0 || qa[3].0!=a[7].0
                    || burn.program_id().0!=a[9].0 || burn.data().len()!=9 || burn.data()[0]!=8 || ba.len()!=3
                    || ba[0].0!=a[5].0 || ba[1].0!=a[3].0 || ba[2].0!=a[0].0
                    || event.program_id().0.as_slice()!=pumpswap::PROGRAM_ID || event.accounts().len()!=1 || event.accounts()[0].0!=a[11].0
                    || e.len()!=216 || e[..16]!=[228,69,165,46,81,203,154,29,63,69,28,22,48,92,194,185]
                    || e[24..56]!=a[3].0[..] || e[88..120]!=a[0].0[..] || e[120..152]!=a[1].0[..]
                    || buy.program_id().0.as_slice()!=pumpswap::PROGRAM_ID || buy.accounts().len()!=1 || buy.accounts()[0].0!=a[11].0
                    || b.len()!=512 || b[..16]!=[228,69,165,46,81,203,154,29,103,244,82,31,44,245,119,119]
                    || b[128..160]!=a[0].0[..] || b[160..192]!=a[7].0[..] || b[192..224]!=[0;32] || b[224..256]!=a[8].0[..]
                    || b[256..320]!=[0;64] || b[409..413]!=18u32.to_le_bytes() || b[413..431]!=b"buy_exact_quote_in"[..] {return None;}
                let u=|data:&[u8],o|data.get(o..o+8).and_then(|x|x.try_into().ok()).map(u64::from_le_bytes);
                let quote=u(q.data(),1)?;let output=u(burn.data(),1)?;
                if quote==0 || output==0 || quote>u(d,8)? || output<u(d,16)? || u(e,152)?!=u(d,8)? || u(e,160)?!=quote || u(e,168)?!=output
                    || u(b,24)?!=output || u(b,32)?!=u(d,8)? || u(b,72)?!=quote || u(b,112)?!=quote || u(b,120)?!=quote
                    || u(b,401)?!=u(d,16)? || b[80..112]!=[0;32] || b[352..369]!=[0;17] || b[431..463]!=[0;32]
                    || u(b,56)?.checked_sub(output)?!=u(e,200)? || u(b,64)?.checked_add(quote)?!=u(e,192)?
                    || u(b,48)?.checked_sub(quote)?!=u(e,208)? || b[463..479]!=e[176..192] {return None;}
                for (role,mint,owner) in [(5,3,0),(6,4,0),(8,4,7)] {
                    let index=keys.iter().position(|k|*k==a[role].0)? as u32;
                    for balance in meta.pre_token_balances.iter().chain(&meta.post_token_balances).filter(|b|b.account_index==index) {
                        if balance.mint!=substreams_solana::base58::encode(a[mint].0) || balance.owner!=substreams_solana::base58::encode(a[owner].0)
                            || balance.program_id!=substreams_solana::base58::encode(a[if mint==3 {9}else{10}].0) {return None;}
                    }
                }
                if c.len()==5 {
                    let close=&c[4];let ca=close.accounts();
                    if close.program_id().0.as_slice()!=token::PROGRAM_ID || close.data().as_slice()!=[9] || ca.len()!=3
                        || ca[0].0!=a[8].0 || ca[1].0!=a[1].0 || ca[2].0!=a[7].0 || u(e,208)?!=0 {return None;}
                }
                result.push(BoostBurn {burn_index:(source+2)as u32,swap:pb::Swap{
                    protocol:pb::Protocol::PumpfunAmmBuyBurn.into(),program_id:pumpswap::PROGRAM_ID.to_vec(),amm:pumpswap::PROGRAM_ID.to_vec(),
                    amm_pool:a[0].0.clone(),user:a[7].0.clone(),input_mint:a[4].0.clone(),output_mint:a[3].0.clone(),
                    input_amount:quote,output_amount:output,source_index:Some(source as u32),source_transfer_index:Some((source+1)as u32),
                    source_burn_index:Some((source+2)as u32),burn_output_verified:true,stack_height:1,..Default::default()
                }});
            } else if program.as_slice()==pumpswap::PROGRAM_ID {
                // Only the two direct native events can occur in this profile.
                if ix.is_root() || a.len()!=1 || !d.starts_with(&[228,69,165,46,81,203,154,29]) {return None;}
            } else if program.iter().all(|v|*v==0) {
                if d.len()!=12 || d[..4]!=[2,0,0,0] || a.len()!=2 {return None;}
                let amount=u64::from_le_bytes(d[4..12].try_into().ok()?);
                *native.entry(a[0].0.clone()).or_default()-=i128::from(amount);*native.entry(a[1].0.clone()).or_default()+=i128::from(amount);
            } else if program.as_slice()==token::PROGRAM_ID || program.as_slice()==token_2022::PROGRAM_ID {
                match d.first().copied()? {
                    12 if d.len()==10 && a.len()==4 && mints.get(a[0].0)==Some(a[1].0) && mints.get(a[2].0)==Some(a[1].0) => {
                        let amount=u64::from_le_bytes(d[1..9].try_into().ok()?);
                        *token_delta.entry(a[0].0.clone()).or_default()-=i128::from(amount);*token_delta.entry(a[2].0.clone()).or_default()+=i128::from(amount);
                        if a[1].0.as_slice()==crate::SOL_MINT {
                            if program.as_slice()!=token::PROGRAM_ID {return None;}
                            *native.entry(a[0].0.clone()).or_default()-=i128::from(amount);*native.entry(a[2].0.clone()).or_default()+=i128::from(amount);
                        }
                    },
                    8 if d.len()==9 && a.len()==3 && mints.get(a[0].0)==Some(a[1].0) => {*token_delta.entry(a[0].0.clone()).or_default()-=i128::from(u64::from_le_bytes(d[1..9].try_into().ok()?));},
                    9 if d.as_slice()==[9] && a.len()==3 && mints.get(a[0].0).is_some_and(|m|m.as_slice()==crate::SOL_MINT) => {
                        let i=keys.iter().position(|k|*k==a[0].0)?;if *meta.post_balances.get(i)?!=0 {return None;}
                        let residual=i128::from(*meta.pre_balances.get(i)?)+native.get(a[0].0).copied().unwrap_or(0);
                        if residual<0 {return None;}
                        *native.entry(a[0].0.clone()).or_default()-=residual;*native.entry(a[1].0.clone()).or_default()+=residual;
                    },
                    _=>return None,
                }
            } else if program.as_slice()!=substreams_solana_macro::b58!("ComputeBudget111111111111111111111111111111") {return None;}
        }
        *native.entry(keys.first()?.to_vec()).or_default()-=i128::from(meta.fee);
        if keys.iter().enumerate().any(|(i,key)|meta.post_balances.get(i).zip(meta.pre_balances.get(i)).map(|(after,before)|i128::from(*after)-i128::from(*before))!=Some(native.get(*key).copied().unwrap_or(0))) {return None;}
        for (key,delta) in token_delta {
            let index=keys.iter().position(|k|**k==key)? as u32;
            let balance=|rows:&[substreams_solana::pb::sf::solana::r#type::v1::TokenBalance]|rows.iter().find(|x|x.account_index==index).map(|x|x.ui_token_amount.as_option()?.amount.parse::<u64>().ok()).unwrap_or(Some(0));
            if i128::from(balance(&meta.post_token_balances)?)-i128::from(balance(&meta.pre_token_balances)?)!=delta {return None;}
        }
        Some(result)
    };
    prove().unwrap_or_default()
}

pub(crate) struct PendingTrade {
    pool: Vec<u8>,
    base_mint: Vec<u8>,
    quote_mint: Vec<u8>,
    user: Vec<u8>,
    is_buy: bool,
    source_index: u32,
    depth: u32,
    event_authority: Vec<u8>,
}

pub(crate) fn handle_instruction(pending_trade: &mut Option<PendingTrade>, instruction: &InstructionView, source_index: u32) -> Option<pb::Swap> {
    // Expire the pending event at the end of its native invocation subtree.
    if pending_trade.as_ref().map_or(false, |trade| instruction.stack_height() <= trade.depth) {
        *pending_trade = None;
    }
    let program_id = instruction.program_id().0;
    if program_id != &pumpswap::PROGRAM_ID {
        return None;
    }

    if let Some(mut trade) = decode_trade_instruction(instruction) {
        trade.source_index = source_index;
        trade.depth = if instruction.is_root() { 1 } else { instruction.stack_height() };
        *pending_trade = Some(trade);
        return None;
    }

    // Permissive trade-event decoder lives in `substreams_solana_idls::pumpswap`
    // — read the comment there for the rationale (pump.fun has appended fields
    // to BuyEvent/SellEvent multiple times since launch and we don't want each
    // sink chasing the IDL).
    let event = pumpswap::events::unpack_trade_event_minimal(instruction.data()).ok()??;
    let trade = pending_trade.take()?;

    let is_buy = matches!(event.direction, TradeDirection::Buy);
    // The stable event prefix contains pool at payload offset 112 and user
    // at offset 144. Bind both identities and direction to this exact self-CPI.
    if instruction.stack_height() != trade.depth + 1
        || instruction.accounts().len() != 1
        || instruction.accounts()[0].0.as_slice() != trade.event_authority.as_slice()
        || event.user.as_slice() != trade.user
        || is_buy != trade.is_buy
        || instruction.data().get(128..160)? != trade.pool
    { return None; }
    let (input_amount, output_amount) = if is_buy {
        (event.quote_amount, event.base_amount)
    } else {
        (event.base_amount, event.quote_amount)
    };

    Some(pb::Swap {
        protocol: pb::Protocol::PumpfunAmm.into(),
        program_id: pumpswap::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        source_index: Some(trade.source_index),
        amm: pumpswap::PROGRAM_ID.to_vec(),
        amm_pool: trade.pool,
        user: event.user.to_vec(),
        input_mint: if is_buy { trade.quote_mint.clone() } else { trade.base_mint.clone() },
        input_amount,
        output_mint: if is_buy { trade.base_mint } else { trade.quote_mint },
        output_amount,
        ..Default::default()
    })
}

pub(crate) fn extract_pool(instruction: &InstructionView) -> Option<Vec<u8>> {
    if instruction.program_id().0 != &pumpswap::PROGRAM_ID {
        return None;
    }
    // `routed_pool::Tracker::observe` only needs the pool address (accounts[0])
    // for cross-protocol routing attribution. We deliberately don't run the
    // full strict `TradeTokenAccounts::try_from` here — pre-filter inner CPI
    // fixtures may carry only the pool slot, and the strict path would reject
    // them with `Missing` for trailing accounts.
    if !is_trade_instruction(instruction) {
        return None;
    }
    instruction.accounts().get(0).map(|a| a.0.to_vec())
}

// These bounded suffix profiles are retained in finalized native originals.
// The primary Anchor ABI consumes the two-u64 trade prefix and, for legacy
// buys, one OptionBool. It does not consume the remaining caller padding.
// Keep this parser shared with the final receipt gate so routed candidates
// cannot bypass rejected lengths or boolean encodings.
pub(crate) fn unpack_trade_instruction(data: &[u8]) -> Result<pumpswap::instructions::PumpSwapInstruction, substreams_solana_idls::common::ParseError> {
    let normalized = if data.starts_with(&pumpswap::instructions::SELL)
        && matches!(data.len(), 25 | 26) && data[24..].iter().all(|flag| *flag <= 1)
        || data.starts_with(&pumpswap::instructions::SELL_V2) && data.len() == 25 && data[24] <= 1
    { &data[..24] }
    else if data.starts_with(&pumpswap::instructions::BUY_EXACT_QUOTE_IN)
        && data.len() == 32 && data[24..] == [0, 0, 0, 0, 0, 0, 0, 1]
    { &data[..25] }
    else { data };
    pumpswap::instructions::unpack(normalized)
}

pub(crate) fn amounts_satisfy(instruction: &InstructionView, input: u64, gross_output: u64, calls: &[InstructionView]) -> bool {
    use pumpswap::instructions::PumpSwapInstruction::*;
    match unpack_trade_instruction(instruction.data()) {
        Ok(Buy(a) | BuyV2(a)) => input <= a.max_quote_amount_in && gross_output >= a.base_amount_out,
        Ok(BuyExactQuoteIn(a) | BuyExactQuoteInV2(a)) => {
            // The official SDK exactQuoteInFees floors the fee-adjusted budget;
            // the committed user debit can therefore leave a rounding remainder.
            // Bind the original budget and minimum to the canonical native event
            // before accepting that debit, rather than demanding equality.
            let accounts = instruction.accounts();
            if accounts.len() <= 15 || input == 0 || input > a.spendable_quote_in
                || gross_output < a.min_base_amount_out { return false; }
            let depth = if instruction.is_root() { 1 } else { instruction.stack_height() };
            let mut events = calls.iter().filter(|event| event.program_id().0 == &pumpswap::PROGRAM_ID
                && event.stack_height() == depth + 1
                && event.data().starts_with(&[0xe4,0x45,0xa5,0x2e,0x51,0xcb,0x9a,0x1d,0x67,0xf4,0x52,0x1f,0x2c,0xf5,0x77,0x77]));
            let Some(event) = events.next() else { return false; };
            if events.next().is_some() || event.accounts().len() != 1
                || event.accounts()[0].0 != accounts[15].0 { return false; }
            let data = event.data();
            let u64_at = |offset: usize| data.get(offset..offset + 8)
                .and_then(|b| b.try_into().ok()).map(u64::from_le_bytes);
            let Some(name_len) = data.get(409..413).and_then(|b| b.try_into().ok()).map(u32::from_le_bytes) else { return false; };
            let expected_name: &[u8] = if instruction.data().starts_with(&pumpswap::instructions::BUY_EXACT_QUOTE_IN_V2)
                { b"buy_exact_quote_in_v2" } else { b"buy_exact_quote_in" };
            name_len as usize == expected_name.len()
                && data.get(413..413 + name_len as usize) == Some(expected_name)
                && data.get(128..160) == Some(accounts[0].0.as_slice())
                && data.get(160..192) == Some(accounts[1].0.as_slice())
                && u64_at(32) == Some(a.spendable_quote_in)
                && u64_at(401) == Some(a.min_base_amount_out)
                && u64_at(24) == Some(gross_output)
        },
        Ok(Sell(a) | SellV2(a)) => input == a.base_amount_in && gross_output >= a.min_quote_amount_out,
        _ => false,
    }
}

fn is_trade_instruction(instruction: &InstructionView) -> bool {
    matches!(
        unpack_trade_instruction(instruction.data()),
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::BuyV2(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteInV2(_))
            | Ok(pumpswap::instructions::PumpSwapInstruction::SellV2(_))
    )
}

fn decode_trade_instruction(instruction: &InstructionView) -> Option<PendingTrade> {
    match unpack_trade_instruction(instruction.data()) {
        Ok(pumpswap::instructions::PumpSwapInstruction::Buy(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteIn(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::Sell(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyV2(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::BuyExactQuoteInV2(_))
        | Ok(pumpswap::instructions::PumpSwapInstruction::SellV2(_)) => {
            // IDL-canonical account layout — see the comment + IDX_* constants
            // in `substreams_solana_idls::pumpswap::accounts`. Hand-indexing
            // `accounts.get(N)` here previously caused an off-by-one
            // (`base_mint` = accounts[4], should have been [3]) which let the
            // user's *base token account* leak into the `mint` slot, producing
            // ~165 spurious `(mint0, mint1)` pairs per pool downstream.
            let accounts = TradeTokenAccounts::try_from(instruction).ok()?;
            Some(PendingTrade {
                pool: accounts.pool.to_bytes().to_vec(),
                base_mint: accounts.base_mint.to_bytes().to_vec(),
                quote_mint: accounts.quote_mint.to_bytes().to_vec(),
                user: accounts.user.to_bytes().to_vec(),
                is_buy: !instruction.data().starts_with(&pumpswap::instructions::SELL) && !instruction.data().starts_with(&pumpswap::instructions::SELL_V2),
                source_index: 0,
                depth: 0,
                event_authority: instruction.accounts().get(15).map(|a| a.0.clone()).unwrap_or_default(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, Message, MessageHeader, Transaction, TransactionStatusMeta,
    };

    #[test]
    fn finalized_boost_burns_bind_actual_quote_debit_and_destroyed_base() {
        let fixtures:&[(&[u8],usize)]=&[
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/0.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/1.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/2.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/3.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/4.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/5.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/6.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/7.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/8.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/9.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/10.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/11.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/12.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/13.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/14.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/15.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/16.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/17.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/18.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/19.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/20.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/21.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/22.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/23.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/24.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/25.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/26.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/27.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/28.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/29.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/30.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/31.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/32.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/33.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/34.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/35.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/36.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/37.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/38.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/39.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/40.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/41.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/42.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/43.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/44.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/45.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/46.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/47.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/48.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/49.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/50.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/51.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/52.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/53.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/54.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/55.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/56.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/57.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/58.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/59.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/60.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/61.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/62.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/63.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/64.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/65.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/66.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/67.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/68.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/69.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/70.transaction.pb").as_slice(),2),
(include_bytes!("../fixtures/swap-coverage/pumpswap-boost-burn27/71.transaction.pb").as_slice(),2),
        ];
        for &(bytes,source) in fixtures {
            let tx=substreams::proto::decode::<substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let rows=decode_boost_burns(&tx);
            assert_eq!(rows.len(),1,"source{source}");
            assert_eq!(rows[0].burn_index,(source+2)as u32);
            let walk:Vec<_>=tx.walk_instructions().collect();let a=walk[source].accounts();let native=walk[source].data().clone();
            let quote=walk[source+1].data().clone();let burn=walk[source+2].data().clone();let buy_event=walk[source+3].data().clone();let event=walk[source+4].data().clone();
            let r=&rows[0].swap;
            assert_eq!((r.protocol,r.source_index,r.source_transfer_index,r.source_burn_index,r.amm_pool.clone(),r.user.clone(),r.input_mint.clone(),r.input_amount,r.output_mint.clone(),r.output_amount),
                (pb::Protocol::PumpfunAmmBuyBurn.into(),Some(source as u32),Some((source+1)as u32),Some((source+2)as u32),a[0].0.clone(),a[7].0.clone(),a[4].0.clone(),u64::from_le_bytes(quote[1..9].try_into().unwrap()),a[3].0.clone(),u64::from_le_bytes(burn[1..9].try_into().unwrap())));
            assert!(r.burn_output_verified && !r.transfer_verified && !r.orderbook_fill_verified);
            let processed=crate::process_transaction(tx.clone()).expect("verified burn transaction");
            let published:Vec<_>=processed.swaps.iter().filter(|s|s.source_index==Some(source as u32)).collect();
            assert_eq!(published.len(),1,"one economic burn row");
            assert_eq!(published[0],r,"published tuple preserves the guarded burn proof");
            for mode in 0..29 {
                let mut wrong=tx.clone();
                if mode<5 {
                    let root=wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().find(|x|x.data==native).unwrap();
                    match mode {0=>root.data[8..16].copy_from_slice(&(r.input_amount-1).to_le_bytes()),1=>root.data[16..24].copy_from_slice(&(r.output_amount+1).to_le_bytes()),2=>root.data.push(0),3=>root.accounts[0]=root.accounts[1],_=>root.accounts[7]=root.accounts[1]}
                } else if mode==23 {
                    let index=tx.resolved_accounts().iter().position(|k|*k==a[5].0).unwrap() as u32;
                    wrong.meta.as_option_mut().unwrap().post_token_balances.iter_mut().find(|b|b.account_index==index).unwrap().ui_token_amount.as_option_mut().unwrap().amount="1".into();
                } else if mode==24 {
                    wrong.meta.as_option_mut().unwrap().post_balances[0]+=1;
                } else if mode==25 {
                    let logs=&mut wrong.meta.as_option_mut().unwrap().log_messages;
                    let index=logs.iter().rposition(|l|l==&format!("Program {} success",substreams_solana::base58::encode(&pumpswap::PROGRAM_ID))).unwrap();
                    logs[index]=format!("Program {} failed: custom program error: 1",substreams_solana::base58::encode(&pumpswap::PROGRAM_ID));
                } else {
                    let data=if matches!(mode,6..=9){&quote}else if matches!(mode,5|10..=13){&burn}else if matches!(mode,14..=18|27){&buy_event}else{&event};
                    let call=wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().flat_map(|g|&mut g.instructions).find(|x|&x.data==data).unwrap();
                    match mode {5=>call.program_id_index^=1,6=>call.data[1]^=1,7=>call.accounts[0]=call.accounts[2],8=>call.accounts[2]=call.accounts[0],9=>call.accounts[3]=call.accounts[2],
                        10=>call.data[1]^=1,11=>call.accounts[0]=call.accounts[1],12=>call.accounts[1]=call.accounts[0],13=>call.accounts[2]=call.accounts[1],
                        14=>call.data[24]^=1,15=>call.data[128]^=1,16=>call.data[160]^=1,17=>call.data[32]^=1,18=>call.data[401]^=1,
                        19=>call.data[24]^=1,20=>call.data[88]^=1,21=>call.data[160]^=1,22=>call.data[168]^=1,26=>call.accounts[0]^=1,27=>call.data[104]=1,_=>call.data[176]^=1}
                }
                assert!(decode_boost_burns(&wrong).is_empty(),"boost source{source} mutation{mode}");
                assert!(crate::process_transaction(wrong).map(|p|p.swaps.iter().all(|s|s.source_index!=Some(source as u32))).unwrap_or(true),"malformed burn cannot publish source{source} mutation{mode}");
            }

        }
    }

    #[test]
    fn finalized_exact_quote_budgets_preserve_all_existing_financial_tuples() {
        let fixtures: &[(&[u8], u32, u32, &str, &str, &str, u64, &str, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/0.transaction.pb").as_slice(), 7, 9, "5K5jeFhcxcPR9SHtGMt98nFfBEUi4SKNhniN1tF3VXr8", "9br5NZy37b78G8b5kzKkpzKm756SN3ToQXjcnvKfLDjw", "B77LX3hJP3Fw12vCWLu5vKQduFgPtZSQnoouVa2bXPg5", 6039430102012, "So11111111111111111111111111111111111111112", 2112103275),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/1.transaction.pb").as_slice(), 12, 13, "2uF4Xh61rDwxnG9woyxsVQP7zuA6kLFpb3NvnRQeoiSd", "BF8DSXz4oqpmgQnkytebdvb42TrWMFfcxuUebAH3vv8m", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", 175659402, "pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn", 33789116588),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/2.transaction.pb").as_slice(), 16, 18, "7XP4BadsRvFCbd9T4zZkeya3ccnfoKimWEoBrrHW8arG", "2DQqmYc5NEQcLaX629Hfz3RXPmHJjnhS8iTYaPAotS16", "XsCPL9dNWBMvFtTmwcCA5v3xWPSMEBCszbQdiLLq6aN", 152456, "cnNgFYRS26pKHstKvfzQXEDQuvuMfUNzTB2AMZypump", 8855578842),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/3.transaction.pb").as_slice(), 3, 5, "E9odDMJHXYq9Cv2XoGQBvzZeWGzUjLzah3VbJPnWsitA", "GLiCiNEnku3pQsmkVkRvnDLbfUN9BWbsrPDWLdPaADng", "So11111111111111111111111111111111111111112", 64192, "H4KUxsEgCp2yDpFvyDuGM5k6XevSKKrrKUxZ3Zr6nyJc", 11366578),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/4.transaction.pb").as_slice(), 10, 12, "kGuUgQ43XTU11qVpqWA3EjQrW9wfu2BH99cYkJamprC", "FhFxFjgCGdQ9L8Z3bdX6dxSiHjPtnD3dRWuiZwFagfxP", "So11111111111111111111111111111111111111112", 22220, "E2owferYxitVgpur2cyPmdjkgvHUHVSajB7WinKbpump", 4859620),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/5.transaction.pb").as_slice(), 10, 12, "kGuUgQ43XTU11qVpqWA3EjQrW9wfu2BH99cYkJamprC", "4CU5CBA6r4gYJkSgmnnMrH2MhMYgeKoc7zaEdVe81F8B", "So11111111111111111111111111111111111111112", 18180, "E2owferYxitVgpur2cyPmdjkgvHUHVSajB7WinKbpump", 3904513),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/6.transaction.pb").as_slice(), 10, 12, "kGuUgQ43XTU11qVpqWA3EjQrW9wfu2BH99cYkJamprC", "HCoBzBwfH9TEL2PucGzWfB5P8cDkhAXLFMbSF2iSR7Fb", "So11111111111111111111111111111111111111112", 16160, "E2owferYxitVgpur2cyPmdjkgvHUHVSajB7WinKbpump", 3451444),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/7.transaction.pb").as_slice(), 7, 9, "2fjFL253ZhEtA2d4xC6mEKZA2VfpXyBArZmLVnAMKgvN", "DqRExYenXCLq7MD4Gn8VZ3Tfo74ySGFqReu9BPMmGV62", "So11111111111111111111111111111111111111112", 12120, "RpjBhw6BcnBMoUVfb76pciChV7ciFY8d8BDDvy8pump", 1848654),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/8.transaction.pb").as_slice(), 30, 31, "5LxBZE9LLdWcovqfvxupj1DE3N5MUnnLkEbiRShXcAiS", "HY6a9Zt92PiywT3VEsfkxXhngRApEkDnavcAxRA5tsfS", "XPLTrL1ZH3tZJBdFWbbtrNjK9qoqZWfuhQJnXibbdfT", 5833790100, "5gCBK3vT6ZjHbpT4GGfRGm3GhU6SViCheUeBVQ4gpump", 163794624165),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/9.transaction.pb").as_slice(), 7, 9, "J8k1KNT1WoTcAJ9uX4Cp1tHa4HGxpz3DBt7ZyjHPqcr2", "F8fDsrFNDUGPsxteDakr4gchZkv29tgoMWTTH15JgGxq", "So11111111111111111111111111111111111111112", 171700, "RWH4aZakcP5pBCsdj2sL3XQYqYTVMmC6jLSVgvrpump", 19167766),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/10.transaction.pb").as_slice(), 10, 11, "9TFZCemBhAet4Wt8buCPZ8GsmhAYc8u3uBpoJrREW6Vt", "AyNWXhor6D98TJhMaaJfzX8hV2f93hPSbnMww3UCCHPn", "XPLTrL1ZH3tZJBdFWbbtrNjK9qoqZWfuhQJnXibbdfT", 75633820794, "CcutWygeq1zNtiU1jhP94NMawxJEy6MvDM6QMkatpump", 2058200727946),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/11.transaction.pb").as_slice(), 18, 19, "8WbBaDHJmZeCe67vGmeCX4c4iHEuCeNJfUpWpFyt9js8", "8qQ37rpLnXVCH9LhJxGRSTzjCrKY3aEbHisL1e3WXSHJ", "Xs7ZdzSHLU9ftNJsii5fCeJhoRWSC32SQGzGQtePxNu", 337848, "5vc8sktTSVC2KNaVhc1Cj54hGLnXYGeYJobtWJNR6rzQ", 131802795682),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/12.transaction.pb").as_slice(), 7, 9, "2fjFL253ZhEtA2d4xC6mEKZA2VfpXyBArZmLVnAMKgvN", "2yNDsBmohCWCuDSBfMTGnznpL8KYQSo74jECXXp1svfP", "So11111111111111111111111111111111111111112", 20200, "RpjBhw6BcnBMoUVfb76pciChV7ciFY8d8BDDvy8pump", 3046634),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/13.transaction.pb").as_slice(), 6, 8, "D2zWyUkgy7jRZgpDjBzB1KM8WnwvTq6mfG2sZbKsg7rQ", "9ngxAKZCijZNceGeSRAhJhuo7Fqkz8AigsSga3NTWsMs", "88Cba2Nr3LjZruw2bpwtH26XcCLTqRfcH3vNibCSSxjk", 49930882614, "So11111111111111111111111111111111111111112", 1508331806),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/14.transaction.pb").as_slice(), 9, 11, "AMRso8TXLzoW13SqaJMF1xBGQ8R5DRgdWVHE9Z2Xjg4w", "8SP5SLUdAdtAypGFQ7Deu4zqCJPxrgBckCpiGqurf192", "So11111111111111111111111111111111111111112", 713060, "EWmP2iGQR5hsNLPVB5z7yMNwVcwX1WnfraRosQJwpump", 73926339),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/15.transaction.pb").as_slice(), 14, 16, "DYRpz8ULr6qNTnAhnLHzQwJUcVVCskrFshnzWPAE5tMV", "HoPnEUbaf7rpF7dxucBtcisieoL9KJSuziNARgN6SfNX", "So11111111111111111111111111111111111111112", 18498375, "Hu8T4yWycHsd54gin4PnXqsk8sY6T4KekwJBT57opump", 118108006540),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/16.transaction.pb").as_slice(), 10, 12, "AE7rqEKiuSLjwKYzcNM136tkWevypUfHsr1md2GWmX97", "Aet97eFLeRV7T9zisGUDQLC6We1oAE4XEqRLWwV6u91N", "So11111111111111111111111111111111111111112", 18498375, "H1hqLeM7YWs2JFHFJWkbQAcLyPh7DWfn7aUCwtGdpump", 53273717736),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/17.transaction.pb").as_slice(), 7, 9, "3sfSGmANQdJe6HVZjFtYLxL1gbem9fs47TabzEsqv5cs", "7WeQCxuuH1kDeam522pSnzhoM8kKmYngwhXeZVwGVuiX", "So11111111111111111111111111111111111111112", 246440, "7Y4uGVEvKDMkf9tBLcQLTfV7CxEhDCVSKN3M75fKwtLd", 33885005),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/18.transaction.pb").as_slice(), 7, 9, "J8k1KNT1WoTcAJ9uX4Cp1tHa4HGxpz3DBt7ZyjHPqcr2", "CWVZ3t5bUirGUpnT5d8YfnRxw37CVjB8UQLEBk3E8dWQ", "So11111111111111111111111111111111111111112", 282800, "RWH4aZakcP5pBCsdj2sL3XQYqYTVMmC6jLSVgvrpump", 30859209),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/19.transaction.pb").as_slice(), 7, 9, "2fjFL253ZhEtA2d4xC6mEKZA2VfpXyBArZmLVnAMKgvN", "E2d33b6XdtBvnnov1FP3cKPoGaCTznYT3ibq4zjkiik9", "So11111111111111111111111111111111111111112", 12120, "RpjBhw6BcnBMoUVfb76pciChV7ciFY8d8BDDvy8pump", 1791150),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/20.transaction.pb").as_slice(), 9, 11, "3T3HEUXincDxFHS42mdoXAnrrUy7PxtWxNafRsCtTmFR", "DrxsXdJBDsUjqQn1JdUXt2SqYWWgyZkJjwYpt4V6XBnt", "So11111111111111111111111111111111111111112", 173284298, "DZLM712LtQQmT9Cejm8rxWTiH3CSSQacKkzS9CFepump", 385352219),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/21.transaction.pb").as_slice(), 7, 9, "4GUovRiQx1P55HbCZnDabGHoiLcSmXSLtYaaEKU5rvNv", "EMqrEADnSbagTS5RQt7CxxHdQXUhr32YLffWGEyL5tYE", "So11111111111111111111111111111111111111112", 290880, "DU2pf91EAxrsuWaMXfYhNGUZbScTQGHPD6yiDU1QVv3k", 32037292),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/22.transaction.pb").as_slice(), 10, 12, "7nG2hjgpHoPrsDkC69a4pzLRFe5JRD8F1SZGMsYU5WV6", "5K2pNU3xJMxgjmP4xQ182MYkaioRuxBfQxPTVxjnEHsM", "So11111111111111111111111111111111111111112", 20200, "Ce7iHN73k2yHvbunNJVr5SdVisfUAZgT8yyzhYmgpump", 3954225),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/23.transaction.pb").as_slice(), 7, 9, "81KqwKbMWhqeCm6cNjzGhvMszG1wkLAf87ntnnz1hNJc", "DcQNaByvstRZCERvb9UJHgHiQe8YX2FMdPptM86psQyz", "5PiVKtJXU8NKNP4MeKpX7vSLq9ZxTSjNjPuvqTMCpLj8", 324051246, "So11111111111111111111111111111111111111112", 337164),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/24.transaction.pb").as_slice(), 10, 12, "E94TX1RU5A3u6cVyujMEahD1tLjVCkXXywM8BoCRYMTK", "AqqqxFzMcjtCJ15ZVc596bXny9je8ptmgb4d83mN6Pdz", "So11111111111111111111111111111111111111112", 24240, "EhSBeXrbWpRm2MSsgLw9ySCrD745nsrsTNS65zLpump", 3766171),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/25.transaction.pb").as_slice(), 7, 9, "2wC9o3GaNxY959BcfCEPE8HKqpFVMPrJfH8Y7xewWsms", "4noZnjNm4toTigKuFkM54uJJEUaoVoTGqvDPSv5mJyWc", "So11111111111111111111111111111111111111112", 197960, "1337yLr5eHXLL3WfW4bdNcMMhif3PrAw2cn6JDiDpump", 34815965),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/26.transaction.pb").as_slice(), 7, 9, "4GUovRiQx1P55HbCZnDabGHoiLcSmXSLtYaaEKU5rvNv", "HBDG3jfPuZ4pym7BwonsBu97pk6MA2UHEjeaPax8PF7k", "So11111111111111111111111111111111111111112", 220180, "DU2pf91EAxrsuWaMXfYhNGUZbScTQGHPD6yiDU1QVv3k", 23733105),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/27.transaction.pb").as_slice(), 7, 9, "4GUovRiQx1P55HbCZnDabGHoiLcSmXSLtYaaEKU5rvNv", "GDZ4qMX7KphcfXgnbiwGmjs8GVyy7hgQrEdqQNzYtm3y", "So11111111111111111111111111111111111111112", 276740, "DU2pf91EAxrsuWaMXfYhNGUZbScTQGHPD6yiDU1QVv3k", 29863037),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/28.transaction.pb").as_slice(), 10, 12, "E94TX1RU5A3u6cVyujMEahD1tLjVCkXXywM8BoCRYMTK", "QKzmPw9NugkBjfrLVN31beucPmGPc9PjR4FbZDJaZSp", "So11111111111111111111111111111111111111112", 16160, "EhSBeXrbWpRm2MSsgLw9ySCrD745nsrsTNS65zLpump", 2492798),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/29.transaction.pb").as_slice(), 12, 13, "DTDNWY5o7vwEoUSd2RJy6V8KvKwmp6X9KjJZDvC73EV8", "3cQEH4qra69qhzbQHZAmSgLL36qSQ4QniPWZF67YRHYr", "So11111111111111111111111111111111111111112", 467544, "ATHUikmKHwRbEKv7pgu3h3YGdcG9rignvuwkq37Gpump", 455629713),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/30.transaction.pb").as_slice(), 7, 9, "4GUovRiQx1P55HbCZnDabGHoiLcSmXSLtYaaEKU5rvNv", "9bpgxQeXzeceppuuqdPzLrH38PjZU19sDFcTpzATWJG8", "So11111111111111111111111111111111111111112", 224220, "DU2pf91EAxrsuWaMXfYhNGUZbScTQGHPD6yiDU1QVv3k", 24092601),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/31.transaction.pb").as_slice(), 10, 12, "E94TX1RU5A3u6cVyujMEahD1tLjVCkXXywM8BoCRYMTK", "8nh31vHckqYFtswYrwEySkCosQiuHwGiMnMrSSAducEa", "So11111111111111111111111111111111111111112", 26260, "EhSBeXrbWpRm2MSsgLw9ySCrD745nsrsTNS65zLpump", 4013876),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/32.transaction.pb").as_slice(), 10, 12, "7nG2hjgpHoPrsDkC69a4pzLRFe5JRD8F1SZGMsYU5WV6", "31PuenjoExQEkFyoNBpEn5iCiksoP2Npwscum2AWBZEg", "So11111111111111111111111111111111111111112", 10100, "Ce7iHN73k2yHvbunNJVr5SdVisfUAZgT8yyzhYmgpump", 1812517),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/33.transaction.pb").as_slice(), 8, 9, "2pS7z7FrHSurdtSSvCzRMoyQRH4gUtKWKNUKs2jjY23W", "2Fg1akHpTyhoGvAUtiEsZ4enDpzjjPmd1ngapF9diPHc", "pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn", 3730570236, "E7eCZS2cq655qJ7uVx3dSHR2zK31kwNQsWS6P6Hpump", 115833619451),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/34.transaction.pb").as_slice(), 7, 9, "AE255UKGwgskdM2oAbeD3F76zz2tSG8LQqdoRiwL4ieY", "Fk5GCPnu7r4PxvDVCDZBiA8WQJ3fmqpiA7q3Awv9p619", "So11111111111111111111111111111111111111112", 161600, "Ktik2gETjeg4UUgVTqxi6ZyGusTxxrKrtvHkbihpump", 32692934),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/35.transaction.pb").as_slice(), 10, 12, "7nG2hjgpHoPrsDkC69a4pzLRFe5JRD8F1SZGMsYU5WV6", "B2dcPgHigPq3U4Qe3wmAJ3qGE17MEUw9eUbsoP1GjMtr", "So11111111111111111111111111111111111111112", 28280, "Ce7iHN73k2yHvbunNJVr5SdVisfUAZgT8yyzhYmgpump", 4792475),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/36.transaction.pb").as_slice(), 10, 12, "E94TX1RU5A3u6cVyujMEahD1tLjVCkXXywM8BoCRYMTK", "GRvQ6w2qbFhcRPxHQvBDnJQfyix8RhvzBjZaxrW38Tew", "So11111111111111111111111111111111111111112", 26260, "EhSBeXrbWpRm2MSsgLw9ySCrD745nsrsTNS65zLpump", 3874075),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/37.transaction.pb").as_slice(), 10, 12, "FbKKszgd9SdTCL97NPRvqNEW4AcZjtZ53XsB7tkS4miV", "5yFtr2muQxXpeDKWdwzWLE5BidYBtXwtsNVRyeaLcsjo", "So11111111111111111111111111111111111111112", 15028200, "FKvBk5kBa76ZEuwWoGaqcYEvRyxGTruN6NHzPrvHCqiv", 21601336516),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/38.transaction.pb").as_slice(), 10, 12, "E94TX1RU5A3u6cVyujMEahD1tLjVCkXXywM8BoCRYMTK", "9ZdWni11u7jtyaztEquhrFtBJcALtLEuzeVLQSFFFJt9", "So11111111111111111111111111111111111111112", 28280, "EhSBeXrbWpRm2MSsgLw9ySCrD745nsrsTNS65zLpump", 4143505),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/39.transaction.pb").as_slice(), 7, 9, "Fjt4WVcMVnHS5V451Lq1ZHcbxwXwMxFd92SNdE2Wodwu", "4WTNDysM2zaFMyNuFvgeT2WYPL8ZK1YL3StydDNNCsCM", "uXVXakC7XeenWbEHJzfqhjwzS6xrjJwN5s3v71woQNM", 5129605740856, "So11111111111111111111111111111111111111112", 1220544311),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/40.transaction.pb").as_slice(), 9, 11, "9GC7uNo39kfEjCswhUTbshiKuLPEprzpVoCzoM9brLRs", "6vPuT8Vvw5VhAfmLGbYgczXASgdZBUM3yWTp8KULgDjY", "So11111111111111111111111111111111111111112", 351247700, "8hqvWebUmySdwyoT2p4NPjYfX1fjgxTHkg9ycm6ypump", 50160089931),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/41.transaction.pb").as_slice(), 9, 11, "9GC7uNo39kfEjCswhUTbshiKuLPEprzpVoCzoM9brLRs", "CN9AMG1pGzKQnLRZBCtvUwVexeNoWHuJyHjtnv57dgUs", "So11111111111111111111111111111111111111112", 420160, "8hqvWebUmySdwyoT2p4NPjYfX1fjgxTHkg9ycm6ypump", 59941384),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/42.transaction.pb").as_slice(), 10, 12, "E94TX1RU5A3u6cVyujMEahD1tLjVCkXXywM8BoCRYMTK", "4wrmrbNxTCLJC6ja9Vwbt7Z2HJUcZtucG81CbyEBwxV6", "So11111111111111111111111111111111111111112", 18180, "EhSBeXrbWpRm2MSsgLw9ySCrD745nsrsTNS65zLpump", 2614118),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/43.transaction.pb").as_slice(), 9, 11, "H7SopFMt5nyi9g264QxJpQjGoSJGt13mpVDCKVoe4ShM", "9wsJEbqKxSJUmP52KykMYKjV6pqdBcm1gUJBYXKS2esB", "So11111111111111111111111111111111111111112", 286858, "5fePPYbUE2yQHD4AfKetsZzVLuz1Hu8K2mSuCBu5pump", 942578),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/44.transaction.pb").as_slice(), 14, 17, "5FdEzA3Q2o3fhfiJWYMMDsNqa2xAQBERRqgSCYH1DjzX", "8k21PguHvtyapXYBpBLWxnQ7pzKVt6CoJaZn1VsLv29z", "So11111111111111111111111111111111111111112", 1943190627, "7UBXrvqHwhnezKx7B2eG2tgTKiKy7Sf3xr4R9P72pump", 963621098595),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/45.transaction.pb").as_slice(), 9, 11, "9GC7uNo39kfEjCswhUTbshiKuLPEprzpVoCzoM9brLRs", "3SeofY6ReB2cJbfV8hfx8dsBSqFrTHkyMWGDtS1q4oP9", "So11111111111111111111111111111111111111112", 337340, "8hqvWebUmySdwyoT2p4NPjYfX1fjgxTHkg9ycm6ypump", 47157296),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/46.transaction.pb").as_slice(), 21, 23, "APNMac2fNY3X3FjbuMEBkGcxNJK9H3qmaukU4yiUvEYE", "FNpyU9nvLySGqq1tysxsvvLwbYebQWcR8yWhAZn52esw", "HLhwEfq8waa2ttszHEdNHxsxH8MQV6sqap1QN5LvWF2a", 31742761207262, "So11111111111111111111111111111111111111112", 5745036284),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/47.transaction.pb").as_slice(), 10, 12, "7nG2hjgpHoPrsDkC69a4pzLRFe5JRD8F1SZGMsYU5WV6", "FhFxFjgCGdQ9L8Z3bdX6dxSiHjPtnD3dRWuiZwFagfxP", "So11111111111111111111111111111111111111112", 26260, "Ce7iHN73k2yHvbunNJVr5SdVisfUAZgT8yyzhYmgpump", 4058257),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/48.transaction.pb").as_slice(), 10, 12, "qCm3yrYe4xSW3KHTFjdscBEAeawwgniijUrBwtc5Phu", "5eXr1aXiTMJR843Rd1Ymnwf59weFccY95xSdyUfmRaHB", "So11111111111111111111111111111111111111112", 176802472, "8CmzXiPx6hsidaGqr6jk8sE8bLYknuguRrBpeijBpump", 313275169449),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/49.transaction.pb").as_slice(), 9, 11, "Cr4f5486Vc5E6jGot8MSrWjwLrUUGg2fraXiFDzDdPXf", "7xsmD42nS8hBmbDF6GZMMFWFcspkigkqnWNPx3QwHVwM", "So11111111111111111111111111111111111111112", 110385, "FLYm6hwvTdHCeiAeHLLdmZKU6kjmDTrwRdTvRvKwpump", 1237281),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/50.transaction.pb").as_slice(), 6, 8, "8ibfGgWvURJ786haZfuaUT44MRnfXt1bWGrXKJUiNJPv", "7GbYP7VMxc4Jk3L6Ld7uirCYmEuS7LKw4jh3bhPRg7S9", "6GPV74iTCizLdRfwcp4tiPgX8uKcKSjcSqhAWfCbGyz", 2113768297880, "So11111111111111111111111111111111111111112", 1312893950),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/51.transaction.pb").as_slice(), 14, 16, "3XNhC29UssQh1Lk8XhkTApvu76zhQiMxfouJ8Royaw3r", "objU12a99hKf4hXkkG8PfNXoSVLrcVcFvtEFEtgYKUF", "Xsc9qvGR1efVDFGLrVsmkzv3qi45LTBjeUKSPmx9qEh", 4012, "A5WR25nLkB9PPLtQss9QwW6w92Q2s8quicwKZWyVfxJh", 542514946),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/52.transaction.pb").as_slice(), 6, 8, "D7CWBVDwkdjg5X8qUGzC7kw14WnZ1KDhjGc86H3ztsGe", "122KR2Zh8mVdmwQpAUtJKUhg2KByiMe8SHQMfiBaFpB", "DDub5dFXZ1i2MVuUfT9vy55nEk74ZN4w9WxxbA56LcG", 10268930648, "So11111111111111111111111111111111111111112", 1753005845),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/53.transaction.pb").as_slice(), 7, 9, "CdiWzUaiKqjYkeg5pdC8cp1gzXcysLpR5fo3GeRgLNMb", "ErvexocsLo8ofC2d4wW8AA4AV2Q6Th3gx3jxt9WTuufC", "EXs3rY8aSdX4TeLiMXk1JHEvJAiUiapuX9E9KmVeWmY4", 571222909098, "So11111111111111111111111111111111111111112", 1200226302),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/54.transaction.pb").as_slice(), 10, 12, "E94TX1RU5A3u6cVyujMEahD1tLjVCkXXywM8BoCRYMTK", "GT8iLXX5RQ2XNDVZ6oPMqe33u4kY2bcCh2doVWrqxnUb", "So11111111111111111111111111111111111111112", 24240, "EhSBeXrbWpRm2MSsgLw9ySCrD745nsrsTNS65zLpump", 3298001),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/55.transaction.pb").as_slice(), 10, 12, "7nG2hjgpHoPrsDkC69a4pzLRFe5JRD8F1SZGMsYU5WV6", "4e3c7ayMq1UQgrDu7yiRpr9nAM3BgfGNtsc7KeGniwzh", "So11111111111111111111111111111111111111112", 22220, "Ce7iHN73k2yHvbunNJVr5SdVisfUAZgT8yyzhYmgpump", 3117595),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/56.transaction.pb").as_slice(), 6, 7, "4sodzjFtwVMYVYEEURHrR4eYdj4fDvcbwWYRR7rJuCQA", "3JX2aEPNU4CMFkzwtKUKP2qb33yqWwzPcVFy7p9onXTN", "So11111111111111111111111111111111111111112", 226615136, "Ft87tocCrRNbeoWh9noaNJxXbjvN5K289kJJws8fpump", 480095983918),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/57.transaction.pb").as_slice(), 7, 9, "AE255UKGwgskdM2oAbeD3F76zz2tSG8LQqdoRiwL4ieY", "AUHtcVBE5dTtWNYAuECc3xMhshnrGTmheq2MQ7rebusF", "So11111111111111111111111111111111111111112", 163620, "Ktik2gETjeg4UUgVTqxi6ZyGusTxxrKrtvHkbihpump", 29204598),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-budget-rounding26/58.transaction.pb").as_slice(), 9, 11, "GXLKcTihFWVEU5EhzkJipBBpnKafeLVzLJzJFaWHxF5B", "5qeW7Dm8zhBDzaaepihtNxj4Su6MSFXfVid35gxNN6tK", "So11111111111111111111111111111111111111112", 661576, "3Ap7DQ1Js5PjEHFhY5Rr8Ng7nF5j9yKr5GUVn1cBpump", 31567384),
        ];
        for &(bytes, source, first, pool, user, input_mint, input, output_mint, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let native_data = tx.walk_instructions().nth(source as usize).unwrap().data().clone();
            let event_data = tx.walk_instructions().skip(source as usize + 1)
                .find(|ix| ix.program_id().0 == &pumpswap::PROGRAM_ID && ix.data().starts_with(&[0xe4,0x45,0xa5,0x2e,0x51,0xcb,0x9a,0x1d,0x67,0xf4,0x52,0x1f,0x2c,0xf5,0x77,0x77])).unwrap().data().clone();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(source)).collect();
            assert_eq!(swaps.len(), 1, "native source {source}");
            let s = swaps[0];
            assert_eq!((s.source_transfer_index, s.input_amount, s.output_amount), (Some(first), input, output));
            for (actual, expected) in [(&s.amm_pool,pool),(&s.user,user),(&s.input_mint,input_mint),(&s.output_mint,output_mint)] {
                assert_eq!(actual.as_slice(), substreams_solana::base58::decode(expected).unwrap().as_slice());
            }
            for mode in 0..9 {
                let mut altered = tx.clone();
                let meta = altered.meta.as_option_mut().unwrap();
                let native_is_root = altered.transaction.as_option().unwrap().message.as_option().unwrap().instructions.iter().any(|ix| ix.data == native_data);
                if mode < 3 {
                    let data = if native_is_root {
                        &mut altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions.iter_mut().find(|ix| ix.data == native_data).unwrap().data
                    } else { &mut meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions).find(|ix| ix.data == native_data).unwrap().data };
                    match mode {
                        0 => data[8] ^= 1,
                        1 => data[8..16].copy_from_slice(&(input - 1).to_le_bytes()),
                        2 => data[16..24].copy_from_slice(&u64::MAX.to_le_bytes()),
                        _ => unreachable!(),
                    }
                } else {
                    let ix = meta.inner_instructions.iter_mut().flat_map(|g| &mut g.instructions).find(|ix| ix.data == event_data).unwrap();
                    match mode {
                        3 => ix.data[32] ^= 1,
                        4 => ix.data[401] ^= 1,
                        5 => ix.data[413] ^= 1,
                        6 => ix.data[128] ^= 1,
                        7 => ix.data[160] ^= 1,
                        8 => ix.data[24] ^= 1,
                        _ => unreachable!(),
                    }
                }
                assert!(crate::process_transaction(altered).map_or(true, |d| d.swaps.iter().all(|s| s.source_index != Some(source))),
                    "exact quote budget/event mutation {mode}, source {source}");
            }
        }
    }

    #[test]
    fn finalized_bounded_padding_and_v2_sell_keep_native_arguments() {
        let fixtures: &[(&[u8], u32, u32, u64, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pump-live-bounded/0.transaction.pb"), 6, 8, 266439947, 1104296149),
            (include_bytes!("../fixtures/swap-coverage/pump-live-bounded/1.transaction.pb"), 18, 19, 178497715, 748153),
        ];
        for &(bytes, source, first, input, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let native_data = tx.walk_instructions().nth(source as usize).unwrap().data().clone();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(source)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!((swaps[0].source_transfer_index, swaps[0].input_amount, swaps[0].output_amount), (Some(first), input, output));
            for mode in 0..6 {
                let mut altered = tx.clone();
                let data = &mut altered.meta.as_option_mut().unwrap().inner_instructions.iter_mut()
                    .flat_map(|g| &mut g.instructions).find(|ix| ix.data == native_data).unwrap().data;
                match mode {
                    0 => data[24] = 2,
                    1 => data.push(0),
                    2 => data.truncate(23),
                    3 => data[8] ^= 1,
                    4 => data[16..24].copy_from_slice(&u64::MAX.to_le_bytes()),
                    5 => { data[0] ^= 1; },
                    _ => unreachable!(),
                }
                assert!(crate::process_transaction(altered).map_or(true, |d| d.swaps.iter().all(|s| s.source_index != Some(source))),
                    "bounded native profile mutation {mode}");
            }
        }
    }

    #[test]
    fn repeated_native_buys_keep_distinct_committed_sources() {
        let fixtures: &[(&[u8], u32, u32, u64, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/0.transaction.pb").as_slice(), 13, 15, 100000, 1695196),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/1.transaction.pb").as_slice(), 13, 15, 100000, 1695196),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/2.transaction.pb").as_slice(), 13, 15, 100000, 1695196),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/3.transaction.pb").as_slice(), 13, 15, 100000, 1695196),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/4.transaction.pb").as_slice(), 13, 15, 100000, 1695196),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/5.transaction.pb").as_slice(), 13, 15, 100000, 1695368),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/6.transaction.pb").as_slice(), 13, 15, 100000, 1695368),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/7.transaction.pb").as_slice(), 13, 15, 100000, 1695197),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/8.transaction.pb").as_slice(), 13, 15, 100000, 1695197),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/9.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/10.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/11.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/12.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/13.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/14.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/15.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/16.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/17.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/18.transaction.pb").as_slice(), 13, 15, 100000, 1695066),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/19.transaction.pb").as_slice(), 13, 15, 100000, 1696485),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/20.transaction.pb").as_slice(), 13, 15, 100000, 1696485),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/21.transaction.pb").as_slice(), 13, 15, 100000, 1699510),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/22.transaction.pb").as_slice(), 13, 15, 100000, 1699510),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/23.transaction.pb").as_slice(), 13, 15, 100000, 1701164),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/24.transaction.pb").as_slice(), 13, 15, 100000, 1701140),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/25.transaction.pb").as_slice(), 13, 15, 100000, 1701140),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/26.transaction.pb").as_slice(), 13, 15, 100000, 1701140),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/27.transaction.pb").as_slice(), 13, 15, 100000, 1701140),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/28.transaction.pb").as_slice(), 13, 15, 100000, 1701140),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-repeated-live/29.transaction.pb").as_slice(), 13, 15, 100000, 1701140),
        ];
        for &(bytes, source, first, input, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let buys: Vec<_> = decoded.swaps.iter().filter(|s| s.protocol == pb::Protocol::PumpfunAmm).collect();
            assert_eq!(buys.len(), 2);
            let restored: Vec<_> = buys.iter().filter(|s| s.source_index == Some(source)).collect();
            assert_eq!(restored.len(), 1);
            assert_eq!((restored[0].source_transfer_index, restored[0].input_amount, restored[0].output_amount), (Some(first), input, output));
            assert!(restored[0].transfer_verified);
        }
        let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixtures[0].0.to_vec()).unwrap();
        // Alter only the second event; it must never be attached to the first
        // otherwise-identical native buy or accepted with another event's user.
        for mode in 0..5 {
            let mut altered = tx.clone();
            let event = &mut altered.meta.as_option_mut().unwrap().inner_instructions.iter_mut()
                .flat_map(|g| &mut g.instructions).filter(|ix| ix.data.starts_with(&[0xe4,0x45,0xa5,0x2e,0x51,0xcb,0x9a,0x1d])).nth(1).unwrap();
            match mode {
                0 => event.stack_height = Some(1),
                1 => event.accounts[0] ^= 1,
                2 => event.data[128] ^= 1, // pool
                3 => event.data[160] ^= 1, // user
                4 => event.data[8..16].copy_from_slice(&pumpswap::events::SELL_EVENT),
                _ => unreachable!(),
            }
            let mut pending = None;
            let candidates: Vec<_> = altered.walk_instructions().enumerate()
                .filter_map(|(index, ix)| handle_instruction(&mut pending, &ix, index as u32)).collect();
            assert_eq!(candidates.len(), 1, "event source mutation {mode}");
            assert_eq!(candidates[0].source_index, Some(2));
        }
    }

    #[test]
    fn finalized_sell_boolean_suffixes_preserve_exact_principal_receipts() {
        let fixtures: &[(&[u8], u32, u32, u64, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/0.transaction.pb").as_slice(), 6, 8, 1054011869465, 738636214),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/1.transaction.pb").as_slice(), 1, 3, 30000, 35368891),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/2.transaction.pb").as_slice(), 4, 6, 3526053355674, 1398305543),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/3.transaction.pb").as_slice(), 3, 5, 414776809974, 724043666),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/4.transaction.pb").as_slice(), 5, 7, 30000, 328177372),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/5.transaction.pb").as_slice(), 5, 7, 30000, 65169439),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/6.transaction.pb").as_slice(), 5, 7, 30000, 335755796),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/7.transaction.pb").as_slice(), 4, 6, 197072404302, 62942423),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/8.transaction.pb").as_slice(), 1, 3, 30000, 75798865),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/9.transaction.pb").as_slice(), 7, 9, 980187577377, 584456716),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/10.transaction.pb").as_slice(), 6, 8, 951168762158, 783003378),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/11.transaction.pb").as_slice(), 1, 3, 30000, 75831409),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/12.transaction.pb").as_slice(), 4, 6, 1060087878201, 482014443),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/13.transaction.pb").as_slice(), 7, 9, 984687619076, 595460134),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/14.transaction.pb").as_slice(), 6, 8, 950289115848, 718834932),
            (include_bytes!("../fixtures/swap-coverage/pumpswap-sell-flags/15.transaction.pb").as_slice(), 6, 8, 873156127824, 736846889),
        ];
        for &(bytes, source, first, input, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let ix = tx.walk_instructions().nth(source as usize).unwrap();
            let original_data = ix.data().clone();
            let a = ix.accounts();
            let (pool, user, input_mint, output_mint) = (a[0].0.clone(), a[1].0.clone(), a[3].0.clone(), a[4].0.clone());
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(source)).collect();
            assert_eq!(swaps.len(), 1, "one physical sell with separately paid fees");
            let sell = swaps[0];
            assert_eq!((sell.input_amount, sell.output_amount, sell.source_transfer_index), (input, output, Some(first)));
            assert_eq!((sell.amm_pool.clone(), sell.user.clone(), sell.input_mint.clone(), sell.output_mint.clone()),
                (pool, user, input_mint, output_mint));
            assert_eq!(sell.protocol, pb::Protocol::PumpfunAmm);
            assert!(sell.transfer_verified);
            for mode in 0..6 {
                let mut altered = tx.clone();
                let message = altered.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap();
                let native = if let Some(position) = message.instructions.iter().position(|ix| ix.data == original_data) {
                    &mut message.instructions[position].data
                } else {
                    &mut altered.meta.as_option_mut().unwrap().inner_instructions.iter_mut()
                        .flat_map(|group| &mut group.instructions).find(|ix| ix.data == original_data).unwrap().data
                };
                match mode {
                    0 => native[24] = 2,
                    1 => { native.resize(26, 0); native[25] = 2; }
                    2 => native.resize(27, 0),
                    3 => native.truncate(23),
                    4 => native[0] ^= 1,
                    5 => {
                        let program = substreams_solana::base58::encode(pumpswap::PROGRAM_ID);
                        for log in &mut altered.meta.as_option_mut().unwrap().log_messages {
                            if log.as_str() == format!("Program {program} success") {
                                *log = format!("Program {program} failed: custom program error: 0x1");
                            }
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(crate::process_transaction(altered).map_or(true, |decoded|
                    decoded.swaps.iter().all(|s| s.source_index != Some(source))), "sell suffix mutation {mode}");
            }
        }
    }

    /// PumpSwap `buy` instruction discriminator from `pumpswap/idl.json`.
    const BUY_DISC: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
    /// PumpSwap `sell` instruction discriminator.
    const SELL_DISC: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

    fn buy_body() -> Vec<u8> {
        // BuyInstruction { base_amount_out: u64, max_quote_amount_in: u64 }
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b
    }

    fn sell_body() -> Vec<u8> {
        // SellInstruction { base_amount_in: u64, min_quote_amount_out: u64 }
        let mut b = Vec::new();
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b
    }

    /// Build a ConfirmedTransaction with one PumpSwap instruction.
    fn make_tx(disc: [u8; 8], body: Vec<u8>, accounts: &[[u8; 32]]) -> ConfirmedTransaction {
        let fee_payer = [0xfe; 32];
        let program = pumpswap::PROGRAM_ID;
        let mut keys: Vec<Vec<u8>> = vec![fee_payer.to_vec(), program.to_vec()];
        let mut acc_idx: Vec<u8> = Vec::new();
        for (i, a) in accounts.iter().enumerate() {
            keys.push(a.to_vec());
            acc_idx.push((i + 2) as u8);
        }
        let mut data = disc.to_vec();
        data.extend_from_slice(&body);

        ConfirmedTransaction {
            transaction: buffa::MessageField::some(Transaction {
                signatures: vec![vec![0u8; 64]],
                message: buffa::MessageField::some(Message {
                    header: buffa::MessageField::some(MessageHeader {
                        num_required_signatures: 1,
                        num_readonly_signed_accounts: 0,
                        num_readonly_unsigned_accounts: 0,
                    })
                    .into(),
                    account_keys: keys,
                    recent_blockhash: vec![0u8; 32],
                    instructions: vec![CompiledInstruction {
                        program_id_index: 1,
                        accounts: acc_idx,
                        data,
                    }],
                    versioned: false,
                    address_table_lookups: vec![],
                    ..Default::default()
                })
                .into(),
            })
            .into(),
            meta: buffa::MessageField::some(TransactionStatusMeta::default()).into(),
        }
    }

    #[test]
    fn buy_decode_resolves_canonical_mints() {
        let pool = [0x01; 32];
        let user = [0x02; 32];
        let global_config = [0x03; 32];
        let base_mint = [0x04; 32];
        let quote_mint = [0x05; 32];
        let user_base_ta = [0x06; 32];
        let user_quote_ta = [0x07; 32];
        let pool_base_ta = [0x08; 32];
        let pool_quote_ta = [0x09; 32];
        let protocol_fee_recipient = [0x0a; 32];
        let protocol_fee_recipient_ta = [0x0b; 32];

        let tx = make_tx(
            BUY_DISC,
            buy_body(),
            &[
                pool,
                user,
                global_config,
                base_mint,
                quote_mint,
                user_base_ta,
                user_quote_ta,
                pool_base_ta,
                pool_quote_ta,
                protocol_fee_recipient,
                protocol_fee_recipient_ta,
            ],
        );
        let ix = tx.walk_instructions().next().unwrap();
        let trade = decode_trade_instruction(&ix).expect("buy instruction must decode");

        assert_eq!(trade.pool, pool.to_vec());
        assert_eq!(trade.base_mint, base_mint.to_vec(), "base_mint must come from accounts[3], not [4]");
        assert_eq!(trade.quote_mint, quote_mint.to_vec(), "quote_mint must come from accounts[4], not [5]");

        // Regression: the previous adapter read accounts[4]/[5] which is
        // exactly (quote_mint, user_base_token_account). Make sure neither
        // slot leaks the user's TA back in.
        assert_ne!(trade.base_mint, user_base_ta.to_vec());
        assert_ne!(trade.quote_mint, user_base_ta.to_vec());
    }

    #[test]
    fn sell_decode_uses_same_layout_as_buy() {
        let pool = [0x21; 32];
        let user = [0x22; 32];
        let global_config = [0x23; 32];
        let base_mint = [0x24; 32];
        let quote_mint = [0x25; 32];
        let accounts: Vec<[u8; 32]> = vec![pool, user, global_config, base_mint, quote_mint]
            .into_iter()
            .chain((6u8..12u8).map(|i| [i; 32]))
            .collect();

        let tx = make_tx(SELL_DISC, sell_body(), &accounts);
        let ix = tx.walk_instructions().next().unwrap();
        let trade = decode_trade_instruction(&ix).expect("sell instruction must decode");

        assert_eq!(trade.pool, pool.to_vec());
        assert_eq!(trade.base_mint, base_mint.to_vec());
        assert_eq!(trade.quote_mint, quote_mint.to_vec());
    }
}
