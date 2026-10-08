//! Replay only decoder additions. Compare payloads rather than output ordinals,
//! which move when a previously missing swap or transaction becomes visible.
use std::collections::HashMap;

use prost::Message;
use proto::pb::dex::swaps::v1 as pb;
use substreams::errors::Error;
use substreams_solana::base58;

#[substreams::handlers::map]
fn map_missing_swaps(previous: pb::Events, repaired: pb::Events) -> Result<pb::Events, Error> {
    missing_swaps(previous, repaired)
}

#[substreams::handlers::map]
fn map_routed_updates(previous: pb::Events, repaired: pb::Events) -> Result<pb::Events, Error> {
    Ok(classify_swaps(previous, repaired)?.1)
}

#[substreams::handlers::map]
fn map_previous_unassigned(mut previous: pb::Events) -> Result<pb::Events, Error> {
    for transaction in &mut previous.transactions {
        transaction.swaps.retain(|swap| matches!(swap.protocol, 4 | 5) && swap.amm_pool.is_empty());
    }
    previous.transactions.retain(|transaction| !transaction.swaps.is_empty());
    Ok(previous)
}

fn missing_swaps(previous: pb::Events, repaired: pb::Events) -> Result<pb::Events, Error> {
    Ok(classify_swaps(previous, repaired)?.0)
}

