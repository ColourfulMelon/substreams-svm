//! Explicit SPL Token Swap forks. Their amount-in is a maximum: canonical
//! constant-product ceiling division can leave input atoms unspent. Only the
//! native typed input/output and intervening LP fee CPIs prove the price.
use crate::{native_venues::Layout, token_mints::TokenMintLookup};
use proto::pb::dex::swaps::v1::Protocol;
use std::collections::HashSet;
use substreams_solana::{b58, block_view::InstructionView, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_idls::spl::{token, token_2022};

pub(crate) const DOOAR: [u8; 32] = b58!("Dooar9JkhdZ7J3LHN3A7YCuoGRUggXhQaG4kijfLGU2j");
pub(crate) const PENGUIN: [u8; 32] = b58!("PSwapMdSai8tjrEXcxFeQth87xC4rRsa4VA5mhGhXkP");
pub(crate) const DEXLAB: [u8; 32] = b58!("DSwpgjMvXhtGn6BsbqmacdBZyfLj6jSWf3HJpdJtmg6N");

pub(crate) fn layout(ix: &InstructionView) -> Option<Layout> {
    let extended = ix.program_id().0.as_slice() == DEXLAB;
    if !extended && ![DOOAR.as_slice(), PENGUIN.as_slice()].contains(&ix.program_id().0.as_slice()) {
        return None;
    }
    let a = ix.accounts();
    let data = ix.data();
    if data.len() != 17
        || data[0] != 1
        || if extended {
            ![14, 15].contains(&a.len())
        } else {
            ![10, 11].contains(&a.len())
        }
        || if extended {
            a[13].0.as_slice() != token::PROGRAM_ID
                || a[11..13]
                    .iter()
                    .any(|p| ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&p.0.as_slice()))
                || a[9].0 == a[10].0
        } else {
            a[9].0.as_slice() != token::PROGRAM_ID
        }
        || a[3].0 == a[6].0
        || a[4].0 == a[5].0
        || a[0].0 == a[2].0
        || a[4..6].iter().any(|v| a[3].0 == v.0 || a[6].0 == v.0)
    {
        return None;
    }
    let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
    if amount == 0 {
        return None;
    }
    Some(Layout {
        protocol: Protocol::SplTokenSwap,
        pool: 0,
        authority: 2,
        users: [3, 6],
        vaults: [4, 5],
        amount,
        exact_input: true,
        input_limit: true,
        input_first: true,
        fixed_users: true,
        minimum_output: u64::from_le_bytes(data[9..17].try_into().ok()?),
    })
}

