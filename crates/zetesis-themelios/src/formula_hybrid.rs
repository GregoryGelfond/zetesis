//! Complete source admission with retained producers and streamed constraints.
//!
//! A core answer is only a proposal for the original program. The constraint
//! checker establishes satisfaction, never reduct minimality. Every successful
//! check exhausts its required source instances; a violation may stop early.

mod selection;

use std::{fmt, sync::Arc};
use themelios_base::{source::Source, span::Location};
use themelios_program::program::{DefaultNegation, Program};
use zetesis_core::{AtomCatalog, AtomIndex, AtomIndexError, AtomPattern, Model};
use zetesis_cpu::{Cancellation, Stop, regions::Region};
use zetesis_ferraris::Theory;

use crate::expansion::Budget;
use crate::formula::Compiled;
use crate::formula_binding::Binding;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::{
    Accounting, CompletedCatalog, CompletedSupport, Counters, PreparedRule, RowFilter,
};
use crate::{
    ConstraintAllowance, ExpansionLimits, FormulaFailure, FormulaLimits, SourceBundle,
    SourceMetadata,
};
use selection::{Selection, SourceRows};

/// Capability deliberately outside the first hybrid schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridFeature {
    /// Objective programs require separate acceptance/scoring composition.
    Objectives,
    /// Hybrid checks currently reuse the completed indexed relation queries.
    TableJoins,
}
impl fmt::Display for HybridFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Objectives => "objectives",
            Self::TableJoins => "table joins",
        })
    }
}

pub(crate) enum SourceOwner {
    Single(Source),
    Bundle(SourceBundle),
}

pub(crate) struct Constraints {
    pub(crate) catalog: CompletedCatalog,
    pub(crate) rules: Vec<RuleIr>,
    pub(crate) instances: u64,
    pub(crate) limits: FormulaLimits,
    pub(crate) location: Location,
}

struct Admitted {
    compiled: Compiled,
    constraints: Option<Constraints>,
    source: SourceOwner,
    metadata: SourceMetadata,
}

