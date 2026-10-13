# Exhaustive Pump diagnostic classification, .27 audit

All 233 Pump/PumpSwap diagnostic sources in slots 456213447–456214372 are individually classified from original protobuf transactions, primary instruction discriminators/roles and complete successful runtime ancestry. This is 231 transactions. No instruction is excluded merely because it has a mint or burn.

The 120 CreateV2 calls mint initial supply into curve/mayhem custody. Six CreatePool calls deposit both assets and mint LP shares. Two MigrateV2 calls transfer curve custody into a new AMM pool, mint LP shares and burn those shares. Four Withdraw calls receive both assets and burn LP shares. These 132 sources are creation/liquidity operations, rather than spot price observations. Their exact token receipt, mint/burn roles and original source hashes appear in `classification.json`.

The remaining 101 actual price observations are 29 native sells and 72 quote purchases followed by base burns. Their full finalized RPC/protobuf parity, physical/native financial proofs and complete account ledgers are independently checked in sibling `pump-roundtrip27`, `pump-native-sell27` and `pumpswap-boost-burn27`. The latter explicitly records protocol 79 burn output rather than trader credit.

Run `node classify.cjs /path/to/backend/package.json /path/to/protobufjs/package.json`. The classifier decodes every retained original and revalidates primary role patterns and ordered full runtime. The classification is scoped to this finite diagnostic corpus; it is not a claim of universal swap coverage.
