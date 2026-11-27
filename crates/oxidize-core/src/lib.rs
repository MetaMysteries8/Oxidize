//! Shared configuration, bounded process execution, and reproducible source snapshots.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

pub mod process;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub python: String,
    pub provider: ProviderConfig,
    pub verification: VerificationConfig,
    pub performance: PerformanceConfig,
    pub execution: ExecutionConfig,
    pub max_repairs: usize,
    /// `oxidize crosscheck`: a second opinion on a port that the app switches on and off with
    /// an environment variable (rust-anything's `RUST_ANYTHING_MODE` by default).
    pub crosscheck: CrosscheckConfig,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            python: "python3".into(),
            provider: ProviderConfig::default(),
            verification: VerificationConfig::default(),
            performance: PerformanceConfig::default(),
            execution: ExecutionConfig::default(),
            max_repairs: 2,
            crosscheck: CrosscheckConfig::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key_env: String,
    pub timeout_seconds: u64,
    pub retries: usize,
    pub max_output_tokens: u32,
    pub max_context_bytes: usize,
}
impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:11434/v1".into(),
            model: "".into(),
            api_key_env: "OXIDIZE_API_KEY".into(),
            timeout_seconds: 120,
            retries: 2,
            max_output_tokens: 4096,
            max_context_bytes: 32768,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VerificationConfig {
    pub cases_file: String,
    /// Argument vector, never interpreted by a shell.
    pub test_command: Vec<String>,
    pub comparison: ComparisonPolicy,
    /// Explicit regular files observed before/after each behavior case, relative to the project.
    pub file_effect_paths: Vec<String>,
}
impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            cases_file: "cases.json".into(),
            test_command: vec![
                "python3".into(),
                "-m".into(),
                "unittest".into(),
                "discover".into(),
                "-v".into(),
            ],
            comparison: ComparisonPolicy::default(),
            file_effect_paths: vec![],
        }
    }
}

/// Opt-ins affect only returned float values and call stdout/stderr, never mutations or errors.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComparisonPolicy {
    pub return_float_tolerance: Option<FloatTolerance>,
    pub normalize_output_line_endings: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FloatTolerance {
    pub absolute: f64,
    pub relative: f64,
}

impl ComparisonPolicy {
    pub fn validate(&self) -> Result<()> {
        if let Some(tolerance) = &self.return_float_tolerance {
            ensure!(
                tolerance.absolute.is_finite()
                    && tolerance.absolute >= 0.0
                    && tolerance.relative.is_finite()
                    && tolerance.relative >= 0.0,
                "return float tolerances must be finite and nonnegative"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PerformanceConfig {
    /// Timed samples per side in each round.
    pub iterations: usize,
    /// Untimed batches each side runs first, in every round (the same for both sides).
    pub warmup: usize,
    pub calls_per_sample: usize,
    /// The slowest round's speedup must reach this for a candidate to be accepted.
    pub minimum_speedup: f64,
    /// Interleaved original/candidate rounds, alternating which side goes first.
    pub rounds: usize,
    /// Largest per-round relative median absolute deviation still treated as a usable
    /// measurement. Noisier rounds make the measurement inconclusive: no speed claim.
    pub max_relative_mad: f64,
    /// Whole-application commands, timed on the original and the accepted workspace.
    pub workloads: Vec<WorkloadConfig>,
}

/// A user-defined whole-application workload: a command run from the project root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadConfig {
    pub name: String,
    pub command: Vec<String>,
}
impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            iterations: 7,
            warmup: 2,
            calls_per_sample: 3,
            minimum_speedup: 1.0,
            rounds: 3,
            max_relative_mad: 0.25,
            workloads: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExecutionConfig {
    pub timeout_seconds: u64,
    pub output_limit_bytes: usize,
    /// Extra toolchain directories visible read-only to bubblewrap. Never add credential directories.
    pub read_only_paths: Vec<PathBuf>,
    /// Cargo must use a pre-populated cache in the isolated default mode.
    pub offline: bool,
    /// Which isolation runs builds and tests. `auto` picks Bubblewrap on Linux when it
    /// works there, otherwise Podman, otherwise Docker, and refuses if none is usable.
    pub sandbox: SandboxKind,
    /// The container image for Docker/Podman, pinned by digest (`name@sha256:…`), with the
    /// Rust and Python toolchains and the project's dependencies available offline.
    pub image: Option<String>,
    /// Container-only resource limits. Bubblewrap and `--trust-code` runs are refused while any
    /// is set, rather than run without them (see SECURITY.md).
    pub memory_mb: Option<u64>,
    pub pids_limit: Option<u64>,
    pub cpus: Option<f64>,
}
impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 180,
            output_limit_bytes: 1_048_576,
            read_only_paths: vec![],
            offline: true,
            sandbox: SandboxKind::Auto,
            image: None,
            memory_mb: None,
            pids_limit: None,
            cpus: None,
        }
    }
}

