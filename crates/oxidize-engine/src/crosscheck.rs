//! `oxidize crosscheck`: a second opinion on a port the application switches on and off.
//!
//! Some tools leave the original code in place and choose between it and a Rust port at run
//! time, through an environment variable. rust-anything does this with
//! `RUST_ANYTHING_MODE=old|new`, for every language it supports. Oxidize does not need to
//! understand that language to check the result: it runs the application's own commands once
//! with the switch on the original and once on the port, each in a fresh disposable copy of
//! the project (sandboxed unless `--trust-code`), and compares the exit code, stdout, stderr
//! and the listed files they leave behind. Only commands that agree are timed, in the same
//! interleaved rounds and noise policy as `oxidize workloads`, so a speed claim is never made
//! for a port that behaves differently.
//!
//! The input project is never modified; every run gets a fresh copy at the same path,
//! `.oxidize/crosscheck/work/project`, so neither side sees a different working directory.
use crate::sampling::{self, Side};
use crate::workloads::{self, WorkloadResult, WorkloadRound, WorkloadSide};
use anyhow::{Context, Result, ensure};
use oxidize_core::{
    Config, CrosscheckConfig, digest, files,
    process::{Output, Runner},
    snapshot,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const SCHEMA: &str = "oxidize/crosscheck@1";

/// File extensions of built native modules that ports load at run time. A rust-anything crate
/// keeps its built libraries in its own `target/release`, which snapshots otherwise skip.
const NATIVE_EXTENSIONS: &[&str] = &["so", "dylib", "dll", "node", "pyd", "wasm"];
/// Bounds on what `include` may copy into one disposable copy.
const INCLUDE_MAX_FILES: usize = 300_000;
const INCLUDE_MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// One listed file after a command ran.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileState {
    pub path: String,
    /// `missing`, `file`, or `not a regular file`.
    pub kind: String,
    pub length: Option<u64>,
    pub sha256: Option<String>,
}
impl FileState {
    fn describe(&self) -> String {
        match (&self.length, &self.sha256) {
            (Some(length), Some(sha)) => format!("{length} bytes, sha256 {}", &sha[..12]),
            _ => self.kind.clone(),
        }
    }
}

/// What one command did with the switch in one position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub files: Vec<FileState>,
    pub elapsed_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub name: String,
    pub command: Vec<String>,
    pub original: Observation,
    pub candidate: Observation,
    /// Every way the two runs differ; empty means they agree.
    pub differences: Vec<String>,
    /// Interleaved timing, present only for commands that agree and exit 0.
    pub timing: Option<WorkloadResult>,
    /// Why there is no timing, when there is none.
    pub not_timed: Option<String>,
}

/// The comparison and timing policy in force, kept with the evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub compare_stderr: bool,
    pub normalize_line_endings: bool,
    pub timing: bool,
    pub rounds: usize,
    pub warmup: usize,
    pub iterations: usize,
    pub max_relative_mad: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crosscheck {
    pub schema: String,
    pub switch: String,
    pub original: String,
    pub candidate: String,
    pub isolation: String,
    pub os: String,
    pub arch: String,
    pub policy: Policy,
    /// Folders holding a `rust-anything.json`, relative to the project root.
    pub ports: Vec<String>,
    /// Built native modules copied from those folders' `target/release`.
    pub native_modules: Vec<String>,
    /// Symlinks inside `include` paths, which copies do not carry.
    pub skipped_symlinks: Vec<String>,
    pub commands: Vec<CommandResult>,
    /// Every command agreed.
    pub agree: bool,
}

/// Every difference between two observations under `policy`.
pub fn compare(original: &Observation, candidate: &Observation, policy: &Policy) -> Vec<String> {
    let mut differences = vec![];
    if original.code != candidate.code {
        differences.push(format!(
            "exit code: original {}, port {}",
            code_text(original.code),
            code_text(candidate.code)
        ));
    }
    let streams: &[(&str, &String, &String)] = if policy.compare_stderr {
        &[
            ("stdout", &original.stdout, &candidate.stdout),
            ("stderr", &original.stderr, &candidate.stderr),
        ]
    } else {
        &[("stdout", &original.stdout, &candidate.stdout)]
    };
    for (name, a, b) in streams {
        let (a, b) = if policy.normalize_line_endings {
            (a.replace("\r\n", "\n"), b.replace("\r\n", "\n"))
        } else {
            (a.to_string(), b.to_string())
        };
        if let Some(at) = first_difference(&a, &b) {
            differences.push(format!(
                "{name} differs from byte {at}: original {:?}, port {:?}",
                excerpt(&a, at),
                excerpt(&b, at)
            ));
        }
    }
    for (a, b) in original.files.iter().zip(&candidate.files) {
        if a != b {
            differences.push(format!(
                "file `{}`: original {}, port {}",
                a.path,
                a.describe(),
                b.describe()
            ));
        }
    }
    differences
}

