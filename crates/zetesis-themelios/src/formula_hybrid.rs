//! Complete program admission with retained producers and streamed constraints.
//!
//! A core answer is only a proposal for the original program. The constraint
//! checker establishes satisfaction, never reduct minimality. Every successful
//! check exhausts its required source instances; a violation may stop early.

mod consequences;
mod selection;
pub use consequences::{ConstraintConsequence, ConstraintRegionPass};

use crate::formula_support::{Context, GroundingWork};

use std::{
    fmt,
    sync::{Arc, OnceLock},
};
use themelios_base::source::Source;
use themelios_program::program::{DefaultNegation, Program};
use zetesis_core::{AtomCatalog, AtomIndexError, AtomLookup, CatalogIndex, Model};
use zetesis_cpu::{Cancellation, Stop, regions::Region};
use zetesis_ferraris::Theory;

use crate::expansion::Budget;
use crate::formula::Compiled;
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_owner::Owner;
use crate::formula_support::{
    Accounting, CompletedSupport, Counters, PositiveRows, PreparedRule, RowFilter,
};
use crate::{
    ConstraintAllowance, ExpansionLimits, FormulaFailure, FormulaLimits, ProgramSite, SourceBundle,
    SourceMetadata,
};
use selection::{ModelSelection, RulePredicates, Selection, SourceRows};

/// Capability outside the hybrid schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridFeature {
    /// Ordinary hybrid probes require the indexed strategy. Certified computed
    /// domains may still use the shared finite-table selector internally.
    TableJoins,
}
impl fmt::Display for HybridFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TableJoins => "table joins",
        })
    }
}

pub(crate) struct Constraints {
    pub(crate) support: crate::formula_support::StreamedSupport,
    pub(crate) rules: Vec<RuleIr>,
    pub(crate) instances: u64,
    pub(crate) limits: FormulaLimits,
    pub(crate) location: ProgramSite,
}

struct Core {
    compiled: Compiled,
    constraints: Option<Constraints>,
    source: Arc<Owner>,
    /// The typed index of `compiled.atoms`, set by the first checker whose
    /// candidate check needs it and lent to every later checker of this core.
    index: OnceLock<CatalogIndex>,
    /// The kept source rows' positions in `compiled.atoms`, set by the first
    /// checker whose candidate check needs them and lent likewise.
    rows: OnceLock<selection::RowPositions>,
    /// Immutable rule dependencies and original atom groups, shared by checkers.
    incremental: OnceLock<consequences::IncrementalPlan>,
}

/// A materialized producer core with the integrity constraints streamed over it:
/// its stable models are proposals, and only those satisfying the streamed
/// constraints are answer sets of the program this core was admitted for (a
/// hybrid owner's whole program, or a terminal owner's base). Checkers and
/// candidate-region filters run over it. Cloning shares all retained storage.
#[derive(Clone)]
pub struct StreamedCore(Arc<Core>);
impl fmt::Debug for StreamedCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamedCore")
            .field("core_theory", self.core_theory())
            .field("streamed_templates", &self.streamed_templates())
            .field("streamed_instances", &self.streamed_instances())
            .finish_non_exhaustive()
    }
}

struct Admitted {
    core: StreamedCore,
    metadata: SourceMetadata,
}

/// One immutable original program: a materialized producer core and prepared
/// integrity constraints over the same atom envelope. Admission closes the
/// completed support, keeping the canonical base and only the relations the
/// streamed constraints read; when no constraint is streamed, the support and
/// plan storage are released instead.
///
/// Cloning shares all retained source, atoms and indexes. The core's answer sets
/// are proposals; only those satisfying the streamed constraints are answer
/// sets of this owner. Objectives are scored only after this complete check.
#[derive(Clone)]
pub struct HybridFormula(Arc<Admitted>);
impl fmt::Debug for HybridFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridFormula")
            .field("core_theory", self.core_theory())
            .field("streamed_templates", &self.streamed_templates())
            .field("streamed_instances", &self.streamed_instances())
            .finish_non_exhaustive()
    }
}
impl HybridFormula {
    pub(crate) fn new(
        compiled: Compiled,
        constraints: Option<Constraints>,
        source: Owner,
        metadata: SourceMetadata,
    ) -> Self {
        // With no eligible constraint the support was released at admission,
        // not closed; the core alone remains.
        Self(Arc::new(Admitted {
            core: StreamedCore::new(compiled, constraints, Arc::new(source)),
            metadata,
        }))
    }

    /// The streamed core: the producer core and the constraints checked over it.
    #[must_use]
    pub fn core(&self) -> &StreamedCore {
        &self.0.core
    }

    /// Exact original admitted-owner identity; clones share it.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Retained producers, ineligible constraints, coherence and support guards.
    /// This is the core theory, not the complete original program.
    #[must_use]
    pub fn core_theory(&self) -> &Theory {
        &self.0.core.0.compiled.theory
    }

