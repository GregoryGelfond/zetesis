//! Opt-in finite source admission into general Ferraris formulas.

use crate::formula_owner::Owner;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::ProgramSite;
use themelios_base::diagnostic::Diagnostic;
use themelios_base::source::Source;
use themelios_program::program::{Program as SourceProgram, Statement};
use zetesis_core::AtomCatalog;
use zetesis_core::catalog::Atoms;
use zetesis_ferraris::Theory;

use crate::{
    AdmissionFailure, AdmissionOptions, BundleAdmissionError, BundleAdmissionOptions,
    ExpansionFailure, ExpansionLimits, ParsedSource, SourceBundle, SourceFailure, SourceMetadata,
    bundle_admission, extended, formula_ir, metadata, profile,
};

mod preparation;
pub(crate) use preparation::Preparation;
pub use preparation::{PreparedFormula, PreparedFormulaBundle};

const DEFAULT_OBJECTIVE_PRESENCE_ENTRIES: usize = 16_384;

/// Independent finite grounding and formula-storage ceilings. No zero value
/// means unlimited. Source parsing and scalar expansion retain their own limits.
#[derive(Clone, Copy, Debug)]
pub struct FormulaLimits {
    /// Inclusive named storage when publishing a formula atom catalog:
    /// the shared source authority's canonical text/terms/rows, discovery and
    /// lookup metadata, current prefix directories and allocation overlap.
    /// Includes identities outside the published occurrence map; shared payload
    /// is counted once within the authority. This independent 128 MiB default
    /// is not cumulative expansion `ScalarBytes`; zero is a real ceiling.
    /// Excludes unrelated support indexes/query buffers, other formula state,
    /// allocator bookkeeping and Arc counters. Projection additionally applies
    /// its retained `max_project_bytes` ceiling.
    pub max_atom_storage_bytes: usize,
    /// Distinct typed atoms in the completed source projection domain.
    pub max_project_atoms: usize,
    /// Complete retained canonical prefix and ordered projection ID-map capacity,
    /// including prefix identities outside that map.
    /// Construction indexes, directories and scratch use `max_atom_storage_bytes`
    /// independently. Fixed empty catalog envelopes are excluded so an empty
    /// explicit domain remains admissible with zero atoms and bytes.
    /// Excludes unrelated support indexes/query buffers and allocator metadata;
    /// this is not process RSS.
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
    /// Live support canonical payload and identity indexes, current prefix,
    /// relation/equality metadata, postings, borrowed snapshot objects, reusable
    /// query-workspace/index capacity and simultaneous row masks, including
    /// named query frames, aggregate coordinate/cache buffers, contribution
    /// buffers, operation scratch and growth overlap. The authority
    /// counts canonical payload once. Allocator/tree/control-runtime overhead
    /// and other grounding state remain separate; this is not a total-memory
    /// or process RSS ceiling.
    pub max_support_bytes: usize,
    /// Distinct aggregate/outer-binding entries retained during final grounding.
    pub max_aggregate_cache_rows: usize,
    /// Retained aggregate context rows, key-coordinate pool and lookup index
    /// capacities. Canonical payload is shared with support; allocator overhead
    /// is excluded. Replacement overlap is checked by `max_support_bytes`.
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
    /// Original statement sites retained in emitted formula-root evidence.
    pub max_origin_locations: usize,
    /// Distinct warning sites retained after successful formula admission.
    /// Repeated evaluations at one statement site retain one warning, not row counts.
    pub max_warnings: usize,
    /// Steps of the key analysis that asks constraints over keyed values, and
    /// of its readings of facts, also bounded by the term work remaining; the
    /// steps spent are charged to the term work. A stop leaves every
    /// constraint not yet asked as written and is reported.
    pub max_key_work: u64,
    /// Final dense atom, formula-node, and theory-root storage ceilings.
    pub theory: zetesis_ferraris::AdmissionLimits,
    /// Per-aggregate translation ceilings, additionally capped by total formula work/nodes.
    pub aggregate: zetesis_ferraris::AggregateLimits,
    /// Independent admission ceilings for lifted objective templates.
    pub objective: zetesis_objective::AdmissionLimits,
    /// Independent term observation template ceilings.
    pub observation: crate::observation::AdmissionLimits,
    /// Independent canonical vocabulary/component capacity for source metadata.
    pub metadata_storage: crate::MetadataStorageLimits,
}
impl Default for FormulaLimits {
    fn default() -> Self {
        Self {
            max_atom_storage_bytes: 134_217_728,
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
            max_warnings: 10_000,
            max_key_work: 1_000_000,
            theory: zetesis_ferraris::AdmissionLimits::default(),
            objective: zetesis_objective::AdmissionLimits::default(),
            observation: crate::observation::AdmissionLimits::default(),
            metadata_storage: crate::MetadataStorageLimits::default(),
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
    /// Named canonical atom payload and interner metadata, including reservation
    /// overlap, bounded independently by `max_atom_storage_bytes`.
    AtomStorageBytes,
    /// Dense semantic atoms.
    Atoms,
    /// Formula DAG nodes.
    Nodes,
    /// Theory roots.
    Roots,
    /// Retained original statement sites.
    Origins,
    /// Distinct statement warnings retained by successful admission.
    Warnings,
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

/// A typed refusal of finite formula admission; never semantic UNSAT.
#[derive(Debug)]
pub enum FormulaFailure {
    /// A typed-input refusal retaining the caller's original canonical program.
    Program {
        /// Original program before normalization or analysis projection.
        program: Arc<SourceProgram>,
        /// Typed preparation or later grounding cause.
        error: Box<FormulaFailure>,
    },
    /// Constructed input exceeded a structural bound or excluded a capability.
    Logical {
        /// Independent logical-structure cause.
        error: crate::ProgramFailureKind,
        /// Canonical part position for a part-wide refusal.
        part: Option<usize>,
        /// Original statement when one is responsible.
        location: ProgramSite,
    },
    /// Canonical template metadata could not retain its source authority.
    TemplateCatalog {
        /// Exact identity, storage, or incomplete-publication cause.
        error: zetesis_core::TemplateCatalogFailure,
        /// Source occurrence whose component admission failed.
        location: ProgramSite,
    },
    /// An immutable admitted source lacks a term required by its checker.
    UnadmittedTerm {
        /// Source occurrence whose computed value was not admitted.
        location: ProgramSite,
    },
    /// Scoped binding metadata could not be resolved or stored.
    TermAssignment {
        /// Exact authority, prefix, slot or storage cause.
        error: zetesis_core::catalog::AssignmentError,
        /// Source occurrence whose binding operation failed.
        location: ProgramSite,
    },
    /// A catalog-backed original-model objective condition is invalid.
    ObjectiveCondition {
        /// Exact occurrence, backward-reference or canonical-storage cause.
        error: zetesis_objective::ConditionError,
        /// Source occurrence whose query was being constructed.
        location: ProgramSite,
    },
    /// Canonical atom storage or representation could not be admitted.
    AtomCatalog {
        /// Exact canonical refusal, distinct from logical absence.
        error: zetesis_core::catalog::Error,
        /// Source occurrence whose operation required canonical storage.
        location: ProgramSite,
    },
    /// A cancellable streamed-source operation stopped before completion.
    Interrupted {
        /// Cancellation or deadline observed at a charged work boundary.
        reason: zetesis_cpu::Stop,
        /// Original source occurrence whose operation stopped.
        location: ProgramSite,
    },
    /// The explicitly requested hybrid schedule cannot handle this capability.
    HybridUnsupported {
        /// Capability that remains available through eager grounding.
        feature: crate::HybridFeature,
        /// Original source occurrence, or the source for a join policy.
        location: ProgramSite,
    },
    /// Formula construction metadata or final root evidence could not reserve capacity.
    MetadataAllocation {
        /// Original reservation error, independent of configured resource limits.
        error: std::collections::TryReserveError,
        /// Source occurrence whose construction required storage.
        location: ProgramSite,
    },
    /// Formula atom or lookup-index capacity could not be allocated.
    AtomAllocation {
        /// Original reservation error, independent of configured resource limits.
        error: std::collections::TryReserveError,
        /// Source occurrence whose new atom required storage.
        location: ProgramSite,
    },
    /// A finite-table index or row selection failed without publishing a result.
    SupportTable {
        /// Exact cause and completed operation receipts; never semantic UNSAT.
        error: zetesis_cpu::table::Failure,
        /// Original positive source occurrence.
        location: ProgramSite,
    },
    /// A typed relation view refused construction or query resolution.
    SupportRelation {
        /// Exact core refusal; never an empty relation or semantic UNSAT.
        error: zetesis_core::relation::Failure,
        /// Source location whose support operation was being performed.
        location: ProgramSite,
    },
    /// Source activity disagrees with completed support or previously established
    /// information. No projection or objective program is published.
    SourceActivity {
        /// Source occurrence being prepared when the invariant was checked.
        location: ProgramSite,
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
        location: ProgramSite,
    },
    /// A required variable lacks a value or a positive binder in its scope.
    UnsafeVariable {
        /// Dense variable index in the rule or local element scope.
        variable: usize,
        /// Original rule span.
        location: ProgramSite,
    },
    /// An evaluated positive argument lacks an independently established input.
    /// This profile does not invert arithmetic to discover source bindings.
    UnboundArgumentInput {
        /// Dense source variable index in the rule or local element scope.
        variable: usize,
        /// Original enclosing rule span.
        location: ProgramSite,
    },
    /// A finite value instruction has no independent producer for an input.
    UnboundValueInput {
        /// Dense input slot in the enclosing rule.
        variable: usize,
        /// Original enclosing rule span.
        location: ProgramSite,
    },
    /// Finite value instructions have a cyclic input dependency. This is a
    /// native scheduling refusal, not an impossibility claim about ASP recursion.
    CyclicValueInput {
        /// An input slot in the unscheduled dependency component.
        variable: usize,
        /// Original enclosing rule span.
        location: ProgramSite,
    },
    /// A bounded finite aggregate translation was refused.
    Aggregate {
        /// Typed constructor failure with partial accounting.
        error: zetesis_ferraris::AggregateError,
        /// Original enclosing rule span.
        location: ProgramSite,
    },
    /// The independent lifted objective admission door rejected construction.
    Objective {
        /// Typed objective admission error.
        error: zetesis_objective::AdmissionError,
        /// Original objective occurrence.
        location: ProgramSite,
    },
    /// The independent formula-theory admission door rejected construction.
    Theory {
        /// Typed formula error.
        error: zetesis_ferraris::AdmissionError,
        /// Original source span.
        location: ProgramSite,
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
    /// The typed cause beneath any retained original-program envelope.
    #[must_use]
    pub fn cause(&self) -> &Self {
        let mut cause = self;
        while let Self::Program { error, .. } = cause {
            cause = error;
        }
        cause
    }

    /// Logical or parsed subject carried by this failure.
    #[must_use]
    pub fn site(&self) -> Option<ProgramSite> {
        match self {
            Self::Program { error, .. } => error.site(),
            Self::Expansion(error) => error.site(),
            Self::Include(_) => None,
            Self::Observation { error } => Some(error.site()),
            Self::Logical { location, .. }
            | Self::Interrupted { location, .. }
            | Self::HybridUnsupported { location, .. }
            | Self::MetadataAllocation { location, .. }
            | Self::AtomAllocation { location, .. }
            | Self::AtomCatalog { location, .. }
            | Self::TermAssignment { location, .. }
            | Self::UnadmittedTerm { location }
            | Self::ObjectiveCondition { location, .. }
            | Self::TemplateCatalog { location, .. }
            | Self::SupportRelation { location, .. }
            | Self::SupportTable { location, .. }
            | Self::SourceActivity { location }
            | Self::Limit { location, .. }
            | Self::UnsafeVariable { location, .. }
            | Self::UnboundArgumentInput { location, .. }
            | Self::UnboundValueInput { location, .. }
            | Self::CyclicValueInput { location, .. }
            | Self::Theory { location, .. }
            | Self::Objective { location, .. }
            | Self::Aggregate { location, .. } => Some(*location),
        }
    }

    /// Original canonical input retained by a typed-input failure.
    #[must_use]
    pub fn original_program(&self) -> Option<&SourceProgram> {
        match self {
            Self::Program { program, .. } => Some(program),
            _ => None,
        }
    }

    /// Resolve a typed-input refusal to the caller's original logical object.
    #[must_use]
    pub fn subject(&self) -> Option<crate::ProgramSubject<'_>> {
        let Self::Program { program, error } = self else {
            return None;
        };
        if let Some(statement) = error.site().and_then(ProgramSite::statement_id) {
            return program
                .statements()
                .nth(statement.index())
                .map(crate::ProgramSubject::Statement);
        }
        if let Self::Logical {
            part: Some(index), ..
        } = error.as_ref()
        {
            return program
                .parts()
                .nth(*index)
                .map(|part| crate::ProgramSubject::Part(part.key()));
        }
        Some(crate::ProgramSubject::Program)
    }

    /// Located diagnostics; bundle identities resolve through its retained catalog.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        match self {
            Self::Program { error, .. } => error.diagnostics(),
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
            Self::Logical { location, .. }
            | Self::Interrupted { location, .. }
            | Self::HybridUnsupported { location, .. }
            | Self::MetadataAllocation { location, .. }
            | Self::AtomAllocation { location, .. }
            | Self::AtomCatalog { location, .. }
            | Self::TermAssignment { location, .. }
            | Self::UnadmittedTerm { location }
            | Self::ObjectiveCondition { location, .. }
            | Self::TemplateCatalog { location, .. }
            | Self::SupportRelation { location, .. }
            | Self::SupportTable { location, .. }
            | Self::SourceActivity { location }
            | Self::Limit { location, .. }
            | Self::UnsafeVariable { location, .. }
            | Self::UnboundArgumentInput { location, .. }
            | Self::UnboundValueInput { location, .. }
            | Self::CyclicValueInput { location, .. }
            | Self::Theory { location, .. }
            | Self::Objective { location, .. }
            | Self::Aggregate { location, .. } => location
                .location()
                .map(|location| {
                    crate::diagnostic::diagnostic("formula-admission", self.to_string(), location)
                })
                .into_iter()
                .collect(),
        }
    }
}
impl fmt::Display for FormulaFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program { error, .. } => error.fmt(f),
            Self::Logical { error, .. } => error.fmt(f),
            Self::Interrupted { reason, .. } => reason.fmt(f),
            Self::HybridUnsupported { feature, .. } => {
                write!(f, "hybrid grounding does not support {feature}")
            }
            Self::MetadataAllocation { error, .. } => {
                write!(f, "formula metadata storage: {error}")
            }
            Self::AtomAllocation { error, .. } => write!(f, "formula atom storage: {error}"),
            Self::AtomCatalog { error, .. } => write!(f, "formula canonical atom storage: {error}"),
            Self::TermAssignment { error, .. } => write!(f, "formula term assignment: {error}"),
            Self::UnadmittedTerm { .. } => {
                f.write_str("completed source vocabulary lacks a required computed term")
            }
            Self::ObjectiveCondition { error, .. } => error.fmt(f),
            Self::TemplateCatalog { error, .. } => error.fmt(f),
            Self::SupportRelation { error, .. } => error.fmt(f),
            Self::SupportTable { error, .. } => error.fmt(f),
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
            } => write!(
                f,
                "formula {resource} limit {limit} exceeded (needed at least {observed}); raise the applicable resource limit or simplify the program"
            ),
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
            Self::Program { error, .. } => Some(error.as_ref()),
            Self::Logical { error, .. } => Some(error),
            Self::Interrupted { reason, .. } => Some(reason),
            Self::AtomCatalog { error, .. } => Some(error),
            Self::TermAssignment { error, .. } => Some(error),
            Self::ObjectiveCondition { error, .. } => Some(error),
            Self::TemplateCatalog { error, .. } => Some(error),
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

