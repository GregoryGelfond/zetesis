//! Writer-free ordinary solves over coherent, already admitted inputs.
//!
//! These sessions use the same retained engine loops as the command adapter.
//! They neither parse source nor render model text. A caller owns admission and
//! can reuse its immutable owner across sessions; each session owns fresh search
//! budgets, its worker pools, pending results and incumbent storage.

use crate::presentation::Diagnostics;
use std::{io, sync::Arc};

use zetesis_core::{GroundProgram, Model, Program};
use zetesis_cpu::Control;
use zetesis_ferraris::Theory;
use zetesis_objective::Score;
use zetesis_themelios::{
    Admitted, AdmittedBundle, AdmittedFormula, AdmittedFormulaBundle, SourceMetadata,
};

use crate::closure_session::ClosureSession;
use crate::formula_execution::Execution;
use crate::formula_session::FormulaSession;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{
    AnswerSelection, Completion, Grounder, Interruption, Oracle, PhaseTimings, RunError,
    SemanticOutcome, SolveConfig, SolveFailure,
};

/// The complete semantic representation supplied to an ordinary session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedProfile {
    /// Finite relational normal program; closure execution chooses lazy or eager.
    Relational,
    /// Coherently indexed finite formulas and their objective/observation programs.
    Formula,
    /// A complete ground graph retaining its original relational program identity.
    Ground,
}

#[derive(Clone, Copy)]
enum Prepared<'a> {
    Relational(&'a Program),
    Formula(crate::countermodel::Input<'a>),
    Ground(&'a Arc<GroundProgram>),
}

/// A coherent borrowed owner, never an independently supplied theory/atom table.
/// Constructors do no admission, grounding, or source replay. Explicit oracle
/// requests incompatible with this representation are rejected at session setup.
#[derive(Clone, Copy)]
pub struct PreparedInput<'a> {
    input: Prepared<'a>,
    metadata: Option<&'a SourceMetadata>,
}
impl<'a> PreparedInput<'a> {
    /// Reuse an admitted normal program and its source metadata.
    #[must_use]
    pub fn admitted(owner: &'a Admitted) -> Self {
        Self {
            input: Prepared::Relational(owner.program()),
            metadata: Some(owner.metadata()),
        }
    }
    /// Reuse an admitted original-source bundle without loading its files again.
    #[must_use]
    pub fn bundle(owner: &'a AdmittedBundle) -> Self {
        Self {
            input: Prepared::Relational(owner.program()),
            metadata: Some(owner.metadata()),
        }
    }
    /// Reuse a formula owner, preserving its exact atom indexing and objectives.
    #[must_use]
    pub fn formula(owner: &'a AdmittedFormula) -> Self {
        Self {
            input: Prepared::Formula(crate::countermodel::Input {
                theory: owner.theory(),
                atoms: owner.atoms(),
                objectives: owner.objectives(),
                observations: owner.metadata().observations(),
                gate_atoms: 0,
            }),
            metadata: Some(owner.metadata()),
        }
    }
    /// Reuse a formula bundle with its original metadata and objective program.
    #[must_use]
    pub fn formula_bundle(owner: &'a AdmittedFormulaBundle) -> Self {
        Self {
            input: Prepared::Formula(crate::countermodel::Input {
                theory: owner.theory(),
                atoms: owner.atoms(),
                objectives: owner.objectives(),
                observations: owner.metadata().observations(),
                gate_atoms: 0,
            }),
            metadata: Some(owner.metadata()),
        }
    }
    /// Reuse the supplied complete graph. Auto chooses eager execution; explicit
    /// lazy or countermodel requests are refused. No second graph is compiled.
    /// Ground-only models have no source-derived observation metadata.
    #[must_use]
    pub const fn ground(owner: &'a Arc<GroundProgram>) -> Self {
        Self {
            input: Prepared::Ground(owner),
            metadata: None,
        }
    }
    /// Representation used for strategy compatibility checks.
    #[must_use]
    pub const fn profile(self) -> PreparedProfile {
        match self.input {
            Prepared::Relational(_) => PreparedProfile::Relational,
            Prepared::Formula(_) => PreparedProfile::Formula,
            Prepared::Ground(_) => PreparedProfile::Ground,
        }
    }
    /// Validated source views, when the prepared owner retained them.
    #[must_use]
    pub const fn metadata(self) -> Option<&'a SourceMetadata> {
        self.metadata
    }
    pub(crate) fn subject(self) -> Subject {
        match self.input {
            Prepared::Relational(program) => Subject::Program(program.clone()),
            Prepared::Formula(input) => Subject::Theory(input.theory.clone()),
            Prepared::Ground(ground) => Subject::Program(ground.program().clone()),
        }
    }
    fn selection(self, requested: AnswerSelection) -> AnswerSelection {
        match self.input {
            Prepared::Formula(input) if input.objectives.is_present() => requested,
            _ => AnswerSelection::All,
        }
    }
    fn configure(self, mut config: SolveConfig) -> Result<SolveConfig, RunError> {
        if !matches!(self.input, Prepared::Relational(_))
            && config.source_batching != crate::SourceBatching::Independent
        {
            return Err(RunError::UnsupportedSourceBatching);
        }
        let compatible = match self.input {
            Prepared::Relational(_) => config.oracle != Oracle::Countermodel,
            Prepared::Formula(_) => {
                config.oracle != Oracle::Closure && config.grounder != Grounder::Lazy
            }
            Prepared::Ground(_) => {
                config.oracle != Oracle::Countermodel && config.grounder != Grounder::Lazy
            }
        };
        if !compatible {
            return Err(RunError::PreparedInput {
                profile: self.profile(),
                oracle: config.oracle,
                grounder: config.grounder,
            });
        }
        if matches!(self.input, Prepared::Ground(_)) {
            config.grounder = Grounder::Eager;
        }
        crate::engine::validate_combination(&config)?;
        Ok(config)
    }
}

