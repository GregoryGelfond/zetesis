//! Opt-in finite source admission into general Ferraris formulas.

use std::collections::BTreeMap;
use std::fmt;

use themelios_base::diagnostic::Diagnostic;
use themelios_base::source::Source;
use themelios_base::span::Location;
use themelios_program::program::{Program as SourceProgram, Statement};
use zetesis_core::{Atom, AtomCatalog};
use zetesis_ferraris::Theory;

use crate::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionError, BundleAdmissionOptions,
    ExpansionFailure, ExpansionLimits, ParsedSource, SourceBundle, SourceFailure, SourceMetadata,
    bundle_admission, extended, formula_ir, formula_keys, metadata, profile,
};

mod preparation;
use preparation::Preparation;
pub use preparation::{PreparedFormula, PreparedFormulaBundle};

const DEFAULT_OBJECTIVE_PRESENCE_ENTRIES: usize = 16_384;

/// Independent finite grounding and formula-storage ceilings. No zero value
/// means unlimited. Source parsing and scalar expansion retain their own limits.
#[derive(Clone, Copy, Debug)]
pub struct FormulaLimits {
    /// Distinct typed atoms in the completed source projection domain.
    pub max_project_atoms: usize,
    /// Final retained projection vector capacity and logical atom payload.
    /// The temporary interner/order envelope is independently derived from
    /// `max_project_atoms`; new payload copies also consume `ScalarBytes`.
    /// Excludes source support and allocator metadata; this is not process RSS.
    pub max_project_bytes: usize,
    /// Conservative slots for objective-presence plans and shared source
    /// activity used by objectives and projection declarations. Includes
    /// predicate traversal, borrowed scope-frame capacity, completed carrier
    /// values and simultaneous old/new activity entries. These are logical
    /// planning slots, not allocator bytes; transient numeric subsets retain
    /// the independent assignment value ceiling.
    pub max_objective_presence_entries: usize,
    /// Typed atoms in one transient scoped objective-body formula. This scratch
    /// catalog cannot add atoms to the original theory or consume its atom cap.
    /// Source-activity and projection validation instead apply `theory.max_atoms`
    /// independently to each temporary builder, without adding original atoms.
    pub max_objective_formula_atoms: usize,
    /// DAG nodes in one transient scoped objective-body formula, including
    /// validation of rows later ignored for nonnumeric fields. Source-activity
    /// and projection validation instead apply `theory.max_nodes` independently
    /// to each temporary builder. Retained original-model objective query nodes
    /// have the independent `objective.max_condition_nodes` ceiling.
    pub max_objective_formula_nodes: usize,
    /// Distinct scalar values in the logical source, independent of join work.
    /// The set behind this count retains each value once so that its payload
    /// is charged to the byte budget once.
    pub max_domain_values: usize,
    /// Candidate values retained while evaluating one assignment, range or
    /// objective-presence subset. Applies independently to each such operation.
    pub max_assignment_values: usize,
    /// Distinct values emitted by binding generators across formula grounding,
    /// including support construction, objective preparation and final rule/local
    /// instantiation. This cumulative population is independent of one assignment
    /// and, like the domain, charges each distinct value's payload once.
    pub max_generated_values: usize,
    /// Distinct owned elements in one unconditional disjunctive head.
    pub max_disjunction_elements: usize,
    /// Retained row identifiers across bound-column support indexes, plus
    /// distinct variable/value entries in optional prepared finite-table indices.
    pub max_support_index_entries: usize,
    /// Live support atom-vector cells, equality layout, postings, snapshot
    /// objects, reusable query-workspace/index capacity and simultaneous row
    /// masks, including named query frames and operation scratch. Nested atom
    /// payloads, allocator/tree/control-runtime overhead and other grounding state retain
    /// separate bounds. This is not a total grounder-memory ceiling.
    pub max_support_bytes: usize,
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
    /// Total grounding expression, relation-view and formula construction work.
    /// Optional table preparation, domain resolution, mask selection and
    /// inspected mask words consume this same cumulative allowance.
    /// Includes shared metadata relocation, origin search/insertion, producer
    /// traversal and final root-evidence traversal/copy. Fixed atom and producer
    /// publication remains part of their existing construction work ticks.
    /// A checked bulk charge may exceed the ceiling by more than one; the
    /// refusal reports the cumulative amount requested before that operation.
    /// Origin classification first scans its already-admitted chain, then
    /// charges comparisons before insertion. First-occurrence origin admission
    /// retains precedence if both origin and work allowances are exhausted.
    pub max_work: u64,
    /// Complete rounds constructing the possible-positive support relation.
    pub max_support_rounds: u64,
    /// Original source locations retained in emitted formula-root evidence.
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
            max_project_atoms: 1_000_000,
            max_project_bytes: 67_108_864,
            // Match the bounded aggregate-plan row scale; this counts borrowed
            // pointer slots, not source bytes or semantic candidate atoms.
            max_objective_presence_entries: DEFAULT_OBJECTIVE_PRESENCE_ENTRIES,
            max_objective_formula_atoms: 65_536,
            max_objective_formula_nodes: 1_048_576,
            max_domain_values: 1_000_000,
            max_assignment_values: 1_000_000,
            max_generated_values: 1_000_000,
            max_disjunction_elements: 1_024,
            max_support_index_entries: 1_000_000,
            max_support_bytes: 134_217_728,
            max_aggregate_cache_rows: 16_384,
            max_aggregate_cache_key_bytes: 8_388_608,
            max_aggregate_cache_elements: 65_536,
            max_aggregate_cache_roots: 65_536,
            max_analysis_nodes: 262_144,
            max_analysis_edges: 1_000_000,
            max_substitutions: 1_000_000,
            max_work: 10_000_000,
            max_support_rounds: 1_000_000,
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
    /// Distinct completed source projection atoms.
    ProjectAtoms,
    /// Named completed source projection capacity and payload.
    ProjectBytes,
    /// Signed coefficient width required by finite integer binding analysis.
    /// The implementation capacity is fixed at 64 bits.
    BindingCoefficientBits,
    /// Signed accumulation width required by finite integer binding analysis.
    /// The implementation capacity is fixed at 128 bits.
    BindingBoundBits,
    /// Simultaneous objective-presence and shared source-activity planning slots.
    ObjectivePresenceEntries,
    /// Distinct finite scalar values.
    DomainValues,
    /// Candidate values in one assignment, range or presence subset.
    AssignmentValues,
    /// Distinct generated binding values across formula grounding.
    GeneratedValues,
    /// Row identifiers retained in positive-support column indexes.
    SupportIndexEntries,
    /// Live authored support snapshot, membership-index and query capacity.
    SupportBytes,
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
    /// Typed atoms in one transient objective-body formula.
    ObjectiveFormulaAtoms,
    /// Nodes in one transient objective-body formula.
    ObjectiveFormulaNodes,
    /// Named atom/interner capacity, bounded by a finite layout-derived envelope
    /// from the applicable atom ceiling. Nested payload remains under `ScalarBytes`.
    AtomStorageBytes,
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
    /// Formula construction metadata or final root evidence could not reserve capacity.
    MetadataAllocation {
        /// Original reservation error, independent of configured resource limits.
        error: std::collections::TryReserveError,
        /// Source occurrence whose construction required storage.
        location: Location,
    },
    /// Formula atom or lookup-index capacity could not be allocated.
    AtomAllocation {
        /// Original reservation error, independent of configured resource limits.
        error: std::collections::TryReserveError,
        /// Source occurrence whose new atom required storage.
        location: Location,
    },
    /// A finite-table index or row selection failed without publishing a result.
    SupportTable {
        /// Exact cause and completed operation receipts; never semantic UNSAT.
        error: zetesis_cpu::table::Failure,
        /// Original positive source occurrence.
        location: Location,
    },
    /// A typed relation view refused construction or query resolution.
    SupportRelation {
        /// Exact core refusal; never an empty relation or semantic UNSAT.
        error: zetesis_core::relation::Failure,
        /// Source location whose support operation was being performed.
        location: Location,
    },
    /// Original Boolean choice occurrences could not be preserved through the
    /// checked statement view. This refuses compilation, never answer sets.
    ChoiceSource {
        /// Original enclosing rule, or the source whose identity disagreed.
        location: Location,
    },
    /// Source activity disagrees with completed support or previously established
    /// information. No projection or objective program is published.
    SourceActivity {
        /// Source occurrence being prepared when the invariant was checked.
        location: Location,
    },
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
        /// Inclusive applicable ceiling, configured or fixed by the representation.
        limit: u128,
        /// Count required by the next operation.
        observed: u128,
        /// Original rule or source span.
        location: Location,
    },
    /// A required variable lacks a value or a positive binder in its scope.
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
            Self::MetadataAllocation { location, .. }
            | Self::AtomAllocation { location, .. }
            | Self::SupportRelation { location, .. }
            | Self::SupportTable { location, .. }
            | Self::ChoiceSource { location }
            | Self::SourceActivity { location }
            | Self::Limit { location, .. }
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
            Self::MetadataAllocation { error, .. } => {
                write!(f, "formula metadata storage: {error}")
            }
            Self::AtomAllocation { error, .. } => write!(f, "formula atom storage: {error}"),
            Self::SupportRelation { error, .. } => error.fmt(f),
            Self::SupportTable { error, .. } => error.fmt(f),
            Self::ChoiceSource { .. } => {
                f.write_str("Boolean choice source occurrences could not be preserved")
            }
            Self::SourceActivity { .. } => f.write_str(
                "source activity disagrees with completed support or established information",
            ),
            Self::Expansion(error) => error.fmt(f),
            Self::Include(error) => error.fmt(f),
            Self::Limit {
                resource,
                limit,
                observed,
                ..
            } => write!(f, "formula {resource} limit {limit}; required {observed}"),
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
            Self::MetadataAllocation { error, .. } | Self::AtomAllocation { error, .. } => {
                Some(error)
            }
            Self::SupportRelation { error, .. } => Some(error),
            Self::SupportTable { error, .. } => Some(error),
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

    /// Written constraints over a keyed value that were asked as the one atom
    /// their key admits before grounding, as the source guide describes. Zero
    /// when no constraint had the form; the answer sets are the same either way.
    #[must_use]
    pub fn keyed_constraints(&self) -> usize {
        self.compiled.keyed_constraints
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
        self.compiled.atoms.atoms()
    }
    /// Shared dense atom owner; selected interpretations retain its payloads
    /// after this source owner is dropped. No atom copying occurs when cloned.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
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

    /// Fixed projection domain completed with this exact original source.
    /// It changes only explicitly requested enumeration identity.
    #[must_use]
    pub fn projection(&self) -> &crate::PreparedProjection {
        &self.compiled.projection
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

    /// Written constraints over a keyed value that were asked as the one atom
    /// their key admits before grounding, as the source guide describes. Zero
    /// when no constraint had the form; the answer sets are the same either way.
    #[must_use]
    pub fn keyed_constraints(&self) -> usize {
        self.compiled.keyed_constraints
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
        self.compiled.atoms.atoms()
    }
    /// Shared dense atom owner used by retained interpretations.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
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

    /// Fixed projection domain completed with this original source bundle.
    #[must_use]
    pub fn projection(&self) -> &crate::PreparedProjection {
        &self.compiled.projection
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
    pub projection: crate::PreparedProjection,
    pub analysis_basis: AnalysisBasis,
    pub analysis: themelios_analysis::Analysis,
    pub analyzed: SourceProgram,
    pub theory: Theory,
    pub count_plan: crate::formula_count_plan::Outcome,
    pub atoms: AtomCatalog,
    pub origins: Vec<Vec<Location>>,
    pub objectives: zetesis_objective::ObjectiveProgram,
    pub objective_origins: Vec<Vec<Location>>,
    pub objective_declarations: Vec<Location>,
    pub keyed_constraints: usize,
}

/// Admit the extended scalar profile, finite conditional choices and body
/// count/sum/sum+/min/max aggregates into a Ferraris theory. Choices may have
/// complete logical bounds; each aggregate retains its separate value/endpoint profile.
/// Positive ordinary atoms bind variables in global or element-local scopes;
/// acyclic positive aggregate equalities may additionally bind named values.
/// Positive and double-negated integer-affine comparisons can generate finite
/// domains through directed closed endpoints, including coupled variables.
/// The complete correlated guard remains. Nonlinear expressions and relational
/// endpoints alone supply no new domain. Binding analysis has explicit signed
/// coefficient/accumulation capacity limits, separate from checked source arithmetic.
/// Completed aggregate/scalar proposals may feed ordinary evaluated arguments,
/// signed negative gates, finite ranges, logical choice bounds, nonbinding
/// aggregate guards and admitted universal body conditionals. Aggregate guard
/// comparisons retain their separate default-negation scope and never bind
/// additional variables. Finite count/sum/sum+ results compare with complete
/// bounds in ASP term order, without numeric coercion. A nonnumeric bound has
/// the same order against every integer; its canonical aggregate comparison
/// therefore retains constant original and frozen truth. Eligibility and
/// positive atom permissions are still validated and compiled separately.
/// Conditional local joins inherit the complete
/// outer binding; neither local witnesses nor vacuity establishes an aggregate
/// equality. Every generated rule retains its original equalities.
/// Default-negated anonymous consequent positions project the complete finite
/// witness family before applying `not` or `not not`. Anonymous positions may
/// occur inside positive function or tuple constructors; named and evaluated
/// inputs must be independently bound. Pools and intervals retain their source
/// alternative quantifier outside each projection. Classical-negative
/// predicates and anonymous unary/arithmetic inputs retain their safety refusal.
/// Ground scalar comparisons use ASP term order; arithmetic expressions
/// require numeric operands. Flat tuple equality/disequality is also supported.
///
/// Eligibility remains a formula even for recursive conditions. Duplicate
/// grounded head atoms combine permission by disjunction. Ordinary choices
/// count distinct signed atoms and original Boolean source occurrences; local
/// eligibility witnesses for each key combine by disjunction. Function heads
/// measure distinct complete tuples selected by any eligible signed operand.
/// All five measures permit either tuple/atom alias direction. Only unsigned
/// atoms supply choice permission or producer support; `not` and `not not`
/// retain their frozen reduct truth. Booleans introduce no atom or support.
/// Atom-only count certificates require wholly unsigned atomic groups with
/// a tuple/atom bijection. Head bounds are constraints and never invent support.
/// Numeric measure is separate from permission: missing/nonnumeric sum weights
/// are zero, and sum+ measures only strictly positive numeric weights. Every
/// positive head retains its independent permission. Unbounded heads need no
/// measure evaluation but still validate source terms and bindings. A bounded
/// extremum projects only present first values, using its ordinary empty value
/// if none remain. Missing tuples retain their independent permissions under
/// this declared extension; some original sources differ from clingo. Positive,
/// default-negated and double-negated ordinary element conditions retain their
/// original eligibility formulas.
/// Min/max first tuple values and guards use complete finite logical values in
/// ASP term order, including genuine extrema and structured values. Empty min is
/// `#sup`; empty max is `#inf`. The integer-endpoint limitation remains separate.
/// Necessary producer guards are double-negated and therefore leave reduct subsets
/// unconstrained whenever the candidate passes them. Grounding uses complete
/// relational joins over a bounded possible-positive closure. Gates are ignored
/// only while constructing that support upper bound. Aggregate guards are also
/// conservatively ignored there; their exact condition formulas remain in the
/// theory. Constraints never derive support. Source scopes are checked before
/// pruning. A bounded cursor proposes generated assignment values; each actual
/// equality formula remains in the reduct theory.
///
/// The objective profile retains lifted `#minimize`/`#maximize` elements and
/// admitted scoped weak constraints. Weights, priorities and tuple fields use
/// finite checked expressions over safely bound inputs; priority defaults to zero.
/// Conditions retain their admitted signed literals, scalar filters, aggregate
/// guards and independently checked local scopes. Source
/// weights normalize before global tuple deduplication; eligible maximize
/// `i32::MIN` weights receive a located overflow refusal. Nonnumeric priorities
/// contribute no key; undefined priority arithmetic remains a located error.
/// Dynamic priorities read ordinary bound positions or generated values with a
/// completed source-carrier certificate. Eligibility precision is selected per
/// objective: qualified positive dependencies can use a tighter carrier, while
/// other admitted dependencies use completed possible support. Those possible
/// values need not all realize in an answer. Source
/// priorities survive zero weights and inactive models; templates with no possible
/// positive/filter binding are omitted. Default-negated, aggregate and recursive
/// producer dependencies retain their original formulas; possible support does
/// not establish their truth in an answer. Original objective conditions are
/// evaluated over the verified full model, independently of the source carrier
/// used to retain priority presence. Unresolved binding dependencies and
/// unsupported local generators remain explicit refusals. Objectives supply no
/// logical support and are evaluated separately on reduct-verified full models.
/// This route invokes no solver, changes no existing S0/extended API contract,
/// and claims no parser-to-Lean refinement.
///
/// # Errors
/// Returns a typed located refusal on diagnostics, unsupported syntax, unsafe
/// variables, undefined arithmetic, or any exceeded source/expansion/formula limit.
/// A comparison over relationally bound variables that is defined and false
/// excludes its substitution, and nothing in an excluded substitution is
/// reached. Arithmetic is checked on every complete possible-positive
/// substitution the comparisons leave; an incomplete positive prefix with no
/// complete extension creates no such obligation. Row-dependent head atom
/// values are evaluated only after their rule or local element's body scalar
/// and range selection. Negative gates and aggregate truth remain formulas and
/// do not select this stage. Closed-term syntax and arithmetic validation still
/// occur during source preparation. Substitutions a binder, interval check,
/// tuple comparison or guard rejects still validate their bodies through
/// isolated formula scratch before being discarded; that scratch supplies no
/// atoms, roots, producers or objective keys to the admitted program. Rule-body
/// scratch uses the existing theory atom/node ceilings; objective-body scratch
/// retains its independent ceilings. Cumulative source work still applies.
/// A reached undefined operation refuses the input rather than emulating
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
    ParsedSource::new(text, options)?
        .prepare_formula(expansion, limits)
        .map_err(SourceFailure::into_error)
}

