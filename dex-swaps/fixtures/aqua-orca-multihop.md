# Orca multihop regression

`aqua-orca-multihop.transaction.pb` encodes the confirmed public Solana transaction
`2ugHpyVU2mTvwz6Q5gYK8RY9Np61iabi3exohoTxdwU3vsJbpALNg6iz76HJBxhQysCPbhrDjp1s4YPFL6RREe3H`
at slot **454558265** (2026-10-08 13:28:35 UTC), in the Substreams Solana
`ConfirmedTransaction` format. It was fetched using RPC `getTransaction` with
`encoding=json`, `commitment=confirmed`, and `maxSupportedTransactionVersion=1`.

The successful transaction has three consecutive Orca CPIs:

| Pool | Input atoms | Output atoms |
| --- | ---: | ---: |
| FVB6knS78TJJswCrBhhB2T3cUKBQ8XyRvqwsjDq3oWwE | 273199007 WSOL | 53097662110 FKU |
| GNzK4LP1nbfFcMNwB1zG4htCkToXjKYn5mn5yVKvaExm | 53097662110 FKU | 2214641291345 AQUA |
| 9TT3NbWogWjbQh7b3s1hfcqYF7VJHmg8uNcXDjtPRnh4 | 2170348465518 AQUA | 276039217 WSOL |

The first legacy Swap instruction contains an extra trailing zero byte. A strict
IDL `try_from_slice` rejects it. Sequential event matching then associates the
first two events with the second and third pools, drops the real third event,
and reports AQUA atoms as WSOL. Pool/direction matching prevents this corruption
even when a preceding instruction is unsupported; fixed-prefix legacy decoding
also preserves all three actual hops.
