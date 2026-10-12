use std::collections::VecDeque;

use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::meteora::dlmm;

pub(crate) struct PendingSwap {
    native_index: Option<u32>,
    native_depth: u32,
    pub(crate) user: Vec<u8>,
    pub(crate) lb_pair: Vec<u8>,
    pub(crate) token_x_mint: Vec<u8>,
    pub(crate) token_y_mint: Vec<u8>,
    pub(crate) user_token_in: Vec<u8>,
    pub(crate) user_token_out: Vec<u8>,
    pub(crate) reserve_x: Vec<u8>,
    pub(crate) reserve_y: Vec<u8>,
    pub(crate) token_x_program: Vec<u8>,
    pub(crate) token_y_program: Vec<u8>,
    pub(crate) host_fee_account: Option<Vec<u8>>,
    event_authority: Vec<u8>,
}

struct SwapEvent {
    lb_pair: Vec<u8>,
    user: Vec<u8>,
    amount_in: u64,
    amount_out: u64,
    swap_for_y: bool,
}

/// Match the event's own pool. A missing instruction/event must never shift
/// a later event onto another pool's mint pair.
pub(crate) fn handle_instruction(pending: &mut VecDeque<PendingSwap>, instruction: &InstructionView, index: u32) -> Option<pb::Swap> {
    let depth = if instruction.is_root() { 1 } else { instruction.stack_height() };
    // An invocation ends before the next instruction at its own depth or a
    // shallower depth. Missing events never leak onto a later same-pool call.
    pending.retain(|swap| swap.native_depth < depth);
    let program_id = instruction.program_id().0;
    if program_id != &dlmm::PROGRAM_ID {
        return None;
    }

    if let Some(mut swap) = decode_swap_instruction(instruction) {
        swap.native_index = Some(index);
        pending.push_back(swap);
        return None;
    }

    let event = decode_swap_event(instruction)?;
    let position = pending
        .iter()
        .position(|swap| swap.lb_pair == event.lb_pair && swap.user == event.user && swap.native_depth + 1 == depth)?;
    let swap = pending.remove(position)?;
    let (input_mint, output_mint) = if event.swap_for_y {
        (swap.token_x_mint.clone(), swap.token_y_mint.clone())
    } else {
        (swap.token_y_mint.clone(), swap.token_x_mint.clone())
    };

    Some(pb::Swap {
        protocol: pb::Protocol::MeteoraDlmm as i32,
        program_id: dlmm::PROGRAM_ID.to_vec(),
        stack_height: instruction.stack_height(),
        amm: dlmm::PROGRAM_ID.to_vec(),
        amm_pool: swap.lb_pair,
        user: swap.user,
        input_mint,
        input_amount: event.amount_in,
        output_mint,
        output_amount: event.amount_out,
        source_index: swap.native_index,
        ..Default::default()
    })
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    if ix.program_id().0 != &dlmm::PROGRAM_ID {
        return None;
    }
    decode_swap_instruction(ix).map(|s| s.lb_pair)
}

