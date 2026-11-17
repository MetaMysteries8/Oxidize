//! Milestone 2 §3: interruptions and write failures at every checkpoint save and file-writing
//! step. After each fault, reopening must keep the previous accepted workspace (or the newly
//! accepted one, never anything partial), leave the input untouched, keep the failed attempt
//! inspectable, and let resume carry on.
//!
//! These standard tests cover everything up to the native build. Points from the build onward
//! need a real PyO3 build and are covered by the opt-in test at the end.
use oxidize_core::{fingerprint, snapshot};
use oxidize_engine::{EventKind, Fault, FaultPoint, Phase, Session};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

const COMPONENT: &str = "compute.py:count_primes";

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
        .canonicalize()
        .unwrap()
}
fn response() -> PathBuf {
    repo("fixtures/python-cli/migration-response.json")
}
/// A copy of `python-cli` with the smallest timing policy, so many runs stay quick.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    snapshot(&repo("fixtures/python-cli"), &root).unwrap();
    let config = fs::read_to_string(root.join("oxidize.toml"))
        .unwrap()
        .replace("iterations = 7", "iterations = 3")
        .replace("warmup = 2", "warmup = 0")
        .replace("calls_per_sample = 3", "calls_per_sample = 1");
    fs::write(root.join("oxidize.toml"), config).unwrap();
    let cases = r#"[{"args": [10]}, {"args": [-1]}, {"args": ["10"]}]"#;
    fs::write(root.join("cases.json"), cases).unwrap();
    (dir, root)
}

/// Record every point a run reaches, applying `fault` at `target`.
fn injector(
    target: Option<FaultPoint>,
    fault: Fault,
    seen: Rc<RefCell<Vec<FaultPoint>>>,
) -> impl FnMut(&FaultPoint) -> Fault {
    move |point| {
        seen.borrow_mut().push(point.clone());
        if Some(point) == target.as_ref() {
            fault
        } else if *point == FaultPoint::Step("native source") {
            // Stop every run before the real build; that part is the opt-in test's.
            Fault::Crash
        } else {
            Fault::None
        }
    }
}

/// The recovery guarantees, checked on a freshly reopened session.
fn assert_recoverable(root: &Path, input: &str, label: &str) -> Session {
    let session = Session::open(root).unwrap_or_else(|e| panic!("{label}: reopen failed: {e:#}"));
    assert_eq!(fingerprint(root).unwrap(), input, "{label}: input changed");
    assert_eq!(
        session.state.current, None,
        "{label}: nothing may be accepted"
    );
    for a in &session.state.attempts {
        assert_ne!(
            a.phase,
            Phase::Accepted,
            "{label}: partial attempt accepted"
        );
    }
    // The baseline checkpoint is intact.
    assert_eq!(
        fingerprint(&session.workspace(None)).unwrap(),
        session.state.source_fingerprint,
        "{label}: baseline changed"
    );
    let report = fs::read_to_string(session.report().unwrap()).unwrap();
    assert!(!report.contains("Status: Accepted"), "{label}: {report}");
    session
}

/// Run a migration with `fault` at `target`; return every point reached and the error.
fn run(root: &Path, target: Option<FaultPoint>, fault: Fault) -> (Vec<FaultPoint>, String) {
    let seen = Rc::new(RefCell::new(vec![]));
    let mut session = Session::open(root).unwrap();
    session.inject_faults(injector(target, fault, seen.clone()));
    let error = session
        .migrate(COMPONENT, Some(&response()), true, false)
        .unwrap_err();
    drop(session);
    let seen = seen.borrow().clone();
    (seen, format!("{error:#}"))
}

/// After any fault, resume (or a fresh migrate, if the attempt never reached disk) gets back
/// to the same place an uninterrupted run reaches: the native build.
fn assert_resumes_to_build(root: &Path, label: &str) {
    let seen = Rc::new(RefCell::new(vec![]));
    let mut session = Session::open(root).unwrap();
    session.inject_faults(injector(None, Fault::None, seen));
    let error = if session.state.attempts.is_empty() {
        session.migrate(COMPONENT, Some(&response()), true, false)
    } else {
        session.resume(Some(&response()), true, false)
    }
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("injected crash at Step(\"native source\")"),
        "{label}: resume didn't reach the build: {error:#}"
    );
    drop(session);
    let session = Session::open(root).unwrap();
    let last = session.state.attempts.last().unwrap();
    assert_eq!(last.phase, Phase::Building, "{label}");
    assert!(
        last.baseline.is_some(),
        "{label}: resume lost baseline evidence"
    );
    assert_eq!(
        last.generations.len(),
        1,
        "{label}: saved code duplicated or lost"
    );
}

