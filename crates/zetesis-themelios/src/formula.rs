//! Opt-in finite source admission into general Ferraris formulas.

use std::collections::BTreeMap;
use std::fmt;

use themelios_base::diagnostic::Diagnostic;
use themelios_base::source::Source;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::{Program as SourceProgram, Statement};
use themelios_program::raise::raise;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::parse::parse;
use zetesis_core::Atom;
use zetesis_ferraris::Theory;

use crate::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionError, BundleAdmissionOptions,
    ExpansionFailure, ExpansionLimits, InputLimit, SourceBundle, SourceMetadata, bundle_admission,
    extended, formula_ir, metadata, profile,
};

mod preparation;
use preparation::Preparation;
pub use preparation::{PreparedFormula, PreparedFormulaBundle};

const DEFAULT_OBJECTIVE_PRESENCE_ENTRIES: usize = 16_384;

/// Independent finite grounding and formula-storage ceilings. No zero value
/// means unlimited. Source parsing and scalar expansion retain their own limits.
#[derive(Clone, Copy, Debug)]
pub struct FormulaLimits {
    /// Simultaneously retained borrowed entries in mixed objective-presence plans;
    /// allocator overhead and internal collection capacity are not byte-accounted.
    pub max_objective_presence_entries: usize,
    /// Distinct scalar values in the logical source, independent of join work.
    pub max_domain_values: usize,
    /// Distinct values in one generated assignment row and across generated heads.
    pub max_assignment_values: usize,
    /// Distinct owned elements in one unconditional disjunctive head.
    pub max_disjunction_elements: usize,
    /// Retained row identifiers across bound-column support indexes.
    pub max_support_index_entries: usize,
    /// Distinct aggregate/outer-binding entries retained during final grounding.
    pub max_aggregate_cache_rows: usize,
    /// Retained key value slots/text payload; allocator overhead is excluded.
    pub max_aggregate_cache_key_bytes: usize,
    /// Total coalesced tuple conditions retained by the final grounding cache.
    pub max_aggregate_cache_elements: usize,
    /// Total assignment-value guard roots retained by the final grounding cache.
    pub max_aggregate_cache_roots: usize,
    /// Visited owned statements, atoms, and terms before upstream analysis.
    pub max_analysis_nodes: usize,
    /// Head-by-dependency occurrence products before graph allocation.
    pub max_analysis_edges: usize,
    /// Total outer and local substitutions, including ones rejected by filters.
    pub max_substitutions: u64,
    /// Total grounding expression and formula construction operations.
    pub max_work: u64,
    /// Complete rounds constructing the possible-positive support relation.
    pub max_support_rounds: u64,
    /// Original source locations copied into emitted formula-root evidence.
    pub max_origin_locations: usize,
    /// Final dense atom, formula-node, and theory-root storage ceilings.
    pub theory: zetesis_ferraris::AdmissionLimits,
    /// Per-aggregate translation ceilings, additionally capped by total formula work/nodes.
    pub aggregate: zetesis_ferraris::AggregateLimits,
    /// Independent admission ceilings for lifted objective templates.
    pub objective: zetesis_objective::AdmissionLimits,
    /// Independent term observation template ceilings.
    pub observation: crate::observation::AdmissionLimits,
}
impl Default for FormulaLimits {
    fn default() -> Self {
        Self {
            // Match the bounded aggregate-plan row scale; this counts borrowed
            // pointer slots, not source bytes or semantic candidate atoms.
            max_objective_presence_entries: DEFAULT_OBJECTIVE_PRESENCE_ENTRIES,
            max_domain_values: 1_024,
            max_assignment_values: 1_024,
            max_disjunction_elements: 1_024,
            max_support_index_entries: 1_000_000,
            max_aggregate_cache_rows: 16_384,
            max_aggregate_cache_key_bytes: 8_388_608,
            max_aggregate_cache_elements: 65_536,
            max_aggregate_cache_roots: 65_536,
            max_analysis_nodes: 262_144,
            max_analysis_edges: 1_000_000,
            max_substitutions: 1_000_000,
            max_work: 10_000_000,
            max_support_rounds: 1_024,
            max_origin_locations: 1_000_000,
            theory: zetesis_ferraris::AdmissionLimits::default(),
            objective: zetesis_objective::AdmissionLimits::default(),
            observation: crate::observation::AdmissionLimits::default(),
            aggregate: zetesis_ferraris::AggregateLimits::default(),
        }
    }
}

