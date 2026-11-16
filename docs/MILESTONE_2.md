# Milestone 2 — Performance and Reliability

**Status: in progress — first implementation slice delivered. Target: v0.2.0.** Baseline: v0.1.1 Working Foundation, including the checkpoint-lock recovery fix. Development branch: `milestone-2-performance-reliability`; package version: `0.2.0-dev`.

The goal is to make the existing Python/PyO3 migration path more reliable and its evidence more useful before adding source languages. The foundation already has basic differential testing, component timing, compiler repair, isolated copies, rollback and reporting. This milestone strengthens those implementations rather than treating them as missing or claiming planned capabilities are complete.

## 1. Reproducible workloads and profiling

- [x] Separate behavior observation from timing so repeated performance runs cannot alter correctness evidence.
- [ ] Introduce a testable workload/sampling layer with symmetric warm-up, fresh inputs, interleaved original/candidate batches, retained raw timings, and a configurable noise/uncertainty policy.
- [x] Record interpreter/toolchain, platform, command, warm-up/sample policy, scope and acceptance threshold with every measurement. Evidence: `Benchmark.interpreter` (reported by the harness from inside the run) and `Benchmark.context` (OS/arch, isolation backend, command, iterations, warm-up, calls per sample, threshold, scope), shown as a "Measured with …" report line; older evidence loads and says its context wasn't recorded (engine unit tests; sandboxed demo report). The context also records `rustc --version` as seen inside the isolated run.
- [ ] Add user-defined whole-application workloads; collect wall time and, on supported platforms, child CPU/peak RSS. Explicitly mark unsupported/unavailable measurements.

Acceptance: deterministic sampler/policy tests cover invalid/insufficient data, a real repeatable workload demonstrates measurement through the integrated interface, and noisy or missing evidence cannot silently become a speed claim. Do not impose brittle machine-specific speedup assertions in normal unit tests.

## 2. Stronger behavioral evidence