/// Immutable identity of the original semantic subject checked by a session.
/// Handles are cheap shared owners. Equality of printed atoms is not identity;
/// use the contained program/theory's `same_instance` method.
#[derive(Clone, Debug)]
pub enum Subject {
    /// Original finite relational program, also retained by a ground graph.
    Program(Program),
    /// Original indexed finite formula theory, before candidate restrictions.
    Theory(Theory),
}
impl Subject {
    /// Whether both handles retain the same original immutable semantic instance.
    /// Structural equality of independently admitted programs is insufficient.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Program(left), Self::Program(right)) => left.same_instance(right),
            (Self::Theory(left), Self::Theory(right)) => left.same_instance(right),
            _ => false,
        }
    }
}

/// A full stable interpretation produced by the session's membership engine.
/// Private construction preserves its subject association after detachment.
/// This is semantic evidence, independently of display selection or publication.
/// It records completed native membership, not a Lean proof or enumeration
/// coverage. Cloning shares the subject and clones the full interpretation.
///
/// ```compile_fail
/// use zetesis_cli::{AnswerSet, Subject};
/// use zetesis_core::Model;
/// fn forge(subject: Subject, interpretation: Model) -> AnswerSet {
///     AnswerSet { subject, interpretation, score: None }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct AnswerSet {
    subject: Subject,
    interpretation: Model,
    score: Option<Score>,
}
impl AnswerSet {
    /// Original immutable subject used by the membership engine.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }
    /// Complete interpretation, including atoms hidden by source observations.
    #[must_use]
    pub const fn interpretation(&self) -> &Model {
        &self.interpretation
    }
    /// Completely evaluated cost, if this solve has an objective. Absence is
    /// distinct from an active objective with zero cost or no priority slots.
    /// Optimality is determined by the session outcome, never by a score alone.
    #[must_use]
    pub const fn score(&self) -> Option<&Score> {
        self.score.as_ref()
    }
    /// Discard the subject association explicitly for raw-model interoperability.
    #[must_use]
    pub fn into_interpretation(self) -> Model {
        self.interpretation
    }
}

/// Compatibility name for the checked answer produced by an ordinary session.
pub type SessionModel = AnswerSet;

