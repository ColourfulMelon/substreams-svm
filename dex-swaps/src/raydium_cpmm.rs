use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use substreams_solana::block_view::InstructionView;
use substreams_solana_idls::raydium;

use crate::logs::{scoped_program_log, ProgramLog};

pub(crate) struct State {
    pending: Vec<InstructionSwap>,
    is_invoked: bool,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            pending: Vec::new(),
            is_invoked: false,
        }
    }

    pub(crate) fn handle_instruction(&mut self, ix: &InstructionView) {
        if let Some(swap) = decode_cpmm_instruction(ix) {
            self.pending.push(swap);
        }
    }

    pub(crate) fn handle_log(&mut self, log_message: &str) -> Option<pb::Swap> {
        let ProgramLog::Data(log_message) =
            scoped_program_log(log_message, &raydium::cpmm::PROGRAM_ID.to_vec(), &mut self.is_invoked)?
        else {
            return None;
        };

        let log = parse_log_data(log_message)?;
        let position = self.pending.iter().position(|swap| swap.pool_state == log.pool_id)?;
        let instruction = self.pending.remove(position);

        Some(pb::Swap {
            protocol: pb::Protocol::RaydiumCpmm as i32,
            program_id: raydium::cpmm::PROGRAM_ID.to_vec(),
            stack_height: instruction.stack_height,
            amm: raydium::cpmm::PROGRAM_ID.to_vec(),
            amm_pool: instruction.pool_state.clone(),
            user: instruction.payer.clone(),
            input_mint: log.input_mint.unwrap_or_else(|| instruction.input_token_mint.clone()),
            input_amount: log.input_amount,
            output_mint: log.output_mint.unwrap_or_else(|| instruction.output_token_mint.clone()),
            output_amount: log.output_amount,
            ..Default::default()
    })
    }
}

struct InstructionSwap {
    stack_height: u32,
    payer: Vec<u8>,
    pool_state: Vec<u8>,
    input_token_mint: Vec<u8>,
    output_token_mint: Vec<u8>,
}

struct LogSwap {
    pool_id: Vec<u8>,
    input_amount: u64,
    output_amount: u64,
    input_mint: Option<Vec<u8>>,
    output_mint: Option<Vec<u8>>,
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    decode_cpmm_instruction(ix).map(|s| s.pool_state)
}

fn decode_cpmm_instruction(ix: &InstructionView) -> Option<InstructionSwap> {
    let program_id = ix.program_id().0;
    if program_id != &raydium::cpmm::PROGRAM_ID {
        return None;
    }

    match raydium::cpmm::instructions::unpack(ix.data()) {
        Ok(raydium::cpmm::instructions::RaydiumCpmmInstruction::SwapBaseInput(_)) => {
            let accounts = raydium::cpmm::accounts::get_swap_base_input_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_token_mint: accounts.input_token_mint.to_bytes().to_vec(),
                output_token_mint: accounts.output_token_mint.to_bytes().to_vec(),
            })
        }
        Ok(raydium::cpmm::instructions::RaydiumCpmmInstruction::SwapBaseOutput(_)) => {
            let accounts = raydium::cpmm::accounts::get_swap_base_output_accounts(&ix).ok()?;
            Some(InstructionSwap {
                stack_height: ix.stack_height(),
                payer: accounts.payer.to_bytes().to_vec(),
                pool_state: accounts.pool_state.to_bytes().to_vec(),
                input_token_mint: accounts.input_token_mint.to_bytes().to_vec(),
                output_token_mint: accounts.output_token_mint.to_bytes().to_vec(),
            })
        }
        _ => None,
    }
}

fn parse_log_data(log_message: &str) -> Option<LogSwap> {
    let data = parse_program_data(log_message)?;
    match raydium::cpmm::events::unpack(data.as_slice()) {
        Ok(raydium::cpmm::events::RaydiumCpmmEvent::SwapEventV1(event)) => Some(LogSwap {
            pool_id: event.pool_id.to_bytes().to_vec(),
            input_amount: event.input_amount,
            output_amount: event.output_amount,
            input_mint: None,
            output_mint: None,
        }),
        Ok(raydium::cpmm::events::RaydiumCpmmEvent::SwapEventV2(event)) => Some(LogSwap {
            pool_id: event.pool_id.to_bytes().to_vec(),
            input_amount: event.input_amount,
            output_amount: event.output_amount,
            input_mint: Some(event.input_mint.to_bytes().to_vec()),
            output_mint: Some(event.output_mint.to_bytes().to_vec()),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_duplicate_event_cannot_shift_another_pools_amounts() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use borsh::BorshSerialize;
        use solana_program::pubkey::Pubkey;
        use raydium::cpmm::events::{SwapEventV1, SWAP_EVENT};

        let mut state = State::new();
        for pool in [1u8, 2] {
            state.pending.push(InstructionSwap {
                stack_height: 1, payer: vec![9; 32], pool_state: vec![pool; 32],
                input_token_mint: vec![pool + 10; 32], output_token_mint: vec![pool + 20; 32],
            });
        }
        let event = SwapEventV1 {
            pool_id: Pubkey::new_from_array([2; 32]), input_vault_before: 0, output_vault_before: 0,
            input_amount: 123, output_amount: 456, input_transfer_fee: 0, output_transfer_fee: 0, base_input: true,
        };
        let mut data = SWAP_EVENT.to_vec();
        event.serialize(&mut data).unwrap();
        let log = format!("Program data:{}", STANDARD.encode(data));
        let program = substreams_solana::base58::encode(&raydium::cpmm::PROGRAM_ID);
        state.handle_log(&format!("Program {program} invoke [1]"));
        let swap = state.handle_log(&log).unwrap();
        assert_eq!(swap.amm_pool, vec![2; 32]);
        assert_eq!(swap.input_mint, vec![12; 32]);
        assert_eq!((swap.input_amount, swap.output_amount), (123, 456));
        assert!(state.handle_log(&log).is_none());
        assert_eq!(state.pending.len(), 1);
        assert_eq!(state.pending[0].pool_state, vec![1; 32]);
    }
}
