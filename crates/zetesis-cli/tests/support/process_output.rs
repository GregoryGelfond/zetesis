//! The process owns flushing; a library record acknowledgement does not.

use std::io::{self, Write};
use std::process::ExitCode;

use crate::presentation::Diagnostics;
use crate::{ColorMode, RunError, RunFailure};

use super::{OUTPUT_BUFFER_BYTES, buffered_output, finish_output};

#[derive(Default)]
struct Sink {
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    fail_write: bool,
    fail_flush: bool,
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_write {
            return Err(io::Error::other("sink write failed"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_flush {
            Err(io::Error::other("sink flush failed"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn flush_failure_overrides_success() {
    let mut sink = Sink {
        fail_flush: true,
        ..Sink::default()
    };
    let mut output = buffered_output(&mut sink, false);
    output.write_all(b"complete document").unwrap();
    let mut messages = Vec::new();
    let status = finish_output(
        output,
        Ok(ExitCode::SUCCESS),
        &mut Diagnostics::new(&mut messages, ColorMode::Never),
    );
    assert_eq!(status, ExitCode::from(2));
    assert_eq!(sink.bytes, b"complete document");
    assert_eq!(sink.flushes, 1);
    assert_eq!(
        String::from_utf8(messages).unwrap(),
        "zetesis: output flush: sink flush failed\n"
    );
}

#[test]
fn flush_failure_overrides_interruption() {
    let mut sink = Sink {
        fail_flush: true,
        ..Sink::default()
    };
    let status = finish_output(
        buffered_output(&mut sink, false),
        Ok(ExitCode::from(3)),
        &mut Diagnostics::new(io::sink(), ColorMode::Never),
    );
    assert_eq!(status, ExitCode::from(2));
    assert_eq!(sink.flushes, 1);
}

#[test]
fn final_flush_preserves_each_prior_failure() {
    let mut failure = RunFailure::from(RunError::Input(io::Error::other("original input failed")));
    failure.secondary_output = Some(io::Error::other("earlier reporting failed"));
    let mut sink = Sink {
        fail_flush: true,
        ..Sink::default()
    };
    let mut messages = Vec::new();
    let status = finish_output(
        buffered_output(&mut sink, false),
        Err(failure),
        &mut Diagnostics::new(&mut messages, ColorMode::Never),
    );
    assert_eq!(status, ExitCode::from(2));
    assert_eq!(
        String::from_utf8(messages).unwrap(),
        "zetesis: standard input ('-'): original input failed\nzetesis: secondary output: earlier reporting failed\nzetesis: output flush: sink flush failed\n"
    );
    assert_eq!(sink.flushes, 1);
}

#[test]
fn failed_buffered_write_is_not_retried_on_drop() {
    let mut sink = Sink {
        fail_write: true,
        ..Sink::default()
    };
    let mut output = buffered_output(&mut sink, false);
    output.write_all(b"pending document").unwrap();
    let status = finish_output(
        output,
        Ok(ExitCode::SUCCESS),
        &mut Diagnostics::new(io::sink(), ColorMode::Never),
    );
    assert_eq!(status, ExitCode::from(2));
    assert_eq!(sink.writes, 1);
    assert_eq!(sink.flushes, 0);
    assert!(sink.bytes.is_empty());
}

#[test]
fn successful_flush_preserves_the_selected_exit() {
    for expected in [ExitCode::SUCCESS, ExitCode::from(3)] {
        let mut sink = Sink::default();
        let status = finish_output(
            buffered_output(&mut sink, false),
            Ok(expected),
            &mut Diagnostics::new(io::sink(), ColorMode::Never),
        );
        assert_eq!(status, expected);
        assert_eq!(sink.flushes, 1);
    }
}

#[test]
fn terminal_output_is_not_delayed() {
    let mut sink = Sink::default();
    let mut output = buffered_output(&mut sink, true);
    output.write_all(b"Answer: 1\na\n").unwrap();
    assert_eq!(output.get_ref().bytes, b"Answer: 1\na\n");
    assert_eq!(output.buffer().len(), 0);
    assert_eq!(
        finish_output(
            output,
            Ok(ExitCode::SUCCESS),
            &mut Diagnostics::new(io::sink(), ColorMode::Never)
        ),
        ExitCode::SUCCESS
    );
}

#[test]
fn redirected_records_share_bounded_process_staging() {
    let mut sink = Sink::default();
    let mut output = buffered_output(&mut sink, false);
    assert_eq!(output.capacity(), OUTPUT_BUFFER_BYTES);
    for record in [b"Answer: 1\na\n", b"Answer: 2\nb\n"] {
        output.write_all(record).unwrap();
    }
    assert_eq!(output.get_ref().writes, 0);
    assert_eq!(
        finish_output(
            output,
            Ok(ExitCode::SUCCESS),
            &mut Diagnostics::new(io::sink(), ColorMode::Never)
        ),
        ExitCode::SUCCESS
    );
    assert_eq!(sink.bytes, b"Answer: 1\na\nAnswer: 2\nb\n");
    assert_eq!(sink.writes, 1);
    assert_eq!(sink.flushes, 1);
}

#[test]
fn stdin_admits_bytes_before_decoding_text() {
    let input = r#"a("é")."#.as_bytes();
    assert_eq!(super::read_text(input, input.len()).unwrap(), r#"a("é")."#);
    for limit in [0, 3, 4, input.len() - 1] {
        let error = super::read_text(input, limit).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), "source byte limit exceeded");
    }
    let invalid = super::read_text(&b"\xff"[..], 1).unwrap_err();
    assert_eq!(invalid.kind(), io::ErrorKind::InvalidData);
    assert!(invalid.to_string().contains("valid UTF-8"));
    let too_large = super::read_text(&b"\xff"[..], 0).unwrap_err();
    assert_eq!(too_large.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(super::read_text(&b""[..], 0).unwrap(), "");
}

#[test]
fn stdin_reader_failure_keeps_its_original_cause() {
    struct Failing;
    impl io::Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "reader unavailable",
            ))
        }
    }
    let error = super::read_text(Failing, 8).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "reader unavailable");
}
