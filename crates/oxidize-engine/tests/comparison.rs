use oxidize_core::{ComparisonPolicy, FloatTolerance};
use oxidize_engine::{Evidence, equivalent, equivalent_with_policy};
use serde_json::{Value, json};

fn evidence(value: Value) -> Evidence {
    serde_json::from_value(json!({
        "observation_schema_version": 2,
        "observations": [{
            "return": value,
            "arguments_after": {
                "args": ["list", [["list", [["int", 1], ["int", 7]]]]],
                "kwargs": ["dict", []]
            },
            "stdout": "result\r\nready",
            "stderr": "diagnostic\r"
        }],
        "import_stdout": "",
        "import_stderr": "",
        "benchmark": {
            "samples_ns": [10, 11, 12], "median_ns": 11, "mad_ns": 1,
            "successful_cases": 1, "calls_per_sample": 1
        }
    }))
    .unwrap()
}

fn policy() -> ComparisonPolicy {
    ComparisonPolicy {
        return_float_tolerance: Some(FloatTolerance {
            absolute: 0.000001,
            relative: 0.00001,
        }),
        normalize_output_line_endings: true,
    }
}

#[test]
fn exact_default_rejects_float_and_line_ending_differences() {
    let original = evidence(json!(["float", 0.1_f64 + 0.2]));
    assert!(equivalent(&original, &original));
    let candidate = evidence(json!(["float", 0.3]));
    assert!(!equivalent(&original, &candidate));
    assert!(equivalent_with_policy(&original, &candidate, &policy()));
    assert!(!equivalent_with_policy(
        &original,
        &candidate,
        &ComparisonPolicy {
            normalize_output_line_endings: true,
            ..ComparisonPolicy::default()
        }
    ));
    let mut candidate = original.clone();
    candidate.observations[0]["stdout"] = json!("result\nready");
    candidate.observations[0]["stderr"] = json!("diagnostic\n");
    assert!(!equivalent(&original, &candidate));
    assert!(equivalent_with_policy(&original, &candidate, &policy()));
    assert!(!equivalent_with_policy(
        &original,
        &candidate,
        &ComparisonPolicy {
            return_float_tolerance: policy().return_float_tolerance,
            ..ComparisonPolicy::default()
        }
    ));
}

#[test]
fn tolerances_apply_to_nested_float_values_but_not_keys_or_types() {
    let original = evidence(json!([
        "dict",
        [[
            ["str", "value"],
            ["tuple", [["float", 1.0], ["list", [["float", 0.0]]]]]
        ]]
    ]));
    let mut candidate = original.clone();
    candidate.observations[0]["return"][1][0][1][1][0][1] = json!(1.000005);
    candidate.observations[0]["return"][1][0][1][1][1][1][0][1] = json!(0.0000005);
    assert!(equivalent_with_policy(&original, &candidate, &policy()));
    assert!(!equivalent(&original, &candidate));
    candidate.observations[0]["return"][1][0][1][0] = json!("list");
    assert!(!equivalent_with_policy(&original, &candidate, &policy()));

    let original = evidence(json!(["dict", [[["float", 1.0], ["int", 2]]]]));
    let candidate = evidence(json!(["dict", [[["float", 1.0000001], ["int", 2]]]]));
    assert!(!equivalent_with_policy(&original, &candidate, &policy()));
    for (a, b) in [
        (json!(["float", 1.0]), json!(["int", 1])),
        (
            json!(["int", 9007199254740992_u64]),
            json!(["int", 9007199254740993_u64]),
        ),
        (json!(["str", "a\r\n"]), json!(["str", "a\n"])),
        (json!(["list", [["float", 1.0]]]), json!(["list", []])),
        (
            json!([
                "dict",
                [[["str", "a"], ["int", 1]], [["str", "b"], ["int", 2]]]
            ]),
            json!([
                "dict",
                [[["str", "b"], ["int", 2]], [["str", "a"], ["int", 1]]]
            ]),
        ),
    ] {
        assert!(!equivalent_with_policy(
            &evidence(a),
            &evidence(b),
            &policy()
        ));
    }
}

