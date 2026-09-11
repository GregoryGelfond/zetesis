//! Bounded clingo invocation, complete output decoding and model reconciliation.
//! Capture retains both original byte streams and process status independently
//! of the record-only convenience operation.
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;

use super::source_records::Records;

#[path = "source_oracle_records.rs"]
mod report;
pub(super) use report::model_records;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zetesis-source-record-oracle-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("oracle directory: {error}"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("oracle fixture cleanup");
    }
}

const ORACLE_OUTPUT_LIMIT: usize = 65_536;

/// Probe at most one byte beyond the combined ceiling, even if a child writes
/// after the last size poll. A successful read contains both complete streams.
pub(super) fn read_capture(
    stdout: impl Read,
    stderr: impl Read,
    maximum: usize,
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let output = bounded_bytes(stdout, maximum)?;
    let diagnostics = bounded_bytes(stderr, maximum - output.len())?;
    Ok((output, diagnostics))
}

fn bounded_bytes(input: impl Read, allowance: usize) -> io::Result<Vec<u8>> {
    let probe = allowance
        .checked_add(1)
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "capture limit overflow"))?;
    let mut bytes = Vec::new();
    input.take(probe).read_to_end(&mut bytes)?;
    if bytes.len() > allowance {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oracle exceeded the combined output limit",
        ));
    }
    Ok(bytes)
}

/// Invoke the unchanged source once, preserving bounded complete raw output.
/// Process completion is checked here; solver enumeration is checked when its
/// decoded output is reconciled into complete records.
pub(super) fn capture(source: &str) -> Output {
    let directory = Directory::new();
    let input = directory.0.join("case.lp");
    let output = directory.0.join("models.json");
    let errors = directory.0.join("stderr.txt");
    fs::write(&input, source).expect("original source");
    let stdout = File::create(&output).expect("oracle output");
    let stderr = File::create(&errors).expect("oracle diagnostics");
    let start = Instant::now();
    let executable = std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into());
    let mut child = Command::new(executable)
        .args(["0", "--outf=2", "--opt-mode=enum", "--warn=none"])
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().expect("output handle"))
        .stderr(stderr.try_clone().expect("diagnostic handle"))
        .spawn()
        .expect("independent clingo on PATH");
    let status = loop {
        if start.elapsed() > Duration::from_secs(5)
            || stdout
                .metadata()
                .expect("output size")
                .len()
                .saturating_add(stderr.metadata().expect("diagnostic size").len())
                > ORACLE_OUTPUT_LIMIT as u64
        {
            let _ = child.kill();
            let _ = child.wait();
            panic!("oracle exceeded time or output limit: {source}");
        }
        if let Some(status) = child.try_wait().expect("oracle status") {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (bytes, diagnostics) = read_capture(
        File::open(output).expect("oracle JSON"),
        File::open(errors).expect("oracle diagnostics"),
        ORACLE_OUTPUT_LIMIT,
    )
    .expect("bounded complete oracle capture");
    let message = std::str::from_utf8(&diagnostics).expect("UTF-8 oracle diagnostics");
    assert!(matches!(status.code(), Some(10 | 20 | 30)), "{message}");
    Output {
        status,
        stdout: bytes,
        stderr: diagnostics,
    }
}

/// Decode the complete captured standard output without losing the byte capture.
pub(super) fn output(capture: &Output) -> Json {
    serde_json::from_slice(&capture.stdout).expect("complete oracle output")
}

/// Capture and reconcile the source when only complete model records are needed.
pub(super) fn records(source: &str) -> Records {
    let captured = output(&capture(source));
    assert_eq!(captured["Solver"].as_str(), Some("clingo version 5.8.2"));
    model_records(&captured)
}
