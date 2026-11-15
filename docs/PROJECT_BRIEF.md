# 🦀 OXIDIZE
## Universal AI-Powered Progressive Rustification & Performance Optimization Engine

### Your Mission

Act as a senior Rust engineer, compiler engineer, programming-language researcher, AI-agent architect, performance optimization specialist, and open-source maintainer.

Your assignment is to **design and implement Oxidize**, a universal, open-source system that progressively converts existing software projects into high-performance Rust implementations.

**This is a real software development assignment. Build functioning code, not a conceptual prototype.**

The system must support two primary forms:

1. **Standalone Rust CLI:** A native application capable of analyzing, migrating, testing, benchmarking, and optimizing existing repositories.
2. **Installable coding-agent skill:** A portable skill that allows compatible agents, including Codex, Claude Code, OpenCode, and Gemini CLI, to perform progressive Rustification using the same methodology.

Oxidize must be provider-agnostic and capable of working with cloud or locally hosted AI models.

Its ultimate objective is:

> **Convert as much of any software project into Rust as technically feasible, incrementally, while preserving the closest possible behavior to the original application and achieving the greatest measurable performance improvements.**

Oxidize should be able to handle anything from a tiny Python utility to a large, multi-language application.

Even projects already written in Rust are valid inputs.

**Everything can be oxidized. Even Rust.**

---

# 1. THE FUNDAMENTAL ARCHITECTURE

Oxidize is **not** a conventional source-code translator.

It is a progressive, performance-driven migration and optimization engine.

Rather than attempting to rewrite an entire project simultaneously, it must:

1. Deeply analyze the existing application.
2. Discover its architecture and dependencies.
3. Establish baseline behavior and performance.
4. Identify components suitable for Rustification.
5. Select one component or a closely related group of components.
6. Reimplement that functionality in Rust.
7. Integrate the Rust implementation into the existing application.
8. Test the modified application.
9. Compare it against the original.
10. Benchmark the change.
11. Repair or revert unsuccessful migrations.
12. Preserve successful changes.
13. Repeat until no more components can reasonably be Rustified.

The original application should remain operational throughout the migration whenever technically feasible.

Initially, the original program calls new Rust implementations through interoperability bridges.

As Rustification progresses, Rust should gradually become the dominant implementation language.

Eventually, Oxidize should attempt to replace the original entry point and remove remaining dependencies on the original language runtime.

This is a form of incremental application replacement inspired by the Strangler Fig pattern.

**The central rule: Never sacrifice correctness merely to increase the percentage of Rust code.**

---

# 2. SUPPORTED SOURCE LANGUAGES

Oxidize should ultimately support migration from virtually any programming language.

Target languages include:

- Python
- JavaScript
- TypeScript
- Java
- Kotlin
- C
- C++
- C#
- Go
- Swift
- Objective-C
- Ruby
- PHP
- Lua
- Dart
- Zig
- Scala
- Haskell
- OCaml
- Elixir
- Perl
- Bash
- PowerShell
- Rust itself

Support mixed-language repositories.

Do not falsely advertise full compatibility with languages whose adapters are incomplete.

Build an extensible language-adapter system so additional languages can be supported without rewriting the migration engine.

Prioritize Python, JavaScript/TypeScript, Go, and existing Rust projects during early development.

---

# 3. DEEP SOURCE-CODE ANALYSIS

Before performing any migration, Oxidize must analyze the entire source project.

This analysis should identify:

- Project structure
- Programming languages
- Source files
- Entry points
- Build systems
- Dependencies
- Internal modules
- Public interfaces
- Application architecture
- Data structures
- Call graphs
- Module dependency graphs
- Configuration files
- Runtime dependencies
- External services
- Native bindings
- Existing tests
- Generated code
- Performance-critical components
- Platform-specific behavior
- Potential migration blockers

Use AST parsing and Tree-sitter where appropriate.

Supplement static analysis with AI-assisted architectural analysis.

Create a persistent intermediate representation of the application.

Track dependencies and relationships between components.

Do not simply concatenate source files and send the entire repository to an AI model.

Implement intelligent context selection.

The analysis engine should provide relevant source code, dependent modules, interfaces, and architectural context to each migration task.

Support very large repositories without requiring their entire contents to fit inside a single model context window.

---

