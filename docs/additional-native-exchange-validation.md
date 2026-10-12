# Additional native exchange validation (.26)

The fixed live original scope is slots 456163419–456164844. These five new
families restore six native exchanges from 6 finalized transactions. Each
independent oracle retains exact original PB/RPC instruction, balance and
runtime parity, deployed program ownership/ELF bytes predating the original,
owned pool/mint/vault evidence and two complete user token ledgers. The current
account snapshot proves typed ownership and identity; it is not a historical
reserve snapshot. There is no universal swap coverage claim.

| Family | Native exchanges | Pinned deployment slot | Owned ELF SHA-256 |
| --- | ---: | ---: | --- |
| Peg | 2 | 421860943 | `8b5f206355a05d6afb46ecc8da134d6812cc0616ec41a84618eb66739b81fd3b` |
| AnchorSwap | 1 | 350360548 | `df48278613f07996935c83e81617c591387816b50aa7e09a4a34a9e7df78160f` |
| RaydiumCpmmFork | 1 | 454595916 | `2c8251b92925ec7d5e3bf3d5bddcbd4aa297a6434537aeaf4bb3ecd77497359e` |
| Exponent | 1 | 453883222 | `42e418676773b3a5f40aea14ee1a8956deee11b80f0bac0a8926f5f27ef4f30e` |
| Janus | 1 | 454313774 | `fbf6e79cfe6d24a8b0faea827c19a78d3148d167b02ad11c0b8a7e0237134551` |

## Runtime contracts

* Peg opcode0: strict17bytes/eightroles, dynamic `quote`+LSTmint pool PDA,
  fixed native authority/SOLvault, two classic principal CPIs. Both persistent
  native vault ledgers bind exact full-transaction receipts, including when a
  user WSOL account closes. Two directions pass; 60 invalid mutations reject.
* AnchorSwap: strict24bytes/15roles, `liquidity_pool`+orderedmint PDA,
  canonical pool-owned vault ATAs and beneficiary output ATA. Three classic
  CPIs distinguish exact input, net output and separate output fee. One
  original and37 invalid mutations pass. Unobserved opposite-order/native
  handler variants are not claimed from this one fixture.
* Raydium CPMM fork: strict24byte SwapBaseInput/13roles, canonical authority
  PDA, two classic TransferChecked CPIs and native-frame170byte SwapEventV2.
  Event pool/mints/input/output/zero transfer fees bind user principals; the
  native pool address may be independently initialized and is not forced into
  an unproved pool PDA. Complete vault ledgers reject payout mutations. One
  original and38 invalid mutations pass.
* Exponent: observed20byte TradePt/29roles, direction PTToSY, Some minimum
  output and None floating spot limit. Exact SY GetState/GetPosition/Withdraw
  plus nested inventory credit, PT debit, net SY receipt, separate treasury
  fee and native290byte CPI event bind the native pool and user. First user
  principal is source+3. The BuyYt ancestor creates claims and inventory; it
  emits no duplicate exchange. Complete ledgers include committed typed PT
  MintTo before the trade. One original and42 invalid mutations pass. RPC logs
  truncate after139 entries: the exact available prefix agrees with the full
  retained165entry original PB runtime, whose ancestor completions are retained.
* Janus: observed12byte opcode7/19roles outcome-to-cash branch. The owned
  executable initializer opcode3 at7715/8245–8264/8316/8537 proves the dynamic
  pool PDA seeds `[authority, predictionMarket, "market"]`; helper8537 creates
  the451byte tag2 owned market. Native opcode7 dispatch15398/15457→9208 checks
  native state ownership. Exact seven-child contract binds user outcome debit,
  Predict merge, two outcome burns, nested cash inventory credit, net cash
  payment and separate cash fee. Complete three vault ledgers include burns.
  The sampled native mint accounts have no transferFeeConfig; prices require
  independently verified gross/net agreement, rather than assuming Token22
  fees. One original and43 invalid mutations pass.

## Financial proof and reruns

The self-contained directories are `dex-swaps/fixtures/swap-coverage/` followed
by `peg-native26`, `anchor-swap-native26`, `raydium-cpmm-fork-native26`,
`exponent-native26`, and `janus-native26`. Each has `independent-oracle.cjs`,
`independent-proof.json`, `cases.tsv`, original `.pb` and `.rpc.json`, raw/parsed
owned accounts and the owned executable/disassembly. Run each oracle with
Node and dependencies bs58, @solana/web3.js and protobufjs; optionally pass
CLIarg2 package.json for bs58/web3 and CLIarg3 for protobufjs. Financial oracle
inputs remain relative to the script directory. Output atoms exclude separate
venue fees; inventory funding is not user proceeds.

## Non-spot diagnostics

`additional-native-classification26/independent-classifier.cjs` retains six
originals and an independently reproducible classification:

* One PoseidonV2 opcode9 is a two-asset liquidity withdrawal. Both PUMP and
  USDC leave canonical vaults owned by the `vault`+pool PDA for the same
  provider. No token payment enters the pool, so their ratio is not a swap
  price. Deployed dispatch2988/3163/3178/3256 validates17bytes;11account native
  branch3733 and PDA helper12912 bind the withdrawal roles.
* Two Overcast RedeemOption calls burn typed option claims and pay a single
  underlying asset. Two MatchOffer calls move premium/collateral and mint
  option claims. The owned executable retains the canonical option redemption
  and offer matching source paths and native typed token CPIs. These are
  derivatives issuance/redemption, not two-asset spot swaps.
* The Janus exchange above is implemented and links to its financial oracle.

The classifier has no missing spot exchange in this six-source subset. This
statement is limited to these retained originals; it does not count liquidity,
issuance wrappers or claim burns as missing spot trades.

## Archived scope expansion

Separate portable wider proof directories retain eight Peg, 29 Guacswap and
five AnchorSwap originals. Their independent oracles reconcile respectively
16, 58 and ten complete user ledgers. Guacswap prices the actual positive user
debit bounded by the instruction's input cap, preserving valid partial fills.

Three AnchorSwap originals execute the reverse exchange: the first committed
CPI pays base output from vault4 to user6, followed by quote input from user7
to vault5 and a separately declared quote fee from user7 to the beneficiary
ATA9. Both quote debits sum exactly to the native request. The user's price
includes that full debit and only the base receipt; the earliest physical
output transfer is the source transfer position. Canonical pool PDA, ordered
base/quote mints, owned vault/beneficiary ATAs and both persistent vault ledgers
remain mandatory. This extends the pinned native handler's observed financial
scope; it does not add a heuristic swap decoder.

Archived AnchorSwap core tests preserve all five original financial tuples,
including three real reverse trades. The reverse profiles reject72 malformed
native transactions and24 invalid routed candidates; three valid routed
candidates preserve the exact gross user debit, net output and earliest
physical position. The original forward fixture and37 mutations remain in
the prior core test.