    /// Shared dense atom meanings, including streamed constraint occurrences.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
        &self.0.core.0.compiled.atoms
    }

    /// Original canonical program before normalization or analysis projection.
    #[must_use]
    pub fn original_program(&self) -> &Program {
        self.0.core.0.source.program()
    }

    /// Original single source, absent for a bundle or logical program input.
    #[must_use]
    pub fn source(&self) -> Option<&Source> {
        self.0.core.0.source.source()
    }

    /// Complete original include bundle, absent for single source or logical input.
    #[must_use]
    pub fn bundle(&self) -> Option<&SourceBundle> {
        self.0.core.0.source.source_bundle()
    }

    /// Original declarations and display policy.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.0.metadata
    }

    /// Complete original projection domain, independent of candidate truth.
    #[must_use]
    pub fn projection(&self) -> &crate::PreparedProjection {
        &self.0.core.0.compiled.projection
    }

    /// Original source-family warnings, completed before this owner is published.
    #[must_use]
    pub fn warnings(&self) -> &[crate::FormulaWarning] {
        &self.0.core.0.compiled.warnings
    }

    /// Render warnings against retained source bytes or an include bundle.
    /// Logical input instead names the original statement index when available.
    #[must_use]
    pub fn warning_view(&self) -> impl fmt::Display + '_ {
        self.0.core.0.source.warning_view(self.warnings())
    }

    /// Original objectives, scored only after complete streamed constraint
    /// acceptance has established an answer of the original program.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.0.core.0.compiled.objectives
    }

    /// Analysis of the original source projection, not a certificate for the core.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.0.core.0.compiled.analysis
    }

    /// Source program to which the retained analysis applies.
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        &self.0.core.0.compiled.analyzed
    }

    /// Whether the analyzed source is exact or a dependency projection.
    #[must_use]
    pub fn analysis_basis(&self) -> crate::AnalysisBasis {
        self.0.core.0.compiled.analysis_basis
    }

    /// Applicable key rewrites made during original source preparation.
    #[must_use]
    pub fn keyed_constraints(&self) -> usize {
        self.0.core.0.compiled.keyed_constraints
    }

    /// Completion of the bounded original key analysis.
    #[must_use]
    pub fn key_analysis(&self) -> crate::KeyAnalysis {
        self.0.core.0.compiled.key_analysis
    }

    /// Source preparation and admission charges, excluding later checks.
    #[must_use]
    pub fn expansion_usage(&self) -> &crate::ExpansionUsage {
        &self.0.core.0.compiled.expansion
    }

    /// Retained lowered constraint templates. Pool alternatives count separately.
    #[must_use]
    pub fn streamed_templates(&self) -> usize {
        self.0.core.streamed_templates()
    }

    /// Scalar-selected instances visited during admission without retaining DAGs.
    #[must_use]
    pub fn streamed_instances(&self) -> u64 {
        self.0.core.streamed_instances()
    }

    /// As [`StreamedCore::checker`].
    ///
    /// # Errors
    /// As [`StreamedCore::checker`].
    pub fn checker(
        &self,
        limits: ConstraintCheckLimits,
    ) -> Result<ConstraintChecker<'_>, ConstraintCheckFailure> {
        self.0.core.checker(limits)
    }

    /// As [`StreamedCore::checker_with_allowance`].
    ///
    /// # Errors
    /// As [`StreamedCore::checker_with_allowance`].
    pub fn checker_with_allowance(
        &self,
        allowance: &ConstraintAllowance,
        cancellation: &Cancellation,
    ) -> Result<ConstraintChecker<'_>, ConstraintCheckFailure> {
        self.0.core.checker_with_allowance(allowance, cancellation)
    }
}

/// One check's join-planning and scalar budget; shared scalar receipts are
/// reported to `allowance`. Planning uses the caller's work ceiling, not an
/// unrelated source-admission default.
fn check_budget(limits: ConstraintCheckLimits, allowance: Option<ConstraintAllowance>) -> Budget {
    let budget = Budget::new(
        ExpansionLimits {
            max_term_work: usize::try_from(limits.max_work).unwrap_or(usize::MAX),
            max_scalar_bytes: limits.max_scalar_bytes,
            ..ExpansionLimits::default()
        },
        0,
    );
    match allowance {
        Some(allowance) => budget.with_allowance(allowance),
        None => budget,
    }
}

impl StreamedCore {
    pub(crate) fn new(
        compiled: Compiled,
        constraints: Option<Constraints>,
        source: Arc<Owner>,
    ) -> Self {
        Self(Arc::new(Core {
            compiled,
            constraints,
            source,
            index: OnceLock::new(),
            rows: OnceLock::new(),
            incremental: OnceLock::new(),
        }))
    }

    pub(crate) fn compiled(&self) -> &Compiled {
        &self.0.compiled
    }

    /// Exact core identity; clones share it.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Retained producers, ineligible constraints, coherence and support guards.
    #[must_use]
    pub fn core_theory(&self) -> &Theory {
        &self.0.compiled.theory
    }