# 4. BASELINE PROFILING AND BEHAVIOR CAPTURE

Before migrating components, Oxidize should establish a baseline.

Where practical, collect:

- Execution time
- CPU usage
- Memory usage
- Peak memory consumption
- Allocation patterns
- Startup time
- Throughput
- Latency
- I/O characteristics
- Test outcomes
- Observable application behavior

Use language-appropriate profiling tools and benchmarking strategies.

Do not assume every repository can be automatically benchmarked.

Allow users to define representative workloads and benchmark commands.

When profiling data is unavailable, record that limitation explicitly.

Use existing tests, fixtures, example inputs, and interface definitions to establish behavioral expectations.

Store baseline results so future migrations can be compared against them.

---

# 5. INTELLIGENT RUSTIFICATION PLANNER

Implement an AI-assisted migration planner.

The planner should identify components that are good candidates for incremental Rustification.

Evaluate candidates based on:

- Expected performance gain
- Current runtime cost
- Complexity
- Dependency relationships
- Interoperability difficulty
- Test coverage
- Migration risk
- Likelihood of behavioral equivalence
- Potential reduction in runtime dependencies
- Ability to unlock future migrations

Prefer migrating computationally expensive, relatively isolated components early.

However, recognize that some components may need to be migrated together to eliminate expensive cross-language boundaries.

Use dependency graphs to determine migration order.

The planner must support revising the plan after each migration.

Generate a persistent, human-readable migration roadmap.

Example:

```text
OXIDIZE MIGRATION PLAN

Project: example-python-app

Detected languages:
- Python: 94%
- C: 6%

Components discovered: 53

Priority candidates:

1. Image processor
   Estimated complexity: Medium
   Performance priority: High
   Interoperability: PyO3

2. Data parser
   Estimated complexity: Low
   Performance priority: High
   Interoperability: PyO3

3. Storage layer
   Estimated complexity: Medium
   Performance priority: Medium

4. HTTP server
   Estimated complexity: High
   Performance priority: Low

Migration strategy:
Progressive replacement

Target:
Maximum practical Rustification
```

Predicted performance improvements must be labeled as estimates, not measured results.

---

# 6. PROGRESSIVE COMPONENT MIGRATION

Implement the core Rustification loop.

For each selected component:

### Step A: Understand

Analyze the original implementation and its dependencies.

Identify inputs, outputs, error behavior, side effects, and invariants.

### Step B: Reimplement

Generate an idiomatic Rust implementation preserving the component's behavior.

Use appropriate Rust crates and architectural patterns.

### Step C: Integrate

Create an interoperability bridge.

Modify the original project so it calls the Rust implementation.

Preserve original interfaces wherever feasible.

### Step D: Compile

Build the new Rust code.

Capture compiler diagnostics.

### Step E: Test

Run relevant tests against the integrated application.

### Step F: Compare

Perform differential testing against the original implementation where possible.

### Step G: Benchmark

Measure performance under representative workloads.

### Step H: Accept or Reject

Keep the migration only when it satisfies the configured correctness and performance acceptance policies.

Otherwise, attempt targeted repairs or revert the change.

### Step I: Continue

Update the dependency graph, migration plan, and project state.

Select the next component.

Repeat.

Each successful migration should produce a usable, checkpointed version of the application.

Never silently remove features or replace implementations with placeholders.

---

# 7. INTEROPERABILITY AND LANGUAGE BRIDGES

Incremental Rustification requires reliable interoperability.

Implement language-specific adapters.

Potential technologies include:

| Original environment | Potential integration |
|---|---|
| Python | PyO3 / maturin |
| Node.js | napi-rs |
| C | C ABI |
| C++ | CXX / FFI |
| Java | JNI |
| C# | Native interoperability |
| Go | C ABI / cgo where applicable |
| Browser applications | WebAssembly |
| Rust | Native crates and modules |

Adapters must handle:

- Ownership
- Memory management
- Thread safety
- Error propagation
- Exceptions
- Data conversion
- Async behavior
- Lifetimes
- Resource cleanup
- ABI compatibility

Avoid unnecessary data copies and serialization.

Measure interoperability overhead.

Whenever multiple components have been converted into Rust, investigate whether their intermediate bridges can be removed.

For example, instead of:

```text
Python → Rust → Python → Rust
```

