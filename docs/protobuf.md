# Substreams and protobuf development

The workspace uses Substreams 0.8, Solana 0.16, database-change 5 and Buffa 0.9.2.
These are the stable releases checked on 2026-10-17. New modules inherit the
workspace dependencies instead of introducing an older SDK or a Prost-generated
output model. The existing `.proto` files remain the wire contracts; Buffa changes
the Rust implementation, not the protobuf format.

The migration follows the public
[Substreams Rust SDK release guidance](https://github.com/streamingfast/substreams-rs/releases),
[ABI 2.0 migration](https://github.com/pinax-network/substreams-abis), and
[EVM balances generation configuration](https://github.com/pinax-network/substreams-evm-balances/blob/8275bce35bcba61ff7ecc37167861d7dd710aa46/proto/buf.gen.yaml).
Check current upstream releases before the next dependency upgrade rather than
treating the dated versions here as permanently current.

## Generate the shared messages

Run `make -C proto protogen` with Buf and Python 3 installed. The committed Buf
configuration pins `buf.build/anthropics/buffa:v0.9.2`, generates owned, view and
lazy-view models, and disables unknown-field retention to match the previous
output model's discard behavior. `generate-modules.py` writes the module tree
from every generated package, including aggregate and protocol-specific types.
Commit the generated files with the schema or generator change. Do not hand-edit
the generated Rust files or depend on an operator's local Substreams CLI choosing
the same protobuf plugin implicitly.

An upgrade must regenerate cleanly, compile the full workspace for WASM, and
preserve existing field numbers, oneof members, enum values and package names.
Regenerating bindings alone is insufficient: all callers must use the matching
runtime and message model.

## Preserve presence, enums and bounds

- Singular messages use `MessageField<T>`. Use `as_option()` when an absent
  message previously caused rejection, `None`, or an error. Implicit dereferencing
  supplies a default value and can conceal malformed input.
- Enum fields use `EnumValue<E>`. Construct known values with `E::Variant.into()`;
  inspect `as_known()` or `to_i32()` according to the existing unknown-value policy.
  Do not coerce unknown protocol values into a supported decoder.
- Protobuf oneof message variants own boxed payloads. Preserve their selected
  variant and presence when converting constructors.
- Optional scalars remain `Option<T>`. Where an old generated getter defaulted an
  absent numeric value to zero, use `unwrap_or_default()` explicitly.
- Buffa's element-memory budget is distinct from encoded message length and from
  total process memory. Native ingestion callers must set and test their own
  bounded decode policy; do not copy a large offline decoder limit blindly.

Views and lazy views are available for new handlers. Switching an existing
decoder to a lazy view is a separate behavior/performance change: propagate
accessor decoding failures and validate it against captured wire fixtures.

## Keep native projectors reusable

`dex-swaps` can be built with `default-features = false`; the Substreams handler
and logging integration are optional while native callers use the public Solana
core model. The IDL library owns Borsh account/instruction/event decoding and
must not pull a Substreams WASM runtime into native consumers just for an error
type or a base58 macro.

The SPL projector accepts a borrowed structural view and has no protobuf runtime
dependency. Its Buffa adapter preserves message presence and explicitly rejects
unsupported transaction versions or inline transaction configuration; recognizing
new upstream model fields does not qualify new transaction coverage.

Preserve captured legacy output bytes alongside native projection and transport
tests. Successful compilation does not prove unchanged client payloads or live
source qualification.
