//! Admission of source and canonical logical programs through pinned themelios.
//! [`prepare_program_formula`] retains its original program through an `Arc` and
//! enters the shared formula compiler without rendering or reparsing.
//! The formula APIs compute a bounded possible-positive
//! relation and ground complete joins into normal, conditional-choice, and finite-aggregate Ferraris
//! formulas. Lifted minimization templates remain separate and score only supplied
//! verified stable models. No source route invokes a solver or external engine.
//!
//! Source admission retains original bytes and parsed origins on success;
//! bundle failures retain the complete source catalog. Typed formula input keeps
//! original statement identity and any actual coordinates, without inventing
//! source bytes. Independent source,
//! syntax, scalar expansion, finite substitution, and formula storage limits
//! refuse rather than return recovered or truncated programs. Finite count/sum
//! assignment cursors may generate values absent from the source while retaining
//! exact reduct equality formulas. Scalar equality and interval bindings follow
//! dependency-ordered plans; evaluated normal heads use fresh value slots and
//! bounded streamed cursors. Pinned upstream analysis reads a separately bounded
//! pool-free owned projection; its facts are retained without claiming
//! its narrower safety reading proves the clingo assignment extension.
//! Syntax walks and
//! term normalization are iterative. The formula route preserves recursive
//! eligibility and uses constraints for choice bounds. Completed theories include
//! a necessary support guard for every atom whose producers all have a
//! condition; an atom with an unconditional producer needs none. These
//! semantically redundant guards remain double-negated so they do not
//! constrain reduct subsets; they are not a selectable constructor option.
//! Source acceptance is not a parser-to-Lean or Rust implementation proof.
#![forbid(unsafe_code)]

mod compile;
mod diagnostic;
mod parsed_source;
mod profile;
mod program_admission;
mod program_limits;
mod program_site;
mod source_diagnostics;
pub use program_site::{ProgramSite, StatementId};
mod bundle;
mod bundle_admission;
mod coherence;
mod expansion;
mod extended;
mod fact_expansion;
mod formula;
mod formula_aggregate_ir;
mod formula_analysis;
mod formula_assignment;
mod formula_assignment_ir;
mod formula_assignment_plan;
mod formula_binding;
mod formula_binding_cursor;
mod formula_binding_guard;
mod formula_binding_ir;
mod formula_binding_plan;
mod formula_choice_ir;
mod formula_conditional;
mod formula_conditional_head_ir;
mod formula_conditional_ir;
mod formula_conditional_projection;
mod formula_consequent_ir;
mod formula_factor;
mod formula_facts;
mod formula_ground;
mod formula_guard;
mod formula_head_aggregate;
mod formula_head_ir;
mod formula_ir;
mod formula_objective_dependencies;
mod formula_owner;
mod formula_pattern;
mod formula_pattern_ir;
mod formula_pool;
mod formula_program_check;
mod formula_project_ir;
mod formula_projection_ir;
mod formula_raise;
mod formula_range_ir;
mod formula_source;
mod formula_source_activity;
pub use formula_source::{AcyclicSource, SourceAdmission, SourceEvaluationStatistics};
mod formula_support;
mod formula_value;
mod formula_value_ir;
mod formula_warning;
mod formula_weak;
mod grounding_observer;
mod integer_range;
mod metadata;
pub mod objective_bound;
pub mod observation;
mod scalar_arithmetic;
pub mod symbols;

pub use parsed_source::{ParsedSource, SourceFailure};
pub use program_admission::{
    CompilationFailure, ProgramAdmissionFailure, ProgramAdmissionOptions, ProgramFailureKind,
    ProgramLimit, ProgramOriginal, ProgramResource,
};
/// Canonical shared frontend tiers, including all vocabulary exposed by this crate.
pub use themelios_analysis as analysis;
/// Canonical source identities, spans, diagnostics and source catalogs.
pub use themelios_base as base;
/// Canonical owned logical program, provenance, terms and symbols.
pub use themelios_program as logical;
/// Canonical lossless syntax and parsing vocabulary.
pub use themelios_syntax as syntax;

use themelios_base::source::SourceId;
use zetesis_core::AdmissionLimits;