#[test]
fn unrelated_mutations_errors_output_imports_and_extra_fields_stay_exact() {
    let original = evidence(json!(["float", 0.3]));
    for (field, value) in [
        (
            "arguments_after",
            json!({"args": ["list", []], "kwargs": ["dict", []]}),
        ),
        ("stdout", json!("different\nready")),
        ("stderr", json!("diagnostic\nextra")),
        ("extra", json!(true)),
        ("return", json!(["float", 0.4])),
    ] {
        let mut candidate = original.clone();
        candidate.observations[0][field] = value;
        assert!(
            !equivalent_with_policy(&original, &candidate, &policy()),
            "{field}"
        );
    }
    let mut candidate = original.clone();
    candidate.import_stdout = "\r\n".into();
    assert!(!equivalent_with_policy(&original, &candidate, &policy()));
    candidate = original.clone();
    candidate.import_stderr = "\r\n".into();
    assert!(!equivalent_with_policy(&original, &candidate, &policy()));
    candidate = original.clone();
    candidate.observations.clear();
    assert!(!equivalent_with_policy(&original, &candidate, &policy()));
    let mut floats_in_arguments = original.clone();
    floats_in_arguments.observations[0]["arguments_after"]["args"] =
        json!(["list", [["float", 1.0]]]);
    candidate = floats_in_arguments.clone();
    candidate.observations[0]["arguments_after"]["args"][1][0][1] = json!(1.0000001);
    assert!(!equivalent_with_policy(
        &floats_in_arguments,
        &candidate,
        &policy()
    ));

    let mut error = original.clone();
    error.observations[0]
        .as_object_mut()
        .unwrap()
        .remove("return");
    error.observations[0]["error"] = json!(["builtins", "ValueError", "bad\r\nvalue"]);
    assert!(!equivalent_with_policy(&original, &error, &policy()));
    for (index, value) in [(0, "other"), (1, "TypeError"), (2, "bad\nvalue")] {
        candidate = error.clone();
        candidate.observations[0]["error"][index] = json!(value);
        assert!(!equivalent_with_policy(&error, &candidate, &policy()));
    }
    candidate = error.clone();
    candidate.observations[0]["arguments_after"]["args"] = json!(["list", []]);
    assert!(!equivalent_with_policy(&error, &candidate, &policy()));
}

#[test]
fn tolerance_boundaries_extremes_and_invalid_policies_fail_closed() {
    let absolute = ComparisonPolicy {
        return_float_tolerance: Some(FloatTolerance {
            absolute: 0.125,
            relative: 0.0,
        }),
        ..ComparisonPolicy::default()
    };
    for (a, b, expected) in [
        (0.0, 0.125, true),
        (0.0, 0.125001, false),
        (f64::MAX, -f64::MAX, false),
        (1.0, 1.0, true),
    ] {
        let original = evidence(json!(["float", a]));
        let candidate = evidence(json!(["float", b]));
        assert_eq!(
            equivalent_with_policy(&original, &candidate, &absolute),
            expected
        );
        assert_eq!(
            equivalent_with_policy(&candidate, &original, &absolute),
            expected
        );
    }
    let relative = ComparisonPolicy {
        return_float_tolerance: Some(FloatTolerance {
            absolute: 0.0,
            relative: 0.01,
        }),
        ..ComparisonPolicy::default()
    };
    assert!(equivalent_with_policy(
        &evidence(json!(["float", 1000.0])),
        &evidence(json!(["float", 1005.0])),
        &relative
    ));
    assert!(!equivalent_with_policy(
        &evidence(json!(["float", 1000.0])),
        &evidence(json!(["float", 1020.0])),
        &relative
    ));
    let zero = ComparisonPolicy {
        return_float_tolerance: Some(FloatTolerance::default()),
        ..ComparisonPolicy::default()
    };
    assert!(!equivalent_with_policy(
        &evidence(json!(["float", 0.1_f64 + 0.2])),
        &evidence(json!(["float", 0.3])),
        &zero
    ));
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        let invalid = ComparisonPolicy {
            return_float_tolerance: Some(FloatTolerance {
                absolute: invalid,
                relative: 0.0,
            }),
            ..ComparisonPolicy::default()
        };
        let e = evidence(json!(["float", 1.0]));
        assert!(!equivalent_with_policy(&e, &e, &invalid));
    }
    let mut legacy = evidence(json!(["float", 1.0]));
    legacy.observation_schema_version = 1;
    assert!(!equivalent_with_policy(&legacy, &legacy, &policy()));
    legacy.observation_schema_version = 2;
    legacy.observations[0]
        .as_object_mut()
        .unwrap()
        .remove("arguments_after");
    assert!(!equivalent_with_policy(&legacy, &legacy, &policy()));
}

