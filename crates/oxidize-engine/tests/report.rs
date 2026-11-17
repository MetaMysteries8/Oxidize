//! Milestone 2 §5: the report for rejected, interrupted, accepted and rolled-back sessions puts
//! status, speed and its variability, the reason, build/test evidence, repairs, workloads,
//! runtime dependencies and evidence limits together, and `report.json` stays versioned.
//!
//! Rejected and interrupted sessions come from real runs, stopped by fault injection before
//! the native build. The accepted session promotes that real attempt's state (with a
//! measurement, a repair and a workload run), so the report generator is exercised for every
//! outcome without a PyO3 build. The opt-in native tests check real accepted reports.
use oxidize_core::{fingerprint, snapshot};
use oxidize_engine::{
    CHECKPOINT_SCHEMA_VERSION, Fault, FaultPoint, Phase, Repair, RepairTrigger, Session,
    sampling::{self, Measurement, Round, Verdict},
    workloads::{self, WorkloadRound, WorkloadRun, WorkloadSide},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const COMPONENT: &str = "compute.py:count_primes";

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
        .canonicalize()
        .unwrap()
}

/// A real run of `python-cli`, stopped at the native build with `fault`.
fn stopped_session(fault: Fault) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    snapshot(&repo("fixtures/python-cli"), &root).unwrap();
    let config = fs::read_to_string(root.join("oxidize.toml"))
        .unwrap()
        .replace("iterations = 7", "iterations = 3")
        .replace("warmup = 2", "warmup = 0");
    fs::write(root.join("oxidize.toml"), config).unwrap();
    fs::write(
        root.join("cases.json"),
        r#"[{"args": [10]}, {"args": [-1]}]"#,
    )
    .unwrap();
    let mut session = Session::open(&root).unwrap();
    session.inject_faults(move |p| {
        if *p == FaultPoint::Step("native source") {
            fault
        } else {
            Fault::None
        }
    });
    let response = repo("fixtures/python-cli/migration-response.json");
    assert!(
        session
            .migrate(COMPONENT, Some(&response), true, false)
            .is_err()
    );
    (dir, root)
}

fn report(root: &Path) -> (String, serde_json::Value) {
    let session = Session::open(root).unwrap();
    let markdown = fs::read_to_string(session.report().unwrap()).unwrap();
    let json =
        serde_json::from_slice(&fs::read(session.directory.join("report.json")).unwrap()).unwrap();
    (markdown, json)
}

/// What every report must say, whatever the outcome.
fn assert_common(markdown: &str, json: &serde_json::Value, label: &str) {
    for needle in [
        "## Summary",
        "Python runtime and PyO3 remain required",
        "Verified functionality coverage: **unknown**",
        "## Comparison policy",
        "## File-effect scope",
        "## History",
        "## Application workloads",
        "Evidence provenance:",
        "- baseline tests: exit Some(0)",
        "## Evidence limits",
    ] {
        assert!(
            markdown.contains(needle),
            "{label}: missing {needle:?}\n{markdown}"
        );
    }
    assert_eq!(json["schema_version"], CHECKPOINT_SCHEMA_VERSION, "{label}");
    assert!(
        json["events"].as_array().is_some_and(|e| !e.is_empty()),
        "{label}"
    );
}

#[test]
fn a_rejected_session_reports_why_and_what_was_kept() {
    let (_dir, root) = stopped_session(Fault::Fail);
    let (markdown, json) = report(&root);
    assert_common(&markdown, &json, "rejected");
    assert!(
        markdown.contains("Status: **rejected** (reason below)"),
        "{markdown}"
    );
    assert!(markdown.contains(
        "- Attempt 1 `compute.py:count_primes`: rejected; speed not measured; 0 repair(s); reason:"
    ));
    assert!(markdown.contains("\nRejection: "), "{markdown}");
    assert!(!markdown.contains("Measured component workload speedup"));
}

#[test]
fn an_interrupted_session_says_nothing_was_accepted_and_how_to_resume() {
    let (_dir, root) = stopped_session(Fault::Crash);
    let (markdown, json) = report(&root);
    assert_common(&markdown, &json, "interrupted");
    assert!(
        markdown.contains(
            "Status: **interrupted** during Building. Nothing from it is accepted; `oxidize resume`"
        ),
        "{markdown}"
    );
    assert!(markdown.contains(": interrupted; speed not measured"));
    assert!(!markdown.contains("Rejection:"));
}

