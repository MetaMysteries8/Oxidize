use oxidize_core::{ComparisonPolicy, digest};
use oxidize_engine::{Evidence, equivalent, equivalent_with_policy};
use serde_json::{Value, json};

fn evidence() -> Evidence {
    serde_json::from_value(json!({
        "observation_schema_version": 2,
        "file_effect_paths": ["artifact.bin"],
        "observations": [{
            "return": ["int", 1], "stdout": "", "stderr": "",
            "arguments_after": {"args": ["list", []], "kwargs": ["dict", []]},
            "file_effects": {"artifact.bin": {
                "before": null, "after": {"size_bytes": 5, "sha256": digest(b"first")}
            }}
        }],
        "import_stdout": "", "import_stderr": "",
        "benchmark": {"samples_ns": [1, 2, 3], "median_ns": 2,
            "mad_ns": 1, "successful_cases": 1, "calls_per_sample": 1}
    }))
    .unwrap()
}

#[test]
fn file_effects_compare_exact_presence_size_and_bytes() {
    let original = evidence();
    assert!(equivalent(&original, &original));
    for after in [
        Value::Null,
        json!({"size_bytes": 6, "sha256": digest(b"first")}),
        json!({"size_bytes": 5, "sha256": digest(b"wrong")}),
    ] {
        let mut candidate = original.clone();
        candidate.observations[0]["file_effects"]["artifact.bin"]["after"] = after;
        assert!(!equivalent(&original, &candidate));
        assert!(!equivalent_with_policy(
            &original,
            &candidate,
            &ComparisonPolicy {
                normalize_output_line_endings: true,
                ..ComparisonPolicy::default()
            }
        ));
    }
    let mut candidate = original.clone();
    candidate.observations[0]["file_effects"]["artifact.bin"]["before"] =
        json!({"size_bytes": 0, "sha256": digest(b"")});
    assert!(!equivalent(&original, &candidate));
}

