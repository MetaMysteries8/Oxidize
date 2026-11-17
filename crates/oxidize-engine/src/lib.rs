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

mod comparison;
mod faults;
mod history;
pub mod sampling;
pub mod workloads;
use comparison::{differential_mismatch, has_argument_evidence, has_file_effect_evidence};
pub use comparison::{equivalent, equivalent_with_policy};
pub use faults::{Fault, FaultPoint, InjectedCrash, InjectedFailure};
pub use history::{CHECKPOINT_SCHEMA_VERSION, EVENT_SCHEMA_VERSION, Event, EventKind};
use sampling::{Measurement, Round, Side, Verdict};
use workloads::{WorkloadRound, WorkloadRun, WorkloadSide};

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
    /// Reported by the harness from inside the run. Missing in evidence from before it existed.
    #[serde(default)]
    pub interpreter: Option<Interpreter>,
    /// What produced this measurement. Missing in evidence recorded before it existed.
    #[serde(default)]
    pub context: Option<MeasurementContext>,
}

/// The Python that actually executed the timing, as it reported itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interpreter {
    pub implementation: String,
    pub version: String,
    pub executable: String,
}

/// Everything needed to judge or reproduce a measurement (Milestone 2 §1): where it ran,
/// how it was isolated, the sampling policy, what it covers and what it had to beat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeasurementContext {
    /// `std::env::consts::OS` and `ARCH` of the host that ran it.
    pub os: String,
    pub arch: String,
    /// `bubblewrap`, `docker`, `podman` or `trust-code`.
    pub isolation: String,
    /// `rustc --version` as seen by the isolated run that built the candidate, if it answered.
    #[serde(default)]
    pub rustc: Option<String>,
    /// The configured interpreter command and the harness mode that ran.
    pub command: Vec<String>,
    pub iterations: usize,
    pub warmup: usize,
    pub calls_per_sample: usize,
    pub minimum_speedup: f64,
    /// What the number covers. Never a whole-application claim.
    pub scope: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    /// Legacy checkpoint evidence remains readable but cannot prove mutation compatibility.
    #[serde(default = "legacy_observation_schema_version")]
    pub observation_schema_version: u32,
    /// Missing in older evidence: no file effects were captured.
    #[serde(default)]
    pub file_effect_paths: Vec<String>,
    pub observations: Vec<Value>,
    pub import_stdout: String,
    pub import_stderr: String,
    pub benchmark: Benchmark,
}
fn legacy_observation_schema_version() -> u32 {
    1
}

/// One report line saying exactly what produced a measurement, or that it wasn't recorded.
pub fn measurement_line(benchmark: &Benchmark) -> String {
    let interpreter = benchmark
        .interpreter
        .as_ref()
        .map(|i| format!("{} {} (`{}`)", i.implementation, i.version, i.executable))
        .unwrap_or_else(|| "an unrecorded interpreter".into());
    match &benchmark.context {
        Some(c) => format!(
            "Measured with {interpreter} and {} on {}/{}, isolation `{}`: {} samples after {} warm-up, {} calls per sample, acceptance threshold {}x. Scope: {}.\n\n",
            c.rustc.as_deref().unwrap_or("an unrecorded rustc"),
            c.os,
            c.arch,
            c.isolation,
            c.iterations,
            c.warmup,
            c.calls_per_sample,
            c.minimum_speedup,
            c.scope
        ),
        None => {
            "Measurement context: not recorded (evidence from before contexts were captured).\n\n"
                .into()
        }
    }
}
#[derive(Deserialize)]
struct Observations {
    observation_schema_version: u32,
    file_effect_paths: Vec<String>,
    observations: Vec<Value>,
    import_stdout: String,
    import_stderr: String,
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
    /// Where the behaviour cases and independent tests came from. Missing in older sessions.
    #[serde(default)]
    pub provenance: Option<CaseProvenance>,
    /// The interleaved original/candidate timing that decided acceptance, with raw samples.
    /// Missing in older sessions, whose speedup compared two separate single runs.
    #[serde(default)]
    pub measurement: Option<Measurement>,
    /// Every repair, in order. Missing in older sessions, which recorded only the count.
    #[serde(default)]
    pub repairs: Vec<Repair>,
}

/// Milestone 2 §2: which evidence the project supplied and which Oxidize made up. Today every
/// behaviour case comes from the project's own cases file and none are generated, but that
/// is a fact worth recording rather than assuming, so a report can never blur the two.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseProvenance {
    /// The project-relative cases file the behaviour cases were read from.
    pub cases_file: String,
    /// SHA-256 of that file when the attempt started.
    pub cases_sha256: String,
    /// Behaviour cases supplied by the project for this component.
    pub project_cases: usize,
    /// Cases Oxidize generated itself. Always 0 until case generation exists.
    pub generated_cases: usize,
    /// The project's own test command, run independently of the behaviour cases.
    pub independent_tests: Vec<String>,
}

