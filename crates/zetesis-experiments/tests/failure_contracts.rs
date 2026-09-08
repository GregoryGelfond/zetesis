//! A failed qualification never publishes a successful experiment marker.

use std::error::Error;
use std::io::{self, Write};
use std::num::NonZeroUsize;

use clap::Parser;
use zetesis_experiments::{BenchmarkError, BenchmarkFixture, Family, Options, run};

struct RefuseAfterLines {
    allowed: usize,
    seen: usize,
    retained: Vec<u8>,
}
impl Write for RefuseAfterLines {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.seen >= self.allowed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "qualification output closed",
            ));
        }
        let count = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| index + 1);
        self.retained.extend_from_slice(&bytes[..count]);
        if bytes.get(count.wrapping_sub(1)) == Some(&b'\n') {
            self.seen += 1;
        }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn options() -> Options {
    Options::try_parse_from([
        "zetesis-bench",
        "--backend",
        "cpu",
        "--atoms",
        "1",
        "--batches",
        "1",
        "--families",
        "wide",
        "--repetitions",
        "1",
        "--workers",
        "1",
    ])
    .unwrap()
}

#[test]
fn output_failures_at_setup_and_sample_boundaries_never_publish_pass() {
    let options = options();
    let mut complete = Vec::new();
    run(&options, &mut complete).unwrap();
    let lines = String::from_utf8(complete).unwrap().lines().count();
    for allowed in 0..lines {
        let mut output = RefuseAfterLines {
            allowed,
            seen: 0,
            retained: Vec::new(),
        };
        let error = run(&options, &mut output).unwrap_err();
        let BenchmarkError::Output(source) = &error else {
            panic!("unexpected failure: {error}")
        };
        assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(
            error.source().unwrap().to_string(),
            "qualification output closed"
        );
        assert!(error.to_string().contains("qualification output closed"));
        assert!(
            !String::from_utf8(output.retained)
                .unwrap()
                .contains("status=PASS")
        );
    }
}

#[test]
fn every_dimension_limit_fails_before_output_or_device_discovery() {
    let base = options();
    let mut invalid = Vec::new();
    for kind in 0..7 {
        let mut candidate = base.clone();
        match kind {
            0 => candidate.atoms.clear(),
            1 => candidate.batches.clear(),
            2 => candidate.families.clear(),
            3 => candidate.atoms = vec![NonZeroUsize::new(4089).unwrap()],
            4 => candidate.batches = vec![NonZeroUsize::new(4097).unwrap()],
            5 => candidate.repetitions = NonZeroUsize::new(101).unwrap(),
            _ => candidate.workers = NonZeroUsize::new(65).unwrap(),
        }
        invalid.push(candidate);
    }
    for options in invalid {
        let mut output = Vec::new();
        let error = run(&options, &mut output).unwrap_err();
        assert!(matches!(error, BenchmarkError::Dimensions));
        assert!(error.source().is_none());
        assert!(
            error
                .to_string()
                .contains("at most 100 repetitions and 64 workers")
        );
        assert!(output.is_empty());
    }
    for count in [0, 4089, usize::MAX] {
        assert!(matches!(
            BenchmarkFixture::new(Family::Wide, count),
            Err(BenchmarkError::Dimensions)
        ));
    }
    let fixture = BenchmarkFixture::new(Family::Wide, 1).unwrap();
    for count in [0, 4097, usize::MAX] {
        assert!(matches!(
            fixture.seeds(count, 0),
            Err(BenchmarkError::Dimensions)
        ));
    }
}

#[test]
fn incomplete_reference_work_keeps_its_cause_and_cannot_qualify() {
    let mut options = options();
    options.max_work = 0;
    let mut output = Vec::new();
    let error = run(&options, &mut output).unwrap_err();
    assert!(matches!(error, BenchmarkError::Stop(_)));
    assert!(error.source().is_some());
    assert!(error.to_string().contains("work"));
    assert!(!String::from_utf8(output).unwrap().contains("status=PASS"));
}

#[test]
fn formula_publication_failure_preserves_completed_rows() {
    let parsed = zetesis_experiments::CommandOptions::try_parse_from([
        "zetesis-bench",
        "formula",
        "--backend",
        "cpu",
        "--atoms",
        "1",
        "--batches",
        "1",
        "--families",
        "choices",
        "--repetitions",
        "1",
        "--cpu-workers",
        "1",
    ])
    .unwrap();
    let Some(zetesis_experiments::Experiment::Formula(options)) = parsed.command else {
        panic!("formula options")
    };
    let mut complete = Vec::new();
    zetesis_experiments::run_formula(&options, &mut complete).unwrap();
    let lines = String::from_utf8(complete).unwrap().lines().count();
    for allowed in 0..lines {
        let mut output = RefuseAfterLines {
            allowed,
            seen: 0,
            retained: Vec::new(),
        };
        let error = zetesis_experiments::run_formula(&options, &mut output).unwrap_err();
        let zetesis_experiments::FormulaBenchmarkError::Output(cause) = &error else {
            panic!("unexpected failure: {error}")
        };
        assert_eq!(cause.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(
            error.source().unwrap().to_string(),
            "qualification output closed"
        );
        let retained = String::from_utf8(output.retained).unwrap();
        assert_eq!(retained.lines().count(), allowed);
        assert!(!retained.contains("status=PASS"));
    }
}
