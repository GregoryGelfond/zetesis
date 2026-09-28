//! Typed objective refusals retain their diagnostic text and accepted byte prefix.
//!
//! These are rendering fixtures. Native bound identity and rollback under an
//! observer failure are checked separately in zetesis-solve's objective tests.

use std::io::{self, Write};

use crate::presentation::Diagnostics;
use crate::test_writer::BoundedWriter;
use crate::{ColorMode, ExecutionObservation, ExecutionObserver};

#[derive(Clone, Copy)]
enum Refusal {
    ForeignTheory,
    VariableLimit,
}

impl Refusal {
    fn observation(self) -> ExecutionObservation<'static> {
        match self {
            Self::ForeignTheory => ExecutionObservation::ObjectiveTheoryMismatch,
            Self::VariableLimit => ExecutionObservation::ObjectiveRestrictionStopped(
                zetesis_sat::Incomplete::Admission(zetesis_sat::AdmissionError::Limit {
                    resource: zetesis_sat::Resource::Variables,
                    observed: 3,
                    limit: 2,
                }),
            ),
        }
    }

    fn expected(self) -> &'static str {
        match self {
            Self::ForeignTheory => {
                "Objective pruning stopped: original theory mismatch; exact search continues\n"
            }
            Self::VariableLimit => {
                "Objective pruning stopped: SAT Variables count 3 exceeds 2; exact search continues\n"
            }
        }
    }

    fn render(self, sink: &mut impl Write) -> io::Result<()> {
        Diagnostics::new(sink, ColorMode::Never).observe(self.observation())
    }
}

fn exact_diagnostic(refusal: Refusal) {
    let mut output = Vec::new();
    refusal.render(&mut output).unwrap();
    assert_eq!(output, refusal.expected().as_bytes());
}

fn every_truncation(refusal: Refusal) {
    let reference = refusal.expected().as_bytes();
    for capacity in 0..reference.len() {
        let mut sink = BoundedWriter::new(capacity);
        let error = refusal.render(&mut sink).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), "diagnostic sink closed");
        assert_eq!(sink.bytes(), &reference[..capacity]);
    }
}

#[test]
fn foreign_theory_refusal_has_exact_diagnostic() {
    exact_diagnostic(Refusal::ForeignTheory);
}

#[test]
fn variable_limit_refusal_has_exact_diagnostic() {
    exact_diagnostic(Refusal::VariableLimit);
}

#[test]
fn foreign_theory_diagnostic_preserves_each_prefix() {
    every_truncation(Refusal::ForeignTheory);
}

#[test]
fn variable_limit_diagnostic_preserves_each_prefix() {
    every_truncation(Refusal::VariableLimit);
}
