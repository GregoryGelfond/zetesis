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
pub struct PublicationReport {
    pub(crate) report: Report,
    pub(crate) semantic: SemanticOutcome,
    pub(crate) publication: Publication,
}
impl PublicationReport {
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
pub struct PublicationFailure {
    /// Original typed cause, retained independently of later reporting failures.
    pub cause: Box<RunError>,
    /// Original compatibility evidence, when a driver was entered.
    pub partial_report: Option<Box<PartialReport>>,
    /// Attempted host timing, independently of semantic completion.
    pub phase_timings: Option<Box<PhaseTimings>>,
    /// Compatibility view of the latest secondary reporting error.
    pub secondary_output: Option<io::Error>,
    pub(crate) subject: Option<crate::Subject>,
    pub(crate) semantic: Option<Box<SemanticOutcome>>,
    pub(crate) publication: Option<Publication>,
    pub(crate) publication_stop: Option<Box<PublicationStop>>,
    diagnostics: Option<Arc<io::Error>>,
    summary: Option<Arc<io::Error>>,
}
impl PublicationFailure {
    /// Known original input, including a prepared session's setup failure.
    /// A subject association does not establish that search started, membership
    /// completed or any coverage was obtained. Failures before source admission
    /// can have no subject.
    #[must_use]
    pub fn subject(&self) -> Option<&crate::Subject> {
        self.semantic()
            .and_then(SemanticOutcome::subject)
            .or(self.subject.as_ref())
    }
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

    /// A cooperative stop preceding this actual writer/reporting failure.
    #[must_use]
    pub fn publication_stop(&self) -> Option<&PublicationStop> {
        self.publication_stop.as_deref()
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
impl From<RunFailure> for PublicationFailure {
    fn from(failure: RunFailure) -> Self {
        Self {
            cause: failure.cause,
            partial_report: failure.partial_report,
            phase_timings: failure.phase_timings,
            secondary_output: failure.secondary_output,
            subject: None,
            semantic: None,
            publication: None,
            publication_stop: None,
            diagnostics: None,
            summary: None,
        }
    }
}
impl From<RunError> for PublicationFailure {
    fn from(cause: RunError) -> Self {
        RunFailure::from(cause).into()
    }
}
impl From<io::Error> for PublicationFailure {
    fn from(cause: io::Error) -> Self {
        RunError::Output(cause).into()
    }
}
impl fmt::Display for PublicationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(formatter)
    }
}
impl std::error::Error for PublicationFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

impl From<zetesis_solve::SolveFailure> for PublicationFailure {
    fn from(failure: zetesis_solve::SolveFailure) -> Self {
        let parts = failure.into_parts();
        let mut failure = Self::from(RunError::from(*parts.cause));
        failure.subject = parts.subject;
        failure.semantic = parts.semantic;
        failure.phase_timings = parts.phase_timings;
        failure
    }
}
impl From<zetesis_solve::SolveError> for PublicationFailure {
    fn from(error: zetesis_solve::SolveError) -> Self {
        Self::from(RunError::from(error))
    }
}

/// The operation whose cooperative control check stopped model publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationPhase {
    /// Evaluating the program's output observations over a verified model.
    Observation,
    /// Encoding a complete structured model view.
    Encoding,
    /// Preparing a complete human record or checking control before emission.
    RecordPreparation,
}

/// Cooperative publication stop, independently of semantic search coverage.
#[derive(Clone, Debug)]
pub struct PublicationStop {
    reason: zetesis_cpu::Stop,
    phase: PublicationPhase,
    observation: Option<Box<zetesis_themelios::observation::Error>>,
}
impl PublicationStop {
    /// Original control reason, without a new poll or inferred clock state.
    #[must_use]
    pub const fn reason(&self) -> zetesis_cpu::Stop {
        self.reason
    }
    /// Operation that observed the stop.
    #[must_use]
    pub const fn phase(&self) -> PublicationPhase {
        self.phase
    }
    /// Located observation evidence, including any completed observation work.
    #[must_use]
    pub fn observation(&self) -> Option<&zetesis_themelios::observation::Error> {
        self.observation.as_deref()
    }

