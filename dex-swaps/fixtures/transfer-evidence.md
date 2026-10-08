# Transfer-evidence regressions

Both fixtures are successful public Solana `getTransaction` results (`encoding=json`,
`commitment=confirmed`, `maxSupportedTransactionVersion=1`), encoded in the
Substreams `ConfirmedTransaction` protobuf format.

- `pumpfun-native-sell.transaction.pb`: signature
  `3wNPnvjQDZdYqFyEQ39C5SKfPhwSjNXdhF8zoo9L7AA5FJrEvhkzSZuP4gN6Gs5RUeGbhaauP9vK8br61uxAo71M`,
  slot 454558265. The program debits 33,070,963 lamports directly from its
  bonding curve, and the Token-2022 CPI transfers 4,013,858,443,174 input atoms.
  The sole pool-bearing invocation and exact pool balance debit prove the
  native leg, without treating transaction-wide user balance changes as swaps.
- `raydium-transfer-fee.transaction.pb`: signature
  `5ZwQBAE9jqczSkvFr4TTBp4FNrHYnD78uTgGpArsCtp1J3x9k5tu7KPL2aeJ8wJWLScX3yFifWNGmp9Egz5KxUAx`,
  slot 454558278. The CPI transfers 279,190,392 Token-2022 atoms; the recipient
  receives 270,814,680 after withholding. Account initialization must not
  invalidate that exact receipt evidence. Changing the decoded output by one
  atom makes verification fail.
