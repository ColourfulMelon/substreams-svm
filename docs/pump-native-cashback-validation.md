# Native Pump V2 cashback, suffixes and account lifecycles

This evidence covers 19 finalized legacy/V2 native sell originals retained in
`dex-swaps/fixtures/swap-coverage/pump-native-cashback/`. It does not classify
creation/migration instructions as swaps or widen ambiguous repeated payouts.

The original RPC instruction/account/data walk, full runtime logs, lamport
snapshots and token owner/program/mint/amount snapshots match the original PBs.
The independent financial oracle validates 168 native account ledgers and 38
principal token source/vault ledgers. `proof.json` includes each exact financial
row, original fingerprints, native state/fee witnesses, refunds and owned reads.

## Cashback

The pinned primary Pump ABI places the native V2 user-volume accumulator at
account 19, with seeds `["user_volume_accumulator", user]`. Seven finalized
program-owned accumulator reads independently validate its discriminator, user
and canonical PDA. The decoder recomputes the PDA and binds the account to the
same committed V2 sell and its exact physical native credit.

TradeEvent's bounded instruction-name extension supplies the cashback rate and
amount. Cashback mode has zero creator fee/rate, and its exact native fee return
routes the creator component to cashback: both the rate and ceiling-rounded
amount must agree. Net proceeds subtract protocol, creator and cashback amounts.
The accumulator credit, wallet proceeds, curve debit and all fee recipients must
match physical ledgers exactly. Old events without the extension retain zero
cashback. Historical creator-fee sells retain their existing calculation.

## Other bounded forms

Three V2 originals have 26 canonical accounts plus one inert remaining account.
Only that exact 27-account form is added. The suffix must alias no canonical
role, have no token identity or child accesses, and have equal pre/post lamports.

Five sell-to-buy cycles share the same wallet across a legacy native sell and
an exact-SOL-input buy. The buy's token output and four explicit native payments
are independently bound to its canonical roles and committed TradeEvent before
the buy may participate in the sell wallet's whole-transaction ledger.

Canonical volume initialization is bound to its PDA, payer/user event and its
optional exact System creation. Temporary WSOL token transfers and closes use
actual recorded funding and native balances, including the original remainder.

Two wrapper remainders are created and closed in the same transaction. Exact
System creation, or Transfer→Allocate→Assign, proves the owner and original
funder. Exclusive account accesses bind creation to its owner-program close;
all other parent/close accounts balance against independently recorded flows.
The refund is the exact original funding, without a guessed rent amount.

One retained D3og owner-state leaf close has an exact program/data/three-role
shape. The funded state is accessed only by that committed CPI-free root and
ends at zero; its token-source role was already closed by SPL. The runtime's
owner-only debit rule therefore binds the state debit to the executing owner,
and conservation leaves the declared seller wallet as its sole recipient.
Both closed accounts and the exact refund are checked in the native ledger.
This permits only that metadata access to an already-zero token source.

## Reserve bounds and validation

Eighteen originals have unique native post-reserve inversions. The remaining
single sell has two possible pre-SOL reserves, differing by one lamport. Its
price is independently proved by its physical curve/wallet ledgers once every
other native contribution is bound. Its inverse is not marked independent;
repeated-user payouts still require unique per-leg reserve proofs.

Core business tests retain all 19 original sell amounts and reject changed
curve balances, gross/fee claims, account roles and token inputs. Dedicated
cashback mutations cover recipient balances, amounts, PDA roles, fee returns
and truncated runtime provenance. Existing repeated-sell tests retain all 22
earlier legs and reject ambiguous or redistributed witnesses.

Primary contracts: [pinned Pump ABI](https://github.com/pump-fun/pump-public-docs/blob/2293f9a66c654e9fe82dc5e8f4618538f24bb35f/idl/pump.json),
[Solana account modification rules](https://solana.com/docs/core/accounts).
