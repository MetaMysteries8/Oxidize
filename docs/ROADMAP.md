# Development roadmap

The project brief describes the destination; this checklist describes implementation status.

The v0.1.0 foundation is the first version. The next target is [Milestone 2 — Performance and Reliability](MILESTONE_2.md), developed on `milestone-2-performance-reliability`. Its detailed acceptance checklist defines what must be verified before v0.2.0; preparing the branch does not complete that milestone.

## Delivered foundation

- [x] Buildable Rust workspace, CLI, validated configuration.
- [x] Python AST components and limited static dependency graph.
- [x] Dependency-first, human-readable heuristic migration plan.
- [x] OpenAI-compatible provider, offline response input, bounded context selection.
- [x] Real top-level Python function replacement using pinned PyO3.
- [x] Existing tests, typed differential JSON cases, warmed component timings.
- [x] Measured acceptance and rejection, compiler-guided repair with finite limits.
- [x] Atomic checkpoints, lock, replay, pointer-based rollback, truthful reports.
- [x] Portable skill, two workflows, install helper and agent-specific docs.
- [x] Actual native migration demo, rejection/recovery tests, deterministic HTTP mock.

## Next: strengthen this vertical slice

Milestone two is in progress. Its first slice adds disposable validation copies, input-mutation comparison (including exception paths), and separate observation/timing processes with fresh inputs. See [the milestone checklist](MILESTONE_2.md) for the remaining acceptance gates.

1. Validate successful Bubblewrap execution on supported Linux hosts; add cgroup/process/memory/disk limits, stronger supervision and crash/fault-injection tests. Current environment blocks user namespaces, so only fail-closed behavior is validated here.
2. Extend the delivered input-mutation evidence to file effects, import/initialization semantics, richer signatures, Python packages, object identity, numerical tolerances and nondeterministic outputs.
3. Interleave timing samples, add confidence policies and whole-application workloads, collect CPU/peak RSS/allocations, measure bridge-only overhead and retain profiler artifacts.
4. Repair integrated test mismatches within explicit budgets and maintain an append-only event journal, schema upgrades, and recovery from disk-full/atomic-write failure.
5. Add automatic candidate selection/replanning/repetition, per-task token/cost budgets, task-specific model routing and user cancellation during in-flight HTTP.

## Then: expand capabilities

- JavaScript/TypeScript with AST parsing and napi-rs, then Go/cgo and native Rust optimization; each needs real end-to-end fixtures before support claims.
- AST/type/ownership-aware interop adapters and dependency grouping/cycles.
- Native Anthropic/Gemini/Ollama/Pollinations/custom protocols, capabilities, fallback models, streaming and usage accounting. OpenAI-compatible servers can already be configured generically, but individual services have not been live tested.
- Explicit Git URL cloning/revision selection, isolated worktrees, local checkpoint commits, opt-in PR automation. No pushes by default.
- Rust entrypoint migration, runtime dependency removal, boundary consolidation and Hyperoxidize.
- Large-repository incremental indexing/cache and terminal dashboard with actual metrics.
- External-agent discovery/invocation tests and live-model migrations gated behind explicit opt-in credentials.

Do not create empty crates for these features or mark them complete because a command name exists.
