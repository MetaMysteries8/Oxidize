# Handoff

Where the work stands, for whoever picks it up next. Keep this current: update it when you
claim work, finish a slice, or hand off (see `CRAB OUT` in [AGENTS.md](../AGENTS.md)).

**Last updated:** 2026-11-15, by Claude (Opus 5.5, via Claude Code).

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

## In progress

Claim work here: `- <who> · <what> · <date started>`

- Hype Agent · §2 item 2: explicit opt-in normalization/tolerance policies, with exact
  comparison remaining the default · 2026-11-15

## Next up (unclaimed)

In rough order. Each is a checklist item in [MILESTONE_2.md](MILESTONE_2.md):

1. §2: controlled file-effect fixtures and comparisons; record which external effects remain
   unverified.
2. §2: keep existing independent tests separate from generated ones; show case provenance and
   coverage limits in JSON and Markdown.
3. §4: validate a successful Bubblewrap build/test flow (the owner's machine can host it).
4. §3: versioned event history and a checkpoint schema-upgrade strategy that preserves v0.1.0
   sessions.

The long-term direction, beyond this milestone: more source languages (JavaScript/TypeScript
and Go first, per [ROADMAP.md](ROADMAP.md)), then whole programs. The goal is almost any
language to Rust.

## Open questions for the owner

- None right now.

## Handoff history

- 2026-11-15 · Claude · Created AGENTS.md (contributor and agent rules, the `CRAB OUT`
  codeword) and this file; checked the baseline; hardened `.gitignore` for contributors.
