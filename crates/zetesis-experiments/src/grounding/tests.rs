//! Observer and failure contracts whose private boundary needs direct access.

use std::path::PathBuf;

use zetesis_themelios::{
    GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork, SourceBundle,
};

use super::{CaptureRefusal, Configuration, Error, Mode, measure, observer::Observer};

fn source(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grounding")
        .join(name)
}

#[test]
fn failed_admission_retains_phase_work() {
    let mut config = Configuration::default();
    config.formula.max_work = 1;
    let bundle = SourceBundle::load(source("arithmetic.lp"), config.bundle).unwrap();
    let (sample, admitted, _) = measure(bundle, &config, 0, Mode::Detailed).unwrap();
    assert!(matches!(admitted, Err(Error::Admission(_))));
    assert!(!sample.admitted);
    assert!(sample.admission_elapsed_ns.is_some());
    assert!(sample.grounding_elapsed_ns.is_some());
    assert_eq!(
        sample.phases.last().unwrap().outcome,
        GroundingOutcome::Failed
    );
    assert!(
        sample
            .phases
            .iter()
            .any(|phase| phase.work.expression_nodes != Some(0)
                || phase.work.join_probes != Some(0)
                || phase.work.support_rounds != Some(0))
    );
}

#[test]
fn record_limit_retains_the_callback_prefix() {
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    observer.enter();
    for phase in [
        GroundingPhase::SupportCompletion,
        GroundingPhase::ObjectiveActivation,
    ] {
        observer.phase_enter(phase, None);
        observer.phase_exit(
            phase,
            None,
            GroundingOutcome::Completed,
            GroundingWork::default(),
        );
    }
    observer.exit();
    let (_, records, refusal) = observer.finish();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].phase, GroundingPhase::SupportCompletion);
    assert_eq!(refusal, Some(CaptureRefusal::RecordLimit));
}

#[test]
fn unmatched_exit_refuses_capture() {
    let observer = Observer::new(Mode::Boundary, 0).unwrap();
    observer.exit();
    assert_eq!(observer.finish().2, Some(CaptureRefusal::Callbacks));
}

#[test]
fn active_phase_refuses_finished_capture() {
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    observer.enter();
    observer.phase_enter(GroundingPhase::SupportCompletion, None);
    assert_eq!(observer.finish().2, Some(CaptureRefusal::Callbacks));
}

#[test]
fn unavailable_work_refuses_complete_attribution() {
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    let mut work = GroundingWork::default();
    work.expression_nodes = None;
    observer.enter();
    observer.phase_enter(GroundingPhase::RuleInstantiation, None);
    observer.phase_exit(
        GroundingPhase::RuleInstantiation,
        None,
        GroundingOutcome::Completed,
        work,
    );
    observer.exit();
    let (_, records, refusal) = observer.finish();
    assert_eq!(records[0].work.expression_nodes, None);
    assert_eq!(refusal, Some(CaptureRefusal::WorkUnavailable));
}

#[test]
fn unwound_phase_retains_its_outcome() {
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    observer.enter();
    observer.phase_enter(GroundingPhase::RuleInstantiation, None);
    observer.phase_exit(
        GroundingPhase::RuleInstantiation,
        None,
        GroundingOutcome::Unwound,
        GroundingWork::default(),
    );
    observer.exit();
    let (_, records, _) = observer.finish();
    assert_eq!(records[0].outcome, GroundingOutcome::Unwound);
}

#[test]
fn diagnostic_byte_limit_is_inclusive() {
    use std::io::Write;
    let mut bytes = super::storage::Bytes::new(3, "test_bytes");
    bytes.write_all(b"abc").unwrap();
    assert!(bytes.write_all(b"d").is_err());
    assert_eq!(bytes.data, b"abc");
    assert!(matches!(
        bytes.refusal,
        Some(Error::Limit {
            resource: "test_bytes",
            limit: 3
        })
    ));
}