/// `oxidize crosscheck`: run the same application commands with a port switched off and on,
/// in disposable copies, and compare what they do; time them only when they agree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CrosscheckConfig {
    /// The environment variable that switches between the original and the port.
    pub switch: String,
    /// The switch's value that runs the original.
    pub original: String,
    /// The switch's value that runs the port.
    pub candidate: String,
    /// Application commands, run from the project root of each disposable copy.
    pub commands: Vec<WorkloadConfig>,
    /// Regular files (relative to the project root) whose presence, length and SHA-256 must
    /// match after each command.
    pub file_paths: Vec<String>,
    /// Paths snapshots normally skip (such as `node_modules` or `.venv`) that the commands
    /// need, copied into each disposable copy. Symlinks inside them are skipped and reported.
    pub include: Vec<String>,
    /// Compare stderr as well as the exit code and stdout.
    pub compare_stderr: bool,
    /// Treat CRLF and LF as the same in stdout and stderr.
    pub normalize_line_endings: bool,
    /// Time commands that agree, in interleaved rounds, with `[performance]`'s policy.
    pub timing: bool,
}
impl Default for CrosscheckConfig {
    fn default() -> Self {
        Self {
            switch: "RUST_ANYTHING_MODE".into(),
            original: "old".into(),
            candidate: "new".into(),
            commands: vec![],
            file_paths: vec![],
            include: vec![],
            compare_stderr: true,
            normalize_line_endings: false,
            timing: true,
        }
    }
}

/// Environment variables the runner sets or passes through itself; a crosscheck switch must
/// not replace them.
const RESERVED_VARIABLES: &[&str] = &[
    "PATH",
    "HOME",
    "CARGO_HOME",
    "CARGO_TARGET_DIR",
    "RUSTUP_HOME",
    "PYO3_PYTHON",
    "CC",
    "AR",
    "SYSTEMROOT",
    "TMPDIR",
    "PYTHONDONTWRITEBYTECODE",
    "PYTHONHASHSEED",
    "XDG_RUNTIME_DIR",
    "DOCKER_HOST",
    "DOCKER_CONTEXT",
    "CONTAINER_HOST",
];

/// Whether any component of `path` names version control, Oxidize state or credentials.
fn credential_like(path: &Path) -> bool {
    path.components().any(|part| {
        let name = part.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git" | ".oxidize" | ".ssh" | ".aws" | ".cargo" | ".rustup"
        ) || name.starts_with(".env")
            || name.ends_with(".pem")
            || name.ends_with(".key")
    })
}

/// Workload and crosscheck command names: 1..64 characters, unique, with non-empty commands.
fn validate_commands(commands: &[WorkloadConfig], what: &str) -> Result<()> {
    ensure!(commands.len() <= 16, "at most 16 {what}");
    let mut names = std::collections::BTreeSet::new();
    for w in commands {
        ensure!(
            !w.name.is_empty()
                && w.name.len() <= 64
                && w.name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)),
            "{what} names must be 1..64 characters of letters, digits, '-', '_' or '.'"
        );
        ensure!(names.insert(&w.name), "duplicate {what} name {:?}", w.name);
        ensure!(
            !w.command.is_empty() && w.command.iter().all(|a| !a.is_empty()),
            "{what} {:?} needs a non-empty command",
            w.name
        );
    }
    Ok(())
}

