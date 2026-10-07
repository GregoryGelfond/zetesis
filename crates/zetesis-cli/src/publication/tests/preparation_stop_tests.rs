//! A preparation operation's typed interruption is not fabricated completion.

use std::io;

use zetesis_cpu::Stop;
use zetesis_test_support::io::BoundedWriter;

use crate::phase_timing::Recorder;
use crate::presentation::Diagnostics;
use crate::{ColorMode, Completion, HumanRenderer, Interruption, RunError};

#[test]
fn preparation_stops_publish_incomplete_coverage() {
    for reason in [Stop::Cancelled, Stop::Deadline] {
        let mut output = Vec::new();
        let progress = super::interrupted(
            reason,
            &mut HumanRenderer::new(&mut output, ColorMode::Never, usize::MAX),
            &mut Diagnostics::new(io::sink(), ColorMode::Never),
            &Recorder::new(false),
        )
        .unwrap();
        let semantic = progress.semantic().unwrap();
        assert_eq!(semantic.completion(), Some(Completion::Interrupted));
        assert_eq!(
            semantic.interruption(),
            Some(Interruption::Preparation(reason))
        );
        assert_eq!(semantic.verified_models(), 0);
        assert!(semantic.subject().is_none());
        assert!(semantic.countermodel_statistics().is_none());
        assert!(semantic.formula_execution().is_none());
        assert!(semantic.lazy_execution().is_none());
        assert!(semantic.shared_execution().is_none());
        assert!(progress.publication.summary());
        assert_eq!(progress.publication.models(), 0);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("INCOMPLETE"), "{output}");
        assert!(!output.contains("Answer:"));
        assert!(!output.contains("UNSATISFIABLE"));
        assert!(!output.contains("OPTIMUM FOUND"));
    }
}

#[test]
fn output_failure_retains_the_preparation_stop() {
    let mut output = BoundedWriter::new(0);
    let failure = super::interrupted(
        Stop::Deadline,
        &mut HumanRenderer::new(&mut output, ColorMode::Never, usize::MAX),
        &mut Diagnostics::new(io::sink(), ColorMode::Never),
        &Recorder::new(false),
    )
    .err()
    .expect("reporting must preserve its failure");
    assert!(matches!(failure.cause.as_ref(), RunError::Output(_)));
    let semantic = failure.semantic().unwrap();
    assert_eq!(semantic.completion(), Some(Completion::Interrupted));
    assert_eq!(
        semantic.interruption(),
        Some(Interruption::Preparation(Stop::Deadline))
    );
    assert_eq!(semantic.verified_models(), 0);
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(!failure.publication().unwrap().summary());
    assert!(output.bytes().is_empty());
}
