# Jupiter custody swaps and Numeraire native validation

The finalized live audit covers slots 456163419–456164844. The public fixture directory `dex-swaps/fixtures/swap-coverage/custody-numeraire26` contains 25 original protobuf transactions, finalized RPC responses, raw and parsed owned account metadata, program-owned IDLs, and the independent oracle. The oracle derives 25 financial tuples and reconciles 50 complete user account ledgers, including wrapped-SOL creation, sync, closure, and other transactions' sibling flows. Original keys, instruction walks, token balances, lamports and complete invocation logs agree between RPC and protobuf.

Jupiter Perps contributes 20 `instantIncreasePositionPreSwap` and four `swapWithTokenLedger` spot exchanges. Each has exactly two immediate classic-token principal transfers and one committed `PoolSwapEvent` CPI. The event binds both custody keys, pool, owner, receiving account, actual debit and output after pool fees. Program-owned custody state binds pool, mint and token vault; owned pool state contains both custodies. The token-ledger cases retain the preceding committed `setTokenLedger` source for historical provenance. Current ledger amount/account fields are mutable and are never used as historical swap input. Other position operations and collateral conversions are excluded.

Canonical `swap2` has the same declared financial event and a separate typed 17-account layout; its supplied Pyth accounts shift custody/vault roles. No finalized `swap2` example occurs in this audit, so finalized financial evidence applies only to the observed 24 sources. Runtime account roles and commitment remain mandatory.

Numeraire contributes one `swap_exact_in`: USDC 513983 atoms to USDG 513892 atoms. Its canonical 11-account checked transfers, scoped `SwapExactIn` event, owned 4024-byte `StablePool`, repr(C) 368-byte virtual pair records, config and initialized vault authorities all agree. Input/output indices 0 and 2 bind their declared pool records. The output mint has no transfer fee. The strict current runtime requires output gross, recipient net and event output to agree; fee-bearing Numeraire receipts remain excluded until the native event's fee semantics are independently established. Hinted and exact-output layouts follow the owned IDL but have no finalized financial example in this audit. Quote-only instructions are excluded.

Core tests exercise all 24 Jupiter originals plus 120 invalid mutations across four observed opcode/direction shapes, and the Numeraire original plus 29 invalid mutations. Both passed. Mutations cover native widths, amounts, pool/custody/program roles, transfer roles/depths, event identity/amounts and failed or missing runtime commitment.

Rerun the oracle with Node and dependencies `bs58`, `@solana/web3.js` and `protobufjs`:

```sh
node dex-swaps/fixtures/swap-coverage/custody-numeraire26/independent-oracle.cjs [dependency-package.json] [protobufjs-package.json]
```

This proves the enumerated originals, not universal or future venue coverage. Current metadata is used only for typed immutable mint/vault identity and applicable mint fee schedules; no historical reserve claim is made.

## Archived packaged replay expansion

The archived original corpus adds a separately retained independent scope under
`wide-custody26` and `wide-numeraire26`. Jupiter Perps has 123 native sources,
including finalized canonical `swap2`, with 246 complete user ledgers. Numeraire
has 41 canonical exact-input originals across owned pools and both index
orientations, with 82 complete user ledgers. Thirty-two Numeraire originals
transfer output before input; financial roles identify the two principals,
while the earliest committed physical transfer fixes the event position.
The same scoped native event, exact input/minimum and two unique checked CPIs
remain required. No tolerance or quote-price inference is introduced.

Four Numeraire RPC responses truncate runtime logs. The exact pre-truncation
prefix and any retained tail agree, in order, with complete original protobuf
logs and their committed invocation ancestry. Each limitation is explicit in
`rpcRuntimeTruncated`; RPC log completeness is not claimed. All original keys,
instructions, token balances and lamports still match. Primary owned mint
extension parsing retains historically applicable transfer-fee schedules;
these observed native receipts satisfy the strict event/net receipt contract.

The Numeraire executable is retained with its owned upgradeable Program and
ProgramData accounts. Deployment slot329553166 predates every archived
original; ELF SHA256 is
`f45357b0b8273edacf1ca3f497edf7c7a008fc3896de4f477d6a9bf9e31f5e04`.
The portable oracle rechecks loader tags, ProgramData address, owner, ELF bytes
and deployment-slot ordering, alongside the program-owned IDL.

Archived core tests preserve all41 originals and reject768 native malformed
variants plus256 invalid routed candidates across32 output-first originals.
Their32 positive routed candidates normalize to the same native protocol,
financial tuple and earliest physical position. The prior finalized Numeraire
fixture remains independently exercised by its original core test.
