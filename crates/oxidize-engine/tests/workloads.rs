//! Milestone 2 §1: whole-application workloads.
use oxidize_core::snapshot;
use oxidize_engine::{
    Session,
    workloads::{WRAPPER, WorkloadSide},
};
use std::{fs, path::Path, process::Command};

fn wrapper(command: &[&str]) -> std::process::Output {
    wrapper_with_limit(command, 1_048_576)
}
fn wrapper_with_limit(command: &[&str], stderr_limit: usize) -> std::process::Output {
    Command::new("python3")
        .args(["-I", "-c", WRAPPER])
        .arg(serde_json::to_string(command).unwrap())
        .args(["1", "3"])
        .arg(stderr_limit.to_string())
        .output()
        .unwrap()
}

#[test]
fn the_wrapper_reports_wall_cpu_and_peak_memory_of_the_real_command() {
    // About 80 MB held briefly, and some CPU work.
    let out = wrapper(&[
        "python3",
        "-c",
        "x = bytearray(80 * 1024 * 1024); sum(range(2_000_000))",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let side: WorkloadSide = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(side.wall_ns.len(), 3, "warm-up runs are not timed");
    assert!(side.wall_ns.iter().all(|w| *w > 0.0));
    if cfg!(unix) {
        let cpu = side.cpu_ns.unwrap();
        assert_eq!(cpu.len(), 3);
        assert!(cpu.iter().all(|c| *c > 0.0), "{cpu:?}");
        let peak = side.peak_rss_kib.unwrap();
        assert!(
            peak >= 80 * 1024,
            "peak RSS {peak} KiB should include the 80 MiB buffer"
        );
        assert!(side.unsupported.is_empty());
    } else {
        assert!(side.cpu_ns.is_none() && side.peak_rss_kib.is_none());
        assert!(
            !side.unsupported.is_empty(),
            "unavailable measurements must be named"
        );
    }
}

#[test]
fn a_failing_workload_is_an_error_not_a_timing() {
    let out = wrapper(&[
        "python3",
        "-c",
        "import sys; sys.stderr.write('boom'); sys.exit(4)",
    ]);
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("boom") && stderr.contains("exited 4"),
        "{stderr}"
    );
    assert!(out.stdout.is_empty());
}

#[test]
fn workloads_need_an_accepted_workspace_and_a_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("app");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python-cli");
    snapshot(&fixture.canonicalize().unwrap(), &root).unwrap();
    let mut session = Session::open(&root).unwrap();
    let error = session.run_workloads(true).unwrap_err();
    assert!(
        format!("{error:#}").contains("no accepted workspace"),
        "{error:#}"
    );
    assert!(session.state.workloads.is_empty());
    let report = fs::read_to_string(session.report().unwrap()).unwrap();
    assert!(
        report.contains("## Application workloads\n\nNone measured."),
        "{report}"
    );
}

#[test]
fn a_workload_flooding_stderr_is_stopped_at_the_output_limit_not_buffered() {
    // 200 MiB of stderr, then a normal exit. Buffering it all would hold 200 MiB in the wrapper.
    let flood = "import sys\nchunk = 'e' * 65536\nfor _ in range(3200): sys.stderr.write(chunk)\n";
    let start = std::time::Instant::now();
    let out = wrapper_with_limit(&["python3", "-c", flood], 64 * 1024);
    assert_eq!(
        out.status.code(),
        Some(3),
        "a flood is an error, not a timing"
    );
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("stderr exceeded 65536 bytes"),
        "{}",
        &stderr[stderr.len().saturating_sub(300)..]
    );
    assert!(
        out.stderr.len() < 8 * 1024,
        "only a bounded tail is kept: {} bytes",
        out.stderr.len()
    );
    assert!(
        start.elapsed() < std::time::Duration::from_secs(30),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn stderr_within_the_limit_is_fine_and_a_failure_shows_its_tail() {
    let chatty = "import sys\nsys.stderr.write('x' * 10000)\n";
    let out = wrapper_with_limit(&["python3", "-c", chatty], 64 * 1024);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let failing = "import sys\nsys.stderr.write('a' * 10000 + 'THE END')\nsys.exit(5)\n";
    let out = wrapper_with_limit(&["python3", "-c", failing], 64 * 1024);
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("THE END") && stderr.contains("exited 5"),
        "{stderr}"
    );
    assert!(out.stderr.len() < 8 * 1024);
}
