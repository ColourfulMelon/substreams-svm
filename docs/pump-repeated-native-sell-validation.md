# Repeated implicit-native Pump sell settlement

The retained finalized sample contains 22 ordinary native sells in eight
transactions, two or three sells by the same user into the same curve. The
previous one-payout guard quarantined every leg because one aggregate user
balance cannot allocate individual proceeds.

The new path binds the canonical native parent, its exact input TransferChecked,
its canonical committed self-CPI TradeEvent, and its direct pump-fees invocation
and runtime return. The state witnesses are the native event's **post-trade
virtual SOL reserve** (full self-CPI bytes 105..113) and **post-trade virtual
token reserve** (113..121). Event `sol_amount` and claimed output do not determine
the price. The current primary ABI is pinned in the fixture proof.

For exact sold input `I`, post-token reserve `T1`, and post-SOL reserve `S1`,
`T0 = T1 - I`. The canonical floor-rounded sell is
`gross = floor(I * S0 / T1)` and `S1 = S0 - gross`; equivalently,
`S1 = ceil(S0 * T0 / T1)`. Therefore the only possible pre-SOL reserves lie in

```
low  = floor((S1 - 1) * T1 / T0) + 1
high = floor(S1 * T1 / T0)
```

Accept only `low == high`, positive reserves/input, and checked arithmetic.
Recompute the gross output and ceiling-rounded protocol/creator fees from the
exact 24-byte fee return (`lp_fee_bps`, `protocol_fee_bps`, `creator_fee_bps`).
Require LP fees zero and total protocol/creator fees at most 100%. The scoped
fee CPI must be the exact 57-byte native `get_fees_with_quote_mint` form with
native flag true and zero quote mint, correct fee-config and Pump program
accounts, committed runtime ancestry, and one direct fee invocation before the
principal transfer. Duplicate, foreign, malformed and truncated fee returns
cannot supply a witness.

Every contribution by a repeated user must independently prove its proceeds.
All previous exact whole-transaction user, curve, protocol, buyback and creator
lamport ledgers, input token conservation, exclusive role/access checks, and
runtime guards still apply. Ambiguous inverse intervals remain diagnostics;
aggregate balances do not choose a candidate. The original single-payout path
remains available under its existing independent physical-balance guards.

No mutable global reserve defaults, mint supply, historical registry, or
physical-token-vault offset enter the decoder. Direct donations to a physical
vault or curve do not change native virtual reserves. A regression adds prior
donations to both physical snapshots and confirms identical per-leg prices.

The independent oracle compares finalized original RPC and retained protobuf
execution walks, complete runtime logs, lamport and token snapshots. All 22
inverse intervals are unique, with exact committed fee returns and all 56
physical native account ledgers and 16 principal source/vault token ledgers
matching. The proof has no supply registry or physical-vault offset dependency.

Fixtures and full proof are in
`dex-swaps/fixtures/swap-coverage/pump-repeated-native/`. Core tests cover all
original legs, 72 negative mutations, prior donations, unknown fee metadata, ambiguous inverse
rounding, invalid amounts/reserves/rates and overflow. This sample establishes
this format's coverage; it does not claim every possible future Pump swap or
ambiguous inverse can be verified.
