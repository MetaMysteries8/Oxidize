//! Milestone 2 §4: cancellation (Ctrl-C) stops a running command's whole tree promptly.
//! Alone in its own test binary, because cancellation is process-wide.
#![cfg(unix)]
use oxidize_core::{ExecutionConfig, process::Runner};
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn cancelling_stops_the_running_command_and_its_descendants() {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let beat = "import time, pathlib\nwhile True:\n    pathlib.Path('beat').write_text(str(time.time_ns()))\n    time.sleep(0.05)\n";
    let program = format!(
        "import subprocess, sys, time, pathlib\nsubprocess.Popen([sys.executable, '-c', {beat:?}])\nwhile not pathlib.Path('beat').exists(): time.sleep(0.01)\npathlib.Path('ready').touch()\ntime.sleep(60)\n"
    );
    let ready = dir.path().join("ready");
    let canceller = std::thread::spawn(move || {
        while !ready.exists() {
            std::thread::sleep(Duration::from_millis(10));
        }
        oxidize_core::process::cancel();
    });
    let c = ExecutionConfig {
        timeout_seconds: 120,
        ..ExecutionConfig::default()
    };
    let start = Instant::now();
    let error = Runner {
        config: &c,
        trust_code: true,
    }
    .run(dir.path(), &["python3".into(), "-c".into(), program])
    .unwrap_err();
    canceller.join().unwrap();
    assert!(format!("{error:#}").contains("cancelled"), "{error:#}");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
    let first = fs::read_to_string(dir.path().join("beat")).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        first,
        fs::read_to_string(dir.path().join("beat")).unwrap(),
        "a descendant outlived cancellation"
    );
    // And a cancelled process refuses to start anything new.
    assert!(
        Runner {
            config: &c,
            trust_code: true,
        }
        .run(
            dir.path(),
            &["python3".into(), "-c".into(), "print(1)".into()]
        )
        .is_err()
    );
}