/// One immutable original program: a materialized producer core and prepared
/// integrity constraints over the same complete atom/support envelope. When no
/// constraints are streamed, the unused completed support and plan storage are
/// released after admission.
///
/// Cloning shares all retained source, atoms and indexes. The core's answer sets
/// are proposals; only those satisfying the streamed constraints are answer
/// sets of this owner. No objectives are admitted by this initial schedule.
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
        constraints: Constraints,
        source: SourceOwner,
        metadata: SourceMetadata,
    ) -> Self {
        // An all-eager fallback needs neither support nor the filtered vector's
        // former capacity. Absence drops both together, after full admission.
        let constraints = (!constraints.rules.is_empty()).then_some(constraints);
        Self(Arc::new(Admitted {
            compiled,
            constraints,
            source,
            metadata,
        }))
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
        &self.0.compiled.theory
    }

    /// Shared dense atom meanings, including streamed constraint occurrences.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
        &self.0.compiled.atoms
    }

    /// Original single source, absent for an admitted bundle.
    #[must_use]
    pub fn source(&self) -> Option<&Source> {
        match &self.0.source {
            SourceOwner::Single(source) => Some(source),
            SourceOwner::Bundle(_) => None,
        }
    }

    /// Complete original include bundle, absent for a single source.
    #[must_use]
    pub fn bundle(&self) -> Option<&SourceBundle> {
        match &self.0.source {
            SourceOwner::Bundle(bundle) => Some(bundle),
            SourceOwner::Single(_) => None,
        }
    }

    /// Original declarations and display policy.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.0.metadata
    }

    /// Complete original projection domain, independent of candidate truth.
    #[must_use]
    pub fn projection(&self) -> &crate::PreparedProjection {
        &self.0.compiled.projection
    }

    /// Original source-family warnings, completed before this owner is published.
    #[must_use]
    pub fn warnings(&self) -> &[crate::FormulaWarning] {
        &self.0.compiled.warnings
    }

    /// Human diagnostics against the retained single source or include bundle.
    #[must_use]
    pub fn warning_view(&self) -> impl fmt::Display + '_ {
        WarningView(self)
    }

    /// Empty objective program; authored objective declarations are refused.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.0.compiled.objectives
    }

    /// Analysis of the original source projection, not a certificate for the core.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.0.compiled.analysis
    }

    /// Source program to which the retained analysis applies.
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        &self.0.compiled.analyzed
    }

    /// Whether the analyzed source is exact or a dependency projection.
    #[must_use]
    pub fn analysis_basis(&self) -> crate::AnalysisBasis {
        self.0.compiled.analysis_basis
    }

    /// Applicable key rewrites made during original source preparation.
    #[must_use]
    pub fn keyed_constraints(&self) -> usize {
        self.0.compiled.keyed_constraints
    }

    /// Completion of the bounded original key analysis.
    #[must_use]
    pub fn key_analysis(&self) -> crate::KeyAnalysis {
        self.0.compiled.key_analysis
    }

    /// Source preparation and admission charges, excluding later checks.
    #[must_use]
    pub fn expansion_usage(&self) -> &crate::ExpansionUsage {
        &self.0.compiled.expansion
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
    pub fn checker(
        &self,
        limits: ConstraintCheckLimits,
    ) -> Result<ConstraintChecker<'_>, ConstraintCheckFailure> {
        self.prepare_checker(limits, Counters::default(), scalar_budget(limits))
    }

    /// Prepare an independent checker whose charges share `allowance` with all
    /// other attached checkers, including final full-model checking. Cancellation
    /// is polled during charged setup work and before publishing the checker.
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
            scalar_budget(limits).with_allowance(allowance.clone()),
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
        budget: Budget,
    ) -> Result<ConstraintChecker<'_>, ConstraintCheckFailure> {
        let prepared = if let Some(constraints) = &self.0.constraints {
            let mut formula_limits = constraints.limits;
            formula_limits.max_work = limits.max_work;
            formula_limits.max_substitutions = limits.max_substitutions;
            let completed = constraints
                .catalog
                .snapshot(&formula_limits, &mut counters, constraints.location)
                .map_err(|error| ConstraintCheckFailure {
                    cause: ConstraintCheckCause::Source(Box::new(error)),
                    statistics: ConstraintCheckStatistics {
                        work: counters.work,
                        substitutions: counters.substitutions,
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
            })
        } else {
            None
        };
        Ok(ConstraintChecker {
            owner: self,
            prepared,
            budget,
            accounting: counters.into_accounting(),
        })
    }
}

fn scalar_budget(limits: ConstraintCheckLimits) -> Budget {
    Budget::new(
        ExpansionLimits {
            max_scalar_bytes: limits.max_scalar_bytes,
            ..ExpansionLimits::default()
        },
        0,
    )
}

struct WarningView<'a>(&'a HybridFormula);
impl fmt::Display for WarningView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0.0.source {
            SourceOwner::Single(source) => {
                crate::formula_warning::source_view(self.0.warnings(), source).fmt(f)
            }
            SourceOwner::Bundle(bundle) => {
                crate::formula_warning::bundle_view(self.0.warnings(), bundle).fmt(f)
            }
        }
    }
}

/// Cumulative allowances for one checker, independent of source admission and
/// reduct-oracle work. Zero is a zero allowance, never unlimited. Per-operation
/// binding/storage bounds remain those admitted with the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstraintCheckLimits {
    /// Charged join, scalar and typed atom-lookup work across all checks.
    pub max_work: u64,
    /// Complete substitutions visited across all checks, including false filters.
    pub max_substitutions: u64,
    /// Cumulative copied scalar payload, including generated owned bindings.
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
    /// Copied scalar payload; not live memory or process RSS.
    pub scalar_bytes: usize,
}

/// Satisfaction of the streamed part only, never answer-set membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintVerdict {
    /// Every required admitted constraint instance was checked without violation.
    Satisfied,
    /// A complete admitted constraint body is true in this interpretation.
    Violated {
        /// Original enclosing source rule.
        location: Location,
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
        /// Original enclosing source rule.
        location: Location,
    },
}

