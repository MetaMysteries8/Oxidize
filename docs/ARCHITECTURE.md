# Architecture and invariants

`oxidize-core`: typed validated TOML; safe relative paths; bounded file discovery; SHA256 source fingerprints; snapshots; atomic JSON writes; child process runner.

`oxidize-analysis`: embedded Python AST parser (stdlib only, no project imports), source spans, function signatures, static calls/imports and blockers; dependency-first heuristic planner; persistent roadmap. Other language extensions are inventoried, not parsed into migration candidates.

`oxidize-providers`: a provider trait, OpenAI-compatible blocking HTTP client, explicit offline response provider, typed code/usage records. The engine owns orchestration rather than allowing a model to issue arbitrary commands or dependency changes.

`oxidize-engine`: session lock, versioned state, immutable baseline and separate attempt copies, selected context, fixed PyO3 crate generation, compiler-guided repair, source replacement, independent tests, typed differential comparisons, warmed measurement, acceptance, reports, replay and rollback.

`oxidize-cli`: Clap commands and explicit source-transmission/execution flags, cancellation handler, printed output workspace/report locations. No simulated commands for future features.

## Transaction

1. Capture a baseline and its source revision/fingerprint. Freeze validated configuration.
2. Analyze the current accepted application, select an eligible component and its supplied cases, and record a prepared attempt referencing its base.
3. Make a separate candidate. Before generating code, run independent tests and behavior/benchmark workloads in disposable copies of the base. Each stage gets a fresh copy and process; test writes cannot seed observations or modify the canonical baseline.
4. Persist each model response and usage. Generate the pinned PyO3 manifest and perform bounded formatting/checking/lint/build repairs. Detect identical responses.
5. Install the native artifact using the configured interpreter's extension suffix and replace only the function's AST line span.
6. Run independent tests, behavior capture and timing in separate disposable copies of the integrated candidate. Compare typed returns/errors/output and post-call positional/keyword inputs, including exception paths. Require exact supported observations and the configured measured speedup. Detect baseline/input edits.
7. Commit the accepted pointer and phase together through atomic state replacement. Rejected attempts leave the old accepted pointer unchanged. All attempts retain evidence.

An interrupted attempt is replayed from its base into a fresh copy. Old partial files are archived. This deliberately favors repeatability over attempting to continue a half-written build. Source revision metadata is supplementary: SHA256 captures noncommitted source too. Rollback moves a pointer rather than running a destructive Git operation.

Validation copies live under `.oxidize/validation/` and are removed by a scoped temporary-directory guard on success/error. Behavior and timing use the same embedded harness with distinct modes, invocation processes and copies. Observation results are serialized before timing begins; only successful original cases are reconstructed for timing. File effects are isolated but not compared. Trusted code can still access host paths outside a copy.

New evidence has `observation_schema_version = 2`. A missing field defaults to legacy version 1 for old checkpoints/reports, but the comparator refuses to claim argument-mutation equivalence for legacy/missing evidence. Resume replays saved code and captures new evidence. The top-level state schema remains 1; this additive field does not upgrade or rewrite historical observations into stronger evidence.

## Extension points

Add language-specific parsing and bridge generation behind an explicit capability contract before adding candidates. Split crates further only when there is real functionality to isolate. A new provider implements `Provider` and must be covered by deterministic transport/payload tests. A new comparator must describe supported observations, tolerances, and evidence limitations. Add state-schema migration before changing durable checkpoint semantics.
