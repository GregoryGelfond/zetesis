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
    /// Necessary closure-candidate restrictions, including interrupted work.
    pub candidate_statistics: Option<zetesis_cpu::CandidateStatistics>,
    /// Original candidate/reduct accounting, when that stream was initialized.
    pub countermodel_statistics: Option<zetesis_sat::Statistics>,
    /// Actual hybrid accounting, including uncommitted candidates and queued models.
    pub formula_execution: Option<crate::FormulaExecutionStatistics>,
    /// Actual lazy device work, including incomplete batch progress.
    pub lazy_execution: Option<crate::LazyExecutionStatistics>,
    /// Shared CPU source/world work, including incomplete batch progress.
    pub shared_execution: Option<crate::SharedExecutionStatistics>,
    /// Prepared independent CPU ownership receipts and any snapshot fault.
    pub query_execution: Option<crate::QueryExecutionObservation>,
    /// Whether semantic search established an optimum, independently of delivery.
    pub optimum_proved: bool,
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

/// Live driver state. Semantic evidence has one authority; acknowledgements and
/// timing describe independent external effects. Compatibility reports are
/// derived only when a consumer needs them, never used as live search state.
pub(crate) struct Progress {
    semantic: Option<crate::SemanticOutcome>,
    pub(crate) stop: Option<crate::PublicationStop>,
    pub(crate) publication: crate::Publication,
    pub(crate) phase_timings: Option<PhaseTimings>,
}

impl Progress {
    pub(crate) fn new() -> Self {
        Self {
            semantic: None,
            stop: None,
            publication: crate::Publication {
                models: 0,
                summary: false,
            },
            phase_timings: None,
        }
    }

    /// Replace the snapshot from the same session after its latest pull. A pull
    /// can establish membership or coverage even when its external consumer fails.
    pub(crate) fn apply(&mut self, semantic: crate::SemanticOutcome) {
        self.semantic = Some(semantic);
    }

    pub(crate) fn semantic(&self) -> Option<&crate::SemanticOutcome> {
        self.semantic.as_ref()
    }

    pub(crate) fn completion(&self) -> Result<Completion, RunError> {
        self.semantic()
            .and_then(crate::SemanticOutcome::completion)
            .ok_or(RunError::CompletionUnavailable)
    }

    /// Materialize the legacy successful view only from established completion.
    /// Missing coverage remains absent in failure evidence; it is never replaced
    /// with exhaustion or a logical interruption to satisfy the legacy field.
    pub(crate) fn report(&self) -> Result<Report, RunError> {
        let semantic = self.semantic().ok_or(RunError::CompletionUnavailable)?;
        let completion = self.completion()?;
        Ok(Report {
            models: self.publication.models,
            checked: semantic.candidate_progress(),
            completion,
            interruption: semantic.interruption(),
            discovered_gate_atoms: semantic.discovered_gate_atoms(),
            candidate_statistics: semantic.candidate_statistics(),
            countermodel_statistics: semantic.countermodel_statistics().copied(),
            formula_execution: semantic.formula_execution().cloned(),
            lazy_execution: semantic.lazy_execution().cloned(),
            shared_execution: semantic.shared_execution().cloned(),
            query_execution: semantic.query_execution().cloned(),
            optimum_proved: semantic.optimum_proved(),
            optimization: semantic.incumbent().cloned(),
            phase_timings: self.phase_timings,
        })
    }

    pub(crate) fn finalize(
        mut self,
    ) -> Result<crate::PublicationOutcome, crate::PublicationFailure> {
        if self.semantic.is_some()
            && let Some(stop) = self.stop.take()
        {
            let semantic = self
                .semantic
                .take()
                .ok_or_else(|| crate::PublicationFailure::from(RunError::CompletionUnavailable))?;
            return Ok(crate::PublicationOutcome::Stopped(Box::new(
                crate::StoppedPublication {
                    stop,
                    semantic,
                    publication: self.publication,
                    phase_timings: self.phase_timings,
                },
            )));
        }
        let report = match self.report() {
            Ok(report) => report,
            Err(cause) => return Err(self.fail(cause)),
        };
        match self.semantic {
            Some(semantic) => Ok(crate::PublicationOutcome::Completed(Box::new(
                crate::PublicationReport {
                    publication: self.publication,
                    semantic,
                    report,
                },
            ))),
            None => Err(self.fail(RunError::CompletionUnavailable)),
        }
    }

    pub(crate) fn fail(self, cause: RunError) -> crate::PublicationFailure {
        let semantic = self.semantic();
        let partial_report = PartialReport {
            published_models: self.publication.models,
            verified_models: semantic.map_or(0, crate::SemanticOutcome::verified_models),
            checked: semantic.map_or(0, crate::SemanticOutcome::candidate_progress),
            completion: semantic.and_then(crate::SemanticOutcome::completion),
            interruption: semantic.and_then(crate::SemanticOutcome::interruption),
            summary_published: self.publication.summary,
            discovered_gate_atoms: semantic
                .map_or(0, crate::SemanticOutcome::discovered_gate_atoms),
            candidate_statistics: semantic.and_then(crate::SemanticOutcome::candidate_statistics),
            countermodel_statistics: semantic
                .and_then(crate::SemanticOutcome::countermodel_statistics)
                .copied(),
            formula_execution: semantic
                .and_then(crate::SemanticOutcome::formula_execution)
                .cloned(),
            lazy_execution: semantic
                .and_then(crate::SemanticOutcome::lazy_execution)
                .cloned(),
            shared_execution: semantic
                .and_then(crate::SemanticOutcome::shared_execution)
                .cloned(),
            query_execution: semantic.and_then(crate::SemanticOutcome::query_execution).cloned(),
            optimum_proved: semantic.is_some_and(crate::SemanticOutcome::optimum_proved),
            optimization: semantic
                .and_then(crate::SemanticOutcome::incumbent)
                .cloned(),
        };
        let mut failure = crate::PublicationFailure::from(RunFailure {
            cause: Box::new(cause),
            phase_timings: self.phase_timings.map(Box::new),
            partial_report: Some(Box::new(partial_report)),
            secondary_output: None,
        });
        failure.semantic = self.semantic.map(Box::new);
        failure.publication = Some(self.publication);
        failure.publication_stop = self.stop.map(Box::new);
        failure
    }
}

#[cfg(test)]
#[path = "../tests/support/outcome_authority.rs"]
mod tests;
