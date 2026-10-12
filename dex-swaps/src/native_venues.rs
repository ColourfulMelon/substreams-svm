//! Native market layouts, gated by program, swap opcode and payload length.
//! Amounts are taken from scoped user/vault CPIs in `quality`, never from a
//! quoted minimum, router total, oracle update or whole-transaction delta.
use proto::pb::dex::swaps::v1::Protocol;
use substreams_solana::{b58, block_view::InstructionView};

pub(crate) const ORCA_V1: [u8; 32] = b58!("DjVE6JNiYqPL2QXyCUUh8rNjHrbz9hXHNYt99MQ59qw1");

pub(crate) const VENUES: &[(Protocol, [u8; 32])] = &[
    (Protocol::GoonfiV2, b58!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE")),
    (Protocol::Bisonfi, b58!("BiSoNHVpsVZW2F7rx2eQ59yQwKxzU5NvBcmKshCSUypi")),
    (Protocol::Manifest, b58!("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms")),
    (Protocol::Humidifi, b58!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp")),
    (Protocol::OrcaV2, b58!("9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP")),
    (Protocol::SplTokenSwap, ORCA_V1),
    (Protocol::SplTokenSwap, substreams_solana_idls::spl::token_swap::PROGRAM_ID),
    (Protocol::Alphaq, b58!("ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA")),
    (Protocol::Kipseli, b58!("3TK9D8aoBFYjYZtKCjciPrVrRStsnvo7KmpcJqDavpaU")),
    (Protocol::Flux, b58!("FLUX6xBayGxLX9UcimVRxXFMHH6q43mAbRvDzSpCsvfK")),
    // Jupiter labels the separate pricing program ojh19... as Scorch. It
    // does not move tokens; native swaps execute in SCoRcH8c... instead.
    (Protocol::Scorch, b58!("SCoRcH8c2dpjvcJD6FiPbCSQyQgu3PcUAWj2Xxx3mqn")),
    (Protocol::Obsidian, b58!("HBVw6bZtcCaezhcBrmfyXBSBRWCdv72271xQ4GPvms2z")),
    (Protocol::Tesserav, b58!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH")),
    (Protocol::Deriverse, b58!("DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD")),
    (Protocol::Zerofi, b58!("ZERor4xhbUycZ6gb9ntrhqscUcZmAbQDjEAtCf4hbZY")),
    (Protocol::MeteoraDbc, b58!("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN")),
    (Protocol::Byreal, b58!("REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2")),
    (Protocol::Pancakeswap, b58!("HpNfyc2Saw7RKkQd8nEL4khUcuPhQ7WwY1B2qjx8jxFq")),
    (Protocol::Fluxbeam, b58!("FLUXubRmkEi2q6K3Y9kBPg9248ggaZVsoSFhtJHSrm1X")),
    (Protocol::PoseidonV2, b58!("6RCmQ8XMQCJnPbNDQa8U9ucAS9hk4tXQeps96V7p6aRY")),
    (Protocol::OrcaWhirlpool, b58!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc")),
    (Protocol::RaydiumClmm, b58!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK")),
    (Protocol::MeteoraDlmm, b58!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo")),
    (Protocol::BisonfiPredict, b58!("2DNbzPochEcyCcWMbL4d9S3u9QqQEj5bbe6cSZFvKsbh")),
    (Protocol::Aquifer, b58!("AQU1FRd7papthgdrwPTTq5JacJh8YtwEXaBfKU3bTz45")),
    (Protocol::Denali, b58!("DNL1tgEj3nJovHw9jtyCCQD3arssCJzkmpDizknwzey4")),
    (Protocol::SolfiV2, substreams_solana_idls::solfi::v2::PROGRAM_ID),
    (Protocol::Saros, substreams_solana_idls::saros::PROGRAM_ID),
    (Protocol::RaydiumLaunchpad, substreams_solana_idls::raydium::launchpad::PROGRAM_ID),
    (Protocol::RaydiumCpmm, substreams_solana_idls::raydium::cpmm::PROGRAM_ID),
    (Protocol::MeteoraDaam, substreams_solana_idls::meteora::daam::PROGRAM_ID),
    (Protocol::Plasma, substreams_solana_idls::plasma::PROGRAM_ID),
    (Protocol::Bonkswap, substreams_solana_idls::bonkswap::PROGRAM_ID),
    (Protocol::Heaven, substreams_solana_idls::heaven::PROGRAM_ID),
    (Protocol::Stabble, substreams_solana_idls::stabble::PROGRAM_ID),
    (Protocol::Defituna, b58!("fUSioN9YKKSa3CUC2YUc4tPkHJ5Y6XW1yz8y6F7qWz9")),
    (Protocol::Invariant, b58!("HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt")),
    (Protocol::Onedex, b58!("DEXYosS6oEGvk8uCDayvwEZz4qEyDJRf9nFgYCaqPMTm")),
    (Protocol::OpenbookV2, b58!("opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb")),
    (Protocol::Mercurial, crate::mercurial::PROGRAM_ID),
    (Protocol::Oasis, crate::oasis::PROGRAM_ID),
];

pub(crate) struct Layout {
    pub protocol: Protocol,
    pub pool: usize,
    pub authority: usize,
    pub users: [usize; 2],
    pub vaults: [usize; 2],
    pub amount: u64,
    pub exact_input: bool,
    pub input_limit: bool,
    pub input_first: bool,
    // DBC fixes users in input/output order, vaults in base/quote order.
    pub fixed_users: bool,
    pub minimum_output: u64,
}

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let program = ix.program_id();
    let protocol = VENUES.iter().find(|(_, id)| id.as_slice() == program.0.as_slice())?.0;
    let raw = ix.data();
    let mut data = raw.clone();
    if protocol == Protocol::SplTokenSwap {
        // Orca V1 and the canonical SwaPp program use this legacy ABI.
        // The later Token-2022 extension has separate mint/program accounts.
        let accounts = ix.accounts();
        if data.len() != 17 || data[0] != 1 || !matches!(accounts.len(), 10 | 11)
            || accounts[9].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
        {
            return None;
        }
        let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
        if amount == 0 {
            return None;
        }
        return Some(Layout {
            protocol, pool: 0, authority: 2, users: [3, 6], vaults: [4, 5], amount,
            minimum_output: u64::from_le_bytes(data[9..17].try_into().ok()?),
            exact_input: true, input_limit: true, input_first: true, fixed_users: true,
        });
    }
    if protocol == Protocol::Humidifi {
        // XOR is symmetrical. Decode with byte operations (no alignment or
        // host-endianness assumptions in WASM).
        let key = [58, 255, 47, 255, 226, 186, 235, 195];
        for (i, byte) in data.iter_mut().enumerate() {
            let position = (i / 8) as u16;
            *byte ^= key[i % 8] ^ position.to_le_bytes()[i % 2];
        }
    }
    if protocol == Protocol::Tesserav && data.first() == Some(&17) {
        // Finalized checked-transfer calls pin the newer ABI. Only its
        // observed zero trailing flag and fourteen-account shape are known.
        let accounts = ix.accounts();
        if data.len() != 19 || data[1] > 1 || data[18] != 0 || accounts.len() != 14
            || ![9, 10].iter().all(|index| [substreams_solana_idls::spl::token::PROGRAM_ID.as_slice(),
                substreams_solana_idls::spl::token_2022::PROGRAM_ID.as_slice()].contains(&accounts[*index].0.as_slice()))
        { return None; }
        let a = usize::from(data[1] == 0);
        let b = 1 - a;
        let amount = u64::from_le_bytes(data[2..10].try_into().ok()?);
        if amount == 0 { return None; }
        return Some(Layout {
            protocol, pool: 1, authority: 2, users: [5 + a, 5 + b], vaults: [3 + a, 3 + b],
            amount, minimum_output: u64::from_le_bytes(data[10..18].try_into().ok()?),
            exact_input: true, input_limit: false, input_first: true, fixed_users: true,
        });
    }
    if protocol == Protocol::OpenbookV2 {
        // Deployed v1.7: global:place_take_order (snake_case). The old
        // OpenBook IDL hashes camelCase and does not match on-chain calls.
        // Limits are LOTS, not token atoms; executed wallet flows supply
        // amounts. Market metadata is required for deposited-balance fills.
        let accounts = ix.accounts();
        if data.len() != 35
            || data[..8] != [3, 44, 71, 3, 26, 199, 203, 85]
            || data[8] > 1
            || !matches!(data[33], 1 | 3)
            || [9, 17, 25].iter().any(|offset| i64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap()) <= 0)
            || accounts.len() < 16
            || accounts[13].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
            || accounts[14].0.iter().any(|byte| *byte != 0)
        {
            return None;
        }
        let bid = data[8] == 0;
        return Some(Layout {
            protocol, pool: 2, authority: 0,
            users: if bid { [10, 9] } else { [9, 10] },
            vaults: if bid { [7, 6] } else { [6, 7] },
            amount: u64::MAX, exact_input: false, input_limit: true,
            input_first: true, fixed_users: true, minimum_output: 0,
        });
    }
    if protocol == Protocol::Manifest {
        // Bonasa-Tech/manifest 5718d3eed80fa7472a826f346c0668644ff32138:
        // both Swap (4) and SwapV2 (13) call SwapContext::load. That loader
        // accepts either a shared payer/owner or a separate owner. Opcode
        // alone never selects the account layout. The required System program
        // separates the market from the token-account prefix in each form.
        let accounts = ix.accounts();
        if data.len() != 19 || !matches!(data[0], 4 | 13) || data[17] > 1 || data[18] > 1 {
            return None;
        }
        let separate_owner = match (accounts.get(2), accounts.get(3)) {
            (Some(system), _) if system.0.as_slice() == [0; 32] => false,
            (_, Some(system)) if system.0.as_slice() == [0; 32] => true,
            _ => return None,
        };
        let shift = usize::from(separate_owner);
        if accounts.len() < 8 + shift
            || ![
                substreams_solana_idls::spl::token::PROGRAM_ID.as_slice(),
                substreams_solana_idls::spl::token_2022::PROGRAM_ID.as_slice(),
            ]
            .contains(&accounts[7 + shift].0.as_slice())
        {
            return None;
        }
        let base_in = data[17] == 1;
        return Some(Layout {
            protocol,
            pool: 1 + shift,
            authority: shift,
            users: if base_in { [3 + shift, 4 + shift] } else { [4 + shift, 3 + shift] },
            vaults: if base_in { [5 + shift, 6 + shift] } else { [6 + shift, 5 + shift] },
            amount: u64::from_le_bytes(data[1..9].try_into().ok()?),
            minimum_output: u64::from_le_bytes(data[9..17].try_into().ok()?),
            exact_input: data[18] == 1,
            input_limit: true,
            input_first: true,
            fixed_users: true,
        });
    }
    // Existing IDLs pin these swap variants and reject other token-moving
    // instructions. Native CPIs remain usable when an event is absent.
    if protocol == Protocol::OrcaWhirlpool {
        use substreams_solana_idls::orca::whirlpool::instructions::{unpack, WhirlpoolInstruction};
        let (amount, threshold, exact_input, legacy) = match unpack(&data).ok()? {
            WhirlpoolInstruction::Swap(x) => (x.amount, x.other_amount_threshold, x.amount_specified_is_input, true),
            WhirlpoolInstruction::SwapV2(x) => (x.amount, x.other_amount_threshold, x.amount_specified_is_input, false),
            _ => return None,
        };
        if ix.accounts().len() < if legacy { 11 } else { 15 } {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: if legacy { 2 } else { 4 },
            authority: if legacy { 1 } else { 3 },
            users: if legacy { [3, 5] } else { [7, 9] },
            vaults: if legacy { [4, 6] } else { [8, 10] },
            amount: if exact_input { amount } else { threshold },
            minimum_output: if exact_input { threshold } else { amount },
            exact_input,
            input_limit: true,
            input_first: false,
            fixed_users: false,
        });
    }
    if protocol == Protocol::RaydiumLaunchpad {
        use substreams_solana_idls::raydium::launchpad::instructions::{unpack, RaydiumLaunchpadInstruction};
        let (buy, exact_input, amount, minimum_output) = match unpack(&data).ok()? {
            RaydiumLaunchpadInstruction::BuyExactIn(x) => (true, true, x.amount_in, x.minimum_amount_out),
            RaydiumLaunchpadInstruction::BuyExactOut(x) => (true, false, x.maximum_amount_in, x.amount_out),
            RaydiumLaunchpadInstruction::SellExactIn(x) => (false, true, x.amount_in, x.minimum_amount_out),
            RaydiumLaunchpadInstruction::SellExactOut(x) => (false, false, x.maximum_amount_in, x.amount_out),
            _ => return None,
        };
        if ix.accounts().len() < 15 || ix.accounts()[14].0 != &substreams_solana_idls::raydium::launchpad::PROGRAM_ID {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 4,
            authority: 0,
            users: if buy { [6, 5] } else { [5, 6] },
            vaults: if buy { [8, 7] } else { [7, 8] },
            amount,
            minimum_output,
            exact_input,
            input_limit: false,
            input_first: true,
            fixed_users: true,
        });
    }
    if protocol == Protocol::Defituna {
        // FusionAMM 1.1.4's owner-verified on-chain IDL. The optional
        // remaining-account slices are bounded, fully decoded Borsh; never
        // infer a swap from a liquidity instruction or an unknown trailer.
        if ix.accounts().len() < 14 || data.len() < 43 || data[..8] != [248, 198, 158, 145, 225, 117, 135, 200] || data[40] > 1 || data[41] > 1 {
            return None;
        }
        match data[42] {
            0 if data.len() == 43 => {}
            1 if data.len() >= 47 => {
                let count = u32::from_le_bytes(data[43..47].try_into().ok()?) as usize;
                if count > 3 || data.len() != 47 + 2 * count {
                    return None;
                }
                let mut used = [false; 8];
                let mut remaining = 0usize;
                for slice in data[47..].chunks_exact(2) {
                    let kind = slice[0] as usize;
                    if !matches!(kind, 0 | 1 | 5) || used[kind] {
                        return None;
                    }
                    used[kind] = true;
                    remaining += slice[1] as usize;
                }
                if ix.accounts().len() < 14 + remaining {
                    return None;
                }
            }
            _ => return None,
        }
        let amount = u64::from_le_bytes(data[8..16].try_into().ok()?);
        let threshold = u64::from_le_bytes(data[16..24].try_into().ok()?);
        let exact_input = data[40] == 1;
        let forward = data[41] == 1;
        return Some(Layout {
            protocol,
            pool: 4,
            authority: 3,
            users: if forward { [7, 8] } else { [8, 7] },
            vaults: if forward { [9, 10] } else { [10, 9] },
            amount: if exact_input { amount } else { threshold },
            minimum_output: if exact_input { threshold } else { amount },
            exact_input,
            input_limit: true,
            input_first: true,
            fixed_users: true,
        });
    }
    if protocol == Protocol::Onedex {
        // 1INTRO's audited swap_exact_amount_in, confirmed against raw
        // executed CPIs and owner-verified PoolState mint/vault identities.
        // Users and vaults are declared in input/output order. Fees are
        // paid from the input vault, after the full user input arrives.
        // Exact-output remains unsupported until independently sampled.
        if ix.accounts().len() != 11
            || data.len() != 24
            || data[..8] != [8, 151, 245, 76, 172, 203, 144, 39]
            || ix.accounts()[10].0.as_slice() != substreams_solana_idls::spl::token::PROGRAM_ID
        {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 1,
            authority: 5,
            users: [6, 7],
            vaults: [3, 4],
            amount: u64::from_le_bytes(data[8..16].try_into().ok()?),
            exact_input: true,
            input_limit: false,
            input_first: true,
            fixed_users: true,
            minimum_output: u64::from_le_bytes(data[16..24].try_into().ok()?),
        });
    }
    if protocol == Protocol::Invariant {
        // Invariant's published Swap contract: direction, amount, exact-input
        // flag, sqrt-price limit. Referral fees are separate user debits.
        if ix.accounts().len() < 10 || data.len() != 34 || data[..8] != [248, 198, 158, 145, 225, 117, 135, 200] || data[8] > 1 || data[17] > 1 {
            return None;
        }
        let amount = u64::from_le_bytes(data[9..17].try_into().ok()?);
        let exact_input = data[17] == 1;
        let forward = data[8] == 1;
        return Some(Layout {
            protocol,
            pool: 1,
            authority: 7,
            users: if forward { [3, 4] } else { [4, 3] },
            vaults: if forward { [5, 6] } else { [6, 5] },
            amount: if exact_input { amount } else { u64::MAX },
            minimum_output: if exact_input { 0 } else { amount },
            exact_input,
            input_limit: false,
            input_first: true,
            fixed_users: true,
        });
    }
    if protocol == Protocol::RaydiumClmm {
        use substreams_solana_idls::raydium::clmm::v3::instructions::{unpack, RaydiumClmmInstruction};
        let (swap, legacy) = match unpack(&data).ok()? {
            RaydiumClmmInstruction::Swap(x) => (x, true),
            RaydiumClmmInstruction::SwapV2(x) => (x, false),
            _ => return None,
        };
        if ix.accounts().len() < if legacy { 10 } else { 13 } {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 2,
            authority: 0,
            users: [3, 4],
            vaults: [5, 6],
            amount: if swap.is_base_input { swap.amount } else { swap.other_amount_threshold },
            minimum_output: if swap.is_base_input { swap.other_amount_threshold } else { swap.amount },
            exact_input: swap.is_base_input,
            input_limit: true,
            input_first: true,
            fixed_users: true,
        });
    }
    if protocol == Protocol::MeteoraDlmm {
        use substreams_solana_idls::meteora::dlmm::instructions::{unpack, MeteoraDlmmInstruction};
        let (amount, minimum_output, exact_input, v2) = match unpack(&data).ok()? {
            MeteoraDlmmInstruction::Swap(x) => (x.amount_in, x.min_amount_out, true, false),
            MeteoraDlmmInstruction::Swap2(x) => (x.amount_in, x.min_amount_out, true, true),
            MeteoraDlmmInstruction::SwapExactOut(x) => (x.max_in_amount, x.out_amount, false, false),
            MeteoraDlmmInstruction::SwapExactOut2(x) => (x.max_in_amount, x.out_amount, false, true),
            MeteoraDlmmInstruction::SwapWithPriceImpact(x) => (x.amount_in, 0, true, false),
            MeteoraDlmmInstruction::SwapWithPriceImpact2(x) => (x.amount_in, 0, true, true),
            _ => return None,
        };
        if ix.accounts().len() < if v2 { 16 } else { 15 } {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 0,
            authority: 10,
            users: [4, 5],
            vaults: [2, 3],
            amount,
            minimum_output,
            exact_input,
            input_limit: false,
            input_first: false,
            fixed_users: true,
        });
    }
    if protocol == Protocol::RaydiumCpmm {
        use substreams_solana_idls::raydium::cpmm::instructions::{unpack, RaydiumCpmmInstruction};
        let (amount, minimum_output, exact_input) = match unpack(&data).ok()? {
            RaydiumCpmmInstruction::SwapBaseInput(x) => (x.amount_in, x.minimum_amount_out, true),
            RaydiumCpmmInstruction::SwapBaseOutput(x) => (x.max_amount_in, x.amount_out, false),
            _ => return None,
        };
        if ix.accounts().len() < 13 {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 3,
            authority: 0,
            users: [4, 5],
            vaults: [6, 7],
            amount,
            minimum_output,
            exact_input,
            input_limit: false,
            input_first: true,
            fixed_users: true,
        });
    }
    if protocol == Protocol::MeteoraDaam {
        use substreams_solana_idls::meteora::daam::instructions::{unpack, MeteoraDammInstruction};
        let (amount, minimum_output, exact_input, input_limit) = match unpack(&data).ok()? {
            MeteoraDammInstruction::Swap(x) => (x.params.amount_in, x.params.minimum_amount_out, true, false),
            MeteoraDammInstruction::Swap2(x) if x.params.swap_mode <= 2 => (
                if x.params.swap_mode == 2 { x.params.amount_1 } else { x.params.amount_0 },
                if x.params.swap_mode == 2 { x.params.amount_0 } else { x.params.amount_1 },
                x.params.swap_mode == 0,
                x.params.swap_mode != 0,
            ),
            _ => return None,
        };
        if ix.accounts().len() < 14 {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 1,
            authority: 8,
            users: [2, 3],
            vaults: [4, 5],
            amount,
            minimum_output,
            exact_input,
            input_limit,
            input_first: false,
            fixed_users: true,
        });
    }
    let tag = *data.first()?;
    if protocol == Protocol::Plasma {
        use substreams_solana_idls::plasma::instructions::{unpack, PlasmaInstruction, Side, SwapType};
        let PlasmaInstruction::Swap(swap) = unpack(&data).ok()? else { return None };
        let (amount, minimum_output, exact_input) = match swap.swap_type {
            SwapType::ExactIn { amount_in, min_amount_out } => (amount_in, min_amount_out, true),
            SwapType::ExactOut { amount_out, max_amount_in } => (max_amount_in, amount_out, false),
        };
        if ix.accounts().len() < 9 {
            return None;
        }
        let buy = matches!(swap.side, Side::Buy);
        return Some(Layout {
            protocol,
            pool: 2,
            authority: 3,
            users: if buy { [5, 4] } else { [4, 5] },
            vaults: if buy { [7, 6] } else { [6, 7] },
            amount,
            minimum_output,
            exact_input,
            input_limit: false,
            input_first: true,
            fixed_users: true,
        });
    }
    if protocol == Protocol::Bonkswap {
        use substreams_solana_idls::bonkswap::instructions::{unpack, BonkSwapInstruction};
        let BonkSwapInstruction::Swap(swap) = unpack(&data).ok()? else { return None };
        if ix.accounts().len() < 17 {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 1,
            authority: 8,
            users: if swap.x_to_y { [6, 7] } else { [7, 6] },
            vaults: if swap.x_to_y { [4, 5] } else { [5, 4] },
            amount: swap.delta_in.v,
            minimum_output: 0,
            exact_input: true,
            input_limit: true,
            input_first: true,
            fixed_users: true,
        });
    }
    let discriminator = data.get(..8);
    if protocol == Protocol::Stabble {
        let v2 = discriminator == Some(&[43, 4, 237, 11, 26, 201, 30, 98]);
        let legacy = discriminator == Some(&substreams_solana_idls::stabble::instructions::SWAP);
        if !v2 && !legacy {
            return None;
        }
        let (amount, minimum_output, input_limit) = match data.get(8) {
            Some(0) if data.len() == 17 => (u64::MAX, u64::from_le_bytes(data[9..17].try_into().ok()?), true),
            Some(1) if data.len() == 25 => (
                u64::from_le_bytes(data[9..17].try_into().ok()?),
                u64::from_le_bytes(data[17..25].try_into().ok()?),
                false,
            ),
            _ => return None,
        };
        if ix.accounts().len() < if v2 { 15 } else { 12 } {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: if v2 { 8 } else { 6 },
            authority: 0,
            users: if v2 { [3, 4] } else { [1, 2] },
            vaults: if v2 { [5, 6] } else { [3, 4] },
            amount,
            minimum_output,
            exact_input: true,
            input_limit,
            input_first: true,
            fixed_users: true,
        });
    }
    if protocol == Protocol::Heaven {
        let buy = discriminator == Some(&[102, 6, 61, 18, 1, 218, 235, 234]);
        let sell = discriminator == Some(&[51, 230, 133, 164, 1, 127, 131, 173]);
        if !buy && !sell {
            return None;
        }
        // Current buy and sell share two u64 values and a Borsh String.
        // Reject trailing bytes and malformed event data rather than accepting
        // only the historical Sell variant or an arbitrary Anchor prefix.
        let length = u32::from_le_bytes(data.get(24..28)?.try_into().ok()?) as usize;
        if data.len() != 28 + length || std::str::from_utf8(&data[28..]).is_err() || ix.accounts().len() < 16 {
            return None;
        }
        return Some(Layout {
            protocol,
            pool: 4,
            authority: 5,
            users: if buy { [9, 8] } else { [8, 9] },
            vaults: if buy { [11, 10] } else { [10, 11] },
            amount: u64::from_le_bytes(data[8..16].try_into().ok()?),
            minimum_output: u64::from_le_bytes(data[16..24].try_into().ok()?),
            exact_input: true,
            input_limit: false,
            input_first: true,
            fixed_users: true,
        });
    }
    let dbc_legacy = discriminator == Some(&[248, 198, 158, 145, 225, 117, 135, 200]);
    let dbc_v2 = discriminator == Some(&[65, 75, 63, 76, 235, 91, 91, 136]);
    let dbc_hook = discriminator == Some(&[183, 93, 153, 40, 24, 230, 194, 151]);
    if protocol == Protocol::MeteoraDbc {
        if dbc_legacy {
            if data.len() != 24 {
                return None;
            }
        } else if dbc_v2 || dbc_hook {
            if *data.get(24)? > 2 {
                return None;
            }
            if dbc_v2 && data.len() != 25 {
                return None;
            }
            if dbc_hook {
                let slices = u32::from_le_bytes(data.get(25..29)?.try_into().ok()?) as usize;
                // Borsh enum + u8 length for the two declared hook slice types.
                if slices > 2 || data.len() != 29 + 2 * slices {
                    return None;
                }
                let mut types = std::collections::HashSet::new();
                let mut extra = 0usize;
                for slice in data[29..].chunks_exact(2) {
                    if slice[0] > 1 || !types.insert(slice[0]) {
                        return None;
                    }
                    extra = extra.checked_add(slice[1] as usize)?;
                }
                if ix.accounts().len() < 15 + extra {
                    return None;
                }
            }
        } else {
            return None;
        }
    }
    // (pool, authority, users A/B, vaults A/B, amount offset, input-first)
    let (pool, authority, users, vaults, offset, input_first) = match protocol {
        Protocol::GoonfiV2 if tag == 1 && matches!(data.len(), 18 | 19) && data[1] <= 1 => (1, 0, [2, 3], [4, 5], 2, false),
        Protocol::Bisonfi if (tag == 2 && data.len() == 18) || (tag == 7 && data.len() == 19) || (tag == 19 && data.len() == 153) => {
            (1, 0, [4, 5], [2, 3], 1, false)
        }
        Protocol::Humidifi if data.len() == 25 && matches!(data[24], 4 | 15 | 20) && u64::from_le_bytes(data[16..24].try_into().ok()?) <= 1 => {
            (1, 0, [4, 5], [2, 3], 8, false)
        }
        Protocol::Humidifi if (data.len() == 113 && data[112] == 28) || (data.len() == 137 && data[136] == 50) => (1, 0, [4, 5], [2, 3], 0, false),
        Protocol::OrcaV2 if tag == 1 && data.len() == 17 => (0, 2, [3, 6], [4, 5], 1, true),
        Protocol::Alphaq if tag == 12 && data.len() == 18 && data[1] <= 1 => (1, 0, [3, 4], [5, 6], 2, false),
        Protocol::Kipseli if data.len() == 25 && data[..8] == [240, 224, 38, 33, 176, 31, 241, 175] && data[24] <= 1 => (1, 0, [6, 7], [2, 3], 8, false),
        Protocol::Flux if tag == 3 && data.len() == 18 && data[17] <= 1 => (1, 0, [4, 5], [2, 3], 9, false),
        Protocol::Scorch if tag == 2 && data.len() == 34 => (15, 1, [2, 3], [4, 5], 18, true),
        Protocol::Scorch if tag == 1 && data.len() == 34 => (11, 1, [2, 3], [4, 5], 18, true),
        Protocol::Obsidian if tag == 1 && data.len() == 9 => (1, 0, [4, 5], [2, 3], 1, false),
        // Account 0 is shared global state; account 1 is the individual market.
        Protocol::Tesserav if tag == 16 && data.len() == 18 && data[1] <= 1 => (1, 2, [5, 6], [3, 4], 2, false),
        // Tag 85 is a leveraged position change with lending CPIs, not a swap.
        Protocol::Deriverse if tag == 26 && data.len() == 32 && data[1] <= 1 && data[2..4] == [0, 0] => (5, 0, [11, 12], [3, 4], 16, false),
        Protocol::Zerofi if tag == 16 && matches!(data.len(), 17 | 18 | 57) => (0, 8, [6, 7], [3, 5], 1, true),
        Protocol::MeteoraDbc => (2, 9, [3, 4], [5, 6], if !dbc_legacy && data[24] == 2 { 16 } else { 8 }, false),
        // CLMM V3 dynamic fees append event fields; immediate CPIs still
        // supply executed amounts using the canonical input/output prefix.
        Protocol::Byreal if discriminator == Some(&[229, 46, 213, 132, 105, 40, 40, 228]) && data.len() == 41 && data[40] <= 1 => {
            (2, 0, [3, 4], [5, 6], if data[40] == 1 { 8 } else { 16 }, true)
        }
        Protocol::Pancakeswap if discriminator == Some(&[43, 4, 237, 11, 26, 201, 30, 98]) && data.len() == 41 && data[40] <= 1 => {
            (2, 0, [3, 4], [5, 6], if data[40] == 1 { 8 } else { 16 }, true)
        }
        Protocol::Fluxbeam if tag == 1 && data.len() == 17 => (0, 2, [3, 6], [4, 5], 1, true),
        // Finalized token CPIs and the program-owned POSEID02 market state
        // independently pin this prefix; unknown opcodes remain excluded.
        Protocol::PoseidonV2 if tag == 4 && data.len() == 18 && data[1] <= 1 => (0, 2, [3, 4], [5, 6], 2, false),
        // Aquifer is a multi-asset bank. Account 8 is its real global state;
        // mint pairs remain explicit, never represented by a fabricated pool.
        // Cash/outcome exchange: account 3 is the real prediction market;
        // cash sits in the shared treasury while outcome inventory is in 4.
        Protocol::BisonfiPredict if tag == 14 && data.len() == 20 && data[9..11].iter().all(|byte| *byte <= 1) && data[11] == 0 => {
            (3, 0, [1, 2], [8, if data[9] == 0 { 4 } else { 5 }], 1, false)
        }
        Protocol::Aquifer if (tag == 1 && data.len() == 9) || (tag == 19 && data.len() == 11 && data[9..] == [0, 0]) => (8, 1, [6, 3], [15, 13], 1, true),
        Protocol::Denali if discriminator == Some(&[104, 104, 131, 86, 161, 189, 180, 216]) && data.len() == 25 && data[8] <= 1 => {
            (1, 0, [10, 11], [4, 5], 9, false)
        }
        Protocol::SolfiV2 if tag == 7 && data.len() == 18 && data[17] <= 1 => (1, 0, [6, 7], [4, 5], 1, false),
        Protocol::Saros if tag == 1 && data.len() == 17 => (0, 2, [3, 6], [4, 5], 1, true),
        _ => return None,
    };
    let count = ix.accounts().len();
    let minimum_accounts = match protocol {
        Protocol::GoonfiV2 => 13,
        Protocol::Bisonfi => 8,
        Protocol::Manifest => {
            if tag == 4 {
                8
            } else {
                9
            }
        }
        Protocol::Humidifi => {
            if data.len() == 25 {
                9
            } else {
                12
            }
        }
        Protocol::OrcaV2 => 10,
        Protocol::Alphaq => 12,
        Protocol::Kipseli => 15,
        Protocol::Flux => 11,
        Protocol::Scorch => {
            if tag == 2 {
                16
            } else {
                12
            }
        }
        Protocol::Obsidian => 8,
        Protocol::Tesserav => 12,
        Protocol::Deriverse => 15,
        Protocol::Zerofi => 14,
        Protocol::MeteoraDbc => 15,
        Protocol::Byreal | Protocol::Pancakeswap => 13,
        Protocol::Fluxbeam => 14,
        Protocol::PoseidonV2 => 11,
        Protocol::BisonfiPredict => 18,
        Protocol::Aquifer => {
            if tag == 1 {
                16
            } else {
                17
            }
        }
        Protocol::Denali => 14,
        Protocol::SolfiV2 => 13,
        Protocol::Saros => 10,
        _ => return None,
    };
    if count < minimum_accounts {
        return None;
    }
    if [pool, authority, users[0], users[1], vaults[0], vaults[1]].iter().any(|index| *index >= count) {
        return None;
    }
    let amount = u64::from_le_bytes(data.get(offset..offset + 8)?.try_into().ok()?);
    let exact_input = match protocol {
        Protocol::MeteoraDbc => dbc_legacy || data[24] == 0,
        Protocol::Byreal | Protocol::Pancakeswap => data[40] == 1,
        Protocol::Manifest => data[18] == 1,
        _ => true,
    };
    let minimum_output = match protocol {
        Protocol::MeteoraDbc => u64::from_le_bytes(data[if !dbc_legacy && data[24] == 2 { 8..16 } else { 16..24 }].try_into().ok()?),
        Protocol::Byreal | Protocol::Pancakeswap => u64::from_le_bytes(data[if exact_input { 16..24 } else { 8..16 }].try_into().ok()?),
        Protocol::Fluxbeam => u64::from_le_bytes(data[9..17].try_into().ok()?),
        Protocol::PoseidonV2 => u64::from_le_bytes(data[10..18].try_into().ok()?),
        Protocol::BisonfiPredict => u64::from_le_bytes(data[12..20].try_into().ok()?),
        Protocol::Denali => u64::from_le_bytes(data[17..25].try_into().ok()?),
        Protocol::SolfiV2 | Protocol::Saros => u64::from_le_bytes(data[9..17].try_into().ok()?),
        _ => 0,
    };
    if amount == 0 && exact_input {
        return None;
    }
    Some(Layout {
        protocol,
        pool,
        authority,
        users,
        vaults,
        amount,
        exact_input,
        input_limit: matches!(
            protocol,
            Protocol::Manifest | Protocol::OrcaV2 | Protocol::Byreal | Protocol::Pancakeswap | Protocol::Saros | Protocol::Fluxbeam
        ) || (protocol == Protocol::MeteoraDbc && !dbc_legacy && data[24] != 0),
        input_first,
        fixed_users: matches!(protocol, Protocol::MeteoraDbc | Protocol::Obsidian),
        minimum_output,
    })
}