Prefer consolidating the implementation into:

```text
Python → Rust → Rust
```

Eventually:

```text
Rust → Rust → Rust
```

The system should actively detect opportunities to eliminate cross-language boundaries.

---

# 8. FULL RUSTIFICATION

Oxidize must attempt to reach full Rustification where technically feasible.

Support these migration outcomes:

### Partial Rustification

Some functionality is implemented in Rust.

The original language runtime remains necessary.

### Predominantly Rust

Most application functionality has been converted.

Some compatibility bridges or original-language components remain.

### Fully Rustified

All application logic is implemented in Rust.

The original language runtime is no longer necessary.

Transitional compatibility bridges have been removed.

The application builds and operates using the new Rust implementation.

Native system libraries and external services may remain where technically necessary, but must be disclosed.

Do not claim full Rustification when critical original-language components remain.

Measure migration coverage through verified functionality, not merely lines of code or file counts.

The system should attempt to migrate application entry points once sufficient supporting components have been converted.

For example:

```text
BEFORE

main.py
  ├── database.py
  ├── parser.py
  ├── processor.py
  └── renderer.py

DURING

main.py
  ├── Rust database
  ├── Rust parser
  ├── Rust processor
  └── renderer.py

AFTER

main.rs
  ├── database.rs
  ├── parser.rs
  ├── processor.rs
  └── renderer.rs
```

Full Rustification is an objective, not a guarantee.

---

# 9. RUST-TO-RUST OXIDIZATION

Oxidize must accept existing Rust repositories.

When the input project is already written in Rust, perform performance analysis and optimization rather than merely regenerating equivalent syntax.

Identify:

- Excessive cloning
- Unnecessary allocations
- Inefficient algorithms
- Poor data structures
- Cache inefficiencies
- Lock contention
- Blocking operations
- Serialization overhead
- Unnecessary dynamic dispatch
- Inefficient asynchronous execution
- Redundant computations
- Excessive intermediate buffers
- Poor parallelization
- Inefficient I/O patterns

Profile the existing code.

Propose targeted optimizations.

Apply changes incrementally.

Run tests and benchmarks after each optimization.

Reject changes that violate the configured correctness or regression policies.

Do not introduce unsafe Rust without a demonstrated need, documented safety invariants, and suitable verification.

Even an already optimized Rust repository should be eligible for analysis.

If no worthwhile optimizations remain, report that result and terminate cleanly.

---

# 10. HYPEROXIDIZE MODE

Implement an advanced optimization mode named:

**Hyperoxidize**

Example:

```bash
oxidize hyperoxidize ./project
```

Hyperoxidize should focus on optimizing an application that has already undergone substantial Rustification.

It should:

1. Profile the resulting application.
2. Identify remaining hotspots.
3. Analyze inter-module communication.
4. Remove unnecessary FFI boundaries.
5. Consolidate compatible Rust components.
6. Reduce allocations and copying.
7. Improve algorithms where feasible.
8. Investigate concurrency bottlenecks.
9. Optimize I/O and serialization.
10. Benchmark each accepted change.

Allow Hyperoxidize to operate on existing Rust projects without requiring a previous Oxidize migration.

The optimization process must remain incremental and reversible.

Never claim that an optimization is faster without benchmark evidence.

---

# 11. UNIVERSAL AI PROVIDER SYSTEM

Oxidize must support arbitrary AI providers and model identifiers.

Implement a reusable provider abstraction.

Support:

- OpenAI-compatible endpoints
- OpenAI
- Anthropic
- Google Gemini
- OpenRouter
- Ollama
- LM Studio
- llama.cpp-compatible servers
- Pollinations
- Custom HTTP endpoints

Additional providers should be installable through adapters.

Configuration options should include:

- Provider
- API endpoint
- API key
- Model identifier
- Context size
- Maximum output tokens
- Temperature
- Request timeout
- Retry policy
- Concurrency limits
- Streaming
- Structured output
- Tool calling
- Reasoning settings where supported
- Cost limits
- Token budgets

Support local models without requiring an external service.

Do not assume every provider supports identical parameters.

Support models without native tool calling through structured text or JSON protocols.

Implement robust error handling for rate limits, timeouts, malformed responses, and context-window overflow.

Protect API credentials.

Never log secret values.

---

