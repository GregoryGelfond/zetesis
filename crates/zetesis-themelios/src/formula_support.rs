//! A finite support upper bound and complete iterative relational joins.

#[cfg(test)]
pub(crate) mod testing;
mod evaluation;
mod computation;
mod context;
pub(crate) use context::{Context, GroundingWork};
pub(crate) mod sort;
pub(crate) mod components;
mod storage;
mod selection;
mod publication;
pub(crate) use computation::Computation;
pub(crate) use publication::Publication;
pub(crate) use selection::SourceSelection;
pub(crate) use storage::{
    StorageLease, growth_capacity, reserve, reserve_exact, reserve_exact_scoped,
};
mod accounting;
mod observation;
mod generated;
mod term_selection;
mod term_table;
pub(crate) use term_table::TermTable;
mod buffer;
pub(crate) use buffer::Buffer;
pub(crate) use term_selection::TermSelection;
mod filters;
mod rows;
pub(crate) mod family;
mod delta;
mod order;
mod prepared;
mod producers;
mod relations;
mod queries;
#[cfg(test)]
mod columnar;
#[cfg(test)]
mod membership;
#[cfg(test)]
mod postings;

use std::borrow::Cow;

use components::Pattern as AtomPattern;
use relations::SupportAppend;
use themelios_base::span::Location;
use themelios_program::program::Relation;
use themelios_program::term::EvalError;
use zetesis_core::catalog::{Atoms, PredicateRef, TermRef};
use zetesis_core::{PatternRef, TemplateTerm};

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Prepared};
use crate::grounding_observer::{Event, Work};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};
use rows::{Frame, Ownership, Staged};

pub(crate) use accounting::{Accounting, AccountingBaseline};
pub(crate) use evaluation::{Evaluation, Failures};
pub(crate) use prepared::PreparedRule;
pub(crate) use queries::{Candidates, Support};
#[cfg(test)]
use relations::RelationRows;
pub(crate) use relations::{
    ClosedSource, Relations, SourceAtom, SourceScope, SupportCatalog, atom_failure, owner_limits,
};
pub(crate) use rows::{FilteredRows, RowFilter};

/// Possible atoms after a complete support round added no new head.
///
/// Construction is private to `build`: reaching a resource ceiling never
/// produces this owner. Its finite fixed point is an observed result, not a
/// promise that recursive value generation terminates for every source.
///
/// Answer-set coverage additionally requires the source producer's reduct
/// projection property. `NormalSupport.projection_compatible` proves it for
/// mathematical normalized rules; complete source bindings, aggregate values
/// and head permissions must connect this builder to that property. The
/// producer map in `proofs/guide/source-support.md` records those boundaries.
pub(crate) struct CompletedCatalog {
    catalog: SupportCatalog,
    completion: Completion,
}

/// An immutable relation view of one successfully completed support owner.
/// Intermediate round snapshots deliberately have only the `Relations` type.
pub(crate) struct CompletedSupport<'source> {
    lookup_owner: Option<&'source zetesis_core::atom_interner::AtomInterner>,
    completion: &'source Completion,
    relations: Relations<'source>,
}

/// A completed support membership has one identity even while harmless term
/// admission continues. Only successful support completion creates this token.
struct Completion(std::sync::Arc<()>);
impl Completion {
    fn same(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl CompletedCatalog {
    pub(crate) fn split(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(CompletedSupport<'_>, SupportAppend<'_>), FormulaFailure> {
        let (relations, append) = self.catalog.split(limits, counters, location)?;
        Ok((
            CompletedSupport {
                lookup_owner: None,
                completion: &self.completion,
                relations,
            },
            append,
        ))
    }

    pub(crate) fn snapshot(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<CompletedSupport<'_>, FormulaFailure> {
        self.catalog
            .snapshot(limits, counters, location)
            .map(|relations| CompletedSupport {
                lookup_owner: Some(self.catalog.owner()),
                completion: &self.completion,
                relations,
            })
    }
}

impl<'source> CompletedSupport<'source> {
    /// Source metadata retains its admitted owner independently of query scratch.
    pub(crate) fn components(&self) -> Option<zetesis_core::TemplateComponentsRef<'source>> {
        self.relations.components()
    }

    /// Borrow original support occurrences without copying typed atoms. Each
    /// slice position is the corresponding predicate relation's row position,
    /// not a dense position in an independently prepared formula catalog.
    pub(crate) fn source_atoms(
        &self,
    ) -> impl Iterator<Item = (PredicateRef<'source>, Atoms<'source>)> + '_ {
        self.relations.source_atoms()
    }

    /// Keep a checker's additional retained index inside the same support-byte
    /// allowance as its borrowed catalog and query descriptor. Preparation also
    /// checks its simultaneous scratch before allocating it.
    pub(crate) fn admit_workspace(
        &self,
        bytes: u128,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::SupportBytes,
            self.relations.bytes as u128
                + size_of::<Support<'_>>() as u128
                + counters.accounting.workspace.bytes() as u128
                + bytes,
            limits.max_support_bytes as u128,
            location,
        )
    }

    pub(crate) fn retain_workspace(&mut self, bytes: usize) {
        // The caller admitted this exact retained capacity before publication.
        self.relations.bytes += bytes;
    }

    /// Borrow the same typed rows used by final grounding and objective joins.
    pub(crate) fn queries(
        &self,
        strategy: crate::JoinStrategy,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<CompletedQueries<'_>, FormulaFailure> {
        Ok(CompletedQueries {
            lookup_owner: self.lookup_owner,
            completion: self.completion,
            support: Support::completed(&self.relations, strategy, limits, counters, location)?,
        })
    }
}

/// Queries over exactly one completed support certificate. Only its completed
/// snapshot constructs this workspace; growing relations cannot claim it.
pub(crate) struct CompletedQueries<'source> {
    lookup_owner: Option<&'source zetesis_core::atom_interner::AtomInterner>,
    completion: &'source Completion,
    support: Support<'source>,
}
impl<'source> CompletedQueries<'source> {
    /// Independent checking resolves only terms already visited during complete
    /// original-source admission. A growing query needs its explicit appender.
    pub(crate) fn computation(
        &self,
        location: Location,
    ) -> Result<Computation<'_, 'source>, FormulaFailure> {
        let owner = self.lookup_owner.ok_or(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            location,
        })?;
        Ok(Computation::frozen(owner.term_lookup(), &self.support))
    }

    pub(crate) fn support(&self) -> &Support<'source> {
        &self.support
    }
}

#[derive(Default)]
pub(crate) struct Counters {
    pub(crate) accounting: Accounting,
    cancellation: Option<zetesis_cpu::Cancellation>,
    observed: Work,
}
impl Counters {
    pub(crate) fn with_allowance(
        allowance: crate::ConstraintAllowance,
        cancellation: &zetesis_cpu::Cancellation,
    ) -> Self {
        Self {
            accounting: Accounting {
                allowance: Some(allowance),
                ..Accounting::default()
            },
            cancellation: Some(cancellation.clone()),
            observed: Work::default(),
        }
    }

