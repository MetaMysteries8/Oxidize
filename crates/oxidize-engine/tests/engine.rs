use oxidize_core::{Config, fingerprint, snapshot};
use oxidize_engine::{Phase, Session};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/python-cli")
        .canonicalize()
        .unwrap()
}
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    snapshot(&fixture(), &dir.path().join("app")).unwrap();
    dir
}

#[test]
fn checkpoints_lock_recover_and_detect_source_edits() {
    let dir = project();
    let root = dir.path().join("app");
    let session = Session::open(&root).unwrap();
    assert!(Session::open(&root).is_err());
    let original = session.state.source_fingerprint.clone();
    drop(session);
    let recovered = Session::open(&root).unwrap();
    assert_eq!(recovered.state.source_fingerprint, original);
    fs::write(root.join("new.py"), "# user edit").unwrap();
    assert!(recovered.assert_source_unchanged().is_err());
    assert!(recovered.workspace(None).join("compute.py").exists());
}

#[test]
fn baseline_tests_and_typed_differential_evidence() {
    let dir = project();
    let root = dir.path().join("app");
    let session = Session::open(&root).unwrap();
    session.verify(true).unwrap();
    let evidence = session.benchmark("compute.py:count_primes", true).unwrap();
    assert_eq!(evidence.observations.len(), 11);
    assert_eq!(evidence.observations[5]["return"][1], 3245);
    assert_eq!(evidence.observations[6]["error"][1], "ValueError");
    assert_eq!(evidence.observations[7]["error"][1], "TypeError");
    assert_eq!(evidence.benchmark.samples_ns.len(), 7);
    assert!(evidence.benchmark.median_ns > 0.0);
}

#[test]
fn rejects_broken_baselines_without_model_calls() {
    let dir = project();
    let root = dir.path().join("app");
    fs::write(root.join("test_broken.py"), "import unittest\nclass Broken(unittest.TestCase):\n def test_fail(self): self.fail('existing bug')\n").unwrap();
    let mut session = Session::open(&root).unwrap();
    assert!(
        session
            .migrate("compute.py:count_primes", None, true, false)
            .is_err()
    );
    assert_eq!(session.state.attempts[0].phase, Phase::Rejected);
    assert!(session.state.attempts[0].generations.is_empty());
    assert!(session.state.current.is_none());
}

#[test]
fn sandbox_does_not_silently_fall_back_to_trusted_execution() {
    let dir = project();
    let root = dir.path().join("app");
    // A missing tool directory cannot be exposed; bubblewrap must fail closed.
    let mut config = Config::load(&root).unwrap();
    config
        .execution
        .read_only_paths
        .push(PathBuf::from("/oxidize-nonexistent-path"));
    let runner = oxidize_core::process::Runner {
        config: &config.execution,
        trust_code: false,
    };
    if let Ok(out) = runner.run(
        &root,
        &[
            "python3".into(),
            "-c".into(),
            "print('MUST_NOT_RUN')".into(),
        ],
    ) {
        assert!(!out.success());
        assert!(!out.stdout.contains("MUST_NOT_RUN"));
    }
}

#[test]
#[ignore = "real PyO3 build; first cargo fetch --manifest-path examples/python-native/Cargo.toml"]
fn real_migration_rejection_resume_and_rollback() {
    let dir = project();
    let root = dir.path().join("app");
    let original = fingerprint(&root).unwrap();
    let response = fixture().join("migration-response.json");
    let mut session = Session::open(&root).unwrap();
    session
        .migrate("compute.py:count_primes", Some(&response), true, false)
        .unwrap();
    assert_eq!(session.state.current, Some(1));
    assert_eq!(session.state.attempts[0].phase, Phase::Accepted);
    assert!(session.state.attempts[0].speedup.unwrap() >= 1.0);
    session.verify(true).unwrap();
    assert_eq!(fingerprint(&root).unwrap(), original);
    let report = session.report().unwrap();
    assert!(
        fs::read_to_string(report)
            .unwrap()
            .contains("coverage: **unknown**")
    );
    session.rollback().unwrap();
    assert!(session.state.current.is_none());

    // Compilable but wrong code must be rejected by independent tests.
    let mut bad: serde_json::Value = serde_json::from_slice(&fs::read(&response).unwrap()).unwrap();
    bad["rust"] = bad["rust"]
        .as_str()
        .unwrap()
        .replace("Ok(count)", "Ok(count + 1)")
        .into();
    let bad_path = dir.path().join("bad.json");
    fs::write(&bad_path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(
        session
            .migrate("compute.py:count_primes", Some(&bad_path), true, false)
            .is_err()
    );
    assert_eq!(session.state.attempts[1].phase, Phase::Rejected);
    assert!(session.state.current.is_none());
    drop(session);
    let mut recovered = Session::open(&root).unwrap();
    recovered.resume(Some(&response), true, false).unwrap();
    assert_eq!(recovered.state.current, Some(2));
    assert_eq!(fingerprint(&root).unwrap(), original);
    assert!(
        recovered
            .workspace(Some(2))
            .parent()
            .unwrap()
            .read_dir()
            .unwrap()
            .any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("interrupted-"))
    );
    recovered.rollback().unwrap();
    assert!(recovered.state.current.is_none());
}

#[test]
#[ignore = "real PyO3 build with deterministic local HTTP provider"]
fn compiler_guided_repair_uses_diagnostics_and_real_native_build() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    let dir = project();
    let root = dir.path().join("app");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let configuration = fs::read_to_string(root.join("oxidize.toml"))
        .unwrap()
        .replace("127.0.0.1:11434", &address.to_string());
    fs::write(root.join("oxidize.toml"), configuration).unwrap();
    let response: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture().join("migration-response.json")).unwrap())
            .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let server = std::thread::spawn(move || {
        let start = Instant::now();
        while count.load(Ordering::Relaxed) < 2 && start.elapsed() < Duration::from_secs(120) {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let mut chunk = [0; 8192];
            loop {
                let n = stream.read(&mut chunk).unwrap();
                assert_ne!(n, 0);
                request.extend_from_slice(&chunk[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let call = count.fetch_add(1, Ordering::Relaxed);
            let code = if call == 0 {
                serde_json::json!({"rust": "fn broken() {\n    no_such_function();\n}\n"})
            } else {
                assert!(String::from_utf8_lossy(&request).contains("Compiler diagnostics"));
                response.clone()
            };
            let body = serde_json::json!({"choices":[{"message":{"content":code.to_string()},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":50}}).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    let mut session = Session::open(&root).unwrap();
    let result = session.migrate("compute.py:count_primes", None, true, true);
    // Engine timeout bounds this test's requests; the local server has its own deadline.
    assert!(result.is_ok(), "{result:?}");
    server.join().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(session.state.attempts[0].repaired, 1);
    assert_eq!(session.state.attempts[0].generations.len(), 2);
    assert_eq!(
        session.state.attempts[0].generations[1].usage.prompt_tokens,
        Some(100)
    );
}
