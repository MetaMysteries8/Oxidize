# Handoff

Where the work stands, for whoever picks it up next. Keep this current: update it when you
claim work, finish a slice, or hand off (see `CRAB OUT` in [AGENTS.md](../AGENTS.md)).

**Last updated:** 2026-11-16, by Claude.

## State

- **`main`:** v0.1.1, the Working Foundation (Python/PyO3 path). Tagged releases are immutable.
- **Working branch:** `milestone-2-performance-reliability`, toward v0.2.0. The plan and its
  checklist are in [MILESTONE_2.md](MILESTONE_2.md).
- **Done on the branch** (ticked in MILESTONE_2.md with evidence; 10/20):
  - behaviour observation separated from timing (§1)
  - argument-mutation comparison (§2)
  - baseline tests run in disposable validation copies (§3)
  - comparison policies, file effects (native demo), case provenance (§2)
  - measurement context incl. interpreter and rustc (§1)
  - Bubblewrap validation, explicit authorization (§4)
  - versioned event history and checkpoint schema upgrades, v0.1.0 sessions preserved (§3)
- **CI:** Linux (fmt, Clippy, tests, native PyO3 tests) plus blocking macOS/Windows build+tests.
- **Baseline, 2026-11-15** (Linux, Rust 1.100.0, Python 3.13): `cargo build --locked
  --workspace` OK, `cargo test --locked --workspace` gave 20 passed, 0 failed, 3 ignored (the
  opt-in real PyO3 tests).
- **Host note for §4:** Bubblewrap user-namespace sandboxes work on the owner's Ubuntu machine,
  even with `apparmor_restrict_unprivileged_userns=1`. That machine can validate the
  sandboxed build/test flow.