    pub(crate) fn resume(accounting: Accounting, observed: Work) -> Self {
        Self {
            accounting,
            observed,
            cancellation: None,
        }
    }
    pub(super) fn record(&self, event: Event) {
        self.observed.record(event);
    }
    pub fn work(
        &mut self,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.charge_work(1, limits, location)
    }
    pub(super) fn charge_work(
        &mut self,
        amount: u128,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        charge_work(
            &mut self.accounting.work,
            self.cancellation.as_ref(),
            self.accounting.allowance.as_ref(),
            amount,
            limits,
            location,
        )
    }
    pub(super) fn generated(
        &mut self,
        value: &zetesis_core::catalog::TermKey,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if self.accounting.generated_values.is_none() {
            let mut generated =
                generated::Generated::new(computation.read(), self.accounting.workspace.lease());
            generated.lease.observe(generated.bytes(), location)?;
            self.accounting.generated_values = Some(generated);
        }
        let generated = self
            .accounting
            .generated_values
            .as_ref()
            .expect("created above");
        let maximum = computation.allowance(&generated.lease, limits, location)?;
        let previous = generated.lease.bytes();
        let generated = self
            .accounting
            .generated_values
            .as_mut()
            .expect("created above");
        let result = generated.select(
            value,
            maximum,
            limits,
            || {
                charge_work(
                    &mut self.accounting.work,
                    self.cancellation.as_ref(),
                    self.accounting.allowance.as_ref(),
                    1,
                    limits,
                    location,
                )
            },
            location,
        );
        generated.lease.observe(generated.bytes(), location)?;
        let generated = self
            .accounting
            .generated_values
            .as_ref()
            .expect("retained through the operation");
        let observed = computation.storage_observed(
            &generated.lease,
            previous,
            size_of::<generated::Generated>(),
            limits,
            self,
            location,
        );
        computation.storage_result(result, &generated.lease, limits, location)?;
        observed
    }
    pub(crate) fn substitution(
        &mut self,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Substitutions,
            u128::from(self.accounting.substitutions) + 1,
            u128::from(limits.max_substitutions),
            location,
        )?;
        if let Some(allowance) = &self.accounting.allowance {
            allowance.substitution(location)?;
        }
        self.accounting.substitutions += 1;
        Ok(())
    }
}

fn charge_work(
    work: &mut u64,
    cancellation: Option<&zetesis_cpu::Cancellation>,
    allowance: Option<&crate::ConstraintAllowance>,
    amount: u128,
    limits: &FormulaLimits,
    location: Location,
) -> Result<(), FormulaFailure> {
    if let Some(cancellation) = cancellation {
        cancellation
            .poll()
            .map_err(|reason| FormulaFailure::Interrupted { reason, location })?;
    }
    ceiling(
        FormulaResource::Work,
        u128::from(*work) + amount,
        u128::from(limits.max_work),
        location,
    )?;
    if let Some(allowance) = allowance {
        allowance.work(amount, location)?;
    }
    *work += u64::try_from(amount).expect("charged work fits its u64 ceiling");
    Ok(())
}

pub(crate) fn build(
    catalog: SupportCatalog,
    prepared: &Prepared,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    fallback: Location,
) -> Result<CompletedCatalog, FormulaFailure> {
    counters.observe_work(Event::SupportConstructionWork, |counters| {
        let plan =
            producers::ProducerPlan::prepare(prepared, &catalog, limits, counters, fallback)?;
        complete(
            catalog,
            prepared,
            plan,
            domains,
            budget,
            GroundingWork::new(limits, counters, fallback),
        )
    })
}

/// Every round joins each selected rule under its domain guards, when the
/// analysis prepared candidates: a row a guard rejects has no complete
/// continuation the exclusion rule keeps, in this round as in the final one.
fn complete(
    mut catalog: SupportCatalog,
    prepared: &Prepared,
    mut plan: Option<producers::ProducerPlan<'_>>,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    budget: &mut Budget,
    work: GroundingWork<'_>,
) -> Result<CompletedCatalog, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location: fallback,
    } = work;
    catalog.prepared_bytes(plan.as_ref().map_or(0, producers::ProducerPlan::bytes));
    catalog.publish(limits, counters, fallback)?;
    #[cfg(test)]
    postings::begin_support();
    let mut rounds = 0_u64;
    loop {
        ceiling(
            FormulaResource::SupportRounds,
            u128::from(rounds) + 1,
            u128::from(limits.max_support_rounds),
            fallback,
        )?;
        rounds += 1;
        counters.record(Event::SupportRound);
        let changed = {
            let mut schedule = plan.as_ref().map_or_else(
                || producers::Schedule::all(&prepared.rules),
                producers::ProducerPlan::schedule,
            );
            let mut selected = schedule.next(limits, counters, fallback)?;
            if selected.is_none() && plan.is_some() {
                // Every zero-input producer ran at bootstrap. Every subsequent
                // new positive binding needs a changed predicate, whose complete
                // posting would have selected its original producer. Thus this
                // admitted round is unchanged without a new relation snapshot.
                return finish(
                    catalog,
                    plan,
                    GroundingWork::new(limits, counters, fallback),
                );
            }
            counters.work(limits, fallback)?;
            counters.record(Event::SupportSnapshotPreparation);
            let (relations, mut delta) = catalog.split(limits, counters, fallback)?;
            let support = Support::indexed(&relations, limits, counters, fallback)?;
            counters.observe_work(Event::SupportProductionWork, |counters| {
                while let Some(index) = selected {
                    let rule = &prepared.rules[index];
                    counters.work(limits, rule.location)?;
                    counters.record(Event::SupportProducerVisit);
                    let mut variants = match &plan {
                        Some(plan) => {
                            plan.variants(index, rule, &support, rounds == 1, limits, counters)?
                        }
                        None => delta::variants(rule, &support, rounds == 1, limits, counters)?,
                    };
                    let mut computation = Computation::new(&mut delta, &support);
                    let guards = match domains {
                        Some(domains) => support.domain_guards(
                            rule,
                            domains.for_rule(index, rule)?,
                            &computation,
                            limits,
                            counters,
                        )?,
                        None => None,
                    };
                    derive_variants(
                        rule,
                        &mut variants,
                        guards.as_ref(),
                        &support,
                        budget,
                        Context::new(&mut computation, limits, counters, rule.location),
                    )?;
                    selected = schedule.next(limits, counters, fallback)?;
                }
                Ok::<_, FormulaFailure>(())
            })?;
            // No join remains live. Release its query descriptor before the
            // pending selection borrows the catalog's semantic ordering index.
            drop(support);
            advance_round(
                &mut delta,
                plan.as_mut(),
                GroundingWork::new(limits, counters, fallback),
            )?
        };
        if !changed {
            return finish(
                catalog,
                plan,
                GroundingWork::new(limits, counters, fallback),
            );
        }
        catalog.publish(limits, counters, fallback)?;
    }
}

/// Prepare the next wake set only after the complete pending population has its
/// publication order. Refusal leaves private round metadata, never completion.
fn advance_round(
    delta: &mut SupportAppend<'_>,
    plan: Option<&mut producers::ProducerPlan<'_>>,
    work: GroundingWork<'_>,
) -> Result<bool, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    if delta.is_empty() {
        return Ok(false);
    }
    counters.observe_work(Event::SupportOrderWork, |counters| {
        delta.order(limits, counters, location)
    })?;
    if let Some(plan) = plan {
        counters.observe_work(Event::SupportWakeWork, |counters| {
            plan.advance(delta.atoms(), limits, counters, location)
        })?;
    }
    Ok(true)
}