/// A bounded finite formula theory retaining its original canonical input.
#[derive(Debug)]
pub struct AdmittedFormula {
    compiled: Compiled,
    source: Owner,
    metadata: SourceMetadata,
}
impl AdmittedFormula {
    /// Warnings from omitted instances, deduplicated in source-location order.
    /// The retained space is bounded by [`FormulaLimits::max_warnings`].
    #[must_use]
    pub fn warnings(&self) -> &[crate::FormulaWarning] {
        &self.compiled.warnings
    }

    /// Render warnings against the retained original source with themelios's
    /// human view. Rendering builds one source line index and one diagnostic at
    /// a time; formatter failures stop rendering. An empty collection is empty.
    #[must_use]
    pub fn warning_view(&self) -> impl fmt::Display + '_ {
        self.source.warning_view(self.warnings())
    }

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

    /// What preparation and grounding charged under the expansion limits:
    /// the receipt the extended profile reports, from the one budget this
    /// admission kept from its first statement to its last root.
    #[must_use]
    pub fn expansion_usage(&self) -> &crate::ExpansionUsage {
        &self.compiled.expansion
    }

    /// How the key analysis behind [`Self::keyed_constraints`] ended: complete,
    /// or stopped by its work ceiling with every constraint not yet asked
    /// left as written.
    #[must_use]
    pub fn key_analysis(&self) -> crate::KeyAnalysis {
        self.compiled.key_analysis
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
    pub fn atoms(&self) -> Atoms<'_> {
        self.compiled.atoms.atoms()
    }
    /// Shared dense atom owner; selected interpretations retain its payloads
    /// after this source owner is dropped. No atom copying occurs when cloned.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
        &self.compiled.atoms
    }
    /// Original statement sites per theory root, preserving merged source evidence.
    /// Necessary support guards collect producer origins and the first atom occurrence.
    #[must_use]
    pub fn formula_origins(&self) -> &[Vec<ProgramSite>] {
        &self.compiled.origins
    }
    /// Original bytes and source identity, absent for a constructed input.
    #[must_use]
    pub fn source(&self) -> Option<&Source> {
        self.source.source()
    }
    /// Original canonical input before normalization or analysis projection.
    #[must_use]
    pub fn original_program(&self) -> &SourceProgram {
        self.source.program()
    }
    /// Lifted objectives evaluated only after stable-model membership is verified.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.compiled.objectives
    }
    /// Original objective element origins, parallel to active objective templates.
    #[must_use]
    pub fn objective_origins(&self) -> &[Vec<ProgramSite>] {
        &self.compiled.objective_origins
    }
    /// Every original objective declaration, including statically unreachable ones.
    #[must_use]
    pub fn objective_declarations(&self) -> &[ProgramSite] {
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
    source: Owner,
    metadata: SourceMetadata,
}
impl AdmittedFormulaBundle {
    /// Warnings from omitted instances, deduplicated in source-location order.
    /// The retained space is bounded by [`FormulaLimits::max_warnings`].
    #[must_use]
    pub fn warnings(&self) -> &[crate::FormulaWarning] {
        &self.compiled.warnings
    }