/// Resources counted by the separate formula admission boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulaResource {
    /// Simultaneously retained entries in completed objective-presence planning.
    ObjectivePresenceEntries,
    /// Distinct finite scalar values.
    DomainValues,
    /// Generated scalar assignment values.
    AssignmentValues,
    /// Row identifiers retained in positive-support column indexes.
    SupportIndexEntries,
    /// Final-grounding aggregate binding cache entries.
    AggregateCacheRows,
    /// Distinct owned elements in one unconditional disjunctive head.
    DisjunctionElements,
    /// Retained aggregate binding key slots and scalar text payload.
    AggregateCacheKeys,
    /// Coalesced tuple conditions retained in aggregate cache entries.
    AggregateCacheElements,
    /// Assignment-value guard roots retained in aggregate cache entries.
    AggregateCacheRoots,
    /// Preflight structural analysis nodes.
    AnalysisNodes,
    /// Preflight structural dependency edge occurrences.
    AnalysisEdges,
    /// Evaluated outer or local substitutions.
    Substitutions,
    /// Grounding expression and formula operations.
    Work,
    /// Unique complete tuple identities in a grounded aggregate.
    AggregateElements,
    /// Original objective elements before raising or deduplication.
    ObjectiveElements,
    /// Complete possible-positive support rounds.
    SupportRounds,
    /// Dense semantic atoms.
    Atoms,
    /// Formula DAG nodes.
    Nodes,
    /// Theory roots.
    Roots,
    /// Retained parsed locations.
    Origins,
    /// Variables in an outer rule or complete local element scope.
    Variables,
    /// Arguments of one predicate.
    Arity,
}
impl fmt::Display for FormulaResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A located refusal of finite formula source admission; never semantic UNSAT.
#[derive(Debug)]
pub enum FormulaFailure {
    /// Located bounded observation compilation failure.
    Observation {
        /// Independent typed observation refusal.
        error: crate::observation::Error,
    },
    /// Existing faithful parsing, profile, or scalar-expansion refusal.
    Expansion(ExpansionFailure),
    /// Include identity cannot be represented by the canonical source graph.
    Include(Box<BundleAdmissionError>),
    /// A finite compilation resource exceeded its inclusive ceiling.
    Limit {
        /// Counted resource.
        resource: FormulaResource,
        /// Inclusive configured ceiling.
        limit: u128,
        /// Count required by the next operation.
        observed: u128,
        /// Original rule or source span.
        location: Location,
    },
    /// A variable has no positive ordinary-atom binder in its scope.
    UnsafeVariable {
        /// Dense variable index in the rule or local element scope.
        variable: usize,
        /// Original rule span.
        location: Location,
    },
    /// An evaluated positive argument lacks an independently established input.
    /// This profile does not invert arithmetic to discover source bindings.
    UnboundArgumentInput {
        /// Dense source variable index in the rule or local element scope.
        variable: usize,
        /// Original enclosing rule span.
        location: Location,
    },
    /// A finite value instruction has no independent producer for an input.
    UnboundValueInput {
        /// Dense input slot in the enclosing rule.
        variable: usize,
        /// Original enclosing rule span.
        location: Location,
    },
    /// Finite value instructions have a cyclic input dependency. This is a
    /// native scheduling refusal, not an impossibility claim about ASP recursion.
    CyclicValueInput {
        /// An input slot in the unscheduled dependency component.
        variable: usize,
        /// Original enclosing rule span.
        location: Location,
    },
    /// A bounded finite aggregate translation was refused.
    Aggregate {
        /// Typed constructor failure with partial accounting.
        error: zetesis_ferraris::AggregateError,
        /// Original enclosing rule span.
        location: Location,
    },
    /// The independent lifted objective admission door rejected construction.
    Objective {
        /// Typed objective admission error.
        error: zetesis_objective::AdmissionError,
        /// Original objective occurrence.
        location: Location,
    },
    /// The independent formula-theory admission door rejected construction.
    Theory {
        /// Typed formula error.
        error: zetesis_ferraris::AdmissionError,
        /// Original source span.
        location: Location,
    },
}
impl From<AdmissionFailure> for FormulaFailure {
    fn from(error: AdmissionFailure) -> Self {
        Self::Expansion(error.into())
    }
}
impl From<ExpansionFailure> for FormulaFailure {
    fn from(error: ExpansionFailure) -> Self {
        Self::Expansion(error)
    }
}
impl FormulaFailure {
    /// Located diagnostics; bundle identities resolve through its retained catalog.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        match self {
            Self::Expansion(error) => error.diagnostics(),
            Self::Include(error) => error.diagnostics(),
            Self::Observation { error } => error
                .location()
                .into_iter()
                .map(|location| {
                    crate::diagnostic::diagnostic(
                        "observation-admission",
                        error.to_string(),
                        location,
                    )
                })
                .collect(),
            Self::Limit { location, .. }
            | Self::UnsafeVariable { location, .. }
            | Self::UnboundArgumentInput { location, .. }
            | Self::UnboundValueInput { location, .. }
            | Self::CyclicValueInput { location, .. }
            | Self::Theory { location, .. }
            | Self::Objective { location, .. }
            | Self::Aggregate { location, .. } => vec![crate::diagnostic::diagnostic(
                "formula-admission",
                self.to_string(),
                *location,
            )],
        }
    }
}
impl fmt::Display for FormulaFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expansion(error) => error.fmt(f),
            Self::Include(error) => error.fmt(f),
            Self::Limit {
                resource,
                limit,
                observed,
                ..
            } => write!(f, "formula {resource} limit {limit} exceeded by {observed}"),
            Self::UnsafeVariable { variable, .. } => {
                write!(f, "unsafe formula variable {variable}")
            }
            Self::UnboundArgumentInput { variable, .. } => write!(
                f,
                "evaluated positive argument requires independently bound input {variable}; arithmetic inversion is unsupported"
            ),
            Self::UnboundValueInput { variable, .. } => write!(
                f,
                "finite value instruction requires independently produced input {variable}"
            ),
            Self::CyclicValueInput { variable, .. } => write!(
                f,
                "cyclic finite value dependency through input {variable} is unsupported"
            ),
            Self::Theory { error, .. } => error.fmt(f),
            Self::Objective { error, .. } => error.fmt(f),
            Self::Aggregate { error, .. } => error.fmt(f),
            Self::Observation { error } => error.fmt(f),
        }
    }
}
impl std::error::Error for FormulaFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Expansion(error) => Some(error),
            Self::Include(error) => Some(error.as_ref()),
            Self::Theory { error, .. } => Some(error),
            Self::Objective { error, .. } => Some(error),
            Self::Aggregate { error, .. } => Some(error),
            Self::Observation { error } => Some(error),
            _ => None,
        }
    }
}