/// The fixed point drops its scheduling metadata before final publication.
fn finish(
    mut catalog: SupportCatalog,
    plan: Option<producers::ProducerPlan<'_>>,
    work: GroundingWork<'_>,
) -> Result<CompletedCatalog, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    drop(plan);
    catalog.release_preparation();
    catalog.publish(limits, counters, location)?;
    Ok(CompletedCatalog {
        catalog,
        completion: Completion(std::sync::Arc::new(())),
    })
}

/// Visit one rule's ordered delta variants under the same admitted guards.
fn derive_variants<'source>(
    rule: &crate::formula_ir::RuleIr,
    variants: &mut delta::Variants<'_, '_>,
    guards: Option<&queries::Guards<'_, 'source>>,
    support: &Support<'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Result<(), FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location: _,
            },
    } = context;
    while let Some(variant) = variants.next(limits, counters)? {
        let mut outer = Join::variant_rule(
            rule,
            variant,
            support,
            guards,
            budget,
            Context::new(computation, limits, counters, rule.location),
        )?;
        derive_rule(
            rule,
            &mut outer,
            support,
            computation,
            limits,
            budget,
            counters,
        )?;
    }
    Ok(())
}

fn derive_rule(
    rule: &crate::formula_ir::RuleIr,
    outer: &mut Join<'_, '_>,
    support: &Support<'_>,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    let mut derivation = Derivation {
        computation,
        limits,
        budget,
        counters,
    };
    let projected = match &rule.head {
        HeadIr::Normal(head) => *head,
        _ => None,
    };
    while let Some(binding) = derivation.next(outer, projected, rule.location)? {
        match &rule.head {
            HeadIr::Normal(Some(head)) => derivation.head(*head, &binding, rule.location)?,
            HeadIr::Disjunction(heads) => {
                // Only positive occurrences can produce possible atoms.
                // Neither default-negation mode supplies support.
                for head in heads.iter().filter_map(|head| head.positive_atom()) {
                    derivation.head(*head, &binding, rule.location)?;
                }
            }
            HeadIr::ConditionalDisjunction { ordinary, elements } => {
                for head in ordinary.iter().filter_map(|head| head.positive_atom()) {
                    derivation.head(*head, &binding, rule.location)?;
                }
                for element in elements {
                    let mut local = Join::local_head(
                        &element.condition,
                        &binding.prefix(element.outer_variables),
                        element.body_variables..element.variables,
                        support,
                        derivation.budget,
                        Context::new(
                            derivation.computation,
                            limits,
                            derivation.counters,
                            rule.location,
                        ),
                    )?;
                    while let Some(binding) = derivation.next(&mut local, None, rule.location)? {
                        if let Some(head) = element.head.positive_atom() {
                            derivation.head(*head, &binding, rule.location)?;
                        }
                    }
                }
            }
            HeadIr::Choice(group) => {
                derivation.choice(group, &binding, support, rule.location)?;
            }
            HeadIr::Normal(None) => unreachable!("constraints never produce support"),
        }
    }
    Ok(())
}

/// A support round publishes newly derived heads after complete checked binding.
/// Existing support and the current delta share one borrowed atom identity.
struct Derivation<'a, 'source, 'round> {
    computation: &'a mut Computation<'source, 'round>,
    limits: &'a FormulaLimits,
    budget: &'a mut Budget,
    counters: &'a mut Counters,
}

impl Derivation<'_, '_, '_> {
    /// Select a complete support binding before the consumer borrows its frame.
    /// Attribution observes admitted operations; it adds no work charges.
    fn next<'join>(
        &mut self,
        join: &'join mut Join<'_, '_>,
        projected: Option<AtomPattern>,
        location: Location,
    ) -> Result<Option<Binding<'join>>, FormulaFailure> {
        self.counters
            .observe_work(Event::SupportJoinWork, |counters| {
                join.next_support(
                    projected,
                    self.computation,
                    self.limits,
                    self.budget,
                    counters,
                    location,
                )
            })
    }

    fn choice(
        &mut self,
        group: &crate::formula_ir::ChoiceIr,
        binding: &Binding,
        support: &Support<'_>,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        crate::formula_head_aggregate::validate_group(
            group,
            binding,
            support,
            self.budget,
            Context::new(self.computation, self.limits, self.counters, location),
        )?;
        for element in &group.elements {
            let mut local = Join::element(
                element,
                binding,
                support,
                self.budget,
                Context::new(self.computation, self.limits, self.counters, location),
            )?;
            while let Some(binding) = self.next(&mut local, None, location)? {
                if let Some(head) = element.head.positive_atom() {
                    self.head(*head, &binding, location)?;
                }
            }
        }
        Ok(())
    }

    fn head(
        &mut self,
        pattern: AtomPattern,
        binding: &Binding,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.counters
            .observe_work(Event::SupportHeadWork, |counters| {
                let pattern =
                    self.computation
                        .static_pattern(pattern, self.limits, counters, location)?;
                let atom =
                    self.computation
                        .atom(pattern, binding, self.limits, counters, location)?;
                self.computation
                    .support(&atom, self.limits, counters, location)
            })
    }
}

/// Validation evidence for one complete positive binding. A comparison that
/// is defined and false excludes the substitution before this certificate is
/// issued, so a certificate says at most that every comparison was decided
/// at its depth and passed. An arithmetic failure is retained until the
/// positive join has a complete extension no comparison excludes; an
/// incomplete prefix alone does not require evaluating a ground source
/// instance.
#[derive(Clone)]
enum Comparisons {
    /// Some check waits for the complete row.
    Deferred,
    Failed(EvalError),
    /// Every comparison was decided at its depth, and passed.
    Verified,
}

// The same whole-argument index serves flat atoms and structural captures.
#[derive(Clone, Copy)]
enum PositivePattern<'a> {
    Flat(PatternRef<'a>),
    Structural(crate::formula_pattern::Pattern<'a>),
}
impl<'a> PositivePattern<'a> {
    fn atom(self) -> PatternRef<'a> {
        match self {
            Self::Flat(atom) => atom,
            Self::Structural(pattern) => pattern.atom(),
        }
    }
}

/// Source occurrence identity survives join reordering.
#[derive(Clone, Copy)]
struct PatternOccurrence<'a> {
    pattern: PositivePattern<'a>,
    source: usize,
}
impl<'a> PatternOccurrence<'a> {
    fn atom(self) -> PatternRef<'a> {
        self.pattern.atom()
    }
}

/// Why an unfilled slot can or cannot remain absent in the current scope.
#[derive(Clone, Copy)]
enum Slot {
    Relational,
    Generated,
    Excluded,
}

/// Whether pruning may select rows or every complete row must supply evidence.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Coverage {
    Selected,
    Complete,
}

/// Base-join progression is separate from generated continuations. Searching
/// owns a live DFS prefix. `PendingUndo` owns one successfully completed nonempty
/// frame at depth == `patterns.len()`; its final depth is undone exactly once.
/// `EmptyVisited` records entering the sole empty-pattern prefix, even if its
/// filtering or completion refused. It becomes Finished only on a subsequent
/// admitted traversal step. Finished has no remaining base row and costs no
/// further traversal work; an existing generator can still drain its own rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Traversal {
    Searching,
    PendingUndo,
    EmptyVisited,
    Finished,
}