    /// Render warnings against the retained original include catalog with
    /// themelios's human view. Rendering uses the catalog's source indexes and
    /// one diagnostic at a time; formatter failures stop rendering.
    #[must_use]
    pub fn warning_view(&self) -> impl fmt::Display + '_ {
        self.source.warning_view(self.warnings())
    }

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

    /// What preparation and grounding charged under the expansion limits:
    /// the receipt the extended profile reports, from the one budget this
    /// admission kept from its first statement to its last root.
    #[must_use]
    pub fn expansion_usage(&self) -> &crate::ExpansionUsage {
        &self.compiled.expansion
    }

    /// How the key analysis behind [`Self::keyed_constraints`] ended: complete,
    /// or stopped by its work ceiling with every constraint not yet asked
    /// left as written.
    #[must_use]
    pub fn key_analysis(&self) -> crate::KeyAnalysis {
        self.compiled.key_analysis
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
    pub fn atoms(&self) -> Atoms<'_> {
        self.compiled.atoms.atoms()
    }
    /// Shared dense atom owner used by retained interpretations.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
        &self.compiled.atoms
    }
    /// Original statement sites per theory root, with parsed evidence when available.
    #[must_use]
    pub fn formula_origins(&self) -> &[Vec<ProgramSite>] {
        &self.compiled.origins
    }
    /// Every original file, source identity, and include occurrence.
    /// Bundle admission retains this catalog by construction.
    #[must_use]
    pub fn bundle(&self) -> &SourceBundle {
        self.source.required_bundle()
    }
    /// Original canonical input before normalization or analysis projection.
    #[must_use]
    pub fn original_program(&self) -> &SourceProgram {
        self.source.program()
    }
    /// Lifted objectives evaluated only after stable-model membership is verified.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.compiled.objectives
    }
    /// Original objective element origins, parallel to active objective templates.
    #[must_use]
    pub fn objective_origins(&self) -> &[Vec<ProgramSite>] {
        &self.compiled.objective_origins
    }
    /// Every original objective declaration, including statically unreachable ones.
    #[must_use]
    pub fn objective_declarations(&self) -> &[ProgramSite] {
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
    /// describe this projection; they are not conclusions about source
    /// semantics. The keyed rewrite reads it soundly, as its module argues:
    /// a projected statement is never a key's producer nor an asked
    /// constraint.
    DependencyProjection,
}