#[test]
fn crash_or_torn_write_at_every_point_before_the_build_is_recoverable() {
    // An uninterrupted run, to list the points it passes before the build.
    let (_dir, root) = project();
    let (points, _) = run(&root, None, Fault::None);
    let saves = points
        .iter()
        .filter(|p| matches!(p, FaultPoint::BeforeSave(_)))
        .count();
    assert!(
        saves >= 6,
        "expected several saves before the build: {points:?}"
    );
    assert!(points.contains(&FaultPoint::Step("attempt workspace")));

    for point in points
        .iter()
        .filter(|p| **p != FaultPoint::Step("native source"))
    {
        let faults: &[Fault] = match point {
            FaultPoint::BeforeSave(_) => &[Fault::Crash, Fault::Torn],
            _ => &[Fault::Crash],
        };
        for &fault in faults {
            let label = format!("{fault:?} at {point:?}");
            let (_dir, root) = project();
            let input = fingerprint(&root).unwrap();
            let (_, error) = run(&root, Some(point.clone()), fault);
            assert!(error.contains("injected crash"), "{label}: {error}");
            let session = assert_recoverable(&root, &input, &label);
            if fault == Fault::Torn {
                // The torn temporary file is ignored and the old checkpoint stays readable.
                assert!(root.join(".oxidize/state.json.tmp").exists(), "{label}");
            }
            drop(session);
            assert_resumes_to_build(&root, &label);
            let session = assert_recoverable(&root, &input, &label);
            assert!(
                session.state.events.iter().any(|e| matches!(
                    e.kind,
                    EventKind::PhaseChanged {
                        phase: Phase::Building,
                        ..
                    }
                )),
                "{label}"
            );
        }
    }
}

#[test]
fn a_failed_write_rejects_the_attempt_and_the_rejection_is_kept() {
    let (_dir, root) = project();
    let (points, _) = run(&root, None, Fault::None);
    let saves: Vec<_> = points
        .iter()
        .filter(|p| matches!(p, FaultPoint::BeforeSave(_) | FaultPoint::AfterSave(_)))
        .cloned()
        .collect();
    for point in saves.iter().chain([&FaultPoint::Step("attempt workspace")]) {
        let label = format!("Fail at {point:?}");
        let (_dir, root) = project();
        let input = fingerprint(&root).unwrap();
        let (_, error) = run(&root, Some(point.clone()), Fault::Fail);
        assert!(error.contains("injected write failure"), "{label}: {error}");
        let session = assert_recoverable(&root, &input, &label);
        // Every failure, including the attempt-start save, is recorded as a rejection with
        // its cause.
        {
            let last = session.state.attempts.last().unwrap();
            assert_eq!(last.phase, Phase::Rejected, "{label}");
            assert!(
                last.error
                    .as_deref()
                    .unwrap()
                    .contains("injected write failure"),
                "{label}"
            );
            assert!(matches!(
                session.state.events.last().unwrap().kind,
                EventKind::Rejected { .. }
            ));
        }
        drop(session);
        assert_resumes_to_build(&root, &label);
    }
}

#[test]
fn when_every_write_fails_the_last_good_checkpoint_survives() {
    let (_dir, root) = project();
    let (points, _) = run(&root, None, Fault::None);
    let saves = points
        .iter()
        .filter(|p| matches!(p, FaultPoint::BeforeSave(_)))
        .count();
    for from in 1..=saves {
        let label = format!("every save from {from} fails");
        let (_dir, root) = project();
        let input = fingerprint(&root).unwrap();
        let mut session = Session::open(&root).unwrap();
        session.inject_faults(move |point| match point {
            FaultPoint::BeforeSave(n) if *n >= from => Fault::Fail,
            FaultPoint::Step("native source") => Fault::Crash,
            _ => Fault::None,
        });
        assert!(
            session
                .migrate(COMPONENT, Some(&response()), true, false)
                .is_err()
        );
        drop(session);
        drop(assert_recoverable(&root, &input, &label));
        assert_resumes_to_build(&root, &label);
    }
}

