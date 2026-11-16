# Verification and performance evidence

The initial demo's independent Python tests cover known prime counts, all limits below 200 against a separate `math.isqrt` reference, and errors for negative/huge integers and noninteger inputs. Eleven supplied differential cases cover boundaries, an actual compute workload, error types/messages, and keyword invocation. PyO3 builds in release mode; the application calls the extension through the original Python interface.

Standard Cargo tests cover config roundtrips/validation, path traversal and secret exclusions, source snapshots/fingerprints, symlink rejection, process timeout/output bounds, static language/graph/plan output, no-execution AST inspection, session locking/recovery/source-edit detection, baseline test failures, evidence capture, fail-closed sandbox behavior, HTTP rate-limit retry/usage parsing and portable skill structure.

Opt-in native tests actually compile and load the PyO3 implementation. They exercise acceptance, independent-test rejection of compilable wrong code, saved-state resume with corrected code, preservation of partial attempts, and rollback. A separate deterministic HTTP integration test exercises compiler-guided model repair. None uses a paid provider. The additional real `python-mutation` fixture demonstrates a compiled candidate that passes existing return/error tests but is rejected for an argument-mutation mismatch, followed by corrected-code resume and rollback. Its independent tests write a normal generated file; the canonical snapshots must remain unchanged. Its permissive performance threshold deliberately allows slowdowns and is not performance evidence.

Measurement uses Python `perf_counter_ns` in independent original/candidate processes, two warm-up batches and seven measured batches by default. Behavior capture and timing run in separate processes and fresh disposable copies, so test/setup and observation state do not seed timing. It measures successful supplied cases through the Python callable; errors are checked separately. Each batch reconstructs case arguments to avoid mutation leaking between repetitions. The report contains raw samples, median and median absolute deviation. Input reconstruction, Python call dispatch and FFI are included in both sides' timings. Sequential ordering and machine noise can bias results. These timings do not measure startup, whole-app throughput, CPU use, allocations or memory; no such metrics are fabricated.

Exact comparisons are the default and preserve scalar types, list/tuple distinctions, dict order, exceptions, stdout/stderr, and post-call positional/keyword argument values, including mutations before exceptions. Non-finite floats and arbitrary objects fail instead of being silently normalized. Unconfigured file/network effects, identity/aliasing/cyclic objects, timing/concurrency, import races, reflection/docstrings/annotations, Python object identity and unsupported calls remain unverified. Passing tests and cases is bounded evidence, not a mathematical proof of equivalence.

Development regressions cover test/workload writes on success and failure, repeated verification without snapshot contamination, fresh inputs for every invocation, distinct observation/timing PIDs and workspaces, temporary-copy cleanup, and readable legacy observation evidence. Observation schema 2 adds typed argument snapshots; legacy schema 1 is reported as missing that evidence and cannot be compared as mutation-equivalent without replay. The top-level checkpoint schema remains 1 for additive fields. Unconfigured file writes in validation copies are discarded, not compared; path-dependent workloads and test-generated setup require explicit preparation. Scoped copies do not sandbox trusted code accessing absolute host paths.

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

## Controlled file-effect opt-in

By default no file effects are observed. To declare a bounded file contract,
configure paths **before** starting a session:

```toml
[verification]
file_effect_paths = ["output/result.bin", "status.txt"]
```

Paths must be unique, project-relative and stay inside the project; at most 32
paths, each at most 1024 bytes. Reserved session/VCS/credential-like paths are
rejected. At capture time each path component is checked for symlinks; existing
leaves must be regular files of at most 1 MiB. Missing files or parents are
recorded as `null`, not as empty files. Existing files produce
`{"size_bytes": N, "sha256": "..."}`. Raw file bytes are never stored in reports.
Unsupported types, oversized files or capture failures fail evidence collection,
not silently skip the affected path.

Each behavior case records before/after states, even when the function raises.
File snapshots are compared exactly, independent of float/output normalization.
Creation, deletion, changed bytes (including binary content), and changed initial
state are visible. Cases run sequentially in one fresh observation copy/process,
so a case can inherit a previous case's files; both states are recorded.
Independent tests and timing still use separate copies, and timing includes no
snapshot/hash work.

Config, observations and JSON evidence retain `file_effect_paths`; each
observation's `file_effects` maps those paths to `before`/`after`. Older configs
default to no paths, and older evidence defaults to an empty captured-path list.
Different scopes or missing/malformed snapshots cannot establish file-effect
equivalence. This additive schema-2 field does not upgrade historical evidence.
Markdown reports distinguish requested scope from actually captured/legacy
evidence.

**Not observed:** unlisted paths, directory trees, modes/permissions/timestamps,
inode identity/hard links, transient writes undone between boundaries,
import/test/timing effects, external services or concurrent writers. Snapshot
hashing is not a hostile-code monitor or a race-proof filesystem boundary.
Prefer the sandbox; this does not strengthen `--trust-code`.

Evidence: [the controlled-file fixture](../fixtures/python-file-effects/README.md)
passes the independent return/error tests for original and altered Python code,
then rejects the changed byte evidence under Bubblewrap. Config, malformed/legacy
evidence and reporting also have deterministic tests. A compiled native
file-effect acceptance/rejection demonstration remains pending; this slice does
not complete the full milestone gate or claim a performance improvement.

## Sandboxed native demo

The full native demo, run inside Bubblewrap without `--trust-code`. Validated on
2026-11-15 on Ubuntu (Linux 7.0, `kernel.apparmor_restrict_unprivileged_userns=1`),
Rust 1.100.0 via rustup, and system CPython 3.14 with development headers.

```bash
cargo fetch --locked --manifest-path examples/python-native/Cargo.toml
cp -R fixtures/python-cli work/sbx
# work/sbx/oxidize.toml: an interpreter the sandbox can see, and the toolchain read-only
#   python = "/usr/bin/python3"
#   test_command = ["/usr/bin/python3", "-m", "unittest", "discover", "-v"]   # [verification]
#   read_only_paths = ["/home/<you>/.rustup", "/home/<you>/.cargo"]             # [execution]
export CARGO_HOME=$HOME/.cargo RUSTUP_HOME=$HOME/.rustup
oxidize analyze work/sbx && oxidize plan work/sbx
oxidize --allow-host-paths migrate work/sbx --component compute.py:count_primes \
  --response fixtures/python-cli/migration-response.json
oxidize --allow-host-paths verify work/sbx
oxidize --allow-host-paths benchmark work/sbx --component compute.py:count_primes
oxidize report work/sbx
/usr/bin/python3 work/sbx/.oxidize/attempts/0001/project/main.py 100   # 25
oxidize rollback work/sbx
```

Observed: attempt 1 accepted; independent tests passed in a disposable copy; a
component workload speedup of 239–288x across two runs (it includes Python/FFI call
overhead, and is not a whole-application figure); the migrated program printed `25`;
rollback restored the baseline. `~/.cargo` held only `bin`, `env` and `registry`, and no
credentials, when it was mounted.

Two problems found and fixed during validation:

- **`linker cc not found`:** Debian/Ubuntu resolve `/usr/bin/cc` through
  `/etc/alternatives`, which wasn't mounted. It is now part of the read-only system mounts.
- **The wrong Python, silently:** with `python = "python3"`, the host resolved a Conda
  3.13 that the sandbox can't see, so the sandbox used system 3.14 instead, and the built
  module (`cpython-314`) then failed to import on the host. Sandboxed runs now refuse a
  program whose host path is outside every mount, and name it.

Not covered: cgroup resource limits, CI (its environment rejects user namespaces), and
non-Ubuntu hosts.
