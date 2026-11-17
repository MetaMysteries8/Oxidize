# Milestone 2 — Performance and Reliability

**Status: checklist complete on the development branch (20/20, gate passed 2026-11-16). Target: v0.2.0, released when the owner merges and tags it.** Baseline: v0.1.1 Working Foundation, including the checkpoint-lock recovery fix. Development branch: `milestone-2-performance-reliability`; package version: `0.2.0-dev`.

The goal is to make the existing Python/PyO3 migration path more reliable and its evidence more useful before adding source languages. The foundation already has basic differential testing, component timing, compiler repair, isolated copies, rollback and reporting. This milestone strengthens those implementations rather than treating them as missing or claiming planned capabilities are complete.

## 1. Reproducible workloads and profiling

- [x] Separate behavior observation from timing so repeated performance runs cannot alter correctness evidence.
- [x] Introduce a testable workload/sampling layer with symmetric warm-up, fresh inputs, interleaved original/candidate batches, retained raw timings, and a configurable noise/uncertainty policy. Evidence: `sampling.rs` (alternating order, per-round speedups, inconclusive on missing, short, non-finite, out-of-order or noisy data, slowest round decides) with unit tests. `performance.rounds` and `max_relative_mad` are config-tested. A real PyO3 acceptance was decided by 3 interleaved rounds whose raw samples are retained ([verification](VERIFICATION.md#interleaved-measurement-and-application-workloads)).
- [x] Record interpreter/toolchain, platform, command, warm-up/sample policy, scope and acceptance threshold with every measurement. Evidence: `Benchmark.interpreter` (reported by the harness from inside the run) and `Benchmark.context` (OS/arch, isolation backend, command, iterations, warm-up, calls per sample, threshold, scope), shown as a "Measured with …" report line; older evidence loads and says its context wasn't recorded (engine unit tests; sandboxed demo report). The context also records `rustc --version` as seen inside the isolated run.
- [x] Add user-defined whole-application workloads; collect wall time and, on supported platforms, child CPU/peak RSS. Explicitly mark unsupported/unavailable measurements. Evidence: `[[performance.workloads]]` and `oxidize workloads`. The wrapper is tested on a real 80 MiB child and a failing command; unsupported CPU/RSS is named in the report. On a real accepted migration, `python3 main.py 30000` measured wall 1.97x and CPU 1.99x on one Linux host ([verification](VERIFICATION.md#interleaved-measurement-and-application-workloads)).

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
- [x] Add deterministic interruption/fault-injection tests around generation, integration, evidence collection and atomic promotion, including write failures. Evidence: an injector sees every checkpoint save (before and after the atomic write) and every file-writing step. Standard tests crash, tear and fail every point before the native build, and an opt-in real PyO3 test crashes every point from the build to promotion. After each fault the previous accepted workspace is preserved or the new one is complete, nothing partial is reported as accepted, the input is unchanged and resume completes. Writing these tests found and fixed an attempt-start write failure that left no rejection ([verification](VERIFICATION.md#checkpoint-fault-injection)).
- [x] Make resume and repeated rollback safe and reproducible without losing unrelated input edits or reusing stale evidence. Evidence: a replay clears the earlier run's speedup, candidate fingerprint and repair count (an interrupted resume used to report the old speedup), and replaying the same response no longer duplicates the generation. Input edits make resume refuse without touching them. The opt-in native fault test rolls back repeatedly to a byte-identical first checkpoint ([verification](VERIFICATION.md#checkpoint-fault-injection)).

Acceptance: after each injected interruption, reopening either preserves the old accepted workspace or resumes a complete new one; partial output is never reported as accepted. Input files remain unchanged and failed attempts remain inspectable.

## 4. Execution and bounded repair

- [x] Validate a successful Bubblewrap build/test flow on a Linux host with user namespaces, including Cargo cache/lock and toolchain mount handling. Evidence: [the sandboxed native demo](VERIFICATION.md#sandboxed-native-demo), run manually on Ubuntu (CI can't host it).
- [x] Add process/resource supervision appropriate to the supported host, explicit unsupported-platform errors, and tests for cancellation and orphan cleanup. Evidence: heartbeat tests show that normal exit, timeout and cancellation leave no running descendant, and that inside Bubblewrap even a `setsid` escapee is stopped. Resource limits the backend can't enforce (Bubblewrap, `--trust-code`) are now refused by name instead of silently ignored. Limits: `--trust-code` on Windows kills only the direct child; container limits aren't engine-validated ([verification](VERIFICATION.md#process-supervision)).
- [x] Extend finite repair budgets to integrated-test failures where useful, retain diagnostics/usage per attempt, and stop on repeated/stalled changes. Evidence: compiler and integrated-test failures share one bounded repair loop, and each repair keeps its trigger, diagnostics and generation (usage is per generation). Real PyO3 tests with a local provider show a test-failure repair accepted, a repeating provider stopped as stalled, and a zero budget rejected with the diagnostics ([verification](VERIFICATION.md#repair-from-integrated-test-failures)).
- [x] Keep provider transmission and additional host filesystem access explicitly authorized; never silently disable isolation to make a test pass. Evidence: `migrate` without `--response` or `--allow-provider` is refused before anything is sent (CLI test); `execution.read_only_paths` is refused without the caller's `--allow-host-paths` (runner test); an unusable or unavailable sandbox is an error, never a fallback to no isolation (backend-selection tests).

Acceptance: the full native demo succeeds inside the supported sandbox, the same path fails closed when unavailable, and resource/repair exhaustion leaves the previous accepted checkpoint intact. Hosts blocking user namespaces remain a documented validation blocker rather than an implicit exception.

## 5. Reports and completion gate

- [x] Present original/current workload metrics, variability, acceptance/rejection reason, build/test evidence, repair history, remaining runtime dependencies and evidence limitations together. Evidence: a per-attempt summary (status, speed with range or no claim, repairs, reason), plain-language status, and measurement, checks, repairs, workloads, runtime dependencies and limits in one report ([verification](VERIFICATION.md#reports)).
- [x] Preserve JSON schema versioning and test report generation for accepted, rejected, interrupted and rolled-back sessions. Evidence: `report.rs` generates and checks all four, plus a new session. `report.json` keeps `schema_version` 2, `events` and the raw samples, and v0.1.0 checkpoints upgrade with a recorded event ([verification](VERIFICATION.md#reports)).
- [x] Update README, skill references, changelog and roadmap with verified capabilities only. Evidence: README status, table and "What works"; `skills/oxidize/references/performance.md`; CHANGELOG sections per slice; ROADMAP marks Milestone 2 done, lists what is still open and adds ACP only as planned. Each claim links to its evidence in VERIFICATION.md.
- [x] Pass workspace formatting/Clippy/standard tests, real native integration tests, checkpoint fault tests, and the successful sandbox demonstration on a suitable host. Evidence: [the completion gate](VERIFICATION.md#milestone-2-completion-gate), 2026-11-16 on Ubuntu: fmt and Clippy clean, 77 standard tests passed, 87 passed with every opt-in native test (including the 21-point native fault test), and the sandboxed native demo passed in Bubblewrap without `--trust-code`. CI (Linux, macOS, Windows) passes.

## How it was delivered

One verified slice per commit, test-first where it could be, each with its evidence in
[VERIFICATION.md](VERIFICATION.md) and its entry in the [changelog](../CHANGELOG.md). Writing
the tests found and fixed real bugs: a silently swapped sandbox interpreter, a missing
`/etc/alternatives` mount, an attempt-start write failure that left no rejection, a resume that
reported the previous run's speedup, duplicated replay generations, unbounded build caches, and
container-only resource limits that were silently ignored elsewhere.

## Deferred

JavaScript/TypeScript, Go, Rust optimization, additional native provider protocols, automatic whole-repository migration, runtime elimination, Hyperoxidize, GitHub automation and a terminal dashboard remain outside milestone two. Preserve the existing architecture and split modules only when the implementation needs it.

## Working procedure

Work from the dedicated branch, keep published foundation versions available as immutable Git tags, and land one verified slice per commit. Update the checklist only when its acceptance evidence exists. Use deterministic providers in standard CI; live models remain explicit opt-in. Do not push generated `.oxidize/` sessions, caches, credentials or local benchmark artifacts.