    /// Shared dense atom meanings, including streamed constraint occurrences.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
        &self.0.compiled.atoms
    }

    /// Objectives of the original admitted program. Score a core answer only
    /// after it satisfies every streamed original constraint.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.0.compiled.objectives
    }

    /// Analysis of the program this core was admitted for.
    #[must_use]
    pub fn analysis(&self) -> &themelios_analysis::Analysis {
        &self.0.compiled.analysis
    }

    /// Semantic status of that analysis input.
    #[must_use]
    pub fn analysis_basis(&self) -> crate::AnalysisBasis {
        self.0.compiled.analysis_basis
    }

    /// Written constraints over a keyed value asked as the one atom their key
    /// admits during preparation.
    #[must_use]
    pub fn keyed_constraints(&self) -> usize {
        self.0.compiled.keyed_constraints
    }

    /// How the key analysis that asked them ended.
    #[must_use]
    pub fn key_analysis(&self) -> crate::KeyAnalysis {
        self.0.compiled.key_analysis
    }

    /// Retained lowered constraint templates. Pool alternatives count separately.
    #[must_use]
    pub fn streamed_templates(&self) -> usize {
        self.0
            .constraints
            .as_ref()
            .map_or(0, |constraints| constraints.rules.len())
    }

    /// Scalar-selected instances visited during admission without retaining DAGs.
    #[must_use]
    pub fn streamed_instances(&self) -> u64 {
        self.0
            .constraints
            .as_ref()
            .map_or(0, |constraints| constraints.instances)
    }

    /// Prepare one independent cumulative checker over the completed support.
    /// Existing dictionaries and equality postings are borrowed, never rebuilt.
    /// Snapshot descriptors cost O(number of predicates); rows are not copied.
    ///
    /// # Errors
    /// Returns a typed preparation/resource refusal with accepted charges.
    /// Logical source failures retain the original canonical program.
    pub fn checker(
        &self,
        limits: ConstraintCheckLimits,
    ) -> Result<ConstraintChecker<'_>, ConstraintCheckFailure> {
        self.prepare_checker(limits, Counters::default(), None)
    }

    /// Prepare an independent checker whose charges share `allowance` with all
    /// other attached checkers, including final full-model checking. Cancellation
    /// is polled during charged setup work and before publishing the checker.
    /// Accepted setup charges are published before return, including on failure.
    /// Checks publish their charges when they return, fail or unwind; live
    /// shared statistics can lag the operation's exact local receipt.
    ///
    /// # Errors
    /// Returns a typed preparation/resource/control refusal with its local
    /// accepted receipt. `allowance.statistics()` retains the combined receipt.
    pub fn checker_with_allowance(
        &self,
        allowance: &ConstraintAllowance,
        cancellation: &Cancellation,
    ) -> Result<ConstraintChecker<'_>, ConstraintCheckFailure> {
        cancellation.poll().map_err(|stop| ConstraintCheckFailure {
            cause: ConstraintCheckCause::Stopped(stop),
            statistics: ConstraintCheckStatistics::default(),
        })?;
        let limits = allowance.limits();
        let checker = self.prepare_checker(
            limits,
            Counters::with_allowance(allowance.clone(), cancellation),
            Some(allowance.clone()),
        )?;
        cancellation.poll().map_err(|stop| ConstraintCheckFailure {
            cause: ConstraintCheckCause::Stopped(stop),
            statistics: checker.statistics(),
        })?;
        Ok(checker)
    }

    fn prepare_checker(
        &self,
        limits: ConstraintCheckLimits,
        mut counters: Counters,
        allowance: Option<ConstraintAllowance>,
    ) -> Result<ConstraintChecker<'_>, ConstraintCheckFailure> {
        let prepared = if let Some(constraints) = &self.0.constraints {
            let mut formula_limits = constraints.limits;
            formula_limits.max_work = limits.max_work;
            formula_limits.max_substitutions = limits.max_substitutions;
            let completed = constraints
                .support
                .snapshot(&formula_limits, &mut counters, constraints.location)
                .map_err(|error| ConstraintCheckFailure {
                    cause: ConstraintCheckCause::Source(Box::new(
                        self.0.source.retain_failure(error),
                    )),
                    statistics: ConstraintCheckStatistics {
                        work: counters.accounting.work,
                        substitutions: counters.accounting.substitutions,
                        scalar_bytes: 0,
                    },
                })?;
            Some(PreparedConstraints {
                source: constraints,
                completed,
                limits: formula_limits,
                index: None,
                rows: None,
                plans: Vec::new(),
                predicates: Vec::new(),
                shared_incremental: &self.0.incremental,
                incremental_plan: None,
                incremental: None,
                incremental_eligible: None,
            })
        } else {
            None
        };
        Ok(ConstraintChecker {
            owner: self,
            prepared,
            budget: check_budget(limits, allowance.clone()),
            limits,
            allowance,
            settled: (0, 0),
            settled_scalar_bytes: 0,
            consequence_active: false,
            accounting: counters.into_accounting(),
        })
    }
}

/// Allowances for each check of a checker (one model, region-refutation scan,
/// or complete candidate closure including all of its consequence passes),
/// independent of source admission and reduct-oracle work. A checker's first
/// check also covers its preparation; each later check is measured from the
/// charges accepted when the previous one ended. They bound the work spent on
/// any one candidate, never the number of candidates. Zero is a zero
/// allowance, never unlimited. Per-operation binding/storage bounds remain
/// those admitted with the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstraintCheckLimits {
    /// Charged join, scalar and typed atom-lookup work in one check.
    /// The same allowance independently bounds source join-planning term work.
    pub max_work: u64,
    /// Complete substitutions visited in one check, including false filters.
    pub max_substitutions: u64,
    /// Bytes requested in one check for capture-delta cells during structural-pattern
    /// matching, charged through source expansion's `ScalarBytes` resource.
    /// The historical field name is retained; these cells borrow canonical
    /// terms. ID-only binding copies and frozen constructor lookups add no
    /// scalar-byte charge. Their storage uses the admitted support allowance.
    pub max_scalar_bytes: usize,
}
impl ConstraintCheckLimits {
    /// Finite defaults matching ordinary source work and expansion allowances.
    pub const DEFAULT: Self = Self {
        max_work: 10_000_000,
        max_substitutions: 1_000_000,
        max_scalar_bytes: 16_777_216,
    };
}
impl Default for ConstraintCheckLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Accepted cumulative charges; failed requested charges are in the typed cause.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConstraintCheckStatistics {
    /// Charged work, including snapshot preparation.
    pub work: u64,
    /// Complete substitutions visited.
    pub substitutions: u64,
    /// Accepted cumulative capture-delta reservations under `max_scalar_bytes`.
    /// Canonical payload is borrowed; this is neither live capacity nor RSS.
    pub scalar_bytes: usize,
}

/// Satisfaction of the streamed part only, never answer-set membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintVerdict {
    /// Every required admitted constraint instance was checked without violation.
    Satisfied,
    /// A complete admitted constraint body is true in this interpretation.
    Violated {
        /// Original enclosing rule and any actual source coordinate.
        site: ProgramSite,
    },
}

