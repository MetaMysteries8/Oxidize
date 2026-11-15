use std::{fs, path::Path, process::Command};

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
