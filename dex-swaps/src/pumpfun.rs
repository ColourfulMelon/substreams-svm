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
        protocol: pb::Protocol::Pumpfun as i32,
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
    const TITAN: [u8;32]=substreams_solana::b58!("T1TANpTeScyeqVzzgNViGDNrkQ6qHz9KrSBS4aNXvGT");
    let parent=source.checked_sub(2)?;
    let root=instructions.get(parent)?; let r=root.accounts(); let d=root.data();
    let native=instructions.get(source)?;let a=native.accounts();
    if !root.is_root() || root.program_id().0.as_slice()!=TITAN || r.len()!=44 || d.len()!=37
        || d[..2]!=[42,2] || d[18..]!=[50,0,2,164,31,0,1,0,0,0,37,0,0,1,0,202,154,59,27]
        || net==0 || native.is_root() || native.stack_height()!=2 || a.len()!=26
        || native.program_id().0.as_slice()!=pumpfun::PROGRAM_ID || a[2].0.as_slice()!=SOL_MINT
        || !matches!(unpack_trade_instruction(native.data()),Ok(pumpfun::instructions::PumpFunInstruction::SellV2(_)))
    {return None;}
    let atlas=solana_pubkey::Pubkey::find_program_address(&[b"atlas"],&solana_pubkey::Pubkey::new_from_array(TITAN)).0.to_bytes();
    if a[13].0.as_slice()!=atlas || r[2].0.as_slice()!=atlas || r[0].0!=r[1].0 || r[0].0==a[13].0
        || r[12].0!=a[14].0 || r[13].0!=a[15].0 || r[14].0!=a[1].0 || r[15].0.as_slice()!=SOL_MINT
        || r[5].0.as_slice()!=token::PROGRAM_ID || r[6].0.as_slice()!=token_2022::PROGRAM_ID
        || r[7].0.iter().any(|v|*v!=0) || r[8].0.as_slice()!=substreams_solana::b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
        || r[9].0.as_slice()!=TITAN {return None;}
    let calls=instructions.get(parent+1..parent+17)?;
    if instructions.get(parent+17).is_some_and(|ix|!ix.is_root()) || !successful.contains(&(parent as u32))
        || calls.iter().enumerate().any(|(n,ix)|ix.is_root() || ix.stack_height()!=[2,2,3,3,3,2,2,2,2,2,3,3,3,3,2,2][n]
            || !successful.contains(&((parent+1+n)as u32))) {return None;}
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
    let meta=tx.meta.as_ref()?;let keys=tx.resolved_accounts();let quote=keys.iter().position(|k|*k==a[15].0)?;let owner=keys.iter().position(|k|*k==a[13].0)?;
    let before=*meta.pre_balances.get(quote)?;if meta.post_balances.get(quote)!=Some(&before) || meta.pre_balances.get(owner)!=meta.post_balances.get(owner) {return None;}
    let pre=meta.pre_token_balances.iter().find(|t|t.account_index==quote as u32)?;let post=meta.post_token_balances.iter().find(|t|t.account_index==quote as u32)?;
    if pre.mint!=substreams_solana::base58::encode(SOL_MINT) || post.mint!=pre.mint || pre.owner!=substreams_solana::base58::encode(&atlas)
        || post.owner!=pre.owner || pre.ui_token_amount.as_ref()?.amount!=post.ui_token_amount.as_ref()?.amount {return None;}
    let saved=pre.ui_token_amount.as_ref()?.amount.parse::<u64>().ok()?;
    let rent=u64::from_le_bytes(create.data()[4..12].try_into().ok()?);let restore_amount=u64::from_le_bytes(restore.data()[4..12].try_into().ok()?);
    if restore_amount!=saved || rent.checked_add(saved)?!=before {return None;}
    let fee=u64::from_le_bytes(route_fee.data()[1..9].try_into().ok()?);if fee>net || net.checked_sub(fee)?<u64::from_le_bytes(d[10..18].try_into().ok()?) {return None;}
    Some(TitanNativeSettlement{parent,close:parent+9,atlas:atlas.to_vec(),funder:r[0].0.clone(),quote:a[15].0.clone(),refund:before.checked_add(net)?.checked_sub(fee)?})
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::{
        CompiledInstruction, ConfirmedTransaction, InnerInstruction, InnerInstructions, Message, MessageHeader, Transaction, TransactionStatusMeta,
    };

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
            if mode<5 {let ix=&mut wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[2];let a=ix.accounts.clone();match mode {
                0=>ix.data.push(0),1=>ix.data[18]^=1,2=>ix.data[2]^=1,3=>ix.accounts[2]=a[0],_=>ix.data[10..18].copy_from_slice(&1103178u64.to_le_bytes())
            }}else if mode<17 {let g=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g|g.index==2).unwrap();let (n,kind)=match mode{5=>(5,0),6=>(5,1),7=>(5,2),8=>(3,1),9=>(1,1),10=>(8,1),11=>(11,0),12=>(11,1),13=>(14,0),14=>(13,0),15=>(9,1),_=>(6,2)};let ix=&mut g.instructions[n];let a=ix.accounts.clone();match kind{
                0=>ix.data[if mode==14{1}else{4}]^=1,1=>ix.accounts[if mode==8{3}else if mode==9{13}else if mode==15{2}else{1}]=a[0],_=>ix.stack_height=Some(99)
            }}else if mode==17{wrong.meta.as_mut().unwrap().log_messages.clear();}else if mode==18{wrong.meta.as_mut().unwrap().post_balances[0]+=1;}else{wrong.meta.as_mut().unwrap().err=Some(Default::default());}
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
                let call = altered.meta.as_mut().unwrap().inner_instructions.iter_mut().flat_map(|g| &mut g.instructions)
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
            let data = &mut altered.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions.iter_mut()
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
                transaction: Some(Transaction {
                    message: Some(Message {
                        header: Some(MessageHeader {
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
                meta: Some(TransactionStatusMeta { inner_instructions: vec![InnerInstructions { index: 0, instructions: vec![InnerInstruction { program_id_index: 12, accounts: vec![10], data: event, stack_height: Some(2) }] }], ..Default::default() }),
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
