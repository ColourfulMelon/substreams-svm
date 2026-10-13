# Patch buyback and Bxlw SellTokensV2 observed profiles

Two finalized original exchanges are independently proved from original protobuf bytes and gzip-compressed raw finalized JSON-RPC response bytes. The oracle checks full keys/walk/depth/accounts/data/signature/slot/fees/native and token balances/runtime parity, then derives the complete financial tuple from physical source-bound token receipts and the committed native event. Every resolved native account and all token principals conserve. Bxlw's transient WSOL output is created/initialized and closed; actual rent plus its net token receipt is refunded to the owner.

Amounts are not copied from the decoder. Patch buyback uses treasury custody (role3) as the economic user, rather than keeper0. Its actual SOL input is2atoms and actual base receipt296atoms. Bxlw input is2,884,643,660,346atoms; source min260,259,115, netSOL output265,570,526; native event gross268,253,056 minus fee2,682,530 equals that physical net receipt.

The adjacent Patch IncreaseLiquidity call is separately retained and classified in primary-contract.md. Primary executable/program-data provenance, pinned deployed slots, disassembly and precise argument pointers are retained. This is bounded support for the two observed profiles, not universal program coverage.

Run `node oracle.cjs /path/to/backend/package.json /path/to/protobufjs/package.json`. `independent-proof.json` pins each original hash and independent tuple/whole ledgers. `cases.tsv` is the financial core oracle. The existing module test mutates native request/minimum/trailers/roles, receipts, source events, runtime ancestry and final vault balances; malformed native and routed candidates both reject.
