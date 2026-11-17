# Input-mutation regression fixture

`total_and_mark` returns the sum of a list and appends that sum to the caller's list. For a negative total it appends first, then raises `ValueError`. The independent tests deliberately assert returns/errors without checking mutation; the supplied differential cases supply the additional evidence.

The real PyO3 integration test removes `values.append(total)?` from the pre-generated Rust response. That replacement compiles and passes the independent tests, but schema-2 differential verification rejects it for changed arguments. The test then reopens the checkpoint, resumes with corrected Rust, verifies the accepted copy and rolls back, checking source fingerprints throughout.

Tests write a normal generated file to their working directory. It must not appear in the canonical baseline or accepted project. The response uses small-integer-list workloads; it is a boundary/mutation fixture, not proof of general numeric/type compatibility. Its permissive performance threshold explicitly allows slowdowns: no speed improvement is claimed.

```bash
cargo fetch --locked --manifest-path examples/python-native/Cargo.toml
cargo test --locked -p oxidize-engine --test engine \
  native_mutation_mismatch_rejects_after_passing_tests_and_resumes_correctly \
  -- --ignored --nocapture
```

This runs the inspectable fixture with the test's explicit trusted-code mode. Successful sandbox execution is a separate milestone gate.