# 12. MULTI-MODEL MIGRATION PIPELINE

Allow different AI models to handle different tasks.

Example configuration:

```toml
[models]
analysis = "provider/model-a"
planning = "provider/model-b"
conversion = "provider/model-c"
repair = "provider/model-d"
verification = "provider/model-e"
optimization = "provider/model-f"
```

Tasks may include:

- Repository analysis
- Architecture planning
- Component migration
- FFI generation
- Compiler-error repair
- Test generation
- Behavioral verification
- Performance optimization

Support single-model configurations as well.

Allow explicit fallback models.

Track usage and costs separately for each model.

Avoid unnecessary model calls.

Cache reusable analysis results where safe.

---

# 13. COMPILER-GUIDED REPAIR

Generated Rust code must be compiled and checked.

Use appropriate commands such as:

```bash
cargo fmt --check
cargo check
cargo clippy
cargo test
```

Capture compiler diagnostics.

Feed relevant errors back into the AI model.

Apply targeted corrections.

Repeat within configurable limits.

Detect stalled repairs and repeated failures.

Avoid endlessly regenerating entire files when a localized change is sufficient.

If a component cannot be repaired, revert that migration and continue with other viable candidates.

Never delete important code simply to achieve compilation success.

---

# 14. BEHAVIORAL EQUIVALENCE

Behavior preservation is a primary requirement.

Implement differential testing wherever practical.

For the same inputs, compare the original implementation and the Rust implementation.

Potential comparison targets include:

- Return values
- CLI output
- Exit codes
- JSON responses
- Serialization formats
- File modifications
- API behavior
- Error conditions
- State transitions
- Timing-sensitive behavior
- Concurrency behavior

Support normalization rules for nondeterministic output.

Use numerical tolerances where appropriate.

Preserve original tests.

Generate new tests when helpful, but distinguish AI-generated tests from existing tests and independent behavioral evidence.

The engine must not claim functional equivalence merely because its own generated tests pass.

Record untested behavior explicitly.

---

# 15. PERFORMANCE BENCHMARKING

The project must include real benchmarking infrastructure.

Measure:

- Execution time
- Throughput
- Latency
- Startup time
- CPU consumption
- Peak memory use
- Allocation behavior where supported

Use repeatable workloads.

Account for warm-up effects, system noise, and statistical variation.

Report median performance and variability where appropriate.

Compare the migrated application against its original baseline.

Support configurable acceptance policies.

Examples:

```toml
[performance]
enabled = true
priority = "speed"
allow_regressions = false
minimum_speedup = 1.0
benchmark_iterations = 10
```

Provide more flexible policies for users who prioritize eliminating original-language dependencies over immediate performance improvements.

Do not assume a Rust port is automatically faster.

Recognize that existing Python, JavaScript, or other applications may already rely on highly optimized native implementations.

Evaluate net performance rather than programming language alone.

---

# 16. MIGRATION POLICIES

Implement configurable goals.

### Performance Priority

Maximize measured performance improvements while preserving required behavior.

### Rustification Priority

Maximize the amount of functionality implemented in Rust, allowing small performance tradeoffs within configurable limits.

### Balanced

Consider performance, Rust coverage, maintainability, and migration complexity.

### Strict Compatibility

Prioritize preserving original behavior and public interfaces.

### Full Rust

Attempt to eliminate all original-language runtime dependencies.

Do not guarantee this outcome.

### Hyperoxidize

Optimize an existing Rust implementation.

All policies must preserve minimum correctness requirements.

---

# 17. CHECKPOINTS AND ROLLBACK

Large migrations may require many hours of execution and numerous AI requests.

Implement durable, resumable checkpoints.

Track:

- Source repository revision
- Migration plan
- Completed components
- Failed components
- Pending components
- Generated Rust files
- Integration bridges
- Build results
- Test results
- Benchmark results
- Provider usage
- Estimated costs
- Migration history

Support:

```bash
oxidize resume
oxidize status
oxidize rollback
```

Use transactional changes or isolated working directories.

Never modify the original repository destructively without explicit authorization.

Provide reliable recovery after interruptions.

---

# 18. GITHUB INTEGRATION

Oxidize must accept both local directories and Git repository URLs.

Examples:

```bash
oxidize migrate ./my-project
```

```bash
oxidize migrate https://github.com/example/project
```

