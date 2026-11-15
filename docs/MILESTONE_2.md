# Milestone 2 — Performance and Reliability

**Status: preparation complete; implementation pending. Target: v0.2.0.** Baseline: v0.1.1 Working Foundation, including the checkpoint-lock recovery fix. Development branch: `milestone-2-performance-reliability`; package version: `0.2.0-dev`.

The goal is to make the existing Python/PyO3 migration path more reliable and its evidence more useful before adding source languages. The foundation already has basic differential testing, component timing, compiler repair, isolated copies, rollback and reporting. This milestone strengthens those implementations rather than treating them as missing or claiming planned capabilities are complete.

## 1. Reproducible workloads and profiling

- [ ] Separate behavior observation from timing so repeated performance runs cannot alter correctness evidence.
- [ ] Introduce a testable workload/sampling layer with symmetric warm-up, fresh inputs, interleaved original/candidate batches, retained raw timings, and a configurable noise/uncertainty policy.
- [ ] Record interpreter/toolchain, platform, command, warm-up/sample policy, scope and acceptance threshold with every measurement.
- [ ] Add user-defined whole-application workloads; collect wall time and, on supported platforms, child CPU/peak RSS. Explicitly mark unsupported/unavailable measurements.

Acceptance: deterministic sampler/policy tests cover invalid/insufficient data, a real repeatable workload demonstrates measurement through the integrated interface, and noisy or missing evidence cannot silently become a speed claim. Do not impose brittle machine-specific speedup assertions in normal unit tests.

## 2. Stronger behavioral evidence

- [ ] Compare supported argument mutation alongside current typed returns, errors, stdout and stderr.
- [ ] Add explicit opt-in normalization/tolerance policies with exact comparison remaining the default.
- [ ] Add controlled file-effect fixtures and comparisons with clear scope; record external effects that remain unverified.
- [ ] Keep existing independent tests separate from generated tests and make case provenance/coverage limits visible in JSON and Markdown.

Acceptance: a compilable candidate that passes existing tests but changes an observed mutation, error, return type or configured file effect is rejected. Intentional tolerance/normalization is tested without masking unrelated changes.

## 3. Transaction and checkpoint recovery

- [ ] Run baseline tests in disposable validation copies so tests cannot corrupt the canonical baseline checkpoint.
- [ ] Add a versioned event history and explicit checkpoint schema-upgrade strategy while preserving v0.1.0 sessions.
- [ ] Add deterministic interruption/fault-injection tests around generation, integration, evidence collection and atomic promotion, including write failures.
- [ ] Make resume and repeated rollback safe and reproducible without losing unrelated input edits or reusing stale evidence.

Acceptance: after each injected interruption, reopening either preserves the old accepted workspace or resumes a complete new one; partial output is never reported as accepted. Input files remain unchanged and failed attempts remain inspectable.

## 4. Execution and bounded repair

- [ ] Validate a successful Bubblewrap build/test flow on a Linux host with user namespaces, including Cargo cache/lock and toolchain mount handling.
- [ ] Add process/resource supervision appropriate to the supported host, explicit unsupported-platform errors, and tests for cancellation and orphan cleanup.
- [ ] Extend finite repair budgets to integrated-test failures where useful, retain diagnostics/usage per attempt, and stop on repeated/stalled changes.
- [ ] Keep provider transmission and additional host filesystem access explicitly authorized; never silently disable isolation to make a test pass.

Acceptance: the full native demo succeeds inside the supported sandbox, the same path fails closed when unavailable, and resource/repair exhaustion leaves the previous accepted checkpoint intact. Hosts blocking user namespaces remain a documented validation blocker rather than an implicit exception.

## 5. Reports and completion gate

- [ ] Present original/current workload metrics, variability, acceptance/rejection reason, build/test evidence, repair history, remaining runtime dependencies and evidence limitations together.
- [ ] Preserve JSON schema versioning and test report generation for accepted, rejected, interrupted and rolled-back sessions.
- [ ] Update README, skill references, changelog and roadmap with verified capabilities only.
- [ ] Pass workspace formatting/Clippy/standard tests, real native integration tests, checkpoint fault tests, and the successful sandbox demonstration on a suitable host.

## First implementation slice

Start with **baseline immutability and argument-mutation evidence**. Add independent fixtures that expose each current gap, implement disposable baseline validation and mutation capture/comparison, then exercise rejection/resume/rollback on the real PyO3 path. This supplies concrete correctness coverage before changing sampling or acceptance statistics. Keep this slice reviewable before taking on the later profiling and sandbox work.

## Deferred

JavaScript/TypeScript, Go, Rust optimization, additional native provider protocols, automatic whole-repository migration, runtime elimination, Hyperoxidize, GitHub automation and a terminal dashboard remain outside milestone two. Preserve the existing architecture and split modules only when the implementation needs it.

## Working procedure

Work from the dedicated branch, keep published foundation versions available as immutable Git tags, and land one verified slice per commit. Update the checklist only when its acceptance evidence exists. Use deterministic providers in standard CI; live models remain explicit opt-in. Do not push generated `.oxidize/` sessions, caches, credentials or local benchmark artifacts.
