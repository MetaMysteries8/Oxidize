# Verification and performance evidence

The initial demo's independent Python tests cover known prime counts, all limits below 200 against a separate `math.isqrt` reference, and errors for negative/huge integers and noninteger inputs. Eleven supplied differential cases cover boundaries, an actual compute workload, error types/messages, and keyword invocation. PyO3 builds in release mode; the application calls the extension through the original Python interface.

Standard Cargo tests cover config roundtrips/validation, path traversal and secret exclusions, source snapshots/fingerprints, symlink rejection, process timeout/output bounds, static language/graph/plan output, no-execution AST inspection, session locking/recovery/source-edit detection, baseline test failures, evidence capture, fail-closed sandbox behavior, HTTP rate-limit retry/usage parsing and portable skill structure.

Opt-in native tests actually compile and load the PyO3 implementation. They exercise acceptance, independent-test rejection of compilable wrong code, saved-state resume with corrected code, preservation of partial attempts, and rollback. A separate deterministic HTTP integration test exercises compiler-guided model repair. None uses a paid provider. The additional real `python-mutation` fixture demonstrates a compiled candidate that passes existing return/error tests but is rejected for an argument-mutation mismatch, followed by corrected-code resume and rollback. Its independent tests write a normal generated file; the canonical snapshots must remain unchanged. Its permissive performance threshold deliberately allows slowdowns and is not performance evidence.

Measurement uses Python `perf_counter_ns` in independent original/candidate processes, two warm-up batches and seven measured batches by default. Behavior capture and timing run in separate processes and fresh disposable copies, so test/setup and observation state do not seed timing. It measures successful supplied cases through the Python callable; errors are checked separately. Each batch reconstructs case arguments to avoid mutation leaking between repetitions. The report contains raw samples, median and median absolute deviation. Input reconstruction, Python call dispatch and FFI are included in both sides' timings. Sequential ordering and machine noise can bias results. These timings do not measure startup, whole-app throughput, CPU use, allocations or memory; no such metrics are fabricated.

Exact comparisons are the default and preserve scalar types, list/tuple distinctions, dict order, exceptions, stdout/stderr, and post-call positional/keyword argument values, including mutations before exceptions. Non-finite floats and arbitrary objects fail instead of being silently normalized. Unconfigured file/network effects, identity/aliasing/cyclic objects, timing/concurrency, import races, reflection/docstrings/annotations, Python object identity and unsupported calls remain unverified. Passing tests and cases is bounded evidence, not a mathematical proof of equivalence.