enum State<'a> {
    Closure(Box<ClosureSession<'a>>),
    Formula(Box<FormulaSession<'a, Execution>>),
    Stopped(Box<SemanticOutcome>),
}

/// A pull-based ordinary solve, independent of argument parsing and writers.
/// [`Self::new`] selects objective ties after search; [`Self::enumerate`] streams
/// the unrestricted original family. `models` limits yielded answers, or
/// retained objective ties in a selected solve. Budgets persist across pulls.
/// Stopping pulls early establishes no additional coverage. The caller decides
/// how to store or publish each full interpretation; no delivery is inferred.
///
/// Enabled driver elapsed time includes time between pulls as unattributed host
/// time. Solving spans cover only setup and active pulls; disabled instrumentation
/// reads no clocks. Existing deadline control remains independent.
///
/// ```
/// use zetesis_cli::{Backend, PreparedInput, Session, SolveConfig};
/// use zetesis_cpu::Control;
/// use zetesis_themelios::{admit, AdmissionOptions};
///
/// let admitted = admit("a.".into(), AdmissionOptions::default())?;
/// let config = SolveConfig { backend: Backend::Cpu, models: 0, ..Default::default() };
/// let mut session = Session::new(PreparedInput::admitted(&admitted), config, Control::default())?;
/// let model = session.next().unwrap()?;
/// assert_eq!(model.interpretation().atoms().len(), 1);
/// assert!(session.next().is_none());
/// assert_eq!(session.outcome().unwrap().verified_models(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Session<'a> {
    state: State<'a>,
    config: SolveConfig,
    control: Control,
    phases: Recorder,
    subject: Subject,
}
impl<'a> Session<'a> {
    /// Start a new ordinary solve over a coherent prepared owner.
    ///
    /// # Errors
    /// Returns typed strategy/backend/setup failures with attempted timing.
    /// Search/control limits remain typed semantic interruptions in the outcome.
    /// After strategy validation, every prepared profile polls control before
    /// allocating worker pools or beginning static compilation. Once bounded
    /// static compilation starts, it is not preemptible; cancellation is observed
    /// at the next cooperative poll. Prepared graphs avoid compilation.
    pub fn new(
        input: PreparedInput<'a>,
        config: SolveConfig,
        control: Control,
    ) -> Result<Self, SolveFailure> {
        Self::with_selection(input, config, control, AnswerSelection::Optimal)
    }
    /// Stream the original program's answer sets, including nonoptimal answers.
    ///
    /// Objective scores are evaluated but never restrict candidates or select
    /// incumbents. Every yielded answer carries completed membership and its
    /// full interpretation. `config.models` remains a yield limit; zero requests
    /// exhaustive enumeration. Formula search work/decisions and objective work
    /// are cumulative. Per-model objective binding/key ceilings and per-candidate
    /// closure work retain [`SolveConfig`]'s resource contracts. Storage is the
    /// engine's bounded batches and one yielded
    /// answer, with no incumbent retention. The caller controls any collection.
    ///
    /// # Errors
    /// Returns the setup failures of [`Self::new`]. During iteration, scoring or
    /// search stops preserve verified accounting without claiming completeness;
    /// an answer whose scoring stopped is not yielded as a fully scored answer.
    pub fn enumerate(
        input: PreparedInput<'a>,
        config: SolveConfig,
        control: Control,
    ) -> Result<Self, SolveFailure> {
        Self::with_selection(input, config, control, AnswerSelection::All)
    }
    fn with_selection(
        input: PreparedInput<'a>,
        config: SolveConfig,
        control: Control,
        selection: AnswerSelection,
    ) -> Result<Self, SolveFailure> {
        let phases = Recorder::new(config.stats);
        let result = Self::initialize(input, config, &control, &phases, input.selection(selection));
        match result {
            Ok((state, config)) => Ok(Self {
                state,
                config,
                control,
                phases,
                subject: input.subject(),
            }),
            Err(error) => {
                let mut failure = SolveFailure::from(error);
                failure.phase_timings = phases.snapshot().map(Box::new);
                Err(failure)
            }
        }
    }
    fn initialize(
        input: PreparedInput<'a>,
        config: SolveConfig,
        control: &Control,
        phases: &Recorder,
        selection: AnswerSelection,
    ) -> Result<(State<'a>, SolveConfig), RunError> {
        let config = input.configure(config)?;
        let _solving = phases.stage(crate::SolveStage::Solving);
        if let Err(stop) = control.poll() {
            let interruption = match input.input {
                Prepared::Formula(_) => Interruption::Countermodel(stop.into()),
                Prepared::Relational(_) | Prepared::Ground(_) => Interruption::Oracle(stop),
            };
            return Ok((
                State::Stopped(Box::new(SemanticOutcome {
                    subject: Some(input.subject()),
                    selection: Some(selection),
                    verified: 0,
                    scored: 0,
                    retained: 0,
                    completion: Some(Completion::Interrupted),
                    interruption: Some(interruption),
                    optimization: None,
                    checked: 0,
                    gate_atoms: 0,
                    countermodel_statistics: None,
                    formula_execution: None,
                    lazy_execution: None,
                    shared_execution: None,
                })),
                config,
            ));
        }
        let mut diagnostics = Diagnostics::new(io::sink(), crate::ColorMode::Never);
        let state = match input.input {
            Prepared::Relational(program) => State::Closure(Box::new(ClosureSession::new(
                program,
                None,
                &config,
                &mut diagnostics,
                control,
                phases,
            )?)),
            Prepared::Ground(ground) => State::Closure(Box::new(ClosureSession::new(
                ground.program(),
                Some(Arc::clone(ground)),
                &config,
                &mut diagnostics,
                control,
                phases,
            )?)),
            Prepared::Formula(input) => {
                let execution = phases.measure(SolvePhase::ExecutionSetup, || {
                    Execution::new(&config, &mut diagnostics)
                })?;
                State::Formula(Box::new(FormulaSession::with_selection(
                    input,
                    execution,
                    &config,
                    &mut diagnostics,
                    control,
                    phases,
                    selection,
                )))
            }
        };
        Ok((state, config))
    }
    /// Final semantic evidence once search finishes, stops, or fails. An
    /// optimized outcome can be available while retained ties await delivery.
    #[must_use]
    pub fn outcome(&self) -> Option<SemanticOutcome> {
        match &self.state {
            State::Closure(state) => state.terminal().then(|| state.outcome()),
            State::Formula(state) => state.finished().then(|| state.outcome(&self.phases)),
            State::Stopped(outcome) => Some((**outcome).clone()),
        }
    }
    /// End an unfinished session and retain its current evidence. Coverage stays
    /// unavailable unless the retained engine already established a terminal state.
    #[must_use]
    pub fn stop(self) -> SemanticOutcome {
        self.snapshot()
    }
    /// Attempted host phases; absence means instrumentation was disabled.
    #[must_use]
    pub fn phase_timings(&self) -> Option<PhaseTimings> {
        self.phases.snapshot()
    }
    fn snapshot(&self) -> SemanticOutcome {
        match &self.state {
            State::Closure(state) => state.outcome(),
            State::Formula(state) => state.outcome(&self.phases),
            State::Stopped(outcome) => (**outcome).clone(),
        }
    }
}
impl Iterator for Session<'_> {
    type Item = Result<AnswerSet, SolveFailure>;
    fn next(&mut self) -> Option<Self::Item> {
        let solving = self.phases.stage(crate::SolveStage::Solving);
        let mut diagnostics = Diagnostics::new(io::sink(), crate::ColorMode::Never);
        let next = match &mut self.state {
            State::Closure(state) => state
                .next(&self.config, &mut diagnostics, &self.control, &self.phases)
                .map(|result| result.map(|model| (model, None))),
            State::Formula(state) => {
                state.next(&self.config, &mut diagnostics, &self.control, &self.phases)
            }
            State::Stopped(_) => None,
        };
        drop(solving);
        next.map(|result| match result {
            Ok((interpretation, score)) => Ok(AnswerSet {
                subject: self.subject.clone(),
                interpretation,
                score,
            }),
            Err(error) => {
                let mut failure = SolveFailure::from(error);
                failure.semantic = Some(Box::new(self.snapshot()));
                failure.phase_timings = self.phases.snapshot().map(Box::new);
                Err(failure)
            }
        })
    }
}
impl std::iter::FusedIterator for Session<'_> {}
