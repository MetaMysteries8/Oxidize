use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use oxidize_core::{Config, atomic_json};
use oxidize_engine::Session;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "oxidize",
    version,
    about = "Incremental Python-to-Rust migration with measured acceptance"
)]
struct Cli {
    /// Permit the extra host paths listed in the reviewed sandbox configuration.
    #[arg(long, global = true)]
    allow_host_paths: bool,
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Create a configuration without overwriting an existing one.
    Init {
        #[arg(default_value = ".")]
        project: PathBuf,
    },
    /// Inspect source using Python AST parsing; repository code is not executed.
    Analyze {
        #[arg(default_value = ".")]
        project: PathBuf,
    },
    /// Rank Python functions and persist a dependency-aware roadmap.
    Plan {
        #[arg(default_value = ".")]
        project: PathBuf,
    },
    /// Migrate one component in a separate workspace; never rewrite the input project.
    Migrate {
        #[arg(default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        component: String,
        /// Explicit pre-generated JSON response for offline/agent-assisted operation.
        #[arg(long)]
        response: Option<PathBuf>,
        /// Permit selected source context to be sent to the configured provider.
        #[arg(long)]
        allow_provider: bool,
        /// Execute project/model code without OS isolation. Only for trusted code.
        #[arg(long)]
        trust_code: bool,
    },
    /// Replay an interrupted or rejected attempt using saved generated code.
    Resume {
        #[arg(default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        response: Option<PathBuf>,
        #[arg(long)]
        allow_provider: bool,
        #[arg(long)]
        trust_code: bool,
    },
    /// Run configured independent tests against the current workspace.
    Verify {
        #[arg(default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        trust_code: bool,
    },
    /// Measure supplied component workloads, including the Python/FFI call boundary.
    Benchmark {
        #[arg(default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        component: String,
        #[arg(long)]
        trust_code: bool,
    },
    /// Time the configured whole-application workloads on the original and the accepted
    /// workspace, interleaved; CPU time and peak RSS where the platform reports them.
    Workloads {
        #[arg(default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        trust_code: bool,
    },
    Status {
        #[arg(default_value = ".")]
        project: PathBuf,
    },
    /// Restore the previous accepted workspace pointer; keep all evidence and files.
    Rollback {
        #[arg(default_value = ".")]
        project: PathBuf,
    },
    /// Write truthful Markdown and JSON evidence reports.
    Report {
        #[arg(default_value = ".")]
        project: PathBuf,
    },
    Providers,
    /// Second opinion on a port the app switches on and off (rust-anything's
    /// RUST_ANYTHING_MODE=old|new by default): run each command both ways in disposable
    /// copies, compare exit code, stdout, stderr and listed files, and time only what agrees.
    /// Exits 2 when the two runs differ.
    Crosscheck {
        #[arg(default_value = ".")]
        project: PathBuf,
        /// Environment variable that switches the port [default: RUST_ANYTHING_MODE].
        #[arg(long)]
        switch: Option<String>,
        /// The switch's value for the original [default: old].
        #[arg(long)]
        original: Option<String>,
        /// The switch's value for the port [default: new].
        #[arg(long)]
        candidate: Option<String>,
        /// A file the command writes, compared by presence, length and SHA-256 (repeatable).
        #[arg(long = "file")]
        files: Vec<String>,
        /// A path snapshots skip that the command needs, such as node_modules (repeatable).
        #[arg(long)]
        include: Vec<String>,
        /// Compare stdout and stderr with CRLF and LF treated as the same.
        #[arg(long)]
        normalize_line_endings: bool,
        /// Do not compare stderr.
        #[arg(long)]
        ignore_stderr: bool,
        /// Compare behaviour only; skip the interleaved timing.
        #[arg(long)]
        no_timing: bool,
        /// Run the commands without OS isolation. Only for code you trust.
        #[arg(long)]
        trust_code: bool,
        /// The command to run, after `--`; replaces any [[crosscheck.commands]].
        #[arg(last = true)]
        command: Vec<String>,
    },
}
fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    ctrlc::set_handler(oxidize_core::process::cancel).context("install cancellation handler")?;
    let cli = Cli::parse();
    if cli.allow_host_paths {
        oxidize_core::process::authorize_host_mounts();
    }
    match cli.command {
        Action::Init { project } => {
            ensure!(project.is_dir(), "project directory does not exist");
            Config::write_new(&project)?;
            println!("Created {}", project.join("oxidize.toml").display());
        }
        Action::Analyze { project } | Action::Plan { project } => {
            let root = project.canonicalize()?;
            let config = if root.join("oxidize.toml").exists() {
                Config::load(&root)?
            } else {
                Config::default()
            };
            let analysis = oxidize_analysis::analyze(&root, &config)?;
            let plan = oxidize_analysis::plan(&analysis);
            let directory = root.join(".oxidize");
            ensure!(!directory.is_symlink(), ".oxidize must not be a symlink");
            fs::create_dir_all(&directory)?;
            atomic_json(&directory.join("analysis.json"), &analysis)?;
            atomic_json(&directory.join("plan.json"), &plan)?;
            let markdown = oxidize_analysis::markdown(&analysis, &plan);
            fs::write(directory.join("plan.md"), &markdown)?;
            print!("{markdown}");
            if !analysis.errors.is_empty() {
                eprintln!(
                    "{} source files could not be parsed; see analysis.json",
                    analysis.errors.len()
                );
            }
        }
        Action::Migrate {
            project,
            component,
            response,
            allow_provider,
            trust_code,
        } => {
            let mut session = Session::open(&project)?;
            let result =
                session.migrate(&component, response.as_deref(), trust_code, allow_provider);
            let report = session.report()?;
            let id = result?;
            println!(
                "Accepted attempt {id}. Workspace: {}\nReport: {}",
                session.current_workspace().display(),
                report.display()
            );
        }
        Action::Resume {
            project,
            response,
            allow_provider,
            trust_code,
        } => {
            let mut session = Session::open(&project)?;
            let result = session.resume(response.as_deref(), trust_code, allow_provider);
            let report = session.report()?;
            let id = result?;
            println!(
                "Accepted resumed attempt {id}. Workspace: {}\nReport: {}",
                session.current_workspace().display(),
                report.display()
            );
        }
        Action::Verify {
            project,
            trust_code,
        } => {
            let session = Session::open(&project)?;
            session.verify(trust_code)?;
            println!(
                "Configured tests passed using a disposable copy of {}",
                session.current_workspace().display()
            );
        }
        Action::Benchmark {
            project,
            component,
            trust_code,
        } => {
            let session = Session::open(&project)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&session.benchmark(&component, trust_code)?)?
            );
        }
        Action::Workloads {
            project,
            trust_code,
        } => {
            let mut session = Session::open(&project)?;
            let run = session.run_workloads(trust_code)?;
            println!(
                "{}",
                oxidize_engine::workloads::report_section(std::slice::from_ref(run))
            );
        }
        Action::Status { project } => {
            let session = Session::open(&project)?;
            println!("{}", serde_json::to_string_pretty(&session.state)?);
            println!(
                "Current workspace: {}",
                session.current_workspace().display()
            );
        }
        Action::Rollback { project } => {
            let mut session = Session::open(&project)?;
            session.rollback()?;
            session.report()?;
            println!("Restored {}", session.current_workspace().display());
        }
        Action::Report { project } => {
            let session = Session::open(&project)?;
            println!("{}", session.report()?.display());
        }
        Action::Crosscheck {
            project,
            switch,
            original,
            candidate,
            files,
            include,
            normalize_line_endings,
            ignore_stderr,
            no_timing,
            trust_code,
            command,
        } => {
            ensure!(project.is_dir(), "project directory does not exist");
            let mut config = if project.join("oxidize.toml").exists() {
                Config::load(&project)?
            } else {
                Config::default()
            };
            let c = &mut config.crosscheck;
            if let Some(switch) = switch {
                c.switch = switch;
            }
            if let Some(original) = original {
                c.original = original;
            }
            if let Some(candidate) = candidate {
                c.candidate = candidate;
            }
            c.file_paths.extend(files);
            c.include.extend(include);
            c.normalize_line_endings |= normalize_line_endings;
            c.compare_stderr &= !ignore_stderr;
            c.timing &= !no_timing;
            if !command.is_empty() {
                c.commands = vec![oxidize_core::WorkloadConfig {
                    name: "command".into(),
                    command,
                }];
            }
            let result = oxidize_engine::crosscheck::run(&project, &config, trust_code)?;
            print!("{}", oxidize_engine::crosscheck::markdown(&result));
            println!(
                "Evidence: {}",
                project.join(".oxidize").join("crosscheck.json").display()
            );
            if !result.agree {
                std::process::exit(2);
            }
        }
        Action::Providers => println!(
            "openai-compatible: cloud HTTPS or local loopback HTTP /v1/chat/completions\nresponse-file: explicit offline implementation input; does not call an AI model\nOther provider protocols are not implemented."
        ),
    }
    Ok(())
}
