# Architecture and invariants

`oxidize-core`: typed validated TOML; safe relative paths; bounded file discovery; SHA256 source fingerprints; snapshots; atomic JSON writes; child process runner.

`oxidize-analysis`: embedded Python AST parser (stdlib only, no project imports), source spans, function signatures, static calls/imports and blockers; dependency-first heuristic planner; persistent roadmap. Other language extensions are inventoried, not parsed into migration candidates.

`oxidize-providers`: a provider trait, OpenAI-compatible blocking HTTP client, explicit offline response provider, typed code/usage records. The engine owns orchestration rather than allowing a model to issue arbitrary commands or dependency changes.

`oxidize-engine`: session lock, versioned state, immutable baseline and separate attempt copies, selected context, fixed PyO3 crate generation, compiler-guided repair, source replacement, independent tests, typed differential comparisons, warmed measurement, acceptance, reports, replay and rollback.

`oxidize-cli`: Clap commands and explicit source-transmission/execution flags, cancellation handler, printed output workspace/report locations. No simulated commands for future features.

## Transaction

1. Capture a baseline and its source revision/fingerprint. Freeze validated configuration.
2. Analyze the current accepted application, select an eligible component and its supplied cases, and record a prepared attempt referencing its base.
3. Make a separate candidate. Run the base's independent tests and behavior/benchmark workloads before generating code.
4. Persist each model response and usage. Generate the pinned PyO3 manifest and perform bounded formatting/checking/lint/build repairs. Detect identical responses.
5. Install the native artifact using the configured interpreter's extension suffix and replace only the function's AST line span.
6. Run independent tests and differential workloads on the integrated application. Require exact supported observations and the configured measured speedup. Detect baseline/input edits.
7. Commit the accepted pointer and phase together through atomic state replacement. Rejected attempts leave the old accepted pointer unchanged. All attempts retain evidence.

An interrupted attempt is replayed from its base into a fresh copy. Old partial files are archived. This deliberately favors repeatability over attempting to continue a half-written build. Source revision metadata is supplementary: SHA256 captures noncommitted source too. Rollback moves a pointer rather than running a destructive Git operation.

## Extension points

Add language-specific parsing and bridge generation behind an explicit capability contract before adding candidates. Split crates further only when there is real functionality to isolate. A new provider implements `Provider` and must be covered by deterministic transport/payload tests. A new comparator must describe supported observations, tolerances, and evidence limitations. Add state-schema migration before changing durable checkpoint semantics.
