# Security model

Repositories and generated Rust are executable, untrusted inputs. A snapshot protects original source from ordinary edits; it does **not** sandbox a process.

The default Linux runner requires Bubblewrap, unshares namespaces/network, exposes only system runtime directories and explicitly configured read-only toolchain/cache paths, makes only the current workspace writable, and provides isolated `/tmp`, `/proc`, and `/dev`. It does not silently fall back. Add required compiler/Python installation/cache directories to `execution.read_only_paths`; avoid home directories and anything containing credentials. Cargo uses offline dependencies by default. Pre-fetch the pinned dependency using `cargo fetch --locked --manifest-path examples/python-native/Cargo.toml` after reviewing that manifest. Configure `CARGO_HOME` and `RUSTUP_HOME` normally and expose the needed toolchain/cache paths read-only. Builds write to the candidate's target directory. The child uses the configured Python for PyO3.

Extra host mounts require the invoking CLI's explicit `--allow-host-paths` flag. Repository-provided configuration cannot authorize those mounts by itself. Review every listed path before using the flag.

Commands are checked before a sandboxed run: if a program (say `python3`) resolves on the host to a path the sandbox can't see, such as a Conda or pyenv Python under your home directory, the run is refused rather than letting a different `python3` on the sandbox's `PATH` build against the wrong interpreter. Give an absolute path to a mounted interpreter (`python = "/usr/bin/python3"`, and the same in `verification.test_command`), or mount its directory read-only. `/etc/alternatives` is mounted read-only because Debian/Ubuntu resolve `cc` through it; the rest of `/etc` stays hidden.

**Validated:** the complete native demo (migrate, verify, benchmark, report, rollback) has run successfully inside the sandbox, without `--trust-code`, on Ubuntu with Linux 7.0, Bubblewrap and `kernel.apparmor_restrict_unprivileged_userns=1`, using rustup's `~/.rustup` and `~/.cargo` mounted read-only. Cargo's offline build worked with a read-only `CARGO_HOME` after `cargo fetch`. See [verification](docs/VERIFICATION.md#sandboxed-native-demo) for the exact configuration.

Bubblewrap is defense in depth, not a complete hostile-code security boundary. cgroup memory/PID/disk limits are not implemented. Standard CI runs in an environment that rejects user-namespace setup, so there the tests confirm closed failure and exercise the known fixture with `--trust-code`; the successful sandboxed run above is a manual validation on a supporting host.

`--trust-code` explicitly runs commands with the invoking user's filesystem and network authority. Never use it just because a model asks. Inspect project tests, generated code and dependencies, and prefer a dedicated isolated machine for unknown projects. Default child time/output limits and process-group termination are not a substitute for OS isolation. Trusted code can deliberately escape process-group supervision.

The process environment is reduced to relevant toolchain variables; provider credentials are not inherited by child builds/tests. Commands are argument vectors, never model-supplied shell scripts. Only fixed Cargo operations, the fixed harness and the user-configured test command are run. The provider cannot add dependencies through its JSON response. Cargo dependencies still contain build scripts and procedural macros; inspect and pin dependencies deliberately.

`--allow-provider` explicitly authorizes transmission of selected source to the configured endpoint. Only the selected module and fitting direct local dependencies are sent, within a byte budget. Source text and compiler diagnostics are described as untrusted data. Prompt-injection resistance is a policy boundary, not a guarantee of safe generated code. Remote endpoints require HTTPS, URL credentials/query strings are rejected, redirects are disabled, and raw provider errors/transport details are omitted. API keys are read from a named environment variable and not persisted in session configuration. Untrusted code output may still contain sensitive project content; protect `.oxidize/` artifacts.

Known credential filenames/directories are excluded from snapshots and source discovery; this is not a secret scanner and cannot find arbitrary embedded credentials. Symlinks, files over 16 MiB, snapshots over 512 MiB and more than 100,000 source files are rejected. Python indexing is currently limited to a 100 KiB path list. Session state and internal workspaces assume the invoking user controls `.oxidize/`; do not share it writable with untrusted users/processes. Editing checkpoint JSON is not a supported API.

Report security issues privately through the repository owner's GitHub contact options. Do not include credentials, private source, or exploit-bearing artifacts in public issues.

## Sandbox backends

`execution.sandbox` chooses the isolation for builds and tests:

| Value | Uses |
|---|---|
| `auto` (default) | Bubblewrap on Linux if it can create a sandbox there, otherwise Podman, otherwise Docker. Refuses to run if none is usable. |
| `bubblewrap` | Linux only. |
| `podman` / `docker` | Any platform with the engine running. Podman is preferred under `auto`: it runs rootless, while membership of the `docker` group is equivalent to root. |

It never falls back silently: an explicit choice that isn't usable is an error.

**Containers** need `execution.image`: an image **pinned by digest** (`name@sha256:…`; tags are refused) that contains the Rust and Python toolchains and the project's dependencies, available offline. Each run gets no network, a read-only root filesystem with a private `/tmp`, every capability dropped, `no-new-privileges`, only the workspace mounted writable (owned by your user: `--user` on Docker, `--userns=keep-id` on Podman), and optional cgroup limits (`memory_mb`, `pids_limit`, `cpus`). Every container is named and removed after the run, including after a timeout, because killing the engine client doesn't stop its container.

```toml
[execution]
sandbox = "podman"
image = "ghcr.io/you/oxidize-toolchain@sha256:<digest>"
memory_mb = 4096
pids_limit = 512
cpus = 2.0
```

**Status:** the container backend is implemented and its command line is unit-tested, but **it has not been validated against a real engine yet.** No Docker or Podman was available on the validation host. Treat it as experimental until the native demo has passed in a container.

**Windows:** containers on Windows run Linux, so they can verify a port's behaviour but build a Linux module. A port that must work on Windows still needs its native build and its tests run on Windows (see [the roadmap](docs/ROADMAP.md)). Evidence from a Linux container is never a Windows claim.
