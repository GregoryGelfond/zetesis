//! Writer-free ordinary solves over coherent, already admitted inputs.
//!
//! These sessions use the same retained engine loops as the command adapter.
//! They neither parse source nor render model text. A caller owns admission and
//! can reuse its immutable owner and explicit device resources across sessions;
//! each session owns fresh search budgets, worker pools, pending results and
//! incumbent storage.

use crate::ExecutionObserver;
use crate::execution_observation::{ExecutionSink, Ignore, Observer};
use std::sync::Arc;

use zetesis_core::{GroundProgram, Model, Program};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Theory;
use zetesis_objective::Score;
use zetesis_themelios::{
    Admitted, AdmittedBundle, AdmittedFormula, AdmittedFormulaBundle, SourceMetadata,
};

use crate::closure_session::ClosureSession;
use crate::formula_execution::Execution;
use crate::formula_session::FormulaSession;
use crate::hybrid_session::HybridSession;
use crate::phase_timing::Recorder;
use crate::terminal_session::TerminalSession;
use crate::{
    AnswerSelection, ExecutionResources, Grounder, Interruption, Oracle, PhaseTimings,
    SemanticOutcome, SolveConfig, SolveError, SolveFailure, WorldView, WorldViewFailure,
    WorldViewLimits,
};

/// The complete semantic representation supplied to an ordinary session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedProfile {
    /// Finite relational normal program; closure execution chooses lazy or eager.
    Relational,
    /// Coherently indexed finite formulas and their objective/observation programs.
    Formula,
    /// Complete producer theory with admitted constraints checked from source.
    Hybrid,
    /// A checked base theory plus terminal definitions of the original source.
    TerminalDefinitions,
    /// A complete ground graph retaining its original relational program identity.
    Ground,
}