    pub(crate) fn classify(error: RunError) -> Result<Self, RunError> {
        use zetesis_themelios::observation::{ErrorKind, ViewError};
        match error {
            RunError::PublicationStopped(reason) if cooperative(reason) => Ok(Self {
                reason,
                phase: PublicationPhase::RecordPreparation,
                observation: None,
            }),
            RunError::JsonRecord(ViewError::Stopped(reason)) if cooperative(reason) => Ok(Self {
                reason,
                phase: PublicationPhase::Encoding,
                observation: None,
            }),
            RunError::Observation(error) => match error.kind() {
                ErrorKind::Stopped(reason) if cooperative(*reason) => Ok(Self {
                    reason: *reason,
                    phase: PublicationPhase::Observation,
                    observation: Some(Box::new(error)),
                }),
                _ => Err(RunError::Observation(error)),
            },
            other => Err(other),
        }
    }
}
const fn cooperative(reason: zetesis_cpu::Stop) -> bool {
    matches!(
        reason,
        zetesis_cpu::Stop::Cancelled | zetesis_cpu::Stop::Deadline
    )
}

impl fmt::Display for PublicationStop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "publication {}: {}",
            match self.phase {
                PublicationPhase::Observation => "observation",
                PublicationPhase::Encoding => "encoding",
                PublicationPhase::RecordPreparation => "record preparation",
            },
            self.reason
        )
    }
}

/// Finalized publication either completed or stopped cooperatively.
/// Writer, encoding-resource and execution failures remain the outer `Err`.
/// Each variant retains one boxed evidence owner; inspecting it borrows that owner.
#[derive(Clone, Debug)]
pub enum PublicationOutcome {
    /// The requested publication completed, with independent search coverage.
    Completed(Box<PublicationReport>),
    /// Publication stopped without invalidating previously established semantics.
    Stopped(Box<StoppedPublication>),
}
impl PublicationOutcome {
    /// Search evidence is retained in both outcomes; delivery cannot revise it.
    #[must_use]
    pub fn semantic(&self) -> &SemanticOutcome {
        match self {
            Self::Completed(report) => report.semantic(),
            Self::Stopped(stopped) => &stopped.semantic,
        }
    }
    /// Complete record and summary acknowledgements.
    #[must_use]
    pub fn publication(&self) -> Publication {
        match self {
            Self::Completed(report) => report.publication(),
            Self::Stopped(stopped) => stopped.publication,
        }
    }
    /// A compatibility report exists only for completed publication.
    #[must_use]
    pub fn report(&self) -> Option<&Report> {
        match self {
            Self::Completed(report) => Some(report.report()),
            Self::Stopped(_) => None,
        }
    }
    /// Adapt a cooperative stop to the original finalized failure shape.
    /// This adapter does not alter the retained semantic outcome.
    ///
    /// # Errors
    /// A stopped publication becomes the legacy `PublicationStopped` cause.
    pub fn into_legacy(self) -> Result<PublicationReport, PublicationFailure> {
        match self {
            Self::Completed(report) => Ok(*report),
            Self::Stopped(stopped) => {
                let reason = stopped.stop.reason();
                let mut progress = crate::failure::Progress::new();
                progress.apply(stopped.semantic);
                progress.publication = stopped.publication;
                progress.phase_timings = stopped.phase_timings;
                progress.stop = Some(stopped.stop);
                Err(progress.fail(RunError::PublicationStopped(reason)))
            }
        }
    }
}

/// A checked semantic prefix with incomplete external publication.
#[derive(Clone, Debug)]
pub struct StoppedPublication {
    pub(crate) stop: PublicationStop,
    pub(crate) semantic: SemanticOutcome,
    pub(crate) publication: Publication,
    pub(crate) phase_timings: Option<PhaseTimings>,
}
impl StoppedPublication {
    /// The operation and original control reason that stopped delivery.
    #[must_use]
    pub const fn stop(&self) -> &PublicationStop {
        &self.stop
    }
    /// Established search and optimum evidence, independently of delivery.
    #[must_use]
    pub const fn semantic(&self) -> &SemanticOutcome {
        &self.semantic
    }
    /// Complete external acknowledgements; no partial record is counted.
    #[must_use]
    pub const fn publication(&self) -> Publication {
        self.publication
    }
    /// Attempted host timings, including interrupted publication work.
    #[must_use]
    pub const fn phase_timings(&self) -> Option<&PhaseTimings> {
        self.phase_timings.as_ref()
    }
}
