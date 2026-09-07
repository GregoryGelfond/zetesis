//! Bounded child lifetime and concurrently drained, capped temporary capture.

use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Capture {
    pub(crate) status: &'static str,
    pub(crate) exit_code: Option<i32>,
    pub(crate) elapsed_ms: u128,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) fn invoke(
    executable: &Path,
    arguments: &[OsString],
    directory: &Path,
    timeout: Duration,
    output_limit: usize,
) -> Result<Capture, String> {
    let mut stdout = tempfile::tempfile().map_err(|error| error.to_string())?;
    let mut stderr = tempfile::tempfile().map_err(|error| error.to_string())?;
    let stdout_copy = stdout.try_clone().map_err(|error| error.to_string())?;
    let stderr_copy = stderr.try_clone().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let deadline = started
        .checked_add(timeout)
        .ok_or("timeout is not representable")?;
    let executable = if executable.is_absolute() || executable.components().count() == 1 {
        executable.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(executable)
    };
    let mut child = Command::new(&executable)
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn {}: {error}", executable.display()))?;
    let remaining = Arc::new(AtomicUsize::new(output_limit));
    let exceeded = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    drain(
        child.stdout.take().ok_or("missing child stdout")?,
        stdout_copy,
        remaining.clone(),
        exceeded.clone(),
        sender.clone(),
    );
    drain(
        child.stderr.take().ok_or("missing child stderr")?,
        stderr_copy,
        remaining,
        exceeded.clone(),
        sender,
    );
    let mut status = "completed";
    let exit = loop {
        if exceeded.load(Ordering::Relaxed) {
            status = "output_limit";
            let _ = child.kill();
            break child.wait().map_err(|error| error.to_string())?;
        }
        if Instant::now() >= deadline {
            status = "timeout";
            let _ = child.kill();
            break child.wait().map_err(|error| error.to_string())?;
        }
        if let Some(exit) = child.try_wait().map_err(|error| error.to_string())? {
            break exit;
        }
        thread::sleep(Duration::from_millis(5));
    };
    // Drain completion is bounded separately after killing/exiting. The command
    // operates on trusted solver executables, without an intermediate shell.
    for _ in 0..2 {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(())) => (),
            Ok(Err(error)) => return Err(format!("child capture: {error}")),
            Err(_) => return Err("child capture did not close after process termination".into()),
        }
    }
    if exceeded.load(Ordering::Relaxed) {
        status = "output_limit";
    }
    let output = read_capture(&mut stdout, status == "completed")?;
    let errors = read_capture(&mut stderr, status == "completed")?;
    Ok(Capture {
        status,
        exit_code: exit.code(),
        elapsed_ms: started.elapsed().as_millis(),
        stdout: output,
        stderr: errors,
    })
}

fn drain(
    mut input: impl Read + Send + 'static,
    mut output: std::fs::File,
    remaining: Arc<AtomicUsize>,
    exceeded: Arc<AtomicBool>,
    sender: mpsc::Sender<std::io::Result<()>>,
) {
    thread::spawn(move || {
        let result = (|| {
            let mut buffer = [0u8; 8192];
            loop {
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    return Ok(());
                }
                let previous = remaining
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |available| {
                        Some(available.saturating_sub(count))
                    })
                    .expect("the atomic update closure always returns Some");
                let retained = previous.min(count);
                output.write_all(&buffer[..retained])?;
                if retained != count {
                    exceeded.store(true, Ordering::Relaxed);
                    return Ok(());
                }
            }
        })();
        let _ = sender.send(result);
    });
}

fn read_capture(file: &mut std::fs::File, require_utf8: bool) -> Result<String, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if require_utf8 {
        String::from_utf8(bytes)
            .map_err(|error| format!("solver emitted non-UTF-8 output: {error}"))
    } else {
        // A hard byte ceiling can split a multibyte character. Such partial
        // evidence is never normalized or accepted as a solver result.
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::invoke;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn capture_limit_bounds_both_streams_and_preserves_exit_classification() {
        let captured = invoke(
            Path::new("/bin/sh"),
            &["-c".into(), "printf '%050000d' 0".into()],
            Path::new("/"),
            Duration::from_secs(2),
            128,
        )
        .unwrap();
        assert_eq!(captured.status, "output_limit");
        assert!(captured.stdout.len() + captured.stderr.len() <= 128);
    }

    #[test]
    fn timeout_kills_the_direct_child() {
        let captured = invoke(
            Path::new("/bin/sh"),
            &["-c".into(), "exec sleep 2".into()],
            Path::new("/"),
            Duration::from_millis(10),
            128,
        )
        .unwrap();
        assert_eq!(captured.status, "timeout");
        assert!(captured.elapsed_ms < 1_000);
    }
}