#[derive(Clone, Copy)]
enum Prepared<'a> {
    Relational(&'a Program),
    Formula(crate::countermodel::Input<'a>),
    Hybrid(&'a zetesis_themelios::HybridFormula),
    TerminalDefinitions(&'a zetesis_themelios::TerminalFormula),
    Ground(&'a Arc<GroundProgram>),
}

/// A coherent borrowed owner, never an independently supplied theory/atom table.
/// Constructors do no admission, grounding, or source replay. Explicit oracle
/// requests incompatible with this representation are rejected at session setup.
#[derive(Clone, Copy)]
pub struct PreparedInput<'a> {
    input: Prepared<'a>,
    metadata: Option<&'a SourceMetadata>,
    projection: Option<&'a zetesis_themelios::PreparedProjection>,
}
impl<'a> PreparedInput<'a> {
    /// Borrow the original source plan with its checked base and terminal
    /// definitions. Each verified base answer is reconstructed before a full
    /// answer can be published. An eager base runs under automatic grounding;
    /// a hybrid base (from lazy materialization) runs under automatic or lazy
    /// grounding, on the CPU backend only, its constraints checked before
    /// reconstruction. An eager request never acquires terminal meaning. Explicit projection and
    /// objectives are outside this profile. No admission or execution occurs in
    /// this borrow.
    #[must_use]
    pub fn terminal(owner: &'a zetesis_themelios::TerminalFormula) -> Self {
        Self {
            input: Prepared::TerminalDefinitions(owner),
            metadata: Some(owner.metadata()),
            projection: None,
        }
    }

    /// Borrow a coherent producer theory and its admitted streamed constraints.
    /// No grounding or solving occurs here. The CPU backend supports lazy and
    /// automatic grounding. Objectives are scored only after complete original
    /// constraint acceptance. Device checking is outside this hybrid profile.
    #[must_use]
    pub fn hybrid(owner: &'a zetesis_themelios::HybridFormula) -> Self {
        Self {
            input: Prepared::Hybrid(owner),
            metadata: Some(owner.metadata()),
            projection: Some(owner.projection()),
        }
    }

    /// Reuse an admitted native relational program without source metadata.
    /// This borrows the program directly: it performs no parsing, carrier
    /// expansion or grounding. Lazy and eager execution use the same relational
    /// session branch as source-admitted programs.
    #[must_use]
    pub const fn program(program: &'a Program) -> Self {
        Self {
            input: Prepared::Relational(program),
            metadata: None,
            projection: None,
        }
    }
    /// Borrow a canonical relational preparation with its original display
    /// metadata. The owner keeps the program and metadata together; this borrow
    /// performs no parsing, expansion or grounding. Lazy and eager sessions use
    /// the same admitted relational program.
    #[must_use]
    pub fn relational(owner: &'a zetesis_themelios::PreparedRelational) -> Self {
        Self {
            input: Prepared::Relational(owner.program()),
            metadata: Some(owner.metadata()),
            projection: None,
        }
    }
    /// Reuse an admitted normal program and its source metadata.
    #[must_use]
    pub fn admitted(owner: &'a Admitted) -> Self {
        Self {
            input: Prepared::Relational(owner.program()),
            metadata: Some(owner.metadata()),
            projection: None,
        }
    }
    /// Reuse an admitted original-source bundle without loading its files again.
    #[must_use]
    pub fn bundle(owner: &'a AdmittedBundle) -> Self {
        Self {
            input: Prepared::Relational(owner.program()),
            metadata: Some(owner.metadata()),
            projection: None,
        }
    }
    /// Reuse a formula owner, preserving its exact atom indexing and objectives.
    #[must_use]
    pub fn formula(owner: &'a AdmittedFormula) -> Self {
        Self {
            input: Prepared::Formula(crate::countermodel::Input {
                theory: owner.theory(),
                atoms: owner.atom_catalog(),
                objectives: owner.objectives(),
                gate_atoms: 0,
                keyed_constraints: owner.keyed_constraints(),
                key_analysis: owner.key_analysis(),
                certificate_order: crate::countermodel::certificate_order(
                    owner.source_analysis(),
                    owner.analysis_basis(),
                ),
            }),
            metadata: Some(owner.metadata()),
            projection: Some(owner.projection()),
        }
    }
    /// Reuse a formula bundle with its original metadata and objective program.
    #[must_use]
    pub fn formula_bundle(owner: &'a AdmittedFormulaBundle) -> Self {
        Self {
            input: Prepared::Formula(crate::countermodel::Input {
                theory: owner.theory(),
                atoms: owner.atom_catalog(),
                objectives: owner.objectives(),
                gate_atoms: 0,
                keyed_constraints: owner.keyed_constraints(),
                key_analysis: owner.key_analysis(),
                certificate_order: crate::countermodel::certificate_order(
                    owner.source_analysis(),
                    owner.analysis_basis(),
                ),
            }),
            metadata: Some(owner.metadata()),
            projection: Some(owner.projection()),
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
            projection: None,
        }
    }
    /// Representation used for strategy compatibility checks.
    #[must_use]
    pub const fn profile(self) -> PreparedProfile {
        match self.input {
            Prepared::Relational(_) => PreparedProfile::Relational,
            Prepared::Formula(_) => PreparedProfile::Formula,
            Prepared::Hybrid(_) => PreparedProfile::Hybrid,
            Prepared::TerminalDefinitions(_) => PreparedProfile::TerminalDefinitions,
            Prepared::Ground(_) => PreparedProfile::Ground,
        }
    }
    /// Validated source views, when the prepared owner retained them.
    #[must_use]
    pub const fn metadata(self) -> Option<&'a SourceMetadata> {
        self.metadata
    }

    /// Fixed source projection domain compiled with this coherent input.
    /// An absent or implicit domain leaves ordinary full enumeration unchanged.
    #[must_use]
    pub const fn projection(self) -> Option<&'a zetesis_themelios::PreparedProjection> {
        self.projection
    }
    pub(crate) fn subject(self) -> Subject {
        match self.input {
            Prepared::Relational(program) => Subject::Program(program.clone()),
            Prepared::Formula(input) => Subject::Theory(input.theory.clone()),
            Prepared::Hybrid(owner) => Subject::Hybrid(owner.clone()),
            Prepared::TerminalDefinitions(owner) => Subject::TerminalDefinitions(owner.clone()),
            Prepared::Ground(ground) => Subject::Program(ground.program().clone()),
        }
    }
    fn selection(self, requested: AnswerSelection) -> AnswerSelection {
        match self.input {
            Prepared::Formula(input) if input.objectives.is_present() => requested,
            Prepared::Hybrid(owner) if owner.objectives().is_present() => requested,
            _ => AnswerSelection::All,
        }
    }
    fn configure(self, mut config: SolveConfig) -> Result<SolveConfig, SolveError> {
        if matches!(self.input, Prepared::Hybrid(_)) {
            config.validate_hybrid()?;
            return Ok(config);
        }
        // A hybrid terminal base runs as the hybrid route does: an eager
        // request contradicts its materialization and is refused, automatic
        // and lazy requests run it, under the hybrid backend and batching
        // restrictions.
        if let Prepared::TerminalDefinitions(owner) = self.input
            && owner.base_kind() == zetesis_themelios::BaseKind::Hybrid
        {
            config.validate_hybrid().map_err(|error| match error {
                SolveError::PreparedInput {
                    oracle, grounder, ..
                } => SolveError::PreparedInput {
                    profile: self.profile(),
                    oracle,
                    grounder,
                },
                other => other,
            })?;
            return Ok(config);
        }
        if !matches!(self.input, Prepared::Relational(_))
            && config.source_batching != crate::SourceBatching::Independent
        {
            return Err(SolveError::UnsupportedSourceBatching);
        }
        let compatible = match self.input {
            Prepared::Relational(_) => config.oracle != Oracle::Countermodel,
            Prepared::Formula(_) => {
                config.oracle != Oracle::Closure && config.grounder != Grounder::Lazy
            }
            Prepared::TerminalDefinitions(_) => {
                config.oracle != Oracle::Closure && config.grounder == Grounder::Auto
            }
            Prepared::Hybrid(_) => unreachable!("hybrid policy validated above"),
            Prepared::Ground(_) => {
                config.oracle != Oracle::Countermodel && config.grounder != Grounder::Lazy
            }
        };
        if !compatible {
            return Err(SolveError::PreparedInput {
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
    /// Original producer core together with its admitted source constraints.
    Hybrid(zetesis_themelios::HybridFormula),
    /// Original source with a checked base/terminal-definition correspondence.
    /// The contained base theory alone is not this semantic subject.
    TerminalDefinitions(zetesis_themelios::TerminalFormula),
}
impl Subject {
    /// Whether both handles retain the same original immutable semantic instance.
    /// Structural equality of independently admitted programs is insufficient.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Program(left), Self::Program(right)) => left.same_instance(right),
            (Self::Theory(left), Self::Theory(right)) => left.same_instance(right),
            (Self::Hybrid(left), Self::Hybrid(right)) => left.same_instance(right),
            (Self::TerminalDefinitions(left), Self::TerminalDefinitions(right)) => {
                left.same_instance(right)
            }
            _ => false,
        }
    }
}

/// A full stable interpretation produced by the session's membership engine.
/// Private construction preserves its subject association after detachment.
/// This is semantic evidence, independently of display selection or publication.
/// It records completed membership under the selected implementation, not a Lean
/// proof or enumeration coverage. Cloning shares the subject, atom catalog and
/// selected interpretation; the optional score's cost vector is cloned. Retained
/// interpretations keep the entire shared catalog alive, including unselected atoms.
///
/// ```compile_fail
/// use zetesis_solve::{AnswerSet, Subject};
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
    Hybrid(Box<HybridSession<'a>>),
    TerminalDefinitions(Box<TerminalSession<'a>>),
    Stopped(Box<SemanticOutcome>),
}

/// An ordinary solve request, before validation or execution begins.
///
/// The input borrows one coherent semantic owner. Resource handles may be shared;
/// candidates, budgets, worker pools and outcomes are created by each start.
/// Modifiers perform no device discovery, grounding, callbacks or control polls.
/// Dropping an unstarted request performs no solve work.
pub struct SessionBuilder<'a> {
    input: PreparedInput<'a>,
    config: SolveConfig,
    cancellation: Cancellation,
    selection: AnswerSelection,
    resources: ExecutionResources,
    measurements: Option<crate::SolveMeasurements>,
    projection: Option<crate::ProjectionLimits>,
}

impl<'a> SessionBuilder<'a> {
    /// Return one full answer-set representative per source `#project` key.
    /// Objective selection happens first. This changes enumeration identity,
    /// not membership, scoring or the atoms retained in each representative.
    /// `config.models` counts representatives; zero requests all classes.
    /// Construction only records the request. Starting requires a completed
    /// explicit projection domain; history limits are independent of search.
    #[must_use]
    pub const fn projected(mut self, limits: crate::ProjectionLimits) -> Self {
        self.projection = Some(limits);
        self
    }

    pub(crate) const fn full_identity(mut self) -> Self {
        self.projection = None;
        self
    }
    /// Choose the answer family. The default is optimal ties when an objective
    /// is present; inputs without objectives always enumerate their full family.
    #[must_use]
    pub const fn selection(mut self, selection: AnswerSelection) -> Self {
        self.selection = selection;
        self
    }

    /// Share the caller's execution resource handles with this request.
    ///
    /// Cloning the handles does not copy device allocations or logical state.
    /// The CPU backend ignores them: supplying a device does not force its use.
    /// A GPU backend must match the supplied context's adapter instead of
    /// discovering another one.
    #[must_use]
    pub fn resources(mut self, resources: &ExecutionResources) -> Self {
        self.resources = resources.clone();
        self
    }

    /// Enumerate and retain the complete original answer-set family using this
    /// request's input, execution resources, configuration and control.
    ///
    /// Collection always uses [`AnswerSelection::All`], overriding any earlier
    /// [`Self::selection`] choice. Objectives annotate every answer, including
    /// nonoptimal answers. The request is consumed before any search begins;
    /// an already started or partially consumed [`Session`] cannot be collected
    /// through this operation. Use `config.models = 0` to request exhaustion.
    ///
    /// Work and retained space follow [`WorldView::collect`]. Members move into
    /// one bounded collection without repeated membership checks or full-model
    /// clones. The resource handles do not retain the collected family.
    ///
    /// # Errors
    /// Preserves [`WorldView::collect`]'s typed setup, execution, incomplete
    /// coverage and storage failures, including the checked prefix and original
    /// subject. Supplied device failures retain their ordinary session boundary.
    pub fn collect(self, limits: WorldViewLimits) -> Result<WorldView, WorldViewFailure> {
        WorldView::collect_request(self, limits, &mut Ignore)
    }

    /// Collect the complete original family with synchronous observations of
    /// both session preparation and every subsequent pull.
    ///
    /// This has [`Self::collect`]'s unrestricted selection and storage contract.
    /// The observer is borrowed for the whole operation and no event queue is
    /// retained. Its effects and storage remain outside solver limits.
    ///
    /// # Errors
    /// Returns [`Self::collect`]'s failures. An observer refusal retains its
    /// original external cause and the checked prefix, stops subsequent work
    /// and callbacks, and cannot establish complete coverage or trigger device
    /// fallback. Formula preparation failures deferred to the first pull retain
    /// [`Self::start_observed`]'s failure semantics.
    pub fn collect_observed(
        self,
        limits: WorldViewLimits,
        observer: &mut impl ExecutionObserver,
    ) -> Result<WorldView, WorldViewFailure> {
        WorldView::collect_request(self, limits, &mut Observer(observer))
    }

    /// Share a caller-owned host measurement scope with this solve.
    ///
    /// Its detailed-measurement setting replaces config.stats. A stages-only
    /// scope records coarse intervals without enabling detailed solver statistics.
    /// Admission and publication can record into the same scope; each session
    /// retains its own search state.
    /// This performs no solve work, device discovery or clock reads.
    #[must_use]
    pub fn measurements(mut self, measurements: &crate::SolveMeasurements) -> Self {
        self.config.stats = measurements.details_enabled();
        self.measurements = Some(measurements.clone());
        self
    }

    /// Validate the request and start a fresh session without observations.
    ///
    /// # Errors
    /// Returns the setup failures of [`Session::new`]. Supplied device resources
    /// additionally retain their explicit adapter, contention and health errors.
    pub fn start(self) -> Result<Session<'a>, SolveFailure> {
        self.start_with(&mut Ignore)
    }

    /// Start with typed preparation observations, borrowing the observer only
    /// for this call. Later observations require [`Session::next_observed`].
    ///
    /// # Errors
    /// Preserves [`Session::new_observed`]'s setup and callback failure behavior.
    pub fn start_observed(
        self,
        observer: &mut impl ExecutionObserver,
    ) -> Result<Session<'a>, SolveFailure> {
        self.start_with(&mut Observer(observer))
    }

    pub(crate) fn subject(&self) -> Subject {
        self.input.subject()
    }

    pub(crate) fn start_with(
        mut self,
        observations: &mut impl ExecutionSink,
    ) -> Result<Session<'a>, SolveFailure> {
        let phases = self
            .measurements
            .unwrap_or_else(|| crate::SolveMeasurements::new(self.config.stats));
        let projected_limit = self.config.models;
        let projection = self
            .projection
            .map(|limits| {
                let _solving = phases.stage(crate::SolveStage::Solving);
                self.cancellation
                    .poll()
                    .map_err(crate::ProjectionError::Control)?;
                let domain = self
                    .input
                    .projection()
                    .ok_or(crate::ProjectionError::MissingDeclaration)?;
                crate::projection::Projection::new(domain, limits)
            })
            .transpose()
            .map_err(|error| {
                let mut failure = SolveFailure::from(SolveError::Projection(error));
                failure.subject = Some(self.input.subject());
                failure.phase_timings = phases.snapshot().map(Box::new);
                failure
            })?;
        if projection.is_some() {
            self.config.models = 0;
        }
        let result = Session::initialize(
            self.input,
            self.config,
            &self.cancellation,
            phases.recorder(),
            self.input.selection(self.selection),
            &self.resources,
            observations,
        );
        match result {
            Ok((state, config)) => Ok(Session {
                state,
                config,
                cancellation: self.cancellation,
                phases,
                subject: self.input.subject(),
                projection,
                projected_limit,
                projection_done: false,
            }),
            Err(error) => {
                let mut failure = SolveFailure::from(error);
                failure.subject = Some(self.input.subject());
                failure.phase_timings = phases.snapshot().map(Box::new);
                Err(failure)
            }
        }
    }
}

