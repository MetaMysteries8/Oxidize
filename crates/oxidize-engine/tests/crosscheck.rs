//! `oxidize crosscheck`: a second opinion on a port switched by an environment variable.
//!
//! The fixtures are small Python programs that read `RUST_ANYTHING_MODE` the way a
//! rust-anything shadow module does, so each test can make the two sides agree or differ in
//! exactly one way. They run with `--trust-code` so they pass on every CI platform; the
//! sandboxed path is the same `Runner` the rest of the engine uses.
use oxidize_core::{Config, WorkloadConfig, fingerprint};
use oxidize_engine::crosscheck::{self, Crosscheck};
use std::{fs, path::Path, process::Command, sync::OnceLock};

/// `python3` where it runs, else `python` (some Windows runners only have the latter).
fn python() -> &'static str {
    static PYTHON: OnceLock<&str> = OnceLock::new();
    PYTHON.get_or_init(|| {
        let works = |name: &str| {
            Command::new(name)
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
        };
        if works("python3") {
            "python3"
        } else {
            "python"
        }
    })
}

fn project(app: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("app.py"), app).unwrap();
    dir
}

fn config(command: &[&str]) -> Config {
    let mut config = Config {
        python: python().into(),
        ..Config::default()
    };
    config.performance.rounds = 2;
    config.performance.warmup = 0;
    config.performance.iterations = 3;
    // Process start-up on a busy CI runner is noisy; these tests check the plumbing, not speed.
    config.performance.max_relative_mad = 10.0;
    config.crosscheck.commands = vec![WorkloadConfig {
        name: "app".into(),
        command: std::iter::once(python())
            .chain(command.iter().copied())
            .map(String::from)
            .collect(),
    }];
    config
}

fn run(dir: &Path, config: &Config) -> Crosscheck {
    crosscheck::run(dir, config, true).unwrap()
}

#[test]
fn a_port_that_behaves_the_same_agrees_and_is_timed() {
    let dir = project(
        "import os\nmode = os.environ['RUST_ANYTHING_MODE']\nopen('out.txt', 'w').write('42')\nprint(sum(range(1000)))\n",
    );
    let before = fingerprint(dir.path()).unwrap();
    let mut config = config(&["app.py"]);
    config.crosscheck.file_paths = vec!["out.txt".into()];
    let result = run(dir.path(), &config);
    assert!(result.agree, "{result:#?}");
    let command = &result.commands[0];
    assert!(command.differences.is_empty());
    assert_eq!(command.original.stdout.trim(), "499500");
    assert_eq!(command.original.files[0].kind, "file");
    assert_eq!(command.original.files[0].length, Some(2));
    let timing = command
        .timing
        .as_ref()
        .expect("agreeing commands are timed");
    assert_eq!(timing.rounds.len(), 2);
    assert!(command.not_timed.is_none());
    // The project was never touched; the evidence went to .oxidize/.
    assert_eq!(fingerprint(dir.path()).unwrap(), before);
    assert!(!dir.path().join("out.txt").exists());
    let saved: Crosscheck =
        serde_json::from_slice(&fs::read(dir.path().join(".oxidize/crosscheck.json")).unwrap())
            .unwrap();
    // Floats may differ in the last digit after a JSON round trip; the evidence must not.
    assert_eq!(saved.schema, "oxidize/crosscheck@1");
    assert!(saved.agree);
    // Raw output bytes stay in memory; the evidence keeps their SHA-256.
    let (kept, ran) = (&saved.commands[0].original, &result.commands[0].original);
    assert_eq!(kept.stdout_sha256, ran.stdout_sha256);
    assert_eq!(kept.stdout_sha256, oxidize_core::digest(&ran.stdout_bytes));
    assert_eq!(kept.files, ran.files);
    assert_eq!(
        saved.commands[0].timing.as_ref().unwrap().rounds,
        timing.rounds
    );
    let report = fs::read_to_string(dir.path().join(".oxidize/crosscheck.md")).unwrap();
    assert!(
        report.contains("Every command behaved the same"),
        "{report}"
    );
    assert!(report.contains("interleaved rounds"), "{report}");
}

#[test]
fn different_stdout_is_a_disagreement_and_is_never_timed() {
    let dir = project("import os\nprint('mode', os.environ['RUST_ANYTHING_MODE'])\n");
    let result = run(dir.path(), &config(&["app.py"]));
    assert!(!result.agree);
    let command = &result.commands[0];
    assert_eq!(command.original.stdout.trim(), "mode old");
    assert_eq!(command.candidate.stdout.trim(), "mode new");
    assert!(
        command.differences[0].starts_with("stdout differs from byte 5"),
        "{:?}",
        command.differences
    );
    assert!(command.timing.is_none());
    assert!(
        command
            .not_timed
            .as_ref()
            .unwrap()
            .contains("no speed claim")
    );
    assert!(crosscheck::markdown(&result).contains("DIFFERENT"));
}

#[test]
fn a_different_file_is_a_disagreement_even_when_output_matches() {
    let dir = project(
        "import os\nopen('out.txt', 'w').write(os.environ['RUST_ANYTHING_MODE'])\nprint('done')\n",
    );
    let mut config = config(&["app.py"]);
    config.crosscheck.file_paths = vec!["out.txt".into(), "never-written.txt".into()];
    let result = run(dir.path(), &config);
    assert!(!result.agree);
    let differences = &result.commands[0].differences;
    assert_eq!(differences.len(), 1, "{differences:?}");
    assert!(
        differences[0].starts_with("file `out.txt`"),
        "{differences:?}"
    );
    assert_eq!(result.commands[0].original.files[1].kind, "missing");
}

