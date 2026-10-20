//! CryptoQuest's deployed market-fill contract. QUEST burn and net settlement
//! are separate committed CPIs; the taker's gross debit includes the burn.
use common::solana::parse_program_data;
use proto::pb::dex::swaps::v1 as pb;
use std::collections::HashMap;
use substreams_solana::{base58, pb::sf::solana::r#type::v1::ConfirmedTransaction};
use substreams_solana_macro::b58;
use substreams_solana_idls::spl::token;

pub(crate) const PROGRAM_ID: [u8;32] = b58!("CQwWoJENUtKmwCMqnyGbEYkg41oxdat23kkNdJLvY7v9");
const QUEST: [u8;32] = b58!("QUESTP8xKMfot3ErcdfWXsHbG3kN9mutieAqrVNw74s");
const GLOBAL: [u8;32] = b58!("CQGRe4JSbpP4dUwQAqCmoeTdk8wosNGgyv79xomteJcn");
const HOUSE: [u8;32] = b58!("HoUseFgqjatXiL3AFjtewENhBKk15fRW8GCX6Gdjw8hQ");
const ATA: [u8;32] = b58!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const FILL: [u8;8] = [117,151,22,99,112,117,139,125];

pub(crate) fn decode_fills(tx: &ConfirmedTransaction) -> Vec<pb::Swap> {
    if !tx.is_successful() { return Vec::new(); }
    let walk:Vec<_>=tx.walk_instructions().collect();
    if !walk.iter().any(|ix|ix.program_id().0.as_slice()==PROGRAM_ID) { return Vec::new(); }
    let committed=crate::logs::successful_invocations(tx);
    // Bind Program data to the exact source and runtime frame. An event inside
    // a token/foreign child cannot prove its parent's settlement.
    let mut events=HashMap::<usize,Vec<(usize,Vec<u8>)>>::new();
    let mut cursor=0usize;let mut stack=Vec::<usize>::new();
    let Some(meta)=tx.meta.as_option() else { return Vec::new(); };
    for (log_index,log) in meta.log_messages.iter().enumerate() {
        let w:Vec<_>=log.split_whitespace().collect();
        if w.first()==Some(&"Program") && w.get(2)==Some(&"invoke") {
            let Some(depth)=w.get(3).and_then(|s|s.strip_prefix('[')).and_then(|s|s.strip_suffix(']')).and_then(|s|s.parse::<u32>().ok()) else { return Vec::new(); };
            while walk.get(cursor).is_some_and(|ix|ix.program_id().to_string()=="ComputeBudget111111111111111111111111111111" && ix.program_id().to_string()!=w[1]) { cursor+=1; }
            let Some(ix)=walk.get(cursor) else { return Vec::new(); };
            if w.len()!=4 || ix.program_id().to_string()!=w[1] || (if ix.is_root(){1}else{ix.stack_height()})!=depth || depth as usize!=stack.len()+1 { return Vec::new(); }
            stack.push(cursor);cursor+=1;
        } else if w.first()==Some(&"Program") && matches!(w.get(2),Some(&"success")|Some(&"failed:")) {
            let Some(index)=stack.pop() else { return Vec::new(); };
            if walk[index].program_id().to_string()!=w[1] { return Vec::new(); }
        } else if let Some(index)=stack.last() {
            if walk[*index].program_id().0.as_slice()!=PROGRAM_ID { continue; }
            if let Some(data)=parse_program_data(log).filter(|d|d.starts_with(&FILL)) { events.entry(*index).or_default().push((log_index,data)); }
        }
    }
    if !stack.is_empty() { return Vec::new(); }
    let accounts=tx.resolved_accounts();let mut fills=Vec::new();
    for (index,ix) in walk.iter().enumerate() {
        let a=ix.accounts();let d=ix.data();
        if ix.program_id().0.as_slice()!=PROGRAM_ID || d.len()!=9 || !matches!(d[0],218|221) || a.len()!=18
            || a[2].0.as_slice()!=GLOBAL || a[4].0.as_slice()!=HOUSE || a[14].0.as_slice()!=QUEST || a[15].0.as_slice()!=token::PROGRAM_ID
            || a[16].0.as_slice()!=ATA || a[17].0.iter().any(|b|*b!=0) || a[8].0==a[14].0 || a[1].0.iter().all(|b|*b==0)
            || !committed.contains(&(index as u32)) { continue; }
        let Some(e)=events.get(&index).filter(|e|e.len()==1).map(|e|&e[0]) else { continue; };
        let event=&e.1;let ask=d[0]==218;
        if event.len()!=201 || event[8]!=u8::from(!ask)
            || [(9,3),(41,5),(73,9),(105,if ask{1}else{4}),(137,8)].iter().any(|(offset,role)|&event[*offset..*offset+32]!=a[*role].0.as_slice()) { continue; }
        let qty=u64::from_le_bytes(event[169..177].try_into().unwrap());let unit=u64::from_le_bytes(event[177..185].try_into().unwrap());
        let gross=u64::from_le_bytes(event[185..193].try_into().unwrap());let burned=u64::from_le_bytes(event[193..201].try_into().unwrap());
        let Some(asset)=qty.checked_mul(1_000_000) else { continue; };
        let Some(expected)=qty.checked_mul(unit).and_then(|n|n.checked_mul(1000)) else { continue; };
        let Some(net)=gross.checked_sub(burned).filter(|n|*n>0) else { continue; };
        if qty==0 || unit==0 || burned==0 || expected!=gross || qty!=u64::from_le_bytes(d[1..9].try_into().unwrap()) { continue; }
        let depth=if ix.is_root(){1}else{ix.stack_height()};
        let end=walk.iter().enumerate().skip(index+1).find(|(_,c)|(if c.is_root(){1}else{c.stack_height()})<=depth).map_or(walk.len(),|(n,_)|n);
        let children=&walk[index+1..end];
        if depth==0 || children.len()!=3 || children.iter().enumerate().any(|(n,c)|c.is_root() || c.stack_height()!=depth+1 || c.program_id().0.as_slice()!=token::PROGRAM_ID || !committed.contains(&((index+n+1)as u32))) { continue; }
        let roles=[[12,14,10,10],[12,13,10,10],[7,11,6,6]];let amounts=[burned,net,asset];
        if children.iter().enumerate().any(|(n,c)| { let ca=c.accounts();let cd=c.data();
            cd.len()!=if n==0{10}else{9} || cd[0]!=if n==0{15}else{3} || n==0 && cd[9]!=9 || ca.len()!=4
                || ca.iter().zip(roles[n]).any(|(key,role)|key.0!=a[role].0) || u64::from_le_bytes(cd[1..9].try_into().unwrap())!=amounts[n]
        }) { continue; }
        // Both sides' persistent town banks prove the whole transaction's
        // asset movements, including other fills in the same instruction list.
        let valid=(||->Option<()> {
            let mut target=HashMap::<Vec<u8>,(Vec<u8>,Vec<u8>,u8,u64,u64,i128)>::new();
            for (role,mint,owner,decimals) in [(7,8,6,6),(11,8,10,6),(12,14,10,9),(13,14,6,9)] {
                let position=accounts.iter().position(|key|*key==a[role].0)?;
                let pre=meta.pre_token_balances.iter().filter(|b|b.account_index as usize==position).collect::<Vec<_>>();
                let post=meta.post_token_balances.iter().filter(|b|b.account_index as usize==position).collect::<Vec<_>>();
                if pre.len()!=1 || post.len()!=1 { return None; }
                let before=pre[0];let after=post[0];let mint_key=base58::encode(a[mint].0);let owner_key=base58::encode(a[owner].0);
                let b=before.ui_token_amount.as_option()?;let p=after.ui_token_amount.as_option()?;
                if before.mint!=mint_key || after.mint!=mint_key || before.owner!=owner_key || after.owner!=owner_key
                    || before.program_id!=base58::encode(token::PROGRAM_ID) || after.program_id!=before.program_id || b.decimals!=decimals || p.decimals!=decimals { return None; }
                if target.insert(a[role].0.to_vec(),(a[mint].0.to_vec(),a[owner].0.to_vec(),decimals as u8,b.amount.parse().ok()?,p.amount.parse().ok()?,0)).is_some() { return None; }
            }
            for (n,c) in walk.iter().enumerate() {
                let ca=c.accounts();let cd=c.data();
                if c.program_id().0.as_slice()!=token::PROGRAM_ID || !ca.iter().any(|key|target.contains_key(key.0)) { continue; }
                if !committed.contains(&(n as u32)) || ca.len()!=4 || ca[2].0!=ca[3].0 { return None; }
                let amount=i128::from(u64::from_le_bytes(cd.get(1..9)?.try_into().ok()?));
                match (cd.first(),cd.len()) {
                    (Some(3),9)=>{
                        let source=target.get(ca[0].0)?;let dest=target.get(ca[1].0)?;
                        if source.0!=dest.0 || source.1.as_slice()!=ca[2].0.as_slice() { return None; }
                        target.get_mut(ca[0].0 )?.5-=amount;target.get_mut(ca[1].0 )?.5+=amount;
                    }
                    (Some(15),10)=>{
                        let source=target.get(ca[0].0)?;
                        if source.0.as_slice()!=ca[1].0.as_slice() || source.1.as_slice()!=ca[2].0.as_slice() || source.2!=cd[9] { return None; }
                        target.get_mut(ca[0].0 )?.5-=amount;
                    }
                    _=>return None,
                }
            }
            target.values().all(|t|i128::from(t.4)-i128::from(t.3)==t.5).then_some(())
        })().is_some();
        if !valid { continue; }
        // The event consistently names town9's buyer, including the House
        // maker when a seller invokes FillBid. Emit one buyer-perspective fill.
        fills.push(pb::Swap {protocol:pb::Protocol::Cryptoquest.into(),program_id:PROGRAM_ID.to_vec(),amm:PROGRAM_ID.to_vec(),amm_pool:a[3].0.to_vec(),user:event[105..137].to_vec(),stack_height:depth,
            input_mint:a[14].0.to_vec(),input_amount:gross,output_mint:a[8].0.to_vec(),output_amount:asset,
            source_index:Some(index as u32),source_log_index:Some(e.0 as u32),orderbook_fill_verified:true,..Default::default() });
    }
    fills
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD,Engine};
    #[test]
    fn retained_market_fills_bind_buyer_burn_principals_and_both_town_ledgers() {
        let signatures=["5q78v5Ri7c63U9dGgLnH7WEVTZVRinYGPPvtRteQaEekPQefdpNzVzZpEmxCkWPsfPGQMZ8ig5AiJ8Y4m8F9eYpz","nWY5ryPesc9dY7jQn3ZvnpMN1zb4PipXteY6vuCojP3B8Mvck1n65TPRtbjZX9WtGjnt7erdQNxGQBYpwA9iv1H"];
        let mut total=0;
        for (file,signature) in signatures.iter().enumerate() {
            let bytes=std::fs::read(format!("{}/fixtures/swap-coverage/cryptoquest-live26/{signature}.pb",env!("CARGO_MANIFEST_DIR"))).unwrap();
            let tx=substreams::proto::decode::<ConfirmedTransaction>(&bytes).unwrap();
            let direct=decode_fills(&tx);assert_eq!(direct.len(),if file==0{2}else{1});
            let processed=crate::process_transaction(tx.clone()).unwrap();
            for row in direct {
                total+=1;let source=row.source_index.unwrap();let log=row.source_log_index.unwrap() as usize;
                let walk:Vec<_>=tx.walk_instructions().collect();let native=walk[source as usize].data().clone();let roles=walk[source as usize].accounts();
                let root_index=tx.transaction.as_option().unwrap().message.as_option().unwrap().instructions.iter().position(|n|n.data==native).unwrap();
                let expected=if file==1{(28_500_000_000,30_000_000)}else if source==2{(159_600_000_000,152_000_000)}else{(162_800_000_000,148_000_000)};
                assert_eq!((row.input_amount,row.output_amount),expected);assert_eq!(row.user,roles[if file==1{4}else{1}].0.clone());
                assert_eq!(row.amm_pool,roles[3].0.clone());assert_eq!(row.input_mint,QUEST);assert_eq!(row.output_mint,roles[8].0.clone());assert_eq!(row.source_transfer_index,None);assert!(row.orderbook_fill_verified);
                assert_eq!(processed.swaps.iter().filter(|s|s.source_index==Some(source)&&s.protocol== pb::Protocol::Cryptoquest).collect::<Vec<_>>(),vec![&row]);
                for mode in 0..38 {
                    let mut wrong=tx.clone();
                    if mode<11 {
                        let n=&mut wrong.transaction.as_option_mut().unwrap().message.as_option_mut().unwrap().instructions[root_index];let a=n.accounts.clone();
                        match mode {0=>n.data[1]^=1,1=>n.data[0]=219,2=>n.data.push(0),3=>{n.accounts.pop();},4=>n.accounts[3]=a[5],5=>n.accounts[5]=a[9],6=>n.accounts[9]=a[5],7=>n.accounts[if file==1{4}else{1}]=a[0],8=>n.accounts[8]=a[14],9=>n.accounts[14]=a[8],_=>n.accounts[15]=a[16]}
                    } else if mode<22 {
                        let logs=&mut wrong.meta.as_option_mut().unwrap().log_messages;let mut e=parse_program_data(&logs[log]).unwrap();
                        match mode {11=>e[8]^=1,12=>e[9]^=1,13=>e[105]^=1,14=>e[137]^=1,15=>e[169]^=1,16=>e[177]^=1,17=>e[185]^=1,18=>e[193]^=1,19=>{e.pop();},20=>logs.insert(log,logs[log].clone()),_=>{let original=logs.remove(log);let child=logs.iter().position(|l|l==&format!("Program {} invoke [2]",base58::encode(token::PROGRAM_ID))).unwrap();logs.insert(child+1,original);}}
                        if mode<20{logs[log]=format!("Program data: {}",STANDARD.encode(e));}
                    } else if mode<31 {
                        let n=&mut wrong.meta.as_option_mut().unwrap().inner_instructions.iter_mut().find(|g|g.index as usize==root_index).unwrap().instructions[if mode<26{0}else if mode<28{1}else{2}];let a=n.accounts.clone();
                        match mode {22|26|28=>n.data[1]^=1,23=>n.accounts[1]=a[0],24|29=>n.accounts[2]=a[0],25=>n.data[9]=6,27=>n.accounts[1]=a[0],_=>n.stack_height=Some(99)}
                    } else {
                        let meta=wrong.meta.as_option_mut().unwrap();match mode {31=>{let b=meta.post_token_balances[0].ui_token_amount.as_option_mut().unwrap();b.amount=(b.amount.parse::<u64>().unwrap()+1).to_string();},32=>meta.post_token_balances[0].owner=base58::encode([1;32]),33=>meta.err=buffa::MessageField::some(Default::default()),34=>meta.log_messages.clear(),35=>meta.log_messages.iter_mut().filter(|l|**l==format!("Program {} success",base58::encode(PROGRAM_ID))).for_each(|l|*l=format!("Program {} failed: test",base58::encode(PROGRAM_ID))),36=>{let group=meta.inner_instructions.iter_mut().find(|g|g.index as usize==root_index).unwrap();group.instructions.push(group.instructions[2].clone());},_=>meta.pre_token_balances[0].ui_token_amount.as_option_mut().unwrap().decimals=8}
                    }
                    assert!(decode_fills(&wrong).iter().all(|s|s.source_index!=Some(source)),"{signature} source{source} invalid mode{mode}");
                    assert!(crate::process_transaction(wrong).map_or(true,|d|d.swaps.iter().all(|s|s.source_index!=Some(source))),"{signature} source{source} invalid mode{mode} fallback");
                }
            }
        }
        assert_eq!(total,3);
    }
}