/// A bounded finite formula theory with original single-source evidence.
#[derive(Debug)]
pub struct AdmittedFormula {
    compiled: Compiled,
    source: Source,
    metadata: SourceMetadata,
}
impl AdmittedFormula {
    /// Structural facts about [`Self::analyzed_program`]. Consult
    /// [`Self::analysis_basis`]: a dependency projection does not certify source
    /// safety or class membership. Unknown never removes runtime ceilings.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.compiled.analysis
    }
    /// Bounded, pool-free analysis input retaining parsed origins. Consult
    /// [`Self::analysis_basis`] before interpreting its structural verdicts.
    #[must_use]
    pub fn analyzed_program(&self) -> &SourceProgram {
        &self.compiled.analyzed
    }

    /// Meaning of the retained analysis input. Dependency projections do not
    /// certify the original source's safety, class, or execution eligibility.
    #[must_use]
    pub fn analysis_basis(&self) -> AnalysisBasis {
        self.compiled.analysis_basis
    }

    /// The admitted general formula theory.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.compiled.theory
    }
    /// Optional source-derived candidate consequences. The original theory is
    /// unchanged; an incomplete optional plan never denotes a source refusal.
    #[must_use]
    pub fn count_plan(&self) -> crate::CountPlanStatus<'_> {
        self.compiled.count_plan.view()
    }
    /// Semantic atom identities in exactly the theory's dense index order.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.compiled.atoms
    }
    /// Parsed origins per emitted theory root, preserving merged source evidence.
    /// Necessary support guards collect producer origins and the first atom occurrence.
    #[must_use]
    pub fn formula_origins(&self) -> &[Vec<Location>] {
        &self.compiled.origins
    }
    /// Original bytes and source identity.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }
    /// Lifted objectives evaluated only after stable-model membership is verified.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.compiled.objectives
    }
    /// Original objective element origins, parallel to active objective templates.
    #[must_use]
    pub fn objective_origins(&self) -> &[Vec<Location>] {
        &self.compiled.objective_origins
    }
    /// Every original objective declaration, including statically unreachable ones.
    #[must_use]
    pub fn objective_declarations(&self) -> &[Location] {
        &self.compiled.objective_declarations
    }
    /// Declarations and presentation selection; never a model projection.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }
}

