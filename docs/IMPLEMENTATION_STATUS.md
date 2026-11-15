# Initial implementation handoff

This records local implementation and validation of the v0.1.0 Working Foundation. See the root README and CHANGELOG for current publication and development status; this document retains the original validation evidence.

## Delivered

Five-crate Rust workspace and CLI; validated TOML; static Python AST analysis and dependency planning; bounded OpenAI-compatible provider and offline code responses; actual incremental PyO3 replacement; compiler checks and finite compiler repair; independent tests and typed differential cases; warmed workload benchmarks and measured acceptance; atomic locked checkpoints, replay, rollback and reports; portable agent skill, installer, fixtures, documentation and CI.

## Scope

The working migration adapter handles top-level synchronous Python functions with the documented signature restrictions. Other source languages are inventoried, not migrated. The provider abstraction has one implemented HTTP protocol (OpenAI-compatible Chat Completions). Local HTTP and credentialed remote HTTPS are configurable; no live external model was called during this implementation. Deterministic local HTTP tests exercise retries, payload/usage handling and compiler repair. Offline migration input is explicitly identified as pre-generated Rust.

## Checks executed

- Workspace build, formatting and Clippy with warnings denied: passed.
- `cargo test --locked --workspace`: 15 passed; 2 native tests opt-in by default.
- `cargo test --locked -p oxidize-engine --test engine -- --ignored --nocapture --test-threads=1`: both native tests passed. These actually compile/load PyO3 and exercise compiler repair, migration acceptance, rejection of wrong code, saved-state resume, and rollback.
- Documented saved CLI demo: analysis, plan, migration, verification, benchmark and report completed; integrated `main.py 100` printed `25`.
- Skill installer copied the package to a selected local project and correctly refused a second installation over existing files. Manifest/reference validation passed. External agent skill discovery was not exercised.
- Known fixture ran with explicit trusted-code execution. Linux Bubblewrap user namespaces are blocked in this managed environment; fail-closed behavior was checked, but successful sandboxed execution is unvalidated.

The saved demonstration in the development workspace is under `work/demo/.oxidize/`, with its accepted application in `attempts/0001/project/` and detailed report in `report.md`/`report.json`. These generated files are excluded from Git; use the README's offline demo to reproduce them in a fresh clone. The toy prime workload produced 11 matching cases, seven timing samples per side, and a measured 471.556x component speedup: original median 93,688,296 ns/sample (MAD 18,708,485), integrated candidate median 198,679 ns/sample (MAD 6,039). The replacement also changes the algorithm to a sieve; this result is not an estimate of gains for arbitrary projects or whole-application speed. Sequential timing and concurrent machine load affect it. A separate candidate remeasurement gave 191,298 ns/sample.

## Build and run

From the checkout, `cargo build --locked --workspace`; use `cargo run --locked -p oxidize-cli -- --help`. Follow README's offline demo to reproduce without model credentials. Install the portable skill using `python3 skills/oxidize/scripts/install.py --agent codex --project /path/to/project`. Other agent-specific destinations and limits are in `docs/AGENT_SKILL.md`.

## Next priorities

Validate and harden sandboxed execution/resource controls, broaden behavioral evidence and Python signature/package support, strengthen benchmark statistics, add automatic multi-component orchestration and budgets, then implement JavaScript/TypeScript, Go and Rust optimization adapters. Native provider protocols, Git URL cloning, GitHub automation, Hyperoxidize, entrypoint migration and original runtime elimination remain future work. The detailed sequence is in `docs/ROADMAP.md`.