fn code_text(code: Option<i32>) -> String {
    code.map_or_else(|| "none (killed or timed out)".into(), |c| c.to_string())
}

fn first_difference(a: &str, b: &str) -> Option<usize> {
    if a == b {
        return None;
    }
    Some(
        a.bytes()
            .zip(b.bytes())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len())),
    )
}

/// Up to 60 bytes of `text` from `at`, on character boundaries.
fn excerpt(text: &str, at: usize) -> String {
    let mut start = at.min(text.len());
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + 60).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[start..end].to_string()
}

/// What a disposable copy carried beyond the normal snapshot.
#[derive(Default)]
struct CopyNotes {
    ports: Vec<String>,
    native_modules: Vec<String>,
    skipped_symlinks: Vec<String>,
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Snapshot `root` into `destination`, then add the ports' built native modules and the
/// configured `include` paths.
fn prepare_copy(root: &Path, destination: &Path, include: &[String]) -> Result<CopyNotes> {
    snapshot(root, destination)?;
    let mut notes = CopyNotes::default();
    for relative in files(root)? {
        if relative
            .file_name()
            .is_none_or(|n| n != "rust-anything.json")
        {
            continue;
        }
        let folder = relative.parent().unwrap_or(Path::new("")).to_path_buf();
        notes.ports.push(match slash(&folder) {
            f if f.is_empty() => ".".into(),
            f => f,
        });
        let release = folder.join("target").join("release");
        let Ok(entries) = fs::read_dir(root.join(&release)) else {
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let native = Path::new(&entry.file_name())
                .extension()
                .is_some_and(|e| NATIVE_EXTENSIONS.contains(&e.to_string_lossy().as_ref()));
            if !native || !entry.file_type()?.is_file() {
                continue;
            }
            let relative = release.join(entry.file_name());
            let target = destination.join(&relative);
            fs::create_dir_all(target.parent().context("missing parent")?)?;
            fs::copy(entry.path(), target)?;
            notes.native_modules.push(slash(&relative));
        }
    }
    let (mut count, mut bytes) = (0usize, 0u64);
    for value in include {
        let relative = PathBuf::from(value);
        let source = root.join(&relative);
        let metadata = fs::symlink_metadata(&source)
            .with_context(|| format!("crosscheck.include path {value} does not exist"))?;
        copy_tree(
            &source,
            &destination.join(&relative),
            &relative,
            &metadata,
            &mut notes.skipped_symlinks,
            &mut count,
            &mut bytes,
        )?;
    }
    notes.ports.sort();
    notes.native_modules.sort();
    notes.skipped_symlinks.sort();
    Ok(notes)
}

fn copy_tree(
    source: &Path,
    target: &Path,
    relative: &Path,
    metadata: &fs::Metadata,
    skipped: &mut Vec<String>,
    count: &mut usize,
    bytes: &mut u64,
) -> Result<()> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        skipped.push(slash(relative));
    } else if file_type.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name();
            copy_tree(
                &entry.path(),
                &target.join(&name),
                &relative.join(&name),
                &fs::symlink_metadata(entry.path())?,
                skipped,
                count,
                bytes,
            )?;
        }
    } else if file_type.is_file() {
        *count += 1;
        *bytes += metadata.len();
        ensure!(
            *count <= INCLUDE_MAX_FILES && *bytes <= INCLUDE_MAX_BYTES,
            "crosscheck.include exceeds {INCLUDE_MAX_FILES} files or 4 GiB"
        );
        fs::create_dir_all(target.parent().context("missing parent")?)?;
        fs::copy(source, target)?;
    }
    Ok(())
}

fn file_states(project: &Path, paths: &[String]) -> Result<Vec<FileState>> {
    paths
        .iter()
        .map(|path| {
            let full = project.join(path);
            let state = |kind: &str| FileState {
                path: path.clone(),
                kind: kind.into(),
                length: None,
                sha256: None,
            };
            Ok(match fs::symlink_metadata(&full) {
                Err(_) => state("missing"),
                Ok(m) if !m.file_type().is_file() => state("not a regular file"),
                Ok(_) => {
                    let content = fs::read(&full)?;
                    FileState {
                        path: path.clone(),
                        kind: "file".into(),
                        length: Some(content.len() as u64),
                        sha256: Some(digest(&content)),
                    }
                }
            })
        })
        .collect()
}