#[test]
fn accepted_and_rolled_back_sessions_show_speed_variability_repairs_and_workloads() {
    let (_dir, root) = stopped_session(Fault::Fail);
    let mut session = Session::open(&root).unwrap();
    // Promote the real attempt as an accepted run would leave it.
    let rounds: Vec<Round> = (0..3)
        .map(|i| Round {
            order: sampling::order(i),
            original_ns: vec![300.0, 310.0, 290.0],
            candidate_ns: vec![100.0 + i as f64 * 10.0; 3],
        })
        .collect();
    let verdict = sampling::decide(&rounds, 3, 0.25);
    let workspace = session.workspace(Some(1));
    let state = &mut session.state;
    {
        let a = &mut state.attempts[0];
        a.phase = Phase::Accepted;
        a.error = None;
        a.candidate = a.baseline.clone();
        a.candidate_fingerprint = Some(fingerprint(&workspace).unwrap());
        a.speedup = Some(3.0);
        a.measurement = Some(Measurement {
            rounds,
            max_relative_mad: 0.25,
            minimum_samples: 3,
            verdict,
        });
        a.repaired = 1;
        a.repairs = vec![Repair {
            trigger: RepairTrigger::IntegratedTests,
            diagnostics: "FAIL: test_known_counts".into(),
            truncated: false,
            generation: 1,
        }];
    }
    state.current = Some(1);
    let side = |wall: f64| WorkloadSide {
        wall_ns: vec![wall; 3],
        cpu_ns: Some(vec![wall / 2.0; 3]),
        peak_rss_kib: Some(20_000),
        unsupported: vec![],
    };
    state.workloads.push(WorkloadRun {
        against: 1,
        isolation: "trust-code".into(),
        os: "linux".into(),
        arch: "x86_64".into(),
        results: vec![workloads::summarize(
            "cli-30000",
            &["python3".into(), "main.py".into(), "30000".into()],
            vec![WorkloadRound {
                order: sampling::order(0),
                original: side(200.0),
                candidate: side(100.0),
            }],
            3,
            0.25,
        )],
    });
    session.save().unwrap();
    drop(session);

    let (markdown, json) = report(&root);
    assert_common(&markdown, &json, "accepted");
    assert!(
        markdown.contains("Status: **accepted**, the current workspace."),
        "{markdown}"
    );
    assert!(
        markdown.contains("- Attempt 1 `compute.py:count_primes`: accepted (current); speed 2.727x (2.500x to 3.000x); 1 repair(s)\n"),
        "{markdown}"
    );
    assert!(markdown.contains("Measured component workload speedup: **2.727x** (range 2.500x to 3.000x across rounds: 3.000x, 2.727x, 2.500x)"), "{markdown}");
    assert!(
        markdown.contains("- Repair 1: integrated tests failed; the provider was given 23 bytes")
    );
    assert!(
        markdown.contains("- `cli-30000` (`python3 main.py 30000`): wall time **2.000x**"),
        "{markdown}"
    );
    assert!(markdown.contains("peak RSS original 20000 KiB, accepted 20000 KiB"));
    assert!(!markdown.contains("Rejection:"));
    // report.json carries the raw samples behind those numbers.
    assert_eq!(
        json["attempts"][0]["measurement"]["rounds"][2]["candidate_ns"][0],
        120.0
    );

    let mut session = Session::open(&root).unwrap();
    session.rollback().unwrap();
    drop(session);
    let (markdown, json) = report(&root);
    assert_common(&markdown, &json, "rolled back");
    assert!(markdown.contains("Status: **rolled back**. Its workspace and evidence are kept"));
    assert!(
        markdown.contains(": rolled back; speed 2.727x"),
        "{markdown}"
    );
    assert!(markdown.contains("Current accepted workspace: `") && markdown.contains("/baseline`"));
    assert!(matches!(
        serde_json::from_value::<Verdict>(json["attempts"][0]["measurement"]["verdict"].clone())
            .unwrap(),
        Verdict::Measured { .. }
    ));
}

#[test]
fn a_new_session_reports_that_nothing_was_attempted() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    snapshot(&repo("fixtures/python-cli"), &root).unwrap();
    drop(Session::open(&root).unwrap());
    let (markdown, json) = report(&root);
    assert!(
        markdown.contains("## Summary\n\nNo migration attempts yet."),
        "{markdown}"
    );
    assert_eq!(json["schema_version"], CHECKPOINT_SCHEMA_VERSION);
}