/// Whether one streamed body is true throughout an original candidate region.
/// This operation establishes neither satisfaction nor reduct membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintRegionVerdict {
    /// No single admitted body was found true throughout the region. Some or
    /// all remaining candidates may still violate the constraints.
    NotRefuted,
    /// One admitted body is true in every interpretation between the bounds.
    Refuted {
        /// Original enclosing rule and any actual source coordinate.
        site: ProgramSite,
    },
}

/// A check stopped without establishing satisfaction or a violation.
#[derive(Debug)]
pub enum ConstraintCheckCause {
    /// The model catalog or supplied region theory has a different owner.
    WrongProgram,
    /// A continuation was requested without an unfinished consequence closure.
    NoActiveRegion,
    /// Region coordinates do not span the authenticated original atom catalog.
    WrongRegionSize {
        /// Original catalog dimension.
        expected: usize,
        /// Supplied region dimension.
        actual: usize,
    },
    /// Preparation of the bounded catalog lookup refused allocation or found
    /// duplicate typed atoms. Source-work stops use `Source` instead.
    Index(AtomIndexError<Box<FormulaFailure>>),
    /// Caller cancellation or deadline, never semantic rejection.
    Stopped(Stop),
    /// Located join/evaluation/allocation/resource refusal.
    Source(Box<FormulaFailure>),
}
impl ConstraintCheckCause {
    /// Report a work or substitution refusal against the check's own
    /// allowance: subtract the charges `start` accepted before the check.
    fn relative_to(self, start: (u64, u64)) -> Self {
        let relative = |error: Box<FormulaFailure>| match *error {
            FormulaFailure::Limit {
                resource:
                    resource @ (crate::FormulaResource::Work | crate::FormulaResource::Substitutions),
                limit,
                observed,
                location,
            } => {
                let base = u128::from(if resource == crate::FormulaResource::Work {
                    start.0
                } else {
                    start.1
                });
                Box::new(FormulaFailure::Limit {
                    resource,
                    limit: limit.saturating_sub(base),
                    observed: observed.saturating_sub(base),
                    location,
                })
            }
            other => Box::new(other),
        };
        match self {
            Self::Source(error) => Self::Source(relative(error)),
            Self::Index(AtomIndexError::Stopped(error)) => {
                Self::Index(AtomIndexError::Stopped(relative(error)))
            }
            other => other,
        }
    }

    fn retain_input(self, owner: &Owner) -> Self {
        match self {
            Self::Source(error) => Self::Source(Box::new(owner.retain_failure(*error))),
            Self::Index(AtomIndexError::Stopped(error)) => Self::Index(AtomIndexError::Stopped(
                Box::new(owner.retain_failure(*error)),
            )),
            other => other,
        }
    }
}
impl fmt::Display for ConstraintCheckCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProgram => f.write_str("constraint candidate belongs to another program"),
            Self::NoActiveRegion => {
                f.write_str("constraint consequence has no active region closure")
            }
            Self::WrongRegionSize { expected, actual } => {
                write!(
                    f,
                    "constraint region has {actual} atoms; expected {expected}"
                )
            }
            Self::Index(error) => error.fmt(f),
            Self::Stopped(stop) => stop.fmt(f),
            Self::Source(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ConstraintCheckCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WrongProgram | Self::NoActiveRegion | Self::WrongRegionSize { .. } => None,
            Self::Index(error) => Some(error),
            Self::Stopped(error) => Some(error),
            Self::Source(error) => Some(error.as_ref()),
        }
    }
}

/// Typed refusal together with all charges accepted before it.
#[derive(Debug)]
pub struct ConstraintCheckFailure {
    /// Exact cause; the original non-clone allocation error is preserved.
    pub cause: ConstraintCheckCause,
    /// Cumulative checker charges, including earlier complete checks.
    pub statistics: ConstraintCheckStatistics,
}
impl ConstraintCheckFailure {
    /// Cancellation/deadline classification without discarding the source error.
    #[must_use]
    pub fn stop(&self) -> Option<Stop> {
        match &self.cause {
            ConstraintCheckCause::Stopped(reason) => Some(*reason),
            ConstraintCheckCause::Source(error)
            | ConstraintCheckCause::Index(AtomIndexError::Stopped(error)) => error.interruption(),
            ConstraintCheckCause::WrongProgram
            | ConstraintCheckCause::NoActiveRegion
            | ConstraintCheckCause::WrongRegionSize { .. }
            | ConstraintCheckCause::Index(_) => None,
        }
    }
}
impl fmt::Display for ConstraintCheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for ConstraintCheckFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Mutable per-session work and scalar state over an immutable admitted owner.
/// No grounded constraint instances are retained. Pending consequences belong
/// to one authenticated closure. Successfully completed no-consequence scans
/// can survive into another closure when its decision masks authenticate their
/// unchanged dependencies. Ordinary checks invalidate candidate evidence.
/// The checker can move between threads. Grounding observation and each check's
/// runtime control remain local to the synchronous operation that uses them.
pub struct ConstraintChecker<'a> {
    owner: &'a StreamedCore,
    prepared: Option<PreparedConstraints<'a>>,
    /// The current check's scalar budget, shared by every consequence pass.
    budget: Budget,
    /// The ceilings each check gets for itself.
    limits: ConstraintCheckLimits,
    /// The shared receipt this checker reports to, if any.
    allowance: Option<ConstraintAllowance>,
    /// Work and substitutions accepted when the previous check ended: the
    /// next check's allowance is measured from here, so the first check's
    /// includes this checker's preparation.
    settled: (u64, u64),
    /// Scalar bytes requested by every finished check's budget.
    settled_scalar_bytes: usize,
    /// An explicit First pass opened one candidate closure. Its counters and
    /// scalar budget survive Hold/Cut results and unwind until that closure ends.
    consequence_active: bool,
    accounting: Accounting,
}
/// A nonempty source and its borrowed snapshot are present or absent together.
struct PreparedConstraints<'a> {
    source: &'a Constraints,
    completed: CompletedSupport<'a>,
    limits: FormulaLimits,
    /// Original dense IDs, borrowed from the core's index on first candidate use
    /// (and built there by the first checker that needs it). Final-model row
    /// selection and complete bodies query the model's canonical true set.
    index: Option<&'a CatalogIndex>,
    /// Source occurrence IDs mapped once into the original dense catalog.
    rows: Option<SourceRows<'a>>,
    /// Lazily prepared after each rule's first successful predicate gate.
    plans: Vec<Option<PreparedRule<'a>>>,
    /// Occurrence-local predicate windows, borrowed from external source/index
    /// owners and prepared before the rule's first region predicate gate.
    predicates: Vec<Option<RulePredicates<'a>>>,
    /// This exact core alone publishes the immutable mapping. Admitting a
    /// borrow once also survives discarded mutable state after an unwind.
    shared_incremental: &'a OnceLock<consequences::IncrementalPlan>,
    incremental_plan: Option<&'a consequences::IncrementalPlan>,
    incremental: Option<consequences::Incremental<'a>>,
    incremental_eligible: Option<bool>,
}

