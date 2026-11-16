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

## Platforms: portable by construction, proven per platform

A port that must run on Windows (or macOS) can't be proven in a Linux sandbox: a Linux
container builds a Linux module, and paths, line endings, file locking, encodings and process
handling all differ. Even the *original* Python behaves differently. So:

- **Sandboxes:** `execution.sandbox` already selects Bubblewrap, Podman or Docker (`auto` by
  default). The container backend still needs validating against a real engine with a
  digest-pinned toolchain image, on Linux first.
- **A portability gate:** for ports marked as needing Windows, run
  `cargo check --target x86_64-pc-windows-msvc` inside the Linux sandbox and reject ports
  that use Unix-only APIs or crates. It's cheap and catches compile breakage early. PyO3's
  `abi3` + `generate-import-lib` can cross-build a `.pyd`, but building isn't testing.
- **Proof on the target:** run the same independent tests, differential cases and
  benchmarks on a real Windows host (a Windows CI runner, Windows Sandbox, a VM, or the
  user's machine), and record the platform with every piece of evidence.
- **The rule:** never claim a platform whose tests didn't run on that platform.
- **Cloud proof first, cheaply:** a GitHub Actions matrix (Linux, Windows, macOS runners,
  free for public repos) running the build and tests gives real per-platform evidence
  without owning the machines.
- **A `remote` sandbox backend, later and opt-in:** a hosted microVM/container service
  (candidates: E2B, Daytona, Modal, Vercel Sandbox, Cloudflare, Google Cloud Run sandboxes).
  **Target E2B's API first:** Pollinations runs E2B- and Azure-hosted Linux sandboxes behind an
  E2B-compatible API (`https://gen.pollinations.ai/alpha/e2b`, paid in Pollen, alpha), so one
  backend covers both.
  It would upload a workspace snapshot, run the whole build/test batch in one go, and return
  the output and evidence labelled with where it ran. Code leaving the machine must be
  explicitly authorized, like provider transmission. API keys never enter the sandbox. Cache
  toolchain images to keep it fast.
- **Providers:** Pollinations is OpenAI-compatible at `https://gen.pollinations.ai/v1`, with
  coding models such as `qwen/qwen3-coder-next` and `openai/gpt-5.3-codex`, so it should work
  through the generic `[provider]` configuration (`endpoint` plus `api_key_env`). Live-test it
  (opt-in) before claiming support.
- **Dependency vetting:** Pollinations' hosted Packages MCP reports crates.io/npm/PyPI versions
  and OSV vulnerabilities, a candidate check before Oxidize adds a dependency to a port.

Do not create empty crates for these features or mark them complete because a command name exists.
