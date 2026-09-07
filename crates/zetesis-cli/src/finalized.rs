//! Finalized semantic evidence and bounded presentation outcomes.

use std::{fmt, io, sync::Arc};

use crate::{PartialReport, PhaseTimings, Report, RunError, RunFailure, SemanticOutcome};

/// Complete records accepted by the external sink, independently of membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Publication {
    pub(crate) models: usize,
    pub(crate) summary: bool,
}
impl Publication {
    /// Complete model records; a partial prefix contributes zero records.
    #[must_use]
    pub const fn models(self) -> usize {
        self.models
    }
    /// Whether the complete final summary reached the sink.
    #[must_use]
    pub const fn summary(self) -> bool {
        self.summary
    }
}

/// A successful invocation with semantic evidence independent of delivery.
#[derive(Clone, Debug)]
pub struct SolveReport {
    pub(crate) report: Report,
    pub(crate) semantic: SemanticOutcome,
    pub(crate) publication: Publication,
}
impl SolveReport {
    /// Finalized exact-membership, objective and search evidence.
    #[must_use]
    pub const fn semantic(&self) -> &SemanticOutcome {
        &self.semantic
    }
    /// External record acknowledgements, without durability claims.
    #[must_use]
    pub const fn publication(&self) -> Publication {
        self.publication
    }
    /// Compatibility report, including existing execution/timing fields.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }
    /// Discard added evidence for the original result API.
    #[must_use]
    pub fn into_report(self) -> Report {
        self.report
    }
}

/// A failure with the same finalized semantic evidence as successful execution.
/// Preparation failures can precede any semantic session. Reporting failures are
/// bounded to one diagnostics and one summary failure; neither replaces the cause.
#[derive(Debug)]
pub struct SolveFailure {
    /// Original typed cause, retained independently of later reporting failures.
    pub cause: Box<RunError>,
    /// Original compatibility evidence, when a driver was entered.
    pub partial_report: Option<Box<PartialReport>>,
    /// Attempted host timing, independently of semantic completion.
    pub phase_timings: Option<Box<PhaseTimings>>,
    /// Compatibility view of the latest secondary reporting error.
    pub secondary_output: Option<io::Error>,
    pub(crate) semantic: Option<Box<SemanticOutcome>>,
    pub(crate) publication: Option<Publication>,
    diagnostics: Option<Arc<io::Error>>,
    summary: Option<Arc<io::Error>>,
}
impl SolveFailure {
    /// Finalized evidence from the entered session, even if publication failed.
    #[must_use]
    pub fn semantic(&self) -> Option<&SemanticOutcome> {
        self.semantic.as_deref()
    }
    /// Independent acknowledgement snapshot from an entered publication adapter.
    /// Mutating compatibility fields cannot change it. Writer-free sessions and
    /// conversion of an arbitrary legacy failure provide no publication evidence.
    #[must_use]
    pub const fn publication(&self) -> Option<Publication> {
        self.publication
    }

    pub(crate) fn acknowledge_summary(&mut self) {
        self.publication
            .get_or_insert(Publication {
                models: 0,
                summary: false,
            })
            .summary = true;
        if let Some(partial) = &mut self.partial_report {
            partial.summary_published = true;
        }
    }
    /// Failure writing diagnostics/statistics, retained even if a footer also fails.
    #[must_use]
    pub fn diagnostics_failure(&self) -> Option<&io::Error> {
        self.diagnostics.as_deref()
    }
    /// Failure writing the final structured summary after an earlier cause.
    #[must_use]
    pub fn summary_failure(&self) -> Option<&io::Error> {
        self.summary.as_deref()
    }
    /// Preserve the original public failure shape while discarding added evidence.
    #[must_use]
    pub fn into_legacy(self) -> RunFailure {
        RunFailure {
            cause: self.cause,
            partial_report: self.partial_report,
            phase_timings: self.phase_timings,
            secondary_output: self.secondary_output,
        }
    }
    pub(crate) fn record_diagnostics(&mut self, error: io::Error) {
        let error = Arc::new(error);
        self.secondary_output = Some(io::Error::new(error.kind(), Arc::clone(&error)));
        self.diagnostics = Some(error);
    }
    pub(crate) fn record_summary(&mut self, error: io::Error) {
        let error = Arc::new(error);
        self.secondary_output = Some(io::Error::new(error.kind(), Arc::clone(&error)));
        self.summary = Some(error);
    }
}
impl From<RunFailure> for SolveFailure {
    fn from(failure: RunFailure) -> Self {
        Self {
            cause: failure.cause,
            partial_report: failure.partial_report,
            phase_timings: failure.phase_timings,
            secondary_output: failure.secondary_output,
            semantic: None,
            publication: None,
            diagnostics: None,
            summary: None,
        }
    }
}
impl From<RunError> for SolveFailure {
    fn from(cause: RunError) -> Self {
        RunFailure::from(cause).into()
    }
}
impl From<io::Error> for SolveFailure {
    fn from(cause: io::Error) -> Self {
        RunError::Output(cause).into()
    }
}
impl fmt::Display for SolveFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(formatter)
    }
}
impl std::error::Error for SolveFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