impl<'a> PreparedConstraints<'a> {
    fn selection<'check>(
        &'check self,
        rule_index: usize,
        region: &'check Region,
    ) -> Selection<'check, 'a> {
        Selection {
            rows: self.rows.as_ref().expect("prepared before region scan"),
            index: self.index.expect("prepared before region scan").lookup(),
            predicates: self.predicates.get(rule_index).and_then(Option::as_ref),
            region,
        }
    }

    /// Resolve every atom occurrence of this rule before its first region gate.
    /// This performs no binding or scalar evaluation. It may charge/refuse
    /// preparation for later occurrences before an earlier predicate would have
    /// rejected the rule; successful views retain no candidate decisions.
    fn prepare_predicates(
        &mut self,
        rule_index: usize,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        if self.predicates.is_empty() {
            self.predicates = RulePredicates::slots(
                &self.source.rules,
                &mut self.completed,
                &self.limits,
                counters,
            )?;
        }
        if self.predicates[rule_index].is_none() {
            self.predicates[rule_index] = Some(RulePredicates::new(
                &self.source.rules[rule_index],
                self.index.expect("prepared before region scan").lookup(),
                &mut self.completed,
                &self.limits,
                counters,
            )?);
        }
        Ok(())
    }

    fn prepare_rule(
        &mut self,
        rule_index: usize,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        if self.plans.is_empty() {
            self.plans = PreparedRule::slots(
                &self.source.rules,
                &mut self.completed,
                &self.limits,
                counters,
            )?;
        }
        if self.plans[rule_index].is_none() {
            self.plans[rule_index] = Some(PreparedRule::new(
                &self.source.rules[rule_index],
                &mut self.completed,
                &self.limits,
                budget,
                counters,
            )?);
        }
        Ok(())
    }

    fn prepare_selection(
        &mut self,
        core: &'a StreamedCore,
        counters: &mut Counters,
    ) -> Result<(), ConstraintCheckCause> {
        self.prepare_index(core, counters)?;
        if self.rows.is_none() {
            let positions = self
                .prepare_positions(core, counters)
                .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
            let rows = SourceRows::attach(
                &self.completed,
                positions,
                &self.limits,
                counters,
                self.source.location,
            )
            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
            // Every reservation checked its actual capacity before any publication.
            self.completed.retain_workspace(
                usize::try_from(rows.retained_bytes()).expect("admitted support bytes fit usize"),
            );
            self.rows = Some(rows);
        }
        Ok(())
    }

    /// Borrow the core's row positions, building them first when no checker
    /// has: one charged index probe per kept support row, against this
    /// checker's work and support ceilings. Races, refusals and the ledger
    /// follow [`Self::prepare_index`].
    fn prepare_positions(
        &mut self,
        core: &'a StreamedCore,
        counters: &mut Counters,
    ) -> Result<&'a selection::RowPositions, FormulaFailure> {
        let location = self.source.location;
        counters.work(&self.limits, location)?;
        if core.0.rows.get().is_none() {
            let built = selection::RowPositions::prepare(
                &self.completed,
                self.index.expect("prepared before its rows").lookup(),
                &self.limits,
                counters,
                location,
            )?;
            // A racing checker may have published first: keep its positions.
            let _ = core.0.rows.set(built);
        }
        let positions = core
            .0
            .rows
            .get()
            .expect("published above or by a racing checker");
        let retained = positions.retained_bytes();
        self.completed
            .admit_workspace(retained, &self.limits, counters, location)?;
        self.completed
            .retain_workspace(usize::try_from(retained).expect("admitted support bytes fit usize"));
        Ok(positions)
    }

    /// Borrow the core's index, building it first when no checker has. A
    /// borrow costs one unit of work; a build costs O(n log n) comparisons
    /// over the core's n atoms, charged to this checker, which also admits
    /// its preparation peak. Checkers racing on the first use each build and
    /// one publishes; none waits on another, so each observes its own
    /// cancellation. A refused build publishes nothing. Either way the
    /// checker's support ledger counts the index's retained bytes, as it
    /// counts the shared support base it reads.
    fn prepare_index(
        &mut self,
        core: &'a StreamedCore,
        counters: &mut Counters,
    ) -> Result<(), ConstraintCheckCause> {
        if self.index.is_some() {
            return Ok(());
        }
        let location = self.source.location;
        counters
            .work(&self.limits, location)
            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
        if core.0.index.get().is_none() {
            let built = self.build_index(&core.0.compiled.atoms, counters)?;
            // A racing checker may have published first: keep its index.
            let _ = core.0.index.set(built);
        }
        let index = core
            .0
            .index
            .get()
            .expect("published above or by a racing checker");
        let retained = index.retained_bytes();
        self.completed
            .admit_workspace(retained, &self.limits, counters, location)
            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
        self.completed
            .retain_workspace(usize::try_from(retained).expect("admitted support bytes fit usize"));
        self.index = Some(index);
        Ok(())
    }

    fn build_index(
        &mut self,
        atoms: &AtomCatalog,
        counters: &mut Counters,
    ) -> Result<CatalogIndex, ConstraintCheckCause> {
        let location = self.source.location;
        // Two retained integer orders and one preparation scratch order, all
        // bounded by the admitted atom count. The index reserves fallibly and
        // charges each comparison/write; it never copies atom payloads.
        let requested = size_of::<CatalogIndex>() as u128
            + size_of::<Vec<usize>>() as u128
            + 3 * atoms.atoms().len() as u128 * size_of::<usize>() as u128;
        self.completed
            .admit_workspace(requested, &self.limits, counters, location)
            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
        let index = CatalogIndex::new_with(atoms, || {
            counters.work(&self.limits, location).map_err(Box::new)
        })
        .map_err(|error| match error {
            AtomIndexError::Stopped(error) => ConstraintCheckCause::Source(error),
            other => ConstraintCheckCause::Index(other),
        })?;
        self.completed
            .admit_workspace(
                index.preparation_peak_bytes(),
                &self.limits,
                counters,
                location,
            )
            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
        Ok(index)
    }
}