/// A bounded finite formula theory with the complete original include catalog.
#[derive(Debug)]
pub struct AdmittedFormulaBundle {
    compiled: Compiled,
    bundle: SourceBundle,
    metadata: SourceMetadata,
}
impl AdmittedFormulaBundle {
    /// Structural facts about [`Self::analyzed_program`]. Consult
    /// [`Self::analysis_basis`]: a dependency projection does not certify source
    /// safety or class membership. Unknown never removes runtime ceilings.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.compiled.analysis
    }
    /// Bounded, pool-free analysis input retaining parsed origins. Consult
    /// [`Self::analysis_basis`] before interpreting its structural verdicts.
    #[must_use]
    pub fn analyzed_program(&self) -> &SourceProgram {
        &self.compiled.analyzed
    }

    /// Meaning of the retained analysis input. Dependency projections do not
    /// certify the original source's safety, class, or execution eligibility.
    #[must_use]
    pub fn analysis_basis(&self) -> AnalysisBasis {
        self.compiled.analysis_basis
    }

    /// The admitted general formula theory.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.compiled.theory
    }
    /// Optional source-derived candidate consequences. The original theory is
    /// unchanged; an incomplete optional plan never denotes a source refusal.
    #[must_use]
    pub fn count_plan(&self) -> crate::CountPlanStatus<'_> {
        self.compiled.count_plan.view()
    }
    /// Semantic atom identities in theory index order.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.compiled.atoms
    }
    /// Parsed origins per theory root, resolvable in the retained catalog.
    #[must_use]
    pub fn formula_origins(&self) -> &[Vec<Location>] {
        &self.compiled.origins
    }
    /// Every original file, source identity, and include occurrence.
    #[must_use]
    pub fn bundle(&self) -> &SourceBundle {
        &self.bundle
    }
    /// Lifted objectives evaluated only after stable-model membership is verified.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.compiled.objectives
    }
    /// Original objective element origins, parallel to active objective templates.
    #[must_use]
    pub fn objective_origins(&self) -> &[Vec<Location>] {
        &self.compiled.objective_origins
    }
    /// Every original objective declaration, including statically unreachable ones.
    #[must_use]
    pub fn objective_declarations(&self) -> &[Location] {
        &self.compiled.objective_declarations
    }
    /// Global declaration and display metadata.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }
}

/// A formula refusal retaining original bundle bytes for every diagnostic.
#[derive(Debug)]
pub struct FormulaBundleFailure {
    bundle: SourceBundle,
    error: Box<FormulaFailure>,
}
impl FormulaBundleFailure {
    /// Original source catalog.
    #[must_use]
    pub fn bundle(&self) -> &SourceBundle {
        &self.bundle
    }
    /// Typed located refusal.
    #[must_use]
    pub fn error(&self) -> &FormulaFailure {
        &self.error
    }
    /// Located diagnostic view.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.error.diagnostics()
    }
}
impl fmt::Display for FormulaBundleFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for FormulaBundleFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

/// What the retained upstream analysis describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisBasis {
    /// A bounded, pool-free normalization of the admitted source program.
    NormalizedProgram,
    /// A pool-free signature/polarity projection. Safety and class verdicts
    /// describe this projection; they are not conclusions about source semantics.
    DependencyProjection,
}

