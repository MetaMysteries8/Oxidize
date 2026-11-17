//! Native migration installs a Linux `.so` or a macOS `.dylib`. Elsewhere a migration is refused
//! explicitly, before anything is built, and nothing is accepted.
#![cfg(not(any(target_os = "linux", target_os = "macos")))]
use oxidize_core::snapshot;
use oxidize_engine::{Phase, Session};
use std::path::Path;

#[test]
fn unsupported_platforms_refuse_migration_before_building() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-cli");
    snapshot(&fixture.canonicalize().unwrap(), &root).unwrap();
    let response = fixture.join("migration-response.json");
    let mut session = Session::open(&root).unwrap();
    let error = session
        .migrate("compute.py:count_primes", Some(&response), true, false)
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("currently supports Linux and macOS"),
        "{error:#}"
    );
    let attempt = &session.state.attempts[0];
    assert_eq!(attempt.phase, Phase::Rejected);
    assert!(
        attempt
            .checks
            .iter()
            .all(|(name, _)| name == "baseline tests"),
        "nothing was built: {:?}",
        attempt.checks.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
    assert_eq!(session.state.current, None);
}
