# Pump gross-to-net price corrections

The combined decoder corrects 825 existing Pump SellV2 financial rows: 738 in
the 4,081-block wide baseline and 87 in the repeated baseline. Their source,
principal positions, pool, user, input and mint pair are unchanged. Earlier
outputs used gross SOL event proceeds; the corrected outputs equal the actual
user proceeds after protocol, creator and applicable cashback charges.

Every correction independently agrees with finalized RPC and the original
`solana-common-v0.4.0` source transaction. The retained protobufs are canonical
re-encodings of the official source JSON with base64 bytes, rather than original
wire bytes. Exact signature, resolved account, instruction, runtime, fee and
native/token snapshot parity is checked. Both completed source streams, the
immutable source package, official schema and re-encoders are pinned by hashes;
each original source JSON transaction is retained separately.

All 825 bounded 27-account native calls have an independently inert suffix.
The exact committed native fee CPI returns 0/95/30 basis points. Canonical native
post-reserves and actual token input yield a unique integer inverse for 781
sells. The other 44 each have one sell, whose complete physical pool and user
ledger proves the individual proceeds. No ambiguous repeated allocation is
accepted. Two zero-creator native states charge no creator fee despite the
nominal returned rate; their complete physical ledgers establish that zero
charge. One cashback recipient has independently verified canonical PDA,
program ownership, initialized account discriminator and user identity.

All 6,592 complete native ledgers and 1,650 principal source/vault token ledgers
match. Two unrelated temporary WSOL accounts omit token snapshots; their exact
native funding, transfer and closure ledgers match, and their absent token
snapshots are not used as principal evidence.

The originals, source JSON, finalized RPC responses, independent financial rows,
owned cashback evidence and per-transaction witnesses are retained in
[`pump-gross-net-corrections`](../dex-swaps/fixtures/swap-coverage/pump-gross-net-corrections).
These are explicitly proved corrections, rather than arbitrary exceptions to
the existing-row preservation check. No additional runtime change was needed.
The evidence is limited to these finalized corpora and does not prove universal
historical or future coverage.

Nine additional Riptide swaps in the repeated baseline also agree with exact
finalized RPC/original protobuf instruction and balance parity, committed native
principals, primary market PDA/mint identities and all 36 principal ledgers.
Each complete financial tuple matches the host decoder exactly once. Their
separate retained evidence is in
[`riptide-repeat-native`](../dex-swaps/fixtures/swap-coverage/riptide-repeat-native).