#[derive(Clone, Copy)]
enum Candidate<'a> {
    Model(&'a Model),
    Region(&'a Theory, &'a Region),
}

impl ConstraintChecker<'_> {
    /// Current cumulative resource receipt, including failed-check prefixes.
    #[must_use]
    pub fn statistics(&self) -> ConstraintCheckStatistics {
        ConstraintCheckStatistics {
            work: self.accounting.work,
            substitutions: self.accounting.substitutions,
            scalar_bytes: self
                .settled_scalar_bytes
                .saturating_add(self.budget.usage().scalar_bytes),
        }
    }

    /// Check original constraint satisfaction against an exact-catalog model.
    /// Joins borrow completed possible support. After exact source-row mapping,
    /// positive rows absent from the supplied model are excluded before matching.
    /// Unmapped rows retain the ordinary path. Every completed body separately
    /// queries that same model; row selection lends no positive truth proof.
    /// Shared support is never treated as candidate truth. A violation returns
    /// early; successful satisfaction exhausts every necessary family.
    /// Runtime control is polled at charged source work boundaries and before
    /// publishing either verdict. Source arithmetic admission has already run.
    ///
    /// # Errors
    /// Wrong catalog, cancellation/deadline or a checked resource/evaluation
    /// refusal. A failed scan certifies neither satisfaction nor exhaustion.
    pub fn check(
        &mut self,
        model: &Model,
        cancellation: &Cancellation,
    ) -> Result<ConstraintVerdict, ConstraintCheckFailure> {
        self.examine(Candidate::Model(model), cancellation)
            .map(|location| match location {
                Some(site) => ConstraintVerdict::Violated { site },
                None => ConstraintVerdict::Satisfied,
            })
    }

    /// Refute an original candidate region only when one admitted constraint
    /// body is true throughout it. Positive and double-negated atoms must be
    /// held; default-negated atoms must be cut. Open atoms establish neither.
    ///
    /// The supplied theory must be this owner's exact core, and the region must
    /// span its dense catalog. These checks authenticate the coordinate convention,
    /// not the provenance of a raw `Region`. No region or candidate is mutated.
    /// Never use this operation to read a candidate's frozen reduct.
    ///
    /// First use borrows the core's typed catalog index and source-row
    /// positions (building each when no checker of this core has) and pairs
    /// the positions with this checker's occurrence maps; later checks reuse
    /// both. Necessary predicate and held-row selections
    /// precede binding/scalar evaluation, over the same completed support and
    /// shared evaluator as `check`. Missing row IDs remain eligible until the
    /// full body check. Source arithmetic admission has already completed.
    /// Failure preserves every accepted charge; `NotRefuted` does not prove
    /// satisfaction. This does not construct a filtered support certificate.
    ///
    /// # Errors
    /// Wrong theory/dimension, cancellation, or a checked index/source refusal.
    pub fn check_region(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<ConstraintRegionVerdict, ConstraintCheckFailure> {
        self.examine(Candidate::Region(theory, region), cancellation)
            .map(|location| match location {
                Some(site) => ConstraintRegionVerdict::Refuted { site },
                None => ConstraintRegionVerdict::NotRefuted,
            })
    }

    fn authenticate(&self, candidate: Candidate<'_>) -> Result<(), ConstraintCheckCause> {
        match candidate {
            Candidate::Model(model) => {
                if !model.catalog().same_owner(self.owner.atom_catalog()) {
                    return Err(ConstraintCheckCause::WrongProgram);
                }
            }
            Candidate::Region(theory, region) => {
                if !theory.same_instance(self.owner.core_theory()) {
                    return Err(ConstraintCheckCause::WrongProgram);
                }
                let expected = self.owner.atom_catalog().atoms().len();
                if region.len() != expected {
                    return Err(ConstraintCheckCause::WrongRegionSize {
                        expected,
                        actual: region.len(),
                    });
                }
            }
        }
        Ok(())
    }

    fn examine(
        &mut self,
        candidate: Candidate<'_>,
        cancellation: &Cancellation,
    ) -> Result<Option<ProgramSite>, ConstraintCheckFailure> {
        self.invalidate_consequences();
        if self.consequence_active {
            self.settle_check();
        }
        let start = self.prepare_pass(cancellation);
        let result = self.authenticate(candidate).and_then(|()| {
            cancellation.poll().map_err(ConstraintCheckCause::Stopped)?;
            let verdict = self
                .accounting
                .with_cancellation(cancellation, |counters| {
                    if let Some(prepared) = &mut self.prepared {
                        prepared.prepare_selection(self.owner, counters)?;
                    }
                    self.budget
                        .with_settled_receipt(|budget| {
                            Self::scan(self.prepared.as_mut(), budget, counters, candidate)
                        })
                        .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))
                })?;
            cancellation.poll().map_err(ConstraintCheckCause::Stopped)?;
            Ok(verdict)
        });
        let statistics = self.statistics();
        self.finish_check();
        result.map_err(|cause| ConstraintCheckFailure {
            cause: cause.relative_to(start).retain_input(&self.owner.0.source),
            statistics,
        })
    }

    /// Publication at each pass is separate from renewal at the closure boundary.
    fn prepare_pass(&mut self, cancellation: &Cancellation) -> (u64, u64) {
        let start = self.settled;
        self.budget.set_cancellation(Some(cancellation.clone()));
        if let Some(prepared) = &mut self.prepared {
            prepared.limits.max_work = start.0.saturating_add(self.limits.max_work);
            prepared.limits.max_substitutions =
                start.1.saturating_add(self.limits.max_substitutions);
        }
        start
    }

    fn invalidate_consequences(&mut self) {
        if let Some(prepared) = &mut self.prepared
            && let Some(incremental) = &mut prepared.incremental
        {
            incremental.reset();
        }
    }

    fn finish_check(&mut self) {
        self.invalidate_consequences();
        self.settle_check();
    }

    /// Renew one closure's allowance independently of retained semantic evidence.
    fn settle_check(&mut self) {
        self.settled = (self.accounting.work, self.accounting.substitutions);
        self.settled_scalar_bytes = self.statistics().scalar_bytes;
        self.budget = check_budget(self.limits, self.allowance.clone());
        self.consequence_active = false;
    }

    fn scan(
        prepared: Option<&mut PreparedConstraints<'_>>,
        budget: &mut Budget,
        counters: &mut Counters,
        candidate: Candidate<'_>,
    ) -> Result<Option<ProgramSite>, FormulaFailure> {
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        let constraints = prepared.source;
        for (rule_index, rule) in constraints.rules.iter().enumerate() {
            if matches!(candidate, Candidate::Region(..)) {
                prepared.prepare_predicates(rule_index, counters)?;
            }
            if let Candidate::Region(_, region) = candidate
                && !prepared.selection(rule_index, region).possible(
                    rule,
                    &prepared.completed,
                    &prepared.limits,
                    counters,
                )?
            {
                continue;
            }
            prepared.prepare_rule(rule_index, budget, counters)?;
            let model_selection;
            let region_selection;
            let filter: &dyn RowFilter = match candidate {
                Candidate::Model(model) => {
                    model_selection = ModelSelection {
                        rows: prepared.rows.as_ref().expect("prepared before model scan"),
                        model,
                    };
                    &model_selection
                }
                Candidate::Region(_, region) => {
                    region_selection = prepared.selection(rule_index, region);
                    &region_selection
                }
            };
            let queries = prepared.completed.queries(
                crate::JoinStrategy::Indexed,
                &prepared.limits,
                counters,
                rule.location,
            )?;
            let mut computation = queries.computation(rule.location)?;
            let mut join = prepared.plans[rule_index]
                .as_ref()
                .expect("prepared above")
                .rows(
                    &queries,
                    Some(filter),
                    &computation,
                    &prepared.limits,
                    budget,
                    counters,
                )?;
            while let Some(row) = join.next_row(
                &mut computation,
                &prepared.limits,
                budget,
                counters,
                rule.location,
            )? {
                if row.passes
                    && body(
                        &rule.body,
                        &row.values,
                        row.positives.as_ref(),
                        candidate,
                        prepared.index.map(CatalogIndex::lookup),
                        prepared.predicates.get(rule_index).and_then(Option::as_ref),
                        Context::new(&computation, &prepared.limits, counters, rule.location),
                    )?
                {
                    return Ok(Some(rule.location));
                }
            }
        }
        Ok(None)
    }
}