#[test]
fn reports_and_reopened_sessions_retain_policy_without_rewriting_raw_evidence() {
    use oxidize_engine::Session;
    use std::{fs, path::Path};

    let dir = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-comparison");
    let root = dir.path().join("app");
    oxidize_core::snapshot(&source, &root).unwrap();
    let session = Session::open(&root).unwrap();
    assert_eq!(session.state.config.verification.comparison, policy());
    let report = session.report().unwrap();
    let markdown = fs::read_to_string(report).unwrap();
    assert!(markdown.contains("\"absolute\": 1e-6"));
    assert!(markdown.contains("\"normalize_output_line_endings\": true"));
    let json: Value =
        serde_json::from_slice(&fs::read(session.directory.join("report.json")).unwrap()).unwrap();
    assert_eq!(
        json["config"]["verification"]["comparison"],
        serde_json::to_value(policy()).unwrap()
    );
    drop(session);
    // Changing the input configuration cannot silently relax an existing
    // checkpoint's frozen comparison contract on reopen.
    fs::write(root.join("oxidize.toml"), "python = '/usr/bin/python3'\n").unwrap();
    let session = Session::open(&root).unwrap();
    assert_eq!(session.state.config.verification.comparison, policy());
    assert!(session.assert_source_unchanged().is_err());
    let original = evidence(json!(["float", 0.30000000000000004]));
    let candidate = evidence(json!(["float", 0.3]));
    let saved = serde_json::to_value(&original).unwrap();
    assert!(equivalent_with_policy(
        &original,
        &candidate,
        &session.state.config.verification.comparison
    ));
    assert_eq!(serde_json::to_value(&original).unwrap(), saved);
}

#[test]
#[ignore = "requires a Linux host with working Bubblewrap and /usr/bin/python3"]
fn sandboxed_python_fixture_preserves_raw_evidence_and_scoped_comparison() {
    use oxidize_engine::Session;
    use std::{fs, path::Path};

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-comparison");
    // The runner mounts an isolated /tmp after binding the workspace; use an
    // ignored non-/tmp directory so that mount cannot hide our test project.
    let dir =
        tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")).unwrap();
    let original_path = dir.path().join("original");
    let candidate_path = dir.path().join("candidate");
    oxidize_core::snapshot(&source, &original_path).unwrap();
    oxidize_core::snapshot(&source, &candidate_path).unwrap();
    let code = fs::read_to_string(candidate_path.join("compare.py")).unwrap();
    let candidate_code = code
        .replace("0.1 + 0.2", "0.3")
        .replace("result\\r\\nready", "result\\nready")
        .replace("diagnostic\\r", "diagnostic\\n");
    fs::write(candidate_path.join("compare.py"), candidate_code).unwrap();
    let original_session = Session::open(&original_path).unwrap();
    let candidate_session = Session::open(&candidate_path).unwrap();
    // Neither fixture runs with --trust-code; fail rather than falling back.
    let original = original_session
        .benchmark("compare.py:observe", false)
        .unwrap();
    let candidate = candidate_session
        .benchmark("compare.py:observe", false)
        .unwrap();
    assert_eq!(original.observations.len(), 3);
    assert_eq!(original.observations[0]["stdout"], "result\r\nready");
    assert_eq!(candidate.observations[0]["stdout"], "result\nready");
    assert_ne!(
        original.observations[0]["return"],
        candidate.observations[0]["return"]
    );
    assert!(!equivalent(&original, &candidate));
    assert!(equivalent_with_policy(
        &original,
        &candidate,
        &original_session.state.config.verification.comparison
    ));
    let mut wrong_mutation = candidate.clone();
    wrong_mutation.observations[2]["arguments_after"]["args"] = json!(["list", []]);
    assert!(!equivalent_with_policy(
        &original,
        &wrong_mutation,
        &policy()
    ));
    let mut wrong_error = candidate.clone();
    wrong_error.observations[2]["error"][2] = json!("bad\nvalue");
    assert!(!equivalent_with_policy(&original, &wrong_error, &policy()));
}