fn classify_swaps(previous: pb::Events, repaired: pb::Events) -> Result<(pb::Events, pb::Events), Error> {
    let mut existing: HashMap<(Vec<u8>, Vec<u8>), Vec<pb::Swap>> = HashMap::new();
    for transaction in previous.transactions {
        for swap in transaction.swaps {
            let mut identity = swap.clone();
            identity.source_index = None;
            identity.source_transfer_index = None;
            identity.transfer_verified = false;
            identity.verification_failure.clear();
            if matches!(identity.protocol, 4 | 5) {
                identity.amm_pool.clear();
            }
            existing
                .entry((transaction.signature.clone(), identity.encode_to_vec()))
                .or_default()
                .push(swap);
        }
    }
    let mut additions = pb::Events {
        parent_slot: repaired.parent_slot,
        parent_hash: repaired.parent_hash.clone(),
        ..Default::default()
    };
    let mut routed_updates = pb::Events {
        parent_slot: repaired.parent_slot,
        parent_hash: repaired.parent_hash.clone(),
        ..Default::default()
    };
    for transaction in repaired.transactions {
        let mut added = transaction.clone();
        added.swaps.clear();
        let mut updated = transaction.clone();
        updated.swaps.clear();
        for swap in &transaction.swaps {
            let mut identity = swap.clone();
            identity.source_index = None;
            identity.source_transfer_index = None;
            identity.transfer_verified = false;
            identity.verification_failure.clear();
            if matches!(identity.protocol, 4 | 5) {
                identity.amm_pool.clear();
            }
            let previous = existing.get_mut(&(transaction.signature.clone(), identity.encode_to_vec()));
            let matched = previous.and_then(|swaps| {
                let index = swaps
                    .iter()
                    .position(|old| old.amm_pool == swap.amm_pool)
                    .or_else(|| swaps.iter().position(|old| matches!(old.protocol, 4 | 5) && old.amm_pool.is_empty()));
                index.map(|index| swaps.swap_remove(index))
            });
            match matched {
                Some(old) if old.amm_pool != swap.amm_pool => updated.swaps.push(swap.clone()),
                Some(_) => {}
                None => added.swaps.push(swap.clone()),
            }
        }
        if !added.swaps.is_empty() {
            additions.transactions.push(added);
        }
        if !updated.swaps.is_empty() {
            routed_updates.transactions.push(updated);
        }
    }
    if let Some(((signature, _), _)) = existing.into_iter().find(|(_, swaps)| !swaps.is_empty()) {
        return Err(Error::msg(format!(
            "old decoder emitted a swap absent from the repaired decoder: {}",
            base58::encode(signature)
        )));
    }
    Ok((additions, routed_updates))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(signature: u8, amounts: &[u64]) -> pb::Events {
        pb::Events {
            transactions: vec![pb::Transaction {
                signature: vec![signature; 64],
                fee_payer: vec![7; 32],
                swaps: amounts
                    .iter()
                    .map(|amount| pb::Swap {
                        input_amount: *amount,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn quality_metadata_does_not_recount_old_financial_payloads() {
        let old = events(1, &[10]);
        let mut new = old.clone();
        new.transactions[0].source_index = Some(123);
        let swap = &mut new.transactions[0].swaps[0];
        swap.source_index = Some(9);
        swap.source_transfer_index = Some(10);
        swap.transfer_verified = true;
        assert!(missing_swaps(old, new).unwrap().transactions.is_empty());
    }

    #[test]
    fn emits_only_new_legs_and_preserves_multiplicity() {
        let added = missing_swaps(events(1, &[10, 10]), events(1, &[10, 20, 10, 10])).unwrap();
        assert_eq!(added.transactions[0].swaps.iter().map(|s| s.input_amount).collect::<Vec<_>>(), [20, 10]);
        assert_eq!(added.transactions[0].fee_payer, vec![7; 32]);
    }

    #[test]
    fn transaction_identity_survives_changed_output_order() {
        let mut before = events(1, &[10]);
        before.transactions.extend(events(2, &[20]).transactions);
        let mut after = events(2, &[20]);
        after.transactions.extend(events(3, &[30]).transactions);
        after.transactions.extend(events(1, &[10]).transactions);
        let result = missing_swaps(before, after).unwrap();
        assert_eq!(result.transactions.len(), 1);
        assert_eq!(result.transactions[0].signature, vec![3; 64]);
    }

    #[test]
    fn pool_enrichment_changes_location_without_recounting_swap() {
        let mut old = events(1, &[10]);
        old.transactions[0].swaps[0].protocol = 5;
        let mut new = old.clone();
        new.transactions[0].swaps[0].amm_pool = vec![9; 32];
        let (added, routed) = classify_swaps(old, new).unwrap();
        assert!(added.transactions.is_empty());
        assert_eq!(routed.transactions[0].swaps[0].amm_pool, vec![9; 32]);
    }

    #[test]
    fn refuses_nonempty_pool_reassignment() {
        let mut old = events(1, &[10]);
        old.transactions[0].swaps[0].protocol = 5;
        old.transactions[0].swaps[0].amm_pool = vec![8; 32];
        let mut new = old.clone();
        new.transactions[0].swaps[0].amm_pool = vec![9; 32];
        assert!(classify_swaps(old, new).is_err());
    }

    #[test]
    fn unchanged_replay_is_empty() {
        assert!(missing_swaps(events(1, &[10]), events(1, &[10])).unwrap().transactions.is_empty());
    }

    #[test]
    fn refuses_to_hide_removed_or_changed_old_payloads() {
        assert!(missing_swaps(events(1, &[10]), events(1, &[20])).is_err());
        assert!(missing_swaps(events(1, &[10, 10]), events(1, &[10])).is_err());
        assert!(missing_swaps(events(1, &[10]), pb::Events::default()).is_err());
    }
}

/// Keep replay output and cursor checkpoints in an isolated database. The high
/// bit reserves a distinct row-key namespace when verified additions are applied
/// to the live raw table; historical output ordinals are not stable identities.
#[substreams::handlers::map]
fn db_backfill(
    additions: substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges,
    routed: substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges,
    unassigned: substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges,
) -> Result<substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges, Error> {
    use substreams_database_change::pb::sf::substreams::sink::database::v1::{table_change::PrimaryKey, DatabaseChanges};
    let mut result = DatabaseChanges::default();
    for (mut changes, table) in [(additions, "pending_swaps"), (routed, "routed_swaps"), (unassigned, "unassigned_swaps")] {
        changes.table_changes.retain(|change| change.table == "swaps");
        for change in &mut changes.table_changes {
            change.table = table.into();
            if let Some(PrimaryKey::CompositePk(key)) = change.primary_key.as_mut() {
                let index = key.keys.get_mut("transaction_index").ok_or_else(|| Error::msg("missing transaction ordinal"))?;
                let value: u32 = index.parse()?;
                if value >= (1 << 31) {
                    return Err(Error::msg("transaction ordinal exceeds the reserved namespace"));
                }
                *index = (value | (1 << 31)).to_string();
            } else {
                return Err(Error::msg("missing composite swap key"));
            }
        }
        result.table_changes.extend(changes.table_changes);
    }
    Ok(result)
}

#[substreams::handlers::map]
fn db_routed(
    clock: substreams::pb::substreams::Clock,
    swaps: pb::Events,
) -> Result<substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges, Error> {
    super::write_swaps(clock, swaps)
}

#[substreams::handlers::map]
fn db_unassigned(
    clock: substreams::pb::substreams::Clock,
    swaps: pb::Events,
) -> Result<substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges, Error> {
    super::write_swaps(clock, swaps)
}

/// Archive the exact deployed decoder output in an isolated daily partition.
/// Historical corrections can change existing payloads, so full-day rebuilds
/// must use the complete repaired history rather than additions alone.
#[substreams::handlers::map]
fn db_archive(
    mut changes: substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges,
) -> Result<substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges, Error> {
    changes.table_changes.retain(|change| matches!(change.table.as_str(), "swaps" | "blocks"));
    for change in &mut changes.table_changes {
        change.table = match change.table.as_str() {
            "blocks" => "canonical_blocks", _ => "canonical_swaps",
        }.into();
    }
    Ok(changes)
}