#[test]
fn finished_and_archived_attempts_drop_their_build_cache() {
    // A rejection removes the build cache the run left behind.
    let (_dir, root) = project();
    let mut session = Session::open(&root).unwrap();
    let workspace = session.workspace(Some(1));
    let planted = workspace.join("target/release");
    session.inject_faults(move |point| {
        if *point == FaultPoint::Step("native source") {
            fs::create_dir_all(&planted).unwrap();
            fs::write(planted.join("libbig.so"), vec![0; 4096]).unwrap();
            Fault::Fail
        } else {
            Fault::None
        }
    });
    assert!(
        session
            .migrate(COMPONENT, Some(&response()), true, false)
            .is_err()
    );
    drop(session);
    assert!(
        workspace.join("compute.py").exists(),
        "the rejected workspace stays inspectable"
    );
    assert!(
        !workspace.join("target").exists(),
        "rejection kept the build cache"
    );

    // A resume archives the previous run without its build cache.
    fs::create_dir_all(workspace.join("target/release")).unwrap();
    assert_resumes_to_build(&root, "archive");
    let archived: Vec<_> = workspace
        .parent()
        .unwrap()
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("interrupted-")
        })
        .collect();
    assert_eq!(archived.len(), 1);
    assert!(
        archived[0].join("compute.py").exists(),
        "the archive stays inspectable"
    );
    assert!(
        !archived[0].join("target").exists(),
        "the archive kept the build cache"
    );
}

/// `python-cli` plus `python-mutation`: two components, so one can be accepted while the
/// other is interrupted.
fn two_component_project() -> (tempfile::TempDir, PathBuf) {
    // On disk rather than /tmp, which may be a small tmpfs: every attempt builds PyO3.
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let root = dir.path().join("app");
    snapshot(&repo("fixtures/python-cli"), &root).unwrap();
    let config = fs::read_to_string(root.join("oxidize.toml"))
        .unwrap()
        .replace("iterations = 7", "iterations = 3")
        .replace("warmup = 2", "warmup = 0")
        .replace("calls_per_sample = 3", "calls_per_sample = 1");
    fs::write(root.join("oxidize.toml"), config).unwrap();
    for file in ["mutate.py", "test_mutate.py"] {
        fs::copy(repo("fixtures/python-mutation").join(file), root.join(file)).unwrap();
    }
    let cases = format!(
        r#"{{"{COMPONENT}": {}, "mutate.py:total_and_mark": {}}}"#,
        r#"[{"args": [10]}, {"args": [-1]}, {"args": ["10"]}]"#,
        fs::read_to_string(repo("fixtures/python-mutation/cases.json")).unwrap()
    );
    fs::write(root.join("cases.json"), cases).unwrap();
    // A recovery test, not a performance claim: permit a measured slowdown.
    let config = fs::read_to_string(root.join("oxidize.toml"))
        .unwrap()
        .replace("minimum_speedup = 1.0", "minimum_speedup = 0.000001");
    fs::write(root.join("oxidize.toml"), config).unwrap();
    (dir, root)
}