/// A check stopped without establishing satisfaction or a violation.
#[derive(Debug)]
pub enum ConstraintCheckCause {
    /// The model catalog or supplied region theory has a different owner.
    WrongProgram,
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
impl fmt::Display for ConstraintCheckCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProgram => f.write_str("constraint candidate belongs to another program"),
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
            Self::WrongProgram | Self::WrongRegionSize { .. } => None,
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
            ConstraintCheckCause::Source(error) => match error.as_ref() {
                FormulaFailure::Interrupted { reason, .. } => Some(*reason),
                _ => None,
            },
            ConstraintCheckCause::Index(AtomIndexError::Stopped(error)) => match error.as_ref() {
                FormulaFailure::Interrupted { reason, .. } => Some(*reason),
                _ => None,
            },
            ConstraintCheckCause::WrongProgram
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
/// No constraint instances or candidate truth are retained between checks.
/// The checker can move between threads. Grounding observation and each check's
/// runtime control remain local to the synchronous operation that uses them.
pub struct ConstraintChecker<'a> {
    owner: &'a HybridFormula,
    prepared: Option<PreparedConstraints<'a>>,
    budget: Budget,
    accounting: Accounting,
}
/// A nonempty source and its borrowed snapshot are present or absent together.
struct PreparedConstraints<'a> {
    source: &'a Constraints,
    completed: CompletedSupport<'a>,
    limits: FormulaLimits,
    /// Original dense IDs, prepared once on first region use. The final-model
    /// path keeps its existing canonical selection lookup.
    index: Option<AtomIndex<'a>>,
    /// Source occurrence IDs mapped once into the original dense catalog.
    rows: Option<SourceRows<'a>>,
    /// Lazily prepared after each rule's first successful predicate gate.
    plans: Vec<Option<PreparedRule<'a>>>,
}

impl<'a> PreparedConstraints<'a> {
    fn prepare_selection(
        &mut self,
        atoms: &'a AtomCatalog,
        counters: &mut Counters,
    ) -> Result<(), ConstraintCheckCause> {
        self.prepare_index(atoms, counters)?;
        if self.rows.is_none() {
            self.rows = Some(
                SourceRows::prepare(
                    &mut self.completed,
                    self.index.as_ref().expect("prepared above"),
                    &self.limits,
                    counters,
                    self.source.location,
                )
                .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?,
            );
        }
        Ok(())
    }