#[derive(Debug)]
pub(crate) struct Compiled {
    pub analysis_basis: AnalysisBasis,
    pub analysis: themelios_analysis::Analysis,
    pub analyzed: SourceProgram,
    pub theory: Theory,
    pub count_plan: crate::formula_count_plan::Outcome,
    pub atoms: Vec<Atom>,
    pub origins: Vec<Vec<Location>>,
    pub objectives: zetesis_objective::ObjectiveProgram,
    pub objective_origins: Vec<Vec<Location>>,
    pub objective_declarations: Vec<Location>,
}

/// Admit the extended scalar profile, finite conditional choices and body
/// count/sum/sum+/min/max aggregates into a Ferraris theory. Choices may have
/// integer bounds; each aggregate retains its separate value/endpoint profile.
/// Positive ordinary atoms bind variables in global or element-local scopes;
/// acyclic positive aggregate equalities may additionally bind named values.
/// Completed aggregate/scalar proposals may feed ordinary evaluated arguments,
/// signed negative gates, finite ranges, integer choice bounds, nonbinding
/// aggregate guards and admitted universal body conditionals. Aggregate guard
/// comparisons retain their separate default-negation scope and never bind
/// additional variables. Conditional local joins inherit the complete
/// outer binding; neither local witnesses nor vacuity establishes an aggregate
/// equality. Every generated rule retains its original equalities.
/// Ground scalar comparisons use ASP term order; arithmetic expressions
/// require numeric operands. Flat tuple equality/disequality is also supported.
///
/// Eligibility remains a formula even for recursive conditions. Duplicate
/// grounded head atoms combine permission by disjunction. Ordinary choices
/// count atoms; function heads measure distinct complete tuples selected by any
/// eligible head occurrence. Count, signed numeric sum and nonnegative numeric
/// sum+ permit either tuple/atom alias direction. Numeric-valued min/max heads
/// require a complete per-group tuple/atom bijection. Head bounds are constraints,
/// so they do not invent support.
/// Numeric measure is separate from permission: zero-weight heads remain
/// selectable. Missing or nonnumeric weights and negative sum+ head weights
/// have an explicit zetesis profile refusal, including closed weights in
/// statically inactive rules. Positive, default-negated and double-negated
/// ordinary element conditions retain their original eligibility formulas.
/// Min/max guards use ASP term order and real empty extrema; first tuple values
/// remain numeric, with the existing internal integer-endpoint limitation.
/// Necessary producer guards are double-negated and therefore leave reduct subsets
/// unconstrained whenever the candidate passes them. Grounding uses complete
/// relational joins over a bounded possible-positive closure. Gates are ignored
/// only while constructing that support upper bound. Aggregate guards are also
/// conservatively ignored there; their exact condition formulas remain in the
/// theory. Constraints never derive support. Source scopes are checked before
/// pruning. A bounded cursor proposes generated assignment values; each actual
/// equality formula remains in the reduct theory.
///
/// The objective profile retains lifted `#minimize`/`#maximize` elements and positive
/// weak constraints normalized into the same path: scalar
/// constant/variable weights and tuples, ground integer priorities (default zero),
/// positive ordinary conditions, and scalar equality/disequality filters. Source
/// weights normalize before global tuple deduplication; eligible maximize
/// `i32::MIN` weights receive a located overflow refusal. Source
/// priorities survive zero weights and inactive models; templates with no possible
/// positive/filter binding are omitted. Objective-enabled programs currently
/// refuse default-negated producer bodies/choice conditions and general aggregate
/// producer bodies. Pure total assignment producers admit only structural direct
/// observers: generated positions are fresh, unshared, unfiltered variables in
/// objective conditions. Unfiltered total assignments may observe another total
/// assignment through aggregate tuple sources; other generated-value consumers
/// and conditional producer dependencies retain explicit refusals. This boundary
/// preserves priority presence without asserting value realizability. Negative
/// and aggregate constraints remain supported. Objectives supply no
/// logical support and are evaluated separately on reduct-verified full models.
/// This route invokes no solver, changes no existing S0/extended API contract,
/// and claims no parser-to-Lean refinement.
///
/// # Errors
/// Returns a typed located refusal on diagnostics, unsupported syntax, unsafe
/// variables, undefined arithmetic, or any exceeded source/expansion/formula limit.
/// Arithmetic in variable comparisons is checked on complete possible-positive
/// joins. Encountered undefined operations refuse the input rather than emulating
/// clingo's warning-and-drop behavior. Numeric typing of a variable objective
/// weight is checked later when its contribution is active in a verified model.
pub fn admit_formula(
    text: String,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula_with_grounding_observer(text, options, expansion, limits, None)
}

