//! Process exit requires checked output delivery as well as semantic completion.

#[path = "support/spelled.rs"]
mod spelled;

use std::io::Write;
#[cfg(unix)]
use std::os::{fd::OwnedFd, unix::net::UnixStream};
use std::process::{Command, Output, Stdio};

fn run(source: Option<&[u8]>, args: &[&str], stdout: Stdio) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(args)
        .stdin(if source.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(source) = source {
        child.stdin.take().unwrap().write_all(source).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn solve(source: &[u8], json: bool, limited: bool, stdout: Stdio) -> Output {
    let mut args = vec![
        "solve",
        "-",
        "--backend",
        "cpu",
        "--threads",
        "1",
        "--all",
        "--color",
        "never",
    ];
    if json {
        args.push("--json");
    }
    if limited {
        args.extend(["--oracle", "countermodel", "--max-search-work", "1"]);
    }
    run(Some(source), &args, stdout)
}

#[test]
fn invalid_stdin_identifies_its_source() {
    let output = solve(b"\xff", false, false, Stdio::piped());
    assert_eq!(output.status.code(), Some(2));
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.starts_with("zetesis: standard input ('-'):"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("valid UTF-8"), "{diagnostic}");
    assert!(output.stdout.is_empty());
}

#[test]
fn missing_files_report_loading_failure() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.lp");
    let output = run(None, &[path.to_str().unwrap()], Stdio::piped());
    assert_eq!(output.status.code(), Some(2));
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.starts_with("zetesis: source loading:"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("missing.lp"), "{diagnostic}");
    assert!(output.stdout.is_empty());
}

#[test]
fn completed_unsat_output_exits_successfully() {
    for json in [false, true] {
        let output = solve(b":-.\n", json, false, Stdio::piped());
        assert_eq!(output.status.code(), Some(0));
        if json {
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["outcome"]["status"], "unsatisfiable");
            assert_eq!(document["outcome"]["completion"], "exhausted");
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("UNSATISFIABLE"), "{text}");
            assert!(text.contains("Coverage: exhausted"), "{text}");
        }
    }
}

#[test]
fn completed_model_output_reaches_the_reader() {
    for json in [false, true] {
        let output = solve(b"a.\n", json, false, Stdio::piped());
        assert_eq!(output.status.code(), Some(0));
        if json {
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["models"].as_array().unwrap().len(), 1);
            assert_eq!(
                serde_json::Value::Array(spelled::spelled(&document, &document["models"][0])),
                serde_json::json!([{"predicate": "a", "sign": "positive", "arguments": []}])
            );
            assert_eq!(document["outcome"]["completion"], "exhausted");
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("Answer: 1\na\n"), "{text}");
            assert!(text.contains("Coverage: exhausted"), "{text}");
        }
    }
}

#[test]
fn interrupted_output_exits_three() {
    for json in [false, true] {
        let output = solve(b"{a;b}.\n", json, true, Stdio::piped());
        assert_eq!(output.status.code(), Some(3));
        if json {
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["outcome"]["status"], "incomplete");
            assert_eq!(document["outcome"]["completion"], "interrupted");
            assert_eq!(document["outcome"]["coverage"], "partial");
        } else {
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("INCOMPLETE"), "{text}");
            assert!(!text.contains("UNSATISFIABLE"), "{text}");
            assert!(!text.contains("Coverage: exhausted"), "{text}");
        }
    }
}

#[cfg(unix)]
fn disabled_output() -> (UnixStream, UnixStream) {
    let (reader, writer) = UnixStream::pair().unwrap();
    // Disable the writing endpoint itself. A concurrent child inheriting a
    // reader before close-on-exec is set cannot make this endpoint writable.
    writer.shutdown(std::net::Shutdown::Write).unwrap();
    (reader, writer)
}

#[cfg(unix)]
fn failed_output() -> Stdio {
    let (_reader, writer) = disabled_output();
    OwnedFd::from(writer).into()
}

#[cfg(unix)]
#[test]
fn disabled_output_fails_with_endpoint_aliases() {
    for json in [false, true] {
        let (reader, writer) = disabled_output();
        let reader_alias = reader.try_clone().unwrap();
        let mut writer_alias = writer.try_clone().unwrap();
        assert_eq!(
            writer_alias.write_all(b"probe").unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe
        );
        let output = solve(b":-.\n", json, false, OwnedFd::from(writer).into());
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("output flush:")
        );
        // Keep every alias alive until after the actual child has finished.
        drop((reader, reader_alias, writer_alias));
    }
}

#[cfg(unix)]
#[test]
fn failed_verdict_output_exits_two() {
    for json in [false, true] {
        let output = solve(b":-.\n", json, false, failed_output());
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("output flush:")
        );
    }
}

#[cfg(unix)]
#[test]
fn failed_interruption_output_exits_two() {
    for json in [false, true] {
        let output = solve(b"{a;b}.\n", json, true, failed_output());
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("output flush:")
        );
    }
}

#[cfg(unix)]
#[test]
fn failed_refusal_output_retains_the_input_error() {
    let output = solve(b"\xff", true, false, failed_output());
    assert_eq!(output.status.code(), Some(2));
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostic.contains("valid UTF-8"), "{diagnostic}");
    assert!(diagnostic.contains("output flush:"), "{diagnostic}");
}

#[cfg(not(feature = "gpu"))]
#[test]
fn completed_device_inventory_exits_successfully() {
    let output = run(None, &["devices"], Stdio::piped());
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("CPU: available"));
    assert!(text.contains("GPU: not compiled into this build"));
}

#[cfg(all(unix, not(feature = "gpu")))]
#[test]
fn failed_device_inventory_output_exits_two() {
    let output = run(None, &["devices"], failed_output());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("output flush:")
    );
}
