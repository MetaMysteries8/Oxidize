use std::{fs, path::Path, process::Command};

/// Milestone 2 §4: source code never goes to a model provider unless the user said so.
/// Unix-only: the fixture's tests run `python3`, which Windows CI runners don't provide.
#[cfg(unix)]
#[test]
fn migrate_refuses_to_send_source_without_explicit_provider_authorization() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("demo");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-cli");
    fs::create_dir(&project).unwrap();
    for entry in fs::read_dir(&fixture).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            fs::copy(entry.path(), project.join(entry.file_name())).unwrap();
        }
    }
    let oxidize = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_oxidize"))
            .args(args)
            .arg(&project)
            .output()
            .unwrap()
    };
    assert!(oxidize(&["analyze"]).status.success());
    assert!(oxidize(&["plan"]).status.success());
    // Neither --response nor --allow-provider: nothing may be sent anywhere.
    let out = Command::new(env!("CARGO_BIN_EXE_oxidize"))
        .arg("migrate")
        .arg(&project)
        .args(["--component", "compute.py:count_primes", "--trust-code"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("requires --allow-provider or an explicit --response file"),
        "{stderr}"
    );
}

#[test]
fn cli_analyzes_and_plans_without_executing_source() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.py"), "raise RuntimeError('must never execute')\n\ndef leaf(x):\n return x + 1\n\ndef caller(x):\n return leaf(x)\n").unwrap();
    fs::write(dir.path().join("utility.js"), "export const x = 1;").unwrap();
    fs::write(dir.path().join("main.go"), "package main").unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    for subcommand in ["analyze", "plan"] {
        let out = Command::new(env!("CARGO_BIN_EXE_oxidize"))
            .arg(subcommand)
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("main.py:leaf"));
    }
    let analysis: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join(".oxidize/analysis.json")).unwrap())
            .unwrap();
    for language in ["Python", "JavaScript", "Go", "Rust"] {
        assert_eq!(analysis["languages_by_files"][language], 1);
    }
    assert_eq!(analysis["edges"][0]["kind"], "call");
}

#[test]
fn portable_skill_is_valid_and_references_exist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills/oxidize");
    let text = fs::read_to_string(root.join("SKILL.md")).unwrap();
    assert!(text.starts_with("---\nname: oxidize\ndescription:"));
    assert!(text.contains("CLI-assisted") && text.contains("Agent-native"));
    for name in [
        "migration-methodology",
        "verification",
        "performance",
        "interoperability",
    ] {
        assert!(root.join(format!("references/{name}.md")).is_file());
    }
}

#[test]
fn config_init_does_not_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let first = Command::new(env!("CARGO_BIN_EXE_oxidize"))
        .arg("init")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(first.status.success());
    let old = fs::read(dir.path().join("oxidize.toml")).unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_oxidize"))
        .arg("init")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert_eq!(old, fs::read(dir.path().join("oxidize.toml")).unwrap());
}

/// `oxidize crosscheck`: the command after `--` runs both ways; a difference exits 2, so CI
/// and coding agents can gate on it.
#[test]
fn crosscheck_exits_2_when_the_port_behaves_differently() {
    let python = if Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        "python3"
    } else {
        "python"
    };
    let dir = tempfile::tempdir().unwrap();
    let crosscheck = |app: &str| {
        fs::write(dir.path().join("app.py"), app).unwrap();
        Command::new(env!("CARGO_BIN_EXE_oxidize"))
            .arg("crosscheck")
            .arg(dir.path())
            .args(["--no-timing", "--trust-code", "--", python, "app.py"])
            .output()
            .unwrap()
    };
    let same = crosscheck("print('same')\n");
    let stdout = String::from_utf8_lossy(&same.stdout);
    assert_eq!(same.status.code(), Some(0), "{stdout}");
    assert!(
        stdout.contains("Every command behaved the same"),
        "{stdout}"
    );
    let different = crosscheck("import os\nprint(os.environ['RUST_ANYTHING_MODE'])\n");
    let stdout = String::from_utf8_lossy(&different.stdout);
    assert_eq!(different.status.code(), Some(2), "{stdout}");
    assert!(stdout.contains("The port behaves differently"), "{stdout}");
    assert!(dir.path().join(".oxidize/crosscheck.json").is_file());
}
