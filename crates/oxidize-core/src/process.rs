use crate::ExecutionConfig;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

static CANCELLED: AtomicBool = AtomicBool::new(false);
static HOST_MOUNTS_ALLOWED: AtomicBool = AtomicBool::new(false);

/// System paths every sandboxed process sees read-only (when they exist on the host).
///
/// `/etc/alternatives` is required on Debian/Ubuntu: `/usr/bin/cc` is a symlink to
/// `/etc/alternatives/cc`, which points back into `/usr/bin`, so without it the C linker
/// vanishes inside the sandbox and every Rust build script fails with "linker `cc` not
/// found". It holds only symlinks to installed tools; the rest of `/etc` stays hidden.
pub const SYSTEM_READ_ONLY_PATHS: &[&str] = &[
    "/usr",
    "/bin",
    "/lib",
    "/lib64",
    "/etc/ld.so.cache",
    "/etc/alternatives",
];
/// Explicit caller authorization; repository-controlled configuration cannot grant itself host access.
pub fn authorize_host_mounts() {
    HOST_MOUNTS_ALLOWED.store(true, Ordering::Relaxed);
}
pub fn cancel() {
    CANCELLED.store(true, Ordering::Relaxed);
}
pub fn cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub elapsed_ms: f64,
}
impl Output {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
    pub fn require_success(&self, operation: &str) -> Result<()> {
        ensure!(
            self.success(),
            "{operation} failed (exit {:?})\n{}\n{}",
            self.code,
            self.stdout,
            self.stderr
        );
        Ok(())
    }
}
pub struct Runner<'a> {
    pub config: &'a ExecutionConfig,
    pub trust_code: bool,
}
impl Runner<'_> {
    pub fn run(&self, cwd: &Path, argv: &[String]) -> Result<Output> {
        self.run_with_env(cwd, argv, &[])
    }
    /// Additional values come only from engine configuration, never generated commands.
    pub fn run_with_env(
        &self,
        cwd: &Path,
        argv: &[String],
        environment: &[(&str, &str)],
    ) -> Result<Output> {
        ensure!(!argv.is_empty(), "empty command");
        // Set when the command runs in a container: (engine, container name), for cleanup.
        let mut container: Option<(&'static str, String)> = None;
        let mut cmd = if self.trust_code {
            let mut cmd = Command::new(&argv[0]);
            cmd.args(&argv[1..]);
            cmd
        } else {
            ensure!(
                self.config.read_only_paths.is_empty()
                    || HOST_MOUNTS_ALLOWED.load(Ordering::Relaxed),
                "extra host mounts require --allow-host-paths after reviewing execution.read_only_paths"
            );
            let backend = resolve_backend(
                self.config.sandbox,
                cfg!(target_os = "linux"),
                bwrap_usable,
                engine_usable,
            )
            .map_err(|e| {
                anyhow::anyhow!(
                    "{e} (--trust-code explicitly permits execution without OS isolation)"
                )
            })?;
            if let Backend::Container(engine) = backend {
                let image = self.config.image.as_deref().context(
                    "container sandbox needs execution.image: an image pinned by digest with the Rust/Python toolchains",
                )?;
                static RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let name = format!(
                    "oxidize-{}-{}",
                    std::process::id(),
                    RUN.fetch_add(1, Ordering::Relaxed)
                );
                let spec = ContainerSpec {
                    engine,
                    image,
                    name: &name,
                    cwd,
                    user: workspace_owner(cwd),
                };
                let mut cmd = Command::new(engine);
                cmd.args(container_args(&spec, self.config, argv, environment));
                container = Some((engine, name));
                cmd
            } else {
                let mut visible: Vec<&Path> =
                    SYSTEM_READ_ONLY_PATHS.iter().map(Path::new).collect();
                visible.extend(self.config.read_only_paths.iter().map(PathBuf::as_path));
                visible.push(cwd);
                check_sandbox_program(&argv[0], std::env::var_os("PATH").as_deref(), &visible)?;
                let mut cmd = Command::new("bwrap");
                cmd.args(["--die-with-parent", "--unshare-all", "--new-session"]);
                for path in SYSTEM_READ_ONLY_PATHS {
                    if Path::new(path).exists() {
                        cmd.args(["--ro-bind", path, path]);
                    }
                }
                for path in &self.config.read_only_paths {
                    cmd.arg("--ro-bind").arg(path).arg(path);
                }
                cmd.arg("--bind")
                    .arg(cwd)
                    .arg(cwd)
                    .args([
                        "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--chdir",
                    ])
                    .arg(cwd)
                    .arg("--")
                    .args(argv);
                cmd
            }
        };
        // Do not pass provider credentials or unrelated host secrets into builds/tests.
        // A container's own environment is set with -e above; the engine client only needs
        // what lets it reach its daemon/socket.
        cmd.env_clear();
        let passthrough: &[&str] = if container.is_some() {
            &[
                "PATH",
                "HOME",
                "XDG_RUNTIME_DIR",
                "DOCKER_HOST",
                "DOCKER_CONTEXT",
                "CONTAINER_HOST",
                "SYSTEMROOT",
            ]
        } else {
            &[
                "PATH",
                "CARGO_HOME",
                "RUSTUP_HOME",
                "PYO3_PYTHON",
                "CC",
                "AR",
                "SYSTEMROOT",
                "TMPDIR",
            ]
        };
        for &key in passthrough {
            if let Some(v) = std::env::var_os(key) {
                cmd.env(key, v);
            }
        }
        cmd.env("PYTHONDONTWRITEBYTECODE", "1")
            .env("PYTHONHASHSEED", "0")
            .env("CARGO_TARGET_DIR", cwd.join("target"));
        for (key, value) in environment {
            cmd.env(key, value);
        }
        cmd.current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let start = Instant::now();
        let mut child = cmd.spawn().with_context(|| {
            format!(
                "could not start {}; install required tools or configure the sandbox",
                argv[0]
            )
        })?;
        let overflow = Arc::new(AtomicBool::new(false));
        fn reader(
            mut stream: impl Read + Send + 'static,
            limit: usize,
            overflow: Arc<AtomicBool>,
        ) -> thread::JoinHandle<Vec<u8>> {
            thread::spawn(move || {
                let mut bytes = vec![];
                let mut chunk = [0; 8192];
                while let Ok(n) = stream.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    let keep = n.min(limit.saturating_sub(bytes.len()));
                    bytes.extend_from_slice(&chunk[..keep]);
                    if keep < n {
                        overflow.store(true, Ordering::Relaxed);
                    }
                }
                bytes
            })
        }
        let stdout = reader(
            child.stdout.take().context("stdout pipe")?,
            self.config.output_limit_bytes,
            overflow.clone(),
        );
        let stderr = reader(
            child.stderr.take().context("stderr pipe")?,
            self.config.output_limit_bytes,
            overflow.clone(),
        );
        let mut limited = false;
        let status = loop {
            if cancelled()
                || overflow.load(Ordering::Relaxed)
                || start.elapsed() > Duration::from_secs(self.config.timeout_seconds)
            {
                limited = true;
                #[cfg(unix)]
                {
                    let _ = Command::new("kill")
                        .args(["-KILL", "--", &format!("-{}", child.id())])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
                let _ = child.kill();
                break child.wait()?;
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(10));
        };
        // Killing the engine client doesn't stop its container, so remove it by name. A no-op
        // after a normal exit (--rm already removed it); after a timeout it stops the work.
        if let Some((engine, name)) = &container {
            let _ = Command::new(engine)
                .args(["rm", "-f", name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        // Kill descendants that inherited pipes, even if the parent exited successfully.
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let out = stdout
            .join()
            .map_err(|_| anyhow::anyhow!("stdout reader failed"))?;
        let err = stderr
            .join()
            .map_err(|_| anyhow::anyhow!("stderr reader failed"))?;
        if limited || overflow.load(Ordering::Relaxed) {
            bail!("command cancelled or exceeded timeout/output limit; process group terminated");
        }
        Ok(Output {
            code: status.code(),
            stdout: String::from_utf8_lossy(&out).into_owned(),
            stderr: String::from_utf8_lossy(&err).into_owned(),
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }
}

/// The isolation a sandboxed command actually runs under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Bubblewrap,
    /// A container engine CLI: `docker` or `podman`.
    Container(&'static str),
}

/// Picks the backend for `kind`. Pure, so every platform's choice is testable anywhere:
/// `linux` says whether we're on Linux, `bwrap_ok` whether Bubblewrap can create a sandbox
/// here, `engine_ok` whether a container engine answers. Never falls back silently: an
/// explicit choice that isn't usable is an error, and so is `auto` with nothing usable.
pub fn resolve_backend(
    kind: crate::SandboxKind,
    linux: bool,
    bwrap_ok: impl Fn() -> bool,
    engine_ok: impl Fn(&str) -> bool,
) -> Result<Backend> {
    use crate::SandboxKind::*;
    let engine = |name: &'static str| {
        ensure!(
            engine_ok(name),
            "execution.sandbox = \"{name}\", but `{name}` isn't installed or its engine isn't running"
        );
        Ok(Backend::Container(name))
    };
    match kind {
        Bubblewrap => {
            ensure!(
                linux,
                "Bubblewrap needs Linux; use sandbox = \"docker\" or \"podman\" on this platform"
            );
            ensure!(
                bwrap_ok(),
                "Bubblewrap can't create a sandbox on this host (is it installed, with user namespaces allowed?)"
            );
            Ok(Backend::Bubblewrap)
        }
        Docker => engine("docker"),
        Podman => engine("podman"),
        Auto => {
            if linux && bwrap_ok() {
                return Ok(Backend::Bubblewrap);
            }
            // Podman first: it runs rootless with no root daemon. Docker group membership is root-equivalent.
            for name in ["podman", "docker"] {
                if engine_ok(name) {
                    return Ok(Backend::Container(name));
                }
            }
            bail!(
                "no usable sandbox: install Bubblewrap (Linux), Podman or Docker, or pass --trust-code to run without isolation"
            )
        }
    }
}

fn probe_ok(argv: &[&str]) -> bool {
    Command::new(argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
fn bwrap_usable() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| probe_ok(&["bwrap", "--unshare-all", "--ro-bind", "/", "/", "true"]))
}
fn engine_usable(name: &str) -> bool {
    static DOCKER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static PODMAN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let cell = if name == "docker" { &DOCKER } else { &PODMAN };
    *cell.get_or_init(|| probe_ok(&[name, "version"]))
}

/// Where the workspace appears inside a container: the same path on Unix-like hosts, so
/// paths in commands and reports keep meaning the same thing; `/workspace` otherwise
/// (a Windows `C:\…` path has no meaning inside a Linux container).
pub fn container_dir(cwd: &Path) -> String {
    let host = cwd.to_string_lossy();
    if host.starts_with('/') {
        host.into_owned()
    } else {
        "/workspace".into()
    }
}

/// What a container run needs beyond the command itself.
pub struct ContainerSpec<'a> {
    pub engine: &'static str,
    pub image: &'a str,
    pub name: &'a str,
    pub cwd: &'a Path,
    /// `uid:gid` that owns the workspace, so files written in it aren't root-owned (Docker).
    pub user: Option<String>,
}

/// The full engine command line for running `argv` in a locked-down container. Pure.
///
/// No network, a read-only root filesystem with a private `/tmp`, every capability dropped,
/// no privilege escalation, only the workspace writable, optional cgroup limits, and the
/// workspace path rewritten inside `argv` and `environment` when it had to move.
pub fn container_args(
    spec: &ContainerSpec,
    config: &ExecutionConfig,
    argv: &[String],
    environment: &[(&str, &str)],
) -> Vec<String> {
    let host = spec.cwd.to_string_lossy().into_owned();
    let inside = container_dir(spec.cwd);
    let map = |value: &str| match value.strip_prefix(host.as_str()) {
        Some(rest) if host != inside => format!("{inside}{}", rest.replace('\\', "/")),
        _ => value.to_string(),
    };
    let mut args: Vec<String> = [
        "run",
        "--rm",
        "--init",
        "--name",
        spec.name,
        "--network",
        "none",
        "--read-only",
        "--tmpfs",
        "/tmp:rw,exec",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    match spec.engine {
        "podman" => args.extend(["--userns".into(), "keep-id".into()]),
        _ => {
            if let Some(user) = &spec.user {
                args.extend(["--user".into(), user.clone()]);
            }
        }
    }
    args.extend(["-v".into(), format!("{host}:{inside}:rw")]);
    for path in &config.read_only_paths {
        let p = path.to_string_lossy();
        args.extend(["-v".into(), format!("{p}:{p}:ro")]);
    }
    args.extend(["-w".into(), inside.clone()]);
    let mut env: Vec<(String, String)> = vec![
        ("HOME".into(), "/tmp".into()),
        ("PYTHONDONTWRITEBYTECODE".into(), "1".into()),
        ("PYTHONHASHSEED".into(), "0".into()),
        ("CARGO_TARGET_DIR".into(), format!("{inside}/target")),
    ];
    if config.offline {
        env.push(("CARGO_NET_OFFLINE".into(), "true".into()));
    }
    env.extend(environment.iter().map(|(k, v)| (k.to_string(), map(v))));
    for (key, value) in env {
        args.extend(["-e".into(), format!("{key}={value}")]);
    }
    if let Some(mb) = config.memory_mb {
        args.extend(["--memory".into(), format!("{mb}m")]);
    }
    if let Some(pids) = config.pids_limit {
        args.extend(["--pids-limit".into(), pids.to_string()]);
    }
    if let Some(cpus) = config.cpus {
        args.extend(["--cpus".into(), cpus.to_string()]);
    }
    args.push(spec.image.to_string());
    args.extend(argv.iter().map(|a| map(a)));
    args
}

/// The workspace owner's `uid:gid`, read from the directory itself (no libc needed).
fn workspace_owner(cwd: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(cwd).ok()?;
        Some(format!("{}:{}", meta.uid(), meta.gid()))
    }
    #[cfg(not(unix))]
    {
        let _ = cwd;
        None
    }
}

/// Refuses to start a sandboxed command whose host executable the sandbox can't see.
///
/// A bare name like `python3` is resolved through the host's `PATH`, the same `PATH` the
/// sandbox inherits. If that host binary lives outside every mounted path (say, a Conda
/// Python under the home directory), the sandbox would silently fall through to a different
/// one on its `PATH` (a system Python of another version): the build would target the wrong
/// interpreter and its result would not import on the host. Fail closed and say how to fix it.
/// A name that isn't on the host `PATH` at all is left for the sandbox to report.
pub fn check_sandbox_program(
    program: &str,
    path_env: Option<&std::ffi::OsStr>,
    visible: &[&Path],
) -> Result<()> {
    let host = if program.contains('/') {
        Some(PathBuf::from(program))
    } else {
        path_env.and_then(|paths| {
            std::env::split_paths(paths)
                .map(|dir| dir.join(program))
                .find(|candidate| candidate.is_file())
        })
    };
    let Some(host) = host else { return Ok(()) };
    if visible.iter().any(|root| host.starts_with(root)) {
        return Ok(());
    }
    bail!(
        "`{program}` resolves to {} on this host, which the sandbox can't see, so a different `{program}` would run inside it. Set an absolute path to an interpreter/tool under a mounted path (e.g. python = \"/usr/bin/python3\"), or add its directory to execution.read_only_paths (then pass --allow-host-paths).",
        host.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_bubblewrap_on_linux_then_podman_then_docker_and_never_falls_back_silently() {
        use crate::SandboxKind::*;
        let yes = || true;
        let no = || false;
        let only = |e: &'static str| move |name: &str| name == e;
        assert_eq!(
            resolve_backend(Auto, true, yes, |_| true).unwrap(),
            Backend::Bubblewrap
        );
        assert_eq!(
            resolve_backend(Auto, true, no, only("docker")).unwrap(),
            Backend::Container("docker")
        );
        assert_eq!(
            resolve_backend(Auto, true, no, |_| true).unwrap(),
            Backend::Container("podman")
        );
        // Windows/macOS: never Bubblewrap, even if a probe said yes.
        assert_eq!(
            resolve_backend(Auto, false, yes, only("docker")).unwrap(),
            Backend::Container("docker")
        );
        assert!(
            resolve_backend(Auto, true, no, |_| false)
                .unwrap_err()
                .to_string()
                .contains("--trust-code")
        );
        // An explicit choice that isn't usable is an error, not a quiet switch to another.
        assert!(resolve_backend(Docker, true, yes, only("podman")).is_err());
        assert!(resolve_backend(Bubblewrap, false, yes, |_| true).is_err());
        assert!(resolve_backend(Bubblewrap, true, no, |_| true).is_err());
    }

    #[test]
    fn container_runs_are_locked_down() {
        let config = ExecutionConfig {
            memory_mb: Some(2048),
            pids_limit: Some(256),
            cpus: Some(2.0),
            ..ExecutionConfig::default()
        };
        let spec = ContainerSpec {
            engine: "docker",
            image: "rust@sha256:abc",
            name: "oxidize-1-0",
            cwd: Path::new("/home/me/proj"),
            user: Some("1000:1000".into()),
        };
        let args = container_args(&spec, &config, &["cargo".into(), "test".into()], &[]);
        let joined = args.join(" ");
        for flag in [
            "--rm",
            "--network none",
            "--read-only",
            "--cap-drop ALL",
            "--security-opt no-new-privileges",
            "--user 1000:1000",
            "-v /home/me/proj:/home/me/proj:rw",
            "-w /home/me/proj",
            "CARGO_NET_OFFLINE=true",
            "--memory 2048m",
            "--pids-limit 256",
            "--cpus 2",
        ] {
            assert!(joined.contains(flag), "missing `{flag}` in: {joined}");
        }
        // The image comes right before the command.
        assert_eq!(
            &args[args.len() - 3..],
            ["rust@sha256:abc", "cargo", "test"]
        );
        // Podman maps the user itself instead.
        let podman = container_args(
            &ContainerSpec {
                engine: "podman",
                ..spec
            },
            &config,
            &["true".into()],
            &[],
        );
        assert!(
            podman.join(" ").contains("--userns keep-id") && !podman.join(" ").contains("--user ")
        );
    }

    #[test]
    fn a_windows_workspace_moves_to_slash_workspace_and_its_paths_follow() {
        let spec = ContainerSpec {
            engine: "docker",
            image: "img@sha256:abc",
            name: "n",
            cwd: Path::new(r"C:\Users\me\proj"),
            user: None,
        };
        let args = container_args(
            &spec,
            &ExecutionConfig::default(),
            &["python3".into(), r"C:\Users\me\proj\tests\run.py".into()],
            &[("EXTRA", r"C:\Users\me\proj\data")],
        );
        let joined = args.join(" ");
        assert!(
            joined.contains(r"-v C:\Users\me\proj:/workspace:rw"),
            "{joined}"
        );
        assert!(
            joined.contains("-w /workspace")
                && joined.contains("CARGO_TARGET_DIR=/workspace/target")
        );
        assert!(joined.contains("EXTRA=/workspace/data"), "{joined}");
        assert_eq!(args.last().unwrap(), "/workspace/tests/run.py");
    }

    #[test]
    fn sandbox_refuses_a_program_it_cannot_see_instead_of_running_another() {
        let host = tempfile::tempdir().unwrap();
        let conda_bin = host.path().join("conda/bin");
        std::fs::create_dir_all(&conda_bin).unwrap();
        std::fs::write(conda_bin.join("python3"), "").unwrap();
        let path_env = std::env::join_paths([conda_bin.as_path(), Path::new("/usr/bin")]).unwrap();
        let usr = Path::new("/usr");

        // The host's first `python3` is outside every mount: refuse, naming it.
        let error = check_sandbox_program("python3", Some(&path_env), &[usr])
            .unwrap_err()
            .to_string();
        assert!(error.contains("can't see"), "{error}");
        assert!(
            error.contains(&conda_bin.join("python3").display().to_string()),
            "{error}"
        );
        // Mounting its directory makes it visible.
        let conda = host.path().join("conda");
        check_sandbox_program("python3", Some(&path_env), &[usr, conda.as_path()]).unwrap();
        // Absolute paths are checked the same way.
        assert!(
            check_sandbox_program(
                &conda_bin.join("python3").display().to_string(),
                None,
                &[usr]
            )
            .is_err()
        );
        check_sandbox_program("/usr/bin/python3", None, &[usr]).unwrap();
        // A name not on the host PATH at all is the sandbox's to report.
        check_sandbox_program("definitely-not-installed-xyz", Some(&path_env), &[usr]).unwrap();
    }

    #[test]
    fn sandbox_exposes_the_alternatives_symlinks_the_linker_needs() {
        // On Debian/Ubuntu `cc` resolves through /etc/alternatives; see SYSTEM_READ_ONLY_PATHS.
        assert!(SYSTEM_READ_ONLY_PATHS.contains(&"/etc/alternatives"));
        // ...but never the rest of /etc, which can hold credentials.
        assert!(!SYSTEM_READ_ONLY_PATHS.contains(&"/etc"));
    }

    #[test]
    fn captures_failure_and_limits() {
        let d = tempfile::tempdir().unwrap();
        let c = ExecutionConfig {
            timeout_seconds: 1,
            ..ExecutionConfig::default()
        };
        let runner = Runner {
            config: &c,
            trust_code: true,
        };
        let out = runner
            .run(
                d.path(),
                &[
                    "python3".into(),
                    "-c".into(),
                    "import sys; print('out'); sys.exit(3)".into(),
                ],
            )
            .unwrap();
        assert_eq!(out.code, Some(3));
        assert_eq!(out.stdout.trim(), "out");
        assert!(
            runner
                .run(
                    d.path(),
                    &[
                        "python3".into(),
                        "-c".into(),
                        "import time; time.sleep(10)".into()
                    ]
                )
                .is_err()
        );
        assert!(
            runner
                .run(
                    d.path(),
                    &["python3".into(), "-c".into(), "print('x' * 2000000)".into()]
                )
                .is_err()
        );
    }
}