/// Bounded, unique, safe relative paths that name no credentials.
fn validate_paths(paths: &[String], what: &str) -> Result<()> {
    ensure!(paths.len() <= 32, "at most 32 {what} may be listed");
    for (index, value) in paths.iter().enumerate() {
        ensure!(value.len() <= 1024, "{what} entry exceeds 1024 bytes");
        let path = safe_relative(value)?;
        ensure!(
            !paths[..index].contains(value),
            "duplicate {what} entry: {value}"
        );
        ensure!(
            !credential_like(&path),
            "reserved or credential-like {what} entry: {value}"
        );
    }
    Ok(())
}

impl CrosscheckConfig {
    pub fn validate(&self) -> Result<()> {
        let s = &self.switch;
        ensure!(
            !s.is_empty()
                && s.len() <= 64
                && s.chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase() || c == '_')
                && s.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
            "crosscheck.switch must be an environment variable name: A-Z, 0-9 and '_', up to 64 characters"
        );
        ensure!(
            !RESERVED_VARIABLES.contains(&s.as_str()),
            "crosscheck.switch must not be {s}: the runner sets or passes it itself"
        );
        for (what, value) in [("original", &self.original), ("candidate", &self.candidate)] {
            ensure!(
                value.len() <= 256 && !value.contains('\0'),
                "crosscheck.{what} must be at most 256 bytes with no NUL"
            );
        }
        ensure!(
            self.original != self.candidate,
            "crosscheck.original and crosscheck.candidate must differ, or both runs are the same"
        );
        validate_commands(&self.commands, "crosscheck commands")?;
        validate_paths(&self.file_paths, "crosscheck.file_paths")?;
        validate_paths(&self.include, "crosscheck.include")?;
        Ok(())
    }
}

/// The isolation backend for builds and tests (`--trust-code` bypasses all of them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxKind {
    #[default]
    Auto,
    Bubblewrap,
    Docker,
    Podman,
}
impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let config: Self = toml::from_str(
            &fs::read_to_string(root.join("oxidize.toml"))
                .context("read oxidize.toml (run oxidize init)")?,
        )
        .context("invalid oxidize.toml")?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        self.verification.comparison.validate()?;
        validate_paths(&self.verification.file_effect_paths, "file-effect paths")?;
        ensure!(!self.python.is_empty(), "python executable is empty");
        ensure!(
            !self.verification.test_command.is_empty(),
            "an independent test command is required"
        );
        safe_relative(&self.verification.cases_file)?;
        ensure!(
            (3..=1000).contains(&self.performance.iterations),
            "performance.iterations must be 3..1000"
        );
        ensure!(
            (1..=10000).contains(&self.performance.calls_per_sample),
            "calls_per_sample must be 1..10000"
        );
        ensure!(self.performance.warmup <= 1000, "warmup exceeds 1000");
        ensure!(
            (1..=100).contains(&self.performance.rounds),
            "performance.rounds must be 1..100"
        );
        ensure!(
            self.performance.max_relative_mad.is_finite()
                && self.performance.max_relative_mad > 0.0
                && self.performance.max_relative_mad <= 10.0,
            "performance.max_relative_mad must be greater than 0 and at most 10"
        );
        validate_commands(&self.performance.workloads, "performance.workloads")?;
        self.crosscheck.validate()?;
        ensure!(
            self.performance.minimum_speedup.is_finite() && self.performance.minimum_speedup > 0.0,
            "minimum_speedup must be positive and finite"
        );
        ensure!(
            (1..=3600).contains(&self.execution.timeout_seconds),
            "execution timeout must be 1..3600 seconds"
        );
        ensure!(
            (1024..=16_777_216).contains(&self.execution.output_limit_bytes),
            "output limit must be 1 KiB..16 MiB"
        );
        ensure!(
            self.execution
                .read_only_paths
                .iter()
                .all(|p| p.is_absolute()),
            "read_only_paths must be absolute host paths"
        );
        if let Some(image) = &self.execution.image {
            ensure!(
                image.contains("@sha256:"),
                "execution.image must be pinned by digest (name@sha256:…), not a tag that can change: {image}"
            );
        }
        ensure!(
            self.execution.cpus.is_none_or(|c| c > 0.0) && self.execution.memory_mb != Some(0),
            "execution.cpus and execution.memory_mb must be positive"
        );
        ensure!(self.max_repairs <= 10, "max_repairs exceeds 10");
        ensure!(
            self.provider.retries <= 5 && (1..=3600).contains(&self.provider.timeout_seconds),
            "invalid provider retry/timeout limits"
        );
        ensure!(
            (1024..=1_048_576).contains(&self.provider.max_context_bytes),
            "provider context limit must be 1 KiB..1 MiB"
        );
        ensure!(
            (1..=65536).contains(&self.provider.max_output_tokens),
            "invalid max_output_tokens"
        );
        Ok(())
    }
    pub fn write_new(root: &Path) -> Result<()> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("oxidize.toml"))
            .context("config already exists or cannot be created")?;
        file.write_all(toml::to_string_pretty(&Self::default())?.as_bytes())?;
        Ok(())
    }
}

