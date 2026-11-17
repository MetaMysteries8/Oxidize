//! Milestone 2 §4: process supervision. Every command Oxidize runs is supervised as a whole
//! process tree: when it exits, times out or is cancelled, nothing it started keeps running.
//! Resource limits the chosen backend can't enforce are refused, never silently dropped.
//!
//! Each test starts a grandchild that rewrites a heartbeat file every 50 ms, then checks the
//! file stops changing. That works the same with `--trust-code` and inside the sandbox.
#![cfg(unix)]
use oxidize_core::{
    ExecutionConfig,
    process::{Runner, bwrap_usable},
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

/// A Python program that starts a heartbeat grandchild (optionally in a new session, escaping
/// the process group) and then does `then`.
fn spawn_heartbeat(python: &str, new_session: bool, then: &str) -> Vec<String> {
    let beat = "import time, pathlib\nwhile True:\n    pathlib.Path('beat').write_text(str(time.time_ns()))\n    time.sleep(0.05)\n";
    vec![
        python.into(),
        "-c".into(),
        format!(
            "import subprocess, sys, time, pathlib\nsubprocess.Popen([sys.executable, '-c', {beat:?}], start_new_session={})\nwhile not pathlib.Path('beat').exists(): time.sleep(0.01)\n{then}\n",
            if new_session { "True" } else { "False" }
        ),
    ]
}

fn assert_heartbeat_stopped(dir: &Path, label: &str) {
    let first = fs::read_to_string(dir.join("beat")).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    let second = fs::read_to_string(dir.join("beat")).unwrap();
    assert_eq!(first, second, "{label}: a descendant outlived its command");
}

/// An interpreter under a path the sandbox mounts (a Conda `python3` is refused there).
const SANDBOX_PYTHON: &str = "/usr/bin/python3";

/// Outside `/tmp`, which the sandbox replaces with its own (see HANDOFF.md).
fn workdir() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

fn config(timeout_seconds: u64) -> ExecutionConfig {
    ExecutionConfig {
        timeout_seconds,
        ..ExecutionConfig::default()
    }
}

#[test]
fn descendants_left_behind_by_a_finished_command_are_stopped() {
    let dir = workdir();
    let c = config(30);
    let out = Runner {
        config: &c,
        trust_code: true,
    }
    .run(
        dir.path(),
        &spawn_heartbeat("python3", false, "print('done')"),
    )
    .unwrap();
    assert!(out.success(), "{}", out.stderr);
    assert_heartbeat_stopped(dir.path(), "trust-code, exited normally");
}

#[test]
fn a_timeout_stops_the_whole_tree_promptly() {
    let dir = workdir();
    let c = config(1);
    let start = Instant::now();
    let error = Runner {
        config: &c,
        trust_code: true,
    }
    .run(
        dir.path(),
        &spawn_heartbeat("python3", false, "time.sleep(60)"),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("timeout"), "{error:#}");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
    assert_heartbeat_stopped(dir.path(), "trust-code, timed out");
}

#[test]
fn inside_the_sandbox_even_a_descendant_that_left_the_process_group_is_stopped() {
    if !bwrap_usable() || !Path::new(SANDBOX_PYTHON).exists() {
        eprintln!("skipped: Bubblewrap or {SANDBOX_PYTHON} is not available on this host");
        return;
    }
    let dir = workdir();
    let c = config(30);
    let out = Runner {
        config: &c,
        trust_code: false,
    }
    .run(
        dir.path(),
        &spawn_heartbeat(SANDBOX_PYTHON, true, "print('done')"),
    )
    .unwrap();
    assert!(out.success(), "{}", out.stderr);
    assert_heartbeat_stopped(dir.path(), "bubblewrap, setsid escape");

    let dir = workdir();
    let c = config(1);
    assert!(
        Runner {
            config: &c,
            trust_code: false,
        }
        .run(
            dir.path(),
            &spawn_heartbeat(SANDBOX_PYTHON, true, "time.sleep(60)")
        )
        .is_err()
    );
    assert_heartbeat_stopped(dir.path(), "bubblewrap, setsid escape, timed out");
}

#[test]
fn limits_the_backend_cannot_enforce_are_refused_not_ignored() {
    let dir = workdir();
    for (label, c) in [
        (
            "memory_mb",
            ExecutionConfig {
                memory_mb: Some(512),
                ..ExecutionConfig::default()
            },
        ),
        (
            "pids_limit",
            ExecutionConfig {
                pids_limit: Some(64),
                ..ExecutionConfig::default()
            },
        ),
        (
            "cpus",
            ExecutionConfig {
                cpus: Some(1.0),
                ..ExecutionConfig::default()
            },
        ),
    ] {
        let mut backends = vec![true];
        if bwrap_usable() {
            backends.push(false);
        }
        for trust_code in backends {
            let error = Runner {
                config: &c,
                trust_code,
            }
            .run(
                dir.path(),
                &[SANDBOX_PYTHON.into(), "-c".into(), "print(1)".into()],
            )
            .unwrap_err();
            let text = format!("{error:#}");
            assert!(
                text.contains(label) && text.contains("only podman or docker can enforce"),
                "{label}, trust {trust_code}: {text}"
            );
        }
    }
}