pub(crate) fn prepare_parsed(
    source: ParsedSource,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<PreparedFormula, SourceFailure<FormulaFailure>> {
    match prepare_source(&source, expansion, limits) {
        Ok((preparation, metadata)) => Ok(PreparedFormula::new(
            preparation,
            source.into_source(),
            metadata,
        )),
        Err(error) => Err(SourceFailure::new(source, error)),
    }
}

fn prepare_source(
    source: &ParsedSource,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<(Preparation, SourceMetadata), FormulaFailure> {
    let parsed = source.parsed();
    let options = source.options();
    profile::check_formula(parsed, options, false)?;
    extended::check_definitions_in(parsed, expansion, &mut BTreeMap::new())?;
    metadata::check_count(parsed, expansion, &mut 0)?;
    formula_ir::check_objectives(parsed, limits, &mut 0)?;
    let mut metadata = metadata::Builder::default();
    let mut budget = crate::expansion::Budget::new(expansion, options.core_limits.max_templates);
    let mut choices = crate::formula_choice_source::Catalog::default();
    let raised =
        crate::formula_choice_source::raise(parsed, &mut metadata, &mut budget, &mut choices)?;
    let location = Location {
        source: source.source().id(),
        span: source.source().span(),
    };
    let preparation = prepare(
        &raised,
        &choices,
        options,
        budget,
        limits,
        location,
        &mut metadata,
    )?;
    Ok((preparation, metadata.finish()))
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
    let mut metadata = metadata::Builder::default();
    let mut statements = Vec::new();
    let mut visited = 0;
    let mut budget = crate::expansion::Budget::new(expansion, options.core_limits.max_templates);
    let mut choices = crate::formula_choice_source::Catalog::default();
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
        let raised = crate::formula_choice_source::raise(
            source.parsed(),
            &mut metadata,
            &mut budget,
            &mut choices,
        )?;
        statements.extend(
            raised
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
        &SourceProgram::of_nodes(statements),
        &choices,
        local,
        budget,
        limits,
        location,
        &mut metadata,
    )?;
    Ok((preparation, metadata.finish()))
}

fn prepare(
    source: &SourceProgram,
    choices: &crate::formula_choice_source::Catalog,
    options: AdmissionOptions,
    mut budget: crate::expansion::Budget,
    limits: &FormulaLimits,
    location: Location,
    metadata: &mut metadata::Builder,
) -> Result<Preparation, FormulaFailure> {
    metadata.observations =
        crate::observation::compile(source, options, limits.observation, &mut budget, location)?;
    let prepared = formula_ir::prepare(source, choices, options, limits, &mut budget, location)?;
    // A constraint over a keyed value is asked as the one atom its key admits;
    // the asked program is prepared again under the remaining budget.
    let prepared = match formula_keys::rewrite(source, &prepared, &mut budget, location)? {
        Some(asked) => {
            let mut prepared = formula_ir::prepare(
                &asked.program,
                choices,
                options,
                limits,
                &mut budget,
                location,
            )?;
            prepared.keyed_constraints = asked.constraints;
            prepared
        }
        None => prepared,
    };
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
