# Aldrin V1 and Cropper native swaps

Finite finalized scope: four Aldrin V1 Ask swaps and two Cropper swaps from slots 456214149–456214306. `verify-originals.cjs` verifies all six original transaction PBs against finalized transaction RPC, including complete account/instruction/fee/balance parity and committed native ancestry. It derives exact principals from typed physical receipts and reproduces them with historical pre-swap vault constant-product arithmetic. No indexed output supplies oracle amounts.

Aldrin's requested input is a cap: ceiling division can leave atoms unspent. Its LP fee is minted separately and is excluded from the user swap principal. Cropper transfers its owner fee directly from the user's input account; the indexed input includes both pool debit and fee debit. The proof checks 30 complete user/vault/fee token ledgers and 14 WSOL native ledgers. Three temporary WSOL accounts are created, explicitly funded, synchronized and closed with 4/7/0 atoms of remaining wrapped balance; those lifecycle operations are included explicitly.

Primary contracts are pinned by the published official Aldrin SDK 0.4.51 archive (including npm integrity and exact extracted file parity), official Cropper instruction/state source, and both owned loader→ProgramData executables deployed before the originals. `primary-source.json` identifies source pins; `aldrin-primary-dispatch.json` pins deployed bytewise swap dispatch and typed argument/core selection. Owned market state binds mints, vaults, LP mint, fee account and signer PDA. Current fee metadata was read after the originals and is **not** claimed as a historical snapshot; exact original vault/receipt arithmetic independently agrees with its 20/10 basis-point constant-product profile. Canonical SPL curve source is retained as an arithmetic reference, not a reproducible build claim for the deployed Cropper program. Exact LP fee mint receipts and the whole fee-account ledger are proved, without claiming historical LP mint supply conversion arithmetic.

Rerun:

```sh
node verify-originals.cjs /path/to/backend/package.json /path/to/protobufjs/package.json
cargo test -p dex-swaps legacy_amm -- --nocapture
```

The Node package must resolve `bs58` and `@solana/web3.js`. The sibling `combined26/original-metadata.cjs` and `solana-type.proto` are required; their bytes are pinned in `sha256.json`. `cases.tsv` is generated only by the independent oracle. The core test verifies six exact native financial tuples and rejects 216 malformed payload/layout/PDA/receipt/bound/commitment/vault-ledger variants. No containing-block metadata or universal future-format coverage is claimed.