Implement:

- Repository cloning
- Branch selection
- Repository analysis
- Migration workspaces
- Local commits
- Migration reports
- Optional pull request creation

Support private repositories through existing authenticated Git tooling where authorized.

Do not require GitHub for local projects.

Never push changes without explicit authorization.

Preserve applicable license notices and attribution.

---

# 19. COMMAND-LINE INTERFACE

Implement a native Rust CLI.

Suggested libraries:

- Clap
- Tokio
- Serde
- Reqwest
- Indicatif
- Ratatui
- Crossterm

Example commands:

```bash
oxidize init

oxidize analyze ./project

oxidize plan ./project

oxidize migrate ./project

oxidize migrate ./project --mode performance

oxidize migrate ./project --mode full-rust

oxidize migrate ./project --provider openrouter --model MODEL_ID

oxidize hyperoxidize ./project

oxidize benchmark ./project

oxidize verify ./project

oxidize status

oxidize resume

oxidize rollback

oxidize report

oxidize providers
```

Make the commands functional.

Provide useful errors, interactive configuration, and noninteractive automation modes.

---

# 20. INTERACTIVE TERMINAL DASHBOARD

Implement a terminal dashboard displaying migration progress.

Example:

```text
╭────────────────────────────────────────────────╮
│ OXIDIZE                                        │
│ Progressive Rustification Engine               │
├────────────────────────────────────────────────┤
│ Project: example-project                       │
│                                                │
│ Rustification Coverage                         │
│ ████████░░░░░░░░░░░░ 42%                        │
│                                                │
│ Original Runtime:       850 ms                 │
│ Current Runtime:        390 ms                 │
│ Measured Speedup:       2.18x                  │
│                                                │
│ Components Migrated:    21 / 50                │
│ Tests Passing:          147 / 150              │
│                                                │
│ Active Provider:        OpenRouter             │
│ Active Model:           configured-model       │
│                                                │
│ Current Operation:                             │
│ Rustifying parser module                       │
│                                                │
│ Status: COMPILING                              │
│                                                │
│ [P] Pause [R] Resume [L] Logs [Q] Quit         │
╰────────────────────────────────────────────────╯
```

Display actual metrics.

Clearly distinguish measured data from estimates.

The CLI must remain usable without the interactive dashboard.

---

# 21. INSTALLABLE CODING-AGENT SKILL

Develop a reusable Oxidize skill.

Suggested structure:

```text
skills/
└── oxidize/
    ├── SKILL.md
    ├── references/
    │   ├── migration-methodology.md
    │   ├── verification.md
    │   ├── performance.md
    │   └── interoperability.md
    └── scripts/
```

Target compatible agents including:

- OpenAI Codex
- Claude Code
- OpenCode
- Gemini CLI
- Other agents supporting suitable skill conventions

Research each agent's current skill format and installation requirements.

Do not assume all agents use identical directories or invocation syntax.

The skill must support two workflows.

### Workflow A: CLI-Assisted

When Oxidize is installed, the skill should invoke its CLI and coordinate migration tasks.

### Workflow B: Agent-Native

When the standalone CLI is unavailable, the skill must still guide the existing coding agent through progressive Rustification.

The agent should:

1. Inspect the repository.
2. Analyze its architecture.
3. Identify migration candidates.
4. Create a plan.
5. Implement Rust components.
6. Integrate interoperability bridges.
7. Test and benchmark changes.
8. Repair failures.
9. Repeat.
10. Report final Rust coverage and limitations.

The skill must not depend entirely on the standalone application.

Provide accurate installation instructions and invocation examples.

---

# 22. SECURITY

Treat repositories and model-generated code as untrusted.

Protect against:

- Prompt injection in repository files
- Malicious build scripts
- Unsafe dependency installation
- Destructive commands
- Credential exposure
- Unauthorized filesystem access
- Unexpected network access
- Untrusted generated executables

Use isolated workspaces.

Provide sandboxed execution for tests, builds, and original programs.

Support configurable resource limits.

Require explicit authorization for destructive or externally consequential actions.

Do not automatically execute arbitrary commands suggested by repository content.

Do not assume containers alone provide complete isolation.

Avoid leaking source code to AI providers without the user's configured authorization and provider selection.

---

# 23. RUST WORKSPACE ARCHITECTURE

