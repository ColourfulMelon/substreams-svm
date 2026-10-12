# GatorSwap finalized native proof

Fixed window: slots456163419..456164844. All57 retained calls settle positively:53 signed113-byte and4 plain41-byte payloads; account counts15/17/18/36. The retained original Firehose transaction PBs are unchanged raw source bytes. Each finalized getTransaction response is retained independently and cross-checked against every account key, instruction payload/accounts/depth, and native balance. Runtime commitment is bound to the raw source trace; one shorter RPC log trace does not replace the complete Firehose trace.

Primary authority is the program-owned deployed SBPF-v2 executable, deployment slot450446971, SHA25630045f7cc1f9069e3ccc0d9671e456d2235fe1180b9cbb4ac9877eb249a928fd. Disassembly of handler21806 reads input u64 at0..8 and mint8..40. The dispatcher reads the final byte; opcode1 enters GatorSwap. The long body reads quote slot40..48 and opaque signed64-byte data48..112. These bytes are not a guessed minimum output. The decoder requires exactly the two retained lengths.

Pool/authority is role3, user role0, user token accounts1/2 and vaults4/5. Pool-owned bytes bind the two mint fields8..40 and40..72 in initialized1984/5120-byte states. Independently read owned vaults bind their mint and role3 token authority. Thirty-two calls use classic principals,19 classic-input/Token-2022-output,6 Token-2022-input/classic-output. All native Token-2022 principals are plain Transfer on retained fee-free mints. The whole decoder additionally verifies net receipts; a future fee-bearing profile with mismatching net is rejected.

Fifteen calls have an exact first SystemCreate for role8 with Gator owner and512-byte space, funded by role0. The actual lamports remain a state-creation movement, excluded from swap input. All transfers must be immediate, committed and match the declared pool/user roles. Reference-account tails do not authorize extra asset movements.

The independent oracle proves57 financial legs,318 complete native-account ledgers, and378 token-account ledgers. Global unrelated Token-2022 fees are cross-checked against both retained owned fee schedules and every actual final token delta; their current mutable configuration is not asserted to be historical metadata or used to allocate any Gator principal. Exact zero-created/closed token lifecycles are separately bounded. One transaction creates/closes two PumpSwap volume states; each refund is exactly its actual paired SystemCreate funding, with a committed source-bound close event.

This is evidence for the retained swap profiles. It does not claim support for the distinct GatorSwap2 opcode or unknown future payloads.

Re-run `oracle.cjs` with Node and the `protobufjs` and `bs58` packages installed. Its default input directory is this fixture folder; an optional first argument selects another folder with the same files. The oracle is independent of decoder outputs. `cases.tsv` is the assertion table used by the Rust core test.

The core test checks all57 financial tuples and 180 malformed witnesses across10 distinct retained ABI/token-program shapes. Both persistent pool vault pre/post token balances must equal their complete committed transaction transfer ledgers, which binds output even when its user WSOL account closes. Unknown operations touching either vault fail this proof.
