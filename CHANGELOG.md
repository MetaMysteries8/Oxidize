# Changelog

## v0.2.0-dev — Unreleased

Milestone 2's checklist is complete on the development branch: 20/20 items, with the
[completion gate](docs/VERIFICATION.md#milestone-2-completion-gate) passed on 2026-11-16.

- Prepared `milestone-2-performance-reliability` from the v0.1.1 baseline, including its checkpoint-lock recovery fix.
- Defined ordered Performance and Reliability slices and observable acceptance criteria in [the milestone-two plan](docs/MILESTONE_2.md).
- Reserved the development version and updated README status. No new runtime features are implemented by this preparation commit.

### Reports

- A summary line per attempt (status, speed with range or "no claim", repairs, reason) and plain-language status for accepted, superseded, rejected, rolled-back and interrupted attempts. Tested for every outcome.

### Repair

- Integrated-test failures can be repaired from the same `max_repairs` budget as compiler failures (the provider gets the test output's last 16 KiB as untrusted data). Each attempt keeps a `repairs` history (trigger, diagnostics, generation), and the report lists it.

### Process supervision

- `execution.memory_mb`, `pids_limit` and `cpus` were silently ignored outside containers; a Bubblewrap or `--trust-code` run with any of them set is now refused by name.
- Tests for orphan cleanup after normal exit, timeout and cancellation (including a `setsid` escapee inside Bubblewrap).

### Measurement

- Acceptance timing runs original and candidate in interleaved rounds (`performance.rounds`, default 3), alternating which goes first. Raw samples are kept. The slowest round must reach `minimum_speedup`, and noisy, short or malformed timings (`max_relative_mad`, default 0.25) are "no claim" and never accepted.
- `[[performance.workloads]]` and `oxidize workloads`: whole-application commands timed on the original and the accepted workspace, with wall time and, on Linux/macOS, CPU time and peak RSS (named as unsupported elsewhere).

### Checkpoint recovery

- Deterministic fault injection (failure, crash, torn write) at every checkpoint save and file-writing step; standard tests up to the native build, an opt-in real PyO3 test from the build to promotion.
- Fixed: a write failure when an attempt starts is now recorded as a rejection; a resume clears the previous run's speedup, candidate fingerprint and repair count; resuming with the same response doesn't record a duplicate generation.
- Finished and archived attempts drop their Cargo `target/` directory (hundreds of MB each, not evidence).

### Checkpoint history and upgrades

- Checkpoint schema 2: `state.json` records a numbered, versioned event history (creation, upgrade, attempt start/resume, phase changes, acceptance, rejection, rollback), each saved atomically with its state change. Reports state the schema and event count.
- v0.1.x sessions upgrade on open, and their original checkpoint is kept as `state.v1.json`. This is tested against a real checkpoint written by v0.1.0. Newer or unknown schemas are refused without touching disk. There is no downgrade: v0.1.x refuses schema 2, so restore `state.v1.json` to go back.

### Evidence context and provenance

- Every measurement records the interpreter (from inside the run), rustc, OS/arch, isolation backend, command, sampling policy, acceptance threshold and scope, and the report states them.
- Every attempt records where its behaviour cases came from (file, SHA-256, project vs generated counts) and the independent test command, with the coverage limit in the report. Older sessions load and say these weren't recorded.
- Native file-effect demo: a compiled candidate that passes the independent tests but writes different bytes is rejected in the sandbox; the corrected replay is accepted.
- Tests pin explicit authorization for provider transmission and extra host mounts.

### Sandbox validation

- Pluggable sandbox backends: `execution.sandbox = auto | bubblewrap | podman | docker`. `auto` picks Bubblewrap on Linux when it works, then Podman, then Docker, and never falls back silently. Containers require an image pinned by digest, and run with no network, a read-only root, all capabilities dropped and optional cgroup limits. They are always removed after a run. The container backend is unit-tested but **not yet validated against a real engine**.

- Validated the complete native demo inside Bubblewrap without `--trust-code` on an Ubuntu host with user namespaces (see [verification](docs/VERIFICATION.md#sandboxed-native-demo)).
- Mount `/etc/alternatives` read-only in the sandbox: Debian/Ubuntu resolve `cc` through it, and without it every build script failed with "linker `cc` not found".
- Refuse a sandboxed command whose host executable lies outside every mount, instead of silently running a different one on the sandbox's `PATH` (a Conda `python3` was being replaced by a system Python of another version).

### First implementation slice

- Run independent tests, behavior observation and timing in fresh disposable copies, preserving canonical baseline and accepted source even when tests/workloads write project files.
- Capture typed positional/keyword argument values after calls, including error paths, and reject mutation differences with the case number in the failure report.
- Split observation and timing into separate processes; reconstruct inputs for every invocation. Test-generated setup and observation state no longer seed later stages.
- Add observation schema 2. Legacy checkpoint evidence stays readable and is labeled incomplete for mutation comparisons; replay records fresh evidence.
- Add deterministic process/workspace/mutation regressions and a real PyO3 fixture that rejects a mutation-only mismatch despite passing independent tests, then verifies resume and rollback. The fixture permits measured slowdowns and makes no speedup claim.

### Scoped comparison policies

- Keep exact comparison as the default; add explicit finite, nonnegative
  absolute/relative tolerances for returned floats and optional call
  stdout/stderr line-ending normalization.
- Preserve exact mutation/error/type/key/order/import checks, raw observations,
  and existing independent tests. Freeze the policy in session config and
  display it in Markdown/JSON reports; older configs/checkpoints default exact.
- Add config/comparator/report regressions and a Bubblewrap-only Python
  observation fixture. No new dependency, native adapter or performance claim.

### Controlled file-effect observation slice

- Opt into bounded project-relative regular-file snapshots via
  `verification.file_effect_paths`; default remains no file-effect claim.
- Capture exact before/after presence, byte length and SHA256 including error
  paths. Reject differing scopes, missing evidence and unsupported file types.
- Keep raw contents out of evidence, skip hashing in timing processes, retain
  explicit path metadata and scope/legacy limitations in reports.
- Add deterministic config/comparator/report tests and sandboxed Python
  fixtures. A native file-effect gate demonstration remains pending; this is
  not a claim of filesystem-wide tracing or a completed milestone.

## v0.1.1 — Checkpoint lock recovery

- Explicitly release a session's advisory lock when the session closes. A child process briefly retaining an inherited descriptor can no longer prevent immediate session reopening.
- Add a deterministic regression that retains a duplicate descriptor and verifies reopening and exclusive locking. The original CI failure is fixed without retries or serializing away the race.
- Validation: 16 standard tests and 2 native migration tests passed locally, along with formatting, Clippy and build checks. v0.1.0 remains an unchanged historical tag.

## v0.1.0 — Working Foundation

First development version of Oxidize, focused on a real incremental Python-to-Rust migration.

### Implemented

- Five-crate Rust workspace and native CLI with validated TOML configuration.
- Static Python AST analysis, direct call/import graph, heuristic dependency-first migration roadmap, and language inventory.
- OpenAI-compatible HTTP provider with bounded requests/retries and token-usage reporting; explicit offline implementation responses.
- One top-level synchronous Python function replaced through a compiled PyO3 extension in a separate workspace, preserving the Python entrypoint.
- Cargo formatting/checking/Clippy/release build, bounded compiler-guided repair, independent tests, typed differential cases, and measured component workloads.
- Configurable speedup acceptance, failure rejection, atomic locked checkpoints, replayable resume, rollback, and Markdown/JSON reports.
- Portable Oxidize skill, agent-specific local installer, deterministic fixtures, offline native example, documentation, and CI.

### Validation and limitations

15 standard tests and 2 opt-in native migration tests passed locally, along with build, formatting and Clippy checks. The native tests exercise actual compilation/loading, compiler repair, wrong-code rejection, resume and rollback without paid model access.

Only the documented Python/PyO3 migration path is implemented. Other languages are detected only. Native provider protocols, automatic migration repetition, package/signature expansion, Hyperoxidize and full runtime elimination are future work. Successful Bubblewrap execution is unvalidated in the development environment, which blocks user namespaces; the known demo uses explicit trusted-code execution. External agent discovery and live model calls were not exercised.

Next: [v0.2.0 — Performance and Reliability](docs/MILESTONE_2.md).
