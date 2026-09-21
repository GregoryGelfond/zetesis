//! The installed command owns signal handling; the library owns bounded cleanup.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};
use zetesis_validation::process::{self, Capture, Invocation, Limits};

fn signal_owned(child: &Child) -> Result<Capture, String> {
    let arguments = ["-TERM".into(), child.id().to_string().into()];
    let (capture, pending) = process::invoke(
        Invocation {
            executable: Path::new("/bin/kill"),
            arguments: &arguments,
            directory: Path::new("/"),
        },
        Limits {
            timeout: Duration::from_secs(1),
            ..Limits::default()
        },
    )
    .map_err(|error| error.to_string())?
    .into_parts();
    if let Some(pending) = pending {
        let cleanup = pending.retry(Duration::from_secs(1));
        if let Some(pending) = cleanup.pending {
            return Err(format!(
                "signal fixture abandoned child {}",
                pending.abandon()
            ));
        }
    }
    Ok(capture)
}

fn reap(child: &mut Child, timeout: Duration) -> io::Result<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "test CLI did not stop",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn wait_for(path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while !path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    path.exists()
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

#[test]
fn terminal_signal_retains_cancelled_child_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let ready = directory.path().join("ready");
    let finished = directory.path().join("finished");
    let producer = directory.path().join("producer");
    let stdout = directory.path().join("stdout");
    let stderr = directory.path().join("stderr");
    // A bounded producer also ends if signal integration regresses. Its output
    // is capped by the CLI capture allowance; fixed-size reports go to files so
    // the owned CLI cannot block on an undrained test pipe.
    fs::write(
        &producer,
        format!(
            "#!/bin/sh\n: > {}\n/bin/sleep 3\n: > {}\n",
            quote(&ready),
            quote(&finished),
        ),
    )
    .unwrap();
    fs::set_permissions(&producer, fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["test", "backend", "--json", "--zetesis"])
        .arg(&producer)
        .args(["--timeout-seconds", "5", "--capture-bytes", "1024"])
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout).unwrap())
        .stderr(fs::File::create(&stderr).unwrap())
        .spawn()
        .unwrap();
    // Do not call wait/try_wait before signalling. The unreaped owned Child
    // reserves the CLI PID even if it exits before its producer is ready.
    let observed = wait_for(&ready, Duration::from_secs(4));
    let signal = observed.then(|| signal_owned(&child));
    let status = reap(&mut child, Duration::from_secs(3));
    if status.is_err() {
        let _ = child.kill();
        if let Err(error) = reap(&mut child, Duration::from_secs(2)) {
            panic!("test CLI {} cleanup did not complete: {error}", child.id());
        }
    }
    // On a broken-handler path, let the autonomous fixture finish before any
    // assertion, without signalling an unowned or potentially reused child PID.
    if !status.as_ref().is_ok_and(|status| status.code() == Some(1)) {
        let _ = wait_for(&finished, Duration::from_secs(4));
    }
    assert!(observed, "the fixture never reached its owned child");
    let signal = signal.unwrap().unwrap();
    assert_eq!(signal.exit().unwrap().code, Some(0), "{signal:?}");
    let status = status.unwrap();
    assert_eq!(status.code(), Some(1), "{status:?}");
    assert!(fs::metadata(&stdout).unwrap().len() <= 1024 * 1024);
    let report: serde_json::Value = serde_json::from_slice(&fs::read(stdout).unwrap()).unwrap();
    assert_eq!(report["passed"], false);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0]["decision"], "cancelled");
    let capture = &cases[0]["reference"]["capture"];
    assert_eq!(capture["stop"], "cancelled");
    assert_eq!(capture["exit"]["signal"], 9);
    assert!(capture["cleanup_failure"].is_null());
    assert!(cases[0]["reference"]["cleanup"].is_null());
    assert!(cases[0]["selected"].is_null());
}
