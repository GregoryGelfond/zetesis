//! Fault injection through the additive public API; no devices are constructed.

use std::error::Error;
use std::io;

use clap::Parser;
use zetesis_algebra::Cancellation;
use zetesis_cli::{
    Completion, GroundingPhase, Interruption, Options, RunError, SolvePhase, StatisticsView,
    run_detailed, run_detailed_with_diagnostics, run_with_diagnostics,
};
use zetesis_test_support::io::{BoundedWriter, FAILED, FailAt};

fn options(oracle: &str) -> Options {
    Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--oracle",
        oracle,
        "--models",
        "0",
    ])
    .unwrap()
}

fn answer_ends(bytes: &[u8], objective: bool) -> Vec<usize> {
    let text = std::str::from_utf8(bytes).unwrap();
    let mut offset = 0;
    let mut remaining = 0;
    let mut ends = Vec::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with("Answer:") {
            remaining = if objective { 3 } else { 2 };
        }
        offset += line.len();
        if remaining != 0 {
            remaining -= 1;
            if remaining == 0 {
                ends.push(offset);
            }
        }
    }
    ends
}

#[test]
fn every_answer_prefix_counts_only_complete_publications_under_each_oracle_setting() {
    for oracle in ["auto", "countermodel"] {
        let options = options(oracle);
        let mut reference = Vec::new();
        let complete = run_detailed(
            "{a}.".into(),
            &options,
            &mut reference,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(complete.models, 2);
        let ends = answer_ends(&reference, false);
        assert_eq!(ends.len(), 2);
        let text = std::str::from_utf8(&reference).unwrap();
        let first_answer = text.find("Answer:").unwrap();
        let timing = text.find("\nTime:").unwrap();
        for capacity in first_answer..=timing {
            let mut output = BoundedWriter::new(capacity);
            let failure = run_detailed(
                "{a}.".into(),
                &options,
                &mut output,
                &Cancellation::default(),
            )
            .unwrap_err();
            assert!(
                matches!(&*failure.cause, RunError::Output(error) if error.kind() == io::ErrorKind::BrokenPipe)
            );
            assert_eq!(output.bytes(), &reference[..capacity]);
            assert!(failure.secondary_output.is_none());
            let partial = failure.partial_report.unwrap();
            let published = ends.iter().filter(|&&end| end <= capacity).count();
            assert_eq!(
                partial.published_models, published,
                "{oracle}, cut {capacity}"
            );
            assert!(partial.verified_models >= published as u64);
            assert!(!partial.summary_published);
            assert_eq!(
                partial.completion,
                (published == 2).then_some(Completion::Exhausted)
            );
            // Both settings take a formula route, whose walk reports its
            // regions even when publication fails part-way.
            assert!(
                partial.countermodel_statistics.is_some(),
                "{oracle}, cut {capacity}"
            );
        }
    }
}

#[test]
fn exhausted_objective_search_retains_all_hidden_ties_before_failed_cost_publication() {
    let mut options = options("countermodel");
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    let source = "1 {a;b} 1. #minimize {1,a:a;1,b:b}. #show.";
    let mut reference = Vec::new();
    run_detailed(
        source.into(),
        &options,
        &mut reference,
        &Cancellation::default(),
    )
    .unwrap();
    let ends = answer_ends(&reference, true);
    assert_eq!(ends.len(), 2);
    let first_answer = std::str::from_utf8(&reference)
        .unwrap()
        .find("Answer:")
        .unwrap();
    for capacity in first_answer..=ends[1] {
        let mut output = BoundedWriter::new(capacity);
        let failure = run_detailed(
            source.into(),
            &options,
            &mut output,
            &Cancellation::default(),
        )
        .unwrap_err();
        let partial = failure.partial_report.unwrap();
        assert_eq!(partial.completion, Some(Completion::Exhausted));
        assert!(!partial.summary_published);
        assert_eq!(partial.verified_models, 2);
        assert_eq!(
            partial.published_models,
            ends.iter().filter(|&&end| end <= capacity).count()
        );
        let incumbent = partial.optimization.unwrap();
        assert_eq!((incumbent.tied_models, incumbent.scored_models), (2, 2));
        assert_eq!(incumbent.score.costs(), &[(0, 1)]);
        let timings = failure.phase_timings.unwrap();
        assert!(timings.get(SolvePhase::ObservationOutput).unwrap().calls > 0);
        assert!(timings.get(SolvePhase::ExactReductMembership).is_some());
        assert_eq!(output.bytes(), &reference[..capacity]);
    }
}

#[test]
fn observation_refusal_retains_verified_membership_without_an_answer_prefix() {
    let mut config = zetesis_cli::PublicationConfig::from(&options("countermodel"));
    config.observations.max_output_bytes = 0;
    let mut output = Vec::new();
    let failure = crate::support::prepared::human(
        "a. #show seen:a.",
        &config,
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        *failure.cause,
        RunError::Observation(_) | RunError::ObservationOutputLimit { .. }
    ));
    let partial = failure.partial_report.unwrap();
    assert_eq!((partial.published_models, partial.verified_models), (0, 1));
    assert_eq!(partial.completion, None);
    assert_eq!(partial.countermodel_statistics.unwrap().stable_models, 1);
    assert!(!std::str::from_utf8(&output).unwrap().contains("Answer:"));
}