    fn prepare_index(
        &mut self,
        atoms: &'a AtomCatalog,
        counters: &mut Counters,
    ) -> Result<(), ConstraintCheckCause> {
        if self.index.is_some() {
            return Ok(());
        }
        let location = self.source.location;
        // Two retained integer orders and one preparation scratch order, all
        // bounded by the admitted atom count. AtomIndex reserves fallibly and
        // charges each comparison/write; it never copies atom payloads.
        let requested = size_of::<AtomIndex<'_>>() as u128
            + size_of::<Vec<usize>>() as u128
            + 3 * atoms.atoms().len() as u128 * size_of::<usize>() as u128;
        self.completed
            .admit_workspace(requested, &self.limits, location)
            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
        let index = AtomIndex::new_with(atoms.atoms(), || {
            counters.work(&self.limits, location).map_err(Box::new)
        })
        .map_err(|error| match error {
            AtomIndexError::Stopped(error) => ConstraintCheckCause::Source(error),
            other => ConstraintCheckCause::Index(other),
        })?;
        self.completed
            .admit_workspace(index.preparation_peak_bytes(), &self.limits, location)
            .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))?;
        self.completed.retain_workspace(
            usize::try_from(index.retained_bytes()).expect("admitted support bytes fit usize"),
        );
        self.index = Some(index);
        Ok(())
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
            scalar_bytes: self.budget.usage().scalar_bytes,
        }
    }

    /// Check original constraint satisfaction against an exact-catalog model.
    /// Joins borrow completed possible support; each atom is separately read in
    /// the supplied candidate. Shared support is never treated as candidate truth.
    /// A violation returns early; successful satisfaction exhausts every family.
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
                Some(location) => ConstraintVerdict::Violated { location },
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
    /// First use prepares a bounded typed catalog index and source-row ID map;
    /// later checks reuse both. Necessary predicate and held-row selections
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
                Some(location) => ConstraintRegionVerdict::Refuted { location },
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
    ) -> Result<Option<Location>, ConstraintCheckFailure> {
        let result = self.authenticate(candidate).and_then(|()| {
            cancellation.poll().map_err(ConstraintCheckCause::Stopped)?;
            let verdict = self
                .accounting
                .with_cancellation(cancellation, |counters| {
                    if matches!(candidate, Candidate::Region(..))
                        && let Some(prepared) = &mut self.prepared
                    {
                        prepared.prepare_selection(self.owner.atom_catalog(), counters)?;
                    }
                    Self::scan(
                        self.prepared.as_mut(),
                        &mut self.budget,
                        counters,
                        candidate,
                    )
                    .map_err(|error| ConstraintCheckCause::Source(Box::new(error)))
                })?;
            cancellation.poll().map_err(ConstraintCheckCause::Stopped)?;
            Ok(verdict)
        });
        result.map_err(|cause| ConstraintCheckFailure {
            cause,
            statistics: self.statistics(),
        })
    }

    fn scan(
        prepared: Option<&mut PreparedConstraints<'_>>,
        budget: &mut Budget,
        counters: &mut Counters,
        candidate: Candidate<'_>,
    ) -> Result<Option<Location>, FormulaFailure> {
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        let constraints = prepared.source;
        let selection = match candidate {
            Candidate::Model(_) => None,
            Candidate::Region(_, region) => Some(Selection {
                rows: prepared.rows.as_ref().expect("prepared before region scan"),
                index: prepared
                    .index
                    .as_ref()
                    .expect("prepared before region scan"),
                region,
            }),
        };
        for (rule_index, rule) in constraints.rules.iter().enumerate() {
            if let Some(selection) = &selection
                && !selection.possible(rule, &prepared.limits, counters)?
            {
                continue;
            }
            let filter = selection
                .as_ref()
                .map(|selection| selection as &dyn RowFilter);
            if prepared.plans.is_empty() {
                prepared.plans = PreparedRule::slots(
                    &constraints.rules,
                    &mut prepared.completed,
                    &prepared.limits,
                    counters,
                )?;
            }
            if prepared.plans[rule_index].is_none() {
                prepared.plans[rule_index] = Some(PreparedRule::new(
                    rule,
                    &mut prepared.completed,
                    &prepared.limits,
                    budget,
                    counters,
                )?);
            }
            let queries = prepared.completed.queries(
                crate::JoinStrategy::Indexed,
                &prepared.limits,
                counters,
                rule.location,
            )?;
            let mut join = prepared.plans[rule_index]
                .as_ref()
                .expect("prepared above")
                .rows(&queries, filter, budget)?;
            while let Some(row) =
                join.next_row(&prepared.limits, budget, counters, rule.location)?
            {
                if row.passes
                    && body(
                        &rule.body,
                        &row.values,
                        candidate,
                        prepared.index.as_ref(),
                        &prepared.limits,
                        counters,
                        rule.location,
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

fn body(
    literals: &[LiteralIr],
    binding: &Binding<'_>,
    candidate: Candidate<'_>,
    index: Option<&AtomIndex<'_>>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    for literal in literals {
        let Some((negation, pattern)) = literal_atom(literal) else {
            continue;
        };
        counters.work(limits, location)?;
        let key = pattern
            .key(binding.slots())
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
                let row = index
                    .expect("prepared before region scan")
                    .lookup()
                    .get_key_with(&key, || counters.work(limits, location))?;
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

fn literal_atom(literal: &LiteralIr) -> Option<(DefaultNegation, &AtomPattern)> {
    match literal {
        LiteralIr::Atom(negation, pattern) => Some((*negation, pattern)),
        LiteralIr::PatternAtom(pattern) => Some((DefaultNegation::None, &pattern.atom)),
        // The join owns scalar comparisons and generated bindings.
        LiteralIr::Compare(..)
        | LiteralIr::TupleCompare(..)
        | LiteralIr::ArgumentCheck { .. }
        | LiteralIr::Guard(_)
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
            assert!(owner.0.constraints.is_none());
        }
    }
}