#[test]
#[ignore = "real PyO3 builds, about two per fault point; run with --include-ignored"]
fn native_crash_at_every_point_from_the_build_to_promotion_keeps_or_completes_a_checkpoint() {
    const SECOND: &str = "mutate.py:total_and_mark";
    let second_response = repo("fixtures/python-mutation/migration-response.json");
    let (_dir, root) = two_component_project();
    let input = fingerprint(&root).unwrap();
    let mut session = Session::open(&root).unwrap();
    session
        .migrate(COMPONENT, Some(&response()), true, false)
        .unwrap();
    let first = session.state.attempts[0]
        .candidate_fingerprint
        .clone()
        .unwrap();
    assert!(
        !session.workspace(Some(1)).join("target").exists(),
        "an accepted attempt keeps no build cache"
    );
    drop(session);

    // An uninterrupted run lists the points; roll it back so every run starts the same way.
    let seen = Rc::new(RefCell::new(vec![]));
    let mut session = Session::open(&root).unwrap();
    let record = seen.clone();
    session.inject_faults(move |p| {
        record.borrow_mut().push(p.clone());
        Fault::None
    });
    session
        .migrate(SECOND, Some(&second_response), true, false)
        .unwrap();
    session.rollback().unwrap();
    drop(session);
    let points = seen.borrow().clone();
    let from = points
        .iter()
        .position(|p| *p == FaultPoint::Step("native source"))
        .unwrap();
    // The last two points are the rollback's own save; this test is about migration.
    let migration_points = &points[from..points.len() - 2];

    for point in migration_points {
        let faults: &[Fault] = match point {
            FaultPoint::BeforeSave(_) => &[Fault::Torn],
            _ => &[Fault::Crash],
        };
        for &fault in faults {
            let label = format!("{fault:?} at {point:?}");
            let target = point.clone();
            let mut session = Session::open(&root).unwrap();
            session.inject_faults(move |p| if *p == target { fault } else { Fault::None });
            let crashed = session.migrate(SECOND, Some(&second_response), true, false);
            assert!(
                format!("{:#}", crashed.unwrap_err()).contains("injected crash"),
                "{label}"
            );
            drop(session);

            let mut session = Session::open(&root).unwrap();
            assert_eq!(fingerprint(&root).unwrap(), input, "{label}: input changed");
            let latest = session.state.attempts.last().unwrap().clone();
            if latest.phase == Phase::Accepted {
                // Crashed after the promotion was saved: the new checkpoint is complete.
                assert_eq!(session.state.current, Some(latest.id), "{label}");
                assert_eq!(
                    fingerprint(&session.current_workspace()).unwrap(),
                    latest.candidate_fingerprint.clone().unwrap(),
                    "{label}"
                );
            } else {
                // Crashed before it: the old checkpoint is current and untouched.
                assert_eq!(session.state.current, Some(1), "{label}");
                assert_eq!(
                    fingerprint(&session.workspace(Some(1))).unwrap(),
                    first,
                    "{label}"
                );
                let report = fs::read_to_string(session.report().unwrap()).unwrap();
                let section = report
                    .split(&format!("## Attempt {}:", latest.id))
                    .nth(1)
                    .unwrap();
                assert!(!section.contains("Status: Accepted"), "{label}: {section}");
                session
                    .resume(Some(&second_response), true, false)
                    .unwrap_or_else(|e| panic!("{label}: resume failed: {e:#}"));
                assert_eq!(session.state.current, Some(latest.id), "{label}");
            }
            // Repeated rollback always returns to the same untouched first checkpoint.
            session.rollback().unwrap();
            assert_eq!(session.state.current, Some(1), "{label}");
            assert_eq!(
                fingerprint(&session.current_workspace()).unwrap(),
                first,
                "{label}"
            );
            eprintln!("ok: {label}");
        }
    }
}

#[test]
fn resume_never_reports_the_previous_runs_measurements() {
    // A rejected attempt that had measured a speedup, as a differential mismatch leaves it.
    let (_dir, root) = project();
    let (_, _) = run(&root, Some(FaultPoint::Step("native source")), Fault::Fail);
    let mut session = Session::open(&root).unwrap();
    {
        let attempt = &mut session.state.attempts[0];
        assert_eq!(attempt.phase, Phase::Rejected);
        attempt.speedup = Some(9.5);
        attempt.candidate_fingerprint = Some("stale".into());
        attempt.repaired = 2;
    }
    session.save().unwrap();
    drop(session);

    // Resume, interrupted before it measures anything new.
    assert_resumes_to_build(&root, "resume");
    let session = Session::open(&root).unwrap();
    let attempt = &session.state.attempts[0];
    assert_eq!(attempt.speedup, None, "stale speedup survived resume");
    assert_eq!(attempt.candidate_fingerprint, None);
    assert_eq!(attempt.repaired, 0);
    let report = fs::read_to_string(session.report().unwrap()).unwrap();
    assert!(!report.contains("9.500x"), "{report}");
}

#[test]
fn resume_and_rollback_never_touch_input_edits() {
    let (_dir, root) = project();
    let (_, _) = run(&root, Some(FaultPoint::Step("native source")), Fault::Fail);
    fs::write(root.join("notes.txt"), "the user's unrelated edit").unwrap();
    let edited = fingerprint(&root).unwrap();
    let mut session = Session::open(&root).unwrap();
    let error = session.resume(Some(&response()), true, false).unwrap_err();
    assert!(
        format!("{error:#}").contains("input project changed"),
        "{error:#}"
    );
    assert!(session.rollback().is_err(), "nothing accepted to roll back");
    drop(session);
    // Refusing kept the edit and the checkpoint exactly as they were.
    assert_eq!(fingerprint(&root).unwrap(), edited);
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "the user's unrelated edit"
    );
    let session = Session::open(&root).unwrap();
    assert_eq!(session.state.attempts[0].phase, Phase::Rejected);
    drop(session);
    // Once the user moves the edit out, resume carries on from the same checkpoint.
    fs::remove_file(root.join("notes.txt")).unwrap();
    assert_resumes_to_build(&root, "after the edit is moved out");
}
