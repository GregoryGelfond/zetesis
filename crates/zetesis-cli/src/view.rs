//! Typed, borrowed publication views and replaceable streaming renderers.

pub(crate) mod human;
mod json;
pub(crate) mod builtin;
pub(crate) mod session;
pub(crate) mod configuration;

pub use configuration::{BackendView, ConfigurationView, GroundingDisplay};
pub use human::HumanRenderer;
pub use json::JsonRenderer;

use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::{Limits, ModelView};

use crate::{Publication, PublicationFailure, PublicationStop, RunError, SemanticOutcome};

/// Solver and observation policy for publication of an already prepared input.
/// Renderer-specific byte limits, styling and output state belong to the chosen
/// renderer. Construction uses ordinary typed values and does not parse arguments.
#[derive(Clone, Debug)]
pub struct PublicationConfig {
    /// Exact solver execution and answer-selection limits.
    pub solve: crate::SolveConfig,
    /// Independent bounds for evaluating the displayed term channel.
    pub observations: Limits,
}

impl Default for PublicationConfig {
    fn default() -> Self {
        let resources = zetesis_solve::Resources::default();
        Self {
            solve: resources.solve_config(),
            observations: resources.observation_limits(),
        }
    }
}

impl From<&crate::Options> for PublicationConfig {
    fn from(options: &crate::Options) -> Self {
        Self {
            solve: options.into(),
            observations: options.resources().observation_limits(),
        }
    }
}

/// One verified answer and its independently evaluated display channels.
/// The controller evaluates `#show` once in the themelios observation layer.
/// Borrowed full identity remains available even when every shown channel is empty.
/// A renderer cannot retain this borrow beyond its callback; retaining its own
/// copy is an explicit consumer responsibility, not a required family buffer.
#[derive(Clone, Copy)]
pub struct AnswerView<'a> {
    pub(crate) number: usize,
    pub(crate) model: &'a ModelView<'a>,
    pub(crate) limits: Limits,
    pub(crate) observations: bool,
}

impl AnswerView<'_> {
    /// One-based publication position; failed records do not advance it.
    #[must_use]
    pub const fn number(&self) -> usize {
        self.number
    }

    /// Full interpretation, selected atoms, evaluated terms and objective score.
    #[must_use]
    pub const fn model(&self) -> &ModelView<'_> {
        self.model
    }
}

/// When a renderer needs its one terminal publication callback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SummaryStage {
    /// Preserve the human stream's summary before statistics are written.
    /// This view makes no claim about later statistics or publication success.
    SearchFinished,
    /// Include subsequent statistics failures and attempted timing snapshots.
    /// This is the default for custom renderers and the JSON document.
    #[default]
    Finalized,
}

/// Whether the renderer accepted a complete terminal record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryDelivery {
    /// The complete record reached the renderer's sink; durability is not implied.
    Accepted,
    /// No complete record was emitted, for example after a partial writer failure.
    Omitted,
}

/// Borrowed semantic evidence beside independent publication and failure evidence.
/// The semantic outcome does not acquire stronger coverage from rendering, and
/// a writer failure cannot turn an established optimum into an unproved one.
#[derive(Clone, Copy)]
pub struct PublicationView<'a> {
    pub(crate) result: Result<&'a crate::failure::Progress, &'a PublicationFailure>,
}

impl<'a> PublicationView<'a> {
    /// Established solver evidence; source/setup failures may precede a session.
    #[must_use]
    pub fn semantic(self) -> Option<&'a SemanticOutcome> {
        match self.result {
            Ok(progress) => progress.semantic(),
            Err(failure) => failure.semantic(),
        }
    }

    /// Complete records accepted before this callback, independently of coverage.
    #[must_use]
    pub fn publication(self) -> Option<Publication> {
        match self.result {
            Ok(progress) => Some(progress.publication),
            Err(failure) => failure.publication(),
        }
    }

    /// Original failure, including independently retained reporting failures.
    #[must_use]
    pub const fn failure(self) -> Option<&'a PublicationFailure> {
        match self.result {
            Ok(_) => None,
            Err(failure) => Some(failure),
        }
    }

    /// Cooperative publication stop, which does not replace semantic coverage.
    #[must_use]
    pub fn stop(self) -> Option<&'a PublicationStop> {
        match self.result {
            Ok(progress) => progress.stop.as_ref(),
            Err(failure) => failure.publication_stop(),
        }
    }

    /// Attempted host timings, available after their controller snapshot.
    #[must_use]
    pub fn phase_timings(self) -> Option<&'a crate::PhaseTimings> {
        match self.result {
            Ok(progress) => progress.phase_timings.as_ref(),
            Err(failure) => failure.phase_timings.as_deref(),
        }
    }

    /// Relational source expansion accounting, when that admission route ran.
    #[must_use]
    pub fn expansion(self) -> Option<zetesis_themelios::ExpansionUsage> {
        match self.result {
            Ok(progress) => progress.expansion,
            Err(failure) => failure.partial_report.as_ref().and_then(|p| p.expansion),
        }
    }
}

/// A replaceable view over the controller's streaming answer publication.
///
/// The renderer owns its sink and any bounded encoding state. It receives one
/// answer at a time; no `WorldView` collection or parsing of human output is
/// required. The controller alone acknowledges successful callbacks. Returning
/// an error after a partial write does not acknowledge that answer. Resource and
/// writer errors remain typed failures; cooperative control errors retain the
/// existing publication-stop classification separately from semantic coverage.
///
/// Renderer work and retained copies are the implementer's responsibility.
/// Built-in renderers preflight bounded complete records before external writes.
/// Successful callbacks mean sink acceptance, not durable storage or semantic
/// verification by the renderer. Terminal callbacks may run after cancellation
/// so that retained evidence can still be reported.
pub trait AnswerRenderer {
    /// Request coarse host stages for this view, independently of detailed
    /// statistics. The controller fixes this preference when publication starts.
    fn needs_stage_timings(&self) -> bool {
        false
    }

    /// Start one document before source admission or prepared execution.
    ///
    /// # Errors
    /// Return a typed encoding or external-output failure.
    fn begin(&mut self) -> Result<(), RunError> {
        Ok(())
    }

    /// Present configuration when an execution backend is selected.
    ///
    /// Preparation may call this again with the effective configuration.
    /// The default emits nothing, preserving machine-readable documents and
    /// custom views that do not present setup metadata.
    ///
    /// # Errors
    /// Return a typed rendering or writer failure; execution then stops before
    /// its next step, preserving any preceding semantic and publication evidence.
    fn configuration(&mut self, _view: ConfigurationView<'_>) -> Result<(), RunError> {
        Ok(())
    }

    /// Select when the terminal view is delivered; no bytes are inspected.
    /// The controller snapshots this choice once after `begin` succeeds.
    fn summary_stage(&self) -> SummaryStage {
        SummaryStage::Finalized
    }

    /// Accept one complete answer record.
    ///
    /// # Errors
    /// A refusal acknowledges no record, even if a writer accepted a prefix.
    fn answer(&mut self, view: AnswerView<'_>, cancellation: &Cancellation)
    -> Result<(), RunError>;

    /// Render the terminal view at the declared stage, at most once.
    /// `SearchFinished` is not reached when source or execution fails;
    /// `Finalized` also receives those failures after reporting is attempted.
    ///
    /// # Errors
    /// A reporting failure preserves any preceding execution or writer failure.
    fn finish(&mut self, view: PublicationView<'_>) -> Result<SummaryDelivery, RunError>;
}
