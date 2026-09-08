//! Failure evidence is owned separately from successful coverage and output.

use std::{fmt, io};

use crate::{Completion, Interruption, Optimization, PhaseTimings, Report, RunError};

/// Trustworthy progress retained after a driver operation failed.
/// Counts describe completed operations, never a partially written Answer.
#[derive(Clone, Debug)]
pub struct PartialReport {
    /// Complete Answer records accepted by the output sink, including their cost.
    /// This is not a durable-storage or terminal-display guarantee.
    pub published_models: usize,
    /// Stable models from completed membership, including queued formula models
    /// and all accepted closure-batch results, even before their consumption.
    pub verified_models: u64,
    /// Closure result/control records consumed, or formula candidates proposed.
    /// A shared batch stop is one control record, not one completed candidate.
    /// A completed closure batch can verify more models than the driver consumes.
    pub checked: u64,
    /// Established search stopping classification, if the loop reached one.
    /// Exhaustion here does not mean all Answer records or summaries were written.
    pub completion: Option<Completion>,
    /// A logical interruption established before any later output failure.
    pub interruption: Option<Interruption>,
    /// Whether the entire final coverage/count summary reached the output sink.
    pub summary_published: bool,
    /// Gate tuples discovered by the closure candidate generator.
    pub discovered_gate_atoms: usize,
    /// Original candidate/reduct accounting, when that stream was initialized.
    pub countermodel_statistics: Option<zetesis_sat::Statistics>,
    /// Actual hybrid accounting, including uncommitted candidates and queued models.
    pub formula_execution: Option<crate::FormulaExecutionStatistics>,
    /// Actual lazy device work, including incomplete batch progress.
    pub lazy_execution: Option<crate::LazyExecutionStatistics>,
    /// Shared CPU source/world work, including incomplete batch progress.
    pub shared_execution: Option<crate::SharedExecutionStatistics>,
    /// Retained incumbent metadata, independent of how many ties were published.
    pub optimization: Option<Optimization>,
}

/// An original typed failure together with any independently retained progress.
/// Source/setup errors can have no semantic report but still have attempted timings.
#[derive(Debug)]
pub struct RunFailure {
    /// The original terminal failure; subsequent reporting never replaces it.
    pub cause: Box<RunError>,
    /// Evidence available before terminal failure, without inventing completion.
    pub partial_report: Option<Box<PartialReport>>,
    /// Opt-in attempted host timings, including failed work.
    pub phase_timings: Option<Box<PhaseTimings>>,
    /// A subsequent failure writing diagnostics/statistics for the original cause.
    pub secondary_output: Option<io::Error>,
}

impl RunFailure {
    /// Discard additional evidence for the original convenience API.
    #[must_use]
    pub fn into_cause(self) -> RunError {
        *self.cause
    }
}

impl fmt::Display for RunFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl std::error::Error for RunFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

impl From<RunError> for RunFailure {
    fn from(cause: RunError) -> Self {
        Self {
            cause: Box::new(cause),
            partial_report: None,
            phase_timings: None,
            secondary_output: None,
        }
    }
}

impl From<io::Error> for RunFailure {
    fn from(cause: io::Error) -> Self {
        RunError::Output(cause).into()
    }
}

/// Live driver evidence. The report's provisional completion is never exported
/// on failure; only the separately established classification is retained.
pub(crate) struct Progress {
    pub(crate) report: Report,
    pub(crate) verified_models: u64,
    pub(crate) observed_interruption: Option<Interruption>,
    pub(crate) completion: Option<Completion>,
    pub(crate) summary_published: bool,
    pub(crate) semantic: Option<crate::SemanticOutcome>,
}

impl Progress {
    pub(crate) fn new(gate_atoms: usize) -> Self {
        Self {
            report: Report {
                models: 0,
                checked: 0,
                completion: Completion::Exhausted,
                interruption: None,
                discovered_gate_atoms: gate_atoms,
                countermodel_statistics: None,
                formula_execution: None,
                lazy_execution: None,
                shared_execution: None,
                optimization: None,
                phase_timings: None,
            },
            verified_models: 0,
            observed_interruption: None,
            completion: None,
            summary_published: false,
            semantic: None,
        }
    }

    pub(crate) fn apply(&mut self, semantic: crate::SemanticOutcome) {
        self.verified_models = semantic.verified_models();
        self.completion = semantic.completion();
        self.observed_interruption = semantic.interruption();
        self.report.checked = semantic.candidate_progress();
        if let Some(completion) = semantic.completion() {
            self.report.completion = completion;
        }
        self.report.interruption = semantic.interruption();
        self.report.discovered_gate_atoms = semantic.discovered_gate_atoms();
        self.report.countermodel_statistics = semantic.countermodel_statistics().copied();
        self.report.formula_execution = semantic.formula_execution().cloned();
        self.report.lazy_execution = semantic.lazy_execution().cloned();
        self.report.shared_execution = semantic.shared_execution().cloned();
        self.report.optimization = semantic.incumbent().cloned();
        self.semantic = Some(semantic);
    }

    pub(crate) fn finalize(self) -> crate::SolveReport {
        crate::SolveReport {
            publication: crate::Publication {
                models: self.report.models,
                summary: self.summary_published,
            },
            semantic: self
                .semantic
                .expect("entered successful solve has semantic evidence"),
            report: self.report,
        }
    }

    pub(crate) fn fail(self, cause: RunError) -> crate::SolveFailure {
        let publication = crate::Publication {
            models: self.report.models,
            summary: self.summary_published,
        };
        let report = self.report;
        let mut failure = crate::SolveFailure::from(RunFailure {
            cause: Box::new(cause),
            phase_timings: report.phase_timings.map(Box::new),
            partial_report: Some(Box::new(PartialReport {
                published_models: report.models,
                verified_models: self.verified_models,
                checked: report.checked,
                completion: self.completion,
                interruption: report.interruption.or(self.observed_interruption),
                summary_published: self.summary_published,
                discovered_gate_atoms: report.discovered_gate_atoms,
                countermodel_statistics: report.countermodel_statistics,
                formula_execution: report.formula_execution,
                lazy_execution: report.lazy_execution,
                shared_execution: report.shared_execution,
                optimization: report.optimization,
            })),
            secondary_output: None,
        });
        failure.semantic = self.semantic.map(Box::new);
        failure.publication = Some(publication);
        failure
    }
}