/// Runs crosschecks for one project. Copies go under `<project>/.oxidize/crosscheck/`.
struct Checker<'a> {
    root: PathBuf,
    config: &'a Config,
    settings: &'a CrosscheckConfig,
    trust: bool,
    scratch: PathBuf,
}
impl Checker<'_> {
    /// A fresh copy of the project, always at the same path: both sides must see the same
    /// working directory, or anything that prints a path would differ for no real reason.
    fn fresh(&self, notes: &mut Option<CopyNotes>) -> Result<PathBuf> {
        let work = self.scratch.join("work");
        if work.exists() {
            fs::remove_dir_all(&work).context("remove the previous crosscheck copy")?;
        }
        let project = work.join("project");
        let copied = prepare_copy(&self.root, &project, &self.settings.include)?;
        notes.get_or_insert(copied);
        Ok(project)
    }
    fn value(&self, side: Side) -> &str {
        match side {
            Side::Original => &self.settings.original,
            Side::Candidate => &self.settings.candidate,
        }
    }
    fn run(&self, project: &Path, argv: &[String], side: Side) -> Result<Output> {
        Runner {
            config: &self.config.execution,
            trust_code: self.trust,
        }
        .run_with_env(project, argv, &[(&self.settings.switch, self.value(side))])
    }
    fn observe(
        &self,
        command: &[String],
        side: Side,
        notes: &mut Option<CopyNotes>,
    ) -> Result<Observation> {
        let project = self.fresh(notes)?;
        let output = self.run(&project, command, side)?;
        Ok(Observation {
            code: output.code,
            stdout: output.stdout,
            stderr: output.stderr,
            files: file_states(&project, &self.settings.file_paths)?,
            elapsed_ms: output.elapsed_ms,
        })
    }
    /// Interleaved rounds, each side in a fresh copy every time, like `oxidize workloads`.
    fn time(
        &self,
        name: &str,
        command: &[String],
        notes: &mut Option<CopyNotes>,
    ) -> Result<WorkloadResult> {
        let performance = &self.config.performance;
        let argv = vec![
            self.config.python.clone(),
            "-I".into(),
            "-c".into(),
            workloads::WRAPPER.into(),
            serde_json::to_string(command)?,
            performance.warmup.to_string(),
            performance.iterations.to_string(),
            self.config.execution.output_limit_bytes.to_string(),
        ];
        let mut rounds = vec![];
        for round in 0..performance.rounds {
            let order = sampling::order(round);
            let (mut o, mut c) = (None, None);
            for side in order {
                let project = self.fresh(notes)?;
                let output = self.run(&project, &argv, side)?;
                output.require_success(&format!(
                    "timing `{name}` with {}={}",
                    self.settings.switch,
                    self.value(side)
                ))?;
                let measured: WorkloadSide =
                    serde_json::from_str(&output.stdout).context("decode crosscheck timing")?;
                match side {
                    Side::Original => o = Some(measured),
                    Side::Candidate => c = Some(measured),
                }
            }
            rounds.push(WorkloadRound {
                order,
                original: o.context("original side missing")?,
                candidate: c.context("port side missing")?,
            });
        }
        Ok(workloads::summarize(
            name,
            command,
            rounds,
            performance.iterations,
            performance.max_relative_mad,
        ))
    }
}