pub(crate) fn settlement(source_index: u32, tx: &ConfirmedTransaction, ix: &InstructionView, calls: &[InstructionView], mints: &TokenMintLookup, successful: &HashSet<u32>) -> bool {
    let Some(layout) = layout(ix) else { return false };
    let a = ix.accounts();
    let extended = ix.program_id().0.as_slice() == DEXLAB;
    let base_accounts = if extended { 14 } else { 10 };
    let depth = if ix.is_root() { 1 } else { ix.stack_height() };
    if !successful.contains(&source_index)
        || !(2..=4).contains(&calls.len())
        || calls
            .iter()
            .enumerate()
            .any(|(offset, child)| child.stack_height() != depth + 1 || !successful.contains(&(source_index + 1 + offset as u32)))
    {
        return false;
    }
    let input = &calls[0];
    let output = &calls[calls.len() - 1];
    let i = input.accounts();
    let o = output.accounts();
    let opcode = if extended { 12 } else { 3 };
    let transfer_len = if extended { 10 } else { 9 };
    let account_len = if extended { 4 } else { 3 };
    let input_program = a[if extended { 11 } else { 9 }].0;
    let output_program = a[if extended { 12 } else { 9 }].0;
    let input_mint = mints.mint_for(a[3].0);
    let output_mint = mints.mint_for(a[6].0);
    if input.program_id().0 != input_program
        || output.program_id().0 != output_program
        || input.data().len() != transfer_len
        || output.data().len() != transfer_len
        || input.data()[0] != opcode
        || output.data()[0] != opcode
        || i.len() != account_len
        || o.len() != account_len
        || i[0].0 != a[3].0
        || i[if extended { 2 } else { 1 }].0 != a[4].0
        || i[account_len - 1].0 != a[2].0
        || o[0].0 != a[5].0
        || o[if extended { 2 } else { 1 }].0 != a[6].0
        || o[account_len - 1].0 != a[1].0
        || input_mint.is_none()
        || output_mint.is_none()
        || input_mint == output_mint
        || input_mint.as_deref() != mints.mint_for(a[4].0).as_deref()
        || output_mint.as_deref() != mints.mint_for(a[5].0).as_deref()
        || extended
            && (i[1].0 != a[9].0 || o[1].0 != a[10].0 || input_mint.as_deref() != Some(a[9].0.as_slice()) || output_mint.as_deref() != Some(a[10].0.as_slice()))
        || input_mint.as_deref() == Some(a[7].0.as_slice())
        || output_mint.as_deref() == Some(a[7].0.as_slice())
    {
        return false;
    }
    let input_amount = u64::from_le_bytes(input.data()[1..9].try_into().unwrap());
    let output_amount = u64::from_le_bytes(output.data()[1..9].try_into().unwrap());
    if input_amount == 0 || input_amount > layout.amount || output_amount == 0 {
        return false;
    }
    let fees = &calls[1..calls.len() - 1];
    if fees.len() == 2 && a.len() != base_accounts + 1 {
        return false;
    }
    let fees_bound = fees.iter().enumerate().all(|(position, fee)| {
        let f = fee.accounts();
        let recipient = if fees.len() == 2 && position == 0 { base_accounts } else { 8 };
        fee.program_id().0 == a[if extended { 13 } else { 9 }].0
            && fee.data().len() == 9
            && fee.data()[0] == 7
            && f.len() == 3
            && f[0].0 == a[7].0
            && f[1].0 == a[recipient].0
            && f[2].0 == a[1].0
            && mints.mint_for(f[1].0).as_deref() == Some(a[7].0.as_slice())
            && [3, 4, 5, 6].iter().all(|role| a[*role].0 != f[1].0)
    });
    if !fees_bound { return false; }
    if ix.program_id().0.as_slice() != PENGUIN { return true; }
    // SPL Token Swap binds its pool authority to [pool, stored bump]. Do not
    // restrict valid pool bumps to the highest PDA bump used by most pools.
    let program = solana_pubkey::Pubkey::new_from_array(PENGUIN);
    if !(0u8..=255).rev().any(|bump| solana_pubkey::Pubkey::create_program_address(&[a[0].0, &[bump]], &program)
        .is_ok_and(|authority| authority.as_ref() == a[1].0)) || output_amount < layout.minimum_output { return false; }
    let Some(meta) = tx.meta.as_ref() else { return false };
    let keys = tx.resolved_accounts();
    let mut roles = vec![4usize, 5, 8];
    if fees.len() == 2 { roles.push(base_accounts); }
    for role in roles {
        let address = a[role].0;
        let Some(position) = keys.iter().position(|key| *key == address) else { return false };
        let Some(before) = meta.pre_token_balances.iter().find(|b| b.account_index as usize == position) else { return false };
        let Some(after) = meta.post_token_balances.iter().find(|b| b.account_index as usize == position) else { return false };
        let mint = if role == 4 { input_mint.as_ref().unwrap().as_slice() } else if role == 5 { output_mint.as_ref().unwrap().as_slice() } else { a[7].0 };
        if before.mint != substreams_solana::base58::encode(mint) || after.mint != before.mint || after.owner != before.owner
            || before.program_id != substreams_solana::base58::encode(&token::PROGRAM_ID) || after.program_id != before.program_id
            || role < 6 && before.owner != substreams_solana::base58::encode(a[1].0) { return false; }
        let mut change = 0i128;
        for (index, c) in tx.walk_instructions().enumerate() {
            if ![token::PROGRAM_ID.as_slice(), token_2022::PROGRAM_ID.as_slice()].contains(&c.program_id().0.as_slice())
                || !c.accounts().iter().any(|key| key.0 == address) { continue; }
            if !successful.contains(&(index as u32)) { continue; }
            if c.program_id().0.as_slice() != token::PROGRAM_ID { return false; }
            let ca = c.accounts(); let d = c.data();
            let amount = if d.len() >= 9 { i128::from(u64::from_le_bytes(d[1..9].try_into().unwrap())) } else { return false };
            match (d[0], d.len(), ca.len()) {
                (3, 9, 3) | (12, 10, 4) => {
                    let from = ca[0].0; let to = ca[if d[0] == 3 { 1 } else { 2 }].0;
                    if from != address && to != address || mints.mint_for(from).as_deref() != Some(mint)
                        || mints.mint_for(to).as_deref() != Some(mint) || d[0] == 12 && ca[1].0.as_slice() != mint { return false; }
                    if from == address { change -= amount; } if to == address { change += amount; }
                }
                (7, 9, 3) | (14, 10, 3) if role >= 8 && ca[0].0 == a[7].0 && ca[1].0 == address && ca[2].0 == a[1].0 => { change += amount; }
                _ => return false,
            }
        }
        let Some(opening) = before.ui_token_amount.as_ref().and_then(|v| v.amount.parse::<u64>().ok()) else { return false };
        let Some(closing) = after.ui_token_amount.as_ref().and_then(|v| v.amount.parse::<u64>().ok()) else { return false };
        if i128::from(closing) - i128::from(opening) != change { return false; }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

    #[test]
    fn finalized_forks_bind_rounded_principal_debits_lp_fees_and_net_receipts() {
        for (fixture, source, input, output, epoch) in [
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/ADWKvbWojpSyPMTR1LoyEjmN5LyScBZoJYsfpBkm6Vfn1VtnuRFAPnbEUCzp1k3hjFeiKFvL5JjQkLw6HSwyBs6.pb"
                )
                .as_slice(),
                8u32,
                192164469u64,
                20822428u64,
                1055u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/4T84DTgi2ax8ejKnb8LyLcUTty7MRwq4iaNydVqks4iGyPYhKX5nU3G8pynn3KXTdU99bK2zcdYVfbULyQ6Py3D6.pb"
                )
                .as_slice(),
                9u32,
                192164467u64,
                20821952u64,
                1055u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/2CqhVUfF3UfoPEUKAGewwRSGoyTsCM2A6KWfRjWRjBqnNUAt5mJN2bGvyHMrpeUcPaQyqygxEkk9cqij4FTqYtpQ.pb"
                )
                .as_slice(),
                8u32,
                192164465u64,
                20821476u64,
                1055u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/rfM2pCjeN7nS85CMnVB2FsB8ZryUBSoae6UiWt9c3rTppMC8kN1f97t1XEo68qEnU8khW3bqYCgkdgdb2Hho7Ug.pb"
                )
                .as_slice(),
                10u32,
                1988000u64,
                48422602552u64,
                1055u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/bZnTLFxWW1KKv2s2FMS4h7vhdhPa7sM6yovukbu75cN2EYbertrr8L1bFCBA5aizGqjZ33jive1zqByc1Z1oJz9.pb"
                )
                .as_slice(),
                10u32,
                1988000u64,
                48349175921u64,
                1055u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/43TEd2DZE7v9xXk7997tXE6o6zfm3a9wud3FFwg2E2cbWie9rQimUSoevT9KaxiHAoTnpVzmBFBFp4BbzXgU3T12.pb"
                )
                .as_slice(),
                17u32,
                9998528246u64,
                3159339818u64,
                1055u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/43TEd2DZE7v9xXk7997tXE6o6zfm3a9wud3FFwg2E2cbWie9rQimUSoevT9KaxiHAoTnpVzmBFBFp4BbzXgU3T12.pb"
                )
                .as_slice(),
                22u32,
                3159339818u64,
                311575569u64,
                1055u64,
            ),
            (
                include_bytes!(
                    "../fixtures/swap-coverage/spl-swap-forks/5gdsTMZLfRnnbM7HNGCVL6EbH8ML5hSx6JBQHvYi1jXY3kmv3wKhbaTeyVsjriycHRJL1HWokzktt8jAFWzZGcXL.pb"
                )
                .as_slice(),
                9u32,
                52914930u64,
                5733148u64,
                1055u64,
            ),
        ] {
            let tx = substreams::proto::decode::<ConfirmedTransaction>(&fixture.to_vec()).unwrap();
            let instructions: Vec<_> = tx.walk_instructions().collect();
            let original = &instructions[source as usize];
            let depth = original.stack_height();
            let end = instructions
                .iter()
                .enumerate()
                .skip(source as usize + 1)
                .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
                .map_or(instructions.len(), |(end, _)| end);
            let mints = TokenMintLookup::new(&tx, tx.meta.as_ref().unwrap());
            let success = crate::logs::successful_invocations(&tx);
            assert!(settlement(source, &tx, original, &instructions[source as usize + 1..end], &mints, &success));
            let native_data = original.data().clone();
            let program = original.program_id().0.clone();
            let a: Vec<_> = original.accounts().iter().map(|a| a.0.clone()).collect();
            let decoded = crate::process_transaction_with_metadata(
                tx.clone(),
                None,
                |_, _| None,
                |_, _| None,
                |mint, _| {
                    let data = match substreams_solana::base58::encode(mint).as_str() {
                        "6AVAUKa9uxQpruHZUinFECpXEh1usRVtzQWK8N2wpump" => {
                            include_bytes!("../fixtures/swap-coverage/spl-swap-forks/fee-mints/6AVAUKa9uxQpruHZUinFECpXEh1usRVtzQWK8N2wpump.bin").as_slice()
                        }
                        "CgNePAbb19MEoCPL2azxHqUtrugsiyiL4ZJbaHgnSKM8" => {
                            include_bytes!("../fixtures/swap-coverage/spl-swap-forks/fee-mints/CgNePAbb19MEoCPL2azxHqUtrugsiyiL4ZJbaHgnSKM8.bin").as_slice()
                        }
                        _ => return None,
                    };
                    let (schedule, initialized) = crate::token_fees::owned_mint(&token_2022::PROGRAM_ID, data)?;
                    assert!(initialized);
                    schedule.at(epoch)
                },
            )
            .unwrap();
            let swaps: Vec<_> = decoded.swaps.iter().filter(|s| s.amm == program && s.source_index == Some(source)).collect();
            assert_eq!(swaps.len(), 1);
            let swap = swaps[0];
            assert_eq!(swap.protocol, Protocol::SplTokenSwap as i32);
            assert_eq!(swap.amm_pool, a[0]);
            assert_eq!(swap.user, a[2]);
            assert_eq!(swap.input_mint, mints.mint_for(&a[3]).unwrap());
            assert_eq!(swap.output_mint, mints.mint_for(&a[6]).unwrap());
            assert_eq!((swap.input_amount, swap.output_amount), (input, output));
            assert_eq!(swap.source_transfer_index, Some(source + 1));
            assert!(swap.transfer_verified);
            assert!(decoded.coverage_candidates.iter().all(|c| c.source_index != source));
            for mode in 0..20 {
                let mut wrong = tx.clone();
                let group = wrong
                    .meta
                    .as_mut()
                    .unwrap()
                    .inner_instructions
                    .iter_mut()
                    .find(|g| g.instructions.iter().any(|ix| ix.data == native_data))
                    .unwrap();
                let position = group.instructions.iter().position(|ix| ix.data == native_data).unwrap();
                let calls = &mut group.instructions;
                let native = calls[position].clone();
                let extended = program.as_slice() == DEXLAB;
                let output_position = (position + 1..calls.len())
                    .find(|n| calls[*n].stack_height.unwrap_or(0) <= depth)
                    .unwrap_or(calls.len()) - 1;
                match mode {
                    0 => calls[position].data[0] = 2,
                    1 => calls[position].data.push(0),
                    2 => calls[position].data[1..9].copy_from_slice(&(input - 1).to_le_bytes()),
                    3 => calls[position + 1].accounts[0] = native.accounts[0],
                    4 => calls[position + 1].accounts[if extended { 2 } else { 1 }] = native.accounts[0],
                    5 => calls[position + 1].accounts[if extended { 3 } else { 2 }] = native.accounts[0],
                    6 => calls[position + 1].data[0] = 7,
                    7 => calls[position + 2].accounts[0] = native.accounts[0],
                    8 => calls[position + 2].accounts[1] = native.accounts[6],
                    9 => calls[position + 2].accounts[2] = native.accounts[2],
                    10 => calls[position + 2].data[0] = 3,
                    11 => calls[output_position].accounts[0] = native.accounts[4],
                    12 => calls[output_position].accounts[if extended { 2 } else { 1 }] = native.accounts[0],
                    13 => calls[output_position].accounts[if extended { 3 } else { 2 }] = native.accounts[2],
                    14 => calls[output_position].data[0] = 7,
                    15 => calls[output_position].stack_height = Some(depth + 2),
                    16 => {
                        let duplicate = calls[position + 1].clone();
                        calls.insert(position + 2, duplicate);
                    }
                    17 => calls[position].accounts[if extended { 13 } else { 9 }] = native.accounts[0],
                    18 => {
                        let success = format!("Program {} success", substreams_solana::base58::encode(&program));
                        let logs = &mut wrong.meta.as_mut().unwrap().log_messages;
                        let ordinal = tx.walk_instructions().take(source as usize).filter(|ix| ix.program_id().0 == &program).count();
                        let end = logs.iter().enumerate().filter(|(_, line)| *line == &success).nth(ordinal).unwrap().0;
                        logs[end] = success.replace(" success", " failed: custom program error: 1");
                    }
                    _ => calls[position + 1].data[1..9].fill(0),
                }
                let views: Vec<_> = wrong.walk_instructions().collect();
                let bad = &views[source as usize];
                let end = views
                    .iter()
                    .enumerate()
                    .skip(source as usize + 1)
                    .find(|(_, child)| child.is_root() || child.stack_height() <= depth)
                    .map_or(views.len(), |(end, _)| end);
                let mints = TokenMintLookup::new(&wrong, wrong.meta.as_ref().unwrap());
                assert!(
                    !settlement(
                        source,
                        &wrong,
                        bad,
                        &views[source as usize + 1..end],
                        &mints,
                        &crate::logs::successful_invocations(&wrong)
                    ),
                    "source {source}, invalid fork settlement {mode}"
                );
            }
        }
    }
    #[test]
    fn finalized_penguin_swaps_require_pool_pda_and_whole_vault_fee_ledgers() {
        use substreams_solana::base58;
        let mut count=0;
        for line in include_str!("../fixtures/swap-coverage/penguin-native27/cases.tsv").lines() {
            let f:Vec<_>=line.split('\t').collect();let source=f[1].parse::<u32>().unwrap();
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&std::fs::read(format!("{}/fixtures/swap-coverage/penguin-native27/{}.pb",env!("CARGO_MANIFEST_DIR"),f[0])).unwrap()).unwrap();
            let decoded=crate::process_transaction(tx.clone()).unwrap();let rows:Vec<_>=decoded.swaps.iter().filter(|s|s.source_index==Some(source)&&s.amm==PENGUIN).collect();
            assert_eq!(rows.len(),1,"{}",f[0]);let row=rows[0];count+=1;
            assert!(row.transfer_verified);assert_eq!(row.protocol,Protocol::SplTokenSwap as i32);assert_eq!(row.source_transfer_index,Some(source+1));
            assert_eq!((base58::encode(&row.amm_pool),base58::encode(&row.user),base58::encode(&row.input_mint),row.input_amount,base58::encode(&row.output_mint),row.output_amount),
                (f[2].into(),f[3].into(),f[4].into(),f[5].parse().unwrap(),f[6].into(),f[7].parse().unwrap()));
            let verify_route=|transaction:&ConfirmedTransaction| {
                let mut candidate=row.clone();candidate.protocol=Protocol::JupiterV4 as i32;let mut candidates=vec![candidate];
                let mints=TokenMintLookup::new(transaction,transaction.meta.as_ref().unwrap());
                crate::quality::verify_with_fees(transaction,&mints,&mut candidates,false,|_,_|None);candidates
            };
            assert_eq!(verify_route(&tx).len(),1);
            let native=tx.walk_instructions().nth(source as usize).unwrap().data().clone();
            for mode in 0..33 {
                let mut wrong=tx.clone();
                if mode<29 || mode==31 {
                    let g=wrong.meta.as_mut().unwrap().inner_instructions.iter_mut().find(|g|g.instructions.iter().any(|n|n.data==native)).unwrap();
                    let p=g.instructions.iter().position(|n|n.data==native).unwrap();let a=g.instructions[p].accounts.clone();let n=&mut g.instructions;
                    match mode {
                        0=>n[p].data[0]=2,1=>n[p].data.push(0),2=>n[p].data[1..9].copy_from_slice(&(row.input_amount-1).to_le_bytes()),
                        3=>n[p].data[9..17].copy_from_slice(&(row.output_amount+1).to_le_bytes()),4=>n[p].accounts[0]=a[2],5=>n[p].accounts[1]=a[2],
                        6=>n[p].accounts[3]=a[4],7=>n[p].accounts[4]=a[5],8=>n[p].accounts[5]=a[4],9=>n[p].accounts[6]=a[3],
                        10=>n[p].accounts[7]=a[0],11=>n[p].accounts[8]=a[6],12=>n[p].accounts[9]=a[0],
                        13=>n[p+1].data[1..9].copy_from_slice(&(row.input_amount+1).to_le_bytes()),14=>n[p+1].data[1..9].fill(0),
                        15=>n[p+1].accounts[0]=a[4],16=>n[p+1].accounts[1]=a[5],17=>n[p+1].accounts[2]=a[1],
                        18=>n[p+2].data[1]^=1,19=>n[p+2].accounts[0]=a[0],20=>n[p+2].accounts[1]=a[6],21=>n[p+2].accounts[2]=a[2],22=>n[p+2].data[0]=3,
                        23=>n[p+3].data[1..9].copy_from_slice(&(row.output_amount+1).to_le_bytes()),24=>n[p+3].data[1..9].fill(0),
                        25=>n[p+3].accounts[0]=a[4],26=>n[p+3].accounts[1]=a[3],27=>n[p+3].accounts[2]=a[2],28=>n[p+3].stack_height=Some(99),
                        _=>n[p+1].program_id_index=u32::from(a[0]),
                    }
                } else if mode<32 {
                    let native_ix=tx.walk_instructions().nth(source as usize).unwrap();let roles=native_ix.accounts();let keys=tx.resolved_accounts();
                    let account=keys.iter().position(|k|*k==roles[if mode==29{5}else{8}].0).unwrap();
                    let b=wrong.meta.as_mut().unwrap().post_token_balances.iter_mut().find(|b|b.account_index as usize==account).unwrap().ui_token_amount.as_mut().unwrap();
                    b.amount=(b.amount.parse::<u64>().unwrap()+1).to_string();
                } else {
                    wrong.meta.as_mut().unwrap().log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PENGUIN))).for_each(|l|*l=format!("Program {} failed: mutation",base58::encode(PENGUIN)));
                }
                assert!(crate::process_transaction(wrong.clone()).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source)||s.amm!=PENGUIN)),"native Penguin mode{mode}");
                assert!(verify_route(&wrong).is_empty(),"routed Penguin mode{mode}");
            }
        }
        assert_eq!(count,4);
    }

}
