//! Transactional, one-component Python migration. Input source is never rewritten.
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use oxidize_analysis::{Analysis, Component, analyze, plan};
use oxidize_core::{
    Config, atomic_json, digest, fingerprint,
    process::{Output, Runner},
    safe_relative, snapshot,
};
use oxidize_providers::{Generation, OpenAiCompatible, Provider, ResponseFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepared,
    Generating,
    Building,
    Verifying,
    Accepted,
    Rejected,
    RolledBack,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Benchmark {
    pub samples_ns: Vec<f64>,
    pub median_ns: f64,
    pub mad_ns: f64,
    pub successful_cases: usize,
    pub calls_per_sample: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub observations: Vec<Value>,
    pub import_stdout: String,
    pub import_stderr: String,
    pub benchmark: Benchmark,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attempt {
    pub id: usize,
    pub component: String,
    pub base: Option<usize>,
    pub phase: Phase,
    pub source_fingerprint: String,
    pub candidate_fingerprint: Option<String>,
    pub generations: Vec<Generation>,
    pub checks: Vec<(String, Output)>,
    pub baseline: Option<Evidence>,
    pub candidate: Option<Evidence>,
    pub speedup: Option<f64>,
    pub error: Option<String>,
    pub repaired: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub source_fingerprint: String,
    pub source_revision: Option<String>,
    pub config: Config,
    pub current: Option<usize>,
    pub attempts: Vec<Attempt>,
}
pub struct Session {
    pub root: PathBuf,
    pub directory: PathBuf,
    pub state: State,
    _lock: fs::File,
}

impl Drop for Session {
    fn drop(&mut self) {
        // Closing our descriptor is insufficient while a forked child retains
        // the same open-file description. Explicitly release the session lock.
        let _ = FileExt::unlock(&self._lock);
    }
}

impl Session {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .context("project directory does not exist")?;
        let directory = root.join(".oxidize");
        ensure!(!directory.is_symlink(), ".oxidize must not be a symlink");
        fs::create_dir_all(&directory)?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("session.lock"))?;
        lock.try_lock_exclusive()
            .context("another Oxidize process owns this project")?;
        let state_path = directory.join("state.json");
        let state = if state_path.exists() {
            let state: State = serde_json::from_slice(&fs::read(&state_path)?)
                .context("checkpoint is corrupt; original project is untouched")?;
            ensure!(state.schema_version == 1, "unsupported checkpoint schema");
            state.config.validate()?;
            ensure!(
                state
                    .attempts
                    .iter()
                    .enumerate()
                    .all(|(i, a)| a.id == i + 1 && a.base.is_none_or(|b| b < a.id && b > 0)),
                "invalid checkpoint history"
            );
            ensure!(
                state
                    .current
                    .is_none_or(|id| id > 0 && id <= state.attempts.len()),
                "invalid current checkpoint"
            );
            state
        } else {
            let config = Config::load(&root)?;
            let source_fingerprint = fingerprint(&root)?;
            let base = directory.join("baseline");
            // An interrupted initial snapshot is never reused as evidence.
            if base.exists() {
                fs::remove_dir_all(&base)?;
            }
            snapshot(&root, &base)?;
            ensure!(
                fingerprint(&base)? == source_fingerprint,
                "project changed while capturing baseline; retry"
            );
            let revision = std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().into());
            let state = State {
                schema_version: 1,
                source_fingerprint,
                source_revision: revision,
                config,
                current: None,
                attempts: vec![],
            };
            atomic_json(&state_path, &state)?;
            state
        };
        Ok(Self {
            root,
            directory,
            state,
            _lock: lock,
        })
    }
    pub fn save(&self) -> Result<()> {
        atomic_json(&self.directory.join("state.json"), &self.state)
    }
    pub fn workspace(&self, id: Option<usize>) -> PathBuf {
        id.map(|i| {
            self.directory
                .join("attempts")
                .join(format!("{i:04}"))
                .join("project")
        })
        .unwrap_or_else(|| self.directory.join("baseline"))
    }
    pub fn current_workspace(&self) -> PathBuf {
        self.workspace(self.state.current)
    }
    pub fn assert_source_unchanged(&self) -> Result<()> {
        ensure!(
            fingerprint(&self.root)? == self.state.source_fingerprint,
            "input project changed since baseline; preserve this session and start from a fresh project copy"
        );
        Ok(())
    }
    fn assert_base_unchanged(&self) -> Result<()> {
        let expected = match self.state.current {
            Some(id) => self.state.attempts[id - 1]
                .candidate_fingerprint
                .as_ref()
                .context("accepted workspace has no fingerprint")?,
            None => &self.state.source_fingerprint,
        };
        ensure!(
            &fingerprint(&self.current_workspace())? == expected,
            "current workspace was changed outside Oxidize; refusing to overwrite or reuse stale evidence"
        );
        Ok(())
    }
    pub fn rollback(&mut self) -> Result<()> {
        let id = self
            .state
            .current
            .context("no accepted migration to roll back")?;
        self.assert_base_unchanged()?;
        let a = &mut self.state.attempts[id - 1];
        a.phase = Phase::RolledBack;
        self.state.current = a.base;
        self.save()
    }
    pub fn migrate(
        &mut self,
        component_id: &str,
        response: Option<&Path>,
        trust_code: bool,
        allow_provider: bool,
    ) -> Result<usize> {
        self.assert_source_unchanged()?;
        self.assert_base_unchanged()?;
        ensure!(
            !self.state.attempts.last().is_some_and(|a| matches!(
                a.phase,
                Phase::Prepared | Phase::Generating | Phase::Building | Phase::Verifying
            )),
            "unfinished attempt exists; use resume"
        );
        let analysis = analyze(&self.current_workspace(), &self.state.config)?;
        let candidate = plan(&analysis)
            .candidates
            .into_iter()
            .find(|c| c.id == component_id)
            .context("component is absent or blocked; inspect oxidize plan")?;
        ensure!(
            !analysis.test_files.is_empty(),
            "independent test files are required before migration"
        );
        ensure!(
            !self
                .state
                .attempts
                .iter()
                .any(|a| a.component == component_id && a.phase == Phase::Accepted),
            "component already migrated"
        );
        validate_component(&candidate)?;
        load_cases(&self.current_workspace(), &self.state.config, component_id)?;
        let id = self.state.attempts.len() + 1;
        self.state.attempts.push(Attempt {
            id,
            component: component_id.into(),
            base: self.state.current,
            phase: Phase::Prepared,
            source_fingerprint: fingerprint(&self.current_workspace())?,
            candidate_fingerprint: None,
            generations: vec![],
            checks: vec![],
            baseline: None,
            candidate: None,
            speedup: None,
            error: None,
            repaired: 0,
        });
        self.save()?;
        self.execute(id, response, trust_code, allow_provider)?;
        Ok(id)
    }
    pub fn resume(
        &mut self,
        response: Option<&Path>,
        trust_code: bool,
        allow_provider: bool,
    ) -> Result<usize> {
        self.assert_source_unchanged()?;
        self.assert_base_unchanged()?;
        let id = self
            .state
            .attempts
            .last()
            .context("no attempt to resume")?
            .id;
        ensure!(
            !matches!(
                self.state.attempts[id - 1].phase,
                Phase::Accepted | Phase::RolledBack
            ),
            "latest attempt is complete"
        );
        ensure!(
            self.state.attempts[id - 1].base == self.state.current,
            "resume base differs from current checkpoint"
        );
        // Replay in a fresh directory using persisted generated code; no partially integrated source is trusted.
        self.execute(id, response, trust_code, allow_provider)?;
        Ok(id)
    }
    fn execute(
        &mut self,
        id: usize,
        response: Option<&Path>,
        trust_code: bool,
        allow_provider: bool,
    ) -> Result<()> {
        let result = self.execute_inner(id, response, trust_code, allow_provider);
        if let Err(error) = result {
            self.state.attempts[id - 1].phase = Phase::Rejected;
            self.state.attempts[id - 1].error = Some(format!("{error:#}"));
            self.save()?;
            return Err(error.context(
                "migration rejected; previous accepted workspace and input project are preserved",
            ));
        }
        Ok(())
    }
    fn set_phase(&mut self, id: usize, phase: Phase) -> Result<()> {
        ensure!(
            !oxidize_core::process::cancelled(),
            "migration cancelled; resume can replay this checkpoint"
        );
        self.state.attempts[id - 1].phase = phase;
        self.save()
    }
    fn check(
        &mut self,
        id: usize,
        root: &Path,
        name: &str,
        argv: &[String],
        trust: bool,
    ) -> Result<Output> {
        let out = Runner {
            config: &self.state.config.execution,
            trust_code: trust,
        }
        .run_with_env(root, argv, &[("PYO3_PYTHON", &self.state.config.python)])?;
        self.state.attempts[id - 1]
            .checks
            .push((name.into(), out.clone()));
        self.save()?;
        Ok(out)
    }
    fn execute_inner(
        &mut self,
        id: usize,
        response: Option<&Path>,
        trust: bool,
        allow_provider: bool,
    ) -> Result<()> {
        let config = self.state.config.clone();
        let base = self.workspace(self.state.attempts[id - 1].base);
        ensure!(
            fingerprint(&base)? == self.state.attempts[id - 1].source_fingerprint,
            "migration base changed"
        );
        let analysis = analyze(&base, &config)?;
        let component = analysis
            .components
            .iter()
            .find(|c| c.id == self.state.attempts[id - 1].component)
            .context("component missing from checkpoint")?
            .clone();
        validate_component(&component)?;
        let cases = load_cases(&base, &config, &component.id)?;
        let project = self.workspace(Some(id));
        let attempt_dir = project.parent().context("attempt parent")?;
        fs::create_dir_all(attempt_dir)?;
        if project.exists() {
            let archived = attempt_dir.join(format!(
                "interrupted-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
            ));
            fs::rename(&project, archived)?;
        }
        snapshot(&base, &project)?;
        self.state.attempts[id - 1].checks.clear();
        self.state.attempts[id - 1].error = None;
        self.state.attempts[id - 1].baseline = None;
        self.state.attempts[id - 1].candidate = None;
        self.set_phase(id, Phase::Prepared)?;

        // Tests and observations happen before asking a provider for implementation.
        let baseline_test = self.check(
            id,
            &base,
            "baseline tests",
            &config.verification.test_command,
            trust,
        )?;
        require_tests(&baseline_test)?;
        let baseline = self.evidence(&base, &component, &cases, trust)?;
        self.state.attempts[id - 1].baseline = Some(baseline.clone());
        self.save()?;

        let module = format!("_oxidize_{}", &digest(component.id.as_bytes())[..12]);
        let provider: Option<Box<dyn Provider>> = if let Some(path) = response {
            Some(Box::new(ResponseFile::load(path)?))
        } else if allow_provider {
            Some(Box::new(OpenAiCompatible::new(config.provider.clone())?))
        } else {
            None
        };
        let prompt = context_prompt(
            &base,
            &component,
            &analysis,
            &module,
            config.provider.max_context_bytes,
        )?;
        self.set_phase(id, Phase::Generating)?;
        if self.state.attempts[id - 1].generations.is_empty() || response.is_some() {
            let generated = provider
                .as_ref()
                .context(
                    "source transmission requires --allow-provider or an explicit --response file",
                )?
                .generate(&prompt)?;
            self.state.attempts[id - 1].generations.push(generated);
            self.save()?;
        }
        let native = project.join("oxidized").join(&module);
        ensure!(
            !base.join("oxidized").join(&module).exists(),
            "native component already exists"
        );
        fs::create_dir_all(native.join("src"))?;
        fs::write(
            native.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{module}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"{module}\"\ncrate-type = [\"cdylib\"]\n[dependencies]\npyo3 = {{ version = \"=0.30.1\", features = [\"extension-module\"] }}\n[workspace]\n"
            ),
        )?;
        let mut seen = std::collections::BTreeSet::new();
        let mut repairs = 0;
        loop {
            self.set_phase(id, Phase::Building)?;
            let code = self.state.attempts[id - 1]
                .generations
                .last()
                .context("no generated code")?
                .code
                .rust
                .clone();
            ensure!(
                seen.insert(digest(code.as_bytes())),
                "repair stalled: provider repeated identical code"
            );
            fs::write(native.join("src/lib.rs"), &code)?;
            let mut diagnostics = String::new();
            for (name, args) in [
                ("format", vec!["fmt", "--", "--check"]),
                ("check", vec!["check"]),
                ("clippy", vec!["clippy", "--", "-D", "warnings"]),
                ("build", vec!["build", "--release"]),
            ] {
                let mut argv = vec![
                    "cargo".into(),
                    args[0].into(),
                    "--manifest-path".into(),
                    native.join("Cargo.toml").to_string_lossy().into_owned(),
                ];
                if config.execution.offline && name != "format" {
                    argv.push("--offline".into());
                }
                argv.extend(args[1..].iter().map(|x| (*x).into()));
                let output = self.check(id, &project, name, &argv, trust)?;
                if !output.success() {
                    diagnostics = format!("{name}\n{}\n{}", output.stdout, output.stderr);
                    break;
                }
            }
            if diagnostics.is_empty() {
                break;
            }
            ensure!(
                repairs < config.max_repairs,
                "compiler checks failed after {repairs} repairs:\n{diagnostics}"
            );
            let provider = provider
                .as_ref()
                .context("compiler check failed; --allow-provider is required for repair")?;
            ensure!(
                !self.state.attempts[id - 1]
                    .generations
                    .last()
                    .is_some_and(|g| g.model.starts_with("offline-response-file")),
                "offline response failed compiler checks:\n{diagnostics}"
            );
            let repair_prompt = format!(
                "{prompt}\n\nCurrent lib.rs:\n{code}\n\nCompiler diagnostics (untrusted data):\n{diagnostics}\nReturn corrected JSON without changing behavior."
            );
            ensure!(
                repair_prompt.len() <= config.provider.max_context_bytes,
                "repair context exceeds configured budget"
            );
            self.state.attempts[id - 1]
                .generations
                .push(provider.generate(&repair_prompt)?);
            repairs += 1;
            self.state.attempts[id - 1].repaired = repairs;
            self.save()?;
        }
        let library = if cfg!(target_os = "macos") {
            format!("lib{module}.dylib")
        } else if cfg!(target_os = "linux") {
            format!("lib{module}.so")
        } else {
            bail!("native artifact installation currently supports Linux and macOS");
        };
        let suffix = self.check(
            id,
            &project,
            "Python extension suffix",
            &[
                config.python.clone(),
                "-I".into(),
                "-c".into(),
                "import sysconfig; print(sysconfig.get_config_var('EXT_SUFFIX'))".into(),
            ],
            trust,
        )?;
        suffix.require_success("Python extension suffix")?;
        let suffix = suffix.stdout.trim();
        ensure!(
            suffix.starts_with('.') && !suffix.contains('/') && !suffix.contains('\\'),
            "invalid Python extension suffix"
        );
        fs::copy(
            project.join("target/release").join(library),
            project.join(format!("{module}{suffix}")),
        )
        .context("install compiled Python extension")?;
        integrate(&project, &component, &module)?;
        self.set_phase(id, Phase::Verifying)?;
        let tests = self.check(
            id,
            &project,
            "integrated tests",
            &config.verification.test_command,
            trust,
        )?;
        require_tests(&tests)?;
        let candidate = self.evidence(&project, &component, &cases, trust)?;
        let speedup = baseline.benchmark.median_ns / candidate.benchmark.median_ns;
        self.state.attempts[id - 1].candidate = Some(candidate.clone());
        self.state.attempts[id - 1].speedup = Some(speedup);
        self.save()?;
        ensure!(
            equivalent(&baseline, &candidate),
            "differential mismatch: return values, types, errors, or stdout/stderr differ"
        );
        ensure!(
            speedup.is_finite() && speedup >= config.performance.minimum_speedup,
            "measured speedup {speedup:.3}x is below required {:.3}x",
            config.performance.minimum_speedup
        );
        ensure!(
            fingerprint(&base)? == self.state.attempts[id - 1].source_fingerprint,
            "tests modified baseline files; acceptance refused"
        );
        self.assert_source_unchanged()?;
        self.state.attempts[id - 1].candidate_fingerprint = Some(fingerprint(&project)?);
        // The accepted pointer and phase are committed together in one atomic state write.
        self.state.attempts[id - 1].phase = Phase::Accepted;
        let previous = self.state.current;
        self.state.current = Some(id);
        if let Err(error) = self.save() {
            self.state.current = previous;
            return Err(error);
        }
        Ok(())
    }
    fn evidence(&self, root: &Path, c: &Component, cases: &Value, trust: bool) -> Result<Evidence> {
        let module = c
            .path
            .strip_suffix(".py")
            .context("not Python")?
            .replace('/', ".");
        let argv = vec![
            self.state.config.python.clone(),
            "-c".into(),
            include_str!("harness.py").into(),
            module,
            c.name.clone(),
            serde_json::to_string(cases)?,
            serde_json::to_string(&self.state.config.performance)?,
        ];
        let output = Runner {
            config: &self.state.config.execution,
            trust_code: trust,
        }
        .run(root, &argv)?;
        output.require_success("differential/benchmark workload")?;
        let evidence: Evidence =
            serde_json::from_str(&output.stdout).context("decode behavior/benchmark evidence")?;
        ensure!(
            evidence.benchmark.median_ns.is_finite() && evidence.benchmark.median_ns > 0.0,
            "invalid benchmark timing"
        );
        Ok(evidence)
    }
    pub fn verify(&self, trust: bool) -> Result<()> {
        self.assert_source_unchanged()?;
        self.assert_base_unchanged()?;
        let out = Runner {
            config: &self.state.config.execution,
            trust_code: trust,
        }
        .run(
            &self.current_workspace(),
            &self.state.config.verification.test_command,
        )?;
        require_tests(&out)
    }
    pub fn benchmark(&self, component_id: &str, trust: bool) -> Result<Evidence> {
        self.assert_source_unchanged()?;
        self.assert_base_unchanged()?;
        let analysis = analyze(&self.workspace(None), &self.state.config)?;
        let c = analysis
            .components
            .iter()
            .find(|c| c.id == component_id)
            .context("component absent from original source")?;
        validate_component(c)?;
        let cases = load_cases(&self.current_workspace(), &self.state.config, component_id)?;
        self.evidence(&self.current_workspace(), c, &cases, trust)
    }
    pub fn report(&self) -> Result<PathBuf> {
        atomic_json(&self.directory.join("report.json"), &self.state)?;
        let mut report = format!(
            "# Oxidize migration report\n\nInput revision: {:?}\n\nOriginal fingerprint: `{}`\n\nCurrent accepted workspace: `{}`\n\nInput repository was not rewritten. Python runtime and PyO3 remain required. Verified functionality coverage: **unknown**. Component counts do not measure functionality coverage.\n\n",
            self.state.source_revision,
            self.state.source_fingerprint,
            self.current_workspace().display()
        );
        for a in &self.state.attempts {
            report += &format!(
                "## Attempt {}: `{}`\n\nStatus: {:?}\n\nCompiler repairs: {}\n\n",
                a.id, a.component, a.phase, a.repaired
            );
            if let Some(speedup) = a.speedup {
                report += &format!(
                    "Measured component workload speedup: **{speedup:.3}x** (includes Python/FFI call overhead). This is not a whole-application speedup.\n\n"
                );
            }
            for (label, e) in [("Original", &a.baseline), ("Candidate", &a.candidate)] {
                if let Some(e) = e {
                    report += &format!(
                        "{label}: {} cases; median {:.0} ns/sample; median absolute deviation {:.0} ns; {} samples.\n\n",
                        e.observations.len(),
                        e.benchmark.median_ns,
                        e.benchmark.mad_ns,
                        e.benchmark.samples_ns.len()
                    );
                }
            }
            for (label, out) in &a.checks {
                report += &format!("- {label}: exit {:?}\n", out.code);
            }
            for g in &a.generations {
                report += &format!(
                    "\nModel: `{}`; prompt tokens {:?}, completion tokens {:?}; cost unknown.\n",
                    g.model, g.usage.prompt_tokens, g.usage.completion_tokens
                );
            }
            if let Some(error) = &a.error {
                report += &format!("\nRejection: {error}\n");
            }
            report += "\n";
        }
        report += "## Evidence limits\n\nOnly configured tests and JSON cases were checked. Stateful behavior, external services, concurrency, floating tolerances, CPU, allocations, peak memory, and bridge-only overhead were not measured. Timing is sequential and subject to system noise. No claim of universal equivalence or full Rustification.\n";
        let path = self.directory.join("report.md");
        fs::write(&path, report)?;
        Ok(path)
    }
}

