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
    pub iterations: usize,
    pub warmup: usize,
    pub calls_per_sample: usize,
    pub minimum_speedup: f64,
}
impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            iterations: 7,
            warmup: 2,
            calls_per_sample: 3,
            minimum_speedup: 1.0,
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
}
impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 180,
            output_limit_bytes: 1_048_576,
            read_only_paths: vec![],
            offline: true,
        }
    }
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
        ensure!(
            self.verification.file_effect_paths.len() <= 32,
            "at most 32 file-effect paths may be observed"
        );
        for (index, value) in self.verification.file_effect_paths.iter().enumerate() {
            let path = safe_relative(value)?;
            ensure!(value.len() <= 1024, "file-effect path exceeds 1024 bytes");
            ensure!(
                !self.verification.file_effect_paths[..index].contains(value),
                "duplicate file-effect path: {value}"
            );
            for part in path.components() {
                let name = part.as_os_str().to_string_lossy();
                ensure!(
                    !matches!(
                        name.as_ref(),
                        ".git" | ".oxidize" | ".ssh" | ".aws" | ".cargo" | ".rustup"
                    ) && !name.starts_with(".env")
                        && !name.ends_with(".pem")
                        && !name.ends_with(".key"),
                    "reserved or credential-like file-effect path"
                );
            }
        }
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
    use std::io::Write;
    let temporary = path.with_extension("json.tmp");
    let mut f = fs::File::create(&temporary)?;
    f.write_all(&serde_json::to_vec_pretty(value)?)?;
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