- **Completed §2 policy slice:** opt-in returned-float absolute/relative tolerance
  and call stdout/stderr line-ending normalization; exact remains the default.
  Mutation/error/type/key/order/import checks stay exact. Raw observations are
  unchanged; the frozen config and reports retain the policy. See
  [configuration, evidence and limits](VERIFICATION.md#explicit-comparison-opt-ins).
- **Controlled file-effect observation slice:** opt-in bounded regular-file
  snapshots and exact comparisons are implemented. Config/comparator/report
  tests and sandboxed Python observation fixtures pass. The full §2 checklist
  item is now ticked: the native rejection and replay were demonstrated 2026-11-15.

## In progress

Claim work here: `- <who> · <what> · <date started>`

- None.

## Next up (unclaimed)

In rough order. Each is a checklist item in [MILESTONE_2.md](MILESTONE_2.md):

1. §3: deterministic interruption/fault-injection tests around generation, integration,
   evidence collection and atomic promotion, including write failures. The event history
   (`crates/oxidize-engine/src/history.rs`) gives these tests something to assert against.
2. **Container sandbox validation:** run the sandboxed native demo with
   `execution.sandbox = "podman"` (or `"docker"`) and a toolchain image pinned by digest;
   fix whatever breaks (likely candidates: the offline Cargo registry inside the image,
   PyO3 finding the image's Python, and Podman's SELinux volume labels). The backend is
   implemented and unit-tested (`crates/oxidize-core/src/process.rs`), but no engine has
   run it yet. Installing Podman or Docker needs the owner's yes; vet it first.
3. **Windows:** the portability gate (`cargo check --target x86_64-pc-windows-msvc`) and
   proof on a real Windows host. See [ROADMAP.md](ROADMAP.md#platforms-portable-by-construction-proven-per-platform).

The long-term direction, beyond this milestone: more source languages (JavaScript/TypeScript
and Go first, per [ROADMAP.md](ROADMAP.md)), then whole programs. The goal is almost any
language to Rust.

## Open questions for the owner

- §4 follow-up: Runner binds its workspace before mounting isolated `/tmp`,
  so projects under host `/tmp` are hidden and fail to chdir. The new optional
  comparison integration test uses ignored `target/` for its disposable
  projects; it does not disable isolation or change the runner. Resolve/test
  mount ordering in the execution slice, not as part of comparison policies.
- §3 upgrades are one-way. v0.1.x can't read a schema-2 checkpoint; restoring `state.v1.json`
  works but drops everything recorded since. Is that acceptable, or do you want a
  `downgrade` command?
- Review/decide later whether additional normalization scopes are wanted.
  This slice intentionally does not normalize returned strings, exception
  messages, import output, argument snapshots, dictionary keys or order.

## Handoff history

- 2026-11-15 · Claude · Created AGENTS.md (contributor and agent rules, the `CRAB OUT`
  codeword) and this file; checked the baseline; hardened `.gitignore` for contributors.
- 2026-11-15 · Hype Agent · Completed §2 item 2 test-first. The initial config
  fixture failed with unknown `verification.comparison`; policy and typed
  comparisons now support scoped opt-ins. Config/comparator regressions plus
  the optional real Bubblewrap/Python fixture verify exact-vs-policy behavior,
  unrelated-change rejection, raw-evidence preservation and policy reporting.
  No new dependencies, provider calls, native adapter or speedup claim.
  Branch remains `milestone-2-performance-reliability`; commits are local only.
  Validation: `cargo fmt --all -- --check`, Clippy workspace/all-targets with
  `-D warnings`, and locked/offline workspace tests pass (28 passed, 4 ignored).
  Explicitly running `cargo test --locked --offline -p oxidize-engine --test
  comparison -- --include-ignored` also passes all 6 comparison tests,
  including the real Bubblewrap/Python fixture. The other 3 ignored native
  PyO3 tests were not rerun for this slice. Next: independent review, then the
  next unclaimed milestone item; pushing still needs the owner's approval.
- 2026-11-15 · Hype Agent · Policy slice initially committed locally as `dc7b939`.
  Added owner-approved optional Support Oxidize section in a separate docs
  commit, using `hype-wallet receive 0.01 USDC --mainnet` output and the plain
  public address. It explicitly buys nothing and promises no investment or
  return. No wallet send, external post or push performed. Ask the owner
  before pushing either commit; Claude's review request is in the shared
  mission log. Temporary `oxidize-awake` user service and desktop caffeine
  were enabled at the owner's request; leave them until the owner is done.
- 2026-11-15 · Ferriscope · Owner authorized pushing the two verified commits.
  GitHub rejected their private author email. Re-authored only those unpublished
  commits with the same public GitHub no-reply identity already used on this
  branch, rerunning fmt/Clippy/workspace tests before each amended commit.
  Ordinary fast-forward push succeeded: policies `3dc4157`, support `f2d3a5f`.
  No published history was rewritten and no Git/account settings were changed.
  Added owner's requested credit line using the codename Ferriscope.
- 2026-11-15 · Ferriscope · Added scoped file-effect observation/comparison
  test-first (initial config fixture failed on unknown `file_effect_paths`).
  Defaults remain off; explicit paths record before/after presence, size and
  SHA256 including exception paths. Snapshots are exact even with normalization
  enabled, and raw contents are not stored. Missing/legacy snapshots fail closed
  for configured scopes. Paths/types/size limits and remaining external-effect
  limitations are documented.
  Validation: fmt and workspace/all-target Clippy `-D warnings` pass;
  locked/offline workspace tests: 34 passed, 6 ignored. Explicit comparison
  suite: 6 passed; explicit file-effect suite: 5 passed (both with
  `--include-ignored`, including actual Bubblewrap/Python runs).
  The Python candidate passes the independent return/error tests but is
  rejected for changed bytes. No native file-effect acceptance claim, new
  dependencies, live provider call, or `--trust-code` fallback. Next steps
  are listed above. Credit line is in separate local commit `287ab42`.
  This new slice is local for review; the earlier policy/support commits are
  already pushed. Ask owner before pushing new commits.
- 2026-11-15 · Claude · Review of 3dc4157..1ae8d68: checks pass (34 passed, 0 failed); exact comparison stays the default; limits documented. Fixed: README credit now names the owner; the tip request is copyable text, since GitHub strips `solana:` links.
- 2026-11-15 · Claude, for Hype Agent (its ChatGPT usage ran out mid-CRAB OUT) · Wrap-up: tree clean and in sync with origin at 6ec0662; checks pass (fmt, Clippy -D warnings, 34 passed / 0 failed / 6 opt-in ignored); nothing in progress. Next: pick from "Next up" above.
- 2026-11-15 · Claude · §4 Bubblewrap validation done (ticked, with evidence in VERIFICATION.md): the full native demo passes in the sandbox without --trust-code. Fixed two blockers: /etc/alternatives mounted for `cc`, and refusing commands whose host binary the sandbox can't see (it was silently swapping Python 3.13 for 3.14).
- 2026-11-15 · Claude · Pluggable sandbox backends (auto/bubblewrap/podman/docker): containers locked down and digest-pinned, implemented and unit-tested, not yet run against a real engine. Platform plan written into ROADMAP.md. Next up #4 and #5 added.
- 2026-11-15 · Claude · Roadmap: cloud per-platform proof (a GitHub Actions matrix first), an opt-in `remote` sandbox backend (E2B/Daytona/Modal/…), and Pollinations as a candidate low-cost provider. No Pollinations sandbox found. A fresh session should pick up Next up #4/#5 (this one's context is full).
- 2026-11-15 · Claude · CI: added a non-blocking `portability` job (macOS + Windows: build + standard tests). Its results are portability findings, not build breaks. Make a platform blocking once it passes.
- 2026-11-15 · Claude · The portability CI job passes on macOS and Windows (after the LF .gitattributes fix) and is now **blocking**. It covers the build and standard tests only; native PyO3 migration on macOS/Windows is still unvalidated.
- 2026-11-15 · Claude · §1 "record the context with every measurement" done (ticked): interpreter from inside the run, plus OS/arch, isolation, command, policy, threshold and scope, in the evidence JSON and as a report line. Gap: the rustc version is not recorded yet.
- 2026-11-15 · Claude · §4 "explicit authorization" ticked, with tests for provider transmission, host mounts, and no silent isolation fallback.
- 2026-11-15 · Claude · §2 "case provenance" ticked: per-attempt provenance in JSON + a report paragraph with the coverage limit. Milestone 2 is now 8/20.
- 2026-11-15 · Claude · §2 native file-effect demo done (ticked): wrong-bytes PyO3 candidate rejected in Bubblewrap, corrected replay accepted; answer files added to the fixture. Also recorded rustc per measurement. Milestone 2: 9/20. Tree clean, pushed, nothing in progress.
- 2026-11-16 · Claude · §3 event history + checkpoint schema upgrade done (ticked). Schema 2 adds `events`, saved atomically with each state change. v0.1.x checkpoints upgrade on open, and the original is kept as `state.v1.json`. Tested against a real checkpoint written by the v0.1.0 binary (`fixtures/legacy-v0.1.0-session`). Checks: fmt, Clippy -D warnings, 49 passed / 6 ignored; the engine suite with `--include-ignored` passes 11/11, including the real PyO3 builds. Milestone 2: 10/20.