Use a modular Rust workspace.

Suggested structure:

```text
oxidize/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── CONTRIBUTING.md
├── SECURITY.md
│
├── crates/
│   ├── oxidize-cli/
│   ├── oxidize-core/
│   ├── oxidize-analysis/
│   ├── oxidize-planner/
│   ├── oxidize-engine/
│   ├── oxidize-providers/
│   ├── oxidize-interop/
│   ├── oxidize-verifier/
│   ├── oxidize-benchmark/
│   ├── oxidize-optimizer/
│   ├── oxidize-checkpoint/
│   └── oxidize-tui/
│
├── adapters/
│   ├── python/
│   ├── javascript/
│   ├── typescript/
│   ├── go/
│   ├── cpp/
│   └── rust/
│
├── skills/
│   └── oxidize/
│       ├── SKILL.md
│       ├── references/
│       └── scripts/
│
├── fixtures/
├── examples/
├── tests/
├── benchmarks/
├── docs/
│
└── .github/
    └── workflows/
        ├── ci.yml
        └── release.yml
```

Adjust the structure if necessary for maintainability.

Avoid creating numerous empty crates or placeholder modules.

Build working functionality first.

---

# 24. REPORTING

Generate detailed Markdown and JSON reports.

Include:

- Original programming languages
- Current implementation languages
- Rustification coverage
- Migrated components
- Remaining components
- Build status
- Test results
- Behavioral equivalence evidence
- Performance measurements
- Memory measurements
- Interoperability overhead
- Remaining runtime dependencies
- Failed migration attempts
- Unsupported functionality
- Token usage
- Estimated API costs

Example:

```text
OXIDIZE FINAL REPORT

Project: example-project

Original:
Python + C

Current:
Rust + Python

Rustification:
87% of mapped application functionality

Behavior:
312 / 315 tests passing

Performance:
Original median: 1040 ms
Current median:  290 ms

Speedup:
3.59x

Memory:
Original peak: 220 MB
Current peak:  135 MB

Full Rustification:
NOT ACHIEVED

Remaining components:
- GUI integration
- Proprietary SDK wrapper

Status:
PARTIALLY OXIDIZED
```

Rustification percentages must have a documented calculation method.

Do not fabricate measurements or claim unverified behavioral equivalence.

---

# 25. TERMINATION CONDITIONS

Oxidize must not run endlessly.

Stop when:

- Full Rustification has been achieved.
- No additional eligible components remain.
- Remaining components have unresolved compatibility blockers.
- Repeated repairs cannot resolve failures.
- Performance acceptance criteria cannot be satisfied.
- Configured token or cost budgets are exhausted.
- Resource limits are reached.
- The user cancels execution.

Produce a useful final report explaining why migration stopped.

Allow unresolved components to be revisited later.

---

# 26. TESTING REQUIREMENTS

Implement real automated testing.

Include:

- Configuration tests
- Provider adapter tests
- Source analysis tests
- Dependency graph tests
- Migration planning tests
- FFI integration tests
- Compiler repair tests
- Checkpoint recovery tests
- Differential verification tests
- Benchmark infrastructure tests
- CLI integration tests
- Agent skill validation

Create representative migration fixtures.

At minimum:

1. Python command-line application.
2. JavaScript utility.
3. Go service.
4. Existing Rust application requiring optimization.

Include an actual end-to-end migration demonstration.

Mock provider responses for deterministic CI.

Live-model tests must be optional.

Do not require paid API access for standard repository tests.

---

# 27. DEVELOPMENT PHASES

Implement the project in practical stages.

## Phase 1 — Working Foundation

Build:

- Rust workspace
- CLI
- Configuration system
- OpenAI-compatible provider
- Source analysis engine
- Dependency graph
- Migration planner
- Python adapter
- Incremental component replacement
- PyO3 interoperability
- Compiler checks
- Basic verification
- Checkpoints

**Deliver an actual working Python-to-Rust component migration.**

## Phase 2 — Performance and Reliability

Implement:

- Profiling integration
- Benchmarking
- Differential testing
- Automated compiler repairs
- Transactional migrations
- Rollback
- Detailed reporting

## Phase 3 — More Languages

Implement:

- JavaScript/TypeScript
- Go
- Existing Rust optimization
- Additional interoperability adapters

