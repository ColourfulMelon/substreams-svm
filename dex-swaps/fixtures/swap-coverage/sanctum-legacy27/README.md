# Sanctum Infinity legacy exact-input swaps

Three finalized original transactions exercise native opcode 1, with the original 12-account prefix. The protocol fee accumulator adds one account before the Token programs, so pool, list, vault and calculator roles shift by one relative to opcode 23. The fee accumulator must be the canonical output-mint ATA of the program's protocol-fee PDA. INF issuance/redemption (sentinel LST indices) and exact-output opcode 2 remain excluded by this profile.

The native request, nine committed calculator/pricing/Token children, actual principal amounts, program-owned state/list, classic mint/vault metadata and complete transaction token ledgers agree independently. The three observed financial rows and nineteen token-ledger checks are retained in `independent-proof.json`; original PB, finalized RPC and exact raw RPC responses are retained. The first routed RPC has truncated logs, explicitly recorded; its retained prefix and ordered tail match the complete original PB runtime. This finite corpus does not prove universal future coverage.

The decoder core test checks three actual swaps and ninety malformed native plus ninety malformed routed candidates. All prior opcode-23 rules remain intact.

Run `node verify-originals.cjs NODE_DEPENDENCY_PACKAGE_JSON PROTOBUF_PACKAGE_JSON` to reproduce the independent proofs. It performs no network calls or database writes. `primary/` retains the pinned native instruction and account contracts from igneous-labs/inf-1.5.