- [x] Compare supported argument mutation alongside current typed returns, errors, stdout and stderr.
- [x] Add explicit opt-in normalization/tolerance policies with exact comparison remaining the default.
- [x] Add controlled file-effect fixtures and comparisons with clear scope; record external effects that remain unverified. Evidence: in Bubblewrap without `--trust-code`, a compiled PyO3 candidate writing `wrong` instead of `first` (`fixtures/python-file-effects/migration-response-wrong-bytes.json`) passes the independent tests and is rejected ("case 1: configured file effects differ"); the corrected replay (`migration-response.json`) in the same session is accepted ([verification](VERIFICATION.md#native-file-effect-demo)).
- [x] Keep existing independent tests separate from generated tests and make case provenance/coverage limits visible in JSON and Markdown. Evidence: each attempt records `provenance` (cases file, its SHA-256, project-supplied case count, `generated_cases` (0: Oxidize generates none yet), and the independent test command) in `state.json`/`report.json`; the Markdown report states it with the coverage limit (engine unit test; sandboxed demo report). When case generation is added, it must increment `generated_cases` and keep its cases separate.

Acceptance: a compilable candidate that passes existing tests but changes an observed mutation, error, return type or configured file effect is rejected. Intentional tolerance/normalization is tested without masking unrelated changes.

## 3. Transaction and checkpoint recovery

- [x] Run baseline tests in disposable validation copies so tests cannot corrupt the canonical baseline checkpoint.
- [x] Add a versioned event history and explicit checkpoint schema-upgrade strategy while preserving v0.1.0 sessions. Evidence: checkpoint schema 2 records numbered, versioned events in the same atomic write as each state change. A real checkpoint written by the v0.1.0 release (`fixtures/legacy-v0.1.0-session`) upgrades with nothing lost, and its original bytes are kept as `state.v1.json`. Interrupted upgrades redo safely, a conflicting backup is never overwritten, and newer or unknown schemas are refused untouched (`checkpoint_upgrade` tests). The real PyO3 rollback test asserts the full event story ([verification](VERIFICATION.md#checkpoint-history-and-upgrades)).
- [ ] Add deterministic interruption/fault-injection tests around generation, integration, evidence collection and atomic promotion, including write failures.
- [ ] Make resume and repeated rollback safe and reproducible without losing unrelated input edits or reusing stale evidence.

Acceptance: after each injected interruption, reopening either preserves the old accepted workspace or resumes a complete new one; partial output is never reported as accepted. Input files remain unchanged and failed attempts remain inspectable.

## 4. Execution and bounded repair

- [x] Validate a successful Bubblewrap build/test flow on a Linux host with user namespaces, including Cargo cache/lock and toolchain mount handling. Evidence: [the sandboxed native demo](VERIFICATION.md#sandboxed-native-demo), run manually on Ubuntu (CI can't host it).
- [ ] Add process/resource supervision appropriate to the supported host, explicit unsupported-platform errors, and tests for cancellation and orphan cleanup.
- [ ] Extend finite repair budgets to integrated-test failures where useful, retain diagnostics/usage per attempt, and stop on repeated/stalled changes.
- [x] Keep provider transmission and additional host filesystem access explicitly authorized; never silently disable isolation to make a test pass. Evidence: `migrate` without `--response` or `--allow-provider` is refused before anything is sent (CLI test); `execution.read_only_paths` is refused without the caller's `--allow-host-paths` (runner test); an unusable or unavailable sandbox is an error, never a fallback to no isolation (backend-selection tests).

Acceptance: the full native demo succeeds inside the supported sandbox, the same path fails closed when unavailable, and resource/repair exhaustion leaves the previous accepted checkpoint intact. Hosts blocking user namespaces remain a documented validation blocker rather than an implicit exception.

## 5. Reports and completion gate

- [ ] Present original/current workload metrics, variability, acceptance/rejection reason, build/test evidence, repair history, remaining runtime dependencies and evidence limitations together.
- [ ] Preserve JSON schema versioning and test report generation for accepted, rejected, interrupted and rolled-back sessions.
- [ ] Update README, skill references, changelog and roadmap with verified capabilities only.
- [ ] Pass workspace formatting/Clippy/standard tests, real native integration tests, checkpoint fault tests, and the successful sandbox demonstration on a suitable host.

## First implementation slice

**Implemented:** baseline and accepted-workspace protection from ordinary test/workload writes; positional/keyword mutation capture on return and exception paths; separate behavior/timing processes and fresh inputs; observation schema 2 with readable legacy evidence. Regression tests first reproduced canonical snapshot pollution and missing argument evidence. The real `python-mutation` fixture exercises a candidate that passes existing tests but fails mutation comparison, corrected-code resume, and rollback with checkpoint hashes preserved.

**Additional verified slice (§2):** explicit return-float absolute/relative
tolerances and call-output line-ending normalization, disabled by default.
Config/comparator/report regressions preserve exact mutations, errors, types,
dictionary keys/order and import output, validate limits, retain raw
observations and the frozen policy, and load old configs with exact defaults.
The `python-comparison` fixture captures actual original/candidate Python
observations inside Bubblewrap and proves exact rejection versus scoped
acceptance; unrelated mutation/error changes remain rejected. This is not a
compiled PyO3 candidate or a sandboxed Cargo build.

**Next slice:** introduce a testable sampling/policy layer, interleaved original/candidate measurements, and explicit insufficient/noisy-evidence handling. Whole-application profiling, file-effect comparison, event history/fault injection and successful sandboxed Cargo execution remain unchecked work. The completed disposable-copy slice is not a claim of stronger OS isolation.

**Partial §2 file-effect slice:** explicitly configured regular files now have
bounded before/after presence, size and SHA256 evidence, compared exactly and
reported with their limits. Sandboxed Python fixtures prove creation/change/
deletion/error-path capture and reject a same-length byte mismatch despite
passing independent tests; malformed/legacy evidence fails closed. The compiled
native-candidate rejection and corrected replay were demonstrated on 2026-11-15,
closing the checkbox. This does not observe unlisted/transient/
import-time writes, metadata or external systems.

## Deferred

JavaScript/TypeScript, Go, Rust optimization, additional native provider protocols, automatic whole-repository migration, runtime elimination, Hyperoxidize, GitHub automation and a terminal dashboard remain outside milestone two. Preserve the existing architecture and split modules only when the implementation needs it.

## Working procedure

Work from the dedicated branch, keep published foundation versions available as immutable Git tags, and land one verified slice per commit. Update the checklist only when its acceptance evidence exists. Use deterministic providers in standard CI; live models remain explicit opt-in. Do not push generated `.oxidize/` sessions, caches, credentials or local benchmark artifacts.