#[test]
fn missing_or_malformed_file_evidence_never_proves_effect_equivalence() {
    for field in ["before", "after"] {
        let mut broken = evidence();
        broken.observations[0]["file_effects"]["artifact.bin"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(!equivalent(&broken, &broken));
    }
    for after in [
        json!({"size_bytes": 0, "sha256": "not a hash"}),
        json!({"size_bytes": 1048577, "sha256": digest(b"")}),
        json!({"sha256": digest(b"")}),
    ] {
        let mut broken = evidence();
        broken.observations[0]["file_effects"]["artifact.bin"]["after"] = after;
        assert!(!equivalent(&broken, &broken));
    }
    let mut missing = evidence();
    missing.observations[0]
        .as_object_mut()
        .unwrap()
        .remove("file_effects");
    assert!(!equivalent(&missing, &missing));
    let original = evidence();
    let mut legacy = serde_json::to_value(&original).unwrap();
    legacy.as_object_mut().unwrap().remove("file_effect_paths");
    let legacy: Evidence = serde_json::from_value(legacy).unwrap();
    assert!(!equivalent(&original, &legacy));
}

#[test]
fn reports_distinguish_requested_scope_from_legacy_uncaptured_effects() {
    use oxidize_engine::Session;
    use std::{fs, path::Path};

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-file-effects");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    oxidize_core::snapshot(&source, &root).unwrap();
    let mut session = Session::open(&root).unwrap();
    let captured = evidence();
    let mut legacy = serde_json::to_value(&captured).unwrap();
    legacy.as_object_mut().unwrap().remove("file_effect_paths");
    legacy["observations"][0]
        .as_object_mut()
        .unwrap()
        .remove("file_effects");
    session.state.attempts.push(
        serde_json::from_value(json!({
            "id": 1, "component": "effects.py:record", "base": null, "phase": "rejected",
            "source_fingerprint": session.state.source_fingerprint,
            "candidate_fingerprint": null, "generations": [], "checks": [],
            "baseline": captured, "candidate": legacy, "speedup": null,
            "error": "configured file-effect evidence unavailable", "repaired": 0
        }))
        .unwrap(),
    );
    let report = fs::read_to_string(session.report().unwrap()).unwrap();
    assert!(report.contains("File-effect scope"));
    assert!(report.contains("File-effect evidence: captured exactly"));
    assert!(report.contains("File-effect evidence: not captured (default/legacy evidence)"));
    let json: Value =
        serde_json::from_slice(&fs::read(session.directory.join("report.json")).unwrap()).unwrap();
    assert_eq!(
        json["attempts"][0]["baseline"]["file_effect_paths"],
        json!(["artifact.bin"])
    );
    assert!(json["attempts"][0]["baseline"]["observations"][0]["file_effects"]["artifact.bin"]["before"].is_null());
    assert_eq!(
        json["config"]["verification"]["file_effect_paths"],
        json!(["artifact.bin", "untouched.bin"])
    );
}

#[test]
#[ignore = "requires a Linux host with working Bubblewrap and /usr/bin/python3"]
fn sandboxed_file_effect_fixture_detects_mismatch_despite_passing_tests() {
    use oxidize_engine::Session;
    use std::{fs, path::Path};

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-file-effects");
    let dir =
        tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")).unwrap();
    let original_path = dir.path().join("original");
    let candidate_path = dir.path().join("candidate");
    oxidize_core::snapshot(&source, &original_path).unwrap();
    oxidize_core::snapshot(&source, &candidate_path).unwrap();
    let code = fs::read_to_string(candidate_path.join("effects.py")).unwrap();
    fs::write(
        candidate_path.join("effects.py"),
        code.replace("first", "wrong"),
    )
    .unwrap();
    let original_session = Session::open(&original_path).unwrap();
    let candidate_session = Session::open(&candidate_path).unwrap();
    original_session.verify(false).unwrap();
    candidate_session.verify(false).unwrap();
    let original = original_session
        .benchmark("effects.py:record", false)
        .unwrap();
    let candidate = candidate_session
        .benchmark("effects.py:record", false)
        .unwrap();
    assert_eq!(original.observations.len(), 5);
    assert!(!equivalent(&original, &candidate));
    let effects = |case: usize| &original.observations[case]["file_effects"]["artifact.bin"];
    assert!(effects(0)["before"].is_null());
    assert_eq!(effects(0)["after"]["size_bytes"], 7);
    assert_eq!(effects(0)["after"]["sha256"], digest(b"first\x00\xff"));
    assert_eq!(effects(1)["before"], effects(0)["after"]);
    assert_eq!(effects(1)["after"]["sha256"], digest(b"second\r\n"));
    assert!(effects(2)["after"].is_null());
    assert!(effects(3)["before"].is_null());
    assert_eq!(effects(3)["after"]["sha256"], digest(b"failed\n"));
    assert_eq!(original.observations[3]["error"][1], "ValueError");
    assert_eq!(effects(4)["before"], effects(4)["after"]);
    assert!(
        !original_session
            .current_workspace()
            .join("artifact.bin")
            .exists()
    );
    assert!(!original_path.join("artifact.bin").exists());
    assert_eq!(
        original.observations[0]["return"],
        candidate.observations[0]["return"]
    );
    let report = fs::read_to_string(original_session.report().unwrap()).unwrap();
    assert!(report.contains("artifact.bin"));
    assert!(report.contains("File-effect scope"));
}

#[test]
#[ignore = "requires a Linux host with working Bubblewrap and /usr/bin/python3"]
fn sandboxed_capture_refuses_symlinks_directories_and_oversize_files() {
    use oxidize_engine::Session;
    use std::{fs, path::Path};
    for code in [
        "from pathlib import Path\n\ndef record(action):\n Path('artifact.bin').symlink_to('cases.json')\n return 1\n",
        "from pathlib import Path\n\ndef record(action):\n Path('artifact.bin').mkdir(exist_ok=True)\n return 1\n",
        "from pathlib import Path\n\ndef record(action):\n Path('artifact.bin').write_bytes(b'x' * 1048577)\n return 1\n",
    ] {
        let dir = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
            .unwrap();
        let root = dir.path().join("app");
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-file-effects");
        oxidize_core::snapshot(&source, &root).unwrap();
        fs::write(root.join("effects.py"), code).unwrap();
        let session = Session::open(&root).unwrap();
        let error = session.benchmark("effects.py:record", false).unwrap_err();
        assert!(format!("{error:#}").contains("file-effect"), "{error:#}");
    }
}
