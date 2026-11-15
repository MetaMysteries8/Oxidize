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
fn verification_keeps_test_writes_out_of_checkpoints_on_success_and_failure() {
    for fails in [false, true] {
        let dir = project();
        let root = dir.path().join("app");
        let outcome = if fails {
            "self.fail('intentional failure')"
        } else {
            "self.assertTrue(True)"
        };
        fs::write(root.join("test_writes.py"), format!("import unittest\nfrom pathlib import Path\nclass Writes(unittest.TestCase):\n def test_write(self):\n  Path('test-output.txt').write_text('test generated')\n  Path('compute.py').write_text('# overwritten by a test')\n  {outcome}\n")).unwrap();
        let session = Session::open(&root).unwrap();
        let expected = session.state.source_fingerprint.clone();
        assert_eq!(session.verify(true).is_err(), fails);
        assert_eq!(fingerprint(&session.current_workspace()).unwrap(), expected);
        assert!(
            session
                .directory
                .join("validation")
                .read_dir()
                .unwrap()
                .next()
                .is_none()
        );
        assert_eq!(fingerprint(&root).unwrap(), expected);
        // A second invocation must start fresh even after a test writes source.
        assert_eq!(session.verify(true).is_err(), fails);
        assert_eq!(fingerprint(&session.current_workspace()).unwrap(), expected);
    }
}

fn mutation_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    Config::write_new(dir.path()).unwrap();
    fs::write(dir.path().join("mutate.py"), "from pathlib import Path\n\ndef touch(values):\n values.append(7)\n Path('workload-output.txt').write_text('workload generated')\n if values[0] < 0:\n  raise ValueError('negative')\n return len(values)\n").unwrap();
    fs::write(
        dir.path().join("cases.json"),
        r#"[{"args":[[1]]},{"args":[],"kwargs":{"values":[2]}},{"args":[[-1]]}]"#,
    )
    .unwrap();
    dir
}

#[test]
fn workloads_preserve_checkpoints_and_capture_mutation_even_on_errors() {
    let dir = mutation_project();
    let session = Session::open(dir.path()).unwrap();
    let expected = session.state.source_fingerprint.clone();
    let evidence = session.benchmark("mutate.py:touch", true).unwrap();
    assert_eq!(fingerprint(&session.current_workspace()).unwrap(), expected);
    assert_eq!(fingerprint(dir.path()).unwrap(), expected);
    assert_eq!(
        evidence.observations[0]["arguments_after"],
        serde_json::json!({
            "args": ["list", [["list", [["int", 1], ["int", 7]]]]], "kwargs": ["dict", []]
        })
    );
    assert_eq!(
        evidence.observations[1]["arguments_after"],
        serde_json::json!({
            "args": ["list", []], "kwargs": ["dict", [[["str", "values"], ["list", [["int", 2], ["int", 7]]]]]]
        })
    );
    assert_eq!(evidence.observations[2]["error"][1], "ValueError");
    assert_eq!(
        evidence.observations[2]["arguments_after"]["args"],
        serde_json::json!(["list", [["list", [["int", -1], ["int", 7]]]]])
    );
    session.benchmark("mutate.py:touch", true).unwrap();
    assert_eq!(fingerprint(&session.current_workspace()).unwrap(), expected);
}

