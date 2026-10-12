# Independent wider finalized originals (.26)

This oracle derives native financial tuples from retained original PB and
finalized RPC data, committed native instructions and exact principal receipts,
program-owned ABI/state, and complete user token ledgers. It does not import
packaged decoder amounts. Its manifest may include originals beyond the latest
partial packaged output; the combined release verifier matches each packaged
addition against an independently derived tuple and finalized containing block.

Run `node independent-oracle.cjs` with bs58, @solana/web3.js and protobufjs
available. Optional CLI argument2 is a dependency-provider package.json;
argument3 provides protobufjs. All proof inputs remain relative to this script.
The checked scope is the retained manifest and proof JSON, not a universal
coverage claim. Owned current pool/vault snapshots establish identity/authority
and immutable mint bindings, not historical reserve balances.

Current retained scope: 41 native originals and 82 complete user ledgers.

Financial role selection preserves native output-first CPI order. sourceTransfer is the earliest physical principal; principalTransfer separately identifies the input-to-vault CPI.
Four RPC runtime truncations have exact prefix and retained-tail ordered-subsequence parity with complete original PB commitment logs; they remain explicitly flagged.
