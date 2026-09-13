//! Real CPU command publication and the bounded writer's independent contract.

use super::{Error, Options, Output, run};
use crate::{
    Backend,
    relation_fixtures::{Family, Payload},
};
use std::{
    error::Error as _,
    io::{self, Write},
};

fn options() -> Options {
    Options {
        family: Family::Independent,
        payload: Payload::Tuple,
        rows: 5,
        queries: 2,
        backend: Backend::Cpu,
        workers: 1,
        warmups: 1,
        repetitions: 2,
    }
}

#[test]
fn command_publishes_the_complete_cpu_mask_schedule() {
    let mut bytes = Vec::new();
    run(&options(), &mut bytes).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.ends_with('\n'));
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 11);
    assert_eq!(events[0]["event"], "start");
    assert_eq!(events[0]["configuration"]["backend"], "cpu");
    assert_eq!(events[1]["event"], "subject");
    assert!(events[1]["adapter"].is_null());
    let subject = &events[1]["sha256"];
    let schedule = [("initial", 0), ("warmup", 0), ("timed", 0), ("timed", 1)];
    for (pair, (phase, repetition)) in events[2..10].chunks_exact(2).zip(schedule) {
        for (record, route) in pair.iter().zip(["scalar", "rayon"]) {
            assert_eq!(record["event"], "observation");
            assert_eq!(record["route"], route);
            assert_eq!(record["phase"], phase);
            assert_eq!(record["repetition"], repetition);
            assert_eq!(&record["subject_sha256"], subject);
            // The first authored query is unconstrained; the second is absent.
            // Five original rows require exactly the low five bits, then zero.
            assert_eq!(record["masks"], serde_json::json!([31, 0]));
            assert_eq!(record["words_per_query"], 1);
            assert!(record["device"].is_null());
        }
    }
    assert_eq!(events[10]["event"], "complete");
    assert_eq!(events[10]["observations"], 8);
}

struct RecordSink {
    permitted: usize,
    records: usize,
    bytes: Vec<u8>,
}

impl Write for RecordSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.records == self.permitted {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "record sink closed",
            ));
        }
        let written = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |position| position + 1);
        self.bytes.extend_from_slice(&bytes[..written]);
        if bytes.get(written.wrapping_sub(1)) == Some(&b'\n') {
            self.records += 1;
        }
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_failed_record_prevents_completion_publication() {
    for permitted in 0..11 {
        let mut sink = RecordSink {
            permitted,
            records: 0,
            bytes: Vec::new(),
        };
        let error = run(&options(), &mut sink).unwrap_err();
        assert!(matches!(error, Error::Output(_)));
        assert!(
            error
                .source()
                .unwrap()
                .to_string()
                .contains("record sink closed")
        );
        assert!(error.to_string().contains("record sink closed"));
        let text = std::str::from_utf8(&sink.bytes).unwrap();
        let records: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), permitted);
        assert!(records.iter().all(|record| record["event"] != "complete"));
        for record in records
            .iter()
            .filter(|record| record["event"] == "observation")
        {
            assert_eq!(record["masks"], serde_json::json!([31, 0]));
            assert!(record["device"].is_null());
        }
    }
}

#[test]
fn invalid_command_scope_leaves_the_sink_untouched() {
    let mut options = options();
    options.rows = 0;
    let mut bytes = Vec::new();
    let error = run(&options, &mut bytes).unwrap_err();
    assert!(matches!(error, Error::Configuration(_)));
    assert!(error.source().is_none());
    assert!(error.to_string().contains("rows/queries"));
    assert!(bytes.is_empty());
}

#[derive(Default)]
struct CountingSink(usize);
impl Write for CountingSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush rejected"))
    }
}

#[test]
fn report_admission_is_inclusive_at_256_mebibytes() {
    const DOCUMENTED_LIMIT: usize = 256 * 1024 * 1024;
    let block = [0; 64 * 1024];
    let mut sink = CountingSink::default();
    let mut output = Output {
        writer: &mut sink,
        bytes: 0,
    };
    for _ in 0..DOCUMENTED_LIMIT / block.len() {
        output.write_all(&block).unwrap();
    }
    assert_eq!(output.writer.0, DOCUMENTED_LIMIT);
    assert!(
        output
            .write(&[0])
            .unwrap_err()
            .to_string()
            .contains("256 MiB")
    );
    assert_eq!(output.writer.0, DOCUMENTED_LIMIT);
    assert_eq!(output.bytes, DOCUMENTED_LIMIT);
}

#[test]
fn bounded_writer_preserves_flush_failure() {
    let mut sink = CountingSink::default();
    let mut output = Output {
        writer: &mut sink,
        bytes: 0,
    };
    let error = output.flush().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "flush rejected");
}