#[test]
fn observation_and_timing_use_separate_processes_and_fresh_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    fs::create_dir(&root).unwrap();
    Config::write_new(&root).unwrap();
    // A controlled test-owned audit file outside the project records actual child identity.
    let audit = dir.path().join("audit.jsonl");
    let audit_literal = serde_json::to_string(&audit.to_string_lossy()).unwrap();
    fs::write(root.join("record.py"), format!("import json, os\nfrom pathlib import Path\n\ndef record(values):\n with open({audit_literal}, 'a') as audit:\n  audit.write(json.dumps([os.getpid(), str(Path.cwd()), len(values)]) + '\\n')\n values.append(7)\n return len(values)\n")).unwrap();
    fs::write(root.join("cases.json"), r#"[{"args":[[1]]}]"#).unwrap();
    let session = Session::open(&root).unwrap();
    let evidence = session.benchmark("record.py:record", true).unwrap();
    assert_eq!(evidence.observations[0]["return"][1], 2);
    let records: Vec<serde_json::Value> = fs::read_to_string(audit)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let policy = &session.state.config.performance;
    assert_eq!(
        records.len(),
        1 + (policy.warmup + policy.iterations) * policy.calls_per_sample
    );
    assert_ne!(
        records[0][0], records[1][0],
        "observations and timing shared a process"
    );
    assert_ne!(
        records[0][1], records[1][1],
        "observations and timing shared a workspace"
    );
    assert!(
        records[1..]
            .iter()
            .all(|r| r[0] == records[1][0] && r[1] == records[1][1])
    );
    assert!(
        records.iter().all(|r| r[2] == 1),
        "mutation leaked into another invocation's inputs"
    );
    assert_eq!(
        fingerprint(&session.current_workspace()).unwrap(),
        session.state.source_fingerprint
    );
    assert!(
        session
            .directory
            .join("validation")
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn legacy_observations_are_readable_but_cannot_claim_mutation_equivalence() {
    let legacy: oxidize_engine::Evidence = serde_json::from_value(serde_json::json!({
        "observations": [{"return": ["int", 3], "stdout": "", "stderr": ""}],
        "import_stdout": "", "import_stderr": "",
        "benchmark": {"samples_ns": [10,11,12], "median_ns": 11, "mad_ns": 1, "successful_cases": 1, "calls_per_sample": 1}
    })).unwrap();
    assert_eq!(legacy.observation_schema_version, 1);
    assert!(!oxidize_engine::equivalent(&legacy, &legacy));
    let mut mislabeled = legacy.clone();
    mislabeled.observation_schema_version = 2;
    assert!(!oxidize_engine::equivalent(&mislabeled, &mislabeled));
}

#[test]
#[ignore = "real PyO3 mutation migration, differential rejection, replay and rollback"]
fn native_mutation_mismatch_rejects_after_passing_tests_and_resumes_correctly() {
    let source = fixture().parent().unwrap().join("python-mutation");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    snapshot(&source, &root).unwrap();
    let original = fingerprint(&root).unwrap();
    let response = source.join("migration-response.json");
    let mut bad: serde_json::Value = serde_json::from_slice(&fs::read(&response).unwrap()).unwrap();
    let code = bad["rust"].as_str().unwrap();
    assert!(code.contains("    values.append(total)?;\n"));
    bad["rust"] = code.replace("    values.append(total)?;\n", "").into();
    let bad_path = dir.path().join("bad-response.json");
    fs::write(&bad_path, serde_json::to_vec(&bad).unwrap()).unwrap();
    let mut session = Session::open(&root).unwrap();
    assert!(
        session
            .migrate("mutate.py:total_and_mark", Some(&bad_path), true, false)
            .is_err()
    );
    let rejected = &session.state.attempts[0];
    assert_eq!(rejected.phase, Phase::Rejected);
    assert!(
        rejected
            .checks
            .iter()
            .any(|(name, output)| name == "integrated tests" && output.success())
    );
    assert!(
        rejected
            .error
            .as_ref()
            .unwrap()
            .contains("case 1: argument mutation differs")
    );
    let baseline = rejected.baseline.as_ref().unwrap();
    let candidate = rejected.candidate.as_ref().unwrap();
    assert_eq!(
        baseline.observations[0]["return"],
        candidate.observations[0]["return"]
    );
    assert_ne!(
        baseline.observations[0]["arguments_after"],
        candidate.observations[0]["arguments_after"]
    );
    assert_eq!(fingerprint(&session.workspace(None)).unwrap(), original);
    let report = session.report().unwrap();
    assert!(
        fs::read_to_string(report)
            .unwrap()
            .contains("argument mutation differs")
    );
    drop(session);
    let mut resumed = Session::open(&root).unwrap();
    resumed.resume(Some(&response), true, false).unwrap();
    assert_eq!(resumed.state.current, Some(1));
    let accepted_path = resumed.current_workspace();
    let accepted = fingerprint(&accepted_path).unwrap();
    assert!(!accepted_path.join("test-output.txt").exists());
    resumed.verify(true).unwrap();
    let evidence = resumed.benchmark("mutate.py:total_and_mark", true).unwrap();
    assert_eq!(evidence.observation_schema_version, 2);
    assert_eq!(fingerprint(&accepted_path).unwrap(), accepted);
    resumed.rollback().unwrap();
    assert!(resumed.state.current.is_none());
    assert_eq!(fingerprint(&root).unwrap(), original);
    assert_eq!(fingerprint(&resumed.workspace(None)).unwrap(), original);
    resumed.verify(true).unwrap();
    assert_eq!(fingerprint(&resumed.workspace(None)).unwrap(), original);
    assert!(
        resumed
            .directory
            .join("validation")
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
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
