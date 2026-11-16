# Verification and performance evidence

The initial demo's independent Python tests cover known prime counts, all limits below 200 against a separate `math.isqrt` reference, and errors for negative/huge integers and noninteger inputs. Eleven supplied differential cases cover boundaries, an actual compute workload, error types/messages, and keyword invocation. PyO3 builds in release mode; the application calls the extension through the original Python interface.

Standard Cargo tests cover config roundtrips/validation, path traversal and secret exclusions, source snapshots/fingerprints, symlink rejection, process timeout/output bounds, static language/graph/plan output, no-execution AST inspection, session locking/recovery/source-edit detection, baseline test failures, evidence capture, fail-closed sandbox behavior, HTTP rate-limit retry/usage parsing and portable skill structure.

Opt-in native tests actually compile and load the PyO3 implementation. They exercise acceptance, independent-test rejection of compilable wrong code, saved-state resume with corrected code, preservation of partial attempts, and rollback. A separate deterministic HTTP integration test exercises compiler-guided model repair. None uses a paid provider. The additional real `python-mutation` fixture demonstrates a compiled candidate that passes existing return/error tests but is rejected for an argument-mutation mismatch, followed by corrected-code resume and rollback. Its independent tests write a normal generated file; the canonical snapshots must remain unchanged. Its permissive performance threshold deliberately allows slowdowns and is not performance evidence.

Measurement uses Python `perf_counter_ns` in independent original/candidate processes, two warm-up batches and seven measured batches by default. Behavior capture and timing run in separate processes and fresh disposable copies, so test/setup and observation state do not seed timing. It measures successful supplied cases through the Python callable; errors are checked separately. Each batch reconstructs case arguments to avoid mutation leaking between repetitions. The report contains raw samples, median and median absolute deviation. Input reconstruction, Python call dispatch and FFI are included in both sides' timings. Sequential ordering and machine noise can bias results. These timings do not measure startup, whole-app throughput, CPU use, allocations or memory; no such metrics are fabricated.

Exact comparisons are the default and preserve scalar types, list/tuple distinctions, dict order, exceptions, stdout/stderr, and post-call positional/keyword argument values, including mutations before exceptions. Non-finite floats and arbitrary objects fail instead of being silently normalized. External file/network effects, identity/aliasing/cyclic objects, timing/concurrency, import races, reflection/docstrings/annotations, Python object identity and unsupported calls remain unverified. Passing tests and cases is bounded evidence, not a mathematical proof of equivalence.

Development regressions cover test/workload writes on success and failure, repeated verification without snapshot contamination, fresh inputs for every invocation, distinct observation/timing PIDs and workspaces, temporary-copy cleanup, and readable legacy observation evidence. Observation schema 2 adds typed argument snapshots; legacy schema 1 is reported as missing that evidence and cannot be compared as mutation-equivalent without replay. The top-level checkpoint schema remains 1 for this additive change. File writes in validation copies are discarded, not compared; path-dependent workloads and test-generated setup require explicit preparation. Scoped copies do not sandbox trusted code accessing absolute host paths.

## Explicit comparison opt-ins

Omitting `verification.comparison` keeps exact comparison. Projects may explicitly
select either or both opt-ins before starting a session:

```toml
[verification.comparison]
normalize_output_line_endings = true

[verification.comparison.return_float_tolerance]
absolute = 0.000001
relative = 0.00001
```

- **Returned floats only:** two finite Python floats match if their absolute
  difference is at most `absolute`, or their relative difference is at most
  `relative`, scaled by the larger absolute value. Limits are inclusive,
  symmetric, finite and nonnegative; omitted limits are zero. Relative
  comparison scales before subtraction to avoid overflow for opposite-sign
  finite extremes. This is ordinary floating-point arithmetic, not exact-real
  arithmetic. The rule recurses through returned lists, tuples and dictionary
  values, preserving lengths, type tags, dictionary keys and insertion order.
  Integers (including values beyond f64's integer precision), booleans and
  return strings are not coerced or approximated.
- **Call stdout/stderr only:** when enabled, CRLF and standalone CR compare as
  LF. Whitespace, extra/missing output, return strings, and exception messages
  are not otherwise changed. Module import stdout/stderr remain exact.
- **Always exact:** positional/keyword argument snapshots, including float
  mutations and mutations before exceptions; error identity and message;
  return-vs-error outcomes; case counts and any other observation fields.
  Legacy observations still cannot establish mutation equivalence.

Independent tests and timing are unchanged by these settings. The migration
acceptance gate uses the frozen session policy; editing project config does not
change an existing session's policy. Start a new project copy/session to choose
different settings. JSON reports/checkpoints retain the policy under
`config.verification.comparison`; Markdown reports display it explicitly.
Raw observations remain unmodified. Old configs/checkpoints without this
additive field load with the exact default; no schema-version change is needed.
The public Rust `equivalent` helper remains exact;
`equivalent_with_policy` requires the caller to supply a policy and fails closed
on invalid tolerances.

Evidence: [the scoped comparison fixture](../fixtures/python-comparison/README.md)
and the standard config/comparator regressions exercise opt-ins without masking
unrelated changes. The opt-in sandboxed fixture also captures and compares real
Python observations on hosts with working Bubblewrap. This is not a new native
adapter, a proof of universal equivalence, a speedup result, or a complete
sandboxed Cargo demonstration. Large tolerances intentionally weaken the
return-value check; choose them according to your workload's contract.
