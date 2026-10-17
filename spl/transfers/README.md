# `spl-transfers`: native SPL transfer projection

`spl_transfers::project` validates a complete Solana block and returns its SPL
token operations, each with its original position. It needs no Substreams
runtime, store, network access or protobuf runtime, so Pinax Streams and
Firehose Parquet can call it on blocks they already decoded. The
[`spl_transfers`](../transfers-substreams) Substreams package is a thin wrapper
over the same function, and `spl-token-common` (the `spl_token` packages) uses
the same instruction decoder.

```toml
spl-transfers = { git = "https://github.com/pinax-network/substreams-svm", rev = "<rev>" }
```

```rust
// Current substreams-solana-core 0.16 / Buffa model; no re-encode.
spl_transfers::block_view_buffa!(fn view_block, substreams_solana_core::pb::sf::solana::r#type::v1);

let projection = spl_transfers::project(&view_block(&block)?, spl_transfers::Coverage::TRANSFER_CHECKED)?;
// projection.clock: slot, parent_slot, hashes, optional block_time, transaction_count, coverage
// projection.operations: execution-ordered `Operation`s
```

## Records

Every validated block yields a `Clock`, even with no operations. Each
`Operation` carries:

| Field | Meaning |
|---|---|
| `transaction_index` | Index in the block, votes and failed transactions included |
| `instruction_index` | Outer (top-level) instruction index |
| `inner_instruction_index` | Index among the inner instructions of `instruction_index`; `None` for an outer instruction |
| `stack_height` | Recorded height of an inner instruction; `None` for outer ones |
| `signature` | First transaction signature (64 bytes) |
| `program`, `kind` | `Token` or `Token2022`; `Transfer`, `TransferChecked`, `MintTo`, `MintToChecked`, `Burn` or `BurnChecked` |
| `amount` | Exact raw `u64` in base units; zero is kept |
| `decimals`, `mint` | From the instruction: checked kinds carry decimals, and every kind except unchecked `Transfer` names the mint |
| `source`, `destination` | Token accounts; `None` for a mint's source and a burn's destination; self transfers are kept |
| `authority`, `extra_accounts` | The signing owner, delegate or mint authority, and the accounts after it |
| `source_owner`, `destination_owner` | Owner wallets recorded in the transaction's token balances (pre entry, else post entry); `None` when unrecorded, e.g. for an account created and closed within the transaction |

Keys and hashes are raw bytes; encode them as case-sensitive base58 at the edge.
Operations are ordered by transaction, then outer instruction, each outer
instruction followed by its inner instructions.

## Coverage

Only executed instructions of successful transactions are projected. Coverage is
declared per projection and recorded on the clock:

| `Coverage` | Adds |
|---|---|
| `TRANSFER_CHECKED` (default, empty params) | Classic Token program `TransferChecked` in top-level instructions |
| `token_2022` | Token-2022. Amounts are the requested ones; withheld transfer fees are not deducted |
| `inner_instructions` | Inner (CPI) instructions |
| `unchecked_transfers` | Unchecked `Transfer`; its mint and decimals stay unknown (no account-to-mint resolver, see #128) |
| `mint_burn` | `MintTo`, `MintToChecked`, `Burn`, `BurnChecked` |
| `ALL` (`all`) | Every class above |

The first scope is narrow. In mainnet slots 350000001–350000040, top-level
classic `TransferChecked` produced 0–13 operations per block against 343–809
under `ALL`; most transfers happen inside CPIs. Native SOL transfers are out of
scope.

## Validation

`project` and `validate_block` reject malformed input with a `ProjectError`
instead of panicking or attributing a wrong account:

- `blockhash` and `previous_blockhash` must be base58 32-byte hashes.
- `parent_slot` must be below `slot`, so skipped slots are accepted.
- `block_time` is optional but never negative; it is never synthesized.
- Every transaction needs its message, header, meta and 64-byte signatures consistent with the header.
- Successful transactions also need:
  - 32-byte keys, at most 256 resolved keys;
  - loaded address counts equal to their address table lookups;
  - in-bounds program, account, inner-instruction-parent and token-balance indexes;
  - well-formed amount-moving token instructions, which the token programs would otherwise have rejected.
- Failed transactions are never projected, so their instruction-level indexes are not checked.

## Protobuf models

The projector works over `model::Block`, a borrowed view, rather than a
generated type. The native crate has no protobuf or Substreams dependency.

- `block_view_buffa!` reads the current `substreams-solana-core` 0.16 model and
  returns `Result<model::Block<'_>, UnsupportedMessage>`. Message presence is
  explicit: absent messages never become Buffa's default instance. Coverage is
  legacy/v0 only: an explicit version above 0, contradictory v0 metadata, or any
  inline transaction configuration rejects the block with its original
  transaction index, including when that transaction failed. An unset version
  with `versioned: true` remains supported for older producers' v0 messages.
- The existing `block_view!` still accepts Prost `Vec<u8>` and `bytes::Bytes`
  models, with its original infallible view signature. Older generated models
  do not expose the newer version/configuration fields; callers must establish
  legacy/v0 coverage before using a model that cannot represent them.

Neither adapter encodes or copies byte fields. `buffa_model` and `bytes_model`
compare borrowed fields and projections on all four pinned blocks. The older
Prost model is a development dependency only, retained for compatibility tests.

The Substreams wrapper uses SDK 0.8, Solana core 0.16 and Buffa 0.9.2. Its output
types are exported through `spl_transfers_substreams::pb`. They are generated
from the unchanged canonical `proto/v1/spl/spl-transfers.proto`; run
`make protogen` in `spl/transfers-substreams` to regenerate with the pinned Buf
plugin. The package-local output avoids linking the rest of the workspace's
older Prost messages. The protobuf package, field numbers and output bytes are
unchanged; `wire_compatibility` checks 16 pre-migration output hashes. A release
WASM still requires rebuilding and repacking after any source change; previous
live evidence applies to the revision that was tested.

## Tests

`cargo test -p spl-transfers -p spl-transfers-substreams -p spl-token-common -p svm-transfers`
runs against four complete mainnet blocks in `tests/fixtures` (see its README):

- success and failure transactions, including failed `TransferChecked`;
- a top-level zero-amount transfer, plus inner self transfers;
- mints from loaded (ALT) addresses, and temporary accounts;
- Token-2022, and skipped slots.

Mutations of those blocks cover:

- malformed indexes and loaded addresses;
- truncated token instructions;
- missing block time and empty blocks.

Pinned records were cross-checked with Solana JSON-RPC `getBlock` and
`getTransaction`. The parity tests compare:

- the native projection with the Substreams module body;
- with the `spl_token` module's transfer records;
- with the `svm-transfers` rows.