#[derive(Debug)]
pub(crate) struct Compiled {
    pub warnings: Vec<crate::FormulaWarning>,
    pub projection: crate::PreparedProjection,
    pub analysis_basis: AnalysisBasis,
    pub analysis: themelios_analysis::Analysis,
    pub analyzed: SourceProgram,
    pub theory: Theory,
    pub count_plan: crate::formula_count_plan::Outcome,
    pub atoms: AtomCatalog,
    pub origins: Vec<Vec<ProgramSite>>,
    pub objectives: zetesis_objective::ObjectiveProgram,
    pub objective_origins: Vec<Vec<ProgramSite>>,
    pub objective_declarations: Vec<ProgramSite>,
    pub keyed_constraints: usize,
    pub key_analysis: crate::KeyAnalysis,
    pub expansion: crate::ExpansionUsage,
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
/// count distinct signed atoms and pool-expanded Boolean occurrences; local
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
/// contribute no key after arithmetic validation. Weight, priority and tuple
/// fields participate jointly in the source-family policy below, including
/// when the weight is nonnumeric. Pooled fragments of one original objective
/// element share its family; distinct elements cannot rescue each other.
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
/// variables, fatal arithmetic, an entirely undefined source family, or any
/// exceeded source/expansion/formula limit.
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
/// Only evaluated numeric division or remainder by zero may omit a source
/// instance. Admission requires a jointly defined instance in that same complete
/// original family; a defined but false instance is a witness. An empty positive
/// join is silent. Each local choice or aggregate element has a separate family
/// for each fixed outer binding. Successful owners retain typed warnings,
/// deduplicated by source span and bounded by [`FormulaLimits::max_warnings`].
/// Overflow, nonnumeric arithmetic and invalid exponents remain fatal. Source
/// evaluation checks independent expression branches and fields after a zero
/// divisor within each reached phase. An operation depending on an undefined
/// operand is not evaluated.
/// An omitted body or condition does not enter its later head, consequent or
/// objective-field phase. Defined false body selection likewise skips head and
/// consequent evaluation; independent-error checks do not cross that boundary.
/// This policy does not weaken closed-term preparation or the strict arithmetic
/// errors of post-solve observations. Numeric typing of a variable objective
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

/// Admit a canonical logical program into a complete finite formula theory.
/// This composes [`prepare_program_formula`] with [`PreparedFormula::ground`].
///
/// # Errors
/// Retains the original program alongside any preparation or grounding failure.
pub fn admit_program_formula(
    program: Arc<SourceProgram>,
    options: crate::ProgramAdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    prepare_program_formula(program, options, expansion, limits)?.ground()
}

/// Prepare a canonical logical program through the general formula pipeline.
///
/// This door never renders or reparses its input. It borrows the shared original
/// program during bounded structural inspection, then uses the same normalization,
/// analysis and grounding preparation as source admission. Statement identities
/// refer to this original program, including failures during deferred grounding.
/// Eager, hybrid and adaptive materialization remain available on the receipt.
///
/// # Errors
/// Retains the original program with structural, capability, preparation and
/// arithmetic failures. An absent source coordinate is never fabricated.
pub fn prepare_program_formula(
    program: Arc<SourceProgram>,
    options: crate::ProgramAdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<PreparedFormula, FormulaFailure> {
    let owner = Owner::logical(program);
    let prepare = || {
        crate::formula_program_check::check(owner.program(), options)
            .map_err(|error| logical_failure(owner.program(), error))?;
        crate::formula_program_check::check_objectives(owner.program(), &limits)?;
        metadata::check_program_count(owner.program(), expansion)?;
        let mut metadata = metadata::Builder::new(limits.metadata_storage);
        metadata::collect_profile(owner.program(), &mut metadata, true)?;
        let budget = crate::expansion::Budget::new(expansion, options.core_limits.max_templates);
        prepare(
            owner.program(),
            options.into(),
            budget,
            &limits,
            ProgramSite::program(),
            metadata,
        )
    };
    match prepare() {
        Ok((preparation, metadata)) => Ok(PreparedFormula::new(preparation, owner, metadata)),
        Err(error) => Err(owner.retain_failure(error)),
    }
}

fn logical_failure(
    program: &SourceProgram,
    error: crate::ProgramAdmissionFailure<'_>,
) -> FormulaFailure {
    let (part, location) = match error.subject {
        crate::ProgramSubject::Program => (None, ProgramSite::program()),
        crate::ProgramSubject::Part(key) => (
            program
                .parts()
                .position(|part| std::ptr::eq(part.key(), key)),
            ProgramSite::program(),
        ),
        crate::ProgramSubject::Statement(statement) => {
            let index = program
                .statements()
                .position(|carrier| std::ptr::eq(carrier, statement))
                .expect("structural inspection borrows an original statement");
            (
                None,
                extended::origin(
                    statement,
                    ProgramSite::statement(crate::StatementId::new(index), None),
                ),
            )
        }
    };
    FormulaFailure::Logical {
        error: error.kind,
        part,
        location,
    }
}

pub(crate) fn prepare_parsed(
    source: ParsedSource,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<PreparedFormula, SourceFailure<FormulaFailure>> {
    match prepare_source(&source, expansion, limits) {
        Ok((preparation, metadata, program)) => Ok(PreparedFormula::new(
            preparation,
            Owner::single(program, source.into_source()),
            metadata,
        )),
        Err(error) => Err(SourceFailure::new(source, error)),
    }
}

fn prepare_source(
    source: &ParsedSource,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<(Preparation, SourceMetadata, Arc<SourceProgram>), FormulaFailure> {
    let parsed = source.parsed();
    let options = source.options();
    profile::check_formula(parsed, options, false)?;
    extended::check_definitions_in(parsed, expansion, &mut BTreeMap::new())?;
    metadata::check_count(parsed, expansion, &mut 0)?;
    formula_ir::check_objectives(parsed, limits, &mut 0)?;
    let mut metadata = metadata::Builder::new(limits.metadata_storage);
    let budget = crate::expansion::Budget::new(expansion, options.core_limits.max_templates);
    let raised = Arc::new(crate::formula_raise::raise(parsed, &mut metadata)?);
    let location = ProgramSite::source(themelios_base::span::Location {
        source: source.source().id(),
        span: source.source().span(),
    });
    let (preparation, metadata) =
        prepare(&raised, options.into(), budget, limits, location, metadata)?;
    Ok((preparation, metadata, raised))
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
        Ok((preparation, metadata, program)) => Ok(PreparedFormulaBundle::new(
            preparation,
            Owner::bundle(program, bundle),
            metadata,
        )),
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
) -> Result<(Preparation, SourceMetadata, Arc<SourceProgram>), FormulaFailure> {
    bundle_admission::check_include_identity(bundle)
        .map_err(|error| FormulaFailure::Include(Box::new(error)))?;
    let mut definitions = BTreeMap::new();
    let mut metadata_count = 0;
    let mut objective_count = 0;
    let mut metadata = metadata::Builder::new(limits.metadata_storage);
    let mut statements = Vec::new();
    let mut visited = 0;
    let budget = crate::expansion::Budget::new(expansion, options.core_limits.max_templates);
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
        let raised = crate::formula_raise::raise(source.parsed(), &mut metadata)?;
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
    let location = ProgramSite::source(themelios_base::span::Location {
        source: entry.id(),
        span: entry.source().span(),
    });
    let program = Arc::new(SourceProgram::of_nodes(statements));
    let (preparation, metadata) =
        prepare(&program, options.into(), budget, limits, location, metadata)?;
    Ok((preparation, metadata, program))
}

fn prepare(
    source: &SourceProgram,
    options: formula_ir::CompilationOptions,
    mut budget: crate::expansion::Budget,
    limits: &FormulaLimits,
    location: ProgramSite,
    mut metadata: metadata::Builder,
) -> Result<(Preparation, SourceMetadata), FormulaFailure> {
    metadata.compile_observations(source, options, limits.observation, &mut budget, location)?;
    let metadata = metadata.finish(location)?;
    let mut catalog = crate::formula_support::SupportCatalog::default();
    let mut counters = crate::formula_support::Counters::default();
    let prepared = formula_ir::PreparationContext {
        options,
        budget: &mut budget,
        catalog: &mut catalog,
        work: crate::formula_support::GroundingWork::new(limits, &mut counters, location),
    }
    .prepare(source, metadata.project_selection().clone())?;
    let accounting = counters.into_accounting();
    Ok((
        Preparation::new(prepared, catalog, accounting, budget, limits, location),
        metadata,
    ))
}

pub(crate) fn ceiling(
    resource: FormulaResource,
    observed: u128,
    limit: u128,
    location: ProgramSite,
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