pub fn safe_relative(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    ensure!(
        !value.is_empty() && !value.contains('\\'),
        "invalid relative path"
    );
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "path must stay inside the project: {value}"
    );
    Ok(path.into())
}

fn included(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !matches!(
        name.as_ref(),
        ".git"
            | ".oxidize"
            | "target"
            | "node_modules"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".pytest_cache"
            | "dist"
            | "build"
            | ".aws"
            | ".ssh"
            | ".cargo"
            | ".rustup"
    ) && !name.starts_with(".env")
        && !name.ends_with(".pem")
        && !name.ends_with(".key")
}
/// No symlink traversal, bounded files. Credential paths are never snapshotted or sent to models.
pub fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = vec![];
    let mut total_bytes = 0u64;
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(included)
    {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            bail!(
                "symlinks are not supported in snapshots: {}",
                entry.path().display()
            );
        }
        if entry.file_type().is_file() {
            total_bytes += entry.metadata()?.len();
            ensure!(
                total_bytes <= 536_870_912,
                "snapshot exceeds 512 MiB of included files"
            );
            ensure!(
                entry.metadata()?.len() <= 16_777_216,
                "file exceeds 16 MiB: {}",
                entry.path().display()
            );
            out.push(entry.path().strip_prefix(root)?.into());
            ensure!(out.len() <= 100_000, "snapshot exceeds 100,000 files");
        }
    }
    out.sort();
    Ok(out)
}
pub fn snapshot(source: &Path, destination: &Path) -> Result<()> {
    ensure!(!destination.exists(), "snapshot destination already exists");
    // Collect before creating a destination inside the source.
    let paths = files(source)?;
    fs::create_dir_all(destination)?;
    for relative in paths {
        let target = destination.join(&relative);
        fs::create_dir_all(target.parent().context("missing parent")?)?;
        fs::copy(source.join(&relative), target)?;
    }
    Ok(())
}
pub fn fingerprint(root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    for relative in files(root)? {
        let name = relative.to_string_lossy();
        let bytes = fs::read(root.join(&relative))?;
        hasher.update((name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}

/// Replace `path` with `bytes` so a reader sees either the old file or the whole new one.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let mut f = fs::File::create(&temporary)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(temporary, path)?;
    #[cfg(unix)]
    fs::File::open(path.parent().context("missing state directory")?)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_roundtrip_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        Config::write_new(dir.path()).unwrap();
        let mut c = Config::load(dir.path()).unwrap();
        c.performance.minimum_speedup = f64::NAN;
        assert!(c.validate().is_err());
        assert!(Config::write_new(dir.path()).is_err());
    }
    #[test]
    fn rejects_paths_and_secrets() {
        for p in ["../x", "/etc/passwd", "x/../y", "", "a\\b"] {
            assert!(safe_relative(p).is_err(), "{p}");
        }
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(".env"), "secret").unwrap();
        fs::write(d.path().join("a.py"), "print(1)").unwrap();
        assert_eq!(files(d.path()).unwrap(), vec![PathBuf::from("a.py")]);
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlinks() {
        let d = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", d.path().join("source.py")).unwrap();
        assert!(files(d.path()).is_err());
    }
    #[test]
    fn snapshot_hash_and_exclusions() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("a"), "before").unwrap();
        let copy = d.path().join(".oxidize/base");
        snapshot(d.path(), &copy).unwrap();
        assert_eq!(fingerprint(d.path()).unwrap(), fingerprint(&copy).unwrap());
        fs::write(copy.join("a"), "after").unwrap();
        assert_ne!(fingerprint(d.path()).unwrap(), fingerprint(&copy).unwrap());
    }
}
