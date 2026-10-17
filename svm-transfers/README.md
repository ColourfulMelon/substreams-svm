# `svm-transfers`

`DatabaseChanges` for SPL token and native SOL transfers. The ClickHouse sink
package and schema live in [`clickhouse/`](clickhouse/).

## Upgrading to v0.4.0

- `spl_transfer` and `transfers` rows are keyed by **original block positions**:
  `transaction_index` and `instruction_index` are the transaction's and outer
  instruction's positions in the complete block, and the new
  `inner_instruction_index` is 0 for an outer instruction, else 1 + its
  position among the inner instructions. Earlier versions wrote dense indexes
  over filtered transactions and instructions, so the same columns mean
  something else for every existing row.
- Both tables use `MergeTree`, so re-sinking a range appends rows instead of
  replacing them. Rebuild them rather than backfilling part of the history:
  apply the schema (it adds `inner_instruction_index` idempotently), truncate
  `spl_transfer` and `transfers`, then re-sink from the start block.
- Unchecked `Transfer` rows now have an empty `mint` (the instruction does not
  name it) and the destination account in `destination`; zero amounts and self
  transfers are kept in `spl_transfer`. The `transfers` view still filters on
  `amount > 0 AND mint IS NOT NULL`, which does not exclude empty mints.
- `system_transfer` (native SOL) keeps its previous dense indexes.
