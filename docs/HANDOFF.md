# Handoff

Where the work stands, for whoever picks it up next. Keep this current: update it when you
claim work, finish a slice, or hand off (see `CRAB OUT` in [AGENTS.md](../AGENTS.md)).

**Last updated:** 2026-11-15, by Ferriscope (GPT-6.1 Sol).

## State

- **`main`:** v0.1.1, the Working Foundation (Python/PyO3 path). Tagged releases are immutable.
- **Working branch:** `milestone-2-performance-reliability`, toward v0.2.0. The plan and its
  checklist are in [MILESTONE_2.md](MILESTONE_2.md).
- **Done on the branch** (6ad68b3, ticked in MILESTONE_2.md with evidence):
  - behaviour observation separated from timing (§1)
  - argument-mutation comparison (§2)
  - baseline tests run in disposable validation copies (§3)
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
  item remains open until native acceptance/rejection evidence exists.

## In progress

Claim work here: `- <who> · <what> · <date started>`

- None. Scoped observation/comparison slice is ready for review.

## Next up (unclaimed)

In rough order. Each is a checklist item in [MILESTONE_2.md](MILESTONE_2.md):

1. §2: finish the controlled file-effect gate with a compiled native candidate
   that passes independent tests but changes a configured file effect; demonstrate
   rejection and successful corrected-code replay inside the sandbox. Capture/
   comparison/reporting are already implemented; don't duplicate them.
2. §2: keep existing independent tests separate from generated ones; show case provenance and
   coverage limits in JSON and Markdown.
3. §4: validate a successful Bubblewrap build/test flow (the owner's machine can host it).
4. §3: versioned event history and a checkpoint schema-upgrade strategy that preserves v0.1.0
   sessions.

The long-term direction, beyond this milestone: more source languages (JavaScript/TypeScript
and Go first, per [ROADMAP.md](ROADMAP.md)), then whole programs. The goal is almost any
language to Rust.

## Open questions for the owner

- §4 follow-up: Runner binds its workspace before mounting isolated `/tmp`,
  so projects under host `/tmp` are hidden and fail to chdir. The new optional
  comparison integration test uses ignored `target/` for its disposable
  projects; it does not disable isolation or change the runner. Resolve/test
  mount ordering in the execution slice, not as part of comparison policies.
- Review/decide later whether additional normalization scopes are wanted.
  This slice intentionally does not normalize returned strings, exception
  messages, import output, argument snapshots, dictionary keys or order.
- The new file-effect slice does not run a compiled PyO3 candidate. Keep the
  full §2 box open until that evidence is available, preferably alongside §4's
  sandboxed native build/test flow. Don't silently add `--trust-code` to get
  the demonstration through.

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
