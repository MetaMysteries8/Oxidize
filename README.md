# 🦀 Oxidize

Progressively replace application components with Rust while checking behavior and measuring performance. Everything can be investigated for Rustification, including Rust itself; this first development slice implements a deliberately narrower, working Python/PyO3 path.

**Status: early development, not the completed universal engine.** This repository implements an actual component migration, not a disconnected rewrite. It leaves input source unchanged and writes accepted applications into separate checkpointed workspaces. See [the original project brief](docs/PROJECT_BRIEF.md) and [roadmap](docs/ROADMAP.md).

## Development status

**Published baseline: [v0.1.1 — Working Foundation](https://github.com/MetaMysteries8/Oxidize/tree/v0.1.1).** This is the **v0.2.0-dev development branch** for milestone two. The first slice adds disposable validation workspaces, argument-mutation comparisons, and separate behavior/timing processes. The full milestone is still in progress. This is early development, not universal language support.

| Milestone | Status | Scope |
| --- | --- | --- |
| 1 — Working Foundation (`v0.1.1`) | Implemented | Rust CLI, Python/PyO3 migration, OpenAI-compatible provider, compiler repair, tests, component benchmarks, checkpoints, rollback, reports, and portable agent skill. |
| 2 — Performance and Reliability (`v0.2.0`) | In progress — first slice implemented | Disposable validation, argument-mutation evidence, and separate behavior/timing runs. Profiling, sampling policy, broader effects, fault recovery, and sandbox validation remain planned. |
| 3 — More Languages | Future | JavaScript/TypeScript, Go, and existing-Rust optimization adapters. |
| 4 — Provider Expansion | Future | Additional native provider protocols and model routing. |
| 5 — Agent Skill Expansion | Early skill delivered; more validation planned | External-agent discovery and workflow validation. |
| 6 — Advanced Oxidization | Future | Entry-point replacement, runtime elimination, Hyperoxidize, large-repository support, and automation. |

Local validation for v0.1.1: **16 standard tests and 2 real PyO3 migration tests passed**, plus formatting, Clippy, and workspace build checks. Tests use deterministic responses; no live external model was called. A working offline demo is included below. CI results are available in [GitHub Actions](https://github.com/MetaMysteries8/Oxidize/actions).

Development-branch validation for the first milestone-two slice: **20 standard tests and 3 real native migration tests passed**, plus Clippy and formatting checks. This includes a compiled mutation-only mismatch that passes independent tests, is rejected by differential verification, then resumes with corrected code and rolls back with checkpoint hashes intact.

**Next development target:** [Milestone 2 — Performance and Reliability](docs/MILESTONE_2.md). Its acceptance checklist is planned work; unchecked items are not implemented features. See [CHANGELOG.md](CHANGELOG.md) for version history and [the detailed roadmap](docs/ROADMAP.md) for remaining work.

## What works

- Native Rust CLI and five modular crates: shared configuration/processes, source analysis/planning, providers, migration engine, CLI.
- Python AST analysis without importing project code; static direct-call/import graphs, syntax blockers, language/manifest/test/entrypoint detection, bounded source context, persistent JSON analysis and Markdown roadmap.
- OpenAI-compatible Chat Completions endpoints with arbitrary model identifiers, environment-based credentials, context/output limits, timeouts, bounded retries, and reported token usage. HTTPS remotely; HTTP for loopback model servers. No live provider is required for tests.
- One top-level synchronous Python function per migration, integrated through a real PyO3 0.30.1 extension. The original function definition is replaced by an import in the candidate copy; other functions and the Python entrypoint remain.
- Cargo formatting, checking, Clippy, release compilation, bounded compiler-guided model repairs and repeated-response detection.
- Existing tests before/after, differential JSON cases with typed results/errors/stdout/stderr, warmed workload benchmarks, raw timing samples, median and median absolute deviation, configurable minimum speedup, rejection on failure.
- Development branch: schema-2 observations also compare mutations to positional and keyword inputs, including before exceptions. Tests, behavior capture and timing run in separate disposable copies/processes; normal test/workload writes do not change canonical checkpoints.
- Development branch: exact comparison remains the default; explicit opt-ins allow tolerances for returned floats and line-ending normalization for call stdout/stderr. Mutations, errors, types, dictionary keys/order and import output stay exact; raw evidence and the frozen policy appear in reports. See [comparison policy configuration and limits](docs/VERIFICATION.md#explicit-comparison-opt-ins).
- Development branch: opt-in [controlled file snapshots](docs/VERIFICATION.md#controlled-file-effect-opt-in) compare presence, length and SHA256 of explicitly listed regular files before/after behavior calls, including exceptions. Unlisted/transient effects and metadata are unverified; native file-effect gate evidence is still pending.
- Locked sessions, atomic durable JSON checkpoints, source fingerprints, replayable resume, pointer-based rollback, retained failure evidence, Markdown/JSON reports.
- Linux Bubblewrap runner that fails closed when unavailable; explicit `--trust-code` escape hatch for code the user authorizes running without OS isolation. Child-process time/output limits, cancellation, reduced environment, no shell interpolation.
- Portable [agent skill](skills/oxidize/SKILL.md) with CLI-assisted and agent-native workflows, local installer, references, and agent-specific installation instructions.

Python is the only implemented migration adapter. JavaScript/TypeScript, Go, Rust and a few other languages are **detected only**. Hyperoxidize, automatic multi-component selection/repetition, package modules, async/decorated/generator/default/variadic signatures, alternate native provider protocols, cloning remote URLs, Git/PR automation, and full entrypoint/runtime removal are not implemented. No command pretends otherwise.

## Build

Requirements: stable Rust 1.100+ with rustfmt/Clippy, Python 3.9+ and its development headers, a C linker, and Cargo registry access for the initial dependency download. Tested on Linux with Rust 1.100.0 and Python 3.12. macOS native artifact naming is implemented but not validated here; Windows native migration is not supported.

```bash
cargo build --locked --workspace
cargo test --locked --workspace
cargo install --path crates/oxidize-cli --locked
oxidize --help
```

No paid API access is needed. The slower real native migration test is opt-in because it needs PyO3 cached in Cargo:

```bash
cargo fetch --locked --manifest-path examples/python-native/Cargo.toml
cargo test --locked -p oxidize-engine --test engine -- --ignored --nocapture
```

## Run a complete offline demonstration

The response fixture contains a real, inspectable Rust sieve implementation of the Python prime counter. It is a deterministic pre-generated implementation, not a live AI result. Builds and measurements are real.

```bash
cargo fetch --locked --manifest-path examples/python-native/Cargo.toml
mkdir -p work
cp -R fixtures/python-cli work/demo
cargo run --locked -p oxidize-cli -- analyze work/demo
cargo run --locked -p oxidize-cli -- plan work/demo
cargo run --locked -p oxidize-cli -- migrate work/demo \
  --component compute.py:count_primes \
  --response fixtures/python-cli/migration-response.json \
  --trust-code
cargo run --locked -p oxidize-cli -- verify work/demo --trust-code
cargo run --locked -p oxidize-cli -- benchmark work/demo \
  --component compute.py:count_primes --trust-code
cargo run --locked -p oxidize-cli -- report work/demo
python3 work/demo/.oxidize/attempts/0001/project/main.py 100
# 25
cargo run --locked -p oxidize-cli -- rollback work/demo
```

`--trust-code` is used above only for this repository's inspectable fixture. It permits builds, tests, and generated code to run with the invoking user's filesystem/network authority. Omit it and configure Bubblewrap for untrusted code; any extra `read_only_paths` require explicit `--allow-host-paths` after reviewing the paths. See [execution security](SECURITY.md). Reusing `work/demo` after rollback is supported; recopying over an existing session is not a clean start. Choose a new directory for a new demo.

## Configure your project and model

```bash
oxidize init ./my-project
oxidize analyze ./my-project
oxidize plan ./my-project
```

Edit the generated `oxidize.toml`. Example:

```toml
python = "python3"
max_repairs = 2

[provider]
endpoint = "http://127.0.0.1:11434/v1"
model = "your-installed-model"
api_key_env = "OXIDIZE_API_KEY"
max_context_bytes = 32768
max_output_tokens = 4096
timeout_seconds = 120
retries = 2

[verification]
cases_file = "cases.json"
test_command = ["python3", "-m", "unittest", "discover", "-v"]

[performance]
iterations = 7
warmup = 2
calls_per_sample = 3
minimum_speedup = 1.0

[execution]
timeout_seconds = 180
output_limit_bytes = 1048576
offline = true
read_only_paths = [] # Configure toolchain/interpreter/cache mounts for Bubblewrap.
```

Set the selected credential environment variable through your normal secret-management mechanism; never store a key in TOML. A local server may need no key. Change `endpoint` to a provider's actual OpenAI-compatible `/v1` base URL, and use any supported model ID. Provider-specific parameters, streaming, tool calling, fallback routing and cost enforcement are not implemented. Actual usage is recorded if the server supplies it; monetary cost is reported as unknown.

This command explicitly authorizes sending the selected module and budget-fitting direct local dependencies to that configured provider:

```bash
oxidize migrate ./my-project --component compute.py:count_primes --allow-provider
```

Local model HTTP calls originate in the engine process; Bubblewrap isolates child builds/tests, not the selected provider request. Raw server error bodies and transport details are omitted from errors to protect credentials. API keys are not passed to child builds/tests.

`cases.json` is a nonempty array such as:

```json
[{"args": [0]}, {"args": [100]}, {"args": [-1]}, {"args": [], "kwargs": {"limit": 10}}]
```

For progressive migration of functions with different inputs, use an object mapping `file.py:function` IDs to their individual case arrays. Configure all workloads before starting the session. At least one successful workload and existing test files are required. Error cases are compared but omitted from timing batches.

For agent-assisted operation without a provider call, supply `--response code.json`. The schema is `{"rust": "complete lib.rs"}`. The module must export the original function through `#[pymodule] fn MODULE(m: &Bound<'_, PyModule>) -> PyResult<()>`, with `MODULE = "_oxidize_" + first 12 hex characters of SHA256("file.py:function")`. Only the pinned PyO3 dependency is available. The response must pass `cargo fmt --check` and Clippy. The complete example is in [examples/python-native](examples/python-native/src/lib.rs).

## Workspaces, recovery, and reporting

```text
PROJECT/.oxidize/
  analysis.json, plan.json, plan.md
  state.json, session.lock   checkpoint with its event history
  state.v1.json              original of an upgraded v0.1.x checkpoint, if any
  baseline/                  immutable source snapshot
  attempts/0001/project/     compiled/integrated candidate
  report.md, report.json
```

`oxidize status PROJECT` shows state and the accepted workspace. `resume PROJECT` replays the latest interrupted/rejected attempt in a fresh directory, retaining the old partial directory and generated code. `--response` can supply a corrected implementation; `--allow-provider` permits further repairs. Every invocation must choose its execution trust policy again. `rollback PROJECT` restores the preceding accepted pointer without deleting source or evidence.

On this development branch, tests, behavior capture and timing each run in fresh temporary copies under `.oxidize/validation/`, cleaned up on return. The original source, canonical baseline and accepted candidate are not used as execution working directories. Test-generated setup files do not seed behavior capture, and observation state does not seed timing; include required workload inputs in the project beforehand. These copies are not an OS sandbox, and trusted code with absolute paths can still access the host. Source fingerprints detect edits; modified original/baseline/accepted source is not silently overwritten. Snapshots exclude known secrets, VCS state, build caches, virtual environments, and dependency directories, and reject symlinks and oversized files. Projects depending on those exclusions require deliberate preparation. Session config is frozen at baseline; preserve a session and start a new copy to change input/configuration.

Existing v0.1 checkpoint evidence remains readable. Reports label its argument-mutation evidence as unavailable. New observations carry `observation_schema_version = 2`; old observations cannot be treated as mutation-equivalent without replay. On this development branch the checkpoint itself is schema 2: it adds an event history saved atomically with each state change. Opening a v0.1.x (schema 1) session upgrades it once and keeps the original bytes as `state.v1.json`; earlier history was never recorded and is not invented. A checkpoint from a newer Oxidize is refused untouched, and v0.1.x refuses an upgraded one, so keep `state.v1.json` if you may go back.

The [mutation fixture](fixtures/python-mutation/README.md) demonstrates a compiled replacement that passes existing tests yet is rejected for missing an in-place mutation, then resumes successfully with corrected Rust code.

Reports distinguish checked cases from universal equivalence and count components without inventing functionality coverage. Timing includes the actual Python/FFI call and input reconstruction; it is a component workload measurement, not total application speed. CPU, memory, allocations, external state, bridge-only overhead, concurrency, and arbitrary Python object identity are not measured. See [verification and benchmarks](docs/VERIFICATION.md).

## Agent skill

Install into a selected target project:

```bash
python3 skills/oxidize/scripts/install.py --agent codex --project /path/to/project
```

For a Codex terminal session that discovers project skills, reload/restart and invoke `$oxidize` with the project and goal. Other supported installation layouts and invocation details are in [agent setup](docs/AGENT_SKILL.md). The installer validates the target and refuses to overwrite existing skills. It never modifies your home directory.

The skill can also operate without this CLI using the coding agent's existing tools. Its broader methodology does not imply that the standalone engine supports every language. External agent discovery and live model migrations need validation in the corresponding installed client; the package and offline CLI workflow are tested here.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

See [contributing](CONTRIBUTING.md), [architecture](docs/ARCHITECTURE.md), [roadmap](docs/ROADMAP.md), and [security](SECURITY.md). MIT licensed.

Built with the urge to oxidize everything by [MetaMysteries8](https://github.com/MetaMysteries8), with Claude, Codex, and Ferriscope (Hype Agent).

## Support Oxidize

If Oxidize was useful, tips are optional thank-yous—not investments. Sending
buys no goods, services, rewards, ownership, or promised returns.
Tiny side quest: Hype Agent and Claude are trying to receive their first cent.

Solana address (USDC or SOL): `FssdeLXuzW91oPu5b9N3KygcNGXV7yw8gmAVtmggVYYX`

Or paste this Solana Pay request into a wallet app for a ready-made 0.01 USDC tip
(GitHub doesn't make `solana:` links clickable):

```text
solana:FssdeLXuzW91oPu5b9N3KygcNGXV7yw8gmAVtmggVYYX?amount=0.01&spl-token=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&label=Hype+Agent
```