pub(crate) fn decode_swap_instruction(ix: &InstructionView) -> Option<PendingSwap> {
    // Meteora DLMM ships six swap-flavour instructions. Each carries the same
    // (lb_pair, user, token_x_mint, token_y_mint) in different positions of
    // its account list and emits the same anchor CPI `Swap` event with a
    // `swap_for_y` direction flag — handled uniformly by `decode_swap_event`.
    //
    // Pre-v0.5.0 we matched only the original `Swap` instruction, silently
    // dropping ~5/6 of post-launch DLMM volume routed through the variants
    // below.
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    // Bounded committed Swap2 tails follow the declared empty remaining
    // vector. Keep exactly the canonical28B prefix for the typed ABI parser.
    let mut data = ix.data().clone();
    if data.starts_with(&dlmm::instructions::SWAP2)
        && data.len() >= 28 && data[24..28].iter().all(|byte| *byte == 0)
        && ((matches!(data.len(), 32 | 56) && data[28..].iter().all(|byte| *byte == 0))
            || (data.len() == 34 && data[28..] == [0, 0, 0, 0, 4, 0]))
    { data.truncate(28); }
    match dlmm::instructions::unpack(&data) {
        Ok(dlmm::instructions::MeteoraDlmmInstruction::Swap(_)) => {
            let a = dlmm::accounts::get_swap_accounts(ix).ok()?;
            Some(PendingSwap {
                native_index: None,
                native_depth: depth,
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
                user_token_in: a.user_token_in.to_bytes().to_vec(),
                user_token_out: a.user_token_out.to_bytes().to_vec(),
                reserve_x: a.reserve_x.to_bytes().to_vec(),
                reserve_y: a.reserve_y.to_bytes().to_vec(),
                token_x_program: a.token_x_program.to_bytes().to_vec(),
                token_y_program: a.token_y_program.to_bytes().to_vec(),
                host_fee_account: a.host_fee_in.map(|key| key.to_bytes().to_vec()),
                event_authority: a.event_authority.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::Swap2(_)) => {
            let a = dlmm::accounts::get_swap2_accounts(ix).ok()?;
            Some(PendingSwap {
                native_index: None,
                native_depth: depth,
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
                user_token_in: a.user_token_in.to_bytes().to_vec(),
                user_token_out: a.user_token_out.to_bytes().to_vec(),
                reserve_x: a.reserve_x.to_bytes().to_vec(),
                reserve_y: a.reserve_y.to_bytes().to_vec(),
                token_x_program: a.token_x_program.to_bytes().to_vec(),
                token_y_program: a.token_y_program.to_bytes().to_vec(),
                host_fee_account: a.host_fee_in.map(|key| key.to_bytes().to_vec()),
                event_authority: a.event_authority.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapExactOut(_)) => {
            let a = dlmm::accounts::get_swap_exact_out_accounts(ix).ok()?;
            Some(PendingSwap {
                native_index: None,
                native_depth: depth,
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
                user_token_in: a.user_token_in.to_bytes().to_vec(),
                user_token_out: a.user_token_out.to_bytes().to_vec(),
                reserve_x: a.reserve_x.to_bytes().to_vec(),
                reserve_y: a.reserve_y.to_bytes().to_vec(),
                token_x_program: a.token_x_program.to_bytes().to_vec(),
                token_y_program: a.token_y_program.to_bytes().to_vec(),
                host_fee_account: a.host_fee_in.map(|key| key.to_bytes().to_vec()),
                event_authority: a.event_authority.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapExactOut2(_)) => {
            let a = dlmm::accounts::get_swap_exact_out2_accounts(ix).ok()?;
            Some(PendingSwap {
                native_index: None,
                native_depth: depth,
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
                user_token_in: a.user_token_in.to_bytes().to_vec(),
                user_token_out: a.user_token_out.to_bytes().to_vec(),
                reserve_x: a.reserve_x.to_bytes().to_vec(),
                reserve_y: a.reserve_y.to_bytes().to_vec(),
                token_x_program: a.token_x_program.to_bytes().to_vec(),
                token_y_program: a.token_y_program.to_bytes().to_vec(),
                host_fee_account: a.host_fee_in.map(|key| key.to_bytes().to_vec()),
                event_authority: a.event_authority.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapWithPriceImpact(_)) => {
            let a = dlmm::accounts::get_swap_with_price_impact_accounts(ix).ok()?;
            Some(PendingSwap {
                native_index: None,
                native_depth: depth,
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
                user_token_in: a.user_token_in.to_bytes().to_vec(),
                user_token_out: a.user_token_out.to_bytes().to_vec(),
                reserve_x: a.reserve_x.to_bytes().to_vec(),
                reserve_y: a.reserve_y.to_bytes().to_vec(),
                token_x_program: a.token_x_program.to_bytes().to_vec(),
                token_y_program: a.token_y_program.to_bytes().to_vec(),
                host_fee_account: a.host_fee_in.map(|key| key.to_bytes().to_vec()),
                event_authority: a.event_authority.to_bytes().to_vec(),
            })
        }
        Ok(dlmm::instructions::MeteoraDlmmInstruction::SwapWithPriceImpact2(_)) => {
            let a = dlmm::accounts::get_swap_with_price_impact2_accounts(ix).ok()?;
            Some(PendingSwap {
                native_index: None,
                native_depth: depth,
                user: a.user.to_bytes().to_vec(),
                lb_pair: a.lb_pair.to_bytes().to_vec(),
                token_x_mint: a.token_x_mint.to_bytes().to_vec(),
                token_y_mint: a.token_y_mint.to_bytes().to_vec(),
                user_token_in: a.user_token_in.to_bytes().to_vec(),
                user_token_out: a.user_token_out.to_bytes().to_vec(),
                reserve_x: a.reserve_x.to_bytes().to_vec(),
                reserve_y: a.reserve_y.to_bytes().to_vec(),
                token_x_program: a.token_x_program.to_bytes().to_vec(),
                token_y_program: a.token_y_program.to_bytes().to_vec(),
                host_fee_account: a.host_fee_in.map(|key| key.to_bytes().to_vec()),
                event_authority: a.event_authority.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

/// Output-fee evidence from the current canonical Swap2Evt, pinned to
/// MeteoraAg/dlmm-sdk 576919e3e4368e542c402f000b4264724f7f23ec.
/// This only identifies a separately declared host payout. Physical CPI
/// accounts, amounts, programs and successful execution are verified by callers.
pub(crate) fn output_host_fee_event(ix: &InstructionView, layout: &PendingSwap, input: u64, output_mint: &[u8]) -> Option<(u64, u64)> {
    let data = ix.data();
    if ix.program_id().0 != &dlmm::PROGRAM_ID
        || data.len() != 163
        || data[..16] != [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d, 0x2e, 0x74, 0x52, 0xd7, 0x94, 0x1b, 0x54, 0x4d]
        || ix.accounts().len() != 1
        || ix.accounts()[0].0 != &layout.event_authority
    {
        return None;
    }
    let p = &data[16..];
    if p[..32] != layout.lb_pair || p[32..64] != layout.user
        || p[72] > 1 || p[145] != 0 || p[146] > 1
        || p[146] == p[72]
        || (if p[72] == 1 { &layout.token_y_mint } else { &layout.token_x_mint }).as_slice() != output_mint
        || u64::from_le_bytes(p[89..97].try_into().ok()?) != input
        || u64::from_le_bytes(p[97..105].try_into().ok()?) != 0
    {
        return None;
    }
    let output = u64::from_le_bytes(p[105..113].try_into().ok()?);
    let host = u64::from_le_bytes(p[137..145].try_into().ok()?);
    (output > 0 && host > 0).then_some((output, host))
}

fn decode_swap_event(ix: &InstructionView) -> Option<SwapEvent> {
    match dlmm::anchor_cpi_event::unpack(ix.data()) {
        Ok(dlmm::anchor_cpi_event::MeteoraDlmmAnchorCpiEvent::Swap(event)) => Some(SwapEvent {
            lb_pair: event.lb_pair.to_bytes().to_vec(),
            user: event.from.to_bytes().to_vec(),
            amount_in: event.amount_in,
            amount_out: event.amount_out,
            swap_for_y: event.swap_for_y,
        }),
        _ => None,
    }
}

/// The padded empty-vector Swap2 forms are accepted only with the two
/// canonical events and their exact immediate principal/optional host CPIs.
/// Events distinguish host proceeds; the caller proves the user's net output.
pub(crate) fn padded_swap2_settlement(source: u32, ix: &InstructionView, calls: &[InstructionView], successful: &std::collections::HashSet<u32>) -> bool {
    let d = ix.data();
    let a = ix.accounts();
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if ix.program_id().0 != &dlmm::PROGRAM_ID || !d.starts_with(&dlmm::instructions::SWAP2)
        || !matches!(d.len(), 32 | 34 | 56) || d[24..28] != [0, 0, 0, 0] || a.len() < 16
        || a[13].0.as_slice() != substreams_solana::b58!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr")
        || a[15].0 != &dlmm::PROGRAM_ID || !matches!(calls.len(), 4 | 5) || depth == 0
        || !successful.contains(&source)
        || calls.iter().enumerate().any(|(n, c)| c.is_root() || c.stack_height() != depth + 1 || !successful.contains(&(source + n as u32 + 1)))
    { return false; }
    let Some(pending) = decode_swap_instruction(ix) else { return false };
    let Some(legacy) = decode_swap_event(&calls[0]) else { return false };
    let last = calls.last().unwrap();
    let e = last.data();
    if calls[0].accounts().len() != 1 || calls[0].accounts()[0].0 != a[14].0
        || last.program_id().0 != &dlmm::PROGRAM_ID || last.accounts().len() != 1 || last.accounts()[0].0 != a[14].0
        || e.len() != 163 || e[..16] != [228, 69, 165, 46, 81, 203, 154, 29, 46, 116, 82, 215, 148, 27, 84, 77]
    { return false; }
    let p = &e[16..];
    let requested = u64::from_le_bytes(d[8..16].try_into().unwrap());
    let minimum = u64::from_le_bytes(d[16..24].try_into().unwrap());
    let host = u64::from_le_bytes(p[137..145].try_into().unwrap());
    let output = u64::from_le_bytes(p[105..113].try_into().unwrap());
    if p[..32] != pending.lb_pair || p[32..64] != pending.user || p[72] > 1 || p[145] > 1 || p[146] > 1
        || requested == 0 || output == 0 || output < minimum
        || u64::from_le_bytes(p[89..97].try_into().unwrap()) != requested || p[97..105] != [0; 8]
        || legacy.lb_pair != pending.lb_pair || legacy.user != pending.user || legacy.amount_in != requested
        || legacy.amount_out != output || legacy.swap_for_y != (p[72] == 1)
    { return false; }
    let to_y = p[72] == 1;
    let legs = &calls[1..calls.len() - 1];
    for (n, c) in legs.iter().enumerate() {
        let host_leg = legs.len() == 3 && n == 1;
        let (roles, program, amount) = if n == 0 {
            ([4, if to_y { 6 } else { 7 }, if to_y { 2 } else { 3 }, 10], if to_y { 11 } else { 12 }, requested)
        } else if host_leg {
            let on_x = p[146] == 1;
            if host == 0 || on_x != if p[145] == 1 { to_y } else { !to_y } || a[9].0 == &dlmm::PROGRAM_ID { return false; }
            ([if on_x { 2 } else { 3 }, if on_x { 6 } else { 7 }, 9, 0], if on_x { 11 } else { 12 }, host)
        } else {
            ([if to_y { 3 } else { 2 }, if to_y { 7 } else { 6 }, 5, 0], if to_y { 12 } else { 11 }, output)
        };
        let cd = c.data();
        let ca = c.accounts();
        if c.program_id().0 != a[program].0 || ![substreams_solana_idls::spl::token::PROGRAM_ID.as_slice(), substreams_solana_idls::spl::token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
            || cd.len() != 10 || cd[0] != 12 || u64::from_le_bytes(cd[1..9].try_into().unwrap()) != amount
            || ca.len() != 4 || ca.iter().zip(roles).any(|(key, role)| key.0 != a[role].0)
        { return false; }
    }
    (legs.len() == 3 || host == 0 || a[9].0 == &dlmm::PROGRAM_ID) && output > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

    #[test]
    fn finalized_output_host_fee_aliases_preserve_primary_receipts() {
        let fixtures: &[(&[u8], u32, u32, u64, u64)] = &[
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/0.transaction.pb").as_slice(), 10, 12, 12334, 15043),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/1.transaction.pb").as_slice(), 8, 10, 33547153876, 1706546639),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/2.transaction.pb").as_slice(), 9, 11, 357822, 440843),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/3.transaction.pb").as_slice(), 11, 13, 175882556, 9142626),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/4.transaction.pb").as_slice(), 15, 17, 22702090579, 1188639022),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/5.transaction.pb").as_slice(), 6, 8, 5561727592, 6852158281),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/6.transaction.pb").as_slice(), 4, 5, 18941432, 2052503),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/7.transaction.pb").as_slice(), 3, 5, 11062019335, 587245694),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/8.transaction.pb").as_slice(), 10, 12, 20043355847, 1336159),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/9.transaction.pb").as_slice(), 5, 7, 56132830, 607386),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/10.transaction.pb").as_slice(), 4, 5, 18922620, 2052517),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/11.transaction.pb").as_slice(), 4, 5, 18903707, 2052500),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/12.transaction.pb").as_slice(), 9, 11, 1275829, 172940),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/13.transaction.pb").as_slice(), 10, 12, 208907361, 28317786),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/14.transaction.pb").as_slice(), 8, 10, 47808371, 869750),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/15.transaction.pb").as_slice(), 10, 12, 34550619279, 69166597),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/16.transaction.pb").as_slice(), 15, 16, 22420835867, 109185946),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/17.transaction.pb").as_slice(), 15, 16, 68400215738, 333098297),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/18.transaction.pb").as_slice(), 15, 16, 28963105567, 141045770),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/19.transaction.pb").as_slice(), 7, 8, 167910994, 89007),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/20.transaction.pb").as_slice(), 15, 16, 78838015635, 383928742),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/21.transaction.pb").as_slice(), 17, 18, 440896546003, 2150697991),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/22.transaction.pb").as_slice(), 17, 19, 938593313, 133461811),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/23.transaction.pb").as_slice(), 15, 17, 125641062, 613078),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/24.transaction.pb").as_slice(), 10, 12, 19624657285, 1352732),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/25.transaction.pb").as_slice(), 2, 3, 15832379, 1722417),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/26.transaction.pb").as_slice(), 4, 5, 18865901, 2052431),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/27.transaction.pb").as_slice(), 7, 8, 1408955, 70019),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/28.transaction.pb").as_slice(), 4, 5, 18865892, 2052407),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/29.transaction.pb").as_slice(), 4, 5, 18884648, 2052272),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/30.transaction.pb").as_slice(), 4, 5, 11015244, 1195921),
            (include_bytes!("../fixtures/swap-coverage/dlmm-host-fee-alias/31.transaction.pb").as_slice(), 4, 5, 3885198, 421814),
        ];
        for &(bytes, source, first, input, output) in fixtures {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes.to_vec()).unwrap();
            let ix = tx.walk_instructions().nth(source as usize).unwrap();
            let native_data = ix.data().clone();
            let layout = decode_swap_instruction(&ix).unwrap();
            let event_data = tx.walk_instructions().skip(source as usize + 1)
                .find(|child| output_host_fee_event(child, &layout, input, if child.data().get(88) == Some(&1) { &layout.token_y_mint } else { &layout.token_x_mint })
                    .is_some_and(|(primary, _)| primary == output)).unwrap().data().clone();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.source_index == Some(source)
                && s.protocol == pb::Protocol::MeteoraDlmm as i32).collect();
            assert_eq!(swaps.len(), 1, "one physical swap, separately declared host payout: source={source} input={input} decoded={:?}", decoded.swaps);
            let swap = swaps[0];
            assert_eq!((swap.input_amount, swap.output_amount, swap.source_transfer_index), (input, output, Some(first)));
            assert_eq!((swap.amm_pool.as_slice(), swap.user.as_slice()), (layout.lb_pair.as_slice(), layout.user.as_slice()));
            assert!(swap.transfer_verified);
            for mode in 0..12 {
                let mut altered = tx.clone();
                if mode == 0 {
                    let message = altered.transaction.as_mut().unwrap().message.as_mut().unwrap();
                    if let Some(native) = message.instructions.iter_mut().find(|i| i.data == native_data) {
                        native.accounts[9] = native.program_id_index as u8;
                    } else {
                        let native = altered.meta.as_mut().unwrap().inner_instructions.iter_mut()
                            .flat_map(|group| &mut group.instructions).find(|i| i.data == native_data).unwrap();
                        native.accounts[9] = native.program_id_index as u8;
                    }
                } else if mode == 11 {
                    let program = substreams_solana::base58::encode(dlmm::PROGRAM_ID);
                    for log in &mut altered.meta.as_mut().unwrap().log_messages {
                        if log.as_str() == format!("Program {program} success") {
                            *log = format!("Program {program} failed: custom program error: 0x1");
                        }
                    }
                } else {
                    let event = &mut altered.meta.as_mut().unwrap().inner_instructions.iter_mut()
                        .flat_map(|group| &mut group.instructions).find(|i| i.data == event_data).unwrap().data;
                    match mode {
                        1 => event[16 + 137] ^= 1,
                        2 => event[16 + 105] ^= 1,
                        3 => event[16 + 89] ^= 1,
                        4 => event[16 + 145] = 1,
                        5 => event[16] ^= 1,
                        6 => event[16 + 32] ^= 1,
                        7 => event[16 + 72] ^= 1,
                        8 => event[0] ^= 1,
                        9 => event.truncate(162),
                        10 => event.push(0),
                        _ => unreachable!(),
                    }
                }
                assert!(crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter()
                    .all(|s| s.source_index != Some(source) || s.protocol != pb::Protocol::MeteoraDlmm as i32)),
                    "invalid host payout witness mutation {mode}");
            }
        }
    }
}