/// The per-project crosscheck lock, released explicitly on drop.
struct Lock(fs::File);
impl Lock {
    fn acquire(path: &Path) -> Result<Self> {
        let file = fs::File::create(path)?;
        fs2::FileExt::try_lock_exclusive(&file)
            .context("another crosscheck is running for this project")?;
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        // Closing our descriptor is not enough while a child forked by another thread still
        // holds the same open-file description between fork and exec (see `Session`).
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

/// Crosscheck every configured command in `root` and save the evidence as
/// `.oxidize/crosscheck.json` and `.oxidize/crosscheck.md`.
pub fn run(root: &Path, config: &Config, trust: bool) -> Result<Crosscheck> {
    config.validate()?;
    let settings = &config.crosscheck;
    ensure!(
        !settings.commands.is_empty(),
        "nothing to crosscheck: pass a command after `--`, or configure [[crosscheck.commands]]"
    );
    let root = root.canonicalize().context("project directory")?;
    let directory = root.join(".oxidize");
    ensure!(!directory.is_symlink(), ".oxidize must not be a symlink");
    let scratch = directory.join("crosscheck");
    fs::create_dir_all(&scratch)?;
    // One crosscheck per project at a time: they share the copy's path.
    let _lock = Lock::acquire(&scratch.join("lock"))?;
    let checker = Checker {
        root: root.clone(),
        config,
        settings,
        trust,
        scratch,
    };
    let policy = Policy {
        compare_stderr: settings.compare_stderr,
        normalize_line_endings: settings.normalize_line_endings,
        timing: settings.timing,
        rounds: config.performance.rounds,
        warmup: config.performance.warmup,
        iterations: config.performance.iterations,
        max_relative_mad: config.performance.max_relative_mad,
    };
    let mut notes = None;
    let mut commands = vec![];
    for c in &settings.commands {
        let original = checker.observe(&c.command, Side::Original, &mut notes)?;
        let candidate = checker.observe(&c.command, Side::Candidate, &mut notes)?;
        let differences = compare(&original, &candidate, &policy);
        let not_timed = if !differences.is_empty() {
            Some("the two runs behave differently, so no speed claim is made".into())
        } else if original.code != Some(0) {
            Some(format!(
                "the command exits {} in both runs; only commands that succeed are timed",
                code_text(original.code)
            ))
        } else if !settings.timing {
            Some("timing is turned off (crosscheck.timing = false or --no-timing)".into())
        } else {
            None
        };
        let timing = match not_timed {
            None => Some(checker.time(&c.name, &c.command, &mut notes)?),
            Some(_) => None,
        };
        commands.push(CommandResult {
            name: c.name.clone(),
            command: c.command.clone(),
            original,
            candidate,
            differences,
            timing,
            not_timed,
        });
    }
    let work = checker.scratch.join("work");
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    let notes = notes.unwrap_or_default();
    let result = Crosscheck {
        schema: SCHEMA.into(),
        switch: settings.switch.clone(),
        original: settings.original.clone(),
        candidate: settings.candidate.clone(),
        isolation: oxidize_core::process::isolation_name(&config.execution, trust),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        policy,
        ports: notes.ports,
        native_modules: notes.native_modules,
        skipped_symlinks: notes.skipped_symlinks,
        agree: commands.iter().all(|c| c.differences.is_empty()),
        commands,
    };
    oxidize_core::atomic_json(&directory.join("crosscheck.json"), &result)?;
    oxidize_core::atomic_write(
        &directory.join("crosscheck.md"),
        markdown(&result).as_bytes(),
    )?;
    Ok(result)
}

fn verdict_text(v: &sampling::Verdict) -> String {
    match v {
        sampling::Verdict::Measured {
            estimate,
            low,
            high,
        } => format!("**{estimate:.3}x** (range {low:.3}x to {high:.3}x)"),
        sampling::Verdict::Inconclusive { reason } => format!("no claim ({reason})"),
    }
}

/// The Markdown report.
pub fn markdown(c: &Crosscheck) -> String {
    let mut text = format!(
        "# Oxidize crosscheck\n\n**{}.** Original: `{}={}`. Port: `{}={}`. Isolation `{}` on {}/{}.\n\n",
        if c.agree {
            "Every command behaved the same with the port on"
        } else {
            "The port behaves differently"
        },
        c.switch,
        c.original,
        c.switch,
        c.candidate,
        c.isolation,
        c.os,
        c.arch
    );
    text += &format!(
        "Compared: exit code, stdout{}{}{}. Each run used a fresh disposable copy of the project; the project itself was not modified.\n\n",
        if c.policy.compare_stderr {
            ", stderr"
        } else {
            " (stderr not compared)"
        },
        if c.policy.normalize_line_endings {
            " (CRLF and LF treated as the same)"
        } else {
            ""
        },
        match c.commands.first().map(|r| r.original.files.len()) {
            Some(n) if n > 0 => format!(", and {n} listed file(s) by presence, length and SHA-256"),
            _ => String::new(),
        }
    );
    if c.ports.is_empty() {
        text += "No `rust-anything.json` was found in the project, so Oxidize cannot say which ports the switch controls.\n\n";
    } else {
        text += &format!(
            "rust-anything ports found: {}. Native modules copied from their `target/release`: {}.\n\n",
            c.ports
                .iter()
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", "),
            if c.native_modules.is_empty() {
                "none".into()
            } else {
                c.native_modules
                    .iter()
                    .map(|p| format!("`{p}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
    }
    if !c.skipped_symlinks.is_empty() {
        text += &format!(
            "Symlinks skipped while copying `include` paths: {}.\n\n",
            c.skipped_symlinks.join(", ")
        );
    }
    text += "## Commands\n\n";
    for r in &c.commands {
        text += &format!(
            "### `{}`: {}\n\n`{}`\n\n",
            r.name,
            if r.differences.is_empty() {
                "same behaviour"
            } else {
                "DIFFERENT"
            },
            r.command.join(" ")
        );
        for d in &r.differences {
            text += &format!("- {d}\n");
        }
        if !r.differences.is_empty() {
            text += "\n";
        }
        match (&r.timing, &r.not_timed) {
            (Some(t), _) => {
                let kib =
                    |k: Option<u64>| k.map_or_else(|| "unsupported".into(), |k| format!("{k} KiB"));
                text += &format!(
                    "Timing ({} interleaved rounds; ratios are original / port, so above 1x means the port was faster): wall time {}; CPU time {}; peak RSS original {}, port {}.{}\n\n",
                    t.rounds.len(),
                    verdict_text(&t.wall),
                    verdict_text(&t.cpu),
                    kib(t.original_peak_rss_kib),
                    kib(t.candidate_peak_rss_kib),
                    if t.unsupported.is_empty() {
                        String::new()
                    } else {
                        format!(" Not measured / caveats: {}.", t.unsupported.join("; "))
                    }
                );
            }
            (None, Some(reason)) => text += &format!("Not timed: {reason}.\n\n"),
            (None, None) => {}
        }
    }
    text + "Limits: a crosscheck sees only what these commands do, with these inputs. It cannot tell whether the switch reached every port, and it does not inspect network traffic, unlisted files, or timing-dependent behaviour.\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(code: i32, stdout: &str) -> Observation {
        Observation {
            code: Some(code),
            stdout: stdout.into(),
            stderr: String::new(),
            files: vec![],
            elapsed_ms: 1.0,
        }
    }
    fn policy() -> Policy {
        Policy {
            compare_stderr: true,
            normalize_line_endings: false,
            timing: true,
            rounds: 3,
            warmup: 2,
            iterations: 7,
            max_relative_mad: 0.25,
        }
    }

    #[test]
    fn identical_runs_agree() {
        let a = observation(0, "same\n");
        assert!(compare(&a, &a.clone(), &policy()).is_empty());
    }

    #[test]
    fn exit_code_and_stdout_differences_are_named_with_where_they_start() {
        let differences = compare(
            &observation(0, "total: 10\n"),
            &observation(1, "total: 11\n"),
            &policy(),
        );
        assert_eq!(differences.len(), 2, "{differences:?}");
        assert!(differences[0].contains("original 0, port 1"));
        assert!(
            differences[1].contains("stdout differs from byte 8"),
            "{differences:?}"
        );
    }

    #[test]
    fn line_endings_and_stderr_follow_the_policy() {
        let mut a = observation(0, "a\r\nb\r\n");
        let mut b = observation(0, "a\nb\n");
        a.stderr = "warning\n".into();
        assert_eq!(compare(&a, &b, &policy()).len(), 2);
        let mut loose = policy();
        loose.normalize_line_endings = true;
        loose.compare_stderr = false;
        assert!(compare(&a, &b, &loose).is_empty());
        b.stdout = "a\nc\n".into();
        assert_eq!(compare(&a, &b, &loose).len(), 1, "content still counts");
    }

    #[test]
    fn the_lock_is_released_even_with_an_inherited_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let lock = Lock::acquire(&path).unwrap();
        // A duplicate models a descriptor a forked child holds before exec closes it.
        let inherited = lock.0.try_clone().unwrap();
        assert!(Lock::acquire(&path).is_err());
        drop(lock);
        let again = Lock::acquire(&path).unwrap();
        drop(inherited);
        drop(again);
        assert!(Lock::acquire(&path).is_ok());
    }

    #[test]
    fn excerpts_stay_on_character_boundaries() {
        assert_eq!(excerpt("ab😀cd", 3), "😀cd");
        assert_eq!(first_difference("abc", "abcd"), Some(3));
        assert_eq!(first_difference("same", "same"), None);
    }

    #[test]
    fn file_differences_are_reported() {
        let file = |sha: &str| FileState {
            path: "out.txt".into(),
            kind: "file".into(),
            length: Some(3),
            sha256: Some(sha.repeat(64)),
        };
        let mut a = observation(0, "");
        let mut b = observation(0, "");
        a.files = vec![file("a")];
        b.files = vec![file("b")];
        let differences = compare(&a, &b, &policy());
        assert_eq!(differences.len(), 1);
        assert!(
            differences[0].starts_with("file `out.txt`"),
            "{differences:?}"
        );
    }
}