/// The report's provenance paragraph: what was compared, where it came from, and its limits.
pub fn provenance_line(provenance: Option<&CaseProvenance>) -> String {
    match provenance {
        Some(p) => format!(
            "Evidence provenance: {} behaviour case(s) from the project's `{}` (sha256 `{}`), {} generated by Oxidize; independent tests: `{}`, run separately from the behaviour cases. Coverage limit: only those inputs were compared, so behaviour on inputs the cases don't represent is unverified.\n\n",
            p.project_cases,
            p.cases_file,
            &p.cases_sha256[..p.cases_sha256.len().min(16)],
            p.generated_cases,
            p.independent_tests.join(" ")
        ),
        None => {
            "Evidence provenance: not recorded (session from before provenance was captured).\n\n"
                .into()
        }
    }
}
/// The report's speed paragraph: what was measured, how, and what it does and doesn't show.
pub fn measurement_paragraph(
    measurement: Option<&Measurement>,
    legacy_speedup: Option<f64>,
    minimum: f64,
) -> String {
    let Some(m) = measurement else {
        return match legacy_speedup {
            Some(s) => format!(
                "Measured component workload speedup: **{s:.3}x** (legacy: two separate, non-interleaved runs; no uncertainty recorded). Includes Python/FFI call overhead; not a whole-application speedup.\n\n"
            ),
            None => String::new(),
        };
    };
    let per_round: Vec<String> = m
        .rounds
        .iter()
        .map(|r| {
            format!(
                "{:.3}x",
                sampling::median(&r.original_ns) / sampling::median(&r.candidate_ns)
            )
        })
        .collect();
    let policy = format!(
        "{} interleaved round(s), alternating which side runs first, each side in a fresh process with the same warm-up and fresh inputs; at least {} samples per side per round; relative MAD at most {}",
        m.rounds.len(),
        m.minimum_samples,
        m.max_relative_mad
    );
    match &m.verdict {
        Verdict::Measured {
            estimate,
            low,
            high,
        } => format!(
            "Measured component workload speedup: **{estimate:.3}x** (range {low:.3}x to {high:.3}x across rounds: {}). {policy}. Acceptance needs the slowest round to reach {minimum}x. Includes Python/FFI call overhead; not a whole-application speedup. Raw samples are in `report.json`.\n\n",
            per_round.join(", ")
        ),
        Verdict::Inconclusive { reason } => format!(
            "Speed: **no claim**. The measurement was inconclusive: {reason}. {policy}. Raw samples are in `report.json`.\n\n"
        ),
    }
}
/// The report's history paragraph: the checkpoint schema, how many events, and any upgrade gap.
pub fn history_line(state: &State) -> String {
    let upgrades: Vec<String> = state
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::SchemaUpgraded { from, to, backup } => Some(format!(
                "upgraded from schema {from} to {to} (original kept as `{backup}`); changes before that were not recorded"
            )),
            _ => None,
        })
        .collect();
    format!(
        "## History\n\nCheckpoint schema {}; {} recorded event(s), each saved atomically with the state change it describes, in `report.json` under `events`.{}\n\n",
        state.schema_version,
        state.events.len(),
        if upgrades.is_empty() {
            String::new()
        } else {
            format!(" This session was {}.", upgrades.join("; then "))
        }
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub source_fingerprint: String,
    pub source_revision: Option<String>,
    pub config: Config,
    pub current: Option<usize>,
    pub attempts: Vec<Attempt>,
    /// Every state change, saved atomically with it. Starts at the upgrade for older sessions.
    pub events: Vec<Event>,
    /// Whole-application workload runs (`oxidize workloads`), oldest first.
    #[serde(default)]
    pub workloads: Vec<WorkloadRun>,
}
pub struct Session {
    pub root: PathBuf,
    pub directory: PathBuf,
    pub state: State,
    faults: faults::Faults,
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
            history::load(&directory)?
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
            let mut state = State {
                schema_version: CHECKPOINT_SCHEMA_VERSION,
                source_fingerprint,
                source_revision: revision,
                config,
                current: None,
                attempts: vec![],
                events: vec![],
                workloads: vec![],
            };
            state.record(EventKind::SessionCreated);
            atomic_json(&state_path, &state)?;
            state
        };
        Ok(Self {
            root,
            directory,
            state,
            faults: faults::Faults::default(),
            _lock: lock,
        })
    }
    pub fn save(&self) -> Result<()> {
        let n = self.faults.next_save();
        let path = self.directory.join("state.json");
        let point = FaultPoint::BeforeSave(n);
        match self.faults.decide(&point) {
            Fault::None => {}
            Fault::Fail => return Err(InjectedFailure(point).into()),
            Fault::Crash => return Err(InjectedCrash(point).into()),
            Fault::Torn => {
                let bytes = serde_json::to_vec_pretty(&self.state)?;
                fs::write(path.with_extension("json.tmp"), &bytes[..bytes.len() / 2])?;
                return Err(InjectedCrash(point).into());
            }
        }
        atomic_json(&path, &self.state)?;
        self.faults.check(FaultPoint::AfterSave(n))
    }
    /// Install a fault injector for recovery tests (see [`Fault`]). It sees every checkpoint save
    /// and every file-writing step this session reaches from now on.
    pub fn inject_faults(&mut self, injector: impl FnMut(&FaultPoint) -> Fault + 'static) {
        self.faults.install(Box::new(injector));
    }
    /// Remove a finished attempt's Cargo build directory. It is hundreds of megabytes per
    /// attempt, isn't evidence (fingerprints and snapshots exclude `target/`), and isn't
    /// needed to run the workspace: the built extension is installed beside the Python code.
    /// Best effort: a leftover cache only costs disk space.
    fn prune_build_cache(&self, id: usize) {
        let _ = fs::remove_dir_all(self.workspace(Some(id)).join("target"));
    }
    fn step(&self, name: &'static str) -> Result<()> {
        self.faults.check(FaultPoint::Step(name))
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
        let restored = a.base;
        self.state.current = restored;
        self.state.record(EventKind::RolledBack {
            attempt: id,
            restored,
        });
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
            provenance: None,
            measurement: None,
            repairs: vec![],
        });
        self.state.record(EventKind::AttemptStarted {
            attempt: id,
            component: component_id.into(),
            base: self.state.current,
        });
        if let Err(error) = self.save() {
            self.reject(id, error)?;
        }
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
        self.state.record(EventKind::AttemptResumed { attempt: id });
        self.save()?;
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
        match self.execute_inner(id, response, trust_code, allow_provider) {
            Ok(()) => Ok(()),
            Err(error) => self.reject(id, error),
        }
    }
    /// Record attempt `id` as rejected because of `error`, and return that error.
    fn reject(&mut self, id: usize, error: anyhow::Error) -> Result<()> {
        if faults::is_crash(&error) {
            // A dead process records nothing more.
            return Err(error);
        }
        self.state.attempts[id - 1].phase = Phase::Rejected;
        self.state.attempts[id - 1].error = Some(format!("{error:#}"));
        self.state.record(EventKind::Rejected {
            attempt: id,
            error: format!("{error:#}"),
        });
        let saved = self.save();
        if saved.is_ok() {
            self.prune_build_cache(id);
        }
        if let Err(save) = saved {
            // Keep the original cause; the checkpoint on disk is the last one that saved.
            return Err(error.context(format!(
                "migration failed, and recording the rejection also failed ({save:#}); the last saved checkpoint is kept and resume can replay it"
            )));
        }
        Err(error.context(
            "migration rejected; previous accepted workspace and input project are preserved",
        ))
    }
    fn set_phase(&mut self, id: usize, phase: Phase) -> Result<()> {
        ensure!(
            !oxidize_core::process::cancelled(),
            "migration cancelled; resume can replay this checkpoint"
        );
        self.state.attempts[id - 1].phase = phase.clone();
        self.state
            .record(EventKind::PhaseChanged { attempt: id, phase });
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
    fn validation_copy(&self, source: &Path) -> Result<tempfile::TempDir> {
        let parent = self.directory.join("validation");
        fs::create_dir_all(&parent)?;
        let temporary = tempfile::Builder::new().prefix("run-").tempdir_in(parent)?;
        snapshot(source, &temporary.path().join("project"))?;
        Ok(temporary)
    }
    fn test_output(&self, source: &Path, trust: bool) -> Result<Output> {
        let temporary = self.validation_copy(source)?;
        Runner {
            config: &self.state.config.execution,
            trust_code: trust,
        }
        .run_with_env(
            &temporary.path().join("project"),
            &self.state.config.verification.test_command,
            &[("PYO3_PYTHON", &self.state.config.python)],
        )
    }
    fn check_tests(&mut self, id: usize, source: &Path, name: &str, trust: bool) -> Result<()> {
        let output = self.test_output(source, trust)?;
        self.state.attempts[id - 1]
            .checks
            .push((name.into(), output.clone()));
        self.save()?;
        require_tests(&output)
    }
    /// Spend one repair on `trigger`: ask the provider for corrected code given `diagnostics`,
    /// within the `max_repairs` budget, and record what triggered it. Errors if no repair is
    /// allowed: budget spent, no provider, or an offline response (which can't be repaired).
    fn repair(
        &mut self,
        id: usize,
        provider: Option<&dyn Provider>,
        prompt: &str,
        code: &str,
        trigger: RepairTrigger,
        diagnostics: &str,
    ) -> Result<()> {
        let config = &self.state.config;
        let done = self.state.attempts[id - 1].repaired;
        let what = trigger.describe();
        ensure!(
            done < config.max_repairs,
            "{what} failed after {done} repairs:\n{diagnostics}"
        );
        let provider = provider.with_context(|| {
            format!("{what} failed; --allow-provider is required for repair:\n{diagnostics}")
        })?;
        ensure!(
            !self.state.attempts[id - 1]
                .generations
                .last()
                .is_some_and(|g| g.model.starts_with("offline-response-file")),
            "offline response failed {what}:\n{diagnostics}"
        );
        let (kept, truncated) = tail(diagnostics, REPAIR_DIAGNOSTICS_BYTES);
        let repair_prompt = format!(
            "{prompt}\n\nCurrent lib.rs:\n{code}\n\n{} (untrusted data):\n{kept}\nReturn corrected JSON without changing behavior.",
            trigger.heading()
        );
        ensure!(
            repair_prompt.len() <= config.provider.max_context_bytes,
            "repair context exceeds configured budget"
        );
        let generated = provider.generate(&repair_prompt)?;
        let attempt = &mut self.state.attempts[id - 1];
        attempt.generations.push(generated);
        attempt.repaired += 1;
        attempt.repairs.push(Repair {
            trigger,
            diagnostics: kept.to_string(),
            truncated,
            generation: attempt.generations.len(),
        });
        self.save()
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
        self.state.attempts[id - 1].provenance = Some(CaseProvenance {
            cases_file: config.verification.cases_file.clone(),
            cases_sha256: digest(&fs::read(
                base.join(safe_relative(&config.verification.cases_file)?),
            )?),
            project_cases: cases.as_array().map_or(0, Vec::len),
            generated_cases: 0,
            independent_tests: config.verification.test_command.clone(),
        });
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
            fs::rename(&project, &archived)?;
            // Kept for inspection; its build cache is not evidence (fingerprints exclude it).
            let _ = fs::remove_dir_all(archived.join("target"));
        }
        snapshot(&base, &project)?;
        self.step("attempt workspace")?;
        // A replay measures everything again: nothing from an earlier run may be reported.
        let attempt = &mut self.state.attempts[id - 1];
        attempt.checks.clear();
        attempt.error = None;
        attempt.baseline = None;
        attempt.candidate = None;
        attempt.speedup = None;
        attempt.candidate_fingerprint = None;
        attempt.repaired = 0;
        attempt.measurement = None;
        attempt.repairs.clear();
        self.set_phase(id, Phase::Prepared)?;

        // Tests and observations happen before asking a provider for implementation.
        self.check_tests(id, &base, "baseline tests", trust)?;
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
            // Resuming with the same response replays it; only new code is a new generation.
            let replay = self.state.attempts[id - 1]
                .generations
                .last()
                .is_some_and(|g| g.code.rust == generated.code.rust && g.model == generated.model);
            if !replay {
                self.state.attempts[id - 1].generations.push(generated);
                self.save()?;
            }
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
        let library = if cfg!(target_os = "macos") {
            format!("lib{module}.dylib")
        } else if cfg!(target_os = "linux") {
            format!("lib{module}.so")
        } else {
            bail!("native artifact installation currently supports Linux and macOS");
        };
        let component_file = safe_relative(&component.path)?;
        let mut seen = std::collections::BTreeSet::new();
        // Build, install, integrate and test; a compiler or integrated-test failure spends one
        // repair from the shared `max_repairs` budget and goes round again.
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
            self.step("native source")?;
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
            if !diagnostics.is_empty() {
                self.repair(
                    id,
                    provider.as_deref(),
                    &prompt,
                    &code,
                    RepairTrigger::Compiler,
                    &diagnostics,
                )?;
                continue;
            }
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
            self.step("install extension")?;
            fs::copy(
                project.join("target/release").join(&library),
                project.join(format!("{module}{suffix}")),
            )
            .context("install compiled Python extension")?;
            // Integration rewrites by source span, so always start from the original file.
            fs::copy(base.join(&component_file), project.join(&component_file))?;
            integrate(&project, &component, &module)?;
            self.step("integrate")?;
            self.set_phase(id, Phase::Verifying)?;
            let output = self.test_output(&project, trust)?;
            self.state.attempts[id - 1]
                .checks
                .push(("integrated tests".into(), output.clone()));
            self.save()?;
            match require_tests(&output) {
                Ok(()) => break,
                Err(failure) => self.repair(
                    id,
                    provider.as_deref(),
                    &prompt,
                    &code,
                    RepairTrigger::IntegratedTests,
                    &format!("{failure:#}"),
                )?,
            }
        }
        let candidate = self.evidence(&project, &component, &cases, trust)?;
        self.state.attempts[id - 1].candidate = Some(candidate.clone());
        self.save()?;
        if let Some(reason) =
            differential_mismatch(&baseline, &candidate, &config.verification.comparison)
        {
            bail!("differential mismatch: {reason}");
        }
        // Behaviour matches; only now is speed worth measuring, both sides interleaved.
        let successful = successful_cases(&cases, &baseline)?;
        let measurement = self.measure(&base, &project, &component, &successful, trust)?;
        let verdict = measurement.verdict.clone();
        self.state.attempts[id - 1].speedup = match verdict {
            Verdict::Measured { estimate, .. } => Some(estimate),
            Verdict::Inconclusive { .. } => None,
        };
        self.state.attempts[id - 1].measurement = Some(measurement);
        self.save()?;
        let minimum = config.performance.minimum_speedup;
        match verdict {
            Verdict::Inconclusive { reason } => bail!(
                "no speed claim can be made ({reason}); acceptance requires a conclusive measurement"
            ),
            Verdict::Measured { low, .. } => ensure!(
                low >= minimum,
                "slowest round's measured speedup {low:.3}x is below required {minimum:.3}x"
            ),
        }
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
        self.state.record(EventKind::Accepted {
            attempt: id,
            previous,
        });
        if let Err(error) = self.save() {
            self.state.current = previous;
            self.state.events.pop();
            return Err(error);
        }
        self.prune_build_cache(id);
        Ok(())
    }
    /// Time original and candidate in interleaved rounds (see [`sampling`]).
    fn measure(
        &self,
        original: &Path,
        candidate: &Path,
        c: &Component,
        successful: &Value,
        trust: bool,
    ) -> Result<Measurement> {
        let performance = &self.state.config.performance;
        let mut rounds = vec![];
        for round in 0..performance.rounds {
            let order = sampling::order(round);
            let (mut original_ns, mut candidate_ns) = (vec![], vec![]);
            for side in order {
                let root = match side {
                    Side::Original => original,
                    Side::Candidate => candidate,
                };
                let timed: Benchmark = self.harness(root, c, successful, "benchmark", trust)?;
                match side {
                    Side::Original => original_ns = timed.samples_ns,
                    Side::Candidate => candidate_ns = timed.samples_ns,
                }
            }
            rounds.push(Round {
                order,
                original_ns,
                candidate_ns,
            });
        }
        let verdict = sampling::decide(
            &rounds,
            performance.iterations,
            performance.max_relative_mad,
        );
        Ok(Measurement {
            rounds,
            max_relative_mad: performance.max_relative_mad,
            minimum_samples: performance.iterations,
            verdict,
        })
    }
    fn harness<T: serde::de::DeserializeOwned>(
        &self,
        root: &Path,
        c: &Component,
        cases: &Value,
        mode: &str,
        trust: bool,
    ) -> Result<T> {
        let temporary = self.validation_copy(root)?;
        let module = c
            .path
            .strip_suffix(".py")
            .context("not Python")?
            .replace('/', ".");
        let argv = vec![
            self.state.config.python.clone(),
            "-c".into(),
            include_str!("harness.py").into(),
            mode.into(),
            module,
            c.name.clone(),
            serde_json::to_string(cases)?,
            serde_json::to_string(&self.state.config.performance)?,
            serde_json::to_string(&self.state.config.verification.file_effect_paths)?,
        ];
        let output = Runner {
            config: &self.state.config.execution,
            trust_code: trust,
        }
        .run(&temporary.path().join("project"), &argv)?;
        output.require_success(&format!("{mode} workload"))?;
        serde_json::from_str(&output.stdout).context("decode workload evidence")
    }
    fn evidence(&self, root: &Path, c: &Component, cases: &Value, trust: bool) -> Result<Evidence> {
        let observed: Observations = self.harness(root, c, cases, "observe", trust)?;
        let cases = cases
            .as_array()
            .context("behavior cases must be an array")?;
        ensure!(
            observed.observation_schema_version == 2 && observed.observations.len() == cases.len(),
            "invalid behavior observation schema or case count"
        );
        ensure!(
            observed.observations.iter().all(has_argument_evidence),
            "behavior observations omitted argument mutation evidence"
        );
        ensure!(
            observed.file_effect_paths == self.state.config.verification.file_effect_paths
                && observed
                    .observations
                    .iter()
                    .all(|o| has_file_effect_evidence(o, &observed.file_effect_paths)),
            "behavior observations omitted or invalidated configured file-effect evidence"
        );
        let successful: Vec<_> = cases
            .iter()
            .zip(&observed.observations)
            .filter(|(_, observation)| observation.get("return").is_some())
            .map(|(case, _)| case.clone())
            .collect();
        ensure!(
            !successful.is_empty(),
            "at least one successful workload is required for benchmarking"
        );
        // A second process and fresh copy prevent tests/observations from seeding timing state.
        let mut benchmark: Benchmark =
            self.harness(root, c, &Value::Array(successful), "benchmark", trust)?;
        let performance = &self.state.config.performance;
        benchmark.context = Some(MeasurementContext {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            isolation: oxidize_core::process::isolation_name(&self.state.config.execution, trust),
            rustc: Runner { config: &self.state.config.execution, trust_code: trust }
                .run(root, &["rustc".into(), "--version".into()])
                .ok()
                .filter(|o| o.success())
                .map(|o| o.stdout.trim().to_string()),
            command: vec![self.state.config.python.clone(), "harness:benchmark".into(), c.id.clone()],
            iterations: performance.iterations,
            warmup: performance.warmup,
            calls_per_sample: performance.calls_per_sample,
            minimum_speedup: performance.minimum_speedup,
            scope: "component workload; includes Python/FFI call overhead; not a whole-application figure".into(),
        });
        let evidence = Evidence {
            observation_schema_version: observed.observation_schema_version,
            file_effect_paths: observed.file_effect_paths,
            observations: observed.observations,
            import_stdout: observed.import_stdout,
            import_stderr: observed.import_stderr,
            benchmark,
        };
        ensure!(
            evidence.benchmark.median_ns.is_finite() && evidence.benchmark.median_ns > 0.0,
            "invalid benchmark timing"
        );
        Ok(evidence)
    }
    pub fn verify(&self, trust: bool) -> Result<()> {
        self.assert_source_unchanged()?;
        self.assert_base_unchanged()?;
        let out = self.test_output(&self.current_workspace(), trust)?;
        require_tests(&out)
    }
    /// Time every configured whole-application workload on the original and the accepted
    /// workspace, interleaved, and keep the result in the checkpoint.
    pub fn run_workloads(&mut self, trust: bool) -> Result<&WorkloadRun> {
        self.assert_source_unchanged()?;
        self.assert_base_unchanged()?;
        let against = self.state.current.context(
            "no accepted workspace: whole-application workloads compare the original with an accepted migration",
        )?;
        let performance = self.state.config.performance.clone();
        ensure!(
            !performance.workloads.is_empty(),
            "no [[performance.workloads]] configured"
        );
        let (original, candidate) = (self.workspace(None), self.current_workspace());
        let mut results = vec![];
        for w in &performance.workloads {
            let mut rounds = vec![];
            for round in 0..performance.rounds {
                let order = sampling::order(round);
                let (mut o, mut c) = (None, None);
                for side in order {
                    let root = match side {
                        Side::Original => &original,
                        Side::Candidate => &candidate,
                    };
                    let temporary = self.validation_copy(root)?;
                    let argv = vec![
                        self.state.config.python.clone(),
                        "-I".into(),
                        "-c".into(),
                        workloads::WRAPPER.into(),
                        serde_json::to_string(&w.command)?,
                        performance.warmup.to_string(),
                        performance.iterations.to_string(),
                    ];
                    let output = Runner {
                        config: &self.state.config.execution,
                        trust_code: trust,
                    }
                    .run(&temporary.path().join("project"), &argv)?;
                    let which = match side {
                        Side::Original => "original",
                        Side::Candidate => "accepted",
                    };
                    output.require_success(&format!(
                        "workload `{}` on the {which} workspace",
                        w.name
                    ))?;
                    let measured: WorkloadSide = serde_json::from_str(&output.stdout)
                        .context("decode workload measurement")?;
                    match side {
                        Side::Original => o = Some(measured),
                        Side::Candidate => c = Some(measured),
                    }
                }
                rounds.push(WorkloadRound {
                    order,
                    original: o.context("original side missing")?,
                    candidate: c.context("accepted side missing")?,
                });
            }
            results.push(workloads::summarize(
                &w.name,
                &w.command,
                rounds,
                performance.iterations,
                performance.max_relative_mad,
            ));
        }
        self.state.workloads.push(WorkloadRun {
            against,
            isolation: oxidize_core::process::isolation_name(&self.state.config.execution, trust),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            results,
        });
        self.state.record(EventKind::WorkloadsMeasured {
            attempt: against,
            workloads: performance.workloads.len(),
        });
        self.save()?;
        Ok(self.state.workloads.last().expect("just pushed"))
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
        report += &format!(
            "## Comparison policy\n\nFrozen session policy (omitted options default to exact comparison):\n\n```json\n{}\n```\n\nTolerances apply only to returned floats, recursively through supported containers; line-ending normalization applies only to call stdout/stderr. Argument mutations, errors, return types, dictionary keys/order, import output and other observation fields remain exact. Raw observations are retained unchanged.\n\n",
            serde_json::to_string_pretty(&self.state.config.verification.comparison)?
        );
        report += &format!(
            "## File-effect scope\n\nRequested project-relative regular files: `{}`. Each configured file's presence, byte length and SHA256 are captured before/after each behavior call, including exceptions, and compared exactly. No file paths configured means no file-effect claim. Limits: 32 paths, 1 MiB per file. Raw contents are not retained. Tests/timing and import-time effects, unlisted paths, transient writes restored between snapshots, metadata/permissions, directory trees and external effects remain unverified.\n\n",
            serde_json::to_string(&self.state.config.verification.file_effect_paths)?
        );
        report += &history_line(&self.state);
        report += &workloads::report_section(&self.state.workloads);
        for a in &self.state.attempts {
            report += &format!(
                "## Attempt {}: `{}`\n\nStatus: {:?}\n\nRepairs: {} (budget `max_repairs` = {}).\n\n",
                a.id, a.component, a.phase, a.repaired, self.state.config.max_repairs
            );
            for (i, r) in a.repairs.iter().enumerate() {
                report += &format!(
                    "- Repair {}: {} failed; the provider was given {} bytes of diagnostics{} and returned generation {}.\n",
                    i + 1,
                    r.trigger.describe(),
                    r.diagnostics.len(),
                    if r.truncated {
                        " (the tail of longer output)"
                    } else {
                        ""
                    },
                    r.generation
                );
            }
            if !a.repairs.is_empty() {
                report += "\n";
            }
            report += &provenance_line(a.provenance.as_ref());
            report += &measurement_paragraph(
                a.measurement.as_ref(),
                a.speedup,
                self.state.config.performance.minimum_speedup,
            );
            for (label, e) in [("Original", &a.baseline), ("Candidate", &a.candidate)] {
                if let Some(e) = e {
                    report += &format!(
                        "{label}: {} cases; median {:.0} ns/sample; median absolute deviation {:.0} ns; {} samples.\n\n",
                        e.observations.len(),
                        e.benchmark.median_ns,
                        e.benchmark.mad_ns,
                        e.benchmark.samples_ns.len()
                    );
                    report += &measurement_line(&e.benchmark);
                    let mutation = if e.observation_schema_version == 2
                        && e.observations.iter().all(has_argument_evidence)
                    {
                        "captured positional/keyword argument mutations, including error paths; tests, behavior capture and timing use separate disposable copies/processes"
                    } else {
                        "legacy evidence: argument mutation and separate validation copies were not recorded"
                    };
                    report += &format!(
                        "Observation schema {}: {mutation}.\n\n",
                        e.observation_schema_version
                    );
                    let file_evidence = if e.file_effect_paths.is_empty() {
                        "not captured (default/legacy evidence)".into()
                    } else if e
                        .observations
                        .iter()
                        .all(|o| has_file_effect_evidence(o, &e.file_effect_paths))
                    {
                        format!(
                            "captured exactly for {}",
                            serde_json::to_string(&e.file_effect_paths)?
                        )
                    } else {
                        "incomplete; cannot establish configured file-effect equivalence".into()
                    };
                    report += &format!("File-effect evidence: {file_evidence}.\n\n");
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
        report += "## Evidence limits\n\nOnly configured tests and JSON cases were checked, under the comparison policy and file-effect scope above. Opt-in tolerances/normalization weaken equality within their stated scope; they are not proof of exact equivalence. New schema-2 observations include supported argument mutations; legacy evidence does not. Unconfigured/transient/import-time file effects, external services, object identity/aliasing, cyclic objects, concurrency, non-finite floats, CPU, allocations, peak memory, and bridge-only overhead were not measured. Timing is sequential and subject to system noise. No claim of universal equivalence or full Rustification.\n";
        let path = self.directory.join("report.md");
        fs::write(&path, report)?;
        Ok(path)
    }
}

/// Diagnostics sent to the provider (and kept) for one repair: the last 16 KiB, where test
/// runners and compilers put their summary.
const REPAIR_DIAGNOSTICS_BYTES: usize = 16 * 1024;

/// The last `limit` bytes of `text` at a character boundary, and whether anything was cut.
fn tail(text: &str, limit: usize) -> (&str, bool) {
    if text.len() <= limit {
        return (text, false);
    }
    let mut start = text.len() - limit;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    (&text[start..], true)
}

/// What a repair was asked to fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairTrigger {
    /// `cargo fmt`, `check`, `clippy` or `build` failed.
    Compiler,
    /// The extension built, but the project's own tests failed with it installed.
    IntegratedTests,
}
impl RepairTrigger {
    fn describe(self) -> &'static str {
        match self {
            Self::Compiler => "compiler checks",
            Self::IntegratedTests => "integrated tests",
        }
    }
    fn heading(self) -> &'static str {
        match self {
            Self::Compiler => "Compiler diagnostics",
            Self::IntegratedTests => {
                "The extension compiled, but the project's own tests failed with it installed. Test output"
            }
        }
    }
}

