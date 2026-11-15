use crate::ExecutionConfig;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::Path,
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
        let mut cmd = if self.trust_code {
            let mut cmd = Command::new(&argv[0]);
            cmd.args(&argv[1..]);
            cmd
        } else {
            ensure!(
                cfg!(target_os = "linux"),
                "sandbox requires Linux bubblewrap; --trust-code explicitly permits execution without OS isolation"
            );
            ensure!(
                self.config.read_only_paths.is_empty()
                    || HOST_MOUNTS_ALLOWED.load(Ordering::Relaxed),
                "extra host mounts require --allow-host-paths after reviewing execution.read_only_paths"
            );
            let mut cmd = Command::new("bwrap");
            cmd.args(["--die-with-parent", "--unshare-all", "--new-session"]);
            for path in ["/usr", "/bin", "/lib", "/lib64", "/etc/ld.so.cache"] {
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
        };
        // Do not pass provider credentials or unrelated host secrets into builds/tests.
        cmd.env_clear();
        for key in [
            "PATH",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "PYO3_PYTHON",
            "CC",
            "AR",
            "SYSTEMROOT",
            "TMPDIR",
        ] {
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

#[cfg(test)]
mod tests {
    use super::*;
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
