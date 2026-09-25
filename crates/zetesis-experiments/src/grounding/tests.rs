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
fn failed_initialization_retains_support_work() {
    let mut config = Configuration::default();
    // Complete support work before refusing the first formula node. A tiny
    // work limit can stop in an operation outside the selected event counters.
    config.formula.theory.max_nodes = 0;
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
    assert_eq!(
        sample.phases.last().unwrap().phase,
        GroundingPhase::FormulaInitialization
    );
    assert!(
        sample
            .phases
            .iter()
            .any(|phase| phase.phase == GroundingPhase::SupportCompletion
                && phase.work.support_rounds.is_some_and(|work| work > 0))
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

#[test]
fn changed_source_is_refused_before_measurement() {
    let config = Configuration::default();
    let bundle = SourceBundle::load(source("identity.lp"), config.bundle).unwrap();
    let reference = super::admit(bundle, &config, None).unwrap();
    let mut report = super::Report::new(&config).unwrap();
    let error = super::sample(
        &source("values.lp"),
        &reference,
        &mut report,
        0,
        Mode::Unobserved,
    )
    .unwrap_err();
    assert!(matches!(error, Error::SourceChanged));
    assert!(report.samples.is_empty());
}

#[test]
fn stale_model_reference_retains_the_failed_sample() {
    let config = Configuration::default();
    let bundle = SourceBundle::load(source("identity.lp"), config.bundle).unwrap();
    let reference = super::admit(bundle, &config, None).unwrap();
    let mut report = super::Report::new(&config).unwrap();
    let (mut models, failure) = super::semantic::enumerate(&reference, &config);
    assert!(failure.is_none());
    models.interpretations.pop();
    report.qualification = Some(models);
    let error = super::sample(
        &source("identity.lp"),
        &reference,
        &mut report,
        0,
        Mode::Detailed,
    )
    .unwrap_err();
    assert!(matches!(error, Error::ModelsChanged));
    assert_eq!(report.samples.len(), 1);
    assert_eq!(report.samples[0].subject_equal, Some(true));
    let observed = report.samples[0].models.as_ref().unwrap();
    assert!(observed.exhausted);
    assert_eq!(observed.interpretations.len(), 2);
}

#[test]
fn nested_grounding_boundaries_refuse_complete_capture() {
    let observer = Observer::new(Mode::Boundary, 0).unwrap();
    observer.enter();
    observer.enter();
    observer.exit();
    assert_eq!(observer.finish().2, Some(CaptureRefusal::Callbacks));
}

#[test]
fn wrong_phase_exit_retains_the_attribution_refusal() {
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    observer.enter();
    observer.phase_enter(GroundingPhase::RuleInstantiation, None);
    observer.phase_exit(
        GroundingPhase::SupportCompletion,
        None,
        GroundingOutcome::Completed,
        GroundingWork::default(),
    );
    observer.exit();
    let (_, records, refusal) = observer.finish();
    assert_eq!(records.len(), 1);
    assert_eq!(refusal, Some(CaptureRefusal::Callbacks));
}

#[test]
fn unmatched_phase_exit_cannot_fabricate_a_record() {
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    observer.phase_exit(
        GroundingPhase::RuleInstantiation,
        None,
        GroundingOutcome::Completed,
        GroundingWork::default(),
    );
    let (_, records, refusal) = observer.finish();
    assert!(records.is_empty());
    assert_eq!(refusal, Some(CaptureRefusal::Callbacks));
}

#[test]
fn nested_phase_refusal_survives_later_record_limits() {
    let observer = Observer::new(Mode::Detailed, 0).unwrap();
    observer.phase_enter(GroundingPhase::RuleInstantiation, None);
    observer.phase_enter(GroundingPhase::SupportCompletion, None);
    observer.phase_exit(
        GroundingPhase::SupportCompletion,
        None,
        GroundingOutcome::Completed,
        GroundingWork::default(),
    );
    assert_eq!(observer.finish().2, Some(CaptureRefusal::Callbacks));
}

#[test]
fn byte_refusal_remains_sticky_across_writer_views() {
    use std::io::Write as _;
    let mut bytes = super::storage::Bytes::new(3, "test_bytes");
    bytes.write_all(b"abc").unwrap();
    assert!(std::fmt::Write::write_str(&mut bytes, "d").is_err());
    assert!(bytes.write_all(b"e").is_err());
    assert_eq!(bytes.data, b"abc");
    assert!(matches!(
        bytes.refusal,
        Some(Error::Limit {
            resource: "test_bytes",
            limit: 3
        })
    ));
}

#[test]
fn objectives_cannot_be_omitted_from_fingerprint_claims() {
    let config = Configuration::default();
    let bundle = SourceBundle::load(source("objective.lp"), config.bundle).unwrap();
    let subject = super::admit(bundle, &config, None).unwrap();
    assert_eq!(
        super::fingerprint::subject(&subject, config.capture.max_subject_bytes).unwrap(),
        super::SubjectFingerprint::Unavailable {
            reason: super::FingerprintUnavailable::Objectives
        }
    );
}

#[test]
fn unavailable_domain_work_refuses_complete_capture() {
    // Observer-only absence control; no fabricated domain execution is claimed.
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    let mut work = GroundingWork::default();
    work.domain_prepare_work = None;
    observer.enter();
    observer.phase_enter(GroundingPhase::DomainAnalysis, None);
    observer.phase_exit(
        GroundingPhase::DomainAnalysis,
        None,
        GroundingOutcome::Failed,
        work,
    );
    observer.exit();
    let (_, records, refusal) = observer.finish();
    assert_eq!(refusal, Some(CaptureRefusal::WorkUnavailable));
    assert_eq!(records[0].work.domain_prepare_work, None);
    assert_eq!(records[0].outcome, GroundingOutcome::Failed);
    let json = serde_json::to_value(records[0]).unwrap();
    assert_eq!(json["phase"], "domain_analysis");
    assert!(json["work"]["domain_prepare_work"].is_null());
}

#[test]
fn unavailable_support_work_refuses_complete_capture() {
    let observer = Observer::new(Mode::Detailed, 1).unwrap();
    let mut work = GroundingWork::default();
    work.support_construction_work = Some(23);
    work.support_production_work = Some(11);
    work.support_join_work = Some(5);
    work.support_head_work = Some(4);
    work.support_order_work = None;
    work.support_wake_work = Some(2);
    work.support_publication_work = Some(3);
    observer.enter();
    observer.phase_enter(GroundingPhase::SupportCompletion, None);
    observer.phase_exit(
        GroundingPhase::SupportCompletion,
        None,
        GroundingOutcome::Failed,
        work,
    );
    observer.exit();
    let (_, records, refusal) = observer.finish();
    assert_eq!(refusal, Some(CaptureRefusal::WorkUnavailable));
    let json = serde_json::to_value(records[0]).unwrap();
    for (name, expected) in [
        ("support_construction_work", 23),
        ("support_production_work", 11),
        ("support_join_work", 5),
        ("support_head_work", 4),
        ("support_wake_work", 2),
        ("support_publication_work", 3),
    ] {
        assert_eq!(json["work"][name], expected);
    }
    assert!(json["work"]["support_order_work"].is_null());
}

#[test]
fn unavailable_support_subdivision_refuses_complete_capture() {
    for name in ["support_join_work", "support_head_work"] {
        let observer = Observer::new(Mode::Detailed, 1).unwrap();
        let mut work = GroundingWork::default();
        if name == "support_join_work" {
            work.support_join_work = None;
        } else {
            work.support_head_work = None;
        }
        observer.enter();
        observer.phase_enter(GroundingPhase::SupportCompletion, None);
        observer.phase_exit(
            GroundingPhase::SupportCompletion,
            None,
            GroundingOutcome::Failed,
            work,
        );
        observer.exit();
        let (_, records, refusal) = observer.finish();
        assert_eq!(refusal, Some(CaptureRefusal::WorkUnavailable));
        let json = serde_json::to_value(records[0]).unwrap();
        assert!(json["work"][name].is_null());
        assert_eq!(json["work"]["support_production_work"], 0);
        assert_eq!(json["work"].as_object().unwrap().len(), 37);
    }
}
