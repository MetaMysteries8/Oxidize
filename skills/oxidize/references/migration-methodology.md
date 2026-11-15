# Progressive replacement

Inspect → baseline → plan → implement → integrate → compile → test → compare → benchmark → accept/revert → checkpoint → repeat.

Begin with a compact, coherent component. Identify inputs, outputs, exceptions, mutation, external state, call relationships, and runtime ownership. Record unsupported behavior before choosing a bridge. Replan after each accepted change; migrate dependency leaves first where practical. Cycles may need a group migration. Keep the application runnable at each checkpoint.

The current CLI performs one component attempt per invocation and retains accepted and failed workspaces under `.oxidize/`. Input source is never modified. Resume replays saved generated code into a fresh copy; it does not trust a partially written extension. Rollback changes the accepted-workspace pointer and retains evidence. New source edits require a new session in a separate copy; never erase an active session to hide failures.

For Rust-to-Rust work, profile first and change one measured bottleneck at a time. Prefer safe Rust. Unsafe code needs a demonstrated requirement, explicit invariants, and suitable verification.