## Phase 4 — Provider Expansion

Implement:

- Anthropic
- Gemini
- Ollama
- OpenRouter
- Pollinations
- Custom endpoints
- Multi-model routing

## Phase 5 — Agent Skill

Deliver:

- Installable skill
- CLI-assisted mode
- Agent-native mode
- Agent-specific installation documentation
- Examples

The skill should be developed early enough to remain aligned with the migration engine rather than being added as an unrelated afterthought.

## Phase 6 — Advanced Oxidization

Implement:

- Full Rust entry-point replacement
- Dependency elimination
- Cross-language boundary consolidation
- Hyperoxidize
- Large-repository support
- Advanced performance optimization
- Terminal dashboard
- GitHub automation

Prioritize complete working features over broad but nonfunctional scaffolding.

---

# 28. ACCEPTANCE CRITERIA

The initial usable release must allow a developer to:

1. Clone the Oxidize repository.
2. Build the project with Cargo.
3. Configure an AI provider.
4. Analyze a supported source project.
5. Generate a migration plan.
6. Select a component for Rustification.
7. Produce an actual Rust implementation.
8. Integrate that implementation into the original application.
9. Compile the new Rust code.
10. Run meaningful tests.
11. Compare original and migrated behavior.
12. Benchmark representative workloads.
13. Accept or revert the migration.
14. Resume an interrupted migration.
15. View a truthful migration report.
16. Install the coding-agent skill.
17. Use the skill with at least one supported coding agent.

At least one sample project must demonstrate a working incremental migration.

Do not claim support for unimplemented languages or incomplete features.

---

# 29. IMPLEMENTATION DIRECTIVES

You are authorized to write and modify files inside the assigned development repository.

Begin by inspecting the repository and its existing contents.

If there is already relevant work, preserve and extend it.

Research current APIs, libraries, and coding-agent skill specifications as necessary.

Implement functional code.

Run available checks and tests.

Fix errors encountered during development.

Do not stop at generating a design document.

Do not substitute empty interfaces, fake benchmark results, or placeholder integrations for working implementations.

Use stable Rust and idiomatic patterns.

Prefer safe Rust.

Use explicit error handling.

Implement robust cancellation and recovery.

Document setup and usage.

Respect any repository-specific instructions unless they conflict with security requirements.

Never commit credentials.

Do not push changes or publish releases without authorization.

If execution constraints prevent completing the entire vision, deliver the largest coherent, tested vertical slice possible.

The first priority is a working progressive migration engine, not a flashy interface.

---

# 30. FINAL DELIVERABLES

Produce:

- A functional Rust workspace.
- A usable CLI.
- A real source analysis engine.
- An AI provider abstraction.
- A working incremental migration pipeline.
- At least one functional interoperability adapter.
- Compilation and testing integration.
- Performance measurement support.
- Durable checkpoints.
- Migration reporting.
- An installable coding-agent skill.
- Documentation.
- Example projects.
- Automated tests.
- A development roadmap.

At the end of implementation, summarize:

1. What was implemented.
2. Which languages and providers actually work.
3. Which features remain incomplete.
4. How to build and run Oxidize.
5. How to install the agent skill.
6. What tests were executed and their results.
7. What the next development priorities should be.

Be precise and transparent.

---

# 31. PROJECT PHILOSOPHY

Oxidize exists to answer one question:

**How much of this application can we replace with Rust while preserving its behavior and making it measurably faster?**

The engine should progressively migrate the existing application, not simply produce a disconnected rewrite.

It should understand the original code deeply enough to preserve functionality.

It should test every meaningful change.

It should measure performance instead of assuming improvements.

It should roll back unsuccessful conversions.

It should continue migrating until additional Rustification is no longer practical.

And when the application is already written in Rust, it should investigate whether the existing Rust implementation can be improved.

**Maximum Rustification. Minimum behavioral deviation. Maximum measurable performance.**

### THE ULTIMATE DIRECTIVE

**ANALYZE IT.**

**RUSTIFY IT.**

**INTEGRATE IT.**

**TEST IT.**

**BENCHMARK IT.**

**OPTIMIZE IT.**

**REPEAT.**

**IF IT'S ALREADY RUST, OXIDIZE IT AGAIN.**

Now build Oxidize as a real, open-source Rust project.