pub(crate) fn eligible(rule: &RuleIr) -> bool {
    matches!(rule.head, HeadIr::Normal(None))
        && rule.body.iter().all(|literal| {
            !matches!(
                literal,
                LiteralIr::ProjectedAtom(..) | LiteralIr::Conditional(_) | LiteralIr::Aggregate(_)
            )
        })
}

/// `scan` pairs a row loan with the same immutable region borrowed by its
/// filter, and with that row's binding. Body identity authenticates the literal
/// occurrences; it does not establish region identity independently. Models
/// always query their own interpretation, even if a loan is supplied.
fn body(
    literals: &[LiteralIr],
    binding: &Binding<'_>,
    positives: Option<&PositiveRows<'_>>,
    candidate: Candidate<'_>,
    index: Option<AtomLookup<'_, '_>>,
    predicates: Option<&RulePredicates<'_>>,
    context: Context<'_, &crate::formula_support::Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    // Positive source evidence is local to this exact body and immutable
    // region. A final model always authenticates its own selected atoms.
    let positives = matches!(candidate, Candidate::Region(..))
        && positives.is_some_and(|proof| proof.covers(literals));
    let mut view = None;
    for (occurrence, literal) in literals.iter().enumerate() {
        let Some((negation, pattern)) = literal_atom(literal) else {
            continue;
        };
        counters.work(limits, location)?;
        if positives && negation == DefaultNegation::None {
            continue;
        }
        let binding_view = if let Some(view) = view {
            view
        } else {
            let checked = binding.view(computation.read(), limits, counters, location)?;
            view = Some(checked);
            checked
        };
        let key = computation
            .static_pattern(*pattern, limits, counters, location)?
            .key(binding_view)
            .map_err(|error| FormulaFailure::UnsafeVariable {
                variable: error.variable,
                location,
            })?;
        let truth = match candidate {
            Candidate::Model(model) => {
                let present = model
                    .lookup()
                    .get_key_with(&key, || counters.work(limits, location))?
                    .is_some();
                match negation {
                    DefaultNegation::Not => !present,
                    DefaultNegation::None | DefaultNegation::NotNot => present,
                }
            }
            Candidate::Region(_, region) => {
                let row = if let Some(prepared) =
                    predicates.and_then(|prepared| prepared.at(literals, occurrence))
                {
                    prepared.get_key_with(&key, || counters.work(limits, location))?
                } else {
                    index
                        .expect("prepared before region scan")
                        .get_key_with(&key, || counters.work(limits, location))?
                };
                // Passing source rows contributed every occurrence to the
                // completed catalog at admission, including unsupported atoms.
                let position = row
                    .ok_or(FormulaFailure::SupportRelation {
                        error: zetesis_core::relation::Failure::Owner,
                        location,
                    })?
                    .position();
                match negation {
                    DefaultNegation::Not => region.is_cut(position),
                    DefaultNegation::None | DefaultNegation::NotNot => region.is_held(position),
                }
            }
        };
        if !truth {
            return Ok(false);
        }
    }
    Ok(true)
}

