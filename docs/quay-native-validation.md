# Quay native settlement

Protocol 62 decodes the observed opcode-32 exact-input contract at
`QUayE6nexQWYNZAEqfN8FxoNwQDSu3CAzT2qq9J1ArG`. The finalized owned
executable was deployed at slot 442062012; its ELF SHA-256 is
`06e35418a8bb498deecdb85ceeced6c69dccf3b5e7a759fda59b020450a365ec`.
The fixture directory `dex-swaps/fixtures/swap-coverage/quay-native` retains
96 original Firehose transaction protobufs, finalized RPC responses, owned
account evidence, an independent oracle and exact expected atom pairs.

The native instruction has exactly 18 bytes and 14 accounts. Its two direction
values select pricing math; the declared token accounts are already in
input/output order in both directions. Request and minimum-output fields are
checked against the two immediate committed classic Token TransferChecked
receipts. Native and token account roles, mints, decimals, vault owners, source
positions and runtime ancestry must agree. Pool/configuration and authority
account formats were independently checked against finalized owned bytes.
Transaction token snapshots also bind each physical vault to its mint, owner,
classic Token program and checked decimals.

All 96 observed swaps agree with the independent native request, principal
receipts and available whole-transaction user/vault conservation checks (381
ledgers). Both directions are retained. Core tests check all originals and
31 malformed payload, account, amount, snapshot and commitment mutations in
each direction. Missing evidence, extra principal transfers, unknown payloads
and uncommitted execution remain rejected. The bounded corpus does not prove
support for unobserved or future Quay instructions.
