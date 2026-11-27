# Oxidize and rust-anything

[rust-anything](https://github.com/erkamyaman/rust-anything) by erKam Yaman and Oxidize had the
same idea at almost the same moment: port an application to Rust one piece at a time, and prove
at every step that it still behaves the same. They were built separately (Oxidize's repository
was created on 2026-11-15, rust-anything's on 2026-11-16), and they're good at different things,
so they work well together instead of competing.

> Trust your agent? Use rust-anything. Want proof? Add Oxidize. Want both? Meld.

- **rust-anything is the "I trust my agent" layer:** the agent ports quickly across eleven
  languages, and rust-anything's own checks decide when a port is done.
- **Oxidize is the "I want safety" layer:** nothing runs outside a sandbox without your
  permission, every step can be rolled back, and a speedup is only reported when the timing
  shows it.
- **Meld** (planned) moves you between them: let the agent run freely, then pass a port
  through Oxidize before relying on it.

| | rust-anything | Oxidize |
| --- | --- | --- |
| Languages | TypeScript, JavaScript, Python, Go, Ruby, Java, C, PHP, C#, Kotlin, PowerShell | Python/PyO3 migration; `crosscheck` works with any language |
| Strength | Breadth: profiling, bridging, edge-case recording, fuzzing, shadow mode, gate, enable/disable switch | Evidence: an OS sandbox that refuses to run without isolation, disposable copies, interleaved timing with a noise policy, file-effect checks, checkpoints and rollback |
| Switches a port | `RUST_ANYTHING_MODE=old\|shadow\|new`, `rust-anything enable`/`disable` | Separate accepted workspaces with pointer-based rollback |

## `oxidize crosscheck`: a second opinion on a rust-anything port

rust-anything leaves the original code in place and picks between it and the Rust port at run
time. `oxidize crosscheck` uses that switch to check the result from the outside, without
needing to understand the port's language:

1. It copies the project into `.oxidize/crosscheck/work/project`, a fresh copy for every run at
   the same path. The copy includes each rust-anything crate's built native modules
   (`<crate>/target/release/*.so|dylib|dll|node|pyd|wasm`), which snapshots normally skip.
2. It runs each of your application commands once with `RUST_ANYTHING_MODE=old` and once with
   `RUST_ANYTHING_MODE=new`, inside the sandbox (Bubblewrap, Podman or Docker) unless you pass
   `--trust-code`.
3. It compares the exit code, the exact bytes of stdout and stderr (so invalid UTF-8 can't hide
   a difference), and any files you list (presence, length and SHA-256, streamed so a huge file
   can't exhaust memory). It names every difference with where it starts.
4. Only commands that behave the same, and exit 0, are timed. They run in interleaved rounds
   (alternating which side goes first) under `[performance]`'s noise policy, with wall time,
   and CPU time and peak RSS where the platform reports them. Every invocation, warm-ups
   included, starts from its own fresh copy, the same starting state as the behaviour runs. A
   round costs `warmup + iterations` copies per side, so large `include` paths make timing
   slower. Too-noisy timings are reported as "no claim".
5. It writes `.oxidize/crosscheck.json` and `.oxidize/crosscheck.md`, and exits 2 when the two
   runs differ, so CI or a coding agent can gate on it. The project itself is never modified.

```bash
# After `rust-anything enable rust/slugify-rs` (or with its shadow module imported):
oxidize crosscheck path/to/app --file out/report.json -- python3 app.py build
oxidize crosscheck path/to/app --include node_modules -- node cli.js --check
```

Or configure it in `oxidize.toml` (every field is optional; these are the defaults, plus a
command):

```toml
[crosscheck]
switch = "RUST_ANYTHING_MODE"   # any environment variable that switches a port works
original = "old"
candidate = "new"
file_paths = []                 # files the commands write, compared by SHA-256
include = []                    # paths snapshots skip that the commands need, e.g. "node_modules", ".venv"
compare_stderr = true
normalize_line_endings = false
timing = true

[[crosscheck.commands]]
name = "build"
command = ["python3", "app.py", "build"]
```

`--file`, `--include`, `--switch`, `--original`, `--candidate`, `--ignore-stderr`,
`--normalize-line-endings` and `--no-timing` override the configuration from the command line,
and a command after `--` replaces `[[crosscheck.commands]]`.

### A suggested loop

```text
rust-anything: profile -> bridge -> record -> port -> diff -> gate -> enable
Oxidize:                                                          crosscheck (sandboxed second opinion, whole-app timing)
```

`rust-anything gate` checks the port against recorded cases. `oxidize crosscheck` then checks the
whole application with the port on, in a sandbox, including the files it writes, and gives a
speed claim only for what agrees. Both use the original program as the source of truth.

### Limits

- A crosscheck sees only what the listed commands do with their inputs. Matching runs are
  evidence, not proof of equivalence.
- Oxidize can't tell whether the switch reached every port. rust-anything's `new` mode fails
  instead of falling back when a native module doesn't load, so missing modules show up as a
  difference rather than a silent pass.
- Network traffic, unlisted files and file metadata are not compared, and neither is behaviour
  that depends on timing.
- Snapshots skip `node_modules`, virtual environments and build folders. List what the commands
  need in `include`. Symlinks inside included paths are not copied and are listed in the report,
  so pnpm-style symlinked `node_modules` need a real install inside the project. Credential-like
  or reserved names inside them (`.env*`, `*.key`, `*.pem`, `.ssh`, `.aws`, `.git`, ...) are never
  copied either, the same rule snapshots use, and the report lists what was skipped.
- Inside Bubblewrap the commands see only the copy and the system toolchain paths. Runtimes
  installed elsewhere (for example under your home folder) need `execution.read_only_paths` and
  `--allow-host-paths`.
- Tested with small Python programs that read the switch the way rust-anything's shadow modules
  do (`crates/oxidize-engine/tests/crosscheck.rs`). It has not yet been run against a real
  rust-anything port; that is the next step.

## Next: Meld (planned, not started)

**Meld** is a planned sublayer that ships with Oxidize and moves you between the two layers:
rust-anything plans, bridges and records; `oxidize crosscheck` runs the app sandboxed with the
port off and on; a unit counts as done only when both `rust-anything gate` and the crosscheck
pass; Oxidize then checkpoints it so it can be rolled back; and you get one combined report. See
[ROADMAP.md](ROADMAP.md#meld-oxidize--rust-anything-orchestration).