#[test]
fn custom_switches_and_stderr_policy_are_honoured() {
    let dir = project(
        "import os, sys\nsys.stderr.write('using ' + os.environ['USE_PORT'] + '\\n')\nprint('same')\n",
    );
    let mut config = config(&["app.py"]);
    config.crosscheck.switch = "USE_PORT".into();
    config.crosscheck.original = "0".into();
    config.crosscheck.candidate = "1".into();
    config.crosscheck.compare_stderr = false;
    config.crosscheck.timing = false;
    let result = run(dir.path(), &config);
    assert!(result.agree, "{result:#?}");
    let command = &result.commands[0];
    assert!(command.original.stderr.contains("using 0"));
    assert!(command.candidate.stderr.contains("using 1"));
    assert!(command.timing.is_none());
    assert!(command.not_timed.as_ref().unwrap().contains("turned off"));
}

#[test]
fn rust_anything_ports_and_their_built_modules_come_along() {
    let dir = project(
        "import os\nprint(os.path.exists('rust/slug-rs/target/release/slug_rs.dll'), os.path.exists('rust/slug-rs/target/release/deps/junk.rlib'))\n",
    );
    let crate_dir = dir.path().join("rust/slug-rs");
    fs::create_dir_all(crate_dir.join("target/release/deps")).unwrap();
    fs::write(crate_dir.join("rust-anything.json"), "{}").unwrap();
    fs::write(crate_dir.join("target/release/slug_rs.dll"), b"native").unwrap();
    fs::write(crate_dir.join("target/release/deps/junk.rlib"), b"junk").unwrap();
    let result = run(dir.path(), &config(&["app.py"]));
    assert_eq!(result.ports, ["rust/slug-rs"]);
    assert_eq!(
        result.native_modules,
        ["rust/slug-rs/target/release/slug_rs.dll"]
    );
    assert_eq!(result.commands[0].original.stdout.trim(), "True False");
    assert!(crosscheck::markdown(&result).contains("rust-anything ports found: `rust/slug-rs`"));
}

#[test]
fn included_dependencies_are_copied_and_a_failing_command_is_not_timed() {
    let dir = project("print(open('node_modules/dep/index.txt').read())\n");
    fs::create_dir_all(dir.path().join("node_modules/dep")).unwrap();
    fs::write(dir.path().join("node_modules/dep/index.txt"), "dependency").unwrap();
    // Without the include, both sides fail the same way: they agree, but nothing is timed.
    let mut config = config(&["app.py"]);
    let result = run(dir.path(), &config);
    assert!(result.agree);
    assert_ne!(result.commands[0].original.code, Some(0));
    assert!(
        result.commands[0]
            .not_timed
            .as_ref()
            .unwrap()
            .contains("only commands that succeed are timed")
    );
    config.crosscheck.include = vec!["node_modules".into()];
    config.crosscheck.timing = false;
    let result = run(dir.path(), &config);
    assert_eq!(result.commands[0].original.code, Some(0));
    assert_eq!(result.commands[0].original.stdout.trim(), "dependency");
}

#[test]
fn nothing_to_run_is_an_error() {
    let dir = project("");
    let error = crosscheck::run(dir.path(), &Config::default(), true).unwrap_err();
    assert!(
        error.to_string().contains("nothing to crosscheck"),
        "{error}"
    );
}

#[test]
fn credentials_inside_included_paths_are_never_copied() {
    let dir = project(
        "import os\nprint(sorted(os.listdir('node_modules/pkg')), os.path.exists('node_modules/.env'))\n",
    );
    let pkg = dir.path().join("node_modules/pkg");
    fs::create_dir_all(pkg.join(".git")).unwrap();
    for name in ["index.js", "private.key", "cert.pem", ".env.local"] {
        fs::write(pkg.join(name), "x").unwrap();
    }
    fs::write(dir.path().join("node_modules/.env"), "SECRET=1").unwrap();
    let mut config = config(&["app.py"]);
    config.crosscheck.include = vec!["node_modules".into()];
    config.crosscheck.timing = false;
    let result = run(dir.path(), &config);
    assert_eq!(
        result.commands[0].original.stdout.trim(),
        "['index.js'] False",
        "{:?}",
        result.commands[0].original.stderr
    );
    assert_eq!(
        result.skipped_credentials,
        [
            "node_modules/.env",
            "node_modules/pkg/.env.local",
            "node_modules/pkg/.git",
            "node_modules/pkg/cert.pem",
            "node_modules/pkg/private.key",
        ]
    );
    assert!(crosscheck::markdown(&result).contains("never copied"));
}

#[test]
fn every_timed_invocation_starts_from_a_fresh_copy() {
    // Exclusive creation fails if anything ran before in the same copy.
    let dir = project("open('out.txt', 'x').write('once')\nprint('ok')\n");
    let mut config = config(&["app.py"]);
    config.performance.warmup = 1;
    let result = run(dir.path(), &config);
    assert!(result.agree, "{result:#?}");
    let timing = result.commands[0].timing.as_ref().expect("timed");
    for round in &timing.rounds {
        // Warm-ups are not samples: 3 timed invocations per side per round.
        assert_eq!(round.original.wall_ns.len(), 3);
        assert_eq!(round.candidate.wall_ns.len(), 3);
    }
}