fn literal_atom(
    literal: &LiteralIr,
) -> Option<(
    DefaultNegation,
    &crate::formula_support::components::Pattern,
)> {
    match literal {
        LiteralIr::Atom(negation, pattern) => Some((*negation, pattern)),
        LiteralIr::PatternAtom(pattern) => Some((DefaultNegation::None, &pattern.atom)),
        // The join owns scalar comparisons and generated bindings.
        LiteralIr::Compare(..)
        | LiteralIr::TupleCompare(..)
        | LiteralIr::ArgumentCheck { .. }
        | LiteralIr::Guard(_)
        | LiteralIr::HeadGuard(_)
        | LiteralIr::Bind { .. }
        | LiteralIr::Range { .. } => None,
        LiteralIr::ProjectedAtom(..) | LiteralIr::Conditional(_) | LiteralIr::Aggregate(_) => {
            unreachable!("only admitted ordinary constraints are streamed")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn binding_aliases_retain_distinct_positive_occurrences() {
        let owner = crate::prepare_formula(
            "{p(1)}. :-p(X),p(Y),X=Y.".into(),
            crate::AdmissionOptions::default(),
            crate::ExpansionLimits::default(),
            crate::FormulaLimits::default(),
        )
        .unwrap()
        .ground_hybrid()
        .unwrap();
        let mut checker = owner
            .checker(crate::ConstraintCheckLimits::default())
            .unwrap();
        let prepared = checker.prepared.as_ref().unwrap();
        assert_eq!(prepared.source.rules.len(), 1);
        assert_eq!(
            prepared.source.rules[0]
                .body
                .iter()
                .filter(|literal| {
                    matches!(
                        super::literal_atom(literal),
                        Some((themelios_program::program::DefaultNegation::None, _))
                    )
                })
                .count(),
            2,
            "the source did not coalesce binding-dependent aliases"
        );
        assert_eq!(
            checker
                .consequence_region(
                    owner.core_theory(),
                    &zetesis_cpu::regions::Region::all_open(1),
                    &zetesis_cpu::Cancellation::default(),
                    crate::ConstraintRegionPass::First,
                )
                .unwrap(),
            crate::ConstraintConsequence::NoConsequence
        );
    }

    #[test]
    fn impossible_predicates_prepare_no_join_plans() {
        let owner = crate::prepare_formula(
            "{p(1);p(2)}. :-p(X),X=2.".into(),
            crate::AdmissionOptions::default(),
            crate::ExpansionLimits::default(),
            crate::FormulaLimits::default(),
        )
        .unwrap()
        .ground_hybrid()
        .unwrap();
        let mut checker = owner
            .checker(crate::ConstraintCheckLimits::default())
            .unwrap();
        let region = zetesis_cpu::regions::Region::all_open(owner.atom_catalog().atoms().len());
        for _ in 0..2 {
            assert_eq!(
                checker
                    .check_region(
                        owner.core_theory(),
                        &region,
                        &zetesis_cpu::Cancellation::default()
                    )
                    .unwrap(),
                crate::ConstraintRegionVerdict::NotRefuted
            );
            assert!(checker.prepared.as_ref().unwrap().plans.is_empty());
        }
    }

    #[test]
    fn empty_stream_releases_completed_support() {
        for source in ["d(1..20).", "d(1..20). :-#count{X:d(X)}<1."] {
            let owner = crate::prepare_formula(
                source.into(),
                crate::AdmissionOptions::default(),
                crate::ExpansionLimits::default(),
                crate::FormulaLimits::default(),
            )
            .unwrap()
            .ground_hybrid()
            .unwrap();
            // The complete source has nonempty support. Absence proves that
            // neither its catalog nor an empty plan's spare capacity is retained.
            assert!(!owner.atom_catalog().atoms().is_empty());
            assert!(owner.0.core.0.constraints.is_none());
        }
    }
}

#[cfg(test)]
mod planning_tests;
