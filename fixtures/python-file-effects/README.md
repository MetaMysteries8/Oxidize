# Controlled file-effect evidence

This fixture deliberately separates independent return/error tests from the
file contract. Both the original and a same-length `first` → `wrong` candidate
pass `test_effects.py`. With `verification.file_effect_paths` enabled, the
engine's real Python observations differ and exact comparison rejects them.

The five cases create a binary file (including NUL and non-UTF-8 bytes), change
it, delete it, write before an exception, and leave it unchanged. The second
configured path remains absent. Each case records presence, length and SHA256
before/after the call. Validation copies keep outputs out of the canonical
source/checkpoint; timing runs separately and captures no file evidence.

The opt-in integration tests use Bubblewrap and `/usr/bin/python3`, never
`--trust-code`. A second sandboxed test verifies that runtime-created symlinks,
directories and files over 1 MiB fail evidence collection. Pure Rust tests cover
configuration, exact hashes/sizes/presence, incomplete/legacy evidence and reports.

```bash
cargo test --locked -p oxidize-engine --test file_effects -- --include-ignored
```

Limits: only explicitly configured regular files, at most 32 paths and 1 MiB
per file, compared at call boundaries. Contents are hashed, not stored. No claim
about unlisted paths, directory trees, permissions/metadata, intermediate writes
restored before capture, imports/tests/timing side effects, external systems,
aliasing or concurrent writers. Cases run in order in one observation process;
the recorded before-state makes carry-over explicit.

This fixture compares Python original/candidate observations, not a compiled
PyO3 migration. Native acceptance/rejection for file effects still needs a
sandboxed native fixture; the milestone's full file-effect checkbox remains
open until that evidence exists. No speedup is claimed.