/// The cursor owns its current assignment, undo trails and bounded expression
/// storage. Ordinary joins enumerate upper support and retain negative gates
/// in emitted formulas. An explicit admitted-source scan may additionally
/// borrow a necessary positive-row filter; it never supplies family evidence.
pub(crate) struct Join<'a, 'source> {
    bindings: Option<&'a crate::formula_assignment_plan::Plan>,
    literals: &'a [LiteralIr],
    generated: bool,
    /// The certificate of the last completed row, taken by its consumer.
    comparisons: Comparisons,
    coverage: Coverage,
    family: family::Evidence,
    head_bounds: &'a [crate::formula_ir::AggregateGuard],
    checked_guard: Option<&'a crate::formula_guard::Guard>,
    /// What each prefix of the current order decides.
    plan: Cow<'a, order::Plan<'a>>,
    /// The conjunction of the comparisons decided up to each depth.
    verdicts: Vec<bool>,
    /// The first evaluation failure on the current prefix and the depth that
    /// met it; released when that depth is undone.
    failure: Option<(usize, EvalError)>,
    evaluation: Evaluation,
    pending: Option<crate::formula_binding_cursor::Cursor<'a, 'source>>,
    pending_head: Option<crate::formula_binding_cursor::Cursor<'a, 'source>>,
    head_slots: std::ops::Range<usize>,
    delta: Option<usize>,
    domains: Option<&'a queries::Guards<'a, 'source>>,
    row_filter: Option<&'a dyn RowFilter>,
    support: &'a Support<'source>,
    values: Binding<'static>,
    lease: StorageLease,
    trail_bytes: usize,
    slots: Vec<Slot>,
    positions: Vec<usize>,
    probes: Vec<Option<Probe<'a, 'source>>>,
    /// Borrowed relations are resolved once per occurrence in this snapshot.
    resolutions: Vec<Resolution<'source>>,
    changes: Vec<Vec<usize>>,
    depth: usize,
    traversal: Traversal,
}
/// One complete body binding and its scalar selection result. Selected rule
/// rows also contain their evaluated head suffix; rejected rows contain only
/// the genuine body frame needed by scoped source validation.
pub(crate) struct Row<'a> {
    pub values: Binding<'a>,
    pub passes: bool,
}

/// A row source retains its original occurrence indices. Indexed positions
/// count posting entries; table positions are the next source row to inspect.
enum Probe<'a, 'source> {
    Indexed(delta::Rows<'a>),
    Table(queries::Rows<'a, 'source>),
}

#[derive(Clone, Copy)]
enum Resolution<'source> {
    Unresolved,
    Resolved(Option<&'source relations::RelationRows<'source>>),
}

impl<'source> Resolution<'source> {
    fn rows(self) -> Option<&'source relations::RelationRows<'source>> {
        match self {
            Self::Unresolved => unreachable!("a prepared probe has resolved its relation"),
            Self::Resolved(rows) => rows,
        }
    }
}
impl Probe<'_, '_> {
    fn next(
        &self,
        position: &mut usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<usize>, FormulaFailure> {
        let row = match self {
            Self::Indexed(rows) => {
                let row = rows.get(*position);
                if row.is_some() {
                    *position += 1;
                }
                row
            }
            Self::Table(rows) => {
                let row = rows.next(*position, limits, counters, location)?;
                if let Some(row) = row {
                    *position = row + 1;
                    counters.record(Event::TableRow);
                }
                row
            }
        };
        Ok(row)
    }
}

impl<'a, 'source> Join<'a, 'source> {
    pub(super) fn rule(
        rule: &'a crate::formula_ir::RuleIr,
        support: &'a Support<'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(
            &rule.body,
            &Binding::new(computation, limits, counters, rule.location)?,
            rule.variables,
            support,
            budget,
            Context::new(computation, limits, counters, rule.location),
        )?;
        join.configure_rule(rule, limits, counters)?;
        Ok(join)
    }

    fn configure_rule(
        &mut self,
        rule: &'a crate::formula_ir::RuleIr,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        self.bindings = rule.bindings.as_ref();
        self.stage_head(
            rule.body_variables..rule.variables,
            limits,
            counters,
            rule.location,
        )?;
        if let HeadIr::Choice(choice) = &rule.head {
            self.head_bounds = &choice.guards;
            if choice
                .guards
                .iter()
                .any(|guard| family::expression(&guard.bound))
            {
                self.coverage = Coverage::Complete;
            }
        }
        Ok(())
    }

