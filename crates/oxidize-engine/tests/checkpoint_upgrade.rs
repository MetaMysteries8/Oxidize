//! Milestone 2 §3: versioned event history and the checkpoint schema-upgrade strategy.
//! The legacy fixture is a real checkpoint written by the v0.1.0 release.
use oxidize_core::snapshot;
use oxidize_engine::{CHECKPOINT_SCHEMA_VERSION, EventKind, Phase, Session};
use std::{fs, path::Path};

fn repo(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
        .canonicalize()
        .unwrap()
}
fn legacy_bytes() -> Vec<u8> {
    fs::read(repo("fixtures/legacy-v0.1.0-session/state.json")).unwrap()
}
/// A copy of the project the v0.1.0 session was made from, carrying that session.
fn legacy_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    snapshot(&repo("fixtures/python-cli"), &root).unwrap();
    fs::create_dir_all(root.join(".oxidize")).unwrap();
    fs::write(root.join(".oxidize/state.json"), legacy_bytes()).unwrap();
    dir
}

#[test]
fn v0_1_0_session_upgrades_without_losing_anything() {
    let dir = legacy_project();
    let root = dir.path().join("app");
    let old: serde_json::Value = serde_json::from_slice(&legacy_bytes()).unwrap();

    let session = Session::open(&root).unwrap();
    let state = &session.state;
    assert_eq!(state.schema_version, CHECKPOINT_SCHEMA_VERSION);
    assert_eq!(state.source_fingerprint, old["source_fingerprint"]);
    assert_eq!(state.current, None);
    let attempt = &state.attempts[0];
    assert_eq!(attempt.phase, Phase::Rejected);
    assert_eq!(
        attempt.error.as_deref(),
        old["attempts"][0]["error"].as_str()
    );
    assert_eq!(
        attempt.generations[0].code.rust,
        old["attempts"][0]["generations"][0]["code"]["rust"]
    );
    assert_eq!(attempt.checks.len(), 2);
    let baseline = attempt.baseline.as_ref().unwrap();
    assert_eq!(
        baseline.observation_schema_version, 1,
        "legacy evidence stays legacy"
    );

    // The upgrade is the only event: earlier history wasn't recorded and isn't invented.
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].seq, 1);
    assert_eq!(
        state.events[0].kind,
        EventKind::SchemaUpgraded {
            from: 1,
            to: CHECKPOINT_SCHEMA_VERSION,
            backup: "state.v1.json".into()
        }
    );
    // The original checkpoint is kept byte for byte, and the upgrade is on disk.
    assert_eq!(
        fs::read(root.join(".oxidize/state.v1.json")).unwrap(),
        legacy_bytes()
    );
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(".oxidize/state.json")).unwrap()).unwrap();
    assert_eq!(on_disk["schema_version"], CHECKPOINT_SCHEMA_VERSION);
    let report = fs::read_to_string(session.report().unwrap()).unwrap();
    assert!(
        report.contains("upgraded from schema 1 to 2 (original kept as `state.v1.json`); changes before that were not recorded"),
        "{report}"
    );
    drop(session);

    // Reopening doesn't upgrade again.
    let again = Session::open(&root).unwrap();
    assert_eq!(again.state.events.len(), 1);
    assert_eq!(
        fs::read(root.join(".oxidize/state.v1.json")).unwrap(),
        legacy_bytes()
    );
}

#[test]
fn interrupted_upgrade_is_redone_and_a_conflicting_backup_is_never_overwritten() {
    // Interrupted after the backup was written but before the new state: upgrade again.
    let dir = legacy_project();
    let root = dir.path().join("app");
    fs::write(root.join(".oxidize/state.v1.json"), legacy_bytes()).unwrap();
    let session = Session::open(&root).unwrap();
    assert_eq!(session.state.schema_version, CHECKPOINT_SCHEMA_VERSION);
    drop(session);

    // A backup that isn't this checkpoint is someone else's data: refuse, touch nothing.
    let dir = legacy_project();
    let root = dir.path().join("app");
    fs::write(root.join(".oxidize/state.v1.json"), b"{\"other\": true}").unwrap();
    let error = Session::open(&root).err().unwrap();
    assert!(format!("{error:#}").contains("state.v1.json"), "{error:#}");
    assert_eq!(
        fs::read(root.join(".oxidize/state.json")).unwrap(),
        legacy_bytes()
    );
    assert_eq!(
        fs::read(root.join(".oxidize/state.v1.json")).unwrap(),
        b"{\"other\": true}"
    );
}

#[test]
fn newer_or_unknown_schemas_are_refused_untouched() {
    for (version, expected) in [
        (
            serde_json::json!(CHECKPOINT_SCHEMA_VERSION + 1),
            "newer Oxidize",
        ),
        (serde_json::json!(0), "unsupported checkpoint schema"),
        (serde_json::json!("1"), "unsupported checkpoint schema"),
    ] {
        let dir = legacy_project();
        let root = dir.path().join("app");
        let mut state: serde_json::Value = serde_json::from_slice(&legacy_bytes()).unwrap();
        state["schema_version"] = version;
        let bytes = serde_json::to_vec_pretty(&state).unwrap();
        fs::write(root.join(".oxidize/state.json"), &bytes).unwrap();
        let error = Session::open(&root).err().unwrap();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
        assert_eq!(fs::read(root.join(".oxidize/state.json")).unwrap(), bytes);
        assert!(!root.join(".oxidize/state.v1.json").exists());
    }
}

#[test]
fn new_sessions_record_their_history_with_each_state_change() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    snapshot(&repo("fixtures/python-cli"), &root).unwrap();
    fs::write(
        root.join("test_broken.py"),
        "import unittest\nclass Broken(unittest.TestCase):\n def test_fail(self): self.fail('existing bug')\n",
    )
    .unwrap();
    let mut session = Session::open(&root).unwrap();
    assert_eq!(session.state.events[0].kind, EventKind::SessionCreated);
    assert!(
        session
            .migrate("compute.py:count_primes", None, true, false)
            .is_err()
    );
    let kinds: Vec<_> = session
        .state
        .events
        .iter()
        .map(|e| e.kind.clone())
        .collect();
    assert_eq!(
        kinds[1],
        EventKind::AttemptStarted {
            attempt: 1,
            component: "compute.py:count_primes".into(),
            base: None
        }
    );
    assert!(kinds.contains(&EventKind::PhaseChanged {
        attempt: 1,
        phase: Phase::Prepared
    }));
    assert!(matches!(
        kinds.last().unwrap(),
        EventKind::Rejected { attempt: 1, error } if error.contains("independent tests failed")
    ));
    assert!(
        session
            .state
            .events
            .iter()
            .enumerate()
            .all(|(i, e)| e.seq == i + 1)
    );
    drop(session);

    // The history on disk is the one in memory: it is saved with the state, atomically.
    let reopened = Session::open(&root).unwrap();
    assert_eq!(
        reopened
            .state
            .events
            .iter()
            .map(|e| e.kind.clone())
            .collect::<Vec<_>>(),
        kinds
    );
}

#[test]
fn tampered_history_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    snapshot(&repo("fixtures/python-cli"), &root).unwrap();
    drop(Session::open(&root).unwrap());
    let path = root.join(".oxidize/state.json");
    let mut state: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["events"][0]["seq"] = serde_json::json!(7);
    fs::write(&path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
    let error = Session::open(&root).err().unwrap();
    assert!(
        format!("{error:#}").contains("invalid event history"),
        "{error:#}"
    );
}