#[test]
fn early_primary_source_error_survives_secondary_statistics_failure_and_legacy_mapping() {
    let mut options = options("countermodel");
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "a :- . @".into(),
        &options,
        &mut output,
        &mut BoundedWriter::new(0),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(&*failure.cause, RunError::FormulaAdmission(_)));
    assert!(
        failure
            .source()
            .unwrap()
            .downcast_ref::<RunError>()
            .is_some()
    );
    assert!(failure.partial_report.is_none());
    assert_eq!(
        failure.secondary_output.unwrap().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        failure
            .phase_timings
            .unwrap()
            .get(SolvePhase::AdmissionMaterialization)
            .unwrap()
            .calls,
        1
    );
    assert!(!std::str::from_utf8(&output).unwrap().contains("Answer:"));
    let original = run_with_diagnostics(
        "a :- . @".into(),
        &options,
        &mut output,
        &mut BoundedWriter::new(0),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(original, RunError::FormulaAdmission(_)));
}

#[test]
fn refused_bundle_preparation_does_not_attempt_materialization() {
    let fixture = zetesis_reference_support::BundleFixture::new();
    fixture.write("entry.lp", "p(X).");
    let bundle = zetesis_themelios::SourceBundle::load(
        fixture.path("entry.lp"),
        zetesis_themelios::BundleLimits::default(),
    )
    .unwrap();
    let mut options = options("countermodel");
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    let mut output = Vec::new();
    let failure = zetesis_cli::run_bundle_detailed_with_diagnostics(
        bundle,
        &options,
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        *failure.cause,
        RunError::FormulaBundleAdmission(_)
    ));
    assert!(failure.partial_report.is_none());
    let timing = failure.phase_timings.unwrap();
    assert_eq!(
        timing
            .get(SolvePhase::AdmissionMaterialization)
            .unwrap()
            .calls,
        1
    );
    for phase in GroundingPhase::ALL {
        assert!(timing.grounding.get(phase).is_none());
    }
    assert!(timing.get(SolvePhase::ExecutionSetup).is_none());
    assert!(!std::str::from_utf8(&output).unwrap().contains("Answer:"));
}

#[test]
fn failure_of_post_summary_statistics_preserves_completed_output_and_search() {
    let mut options = options("countermodel");
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    let mut diagnostics = FailAt::new(b"Statistics:");
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "a.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(_)));
    assert!(failure.secondary_output.is_none());
    let partial = failure.partial_report.unwrap();
    assert!(partial.summary_published);
    assert_eq!(partial.completion, Some(Completion::Exhausted));
    assert_eq!((partial.published_models, partial.verified_models), (1, 1));
    assert!(
        std::str::from_utf8(&output)
            .unwrap()
            .contains("SATISFIABLE\nModels: 1\n")
    );
    assert!(failure.phase_timings.is_some());
}

#[test]
fn failed_restriction_diagnostic_retains_the_committed_restriction_and_incumbent() {
    let mut options = options("countermodel");
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    let mut diagnostics = FailAt::new(b"Objective pruning:");
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "{a}. #minimize{1:a}.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(_)));
    assert!(failure.secondary_output.is_some());
    let partial = failure.partial_report.unwrap();
    assert_eq!(partial.completion, None);
    assert_eq!(partial.published_models, 0);
    assert_eq!(partial.verified_models, 1);
    assert_eq!(
        partial
            .countermodel_statistics
            .unwrap()
            .candidate_restrictions,
        1
    );
    assert_eq!(partial.optimization.unwrap().scored_models, 1);
    assert!(
        !std::str::from_utf8(&output).unwrap().contains("Answer:"),
        "unrelated failure must not publish a retained incumbent"
    );
}

#[test]
fn a_cancelled_request_keeps_its_interruption_when_its_summary_sink_fails() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let failure = run_detailed(
        "a.".into(),
        &options("countermodel"),
        &mut FailAt::new(b"INCOMPLETE"),
        &cancellation,
    )
    .unwrap_err();
    let RunError::Output(error) = failure.cause.as_ref() else {
        panic!("summary writer remains the primary failure")
    };
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), FAILED);
    let partial = failure.partial_report.unwrap();
    assert_eq!(partial.completion, Some(Completion::Interrupted));
    assert_eq!(
        partial.interruption,
        Some(Interruption::Preparation(zetesis_algebra::Stop::Cancelled))
    );
    assert_eq!(
        (
            partial.published_models,
            partial.verified_models,
            partial.checked
        ),
        (0, 0, 0)
    );
    assert!(!partial.summary_published);
    assert!(partial.countermodel_statistics.is_none());
    assert!(partial.formula_execution.is_none());
}

#[test]
fn requested_models_survive_a_later_statistics_failure() {
    let source = "{a}. {b}.";
    let mut options = options("countermodel");
    assert_eq!(
        zetesis_cli::SolveConfig::from(&options).batch_size.get(),
        64
    );
    options.models = 2;
    options.stats = true;
    options.statistics_view = StatisticsView::Records;
    let mut diagnostics = FailAt::new(b"Statistics:");
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap_err();
    let partial = failure.partial_report.unwrap();
    // The CPU formula walk verifies the models it publishes and stops at the
    // requested count, so nothing beyond the two is verified.
    assert_eq!((partial.published_models, partial.verified_models), (2, 2));
    assert_eq!(
        partial.checked, 2,
        "only two results were consumed before the requested stop"
    );
    assert_eq!(partial.completion, Some(Completion::RequestedModels));
    assert!(partial.summary_published);
    assert_eq!(answer_ends(&output, false).len(), 2);
    assert!(
        std::str::from_utf8(&output)
            .unwrap()
            .contains("Models: 2 (answer limit reached)\n")
    );
}

#[test]
fn a_failed_timing_line_preserves_exhaustion() {
    let failure = run_detailed(
        "a.".into(),
        &options("countermodel"),
        &mut FailAt::new(b"Time"),
        &Cancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(_)));
    let partial = failure.partial_report.unwrap();
    assert_eq!(partial.completion, Some(Completion::Exhausted));
    assert_eq!((partial.published_models, partial.verified_models), (1, 1));
    assert!(!partial.summary_published);
}