pub fn equivalent(a: &Evidence, b: &Evidence) -> bool {
    a.observations == b.observations
        && a.import_stdout == b.import_stdout
        && a.import_stderr == b.import_stderr
}
fn require_tests(out: &Output) -> Result<()> {
    out.require_success("independent tests")?;
    let combined = format!("{}{}", out.stdout, out.stderr);
    ensure!(
        !combined.contains("Ran 0 tests") && !combined.contains("no tests ran"),
        "test runner discovered zero tests"
    );
    Ok(())
}
fn validate_component(c: &Component) -> Result<()> {
    safe_relative(&c.path)?;
    ensure!(
        !c.path.contains('/') && c.path.ends_with(".py"),
        "initial Python adapter supports top-level modules only"
    );
    ensure!(
        c.blockers.is_empty(),
        "component is blocked: {:?}",
        c.blockers
    );
    ensure!(
        c.start_line > 0 && c.end_line >= c.start_line,
        "invalid AST source span"
    );
    Ok(())
}
fn load_cases(root: &Path, c: &Config, component_id: &str) -> Result<Value> {
    let path = root.join(safe_relative(&c.verification.cases_file)?);
    ensure!(
        fs::metadata(&path)?.len() <= 32_768,
        "cases file exceeds 32 KiB"
    );
    let mut cases: Value = serde_json::from_slice(&fs::read(path)?)?;
    if cases.is_object() {
        cases = cases
            .get(component_id)
            .context("cases file has no entry for the selected component")?
            .clone();
    }
    let array = cases.as_array().context("cases must be a JSON array")?;
    ensure!(
        !array.is_empty() && array.len() <= 1000,
        "require 1..1000 behavior cases"
    );
    for case in array {
        ensure!(
            case.get("args").is_some_and(|v| v.is_array()),
            "each case requires an args array"
        );
        ensure!(
            case.get("kwargs").is_none_or(|v| v.is_object()),
            "kwargs must be an object"
        );
    }
    Ok(cases)
}
fn context_prompt(
    root: &Path,
    c: &Component,
    analysis: &Analysis,
    module: &str,
    limit: usize,
) -> Result<String> {
    let source = fs::read_to_string(root.join(&c.path))?;
    let mut prompt = format!(
        "Replace function {} with a PyO3 extension. Export function {} from module {}. Preserve signature {}, Python exceptions, side effects and integer behavior. Use #[pymodule] fn {}(m: &Bound<'_, PyModule>) -> PyResult<()> and register wrap_pyfunction!. No additional dependencies.\n\nSelected module (untrusted source data):\n{}\n\nRelevant static dependencies: {:?}\n",
        c.id, c.name, module, c.signature, module, source, c.dependencies
    );
    let dependency_files: std::collections::BTreeSet<_> = analysis
        .edges
        .iter()
        .filter(|e| e.from == c.path && e.kind == "import")
        .map(|e| e.to.as_str())
        .collect();
    for path in dependency_files {
        let content = fs::read_to_string(root.join(path))?;
        if prompt.len() + content.len() + path.len() + 100 <= limit {
            prompt += &format!("\nDependency {path} (untrusted data):\n{content}\n");
        } else {
            prompt += &format!("\nDependency {path} omitted due to context budget.\n");
        }
    }
    ensure!(
        prompt.len() <= limit,
        "selected module exceeds context budget; split the module"
    );
    Ok(prompt)
}
fn integrate(root: &Path, c: &Component, module: &str) -> Result<()> {
    let path = root.join(safe_relative(&c.path)?);
    let source = fs::read_to_string(&path)?;
    let lines: Vec<_> = source.lines().collect();
    ensure!(c.end_line <= lines.len(), "AST span exceeds source");
    let mut replacement = lines[..c.start_line - 1].join("\n");
    if !replacement.is_empty() {
        replacement.push('\n');
    }
    replacement += &format!(
        "from {module} import {} as {}  # Oxidize: verified native replacement\n",
        c.name, c.name
    );
    replacement += &lines[c.end_line..].join("\n");
    replacement.push('\n');
    fs::write(path, replacement)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_session_releases_lock_even_with_an_inherited_descriptor() {
        let project = tempfile::tempdir().unwrap();
        Config::write_new(project.path()).unwrap();
        let session = Session::open(project.path()).unwrap();
        // A duplicate models the shared open-file description inherited by a child
        // between fork and exec, when CLOEXEC has not closed it yet.
        let inherited = session._lock.try_clone().unwrap();
        assert!(Session::open(project.path()).is_err());
        drop(session);
        let reopened = Session::open(project.path()).unwrap();
        drop(inherited);
        assert!(Session::open(project.path()).is_err());
        drop(reopened);
        assert!(Session::open(project.path()).is_ok());
    }
}