/// A pull-based ordinary solve, independent of argument parsing and writers.
/// [`Self::new`] selects objective ties after search; [`Self::enumerate`] streams
/// the unrestricted original family. `models` limits yielded answers, or
/// retained objective ties in a selected solve. Budgets persist across pulls.
/// Stopping pulls early establishes no additional coverage. The caller decides
/// how to store or publish each full interpretation; no delivery is inferred.
///
/// Enabled elapsed time spans the measurement owner's lifetime through each
/// snapshot. Time between pulls is unattributed unless the caller records another
/// stage. A scope injected with [`SessionBuilder::measurements`] includes all work
/// recorded by its owners, including other sessions. This session's solving spans
/// cover setup and active pulls. Disabled instrumentation reads no clocks;
/// deadline control remains independent.
///
/// ```
/// use zetesis_solve::{Backend, PreparedInput, Session, SolveConfig};
/// use zetesis_cpu::Cancellation;
/// use zetesis_themelios::{admit, AdmissionOptions};
///
/// let admitted = admit("a.".into(), AdmissionOptions::default())?;
/// let config = SolveConfig { backend: Backend::Cpu, models: 0, ..Default::default() };
/// let mut session = Session::new(PreparedInput::admitted(&admitted), config, Cancellation::default())?;
/// let model = session.next().unwrap()?;
/// assert_eq!(model.interpretation().atoms().len(), 1);
/// assert!(session.next().is_none());
/// assert_eq!(session.outcome().unwrap().verified_models(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Session<'a> {
    state: State<'a>,
    config: SolveConfig,
    cancellation: Cancellation,
    phases: crate::SolveMeasurements,
    subject: Subject,
    projection: Option<crate::projection::Projection<'a>>,
    projected_limit: usize,
    projection_done: bool,
}