/// Like [`admit_formula`], with optional observation of actual eager grounding.
/// Source parsing, raising, analysis and IR preparation precede this boundary.
/// The caller retains its observer after errors; no clock is read by this API.
///
/// # Errors
/// Returns the same admission failures as [`admit_formula`].
pub fn admit_formula_with_grounding_observer(
    text: String,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
    observer: Option<&dyn crate::GroundingObserver>,
) -> Result<AdmittedFormula, FormulaFailure> {
    prepare_formula(text, options, expansion, limits)?.ground_with_observer(observer)
}

/// Prepare finite formula source without materializing possible support or a theory.
///
/// The result exposes the bounded analysis projection and retains the remaining
/// expansion budget for [`PreparedFormula::ground`]. Preparation establishes the
/// current compiler's source and binding contract, not successful grounding,
/// lazy-execution eligibility, or stable-model membership. Runtime arithmetic and
/// grounding limits are still checked when materialization is requested.
///
/// ```
/// use zetesis_themelios::{
///     AdmissionOptions, ExpansionLimits, FormulaLimits, prepare_formula,
/// };
/// let prepared = prepare_formula(
///     "edge(1,2). reach(X,Y) :- edge(X,Y).".into(),
///     AdmissionOptions::default(),
///     ExpansionLimits::default(),
///     FormulaLimits::default(),
/// )?;
/// assert!(prepared.source_analysis().safety().is_safe());
/// let admitted = prepared.ground()?;
/// assert_eq!(admitted.atoms().len(), 2);
/// # Ok::<(), zetesis_themelios::FormulaFailure>(())
/// ```
///
/// # Errors
/// Returns the parsing, raising, source-profile, analysis, binding and preparation
/// failures of [`admit_formula`], before the eager-grounding boundary.
pub fn prepare_formula(
    text: String,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<PreparedFormula, FormulaFailure> {
    let start = Location {
        source: options.source_id,
        span: Span::empty(ByteOffset::new(0)),
    };
    if text.len() > options.max_source_bytes {
        return Err(AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            limit: options.max_source_bytes,
            observed: text.len(),
            location: start,
        }
        .into());
    }
    let source =
        Source::new(options.source_id, text).map_err(|error| AdmissionFailure::Source {
            error,
            location: start,
        })?;
    let parsed = parse(&source, Dialect::Clingo);
    if !parsed.diagnostics().is_empty() {
        let diagnostics = parsed.diagnostics().to_vec();
        return Err(
            AdmissionFailure::Syntax(crate::SyntaxFailure::new(source, diagnostics)).into(),
        );
    }
    profile::check_formula(&parsed, options, false)?;
    extended::check_definitions_in(&parsed, expansion, &mut BTreeMap::new())?;
    metadata::check_count(&parsed, expansion, &mut 0)?;
    formula_ir::check_objectives(&parsed, &limits, &mut 0)?;
    let raised = raise(&parsed);
    if !raised.diagnostics().is_empty() {
        return Err(AdmissionFailure::Raise(raised.diagnostics().to_vec()).into());
    }
    let mut metadata = SourceMetadata::default();
    metadata::collect_profile(raised.program(), &mut metadata, true)?;
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let preparation = prepare(
        raised.program(),
        options,
        expansion,
        &limits,
        location,
        &mut metadata,
    )?;
    Ok(PreparedFormula::new(preparation, source, metadata.finish()))
}

/// Admit the same finite formula profile across original include graphs.
/// Include identity, global constants, and source metadata follow the existing
/// extended bundle contract, without textual concatenation.
///
/// # Errors
/// Retains the complete source catalog alongside every typed located refusal.
pub fn admit_bundle_formula(
    bundle: SourceBundle,
    options: BundleAdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
    admit_bundle_formula_with_grounding_observer(bundle, options, expansion, limits, None)
}

