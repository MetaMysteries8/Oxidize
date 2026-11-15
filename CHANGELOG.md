# Changelog

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
