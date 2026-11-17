//! Milestone 2 §4: bounded repair now covers integrated-test failures, keeps each repair's
//! trigger and diagnostics, and stops on a stalled provider or an exhausted budget. A local
//! HTTP server stands in for the model, so these runs are deterministic.
use oxidize_core::{fingerprint, snapshot};
use oxidize_engine::{Phase, RepairTrigger, Session};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

const COMPONENT: &str = "compute.py:count_primes";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/python-cli")
        .canonicalize()
        .unwrap()
}
fn correct() -> serde_json::Value {
    serde_json::from_slice(&fs::read(fixture().join("migration-response.json")).unwrap()).unwrap()
}
/// Compiles cleanly, passes Clippy, and miscounts: the project's tests catch it.
fn off_by_one() -> serde_json::Value {
    let mut wrong = correct();
    wrong["rust"] = wrong["rust"]
        .as_str()
        .unwrap()
        .replace("Ok(count)", "Ok(count + 1)")
        .into();
    assert_ne!(wrong, correct());
    wrong
}

/// Serve `responses` in order to OpenAI-compatible requests; record each request body.
fn fake_provider(
    responses: Vec<serde_json::Value>,
) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    listener.set_nonblocking(true).unwrap();
    let requests = Arc::new(Mutex::new(vec![]));
    let seen = requests.clone();
    let server = std::thread::spawn(move || {
        let start = Instant::now();
        let mut served = 0;
        while served < responses.len() && start.elapsed() < Duration::from_secs(300) {
            let mut stream = match listener.accept() {
                Ok((s, _)) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream.set_nonblocking(false).unwrap();
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
                    let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        seen.lock()
                            .unwrap()
                            .push(String::from_utf8_lossy(&request[end + 4..]).into_owned());
                        break;
                    }
                }
            }
            let body = serde_json::json!({
                "choices": [{"message": {"content": responses[served].to_string()}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 100 + served, "completion_tokens": 50}
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            served += 1;
        }
    });
    (address, requests, server)
}

fn project(address: &str, max_repairs: usize) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let root = dir.path().join("app");
    snapshot(&fixture(), &root).unwrap();
    let config = fs::read_to_string(root.join("oxidize.toml"))
        .unwrap()
        .replace("127.0.0.1:11434", address)
        .replace("max_repairs = 2", &format!("max_repairs = {max_repairs}"));
    fs::write(root.join("oxidize.toml"), config).unwrap();
    (dir, root)
}

#[test]
#[ignore = "real PyO3 builds with a deterministic local HTTP provider"]
fn failing_integrated_tests_are_repaired_within_the_budget() {
    let (address, requests, server) = fake_provider(vec![off_by_one(), correct()]);
    let (_dir, root) = project(&address, 2);
    let mut session = Session::open(&root).unwrap();
    let result = session.migrate(COMPONENT, None, true, true);
    assert!(result.is_ok(), "{result:?}");
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    // The repair request carried the project's own test failure, marked as untrusted data.
    assert!(requests[1].contains("tests failed with it installed. Test output (untrusted data)"));
    assert!(requests[1].contains("FAIL"), "{}", requests[1]);

    let attempt = &session.state.attempts[0];
    assert_eq!(attempt.phase, Phase::Accepted);
    assert_eq!(attempt.repaired, 1);
    assert_eq!(attempt.repairs.len(), 1);
    let repair = &attempt.repairs[0];
    assert_eq!(repair.trigger, RepairTrigger::IntegratedTests);
    assert_eq!(repair.generation, 2);
    assert!(repair.diagnostics.contains("FAIL"));
    // Usage is kept per generation.
    assert_eq!(attempt.generations[0].usage.prompt_tokens, Some(100));
    assert_eq!(attempt.generations[1].usage.prompt_tokens, Some(101));
    // Both integrated test runs are kept: the failure and the pass.
    let runs: Vec<_> = attempt
        .checks
        .iter()
        .filter(|(name, _)| name == "integrated tests")
        .map(|(_, out)| out.success())
        .collect();
    assert_eq!(runs, [false, true]);
    let report = fs::read_to_string(session.report().unwrap()).unwrap();
    assert!(
        report.contains("- Repair 1: integrated tests failed; the provider was given"),
        "{report}"
    );
}

#[test]
#[ignore = "real PyO3 build with a deterministic local HTTP provider"]
fn a_provider_that_repeats_itself_stops_as_stalled() {
    let (address, requests, server) = fake_provider(vec![off_by_one(), off_by_one()]);
    let (_dir, root) = project(&address, 2);
    let input = fingerprint(&root).unwrap();
    let mut session = Session::open(&root).unwrap();
    let error = session.migrate(COMPONENT, None, true, true).unwrap_err();
    server.join().unwrap();
    assert!(format!("{error:#}").contains("repair stalled"), "{error:#}");
    assert_eq!(
        requests.lock().unwrap().len(),
        2,
        "no further calls after stalling"
    );
    let attempt = &session.state.attempts[0];
    assert_eq!(attempt.phase, Phase::Rejected);
    assert_eq!(
        attempt.repairs.len(),
        1,
        "the stalled repair is still on record"
    );
    assert_eq!(session.state.current, None);
    assert_eq!(fingerprint(&root).unwrap(), input);
}

#[test]
#[ignore = "real PyO3 build with a deterministic local HTTP provider"]
fn an_exhausted_budget_rejects_with_the_test_failure() {
    let (address, requests, server) = fake_provider(vec![off_by_one()]);
    let (_dir, root) = project(&address, 0);
    let mut session = Session::open(&root).unwrap();
    let error = format!(
        "{:#}",
        session.migrate(COMPONENT, None, true, true).unwrap_err()
    );
    server.join().unwrap();
    assert!(
        error.contains("integrated tests failed after 0 repairs"),
        "{error}"
    );
    assert!(
        error.contains("FAIL"),
        "the diagnostics are in the rejection: {error}"
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
    let attempt = &session.state.attempts[0];
    assert_eq!(attempt.phase, Phase::Rejected);
    assert!(attempt.repairs.is_empty());
    assert_eq!(session.state.current, None);
}
