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
    match dlmm::instructions::unpack(ix.data()) {
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
            })
        }
        _ => None,
    }
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
