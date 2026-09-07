//! Portable failure evidence at the public lazy source/round boundary.

use std::error::Error;

use zetesis_core::{AdmissionLimits, AtomPattern, Model, Predicate, Program, Seed, Template};
use zetesis_cpu::{Control, Stop, lazy, source};

fn atom(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

fn program() -> Program {
    Program::new(
        vec![Template::new(
            Some(atom("a")),
            vec![],
            vec![],
            vec![atom("b")],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn source_consumer_failure_preserves_its_external_cause() {
    let failure = source::scan(
        &program(),
        &Model::default(),
        source::ScanLimits::default(),
        &Control::default(),
        |_| Err(std::io::Error::other("consumer disconnected")),
    )
    .unwrap_err();
    assert_eq!(failure.statistics.bindings, 1);
    assert!(failure.to_string().contains("consumer disconnected"));
    let cause = failure.source().unwrap().source().unwrap();
    assert_eq!(
        cause.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::Other
    );
}

#[test]
fn source_interruption_preserves_its_typed_cause() {
    let failure = source::scan(
        &program(),
        &Model::default(),
        source::ScanLimits {
            max_work: 0,
            ..Default::default()
        },
        &Control::default(),
        |_| Ok::<_, std::io::Error>(()),
    )
    .unwrap_err();
    assert_eq!(failure.to_string(), Stop::WorkLimit.to_string());
    assert_eq!(
        failure
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<Stop>(),
        Some(&Stop::WorkLimit)
    );
    assert_eq!(failure.statistics.bindings, 0);
}

#[test]
fn round_execution_failure_preserves_its_external_cause() {
    let program = program();
    let failure = lazy::check_with(
        &program,
        &[Seed::new(&program, []).unwrap()],
        lazy::Limits::default(),
        &Control::default(),
        |_| Err(std::io::Error::other("execution disconnected")),
    )
    .unwrap_err();
    assert!(failure.to_string().contains("execution disconnected"));
    assert_eq!(
        failure
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .to_string(),
        "execution disconnected"
    );
    assert_eq!(failure.progress.rounds, 0);
}

#[test]
fn round_limit_does_not_establish_a_final_closure() {
    let program = program();
    let failure = lazy::check_with(
        &program,
        &[Seed::new(&program, []).unwrap()],
        lazy::Limits {
            max_rounds: 1,
            ..Default::default()
        },
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap_err();
    assert_eq!(failure.progress.rounds, 1);
    assert_eq!(failure.to_string(), Stop::WorkLimit.to_string());
    assert_eq!(
        failure
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<Stop>(),
        Some(&Stop::WorkLimit)
    );
}

#[test]
fn invalid_violation_flags_never_complete_a_batch() {
    let program = program();
    let failure = lazy::check_with(
        &program,
        &[Seed::new(&program, []).unwrap()],
        lazy::Limits::default(),
        &Control::default(),
        |chunk| {
            let mut output = lazy::evaluate(chunk)?;
            output[chunk.violation_offset()] = 2;
            Ok::<_, Stop>(output)
        },
    )
    .unwrap_err();
    assert!(matches!(failure.cause, lazy::Cause::InvalidOutput));
    assert_eq!(
        failure.to_string(),
        "lazy chunk output violated its encoded contract"
    );
    assert!(failure.source().unwrap().source().is_none());
    assert_eq!(failure.progress.rounds, 0);
}

#[test]
fn an_unoffered_head_never_becomes_a_consequence() {
    let program = program();
    let failure = lazy::check_with(
        &program,
        &[Seed::new(&program, []).unwrap()],
        lazy::Limits::default(),
        &Control::default(),
        |chunk| {
            let mut output = lazy::evaluate(chunk)?;
            let gate = chunk.records()[lazy::RECORD_HEADER_WORDS] as usize;
            output[gate / 32] |= 1 << (gate % 32);
            Ok::<_, Stop>(output)
        },
    )
    .unwrap_err();
    assert!(matches!(failure.cause, lazy::Cause::InvalidOutput));
    assert_eq!(failure.progress.rounds, 0);
}

#[test]
fn a_repeated_old_head_is_not_a_new_delta() {
    let program = program();
    let failure = lazy::check_with(
        &program,
        &[Seed::new(&program, []).unwrap()],
        lazy::Limits::default(),
        &Control::default(),
        |chunk| {
            let mut output = lazy::evaluate(chunk)?;
            let head = chunk.records()[0] as usize - 1;
            output[head / 32] |= 1 << (head % 32);
            Ok::<_, Stop>(output)
        },
    )
    .unwrap_err();
    assert!(matches!(failure.cause, lazy::Cause::InvalidOutput));
    assert_eq!(failure.progress.rounds, 1);
}

#[test]
fn empty_batches_perform_no_source_work() {
    let result = lazy::check_with(
        &program(),
        &[],
        lazy::Limits {
            max_atoms: 0,
            ..Default::default()
        },
        &Control::default(),
        |_| panic!("empty batch dispatched"),
    )
    .unwrap_or_else(|failure: lazy::Failure<Stop>| panic!("{failure}"));
    assert!(result.checks.is_empty());
    assert_eq!(result.progress, lazy::Progress::default());
}

#[test]
fn completed_checks_retain_the_original_program() {
    let program = program();
    let batch = lazy::check_with(
        &program,
        &[Seed::new(&program, []).unwrap()],
        lazy::Limits::default(),
        &Control::default(),
        lazy::evaluate,
    )
    .unwrap();
    assert!(batch.checks[0].program().same_instance(&program));
}

#[test]
fn invalid_batch_dimensions_fail_before_execution() {
    let program = program();
    let seeds = [Seed::new(&program, []).unwrap()];
    for limits in [
        lazy::Limits {
            max_candidates: 0,
            ..Default::default()
        },
        lazy::Limits {
            max_atoms: 0,
            ..Default::default()
        },
        lazy::Limits {
            max_atoms: u32::MAX as usize,
            ..Default::default()
        },
        lazy::Limits {
            max_chunk_rules: 0,
            ..Default::default()
        },
        lazy::Limits {
            max_chunk_words: lazy::RECORD_HEADER_WORDS - 1,
            ..Default::default()
        },
    ] {
        let failure = lazy::check_with(&program, &seeds, limits, &Control::default(), |_| {
            panic!("invalid dimensions dispatched")
        })
        .unwrap_err();
        assert!(matches!(
            failure.cause,
            lazy::Cause::<Stop>::Source(Stop::CarrierLimit)
        ));
        assert_eq!(failure.progress, lazy::Progress::default());
    }
}