pub use bundle::{
    BundleError, BundleIoOperation, BundleLimits, BundleResource, BundleRoot, BundleSource,
    IncludeEdge, IncludeResolution, SourceBundle,
};
pub use bundle_admission::{BundleAdmissionError, BundleAdmissionOptions};
pub use diagnostic::{AdmissionFailure, InputLimit, ProfileFeature, SyntaxFailure};
pub use expansion::{ExpansionFailure, ExpansionLimits, ExpansionResource, ExpansionUsage};
pub use formula::{
    AdmittedFormula, AdmittedFormulaBundle, AnalysisBasis, FormulaBundleFailure, FormulaFailure,
    FormulaLimits, FormulaPurpose, FormulaResource, PreparedFormula, PreparedFormulaBundle,
    ProgramFormulaOptions, admit_bundle_formula, admit_bundle_formula_with_grounding_observer,
    admit_formula, admit_formula_with_grounding_observer, admit_program_formula,
    prepare_bundle_formula, prepare_bundle_formula_with_cancellation, prepare_formula,
    prepare_program_formula, prepare_program_formula_with, validate_program_formula,
};
pub use formula_ir::ComparisonForms;
pub use formula_warning::FormulaWarning;
mod formula_hybrid;
mod formula_terminal;
pub use formula_terminal::{
    BaseKind, FormulaMaterialization, ReconstructionCharges, ReconstructionError,
    ReconstructionStatistics, TerminalBase, TerminalFormula, TerminalReconstruction,
};
mod constraint_allowance;
pub use constraint_allowance::ConstraintAllowance;
pub use formula_hybrid::{
    ConstraintCheckCause, ConstraintCheckFailure, ConstraintCheckLimits, ConstraintCheckStatistics,
    ConstraintChecker, ConstraintConsequence, ConstraintRegionPass, ConstraintRegionVerdict,
    ConstraintVerdict, HybridFeature, HybridFormula, StreamedCore,
};
mod formula_count_plan;
pub use formula_count_plan::{
    CountPlan, CountPlanFailure, CountPlanFailureKind, CountPlanLimits, CountPlanResource,
    CountPlanStatistics, CountPlanStatus, RequiredChoices,
};
mod formula_domains;
mod formula_fixed_constraints;
mod formula_keys;
mod formula_rewrite;
mod grounding_options;
#[cfg(test)]
mod test_support;
mod word_hash;
pub use formula_keys::KeyAnalysis;
pub use grounding_observer::{
    DomainObservation, GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork,
};
pub use grounding_options::{DomainLimits, DomainStop, GroundingOptions, JoinStrategy};
pub use metadata::{
    AtomSelection, AtomSelectionError, AtomSelectionLimits, Directives, LocatedDirective,
    MetadataError, MetadataFeature, MetadataLimits, MetadataResource, MetadataStorageError,
    MetadataStorageLimits, OutputSelection, PreparedProjection, PreparedSelection,
    ProjectSelection, Signatures, SourceDirective, SourceMetadata,
};

/// Explicit host admission ceilings. Zero means that no resource of that kind
/// may be consumed; limits never mean unlimited.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionOptions {
    /// Identity attached to every source location.
    pub source_id: SourceId,
    /// Maximum UTF-8 bytes accepted before parsing.
    pub max_source_bytes: usize,
    /// Maximum syntax nodes visited before raising.
    pub max_syntax_nodes: usize,
    /// Maximum syntax-node nesting, including every nested term.
    pub max_syntax_depth: usize,
    /// Maximum source body elements per rule, before set deduplication.
    pub max_body_elements: usize,
    /// Formula template, variable, predicate-arity and value ceilings.
    pub core_limits: AdmissionLimits,
}

impl Default for AdmissionOptions {
    fn default() -> Self {
        Self {
            source_id: SourceId::new(0),
            max_source_bytes: 1_048_576,
            max_syntax_nodes: 262_144,
            max_syntax_depth: 128,
            max_body_elements: 1_024,
            core_limits: AdmissionLimits::default(),
        }
    }
}

mod structural_value;

/// Borrowed numeric-domain evidence from prepared canonical producers.
pub use formula_domains::numeric_ir::{
    NumericAnalysis, NumericBody, NumericCensus, NumericComparison, NumericOperand,
    NumericRuleAnalysis,
};