/// Bundle counterpart of [`admit_formula_with_grounding_observer`].
/// Existing source loading and parsing remain outside this admission boundary.
///
/// # Errors
/// Returns the same located failures and original bundle as [`admit_bundle_formula`].
pub fn admit_bundle_formula_with_grounding_observer(
    bundle: SourceBundle,
    options: BundleAdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
    observer: Option<&dyn crate::GroundingObserver>,
) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
    prepare_bundle_formula(bundle, options, expansion, limits)?.ground_with_observer(observer)
}

/// Prepare the finite formula profile across an original source bundle.
///
/// Include identity, global constants and metadata have the same contract as
/// [`admit_bundle_formula`]. The result exposes analysis before possible-support
/// completion and retains its remaining budget for materialization.
///
/// # Errors
/// Retains the original bundle alongside every located preparation refusal.
pub fn prepare_bundle_formula(
    bundle: SourceBundle,
    options: BundleAdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<PreparedFormulaBundle, FormulaBundleFailure> {
    match prepare_bundle(&bundle, options, expansion, &limits) {
        Ok((preparation, metadata)) => {
            Ok(PreparedFormulaBundle::new(preparation, bundle, metadata))
        }
        Err(error) => Err(FormulaBundleFailure {
            bundle,
            error: Box::new(error),
        }),
    }
}

fn prepare_bundle(
    bundle: &SourceBundle,
    options: BundleAdmissionOptions,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<(Preparation, SourceMetadata), FormulaFailure> {
    bundle_admission::check_include_identity(bundle)
        .map_err(|error| FormulaFailure::Include(Box::new(error)))?;
    let mut definitions = BTreeMap::new();
    let mut metadata_count = 0;
    let mut objective_count = 0;
    let mut metadata = SourceMetadata::default();
    let mut statements = Vec::new();
    let mut visited = 0;
    for source in bundle.sources() {
        let local = AdmissionOptions {
            source_id: source.id(),
            max_source_bytes: 0,
            max_syntax_nodes: options.max_syntax_nodes - visited,
            max_syntax_depth: options.max_syntax_depth,
            max_body_elements: options.max_body_elements,
            core_limits: options.core_limits,
        };
        visited += profile::check_formula(source.parsed(), local, true)?;
        extended::check_definitions_in(source.parsed(), expansion, &mut definitions)?;
        metadata::check_count(source.parsed(), expansion, &mut metadata_count)?;
        formula_ir::check_objectives(source.parsed(), limits, &mut objective_count)?;
        let raised = raise(source.parsed());
        if !raised.diagnostics().is_empty() {
            return Err(AdmissionFailure::Raise(raised.diagnostics().to_vec()).into());
        }
        metadata::collect_profile(raised.program(), &mut metadata, true)?;
        statements.extend(
            raised
                .program()
                .statements()
                .filter(|carrier| !matches!(carrier.get(), Statement::Include(_)))
                .cloned(),
        );
    }
    let entry = bundle
        .get(bundle.entry())
        .expect("loaded bundle has its entry");
    let location = Location {
        source: entry.id(),
        span: entry.source().span(),
    };
    let local = AdmissionOptions {
        source_id: entry.id(),
        core_limits: options.core_limits,
        max_body_elements: options.max_body_elements,
        ..AdmissionOptions::default()
    };
    let preparation = prepare(
        &SourceProgram::of(statements),
        local,
        expansion,
        limits,
        location,
        &mut metadata,
    )?;
    Ok((preparation, metadata.finish()))
}

fn prepare(
    source: &SourceProgram,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
    location: Location,
    metadata: &mut SourceMetadata,
) -> Result<Preparation, FormulaFailure> {
    let mut budget = crate::expansion::Budget::new(expansion, options.core_limits.max_templates);
    metadata.observations =
        crate::observation::compile(source, options, limits.observation, &mut budget, location)?;
    let prepared = formula_ir::prepare(source, options, limits, &mut budget, location)?;
    Ok(Preparation::new(prepared, budget, limits, location))
}

pub(crate) fn ceiling(
    resource: FormulaResource,
    observed: u128,
    limit: u128,
    location: Location,
) -> Result<(), FormulaFailure> {
    if observed > limit {
        Err(FormulaFailure::Limit {
            resource,
            observed,
            limit,
            location,
        })
    } else {
        Ok(())
    }
}
