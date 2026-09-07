//! Fault injection through the additive public API; no devices are constructed.

use std::error::Error;
use std::io::{self, Write};

use clap::Parser;
use zetesis_cli::{
    Completion, Interruption, Options, RunError, SolvePhase, run_detailed,
    run_detailed_with_diagnostics, run_with_diagnostics,
};
use zetesis_cpu::Control;

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

struct Cut {
    capacity: usize,
    bytes: Vec<u8>,
}
impl Cut {
    fn at(capacity: usize) -> Self {
        Self {
            capacity,
            bytes: Vec::new(),
        }
    }
}
impl Write for Cut {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let length = bytes.len().min(self.capacity - self.bytes.len());
        if length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected output failure",
            ));
        }
        self.bytes.extend_from_slice(&bytes[..length]);
        Ok(length)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// This writer fails at a selected diagnostic record without affecting earlier records.
struct FailAt {
    marker: &'static [u8],
    bytes: Vec<u8>,
    failed: bool,
}
impl Write for FailAt {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes
            .windows(self.marker.len())
            .any(|window| window == self.marker)
        {
            self.failed = true;
        }
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected diagnostic failure",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
fn every_answer_prefix_counts_only_complete_publications_on_both_oracles() {
    for oracle in ["closure", "countermodel"] {
        let options = options(oracle);
        let mut reference = Vec::new();
        let complete =
            run_detailed("{a}.".into(), &options, &mut reference, &Control::default()).unwrap();
        assert_eq!(complete.models, 2);
        let ends = answer_ends(&reference, false);
        assert_eq!(ends.len(), 2);
        for capacity in 0..reference.len() {
            let mut output = Cut::at(capacity);
            let failure = run_detailed("{a}.".into(), &options, &mut output, &Control::default())
                .unwrap_err();
            assert!(
                matches!(&*failure.cause, RunError::Output(error) if error.kind() == io::ErrorKind::BrokenPipe)
            );
            assert_eq!(output.bytes, reference[..capacity]);
            assert!(failure.phase_timings.is_none());
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
            assert_eq!(
                partial.countermodel_statistics.is_some(),
                oracle == "countermodel"
            );
        }
    }
}

#[test]
fn exhausted_objective_search_retains_all_hidden_ties_before_failed_cost_publication() {
    let mut options = options("countermodel");
    options.stats = true;
    let source = "1 {a;b} 1. #minimize {1,a:a;1,b:b}. #show.";
    let mut reference = Vec::new();
    run_detailed(source.into(), &options, &mut reference, &Control::default()).unwrap();
    let ends = answer_ends(&reference, true);
    assert_eq!(ends.len(), 2);
    for capacity in 0..=ends[1] {
        let mut output = Cut::at(capacity);
        let failure =
            run_detailed(source.into(), &options, &mut output, &Control::default()).unwrap_err();
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
        assert_eq!(output.bytes, reference[..capacity]);
    }
}

#[test]
fn observation_refusal_retains_verified_membership_without_an_answer_prefix() {
    let mut options = options("countermodel");
    options.max_observation_bytes = 0;
    let mut output = Vec::new();
    let failure = run_detailed(
        "a. #show seen:a.".into(),
        &options,
        &mut output,
        &Control::default(),
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
    assert!(output.is_empty());
}

#[test]
fn early_primary_source_error_survives_secondary_statistics_failure_and_legacy_mapping() {
    let mut options = options("countermodel");
    options.stats = true;
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "a :- . @".into(),
        &options,
        &mut output,
        &mut Cut::at(0),
        &Control::default(),
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
    assert!(output.is_empty());
    let original = run_with_diagnostics(
        "a :- . @".into(),
        &options,
        &mut output,
        &mut Cut::at(0),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(original, RunError::FormulaAdmission(_)));
}

#[test]
fn failure_of_post_summary_statistics_preserves_completed_output_and_search() {
    let mut options = options("countermodel");
    options.stats = true;
    let mut diagnostics = FailAt {
        marker: b"Statistics:",
        bytes: Vec::new(),
        failed: false,
    };
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "a.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
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
            .contains("Coverage: exhausted")
    );
    assert!(failure.phase_timings.is_some());
}

#[test]
fn failed_restriction_diagnostic_retains_the_committed_restriction_and_incumbent() {
    let mut options = options("countermodel");
    options.stats = true;
    let mut diagnostics = FailAt {
        marker: b"Objective pruning:",
        bytes: Vec::new(),
        failed: false,
    };
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        "{a}. #minimize{1:a}.".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
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
        output.is_empty(),
        "unrelated failure must not publish a retained incumbent"
    );
}

#[test]
fn a_cancelled_request_keeps_its_interruption_when_its_summary_sink_fails() {
    let control = Control::default();
    control.cancel();
    let failure = run_detailed(
        "a.".into(),
        &options("countermodel"),
        &mut Cut::at(0),
        &control,
    )
    .unwrap_err();
    let partial = failure.partial_report.unwrap();
    assert_eq!(partial.completion, Some(Completion::Interrupted));
    assert!(matches!(
        partial.interruption,
        Some(Interruption::Countermodel(_))
    ));
    assert_eq!(
        (
            partial.published_models,
            partial.verified_models,
            partial.checked
        ),
        (0, 0, 0)
    );
    assert!(!partial.summary_published);
}

#[test]
fn an_observed_closure_candidate_stop_survives_a_buffered_answer_failure() {
    let source = "{a}. {b}.";
    let mut options = options("closure");
    options.batch_size = std::num::NonZeroUsize::new(64).unwrap();
    options.max_candidates = 3;
    let mut reference = Vec::new();
    let report =
        run_detailed(source.into(), &options, &mut reference, &Control::default()).unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert_eq!(report.models, 3);
    let first = answer_ends(&reference, false)[0];
    let mut output = Cut::at(first + 4);
    let failure =
        run_detailed(source.into(), &options, &mut output, &Control::default()).unwrap_err();
    assert!(matches!(*failure.cause, RunError::Output(_)));
    let partial = failure.partial_report.unwrap();
    assert_eq!(partial.interruption, report.interruption);
    assert!(matches!(
        partial.interruption,
        Some(Interruption::Oracle(_))
    ));
    assert_eq!((partial.published_models, partial.verified_models), (1, 3));
    assert_eq!(
        partial.completion, None,
        "buffered results were not all consumed"
    );
    assert_eq!(output.bytes, reference[..first + 4]);

    // A successfully requested stop keeps the established public Report invariant;
    // the observed candidate stop belongs to detailed failure evidence only.
    options.models = 2;
    let requested = run_detailed(
        source.into(),
        &options,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(requested.completion, Completion::RequestedModels);
    assert_eq!(requested.interruption, None);
}

#[test]
fn completed_closure_batch_membership_survives_a_later_requested_output_statistics_failure() {
    let source = "{a}. {b}.";
    let mut options = options("closure");
    options.batch_size = std::num::NonZeroUsize::new(64).unwrap();
    options.models = 2;
    options.stats = true;
    let mut diagnostics = FailAt {
        marker: b"Statistics:",
        bytes: Vec::new(),
        failed: false,
    };
    let mut output = Vec::new();
    let failure = run_detailed_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    )
    .unwrap_err();
    let partial = failure.partial_report.unwrap();
    assert_eq!((partial.published_models, partial.verified_models), (2, 4));
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
            .contains("Coverage: partial")
    );
}
