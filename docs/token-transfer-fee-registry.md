# Token-2022 fee registry work in progress

The deployed package remains `v0.5.2-pluto.12`. The fee registry is staged
source and is **not wired into the streaming manifest or live pricing yet**.
Do not package these changes under the immutable `.12` release. A subsequent
release requires the complete registry, ordinal-aware lookups and replay proof.

## Verified financial gap

Two finalized successful transactions contain five DLMM Swap2 instructions
whose gross token output cannot be allocated to exact receipts from their
shared destination balance alone. All five output mint
`DEW9dSN6QpWyNthphCpMmAbZP1Q4cEKR9xQXAri98WDP` (Token-2022, 100 basis-point fee).

The core fixtures in `dex-swaps/fixtures/token-fees/` retain original Firehose
transactions, independently read finalized mint bytes and the mainnet
EpochSchedule. `proof.json` records the ownership/read context. Both owned
fee schedules have epoch 1039, predating the transactions at epoch 1055.
A later fee change would activate two epochs after its execution; a later
mint re-creation would initialize its schedule at the later epoch. These
unchanged older/newer schedules therefore justify this retrospective fixture
check. This reasoning does not permit arbitrary current fee settings to be
applied to historical transactions.

| Fixture | Native instruction | Gross output | Verified received output |
| --- | ---: | ---: | ---: |
| two-receipts | 17 | 98,424,324,865 | 97,440,081,616 |
| two-receipts | 23 | 62,357,111,407 | 61,733,540,292 |
| three-receipts | 13 | 15,879,366,251 | 15,720,572,588 |
| three-receipts | 19 | 2,372,764,271 | 2,349,036,628 |
| three-receipts | 25 | 2,969,123,294 | 2,939,432,061 |

Fee-aware offline decoding verifies these five additional financial legs
while preserving all three existing financial rows byte-for-byte. It requires
the declared DLMM user/reserve accounts, exact mint pair, token programs,
authorities, immediate CPI depth and committed runtime invocation. Missing
logs cannot prove a fee-bearing transfer. When all destination receipts/debits
are known, their sum must match the finalized balance delta. Disagreement
invalidates derived metadata receipts and permits recovery only through the
established independent conservation proof. Original gross events remain
available as diagnostic replacements.

The business logic tests include rounding/caps/u64 maximum amounts,
epoch-delayed fee changes, superseded pending changes, mint close/re-creation,
unproven calls, malformed owned mint data, history corruption, real repeated
receipts, deliberately incorrect fee settings and successful children rolled
back by failed ancestors. All 115 decoder tests and 12 SQL mapper tests pass;
the staged source also compiles for wasm32-unknown-unknown with Rust 1.85.1.

## Rules and source

Native Token-2022 rules are pinned to
[`b4724e6e7fecc8fc36295f861a769401bf92b270`](https://github.com/solana-program/token-2022/tree/b4724e6e7fecc8fc36295f861a769401bf92b270):

- `interface/src/extension/transfer_fee/mod.rs`: ceil(gross*bps/10000), capped
  by maximum fee, subtracted from gross; use u128 for intermediate arithmetic.
- `program/src/extension/transfer_fee/processor.rs`: initialize both schedules
  at the current epoch; SetTransferFee activates at epoch+2 and retains the
  applicable older schedule.
- `interface/src/extension/transfer_fee/instruction.rs`: native encodings for
  InitializeTransferFeeConfig and SetTransferFee.
- `interface/src/extension/mod.rs`: an uninitialized fee mint has account type
  **0**, becoming type **1** only when its base mint is initialized. Fee
  initialization alone does not perform that transition.

A global snapshot must therefore include both initialized and pending fee
mints. Filtering only type 1, or performing separate type 1/type 0 scans,
can miss an existing pending fee mint that initializes between page reads.
Use a stable mint-padding filter (83 zero bytes starting at offset 82), then
validate ownership, base layout, account type, initialization flag and fee TLV.
Exclude reserved multisig length 355 and token-account layouts. The filter
excludes unextended 82-byte mints, which cannot carry transfer fee config.

The [Helius pagination contract](https://github.com/helius-labs/helius-rust-sdk/blob/dev/llms.txt)
requires continuing until paginationKey is absent. Short or empty filtered
pages do not prove completion. Keep pagination keys local and private.

## Required streaming integration

The staged handlers collect native changes, track addresses ever carrying a
fee config, and append their history. Close/InitializeMint events are retained
only for tracked addresses, avoiding ordinary token-account churn. Successful
transactions prove root metadata calls committed, including when logs are
missing. CPI mutations require a completed successful ancestor chain; missing
or truncated proof marks their affected state unknown. Proven rolled-back
mutations are ignored.

Context seeding uses sorted 80-byte records and binary-searches each block
window. Compact state retains known, pending and unknown configurations in
at most 40 bytes. A seed-count store supplements the completion marker: both
the exact expected count and marker must be checked before treating an absent
mint as fee-free. The history remains available to reconstruct state at each
mutation ordinal.

`scripts/gen-token-fee-snapshot.py` refuses incomplete scans and verifies all
retained raw mint bytes against the declared owner, fee fields, read contexts,
record SHA-256 and owned mainnet EpochSchedule. It produces `mint-fees.bin`,
`proof.json` and `snapshot.rs` only after the complete input validates. The
scan integrity digest is an audit record, not a cryptographic ownership proof.

```sh
python3 scripts/gen-token-fee-snapshot.py FINALIZED_SCAN_DIRECTORY \
  --epoch-proof OWNED_EPOCH_SCHEDULE_RPC_JSON \
  --output dex-swaps/fixtures/token-fees/registry
```

Before deployment:

1. Finish a complete finalized owned-mint scan. Retain the original read
   context for every record, validate the page/record proof and epoch schedule,
   and generate the compact deterministic seed.
2. Replay all native changes from the snapshot anchor, including the anchor
   block. Seed each mint at the end of its own read-context block, rather than
   assuming all RPC pages describe the earliest context. Apply each block's
   updates in ordinal order, with end-of-block seeds using i64::MAX.
3. Require an exact completion marker after the maximum read-context slot.
   An absent mint key proves zero fees only after this marker and complete
   mutation replay. A known-but-unproven mint never defaults to zero fees.
4. Wire the new map/store modules into the manifests and resolve history at
   each actual transfer ordinal using compact state. Set map_events initialBlock after the maximum
   snapshot context. No network calls belong in WASM.
5. Verify comprehensive old/new financial parity and a finalized streaming
   window, including pending initialization, closures, epoch boundaries and
   updates earlier in the same block. Warm all new stores before cursor adoption.
6. Publish an immutable subsequent release and deploy through the existing
   single-writer/cursor continuity procedure. Keep production custom limit
   orders disabled and the production API running.

Other decoder gaps remain; this fixture result does not establish universal
swap coverage. No changes here increase Jupiter request rates.
