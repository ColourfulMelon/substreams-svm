# DAMM legacy suffix settlement

The observed DAMM v2 legacy exact-input instruction has its canonical 24-byte
prefix plus one of four bounded suffix shapes: a 25th boolean byte (zero or
one), an all-zero 32-byte form, or an all-zero 41-byte form. Arbitrary trailing
bytes are rejected. The primary contract is pinned to
[`MeteoraAg/damm-v2@a85c926607433f23f0ea60f4ca7b1ae92f4156cb`](https://github.com/MeteoraAg/damm-v2/tree/a85c926607433f23f0ea60f4ca7b1ae92f4156cb).

All 217 observed calls across 196 original finalized transactions and 73 pools
are retained in `dex-swaps/fixtures/swap-coverage/damm-legacy-suffix` with RPC,
Firehose protobuf, exact atom expectations, independent oracle and owned fee
mint evidence. The current committed EvtSwap2 binds the declared pool,
requested amount, minimum, direction, exact-input mode, zero leftover and
gross/net output to the typed principal token receipts. Pool substitution or
unrelated events cannot supply an otherwise valid routed candidate.

The primary user output excludes the optional referral payment, including
same-account referrals. Collect-fee mode zero uses the output mint; modes one
and two use token B. Original owned raw and parsed Token-2022 fee schedules
independently reproduce net outputs. Mutable fee authority is retained in the
evidence; a current account alone does not prove a historical schedule. The
canonical event and all available full transaction user ledgers must also
agree. There are 375 available user ledger checks and 23 separate fee outputs.

Core tests compare all 217 complete financial legs, preserve the canonical
24-byte prefix behavior, reject malformed headers and suffixes, and reject
pool/event/principal substitutions across all four shapes. These observed
suffixes require complete committed event and principal evidence; unknown
suffixes or unproven receipts remain diagnostic.