Development regressions cover test/workload writes on success and failure, repeated verification without snapshot contamination, fresh inputs for every invocation, distinct observation/timing PIDs and workspaces, temporary-copy cleanup, and readable legacy observation evidence. Observation schema 2 adds typed argument snapshots; legacy schema 1 is reported as missing that evidence and cannot be compared as mutation-equivalent without replay. The top-level checkpoint schema is 2 (see [checkpoint upgrades](#checkpoint-history-and-upgrades)). Unconfigured file writes in validation copies are discarded, not compared; path-dependent workloads and test-generated setup require explicit preparation. Scoped copies do not sandbox trusted code accessing absolute host paths.

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

## Native file-effect demo

Validated 2026-11-15 on Ubuntu (Bubblewrap, no `--trust-code`, CPython 3.14.4, rustc 1.100.0).
Copy `fixtures/python-file-effects` to a working directory, add the sandbox's toolchain mounts
from [the sandboxed native demo](#sandboxed-native-demo) (`[execution] read_only_paths`), then:

```sh
oxidize analyze DIR && oxidize plan DIR
oxidize --allow-host-paths migrate DIR --component effects.py:record \
  --response fixtures/python-file-effects/migration-response-wrong-bytes.json
# rejected: "differential mismatch: case 1: configured file effects differ"
oxidize --allow-host-paths migrate DIR --component effects.py:record \
  --response fixtures/python-file-effects/migration-response.json
# Accepted attempt 2
```

The wrong candidate writes `wrong\0\xff` instead of `first\0\xff`: same length, and it passes
`test_effects.py`, which checks only the return values and errors. Only the configured
file-effect comparison catches it. Scope: only the listed files are observed. Unlisted,
transient or import-time writes, metadata and external systems remain unverified.

## Checkpoint history and upgrades

Milestone 2 §3. `state.json` (schema 2) carries `events`: numbered, versioned records of
session creation, schema upgrades, attempt start/resume, phase changes, acceptance (with the
previous pointer), rejection (with the error) and rollback (with the restored pointer). Each
event is written in the same atomic write as the state change it describes, so they can't
disagree after a crash. Opening validates the history: contiguous numbering, a known event
version, a creation or upgrade event first, and only real attempt numbers.

Evidence (`crates/oxidize-engine/tests/checkpoint_upgrade.rs`, standard tests):

- `fixtures/legacy-v0.1.0-session/state.json` is an unmodified checkpoint written by the
  `v0.1.0` release (how it was made is in that folder's README). Opening it gives schema 2 with
  the config, attempt, saved code, checks, error and legacy schema-1 evidence intact, and a single
  `schema_upgraded` event. The original is kept byte for byte as `state.v1.json`, the report
  states the upgrade and the gap before it, and reopening doesn't upgrade again.
- Interrupted after the backup and before the new state, the next open upgrades again. A
  `state.v1.json` that isn't this checkpoint is never overwritten: opening is refused and both
  files stay as they were.
- A newer schema is refused with "written by a newer Oxidize". Schema 0 and a string version are
  refused as unsupported. In each case nothing on disk changes.
- A new session records its creation, and a rejected attempt records its start, phases and
  rejection. Reopening returns the same history. A renumbered event is refused.
- The opt-in real PyO3 test `real_migration_rejection_resume_and_rollback` asserts the
  history of a real session: accepted 1, rolled back 1, rejected 2, resumed 2, accepted 2,
  rolled back 2 (run with `--include-ignored` on Linux, 2026-11-16).

Limits: there is no downgrade. The v0.1.0 binary refuses an upgraded checkpoint, failing
closed on its newer fields ("checkpoint is corrupt … unknown field `comparison`"). Copying
`state.v1.json` back over `state.json` makes it readable by v0.1.0 again (checked by hand,
2026-11-16). That loses everything recorded since the
upgrade. Events are a log for people and reports. Resume and rollback still act on the
attempts and the accepted pointer, not by replaying events. Timestamps come from the host
clock and are not evidence of ordering. `seq` is.

## Checkpoint fault injection

Milestone 2 §3. A test can install a fault injector on a session
(`crates/oxidize-engine/src/faults.rs`). It sees every checkpoint save, before and after the
atomic write, and every step that writes files (`attempt workspace`, `native source`,
`install extension`, `integrate`). At each point it can inject:

- a **failure**, such as a full disk. Normal error handling runs.
- a **crash**. Recovery code is skipped, so the disk holds what a killed process leaves.
- a **torn write**. Half of the new checkpoint reaches the temporary file and `state.json` is
  never replaced.

Production sessions have no injector.

Evidence (`crates/oxidize-engine/tests/faults.rs`):

- **Standard tests, every point before the native build.** At each point a crash, a torn write
  (save points) and a write failure, plus every save failing from point *n* on. After each
  fault, reopening works, the input fingerprint is unchanged, nothing is accepted or reported
  as accepted, and the baseline is intact. A torn temporary file is ignored. A write failure is
  recorded as a rejection with its cause, including at the attempt-start save, which used to
  leave a half-recorded attempt. Resume (or a fresh migrate, if the attempt never reached disk)
  gets back to the build with the baseline evidence intact and the generated code saved once.
- **Opt-in real PyO3 test, every point from the build to promotion**
  (`native_crash_at_every_point_from_the_build_to_promotion_keeps_or_completes_a_checkpoint`).
  A first component is accepted; a second is crashed (or torn) at each point. Reopening shows
  either the first checkpoint, current and byte-identical, or the second one complete and
  accepted (a crash after the promotion was saved). Resume then completes it, and rolling back
  returns to the untouched first checkpoint every time. Run on Linux on 2026-11-16: 21 points,
  all passed (about 19 minutes).
- **Replays never reuse stale results.** Speedup, candidate fingerprint and repair count from an
  earlier run are cleared when a resume starts (previously they survived, and an interrupted
  resume would have reported the old speedup). Resuming with the same `--response` replays it
  rather than recording it as a second generation.
- **Input edits are never touched.** With an unrelated edit in the input, resume refuses and
  rollback has nothing to do. The edit and the checkpoint stay as they were, and once the edit
  is moved out, resume carries on.
- **Build caches.** Finished and archived attempts drop their Cargo `target/` directory, which
  fingerprints and snapshots already exclude. Without this, the opt-in test above ran a 3.4 GB
  tmpfs out of space after 11 attempts ("Disk quota exceeded").

Limits: a crash is simulated in-process, at the points listed. A real `SIGKILL` between two
system calls inside one step isn't covered, except that `state.json` changes only by atomic
rename. Validation copies left by a real kill stay under `.oxidize/validation/`. Archived
`interrupted-*` runs are kept for inspection and are not cleaned up automatically.

## Interleaved measurement and application workloads

Milestone 2 §1. A candidate whose behaviour matches is timed against the original in
`performance.rounds` interleaved rounds (default 3), alternating which side goes first. Each
side runs in a fresh process and validation copy, with the same `warmup` and fresh inputs for
every call. All raw samples are kept in the attempt's `measurement` in `state.json`/`report.json`.
`sampling::decide` gives either a speedup estimate (the median of per-round speedups) with
its range, or **inconclusive** when there are no rounds, too few samples, a missing, zero or
non-finite timing, rounds out of order, or a round noisier than `max_relative_mad` (default
0.25). Acceptance needs the *slowest* round to reach `minimum_speedup`. An inconclusive
measurement is never accepted, and the report then says "Speed: **no claim**".

`[[performance.workloads]]` (name + command) and `oxidize workloads` time whole-application
commands on the original and the accepted workspace, in the same interleaved rounds. A small
Python wrapper inside the sandbox runs each command. It records wall time, and, where Python's
`resource` module exists (Linux, macOS), CPU time and peak RSS of that run's process tree.
Elsewhere those are listed as unsupported, never left blank. Results are kept in the checkpoint
(`workloads`) and reported.

Evidence:

- Unit tests (`sampling.rs`, `workloads.rs`): the order alternates; steady rounds give the
  estimate and range, and the slowest round decides acceptance. Empty, short, zero, NaN and
  infinite data, rounds out of order and noisy rounds are inconclusive and can't clear any
  threshold. Unsupported CPU/RSS is named in the report.
- Config tests (`sampling_config.rs`): defaults for old configs and checkpoints, range
  checks, and workload names and commands validated.
- Wrapper tests (`tests/workloads.rs`): a real child that holds an 80 MiB buffer reports three
  timed runs, positive CPU and a peak RSS of at least 80 MiB. A failing command is an error, not
  a timing.
- Opt-in real PyO3 test `real_migration_rejection_resume_and_rollback`: the accepted
  `count_primes` migration was decided by 3 interleaved rounds of 7 samples per side, and
  `oxidize workloads` timed `python3 main.py 30000` on the original and the accepted
  workspace. On the owner's Linux machine on 2026-11-16 that measured wall time **1.97x**
  (range 1.89x to 2.10x) and CPU time 1.99x (range 1.90x to 2.11x). That is one host's figure
  for one command, not a general speed claim, and the test asserts only the structure, never a
  speedup.

Limits: rounds run back to back on one machine; interleaving reduces drift but not contention
from other load. Ranges are min/max over a few rounds, not statistical confidence intervals.
Peak RSS is the largest single process (across warm-up and timed runs), not a total. CPU
time is the process tree's user + system time as the OS accounts it, which can be coarse for
very short runs (such rounds tend to come out inconclusive). Workloads need an accepted
workspace, and their commands run in the same sandbox as tests.

## Process supervision

Milestone 2 §4. Every command runs in its own process group (`--trust-code`) or its own
Bubblewrap PID namespace with `--die-with-parent`. When it exits, times out (`timeout_seconds`),
exceeds `output_limit_bytes` or is cancelled (Ctrl-C), the whole tree is killed and nothing it
started keeps running. A cancelled Oxidize refuses to start new commands. Resource limits
(`memory_mb`, `pids_limit`, `cpus`) are enforced only by Podman/Docker. With Bubblewrap or
`--trust-code` a run with any of them set is refused, naming the limits, instead of running
unlimited. Before this change they were silently ignored.

Evidence (`crates/oxidize-core/tests/supervision.rs`, `cancellation.rs`, standard tests on
Unix): each test starts a grandchild that rewrites a heartbeat file every 50 ms and checks the
file stops changing.

- A command that exits normally leaves no running descendant (`--trust-code`).
- A 1 s timeout stops a tree that would sleep for 60 s, within 10 s (`--trust-code`).
- Inside Bubblewrap, even a descendant that started its own session (escaping the process
  group) is stopped, both after a normal exit and after a timeout. Skipped where Bubblewrap or
  `/usr/bin/python3` is unavailable.
- Cancellation stops a running tree within 10 s, and the next command is refused.
- Each of the three limits is refused with `--trust-code` and with Bubblewrap.

Limits: with `--trust-code`, a descendant that calls `setsid()` escapes the process-group kill
(Bubblewrap's PID namespace doesn't have that gap). On Windows, `--trust-code` kills only the
direct child, and these tests don't run there. Container limits are unit-tested as arguments
only; no engine has run them yet (see the container note in HANDOFF.md).

## Repair from integrated-test failures

Milestone 2 §4. Repair used to cover only compiler failures. Now, when the extension builds
but the project's own tests fail with it installed, that failure can be repaired too, from the
same `max_repairs` budget. The provider gets the last 16 KiB of the test output, marked as
untrusted data. Before each re-integration the component's file is restored from the
baseline. Every repair records its trigger (`compiler` or `integrated_tests`), the diagnostics
the provider was given, whether they were truncated, and the generation it produced. Every
test run and each generation's token usage are kept, and the report lists the repairs. The
loop stops when the budget is spent, when a provider repeats identical code ("repair
stalled"), for an offline `--response` (which can't be repaired), or without
`--allow-provider`.

Evidence (`crates/oxidize-engine/tests/repair.rs`, opt-in real PyO3 builds with a local HTTP
server standing in for the model):

- Code that compiles but miscounts fails the project's tests. The repair request carries
  that failure, the corrected code is accepted after 1 repair, and both test runs (fail, pass),
  both generations' usage and the repair record are kept and reported.
- A provider that returns the same wrong code again stops as stalled after 2 calls. The
  attempt is rejected with the repair on record, nothing is accepted, and the input is unchanged.
- With `max_repairs = 0`, the failure is rejected at once with the test output in the error,
  after one call.
- The existing compiler-repair test still passes through the unified loop.

Limits: differential mismatches (behaviour that passes the tests but differs from the original)
are not sent for repair. Their reasons would describe the original's outputs, and that
evidence should stay independent of the generated code.

## Reports

Milestone 2 §5. `report.md` opens with a summary line per attempt: status, speed with its range
or "no claim", repairs and the rejection reason. Each attempt then states what its status
means: accepted (current or superseded), rejected (the previous workspace was kept), rolled back
(kept for inspection) or interrupted (nothing accepted; `oxidize resume` replays it). Below
that come the interleaved measurement, the build/test checks with exit codes, the repair
history, the evidence provenance and the model usage. The report also covers application
workloads, the event history, the comparison and file-effect scope, the remaining runtime
dependencies (Python and PyO3) and the evidence limits. `report.json` is the checkpoint itself:
`schema_version` 2 with `events`, raw measurement samples and workload runs.

Evidence (`crates/oxidize-engine/tests/report.rs`, standard tests): reports for a real rejected
session, a real interrupted one (both stopped by fault injection before the native build), an
accepted one built by promoting that real attempt's state with a measurement, a repair and a
workload run, the same session rolled back, and a new session. Each must contain every common
section and the outcome-specific wording, and `report.json` must carry the schema version,
the events and the raw samples. The opt-in native tests check real accepted reports.
