# Scoped comparison fixture

`compare.py:observe` returns a nested float (`0.1 + 0.2`) and a string with
literal CRLF, emits stdout/stderr with CRLF/CR, and mutates a positional or
keyword list. A negative input mutates the list and then raises an error.

`oxidize.toml` opts into return-float tolerances and call-output line-ending
normalization. Those settings are disabled by default in ordinary projects.

The deterministic comparator tests exercise near-zero absolute tolerance,
relative tolerance, nested containers, exact keys/order/types, strict mutation
and exception paths, invalid policies, raw-evidence preservation and reports.
The first configuration test failed before implementation because
`verification.comparison` was unknown.

An opt-in engine integration test captures actual observations and timings in
Bubblewrap, without `--trust-code`, from this source and a disposable candidate
that returns `0.3` and emits LF instead. It proves:

- exact comparison rejects the candidate;
- the explicitly selected policy accepts those differences;
- return strings, argument mutation and exception messages are not normalized;
- raw original/candidate observations are retained.

Run on Linux with working Bubblewrap and `/usr/bin/python3`:

```bash
cargo test --locked -p oxidize-engine --test comparison -- --include-ignored
```

This fixture does not compile a new PyO3 candidate or claim a speedup. It tests
the policy and the existing engine's real Python observation path. Timings are
not asserted. The integration test uses a disposable directory under ignored
`target/` because the runner's isolated `/tmp` hides host `/tmp` workspaces.
Fixing the general `/tmp` mount-order limitation belongs to execution work.