// FusionAMM 1.1.4 owner-verified IDL: two pools and three directed
// mints, with fully consumed optional transfer-hook/tick-array slices.
pub(crate) fn defituna_two_hop(ix: &InstructionView) -> Option<(u64, u64, bool)> {
    let data = ix.data();
    let accounts = ix.accounts();
    if ix.program_id().0.as_slice() != b58!("fUSioN9YKKSa3CUC2YUc4tPkHJ5Y6XW1yz8y6F7qWz9")
        || accounts.len() < 22
        || data.len() < 60
        || data[..8] != [195, 96, 237, 108, 68, 162, 219, 230]
        || data[24..27].iter().any(|flag| *flag > 1)
        || accounts[0].0 == accounts[1].0
        || accounts[2].0 == accounts[3].0
        || accounts[3].0 == accounts[4].0
        || accounts[5..8]
            .iter()
            .any(|program| program.0 != &substreams_solana_idls::spl::token::PROGRAM_ID && program.0 != &substreams_solana_idls::spl::token_2022::PROGRAM_ID)
        || accounts[21].0.as_slice() != b58!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr")
    {
        return None;
    }
    let mut remaining = 0;
    match data[59] {
        0 if data.len() == 60 => {}
        1 if data.len() >= 64 => {
            let count = u32::from_le_bytes(data[60..64].try_into().ok()?) as usize;
            if count > 5 || data.len() != 64 + 2 * count {
                return None;
            }
            let mut used = [false; 8];
            for slice in data[64..].chunks_exact(2) {
                let kind = slice[0] as usize;
                if !matches!(kind, 2 | 3 | 4 | 6 | 7) || used[kind] {
                    return None;
                }
                used[kind] = true;
                remaining += slice[1] as usize;
            }
        }
        _ => return None,
    }
    if accounts.len() < 22 + remaining {
        return None;
    }
    let amount = u64::from_le_bytes(data[8..16].try_into().ok()?);
    let threshold = u64::from_le_bytes(data[16..24].try_into().ok()?);
    (amount > 0).then_some((amount, threshold, data[24] == 1))
}

