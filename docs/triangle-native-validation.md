# Triangle native receipt validation

All **119** uncovered Triangle swaps in finalized slots **456081600–456085680** independently match exact pool/user/mints/atom amounts/source indices and **238 complete Token-2022 user ledgers**. Eight buy/sell/outcome/conversion shapes pass264 invalid mutations and32 allowed priority invariance checks.

The primary deployed executable is program `Tri3NG4HkZ6DddYPKoX2ehgkqFtDuej9Aspw5BmvSo4`, ProgramData `45i3gh5ey3JmXMZEnNCX4KKcivjNkTauVFrHcS3zSiCb`, deployment slot **452701164**, ELF SHA256 **e366e2f33caa37d6ae75e00b773b189cb2b8c612cfeb20d9d205def67770cbf8**. Owned loader account state, ProgramData payload and extracted ELF agree. The deployment predates all original calls. The retained primary handler evidence records dispatch5179, native handler3931, group-owner check4110 and typed outcome/vault binding4242–4295. The strict ABI is opcode4,13 bytes,17 accounts, bounded bools and priority0–4, with exact positive requested principal.

**81** originals invoke typed Predict complete-set mint/merge operations between the two direct user principals. Their nested collateral transfer and outcome mint/burn quantities are excluded from price. The remaining38 use existing inventory. Every priced user principal has the exact authority, mint, vault, account order and immediate depth; whole Token-2022 ledgers include nested mint/burn operations without conflating them with exchange proceeds.

118 original RPC traces prove full native commitment. One RPC trace is explicitly truncated; its exact retained prefix agrees with the complete original protobuf runtime trace. This exception is recorded per original, not silently treated as complete RPC evidence. Current closed outcome groups are not asserted to retain historical reserves: the immutable pre-existing executable's successful owned-group checks establish the historical native binding.

Fixtures, original RPC/PB/inspection, executable provenance, independent oracle and exact cases are retained in [`triangle-native`](../dex-swaps/fixtures/swap-coverage/triangle-native). These bounded results do not establish universal swap coverage.