    /// Select rows only after the caller has admitted the complete source
    /// family. The filter must preserve every witness sought by that consumer;
    /// it is not a new support owner or an arithmetic admission certificate.
    /// The returned view deliberately has no family-evidence operation.
    fn filtered_rule(
        rule: &'a crate::formula_ir::RuleIr,
        support: &'a Support<'source>,
        filter: Option<&'a dyn RowFilter>,
        plan: Option<&'a order::Plan<'a>>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<FilteredRows<'a, 'source>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location: _,
                },
        } = context;
        let mut join = if let Some(plan) = plan {
            let mut join = Self::with_plan(
                &rule.body,
                &Binding::new(computation, limits, counters, rule.location)?,
                rule.variables,
                support,
                Cow::Borrowed(plan),
                Context::new(computation, limits, counters, rule.location),
            )?;
            join.configure_rule(rule, limits, counters)?;
            join
        } else {
            Self::rule(rule, support, computation, limits, budget, counters)?
        };
        join.row_filter = filter;
        Ok(FilteredRows::new(join))
    }

    /// Attach necessary domains to their exact rule and support owner.
    pub(super) fn domain_rule(
        rule: &'a crate::formula_ir::RuleIr,
        support: &'a Support<'source>,
        domains: Option<&'a queries::Guards<'a, 'source>>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        Self::variant_rule(
            rule,
            delta::Variant::Full,
            support,
            domains,
            budget,
            Context::new(computation, limits, counters, rule.location),
        )
    }

    fn variant_rule(
        rule: &'a crate::formula_ir::RuleIr,
        variant: delta::Variant,
        support: &'a Support<'source>,
        domains: Option<&'a queries::Guards<'a, 'source>>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location: _,
                },
        } = context;
        if domains.is_some_and(|guards| !guards.belongs_to(rule, support)) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        }
        let mut join = Self::new_with_rows(
            &rule.body,
            &Binding::new(computation, limits, counters, rule.location)?,
            rule.variables,
            support,
            budget,
            Context::new(computation, limits, counters, rule.location),
            variant,
        )?;
        join.configure_rule(rule, limits, counters)?;
        join.domains = domains;
        Ok(join)
    }

    pub(super) fn element(
        element: &'a crate::formula_ir::Element,
        prefix: &Binding,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        Self::local_head(
            &element.condition,
            prefix,
            element.body_variables..element.variables,
            support,
            budget,
            Context::new(computation, limits, counters, location),
        )
    }

    /// Choice and conditional heads share one condition/head frame boundary.
    pub(super) fn local_head(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        head_slots: std::ops::Range<usize>,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut join = Self::new(
            literals,
            prefix,
            head_slots.end,
            support,
            budget,
            Context::new(computation, limits, counters, location),
        )?;
        join.stage_head(head_slots, limits, counters, location)?;
        Ok(join)
    }

    fn stage_head(
        &mut self,
        slots: std::ops::Range<usize>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.values
            .truncate(slots.start, limits, counters, location)?;
        self.slots.truncate(slots.start);
        self.generated = self
            .slots
            .iter()
            .any(|slot| matches!(slot, Slot::Generated));
        self.head_slots = slots;
        Ok(())
    }

    pub(crate) fn objective(
        objective: &'a crate::formula_ir::ObjectiveIr,
        support: &'a Support<'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(
            objective.condition.literals(),
            &Binding::new(computation, limits, counters, objective.location)?,
            objective.variables,
            support,
            budget,
            Context::new(computation, limits, counters, objective.location),
        )?;
        if let crate::formula_ir::ObjectiveCondition::Body { bindings, .. } = &objective.condition {
            join.bindings = bindings.as_ref();
        }
        Ok(join)
    }

    pub(super) fn component(
        literals: &'a [LiteralIr],
        variables: usize,
        used: &[usize],
        fixed: &Binding,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut join = Self::new(
            literals,
            &Binding::new(computation, limits, counters, location)?,
            variables,
            support,
            budget,
            Context::new(computation, limits, counters, location),
        )?;
        for slot in 0..fixed.len().min(join.values.len()) {
            counters.work(limits, location)?;
            if let Some(value) = fixed
                .slots()
                .key(slot)
                .map_err(|error| crate::formula_binding::assignment(error, location))?
            {
                join.values.set(slot, &value, limits, counters, location)?;
            }
        }
        let mut used_cursor = 0;
        for (variable, slot) in join.slots.iter_mut().enumerate() {
            counters.work(limits, location)?;
            while used.get(used_cursor).is_some_and(|&used| used < variable) {
                counters.work(limits, location)?;
                used_cursor += 1;
            }
            if used.get(used_cursor) != Some(&variable) {
                *slot = Slot::Excluded;
            }
        }
        join.decide(computation, limits, budget, counters, location)?;
        Ok(join)
    }
    pub fn new(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        Self::new_with_rows(
            literals,
            prefix,
            variables,
            support,
            budget,
            context,
            delta::Variant::Full,
        )
    }

    fn new_with_rows(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
        variant: delta::Variant,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut admit =
            |capacity| computation.preparation_capacity(capacity, limits, counters, location);
        let plan = order::Plan::new(
            literals,
            prefix,
            variables,
            order::SourceRows {
                relations: support,
                pivot: match variant {
                    delta::Variant::Full => None,
                    delta::Variant::Delta(pivot) => Some(pivot),
                },
            },
            budget,
            location,
            Some(&mut admit),
        )?;
        Self::with_plan(
            literals,
            prefix,
            variables,
            support,
            Cow::Owned(plan),
            Context::new(computation, limits, counters, location),
        )
    }

    fn with_plan(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        plan: Cow<'a, order::Plan<'a>>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let count = plan.patterns.len();
        let delta = plan.pivot;
        let mut lease = computation.lease();
        let plan_bytes = match &plan {
            Cow::Owned(plan) => plan.retained_bytes(),
            Cow::Borrowed(_) => 0,
        };
        let header = size_of::<Self>() - size_of::<Binding>() - size_of::<Evaluation>();
        let initial = usize::try_from(plan_bytes + header as u128).map_err(|_| {
            crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Storage(
                    zetesis_core::catalog::Error::Overflow,
                ),
                location,
            )
        })?;
        lease.observe(initial, location)?;
        computation.storage_observed(&lease, 0, initial, limits, counters, location)?;
        let mut values = prefix.copied(computation, limits, counters, location)?;
        values.extend_scope(variables, computation, limits, counters, location)?;
        let mut join = Self {
            bindings: None,
            literals,
            generated: literals
                .iter()
                .any(|literal| crate::formula_binding_cursor::target(literal).is_some()),
            pending: None,
            pending_head: None,
            head_slots: variables..variables,
            comparisons: Comparisons::Deferred,
            coverage: if family::partial(literals) {
                Coverage::Complete
            } else {
                Coverage::Selected
            },
            family: family::Evidence::default(),
            head_bounds: &[],
            checked_guard: None,
            plan,
            verdicts: Vec::new(),
            failure: None,
            evaluation: Evaluation::default(),
            delta,
            domains: None,
            row_filter: None,
            support,
            values,
            lease,
            trail_bytes: 0,
            slots: Vec::new(),
            positions: Vec::new(),
            probes: Vec::new(),
            resolutions: Vec::new(),
            changes: Vec::new(),
            depth: 0,
            traversal: Traversal::Searching,
        };
        join.lease.observe(join.storage_bytes(), location)?;
        computation.storage_observed(
            &join.lease,
            0,
            join.storage_header(),
            limits,
            counters,
            location,
        )?;
        join.initialize_storage(variables, count, computation, limits, counters, location)?;
        for target in literals
            .iter()
            .filter_map(crate::formula_binding_cursor::target)
        {
            join.slots[target] = Slot::Generated;
        }
        for (index, slot) in join.slots.iter_mut().enumerate().take(prefix.len()) {
            counters.work(limits, location)?;
            if !prefix.is_bound(index, location)? {
                *slot = Slot::Excluded;
            }
        }
        Ok(join)
    }

    fn storage_header(&self) -> usize {
        size_of::<Self>()
            - size_of::<Binding>()
            - size_of::<Evaluation>()
            - usize::from(self.pending.is_some())
                * size_of::<crate::formula_binding_cursor::Cursor<'_, '_>>()
            - usize::from(self.pending_head.is_some())
                * size_of::<crate::formula_binding_cursor::Cursor<'_, '_>>()
    }
    fn storage_bytes(&self) -> usize {
        let plan = match &self.plan {
            Cow::Owned(plan) => plan.retained_bytes() as usize,
            Cow::Borrowed(_) => 0,
        };
        self.storage_header()
            + plan
            + self.trail_bytes
            + self.slots.capacity() * size_of::<Slot>()
            + self.verdicts.capacity() * size_of::<bool>()
            + self.positions.capacity() * size_of::<usize>()
            + self.probes.capacity() * size_of::<Option<Probe<'_, '_>>>()
            + self.resolutions.capacity() * size_of::<Resolution<'_>>()
            + self.changes.capacity() * size_of::<Vec<usize>>()
    }
    fn initialize_storage(
        &mut self,
        variables: usize,
        count: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let other = self.storage_bytes();
        reserve(
            &mut self.slots,
            variables,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..variables {
            counters.work(limits, location)?;
            self.slots.push(Slot::Relational);
        }
        let other = self.storage_bytes();
        reserve(
            &mut self.verdicts,
            count.max(1),
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..count.max(1) {
            counters.work(limits, location)?;
            self.verdicts.push(true);
        }
        let other = self.storage_bytes();
        reserve(
            &mut self.positions,
            count,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..count {
            counters.work(limits, location)?;
            self.positions.push(0);
        }
        let other = self.storage_bytes();
        reserve(
            &mut self.probes,
            count,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..count {
            counters.work(limits, location)?;
            self.probes.push(None);
        }
        let other = self.storage_bytes();
        reserve(
            &mut self.resolutions,
            count,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..count {
            counters.work(limits, location)?;
            self.resolutions.push(Resolution::Unresolved);
        }
        let other = self.storage_bytes();
        reserve(
            &mut self.changes,
            count,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in 0..count {
            counters.work(limits, location)?;
            self.changes.push(Vec::new());
        }
        Ok(())
    }
    fn reserve_trail(
        &mut self,
        additional: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let old = self.changes[self.depth].capacity() * size_of::<usize>();
        let other = self.storage_bytes() - old;
        let result = reserve(
            &mut self.changes[self.depth],
            additional,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        );
        self.trail_bytes =
            self.trail_bytes - old + self.changes[self.depth].capacity() * size_of::<usize>();
        result
    }
    /// Fix, for the current order and prefix, the depth at which each
    /// comparison is decided and whether any check waits for the complete
    /// row. Called after every arrangement and after the prefix is fixed.
    pub(crate) fn check_guard(&mut self, guard: &'a crate::formula_guard::Guard) {
        if guard.expressions().any(family::expression) {
            self.coverage = Coverage::Complete;
        }
        self.checked_guard = Some(guard);
    }
    pub(crate) fn evidence(&mut self) {
        self.coverage = Coverage::Complete;
        self.domains = None;
    }
    pub(crate) fn take_family(&mut self) -> family::Evidence {
        std::mem::take(&mut self.family)
    }
    fn owned_plan(&mut self) -> &mut order::Plan<'a> {
        match &mut self.plan {
            Cow::Owned(plan) => plan,
            Cow::Borrowed(_) => unreachable!("prepared plans are immutable"),
        }
    }
    fn bound_slots(
        &self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Buffer<bool>, FormulaFailure> {
        let mut prefix = Buffer::new(computation, limits, counters, location)?;
        for slot in 0..self.values.len() {
            counters.work(limits, location)?;
            prefix.push(
                self.values.is_bound(slot, location)?,
                computation,
                limits,
                counters,
                location,
            )?;
        }
        Ok(prefix)
    }
    fn decide(
        &mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let prefix = self.bound_slots(computation, limits, counters, location)?;
        let mut admit =
            |capacity| computation.preparation_capacity(capacity, limits, counters, location);
        let decisions = order::Decisions::checked(
            self.literals,
            &self.plan.patterns,
            prefix.slice(),
            budget,
            location,
            &mut admit,
        )?;
        self.owned_plan().decisions = decisions;
        self.lease.observe(self.storage_bytes(), location)?;
        computation.storage_observed(
            &self.lease,
            self.lease.bytes(),
            self.storage_header(),
            limits,
            counters,
            location,
        )
    }
    pub fn next(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        Ok(self
            .next_selected(
                Ownership::Own,
                None,
                budget,
                Context::new(computation, limits, counters, location),
            )?
            .map(Frame::into_owned))
    }
    /// Return each completed positive binding with its scalar selection result.
    /// Ordinary admission joins have no external row filter and visit the full family.
    pub(crate) fn next_row(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Row<'_>>, FormulaFailure> {
        let row = self.next_staged(
            Ownership::Lend,
            None,
            budget,
            Context::new(computation, limits, counters, location),
        )?;
        Ok(row.map(|row| Row {
            values: row.frame.into_binding(&self.values),
            passes: row.passes,
        }))
    }
    /// A continuation that outlives the current traversal step requests its
    /// snapshot before filtering, rather than copying a borrowed result later.
    pub(crate) fn next_owned_row(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Row<'static>>, FormulaFailure> {
        let row = self.next_staged(
            Ownership::Own,
            None,
            budget,
            Context::new(computation, limits, counters, location),
        )?;
        Ok(row.map(|row| Row {
            values: row.frame.into_owned(),
            passes: row.passes,
        }))
    }
    /// Support consumes a selected binding before advancing this join. Lend the
    /// current frame; retained arithmetic generators still produce owned frames.
    /// The next advance performs the suspended undo after head admission.
    fn next_support(
        &mut self,
        projected: Option<AtomPattern>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'_>>, FormulaFailure> {
        let frame = self.next_selected(
            Ownership::Lend,
            projected,
            budget,
            Context::new(computation, limits, counters, location),
        )?;
        Ok(frame.map(|frame| frame.into_binding(&self.values)))
    }
    /// Select complete rows before borrowing their frame, so rejected rows can
    /// resume traversal without extending a borrow across the mutation loop.
    fn next_selected(
        &mut self,
        ownership: Ownership,
        projected: Option<AtomPattern>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Frame>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        while let Some(row) = self.next_staged(
            ownership,
            projected,
            budget,
            Context::new(computation, limits, counters, location),
        )? {
            if row.passes {
                return Ok(Some(row.frame));
            }
        }
        Ok(None)
    }
    /// Body data and scalar/range filters complete before head-only generation.
    /// Negative gates and aggregate truth remain formulas, never row selection.
    fn next_staged(
        &mut self,
        ownership: Ownership,
        projected: Option<AtomPattern>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Staged>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        // Generators own their input beyond this traversal step. The same
        // consumer row carries their owned output; only unextended rows lend.
        let ownership = if self.generated || !self.head_slots.is_empty() {
            Ownership::Own
        } else {
            ownership
        };
        loop {
            if let Some(pending) = &mut self.pending_head {
                match pending.next(
                    &mut self.evaluation,
                    computation,
                    limits,
                    budget,
                    counters,
                    location,
                ) {
                    Ok(Some(values)) => {
                        self.family.defined = true;
                        return Ok(Some(Staged {
                            frame: Frame::Owned(values),
                            passes: true,
                        }));
                    }
                    Ok(None) => {}
                    Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. }))
                        if self.evaluation.zero_divisor() =>
                    {
                        self.family.zero.get_or_insert(error);
                        pending.reject(location)?;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            self.pending_head = None;
            self.lease.observe(self.storage_bytes(), location)?;
            let Some(row) = self.next_inner(
                ownership,
                projected,
                budget,
                Context::new(computation, limits, counters, location),
            )?
            else {
                return Ok(None);
            };
            if !row.passes || self.head_slots.is_empty() {
                self.family.defined = true;
                return Ok(Some(row));
            }
            self.pending_head = Some(crate::formula_binding_cursor::Cursor::new(
                self.literals,
                row.frame.into_owned(),
                self.support,
                self.bindings,
                self.head_slots.clone(),
                Context::new(computation, limits, counters, location),
            )?);
            self.lease.observe(self.storage_bytes(), location)?;
        }
    }

    fn selected_row(&mut self, frame: Frame, selection: filters::Selection) -> Option<Staged> {
        match selection {
            filters::Selection::Defined(passes) => Some(Staged { frame, passes }),
            filters::Selection::Excluded => None,
            filters::Selection::Zero(error) => {
                self.family.zero.get_or_insert(error);
                None
            }
        }
    }

    fn next_inner(
        &mut self,
        ownership: Ownership,
        projected: Option<AtomPattern>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Staged>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        if !self.generated {
            return self.next_plain(
                ownership,
                projected,
                budget,
                Context::new(computation, limits, counters, location),
            );
        }
        loop {
            if let Some(pending) = &mut self.pending {
                match pending.next(
                    &mut self.evaluation,
                    computation,
                    limits,
                    budget,
                    counters,
                    location,
                ) {
                    Ok(Some(binding)) => {
                        let binding = Frame::Owned(binding);
                        let selection = self.filters(
                            &binding,
                            self.comparisons.clone(),
                            computation,
                            limits,
                            counters,
                            location,
                        )?;
                        if let Some(row) = self.selected_row(binding, selection) {
                            return Ok(Some(row));
                        }
                        continue;
                    }
                    Ok(None) => {}
                    Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. })) => {
                        let zero = self.evaluation.zero_divisor();
                        let excluded = filters::excludes(
                            (self.literals, &self.plan.decisions),
                            &mut self.evaluation,
                            pending.binding(),
                            computation,
                            limits,
                            counters,
                            location,
                        )?;
                        pending.reject(location)?;
                        if !excluded {
                            if !zero {
                                return Err(error.into());
                            }
                            self.family.zero.get_or_insert(error);
                        }
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            let Some(binding) = self.next_base(
                Ownership::Own,
                projected,
                budget,
                Context::new(computation, limits, counters, location),
            )?
            else {
                return Ok(None);
            };
            self.pending = Some(crate::formula_binding_cursor::Cursor::new(
                self.literals,
                binding.into_owned(),
                self.support,
                self.bindings,
                0..self.head_slots.start,
                Context::new(computation, limits, counters, location),
            )?);
            self.lease.observe(self.storage_bytes(), location)?;
        }
    }
    /// A base-only join consumes its complete-row comparison certificate once.
    fn next_plain(
        &mut self,
        ownership: Ownership,
        projected: Option<AtomPattern>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Staged>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        loop {
            let Some(binding) = self.next_base(
                ownership,
                projected,
                budget,
                Context::new(computation, limits, counters, location),
            )?
            else {
                return Ok(None);
            };
            let comparisons = std::mem::replace(&mut self.comparisons, Comparisons::Deferred);
            let selection = self.filters(
                &binding,
                comparisons,
                computation,
                limits,
                counters,
                location,
            )?;
            if let Some(row) = self.selected_row(binding, selection) {
                return Ok(Some(row));
            }
        }
    }
    fn next_base(
        &mut self,
        ownership: Ownership,
        projected: Option<AtomPattern>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Option<Frame>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        self.resume(limits, counters, location)?;
        self.comparisons = Comparisons::Deferred;
        if self.traversal == Traversal::Finished {
            return Ok(None);
        }
        loop {
            counters.work(limits, location)?;
            if self.skip_derived(projected, computation, limits, counters, location)? {
                if self.traversal == Traversal::Finished {
                    return Ok(None);
                }
                continue;
            }
            if self.plan.patterns.is_empty() {
                if self.traversal == Traversal::EmptyVisited {
                    self.traversal = Traversal::Finished;
                    return Ok(None);
                }
                self.traversal = Traversal::EmptyVisited;
                if !self.filter_prefix(computation, limits, counters, location)? {
                    continue;
                }
                self.comparisons = self.certificate();
                return self
                    .complete(ownership, computation, limits, counters, location)
                    .map(Some);
            }
            if self.depth == self.plan.patterns.len() {
                self.comparisons = self.certificate();
                return self
                    .complete(ownership, computation, limits, counters, location)
                    .map(Some);
            }
            let pattern = self.plan.patterns[self.depth];
            self.prepare_probe(
                pattern,
                Context::new(computation, limits, counters, location),
            )?;
            let row = self.probes[self.depth]
                .as_ref()
                .expect("prepared probe")
                .next(&mut self.positions[self.depth], limits, counters, location)?;
            let atom = row.and_then(|row| self.resolutions[self.depth].rows()?.row(row));
            let Some(atom) = atom else {
                self.positions[self.depth] = 0;
                self.probes[self.depth] = None;
                if self.depth == 0 {
                    self.traversal = Traversal::Finished;
                    return Ok(None);
                }
                self.undo_at(self.depth - 1, limits, counters, location)?;
                self.depth -= 1;
                continue;
            };
            counters.record(Event::JoinRow);
            if !self.permits_row(pattern, atom, limits, counters, location)? {
                continue;
            }
            let matches = self.match_row(
                pattern.pattern,
                atom,
                budget,
                Context::new(computation, limits, counters, location),
            )?;
            if matches && self.filter_prefix(computation, limits, counters, location)? {
                self.depth += 1;
            } else {
                self.undo_at(self.depth, limits, counters, location)?;
            }
        }
    }
    /// Resolve the selected source's probe before its cursor advances.
    fn prepare_probe(
        &mut self,
        pattern: PatternOccurrence<'a>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        // Relation ownership is independent of how the row source was prepared.
        // A preselected probe still needs this snapshot's owner before row access.
        if matches!(self.resolutions[self.depth], Resolution::Unresolved) {
            self.resolutions[self.depth] = Resolution::Resolved(self.support.resolve(
                pattern.atom(),
                limits,
                counters,
                location,
            )?);
        }
        if self.probes[self.depth].is_none() {
            let source = self.resolutions[self.depth].rows();
            let binding = self
                .values
                .view(computation.read(), limits, counters, location)?;
            self.probes[self.depth] = Some(
                if let Some(rows) = self.support.select_at(
                    source,
                    pattern.pattern,
                    binding,
                    limits,
                    counters,
                    location,
                )? {
                    Probe::Table(rows)
                } else {
                    let posting = self.support.probe_at(
                        source,
                        pattern.atom(),
                        binding,
                        limits,
                        counters,
                        location,
                    )?;
                    let total = source.map_or(0, relations::RelationRows::row_count);
                    Probe::Indexed(if self.delta.is_some() {
                        let old = source.map_or(0, relations::RelationRows::old_rows);
                        let range = delta::interval(self.delta, pattern.source, old, total);
                        delta::Rows::within(posting, range, limits, counters, location)?
                    } else {
                        delta::Rows::all(posting, total)
                    })
                },
            );
        }
        Ok(())
    }
    /// External admitted-source selection and ordinary necessary domains meet
    /// at the same pre-binding boundary. Complete arithmetic admission ignores
    /// domains as before; only an explicit filtered consumer has a row filter.
    fn permits_row(
        &self,
        pattern: PatternOccurrence<'_>,
        atom: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        if let Some(filter) = self.row_filter
            && !filter.permits(atom, limits, counters, location)?
        {
            return Ok(false);
        }
        if self.coverage == Coverage::Selected
            && let Some(domains) = self.domains
        {
            return domains.permits(pattern.source, atom.position(), limits, counters, location);
        }
        Ok(true)
    }
    fn match_row(
        &mut self,
        pattern: PositivePattern<'_>,
        atom: zetesis_core::relation::Row<'_, '_>,
        budget: &mut Budget,
        context: Context<'_, &mut Computation<'_, '_>>,
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
        if let PositivePattern::Structural(pattern) = pattern {
            let view = self
                .values
                .view(computation.read(), limits, counters, location)?;
            let delta = pattern.matches(
                atom,
                view,
                computation,
                &mut crate::formula_pattern::MatchContext {
                    work: GroundingWork::new(limits, counters, location),
                    budget,
                },
            )?;
            let Some(delta) = delta else {
                return Ok(false);
            };
            self.reserve_trail(delta.len(), computation, limits, counters, location)?;
            for (slot, value) in delta {
                let key = computation.read().term_key(value).map_err(|error| {
                    crate::formula_binding::assignment(
                        zetesis_core::catalog::AssignmentError::Read(error),
                        location,
                    )
                })?;
                self.values.set(slot, &key, limits, counters, location)?;
                self.changes[self.depth].push(slot);
            }
            return Ok(true);
        }
        let terms = pattern.atom().terms();
        for column in 0..terms.len() {
            counters.work(limits, location)?;
            let term = terms.at(column).expect("checked pattern arity");
            let value = atom.value(column).expect("checked pattern arity");
            let expected = match term {
                TemplateTerm::Constant(value) => Some(value),
                TemplateTerm::Variable(variable) if self.values.is_bound(variable, location)? => {
                    Some(self.values.read(variable, computation.read(), location)?)
                }
                TemplateTerm::Variable(variable) => {
                    let key = computation.read().term_key(value).map_err(|error| {
                        crate::formula_binding::assignment(
                            zetesis_core::catalog::AssignmentError::Read(error),
                            location,
                        )
                    })?;
                    self.reserve_trail(1, computation, limits, counters, location)?;
                    self.values
                        .set(variable, &key, limits, counters, location)?;
                    self.changes[self.depth].push(variable);
                    None
                }
            };
            if let Some(expected) = expected
                && !expected.equals_ref_with(value, || counters.work(limits, location))?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// Evaluate the comparisons the current depth decides, once, and fold
    /// them into the prefix's verdict. A comparison that is defined and false
    /// excludes every substitution of the prefix, so the prefix is pruned and
    /// nothing beneath it is reached; an evaluation failure is retained, and
    /// becomes a refusal only if no comparison of the complete substitution
    /// excludes it.
    fn filter_prefix(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        if self.coverage == Coverage::Complete {
            return Ok(true);
        }
        let depth = self.depth;
        let mut passes = depth == 0 || self.verdicts[depth - 1];
        for index in self.plan.decisions.decided_at(depth) {
            let (left, relation, right) =
                comparison(&self.literals[index]).expect("a decided literal is a comparison");
            let values = self.evaluation.source_values(
                [left, right],
                |variable| self.values.key(variable, location),
                computation,
                limits,
                counters,
                location,
            );
            match values {
                Ok([left, right]) => {
                    let read = computation.read();
                    let left = read.term(&left).map_err(|error| {
                        crate::formula_binding::assignment(
                            zetesis_core::catalog::AssignmentError::Read(error),
                            location,
                        )
                    })?;
                    let right = read.term(&right).map_err(|error| {
                        crate::formula_binding::assignment(
                            zetesis_core::catalog::AssignmentError::Read(error),
                            location,
                        )
                    })?;
                    passes &= compare(left, relation, right, limits, counters, location)?;
                }
                Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. })) => {
                    // Retained, not raised: a comparison decided here or
                    // deeper may still exclude the substitution.
                    if self.failure.is_none() {
                        self.failure = Some((depth, error));
                    }
                }
                Err(error) => return Err(error),
            }
        }
        self.verdicts[depth] = passes;
        Ok(passes)
    }
    /// The certificate of the completed prefix: a retained failure, the
    /// verdict when every comparison is decided, or deferral. A prefix
    /// advances only when its comparisons pass, so a completed one passed
    /// them all.
    fn certificate(&self) -> Comparisons {
        if let Some((_, error)) = &self.failure {
            return Comparisons::Failed(error.clone());
        }
        let depth = self.plan.patterns.len().saturating_sub(1);
        if self.plan.decisions.certifies(depth) {
            Comparisons::Verified
        } else {
            Comparisons::Deferred
        }
    }
    fn undo_at(
        &mut self,
        depth: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.values
            .clear_trail(&mut self.changes[depth], limits, counters, location)?;
        if self.failure.as_ref().is_some_and(|(at, _)| *at >= depth) {
            self.failure = None;
        }
        Ok(())
    }
    fn skip_derived(
        &mut self,
        projected: Option<AtomPattern>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        if !self.already_derived(projected, computation, limits, counters, location)? {
            return Ok(false);
        }
        if self.depth == 0 {
            self.traversal = Traversal::Finished;
        } else {
            if self.depth < self.plan.patterns.len() {
                self.positions[self.depth] = 0;
                self.probes[self.depth] = None;
            }
            self.undo_at(self.depth - 1, limits, counters, location)?;
            self.depth -= 1;
        }
        Ok(true)
    }
    fn already_derived(
        &self,
        projected: Option<AtomPattern>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let Some(head) = projected else {
            return Ok(false);
        };
        let head = computation.static_pattern(head, limits, counters, location)?;
        for variable in head.terms().variables() {
            counters.work(limits, location)?;
            if variable >= self.values.len() || !self.values.is_bound(variable, location)? {
                return Ok(false);
            }
        }
        computation.contains_pattern(head, &self.values, limits, counters, location)
    }
    /// Materialization, when requested, remains before complete-row filters.
    /// Its failure leaves the same unfinished depth and counters as before.
    fn complete(
        &mut self,
        ownership: Ownership,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Frame, FormulaFailure> {
        counters.substitution(limits, location)?;
        for (variable, slot) in self.slots.iter().enumerate() {
            if !self.values.is_bound(variable, location)? && matches!(slot, Slot::Relational) {
                return Err(FormulaFailure::UnsafeVariable { variable, location });
            }
        }
        let frame = match ownership {
            Ownership::Own => {
                let values = self
                    .values
                    .copied(computation, limits, counters, location)?;
                counters.record(Event::BindingSnapshot);
                Frame::Owned(values)
            }
            Ownership::Lend => {
                // The same admitted slot span is inspected for a lent frame.
                // No scalar payload is copied, and no snapshot is recorded.
                for _ in 0..self.values.len() {
                    counters.work(limits, location)?;
                }
                Frame::Current
            }
        };
        if !self.plan.patterns.is_empty() {
            self.traversal = Traversal::PendingUndo;
        }
        if matches!(ownership, Ownership::Own) {
            self.resume(limits, counters, location)?;
        }
        Ok(frame)
    }

    /// Exactly one undo follows a successfully completed nonempty join. Owned
    /// snapshots resume immediately; a lent row resumes on its next advance.
    fn resume(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if self.traversal == Traversal::PendingUndo {
            self.undo_at(self.depth - 1, limits, counters, location)?;
            self.depth -= 1;
            self.traversal = Traversal::Searching;
        }
        Ok(())
    }
}

/// Captured arguments reuse comparison evaluation, never binding inference.
fn comparison(literal: &LiteralIr) -> Option<(&Expression, Relation, &Expression)> {
    match literal {
        LiteralIr::Compare(left, relation, right) => Some((left, *relation, right)),
        LiteralIr::ArgumentCheck { captured, value } => Some((captured, Relation::Eq, value)),
        _ => None,
    }
}

pub(crate) fn expression(
    expression: &Expression,
    assignment: &Binding,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<zetesis_core::catalog::TermKey, FormulaFailure> {
    Evaluation::default().source_expression(
        expression,
        |variable| assignment.key(variable, location),
        computation,
        limits,
        counters,
        location,
    )
}

pub(super) fn compare(
    left: TermRef<'_>,
    relation: Relation,
    right: TermRef<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    let order = left.compare_terms_with(right, || counters.work(limits, location))?;
    Ok(match relation {
        Relation::Eq => order.is_eq(),
        Relation::Neq => !order.is_eq(),
        Relation::Lt => order.is_lt(),
        Relation::Le => !order.is_gt(),
        Relation::Gt => order.is_gt(),
        Relation::Ge => !order.is_lt(),
    })
}

pub(super) fn undefined(location: Location) -> FormulaFailure {
    ExpansionFailure::Evaluation {
        error: EvalError::Undefined,
        location,
    }
    .into()
}