pub(crate) fn extract_pool(ix: &InstructionView) -> Option<Vec<u8>> {
    let layout = layout(ix)?;
    Some(ix.accounts().get(layout.pool)?.0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};

    #[test]
    fn finalized_orca_v1_swaps_require_exact_committed_user_and_vault_flows() {
        for (fixture, index, input, output) in [
            (include_bytes!("../fixtures/native-venues/orca-v1-route-1.transaction.pb").as_slice(), 7, 5_914_841_313u64, 655_895u64),
            (include_bytes!("../fixtures/native-venues/orca-v1-route-2.transaction.pb").as_slice(), 12, 46_517u64, 5_175u64),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let native = tx.walk_instructions().nth(index as usize).unwrap();
            let native_data = native.data().clone();
            let expected_pool = native.accounts()[0].0.clone();
            let expected_user = native.accounts()[2].0.clone();
            drop(native);
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(index)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!(swaps[0].protocol, Protocol::SplTokenSwap as i32);
            assert_eq!(swaps[0].amm, ORCA_V1);
            assert_eq!((swaps[0].input_amount, swaps[0].output_amount), (input, output));
            assert_eq!(swaps[0].amm_pool, expected_pool);
            assert_eq!(swaps[0].user, expected_user);
            assert_eq!(swaps[0].source_transfer_index, Some(index + 1));
            assert!(swaps[0].transfer_verified);
            for mode in 0..19 {
                let mut altered = tx.clone();
                let meta = altered.meta.as_mut().unwrap();
                let (group, position) = meta.inner_instructions.iter().enumerate().find_map(|(g, group)| {
                    group.instructions.iter().position(|ix| ix.data == native_data).map(|p| (g, p))
                }).unwrap();
                let instructions = &mut meta.inner_instructions[group].instructions;
                let accounts = instructions[position].accounts.clone();
                match mode {
                    0 => instructions[position].data[1..9].copy_from_slice(&(input - 1).to_le_bytes()),
                    1 => instructions[position].data[9..17].copy_from_slice(&(output + 1).to_le_bytes()),
                    2 => instructions[position].data[0] = 2,
                    3 => { instructions[position].data.pop(); }
                    4 => instructions[position].data.push(0),
                    5 => instructions[position].accounts[9] = accounts[0],
                    6 => instructions[position].accounts[1] = accounts[0],
                    7 => instructions[position].accounts[4] = accounts[0],
                    8 => instructions[position].accounts[2] = accounts[0],
                    9 => instructions[position + 1].accounts[2] = accounts[0],
                    10 => instructions[position + 2].accounts[2] = accounts[0],
                    11 => { instructions.remove(position + 2); }
                    12 => { let duplicate = instructions[position + 2].clone(); instructions.insert(position + 3, duplicate); }
                    13 => instructions[position + 2].stack_height = Some(4),
                    14 | 15 => {
                        let program = base58::encode(ORCA_V1);
                        let invoke = meta.log_messages.iter().position(|log| log == &format!("Program {program} invoke [2]")).unwrap();
                        let end = meta.log_messages.iter().enumerate().skip(invoke).find(|(_, log)| {
                            if mode == 14 {
                                **log == format!("Program {} success", base58::encode(substreams_solana_idls::jupiter::v4::PROGRAM_ID))
                            } else {
                                **log == format!("Program {program} success")
                            }
                        }).map(|(i, _)| i).unwrap();
                        if mode == 14 {
                            meta.log_messages[end] = format!("Program {} failed: custom program error: 1", base58::encode(substreams_solana_idls::jupiter::v4::PROGRAM_ID));
                        } else {
                            meta.log_messages.truncate(end);
                        }
                    }
                    16 => instructions[position + 1].data[1..9].copy_from_slice(&0u64.to_le_bytes()),
                    17 => instructions[position + 2].program_id_index = instructions[position].program_id_index,
                    _ => { let duplicate = instructions[position + 1].clone(); instructions.insert(position + 2, duplicate); }
                }
                assert!(crate::process_transaction(altered).map_or(true, |decoded| {
                    decoded.swaps.iter().all(|swap| swap.source_index != Some(index))
                }), "Orca V1 mode {mode}");
            }
        }
    }

    #[test]
    fn launchpad_native_user_receipts_do_not_depend_on_event_version() {
        use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions, TokenBalance, UiTokenAmount};
        use substreams_solana_idls::{raydium::launchpad, spl::token};
        for (buy, exact_input, tag) in [
            (true, true, launchpad::instructions::BUY_EXACT_IN),
            (true, false, launchpad::instructions::BUY_EXACT_OUT),
            (false, true, launchpad::instructions::SELL_EXACT_IN),
            (false, false, launchpad::instructions::SELL_EXACT_OUT),
        ] {
            let mut accounts: Vec<_> = (1..=15).map(|i| [i; 32]).collect();
            accounts[9] = [31; 32];
            accounts[10] = [32; 32];
            accounts[11] = token::PROGRAM_ID;
            accounts[12] = token::PROGRAM_ID;
            accounts[14] = launchpad::PROGRAM_ID;
            let mut data = tag.to_vec();
            data.extend_from_slice(&(if exact_input { 100u64 } else { 200 }).to_le_bytes());
            data.extend_from_slice(&(if exact_input { 190u64 } else { 105 }).to_le_bytes());
            data.extend_from_slice(&0u64.to_le_bytes());
            let mut tx = crate::routed_pool::test_fixture::make_tx(launchpad::PROGRAM_ID, &accounts, data);
            let mut input = vec![3];
            input.extend_from_slice(&100u64.to_le_bytes());
            let mut output = vec![3];
            output.extend_from_slice(&200u64.to_le_bytes());
            let meta = tx.meta.as_mut().unwrap();
            meta.inner_instructions = vec![InnerInstructions {
                index: 0,
                instructions: vec![
                    InnerInstruction {
                        program_id_index: 13,
                        accounts: if buy { vec![8, 10, 2] } else { vec![7, 9, 2] },
                        data: input,
                        stack_height: Some(2),
                    },
                    InnerInstruction {
                        program_id_index: 13,
                        accounts: if buy { vec![9, 7, 3] } else { vec![10, 8, 3] },
                        data: output,
                        stack_height: Some(2),
                    },
                ],
            }];
            for (position, mint) in [(5, [31; 32]), (7, [31; 32]), (6, [32; 32]), (8, [32; 32])] {
                meta.pre_token_balances.push(TokenBalance {
                    account_index: position + 2,
                    mint: base58::encode(mint),
                    ui_token_amount: Some(UiTokenAmount {
                        amount: "1000".into(),
                        decimals: 6,
                        ..Default::default()
                    }),
                    ..Default::default()
                });
            }
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            assert_eq!(decoded.swaps.len(), 1);
            let swap = &decoded.swaps[0];
            assert_eq!(swap.protocol, Protocol::RaydiumLaunchpad as i32);
            assert_eq!((swap.input_amount, swap.output_amount), (100, 200));
            assert_eq!(swap.amm_pool, accounts[4]);
            assert_eq!(swap.input_mint, accounts[if buy { 10 } else { 9 }]);
            assert!(swap.transfer_verified);
            for mode in 0..7 {
                let mut wrong = tx.clone();
                match mode {
                    0 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[0].accounts[2] = 3,
                    1 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[1].accounts[2] = 2,
                    2 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].accounts[9] = 12,
                    3 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].accounts[11] = 2,
                    4 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[8..16]
                        .copy_from_slice(&(if exact_input { 99u64 } else { 201 }).to_le_bytes()),
                    5 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data.push(0),
                    6 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[0].stack_height = Some(3),
                    _ => unreachable!(),
                }
                assert!(
                    crate::process_transaction(wrong).map_or(true, |decoded| decoded.swaps.is_empty()),
                    "unproved Launchpad buy={buy} exact_input={exact_input} mode={mode}"
                );
            }
        }
    }

    #[test]
    fn onedex_uses_gross_user_input_and_only_declared_vault_paid_fees() {
        use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions, TokenBalance, UiTokenAmount};
        use substreams_solana_idls::spl::token;
        let program = b58!("DEXYosS6oEGvk8uCDayvwEZz4qEyDJRf9nFgYCaqPMTm");
        let mut accounts: Vec<_> = (1..=11).map(|i| [i; 32]).collect();
        accounts[10] = token::PROGRAM_ID;
        let mut data = vec![8, 151, 245, 76, 172, 203, 144, 39];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&200u64.to_le_bytes());
        let mut tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
        let transfer = |amount: u64, accounts: Vec<u8>| {
            let mut data = vec![3];
            data.extend_from_slice(&amount.to_le_bytes());
            InnerInstruction {
                program_id_index: 12,
                accounts,
                data,
                stack_height: Some(2),
            }
        };
        let meta = tx.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![
                transfer(100, vec![8, 5, 7]),
                transfer(1, vec![5, 10, 4]),
                transfer(2, vec![5, 11, 4]),
                transfer(200, vec![6, 9, 4]),
            ],
        }];
        for (index, mint) in [(8, [31; 32]), (5, [31; 32]), (10, [31; 32]), (11, [31; 32]), (6, [32; 32]), (9, [32; 32])] {
            meta.pre_token_balances.push(TokenBalance {
                account_index: index,
                mint: base58::encode(mint),
                ui_token_amount: Some(UiTokenAmount {
                    amount: "1000".into(),
                    decimals: 6,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        assert_eq!(decoded.swaps.len(), 1);
        assert_eq!((decoded.swaps[0].input_amount, decoded.swaps[0].output_amount), (100, 200));
        assert_eq!(decoded.swaps[0].amm_pool, accounts[1]);
        assert!(decoded.swaps[0].transfer_verified);
        let mut no_fees = tx.clone();
        no_fees.meta.as_mut().unwrap().inner_instructions[0].instructions.drain(1..3);
        assert_eq!(crate::process_transaction(no_fees).unwrap().swaps[0].input_amount, 100);
        let mut aliased_fees = tx.clone();
        aliased_fees.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].accounts[9] = 10;
        aliased_fees.meta.as_mut().unwrap().inner_instructions[0].instructions[2].accounts[1] = 10;
        assert_eq!(crate::process_transaction(aliased_fees).unwrap().swaps[0].input_amount, 100);
        for mode in 0..10 {
            let mut wrong = tx.clone();
            match mode {
                0 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[0].data[1] = 99,
                1 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[1].accounts[1] = 9,
                2 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[1].accounts[0] = 8,
                3 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[3].accounts[2] = 7,
                4 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[2].accounts[1] = 10,
                5 => wrong.meta.as_mut().unwrap().inner_instructions[0].instructions[1].data[1] = 98,
                6 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[16] = 201,
                7 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data.push(0),
                8 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0]
                    .accounts
                    .truncate(10),
                9 => wrong.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0].data[..8]
                    .copy_from_slice(&[250, 23, 49, 134, 38, 84, 219, 125]),
                _ => unreachable!(),
            }
            assert!(
                crate::process_transaction(wrong).map_or(true, |decoded| decoded.swaps.is_empty()),
                "unproved 1DEX mode {mode}"
            );
        }
    }

    #[test]
    fn onedex_preserves_declared_alias_fee_recipients() {
        let tx = substreams::proto::decode::<substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction>(
            &include_bytes!("../fixtures/swap-coverage/onedex-aliased-fee-recipients.transaction.pb").to_vec(),
        ).unwrap();
        let decoded = crate::process_transaction(tx).unwrap();
        let legs: Vec<_> = decoded.swaps.iter().filter(|s| s.protocol == Protocol::Onedex as i32).collect();
        // Independent original-RPC vault/user conservation proves both fees
        // use the same declared recipient. Fees belong to the gross input.
        assert_eq!(legs.len(), 1);
        let leg = legs[0];
        assert_eq!(leg.source_index, Some(23));
        assert_eq!(leg.source_transfer_index, Some(24));
        assert_eq!((leg.input_amount, leg.output_amount), (20_157_333, 2_254_971));
        assert_eq!(leg.amm_pool, b58!("DbuvwPuLvH8uy2B1sKuu18aCd2QpCvfZdfDtdRZztBd2"));
        assert_eq!(leg.input_mint, b58!("So11111111111111111111111111111111111111112"));
        assert_eq!(leg.output_mint, b58!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"));
        assert!(leg.transfer_verified);
        assert_eq!(decoded.swaps.len(), 4);
        assert!(!decoded.coverage_candidates.iter().any(|c| c.source_index == 23));
    }

    #[test]
    fn invariant_referral_debits_are_part_of_the_users_swap_price() {
        use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions, TokenBalance, UiTokenAmount};
        use substreams_solana_idls::spl::token;
        let program = b58!("HyaB3W9q6XdA5xwpU4XnSZV94htfmbmqJXZcEbRaJutt");
        let mut accounts: Vec<_> = (1..=11).map(|i| [i; 32]).collect();
        accounts[9] = token::PROGRAM_ID;
        let mut data = vec![248, 198, 158, 145, 225, 117, 135, 200, 1];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.push(1);
        data.extend_from_slice(&0u128.to_le_bytes());
        let mut tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
        let transfer = |amount: u64, accounts: Vec<u8>| {
            let mut data = vec![3];
            data.extend_from_slice(&amount.to_le_bytes());
            InnerInstruction {
                program_id_index: 11,
                accounts,
                data,
                stack_height: Some(2),
            }
        };
        let meta = tx.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![transfer(200, vec![8, 6, 10]), transfer(95, vec![5, 7, 9]), transfer(5, vec![5, 12, 9])],
        }];
        for (index, mint) in [(5, [31; 32]), (7, [31; 32]), (12, [31; 32]), (6, [32; 32]), (8, [32; 32])] {
            meta.pre_token_balances.push(TokenBalance {
                account_index: index,
                mint: base58::encode(mint),
                ui_token_amount: Some(UiTokenAmount {
                    amount: "1000".into(),
                    decimals: 6,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        let decoded = crate::process_transaction(tx.clone()).unwrap();
        assert_eq!(decoded.swaps.len(), 1);
        assert_eq!((decoded.swaps[0].input_amount, decoded.swaps[0].output_amount), (100, 200));
        assert_eq!(decoded.swaps[0].amm_pool, accounts[1]);
        assert!(decoded.swaps[0].transfer_verified);
        let mut wrong_amount = tx.clone();
        wrong_amount.meta.as_mut().unwrap().inner_instructions[0].instructions[2].data[1] = 6;
        assert!(crate::process_transaction(wrong_amount).unwrap().swaps.is_empty());
        let mut undeclared_recipient = tx.clone();
        undeclared_recipient.transaction.as_mut().unwrap().message.as_mut().unwrap().instructions[0]
            .accounts
            .truncate(10);
        assert!(crate::process_transaction(undeclared_recipient).unwrap().swaps.is_empty());
        let mut extra_debit = tx;
        extra_debit.meta.as_mut().unwrap().inner_instructions[0]
            .instructions
            .push(transfer(1, vec![5, 12, 9]));
        assert!(crate::process_transaction(extra_debit).unwrap().swaps.is_empty());
    }

    #[test]
    fn fusion_swap_direction_amounts_and_remaining_slices_are_strict() {
        let program = b58!("fUSioN9YKKSa3CUC2YUc4tPkHJ5Y6XW1yz8y6F7qWz9");
        let accounts: Vec<_> = (1..=16).map(|i| [i; 32]).collect();
        for exact_input in [false, true] {
            for forward in [false, true] {
                let mut data = vec![248, 198, 158, 145, 225, 117, 135, 200];
                data.extend_from_slice(&100u64.to_le_bytes());
                data.extend_from_slice(&200u64.to_le_bytes());
                data.extend_from_slice(&0u128.to_le_bytes());
                data.extend_from_slice(&[u8::from(exact_input), u8::from(forward), 1]);
                data.extend_from_slice(&1u32.to_le_bytes());
                data.extend_from_slice(&[5, 2]);
                let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data.clone());
                let decoded = layout(&tx.walk_instructions().next().unwrap()).unwrap();
                assert_eq!(decoded.amount, if exact_input { 100 } else { 200 });
                assert_eq!(decoded.minimum_output, if exact_input { 200 } else { 100 });
                assert_eq!(decoded.vaults, if forward { [9, 10] } else { [10, 9] });
                assert_eq!(decoded.users, if forward { [7, 8] } else { [8, 7] });
                for (position, invalid) in [(40, 2), (41, 2), (42, 2), (43, 255), (47, 8), (47, 2), (48, 3), (0, 0)] {
                    let mut malformed = data.clone();
                    malformed[position] = invalid;
                    let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, malformed);
                    assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
                }
                for count in [13, 14, 15] {
                    let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts[..count], data.clone());
                    assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
                }
                data.push(0);
                let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
                assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
            }
        }
    }

    #[test]
    fn dbc_modes_and_hook_slices_have_strict_amount_and_account_layouts() {
        let program = b58!("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN");
        let accounts: Vec<_> = (1..=17).map(|i| [i; 32]).collect();
        for mode in 0..=2 {
            let mut data = vec![65, 75, 63, 76, 235, 91, 91, 136];
            data.extend_from_slice(&100u64.to_le_bytes());
            data.extend_from_slice(&200u64.to_le_bytes());
            data.push(mode);
            let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data.clone());
            let layout = layout(&tx.walk_instructions().next().unwrap()).unwrap();
            assert_eq!(layout.amount, if mode == 2 { 200 } else { 100 });
            assert_eq!(layout.minimum_output, if mode == 2 { 100 } else { 200 });
            assert_eq!(layout.exact_input, mode == 0);
            assert!(layout.fixed_users);
            assert_eq!(layout.input_limit, mode != 0);
            data[24] = 3;
            let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
            assert!(super::layout(&tx.walk_instructions().next().unwrap()).is_none());
        }
        let mut data = vec![183, 93, 153, 40, 24, 230, 194, 151];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&200u64.to_le_bytes());
        data.push(0);
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0, 2]);
        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data.clone());
        assert!(layout(&tx.walk_instructions().next().unwrap()).is_some());
        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts[..16], data.clone());
        assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
        data[29] = 2;
        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
        assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
    }

    #[test]
    fn manifest_opcodes_accept_both_owner_layouts_and_bind_direction() {
        let program = b58!("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms");
        for opcode in [4, 13] {
            for separate in [false, true] {
                let shift = usize::from(separate);
                let mut accounts: Vec<_> = (1..=9 + shift).map(|index| [index as u8; 32]).collect();
                accounts[2 + shift] = [0; 32];
                accounts[7 + shift] = substreams_solana_idls::spl::token::PROGRAM_ID;
                for base_in in [false, true] {
                    for exact_in in [false, true] {
                        let mut data = vec![opcode];
                        data.extend_from_slice(&100u64.to_le_bytes());
                        data.extend_from_slice(&50u64.to_le_bytes());
                        data.extend_from_slice(&[u8::from(base_in), u8::from(exact_in)]);
                        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data.clone());
                        let decoded = layout(&tx.walk_instructions().next().unwrap()).unwrap();
                        assert_eq!(decoded.pool, 1 + shift);
                        assert_eq!(decoded.authority, shift);
                        assert_eq!(decoded.users, if base_in { [3 + shift, 4 + shift] } else { [4 + shift, 3 + shift] });
                        assert_eq!(decoded.vaults, if base_in { [5 + shift, 6 + shift] } else { [6 + shift, 5 + shift] });
                        assert_eq!(decoded.amount, 100);
                        assert_eq!(decoded.minimum_output, 50);
                        assert_eq!(decoded.exact_input, exact_in);
                        assert!(decoded.input_limit && decoded.input_first && decoded.fixed_users);
                        for position in [0, 17, 18] {
                            let mut invalid = data.clone();
                            invalid[position] = if position == 0 { 6 } else { 2 };
                            let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, invalid);
                            assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
                        }
                        let mut invalid = data.clone();
                        invalid.push(0);
                        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, invalid);
                        assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
                        for position in [2 + shift, 7 + shift] {
                            let mut wrong = accounts.clone();
                            wrong[position] = [99; 32];
                            let tx = crate::routed_pool::test_fixture::make_tx(program, &wrong, data.clone());
                            assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
                        }
                        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts[..7 + shift], data);
                        assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
                    }
                }
            }
        }
    }

    #[test]
    fn fluxbeam_prices_use_the_executed_debit_within_the_requested_limit() {
        for (fixture, index, input, output) in [
            (
                include_bytes!("../fixtures/native-venues/fluxbeam-rounded-input-1.transaction.pb").as_slice(),
                8,
                479_999_757u64,
                1_242_776u64,
            ),
            (
                include_bytes!("../fixtures/native-venues/fluxbeam-rounded-input-2.transaction.pb").as_slice(),
                11,
                1_175_237_973u64,
                92_246_249u64,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let native = tx.walk_instructions().nth(index as usize).unwrap();
            let native_data = native.data().clone();
            let declared_input = u64::from_le_bytes(native_data[1..9].try_into().unwrap());
            assert!(declared_input > input);
            let expected_pool = native.accounts()[0].0.clone();
            let expected_user = native.accounts()[2].0.clone();
            drop(native);
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(index)).collect();
            assert_eq!(swaps.len(), 1);
            assert_eq!(swaps[0].protocol, Protocol::Fluxbeam as i32);
            assert_eq!((swaps[0].input_amount, swaps[0].output_amount), (input, output));
            assert_eq!(swaps[0].amm_pool, expected_pool);
            assert_eq!(swaps[0].user, expected_user);
            assert_eq!(swaps[0].source_transfer_index, Some(index + 1));
            assert!(swaps[0].transfer_verified);

            for mode in 0..13 {
                let mut altered = tx.clone();
                let mut changed = 0;
                for group in &mut altered.meta.as_mut().unwrap().inner_instructions {
                    for instruction in &mut group.instructions {
                        if instruction.data == native_data {
                            match mode {
                                0 => instruction.data[1..9].copy_from_slice(&(input - 1).to_le_bytes()),
                                1 => instruction.data[9..17].copy_from_slice(&(output + 1).to_le_bytes()),
                                2 => instruction.data[0] = 2,
                                3 => {
                                    instruction.data.pop();
                                }
                                _ => instruction.accounts[[1, 2, 3, 4, 5, 6, 9, 10, 11][mode - 4]] = instruction.accounts[0],
                            }
                            changed += 1;
                        }
                    }
                }
                assert_eq!(changed, 1);
                assert!(
                    crate::process_transaction(altered).map_or(true, |decoded| decoded.swaps.iter().all(|swap| swap.source_index != Some(index))),
                    "Fluxbeam mode {mode}"
                );
            }
            let mut wrong_output_program = tx;
            let mut changed = 0;
            for group in &mut wrong_output_program.meta.as_mut().unwrap().inner_instructions {
                for instruction in &mut group.instructions {
                    if instruction.data == native_data {
                        instruction.accounts[12] = instruction.accounts[0];
                        changed += 1;
                    }
                }
            }
            assert_eq!(changed, 1);
            assert!(crate::process_transaction(wrong_output_program).map_or(true, |decoded| decoded.swaps.iter().all(|swap| swap.source_index != Some(index))));
        }
    }

    #[test]
    fn finalized_tessera_checked_swaps_require_exact_roles_amounts_and_committed_cpis() {
        for line in include_str!("../fixtures/native-venues/tessera-checked/cases.tsv").lines() {
            let fields: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/native-venues/tessera-checked/{}", env!("CARGO_MANIFEST_DIR"), fields[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            assert_eq!(base58::encode(tx.hash()), fields[1]);
            let source = fields[2].parse::<u32>().unwrap();
            let native_data = tx.walk_instructions().nth(source as usize).unwrap().data().clone();
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|swap| swap.source_index == Some(source)).collect();
            assert_eq!(swaps.len(), 1, "{}", fields[0]);
            let swap = swaps[0];
            assert_eq!(swap.protocol, Protocol::Tesserav as i32);
            assert!(swap.transfer_verified);
            assert_eq!(base58::encode(&swap.amm_pool), fields[3]);
            assert_eq!(base58::encode(&swap.user), fields[4]);
            assert_eq!(base58::encode(&swap.input_mint), fields[5]);
            assert_eq!(swap.input_amount, fields[6].parse::<u64>().unwrap());
            assert_eq!(base58::encode(&swap.output_mint), fields[7]);
            assert_eq!(swap.output_amount, fields[8].parse::<u64>().unwrap());
            assert_eq!(swap.source_transfer_index, fields[9].parse::<u32>().ok());
            for mode in 0..19 {
                let mut wrong = tx.clone();
                let mut changed = 0;
                for group in &mut wrong.meta.as_mut().unwrap().inner_instructions {
                    for ix in &mut group.instructions {
                        if ix.data != native_data { continue; }
                        match mode {
                            0 => ix.data[0] = 18,
                            1 => ix.data[1] ^= 1,
                            2 => ix.data[2..10].copy_from_slice(&(swap.input_amount + 1).to_le_bytes()),
                            3 => ix.data[10..18].copy_from_slice(&(swap.output_amount + 1).to_le_bytes()),
                            4 => ix.data[18] = 1,
                            5 => { ix.data.pop(); },
                            6 => ix.data.push(0),
                            7 => { ix.accounts.pop(); },
                            8 => ix.accounts[1] = ix.accounts[2],
                            9 => ix.accounts[2] = ix.accounts[0],
                            _ => ix.accounts[[0, 3, 4, 5, 6, 7, 8, 9, 10][mode - 10]] = ix.accounts[2],
                        }
                        changed += 1;
                    }
                }
                assert_eq!(changed, 1);
                assert!(crate::process_transaction(wrong).map_or(true, |decoded| decoded.swaps.iter().all(|swap| swap.source_index != Some(source))),
                    "{} mutation {mode}", fields[0]);
            }
            for failure in [false, true] {
                let mut wrong = tx.clone();
                let logs = &mut wrong.meta.as_mut().unwrap().log_messages;
                let exit = logs.iter().position(|log| log == "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH success").unwrap();
                if failure { logs[exit] = "Program TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH failed: custom program error: 1".into(); }
                else { logs.truncate(exit); }
                assert!(crate::process_transaction(wrong).map_or(true, |decoded| decoded.swaps.iter().all(|swap| swap.source_index != Some(source))),
                    "{} unsuccessful invocation", fields[0]);
            }
        }
    }

    #[test]
    fn finalized_helius_swaps_match_exact_native_pools_and_user_flows() {
        let mut protocols = std::collections::HashSet::new();
        for line in include_str!("../fixtures/native-venues/cases.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/native-venues/{}", env!("CARGO_MANIFEST_DIR"), fields[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            assert_eq!(base58::encode(tx.hash()), fields[1]);
            let decoded = crate::process_transaction(tx);
            let mut actual = Vec::new();
            let mut identities = std::collections::HashSet::new();
            if let Some(decoded) = decoded {
                for swap in &decoded.swaps {
                    assert!(
                        identities.insert((swap.source_index, swap.source_transfer_index)),
                        "duplicate CPI in {}",
                        fields[0]
                    );
                    if !(21..=33).contains(&swap.protocol) {
                        continue;
                    }
                    assert!(swap.transfer_verified);
                    protocols.insert(swap.protocol);
                    actual.push(format!(
                        "{}:{}:{}:{}:{}:{}:{}:{}",
                        swap.protocol,
                        swap.source_index.unwrap(),
                        base58::encode(&swap.amm_pool),
                        base58::encode(&swap.user),
                        base58::encode(&swap.input_mint),
                        swap.input_amount,
                        base58::encode(&swap.output_mint),
                        swap.output_amount
                    ));
                }
            }
            assert_eq!(actual.join("|"), fields[3], "Helius mismatch in {}", fields[0]);
        }
        assert_eq!(protocols.len(), 13, "every original venue requires a real validated swap");
    }

    #[test]
    fn finalized_openbook_immediate_fills_match_wallet_cpis_and_reject_wrong_roles() {
        let mut count = 0;
        for line in include_str!("../fixtures/openbook/cases.tsv").lines().filter(|line| !line.starts_with('#')) {
            let f: Vec<_> = line.split('\t').collect();
            let bytes = std::fs::read(format!("{}/fixtures/openbook/{}", env!("CARGO_MANIFEST_DIR"), f[0])).unwrap();
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            assert_eq!(base58::encode(tx.hash()), f[1]);
            let decoded = crate::process_transaction(tx.clone()).unwrap();
            let fills: Vec<_> = decoded.swaps.iter().filter(|x| x.protocol == Protocol::OpenbookV2 as i32).collect();
            assert_eq!(fills.len(), 1, "{}", f[1]);
            let fill = fills[0];
            assert!(fill.transfer_verified && !fill.orderbook_fill_verified);
            assert_eq!(fill.source_index.unwrap().to_string(), f[2]);
            assert_eq!(fill.source_transfer_index.unwrap().to_string(), f[3]);
            assert_eq!([base58::encode(&fill.amm_pool), base58::encode(&fill.user), base58::encode(&fill.input_mint),
                fill.input_amount.to_string(), base58::encode(&fill.output_mint), fill.output_amount.to_string()], f[4..10]);
            // Wrong instruction type, vault authority or declared Token program
            // must not borrow matching amounts elsewhere in a routed transaction.
            for variant in 0..3 {
                let mut wrong = tx.clone();
                let meta = wrong.meta.as_mut().unwrap();
                let native = meta.inner_instructions.iter_mut().flat_map(|x| &mut x.instructions)
                    .find(|x| x.data.starts_with(&[3,44,71,3,26,199,203,85])).unwrap();
                match variant {
                    0 => native.data[33] = 0,
                    1 => native.accounts[3] = native.accounts[0],
                    _ => native.accounts[13] = native.accounts[14],
                }
                assert!(crate::process_transaction(wrong).map_or(true, |x| x.swaps.iter().all(|s| s.protocol != Protocol::OpenbookV2 as i32)));
            }
            count += 1;
        }
        assert_eq!(count, 47);
    }

    #[test]
    fn malformed_native_layouts_and_unknown_programs_are_not_swaps() {
        let (_, program) = VENUES[0];
        let accounts: Vec<_> = (1..=13).map(|i| [i; 32]).collect();
        let mut data = vec![1, 0];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        for (program, count, data) in [(program, 13, vec![1]), ([44; 32], 13, data.clone()), (program, 4, data.clone())] {
            let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts[..count], data);
            assert!(layout(&tx.walk_instructions().next().unwrap()).is_none());
        }
        let tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
        assert!(layout(&tx.walk_instructions().next().unwrap()).is_some());
        // A recognized swap without token CPIs can never publish a price.
        assert!(crate::process_transaction(tx).map_or(true, |decoded| decoded.swaps.is_empty()));
    }

    fn fee_fixture(explicit_fee: bool) -> ConfirmedTransaction {
        use substreams_solana::pb::sf::solana::r#type::v1::{InnerInstruction, InnerInstructions, TokenBalance, UiTokenAmount};
        use substreams_solana_idls::spl::{token, token_2022};
        let (_, program) = VENUES[0];
        let mut accounts: Vec<_> = (1..=13).map(|i| [i; 32]).collect();
        accounts[11] = token::PROGRAM_ID;
        accounts[12] = token_2022::PROGRAM_ID;
        let mut data = vec![1, 0];
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        let mut tx = crate::routed_pool::test_fixture::make_tx(program, &accounts, data);
        let mut debit = vec![3];
        debit.extend_from_slice(&100u64.to_le_bytes());
        let mut receipt = if explicit_fee { vec![26, 1] } else { vec![12] };
        receipt.extend_from_slice(&200u64.to_le_bytes());
        receipt.push(6);
        if explicit_fee {
            receipt.extend_from_slice(&6u64.to_le_bytes());
        }
        let meta = tx.meta.as_mut().unwrap();
        meta.inner_instructions = vec![InnerInstructions {
            index: 0,
            instructions: vec![
                InnerInstruction {
                    program_id_index: 13,
                    accounts: vec![4, 6, 2],
                    data: debit,
                    stack_height: Some(2),
                },
                InnerInstruction {
                    program_id_index: 14,
                    accounts: vec![7, 9, 5, 3],
                    data: receipt,
                    stack_height: Some(2),
                },
            ],
        }];
        for (balances, source, vault_in, vault_out, output) in [
            (&mut meta.pre_token_balances, 100, 0, 1000, 0),
            (&mut meta.post_token_balances, 0, 100, 800, 194),
        ] {
            for (index, mint, amount) in [
                (4, accounts[6], source),
                (6, accounts[6], vault_in),
                (7, accounts[7], vault_out),
                (5, accounts[7], output),
            ] {
                balances.push(TokenBalance {
                    account_index: index,
                    mint: base58::encode(mint),
                    ui_token_amount: Some(UiTokenAmount {
                        amount: amount.to_string(),
                        ..Default::default()
                    }),
                    ..Default::default()
                });
            }
        }
        tx
    }

    #[test]
    fn fee_outputs_use_proven_receipts_and_reject_ambiguous_net_amounts() {
        for explicit_fee in [false, true] {
            let decoded = crate::process_transaction(fee_fixture(explicit_fee)).unwrap();
            assert_eq!(decoded.swaps.len(), 1);
            assert_eq!((decoded.swaps[0].input_amount, decoded.swaps[0].output_amount), (100, 194));
        }
        let mut unknown_receipt = fee_fixture(false);
        let meta = unknown_receipt.meta.as_mut().unwrap();
        meta.pre_token_balances.retain(|b| b.account_index != 5);
        meta.post_token_balances.retain(|b| b.account_index != 5);
        // Checked transfer proves the mint, but cannot prove an implicit fee
        // without the recipient balance. Do not publish the gross output.
        assert!(crate::process_transaction(unknown_receipt).map_or(true, |decoded| decoded.swaps.is_empty()));
    }

    #[test]
    fn self_harvest_refunds_current_fees_without_pricing_older_collected_fees() {
        use substreams_solana::pb::sf::solana::r#type::v1::InnerInstruction;
        for (explicit, fee) in [(false, 6u64), (true, 6), (true, 200)] {
            let mut tx = fee_fixture(explicit);
            let meta = tx.meta.as_mut().unwrap();
            if explicit {
                meta.inner_instructions[0].instructions[1].data[11..19].copy_from_slice(&fee.to_le_bytes());
            }
            // 200 current tokens plus 9 older withheld fees. Only 200 belong
            // to this swap, even when the aggregate account delta is 209.
            meta.post_token_balances
                .iter_mut()
                .find(|balance| balance.account_index == 5)
                .unwrap()
                .ui_token_amount
                .as_mut()
                .unwrap()
                .amount = "209".into();
            meta.inner_instructions[0].instructions.push(InnerInstruction {
                program_id_index: 14,
                accounts: vec![9, 5, 3, 5],
                data: vec![26, 3, 1],
                stack_height: Some(2),
            });
            let root = base58::encode(tx.walk_instructions().next().unwrap().program_id().0);
            let mut logs = vec![format!("Program {root} invoke [1]")];
            for ix in tx.walk_instructions().skip(1) {
                let program = base58::encode(ix.program_id().0);
                logs.extend([format!("Program {program} invoke [2]"), format!("Program {program} success")]);
            }
            logs.push(format!("Program {root} success"));
            tx.meta.as_mut().unwrap().log_messages = logs;
            let decoded = crate::process_transaction(tx).unwrap();
            assert_eq!(decoded.swaps.len(), 1);
            assert_eq!(decoded.swaps[0].output_amount, 200);
        }
        for field in 0..7 {
            let mut tx = fee_fixture(false);
            let meta = tx.meta.as_mut().unwrap();
            meta.pre_token_balances.retain(|balance| balance.account_index != 5);
            meta.post_token_balances.retain(|balance| balance.account_index != 5);
            let mut refund = InnerInstruction {
                program_id_index: 14,
                accounts: vec![9, 5, 3, 5],
                data: vec![26, 3, 1],
                stack_height: Some(2),
            };
            match field {
                0 => refund.accounts[1] = 7,
                1 => refund.accounts[3] = 7,
                2 => refund.data[2] = 2,
                3 => meta.inner_instructions[0].instructions.push(InnerInstruction {
                    program_id_index: 14,
                    accounts: vec![9, 5],
                    data: vec![26, 4],
                    stack_height: Some(2),
                }),
                4 => meta.inner_instructions[0].instructions.insert(1, refund.clone()),
                _ => {}
            }
            if field != 4 {
                meta.inner_instructions[0].instructions.push(refund);
            }
            let root = base58::encode(tx.walk_instructions().next().unwrap().program_id().0);
            let calls: Vec<_> = tx.walk_instructions().skip(1).map(|ix| base58::encode(ix.program_id().0)).collect();
            let mut logs = vec![format!("Program {root} invoke [1]")];
            for (index, program) in calls.iter().enumerate() {
                logs.push(format!("Program {program} invoke [2]"));
                if field == 6 && index + 1 == calls.len() {
                    break;
                }
                logs.push(if field == 5 && index + 1 == calls.len() {
                    format!("Program {program} failed: caught refund failure")
                } else {
                    format!("Program {program} success")
                });
            }
            if field != 6 {
                logs.push(format!("Program {root} success"));
            }
            tx.meta.as_mut().unwrap().log_messages = logs;
            assert!(
                crate::process_transaction(tx).map_or(true, |decoded| decoded.swaps.is_empty()),
                "invalid refund {field}"
            );
        }
    }

    #[test]
    fn caught_parent_failure_rolls_back_successful_transfer_evidence() {
        use substreams_solana::pb::sf::solana::r#type::v1::{CompiledInstruction, InnerInstruction};
        let mut tx = fee_fixture(true);
        let message = tx.transaction.as_mut().unwrap().message.as_mut().unwrap();
        let original = message.instructions[0].clone();
        let parent_index = message.account_keys.len() as u32;
        message.account_keys.push(vec![0xaa; 32]);
        message.instructions[0] = CompiledInstruction {
            program_id_index: parent_index,
            accounts: original.accounts.clone(),
            data: vec![],
        };
        let meta = tx.meta.as_mut().unwrap();
        for call in &mut meta.inner_instructions[0].instructions {
            call.stack_height = Some(3);
        }
        meta.inner_instructions[0].instructions.insert(
            0,
            InnerInstruction {
                program_id_index: original.program_id_index,
                accounts: original.accounts,
                data: original.data,
                stack_height: Some(2),
            },
        );
        let programs: Vec<_> = tx.walk_instructions().map(|ix| base58::encode(ix.program_id().0)).collect();
        let logs = vec![
            format!("Program {} invoke [1]", programs[0]),
            format!("Program {} invoke [2]", programs[1]),
            format!("Program {} invoke [3]", programs[2]),
            format!("Program {} success", programs[2]),
            format!("Program {} invoke [3]", programs[3]),
            format!("Program {} success", programs[3]),
            format!("Program {} failed: caught swap failure", programs[1]),
            format!("Program {} success", programs[0]),
        ];
        tx.meta.as_mut().unwrap().log_messages = logs;
        let outcomes = crate::logs::invocation_outcomes(&tx);
        assert_eq!(outcomes.completed, std::collections::HashSet::from([0]));
        assert_eq!(outcomes.rolled_back, std::collections::HashSet::from([1, 2, 3]));
        assert!(crate::process_transaction(tx).map_or(true, |decoded| decoded.swaps.is_empty()));
    }

    #[test]
    fn wrong_vault_authority_amount_depth_and_failed_transactions_emit_nothing() {
        for field in 0..9 {
            let mut tx = fee_fixture(true);
            let meta = tx.meta.as_mut().unwrap();
            match field {
                0 => meta.inner_instructions[0].instructions[0].accounts[1] = 7,
                1 => meta.inner_instructions[0].instructions[0].accounts[2] = 3,
                2 => meta.inner_instructions[0].instructions[0].data[1] += 1,
                3 => meta.inner_instructions[0].instructions[1].stack_height = None,
                4 => {
                    meta.pre_token_balances.clear();
                    meta.post_token_balances.clear();
                }
                5 => meta.inner_instructions[0].instructions[1].data[11..19].copy_from_slice(&201u64.to_le_bytes()),
                6 => meta.err = Some(Default::default()),
                7 => meta.inner_instructions[0].instructions[1].data[11..19].copy_from_slice(&200u64.to_le_bytes()),
                _ => {
                    let duplicate = meta.inner_instructions[0].instructions[0].clone();
                    meta.inner_instructions[0].instructions.insert(1, duplicate);
                }
            }
            assert!(
                crate::process_transaction(tx).map_or(true, |decoded| decoded.swaps.is_empty()),
                "invalid evidence {field}"
            );
        }
    }
}