impl<'a> Session<'a> {
    /// Compose answer selection, execution resources and preparation
    /// observations before starting an ordinary solve. Construction performs no
    /// validation or execution. Existing convenience constructors use this same
    /// request with independently owned execution resources.
    #[must_use]
    pub fn builder(
        input: PreparedInput<'a>,
        config: SolveConfig,
        cancellation: Cancellation,
    ) -> SessionBuilder<'a> {
        SessionBuilder {
            input,
            config,
            cancellation,
            selection: AnswerSelection::Optimal,
            resources: ExecutionResources::default(),
            measurements: None,
            projection: None,
        }
    }

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
        cancellation: Cancellation,
    ) -> Result<Self, SolveFailure> {
        Self::builder(input, config, cancellation).start()
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
        cancellation: Cancellation,
    ) -> Result<Self, SolveFailure> {
        Self::builder(input, config, cancellation)
            .selection(AnswerSelection::All)
            .start()
    }
    /// Start an ordinary solve with synchronous typed execution observations.
    ///
    /// The observer is borrowed only during preparation. Use
    /// [`Self::next_observed`] for subsequent pulls; ordinary iteration discards
    /// later observations. Events belong to this prepared input, not any other
    /// session the observer may also serve. No event queue is retained.
    ///
    /// # Errors
    /// Execution-setup failures return immediately, retaining the known subject.
    /// Formula initialization owns its terminal failure: an observer failure at
    /// formula, certificate or objective setup stops further initialization and
    /// is returned by the first pull. No later callback runs for that failure.
    /// Both paths preserve the original cause and establish no coverage.
    pub fn new_observed(
        input: PreparedInput<'a>,
        config: SolveConfig,
        cancellation: Cancellation,
        observer: &mut impl ExecutionObserver,
    ) -> Result<Self, SolveFailure> {
        Self::builder(input, config, cancellation).start_observed(observer)
    }

    /// Enumerate all answer sets with typed preparation observations.
    ///
    /// This has [`Self::enumerate`]'s selection policy and
    /// [`Self::new_observed`]'s observation lifetime and failure contract.
    ///
    /// # Errors
    /// Returns the setup and observation failures of [`Self::new_observed`].
    pub fn enumerate_observed(
        input: PreparedInput<'a>,
        config: SolveConfig,
        cancellation: Cancellation,
        observer: &mut impl ExecutionObserver,
    ) -> Result<Self, SolveFailure> {
        Self::builder(input, config, cancellation)
            .selection(AnswerSelection::All)
            .start_observed(observer)
    }
    fn initialize(
        input: PreparedInput<'a>,
        config: SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
        selection: AnswerSelection,
        resources: &ExecutionResources,
        observations: &mut impl ExecutionSink,
    ) -> Result<(State<'a>, SolveConfig), SolveError> {
        let config = input.configure(config)?;
        if let Prepared::TerminalDefinitions(owner) = input.input {
            phases.terminal_grounding(owner.base_kind());
        }
        let _solving = phases.stage(crate::SolveStage::Solving);
        if let Err(stop) = cancellation.poll() {
            let interruption = Interruption::Preparation(stop);
            return Ok((
                State::Stopped(Box::new(SemanticOutcome {
                    projection: None,
                    subject: Some(input.subject()),
                    selection: Some(selection),
                    verified: 0,
                    scored: 0,
                    retained: 0,
                    search_state: Some(crate::SearchState::Interrupted(interruption)),
                    optimization: None,
                    objective_work: 0,
                    checked: 0,
                    gate_atoms: 0,
                    candidate_statistics: None,
                    countermodel_statistics: None,
                    formula_execution: None,
                    lazy_execution: None,
                    shared_execution: None,
                    closure_execution: None,
                    query_execution: None,
                    hybrid_execution: None,
                    terminal_execution: None,
                    model_construction: None,
                })),
                config,
            ));
        }
        let state = match input.input {
            Prepared::Relational(program) => {
                State::Closure(Box::new(ClosureSession::with_resources(
                    program,
                    None,
                    &config,
                    resources,
                    observations,
                    cancellation,
                    phases,
                )?))
            }
            Prepared::Ground(ground) => State::Closure(Box::new(ClosureSession::with_resources(
                ground.program(),
                Some(Arc::clone(ground)),
                &config,
                resources,
                observations,
                cancellation,
                phases,
            )?)),
            Prepared::Formula(input) => State::Formula(Box::new(FormulaSession::with_resources(
                input,
                &config,
                resources,
                observations,
                cancellation,
                phases,
                selection,
            )?)),
            Prepared::Hybrid(owner) => State::Hybrid(Box::new(HybridSession::observed(
                crate::hybrid_session::HybridInput {
                    core: owner.core(),
                    subject: crate::Subject::Hybrid(owner.clone()),
                },
                &config,
                resources,
                observations,
                cancellation,
                phases,
                selection,
            )?)),
            Prepared::TerminalDefinitions(owner) => {
                State::TerminalDefinitions(Box::new(TerminalSession::new(
                    owner,
                    &config,
                    resources,
                    observations,
                    cancellation,
                    phases,
                )?))
            }
        };
        Ok((state, config))
    }
    /// Final semantic evidence once search finishes, stops, or fails. An
    /// optimized outcome can be available while retained ties await delivery.
    #[must_use]
    pub fn outcome(&self) -> Option<SemanticOutcome> {
        if self.projection_done {
            return Some(self.progress());
        }
        let mut outcome = match &self.state {
            State::Closure(state) => state.terminal().then(|| state.outcome()),
            State::Formula(state) => state
                .finished()
                .then(|| state.outcome(self.phases.recorder())),
            State::Hybrid(state) => state
                .finished()
                .then(|| state.outcome(self.phases.recorder())),
            State::TerminalDefinitions(state) => state
                .finished()
                .then(|| state.outcome(self.phases.recorder())),
            State::Stopped(outcome) => Some((**outcome).clone()),
        };
        if let Some(outcome) = &mut outcome {
            outcome.projection = self
                .projection
                .as_ref()
                .map(crate::projection::Projection::statistics);
        }
        outcome
    }
    /// End an unfinished session and retain its current evidence. Coverage stays
    /// unavailable unless the retained engine already established a terminal state.
    #[must_use]
    pub fn stop(mut self) -> SemanticOutcome {
        if let State::TerminalDefinitions(state) = &mut self.state {
            state.stop(self.phases.recorder());
        }
        self.progress()
    }
    /// Attempted host phases from this session's measurement owner; absence means
    /// instrumentation was disabled. An injected owner includes all recorded
    /// caller work and other sessions sharing that scope.
    #[must_use]
    pub fn phase_timings(&self) -> Option<PhaseTimings> {
        self.phases.snapshot()
    }
    /// Current checked evidence, including results buffered before delivery.
    ///
    /// A snapshot does not stop search. Missing completion remains absent; only
    /// the recorded coverage determines whether enumeration is complete.
    #[must_use]
    pub fn progress(&self) -> SemanticOutcome {
        let mut outcome = match &self.state {
            State::Closure(state) => state.outcome(),
            State::Formula(state) => state.outcome(self.phases.recorder()),
            State::Hybrid(state) => state.outcome(self.phases.recorder()),
            State::TerminalDefinitions(state) => state.outcome(self.phases.recorder()),
            State::Stopped(outcome) => (**outcome).clone(),
        };
        outcome.projection = self
            .projection
            .as_ref()
            .map(crate::projection::Projection::statistics);
        if self.projection_done
            && self.projected_limit != 0
            && outcome
                .projection
                .is_some_and(|stats| stats.representatives >= self.projected_limit)
            && outcome.search_state.is_none()
        {
            outcome.search_state = Some(crate::SearchState::RequestedModels);
        }
        outcome
    }
    /// Pull the next answer using synchronous typed execution observations.
    ///
    /// The observer is borrowed only for this pull. Events describe this session's
    /// attempted work and cannot establish answer-set membership or coverage.
    /// An observer error stops this session: later pulls return None, with its
    /// checked prefix retained in the failure and [`Self::outcome`]. Successful
    /// callbacks do not imply that subsequent work or answer publication succeeds.
    ///
    /// # Errors
    /// Returns ordinary solve faults or the observer's external error, wrapped
    /// separately from device faults so it cannot trigger fallback or retry.
    pub fn next_observed(
        &mut self,
        observer: &mut impl ExecutionObserver,
    ) -> Option<Result<AnswerSet, SolveFailure>> {
        self.pull(&mut Observer(observer))
    }

    pub(crate) fn pull(
        &mut self,
        observations: &mut impl ExecutionSink,
    ) -> Option<Result<AnswerSet, SolveFailure>> {
        if self.projection_done {
            return None;
        }
        loop {
            let next = self.pull_original(observations);
            let Some(projection) = self.projection.as_mut() else {
                return next;
            };
            match next {
                Some(Ok(answer)) => {
                    let solving = self.phases.stage(crate::SolveStage::Solving);
                    let inserted = projection.insert(answer.interpretation(), &self.cancellation);
                    drop(solving);
                    match inserted {
                        Ok(false) => {}
                        Ok(true) => {
                            self.projection_done = self.projected_limit != 0
                                && projection.statistics().representatives >= self.projected_limit;
                            return Some(Ok(answer));
                        }
                        Err(error) => {
                            self.projection_done = true;
                            let mut failure = SolveFailure::from(SolveError::Projection(error));
                            failure.subject = Some(self.subject.clone());
                            failure.semantic = Some(Box::new(self.progress()));
                            failure.phase_timings = self.phase_timings().map(Box::new);
                            return Some(Err(failure));
                        }
                    }
                }
                Some(Err(error)) => {
                    self.projection_done = true;
                    return Some(Err(error));
                }
                None => {
                    self.projection_done = true;
                    let complete =
                        self.progress().completion() == Some(crate::Completion::Exhausted);
                    self.projection
                        .as_mut()
                        .expect("projected stream")
                        .finish(complete);
                    return None;
                }
            }
        }
    }

    fn pull_original(
        &mut self,
        observations: &mut impl ExecutionSink,
    ) -> Option<Result<AnswerSet, SolveFailure>> {
        let solving = self.phases.stage(crate::SolveStage::Solving);
        let next = match &mut self.state {
            State::Closure(state) => state
                .next(&self.config, &self.cancellation, self.phases.recorder())
                .map(|result| result.map(|model| (model, None))),
            State::Formula(state) => state.next(
                &self.config,
                observations,
                &self.cancellation,
                self.phases.recorder(),
            ),
            State::Hybrid(state) => state.next(
                &self.config,
                observations,
                &self.cancellation,
                self.phases.recorder(),
            ),
            State::TerminalDefinitions(state) => state.next(
                &self.config,
                observations,
                &self.cancellation,
                self.phases.recorder(),
            ),
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
                failure.semantic = Some(Box::new(self.progress()));
                failure.phase_timings = self.phases.snapshot().map(Box::new);
                Err(failure)
            }
        })
    }
}
impl Iterator for Session<'_> {
    type Item = Result<AnswerSet, SolveFailure>;
    fn next(&mut self) -> Option<Self::Item> {
        self.pull(&mut Ignore)
    }
}
impl std::iter::FusedIterator for Session<'_> {}
