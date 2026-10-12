# Riptide native exact-input settlement

The decoder binds the publisher's `riptide-amm` 2.1.1 SwapExactIn ABI to the
native program `riptK81hDxhe5pW5jSzSM9iRA8azgEgLJ4dXkPtBS7j`.
The published crate SHA256 is
`7ff4b3eb277827544fce2c29ef976053f182485c2e8b8a7637de0c6719db2553`,
verified against the publisher's version checksum. The crate VCS metadata has
a dirty marker; the crate digest identifies the exact contract source.

Opcode 2 contains a u64 input, a bounded direction boolean, the complete
SlippageTolerance enum (None, u128 maximum execution price, or u64 output
threshold), then a bounded partial-fill boolean. Lengths are exactly 12, 28 or
20 bytes. Unknown tags, trailing bytes and zero input fail closed. The native
program enforces its slippage guard; indexed price and amounts come from the
actual committed token receipts. Exact-output opcode 3 remains unsupported
until an original settlement format has independent evidence.

The twelve primary roles bind authority, market, two mints, two users, two
vaults, their respective token programs, the Memo program and instruction
sysvar. The first eight roles cannot alias. An optional thirteenth account is
restricted to either retained Jito marker. It must not alias a role, have token
identity, appear in either child CPI or change its native balance.

Settlement requires exactly two direct committed token children in the
specified direction: user-to-market-vault under user authority, followed by
market-vault-to-user under market authority. TransferChecked and the exact
Token-2022 TransferCheckedWithFee form bind mint, decimals, token program,
amount and authority. Every original vault snapshot must identify the primary
market owner and declared mint/program. Exact fills equal the input argument;
partial fills must be positive and bounded by that argument. Fee withholding
is included in the existing physical conservation check and output proceeds.
No price is inferred from an oracle, threshold or router aggregate.

The fixture proof records finalized RPC and original protobuf account,
instruction and balance parity, a shared exact log prefix and independently
committed native ancestry in each record. A finalized successful root can
commit already-completed inner frames before an explicit log truncation;
uncompleted inner frames cannot supply receipts. Whole principal token
ledgers are checked, and WSOL lifecycles use actual native funding, transfers,
closes and recipient balances. Program-owned market reads independently bind
the primary market PDA and mint roles; owned mint reads verify initialization,
token program and decimals.

One routed original uses the primary SPL Token Batch opcode 255 for two
legacy Transfer children. Strict batch framing, sequential account slicing,
committed ancestry and source/destination mint/program snapshots supply only
conservation credits and debits. Unknown child forms, nested batches and
incomplete framing retain the unknown-operation blocker. Native principal
positions continue to refer to actual individual instructions. This extra
conservation support was not required to recover the 388 Riptide legs; the
pre-batch host already emitted all their exact financial tuples. The primary
Token revision and source fingerprints are pinned in the fixture proof.

The core business test reads every exact financial tuple in `cases.tsv` and
rejects malformed data, wrong roles, mutated principal instructions/decimals,
unknown ancestors, failed transactions, fee claims and altered vault metadata.
The sample's final scope and fingerprints are recorded in `proof.json`; it
does not establish universal coverage of unknown future Riptide formats.

Primary contract: [published SDK](https://docs.rs/crate/riptide-amm/2.1.1/source/),
[publisher version](https://crates.io/crates/riptide-amm/2.1.1).