/// One repair: what failed, the diagnostics the provider was given, and the generation
/// (counting from 1) it produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repair {
    pub trigger: RepairTrigger,
    pub diagnostics: String,
    /// Whether `diagnostics` is the tail of longer output.
    pub truncated: bool,
    pub generation: usize,
}

/// The cases that returned (rather than raised) in the original: the timed workload.
fn successful_cases(cases: &Value, baseline: &Evidence) -> Result<Value> {
    let cases = cases
        .as_array()
        .context("behavior cases must be an array")?;
    ensure!(
        cases.len() == baseline.observations.len(),
        "baseline observations don't match the cases"
    );
    let successful: Vec<_> = cases
        .iter()
        .zip(&baseline.observations)
        .filter(|(_, o)| o.get("return").is_some())
        .map(|(case, _)| case.clone())
        .collect();
    ensure!(
        !successful.is_empty(),
        "at least one successful workload is required for benchmarking"
    );
    Ok(Value::Array(successful))
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
    fn the_report_says_where_cases_came_from_and_what_they_cannot_show() {
        let provenance = CaseProvenance {
            cases_file: "cases.json".into(),
            cases_sha256: "0123456789abcdef0123".into(),
            project_cases: 11,
            generated_cases: 0,
            independent_tests: vec!["python3".into(), "-m".into(), "unittest".into()],
        };
        let line = provenance_line(Some(&provenance));
        for part in [
            "11 behaviour case(s) from the project's `cases.json`",
            "sha256 `0123456789abcdef`",
            "0 generated by Oxidize",
            "`python3 -m unittest`, run separately",
            "is unverified",
        ] {
            assert!(line.contains(part), "missing `{part}` in: {line}");
        }
        assert!(provenance_line(None).contains("not recorded"));
    }

    #[test]
    fn older_benchmarks_without_a_context_still_load_and_say_so() {
        let legacy: Benchmark = serde_json::from_str(
            r#"{"samples_ns":[1.0],"median_ns":1.0,"mad_ns":0.0,"successful_cases":1,"calls_per_sample":3}"#,
        )
        .unwrap();
        assert!(legacy.context.is_none() && legacy.interpreter.is_none());
        assert!(measurement_line(&legacy).contains("not recorded"));
    }

    #[test]
    fn a_measurement_states_where_how_and_what_it_covers() {
        let benchmark = Benchmark {
            samples_ns: vec![1.0],
            median_ns: 1.0,
            mad_ns: 0.0,
            successful_cases: 1,
            calls_per_sample: 3,
            interpreter: Some(Interpreter {
                implementation: "cpython".into(),
                version: "3.14.4".into(),
                executable: "/usr/bin/python3".into(),
            }),
            context: Some(MeasurementContext {
                os: "linux".into(),
                arch: "x86_64".into(),
                isolation: "bubblewrap".into(),
                rustc: Some("rustc 1.100.0 (0ef1bc386 2026-11-09)".into()),
                command: vec![
                    "/usr/bin/python3".into(),
                    "harness:benchmark".into(),
                    "compute.py:f".into(),
                ],
                iterations: 7,
                warmup: 2,
                calls_per_sample: 3,
                minimum_speedup: 1.0,
                scope: "component workload".into(),
            }),
        };
        let line = measurement_line(&benchmark);
        for part in [
            "cpython 3.14.4",
            "rustc 1.100.0",
            "/usr/bin/python3",
            "linux/x86_64",
            "`bubblewrap`",
            "7 samples after 2 warm-up",
            "threshold 1x",
            "component workload",
        ] {
            assert!(line.contains(part), "missing `{part}` in: {line}");
        }
        // Round-trips through checkpoint JSON unchanged.
        let back: Benchmark =
            serde_json::from_str(&serde_json::to_string(&benchmark).unwrap()).unwrap();
        assert_eq!(back.context, benchmark.context);
    }

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
