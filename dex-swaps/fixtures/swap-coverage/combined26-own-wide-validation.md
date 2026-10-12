# Independent wide-original financial validation

The three retained wide-original families prove 288 native financial legs independently of packaged decoder output:

| Family | Finalized originals | Complete principal token ledgers | Complete native ledgers |
| --- | ---: | ---: | ---: |
| `current-proprietary-wide26` | 232 | 928 | — |
| `rfq-wide26` | 29 | 92 | 150 |
| `anchor-suffix-wide26` | 27 | 108 | — |

The proprietary scope comprises 158 Deriverse, 18 Humidifi, 52 Tesserav, and four GoonFi swaps. The suffix scope comprises 24 Raydium CPMM and three Meteora DBC swaps. Every portable `verify-originals.cjs` derives financial amounts from typed primary native calls, exact committed physical receipts, fees, and complete account ledgers. Packaged rows select original identities; they never supply the oracle amounts. Each directory retains its original transaction PB, finalized transaction RPC, primary program/state evidence, and independently generated proof.

One proprietary original has truncated RPC logs. Its complete PB logs authenticate all committed calls; the oracle checks the exact RPC prefix and the exact ordered retained tail against that complete PB. The proof explicitly records this limitation. Fresh owned state evidence establishes program/account identity and does not claim historical reserve snapshots.

`combined26-own-wide-proof-manifest.json` pins each oracle and proof hash. These proofs do not attest a package or global swap coverage. The separate combined packaged verifier must validate the final package hash, completed replay, preserved prior rows, all added/corrected financial fields, and original transaction/containing-block metadata before release.

Run an oracle from its directory with Node and pass the available backend package manifest and protobufjs package manifest as its two arguments. No live RPC call or database write is performed by these portable oracles.
