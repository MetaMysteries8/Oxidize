//! `[crosscheck]` configuration: rust-anything's switch by default, and bounded, safe values.
use oxidize_core::{Config, WorkloadConfig};

#[test]
fn defaults_follow_rust_anything_and_round_trip() {
    let config = Config::default();
    assert_eq!(config.crosscheck.switch, "RUST_ANYTHING_MODE");
    assert_eq!(
        (
            config.crosscheck.original.as_str(),
            config.crosscheck.candidate.as_str()
        ),
        ("old", "new")
    );
    assert!(config.crosscheck.compare_stderr && config.crosscheck.timing);
    let text = r#"
[crosscheck]
switch = "USE_PORT"
original = "0"
candidate = "1"
file_paths = ["out/report.json"]
include = ["node_modules"]
normalize_line_endings = true

[[crosscheck.commands]]
name = "cli"
command = ["node", "app.js"]
"#;
    let config: Config = toml::from_str(text).unwrap();
    config.validate().unwrap();
    assert_eq!(config.crosscheck.commands[0].command, ["node", "app.js"]);
    assert!(config.crosscheck.normalize_line_endings);
}

#[test]
fn unsafe_or_meaningless_settings_are_rejected() {
    let check = |edit: &dyn Fn(&mut Config)| {
        let mut config = Config::default();
        edit(&mut config);
        config.validate()
    };
    for switch in ["", "PATH", "PYO3_PYTHON", "lower", "1ST", "HAS-DASH"] {
        assert!(
            check(&|c| c.crosscheck.switch = switch.into()).is_err(),
            "{switch}"
        );
    }
    assert!(check(&|c| c.crosscheck.candidate = "old".into()).is_err());
    assert!(check(&|c| c.crosscheck.candidate = "a\0b".into()).is_err());
    for path in [
        "../outside",
        "/etc/passwd",
        ".env",
        ".ssh",
        "a/key.pem",
        ".git",
    ] {
        assert!(
            check(&|c| c.crosscheck.include = vec![path.into()]).is_err(),
            "include {path}"
        );
        assert!(
            check(&|c| c.crosscheck.file_paths = vec![path.into()]).is_err(),
            "file {path}"
        );
    }
    assert!(check(&|c| c.crosscheck.include = vec!["node_modules".into(); 2]).is_err());
    assert!(
        check(&|c| c.crosscheck.commands = vec![WorkloadConfig {
            name: "x".into(),
            command: vec![],
        }])
        .is_err()
    );
    assert!(check(&|c| c.crosscheck.include = vec![".venv".into()]).is_ok());
}
