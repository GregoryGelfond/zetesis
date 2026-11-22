//! A finite support upper bound and complete iterative relational joins.

mod computation;
mod context;
mod evaluation;
#[cfg(test)]
pub(crate) mod testing;
pub(crate) use context::{Context, GroundingWork};
pub(crate) mod components;
mod publication;
mod scalar_selection;
mod selection;
pub(crate) mod sort;
mod storage;
pub(crate) use computation::Computation;
pub(crate) use publication::Publication;
pub(crate) use selection::{FactAtoms, SourceBatch, SourceSelection};
pub(crate) use storage::{
    StorageLease, growth_capacity, growth_target, reserve, reserve_exact, reserve_exact_scoped,
};
mod accounting;
mod expression_keys;
mod generated;
mod head_run;
mod heads;
mod integer_comparisons;
pub(crate) use heads::RowHead;
mod observation;
mod term_selection;
mod term_table;
pub(crate) mod witnesses;
pub(crate) use term_table::TermTable;
mod buffer;
mod structural;
pub(crate) use buffer::Buffer;
pub(crate) use term_selection::TermSelection;
#[cfg(test)]
mod captures;
pub(crate) mod certain;
#[cfg(test)]
mod columnar;
#[cfg(test)]
mod continuations;
mod delta;
mod demand;
mod factoring;
pub(crate) mod facts;
pub(crate) mod family;
mod filters;
mod keys;
#[cfg(test)]
mod membership;
mod multiples;
#[cfg(test)]
mod numeric_domains_tests;
mod occurrences;
mod range_selection;
pub(crate) use range_selection::Prepared as RangeSelection;
#[cfg(test)]
pub(crate) use range_selection::selection_enabled as range_selection_enabled;
mod order;
pub(crate) use order::comparison_depth;
#[cfg(test)]
mod postings;
mod prepared;
pub(crate) mod component_plan;
mod probe_selection;
mod producers;
mod parallel_executor;
#[cfg(test)]
pub(crate) use parallel_executor::with_source_workers;
pub(crate) use parallel_executor::{Executor, SourceExecution};
pub(crate) mod row_blocks;
mod expression_memos;
pub(crate) use expression_memos::supports_column_totality;
mod queries;
mod relations;
mod rows;
pub(crate) use factoring::Continuations;

mod local_plans;
use local_plans::Plan as JoinPlan;
pub(crate) use local_plans::{Certificate as LocalPlanCertificate, Ir};

use crate::ProgramSite;
use components::Pattern as AtomPattern;
use relations::SupportAppend;
use themelios_program::program::{DefaultNegation, Relation};
use themelios_program::term::EvalError;
use zetesis_core::catalog::{Atoms, PredicateRef, TermRef};
use zetesis_core::{PatternRef, TemplateTerm};

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Prepared};
use crate::grounding_observer::{Event, Work};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource, GroundingOperator};
use rows::{Advance, Frame, Ownership, Staged};

pub(crate) use accounting::{Accounting, AccountingBaseline};
pub(crate) use component_plan::{ComponentPlans, ComponentScope};
pub(crate) use evaluation::{Evaluation, Failures};
pub(crate) use prepared::PreparedRule;
pub(crate) use producers::AcyclicPlan;
pub(crate) use queries::{Candidates, Support};
#[cfg(test)]
use relations::RelationRows;
pub(crate) use relations::{
    ClosedSource, Relations, SourceAtom, SourceScope, SupportCatalog, atom_failure, owner_limits,
};
pub(crate) use rows::{BindRestriction, PositiveRows, RestrictedRows, RowSelection};

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
    lookup_owner: Option<&'source zetesis_core::atom_registry::AtomRegistry>,
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
        location: ProgramSite,
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
}

/// Completed support kept for streamed constraints over the closed base: the
/// closed canonical storage, the relations the constraints read, and a
/// read-only descendant writer that resolves the base's terms. The relations
/// are read through the closed catalog, which keeps their writer's scopes; the
/// descendant serves only vocabulary-scoped term lookups.
pub(crate) struct StreamedSupport {
    /// Shared with a terminal owner's reconstruction plan when this support is
    /// a hybrid base's.
    closed: std::sync::Arc<ClosedSource>,
    rows: relations::StreamedRows,
    lookup: zetesis_core::atom_registry::AtomRegistry,
    completion: Completion,
}

impl CompletedCatalog {
    /// Close this support, keeping only the relations of the predicates the
    /// atoms of `rules` name. The close is charged and its peak admitted as any
    /// close is.
    pub(crate) fn into_streamed(
        self,
        rules: &[crate::formula_ir::RuleIr],
        work: GroundingWork<'_>,
    ) -> Result<StreamedSupport, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let keep = match self.catalog.component_view(limits, counters, location)? {
            Some(view) => demand::read_predicates(rules, view, limits, counters, location)?,
            None => None,
        };
        let max_bytes = limits.max_atom_storage_bytes;
        let (closed, rows) = self
            .catalog
            .into_streamed(
                keep.as_ref(),
                GroundingWork::new(limits, counters, location),
            )
            .map_err(|failure| failure.into_parts().0)?;
        let lookup = zetesis_core::atom_registry::AtomRegistry::for_closed_catalog(
            &closed.storage,
            max_bytes,
        )
        .map_err(|error| FormulaFailure::AtomCatalog { error, location })?;
        Ok(StreamedSupport {
            closed: std::sync::Arc::new(closed),
            rows,
            lookup,
            completion: self.completion,
        })
    }
}

impl StreamedSupport {
    /// The closed base these relations index.
    pub(crate) fn closed(&self) -> &std::sync::Arc<ClosedSource> {
        &self.closed
    }

    /// Named bytes of the kept relations and their postings.
    pub(crate) fn relation_bytes(&self) -> usize {
        self.rows.bytes()
    }

    /// A completed view for an independent checker; nothing is copied.
    pub(crate) fn snapshot(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<CompletedSupport<'_>, FormulaFailure> {
        self.rows
            .snapshot(&self.closed, limits, counters, location)
            .map(|relations| CompletedSupport {
                lookup_owner: Some(&self.lookup),
                completion: &self.completion,
                relations,
            })
    }
}

#[cfg(test)]
impl CompletedCatalog {
    /// A completed view over the open owner, which tests read directly;
    /// production checkers read the closed base (`StreamedSupport`).
    pub(crate) fn snapshot(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        location: ProgramSite,
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
        location: ProgramSite,
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
    lookup_owner: Option<&'source zetesis_core::atom_registry::AtomRegistry>,
    completion: &'source Completion,
    support: Support<'source>,
}
impl<'source> CompletedQueries<'source> {
    /// Independent checking resolves only terms already visited during complete
    /// original-source admission. A growing query needs its explicit appender.
    pub(crate) fn computation(
        &self,
        location: ProgramSite,
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
    control: Option<accounting::AccountControl>,
    observed: Work,
}
impl Counters {
    pub(crate) fn cancellation(&self) -> Option<&zetesis_algebra::Cancellation> {
        self.control
            .as_ref()
            .and_then(accounting::AccountControl::cancellation)
    }

    pub(crate) fn resume(accounting: Accounting, observed: Work) -> Self {
        Self {
            accounting,
            observed,
            control: None,
        }
    }
    pub(super) fn record(&self, event: Event) {
        self.observed.record(event);
    }
    #[inline]
    pub(crate) fn work(
        &mut self,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.charge_work(1, limits, location)
    }
    #[inline]
    pub(super) fn charge_work(
        &mut self,
        amount: u128,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        charge_work(
            &mut self.accounting.work,
            self.control.as_ref(),
            self.accounting.allowance.as_mut(),
            amount,
            limits,
            location,
        )
    }
    /// The first half of a charge: apply the cadence and check `amount`
    /// further units against the work ceiling, recording and counting
    /// nothing. Work counted later through [`Self::count_work`], up to
    /// `amount`, stays under the ceiling this check admitted.
    #[inline]
    pub(super) fn check_work(
        &self,
        amount: u128,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        check_work(
            self.accounting.work,
            self.control.as_ref(),
            amount,
            limits,
            location,
        )
    }
    /// The second half of a charge: record the local pending receipt and
    /// count `amount` units, which a preceding [`Self::check_work`] for at
    /// least `amount` admitted.
    #[inline]
    pub(super) fn count_work(&mut self, amount: u64) {
        count_work(
            &mut self.accounting.work,
            self.accounting.allowance.as_mut(),
            amount,
        );
    }
    pub(super) fn generated(
        &mut self,
        value: &zetesis_core::catalog::TermKey,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        location: ProgramSite,
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
                    self.control.as_ref(),
                    self.accounting.allowance.as_mut(),
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
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Substitutions,
            u128::from(self.accounting.substitutions) + 1,
            u128::from(limits.max_substitutions),
            location,
        )?;
        if let Some(allowance) = &mut self.accounting.allowance {
            allowance.substitution();
        }
        self.accounting.substitutions += 1;
        Ok(())
    }
}

/// Admit `amount` units before their effect: observe at a charged-range boundary
/// or before a work refusal, check the ceiling, record the receipt, then count.
/// Zero admits no units and never observes; use `Counters::poll` explicitly. Every charged unit runs
/// on this path, so its admission inlines into its callers; the site is read
/// only by the cold decision that observes control or builds a failure.
#[inline]
fn charge_work(
    work: &mut u64,
    control: Option<&accounting::AccountControl>,
    allowance: Option<&mut crate::constraint_allowance::Pending>,
    amount: u128,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    check_work(*work, control, amount, limits, location)?;
    count_work(
        work,
        allowance,
        u64::try_from(amount).expect("charged work fits its u64 ceiling"),
    );
    Ok(())
}

/// Check the charged range and ceiling. A refused charge observes first; an
/// admitted range observes only when it contains a cadence boundary.
///
/// Nearly every charge is a machine-width amount that stays under the ceiling
/// and whose range holds no cadence boundary. That charge is admitted with no
/// observation and builds no failure, so it is decided here, inline in its
/// caller, by one addition and two comparisons. Every other charge takes
/// [`check_observed_work`], which decides all charges alike: the two agree
/// wherever this test admits.
#[inline]
fn check_work(
    work: u64,
    control: Option<&accounting::AccountControl>,
    amount: u128,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    if let Ok(amount) = u64::try_from(amount)
        && let Some(observed) = work.checked_add(amount)
        && observed <= limits.max_work
        && !accounting::charge_observes(control, work, amount)
    {
        return Ok(());
    }
    check_observed_work(work, control, amount, limits, location)
}

/// The complete decision of a charge: one that observes control, reaches the
/// ceiling or needs the wide check. Marked cold, which keeps it off the fast
/// path of callers that carry the inline admission of [`check_work`]; the
/// attribute does not itself forbid inlining.
#[cold]
fn check_observed_work(
    work: u64,
    control: Option<&accounting::AccountControl>,
    amount: u128,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    let admission = zetesis_algebra::Meter::check_receipt(work, amount, limits.max_work);
    if let Some(control) = control {
        let observation = if admission.is_err() {
            control.observe()
        } else {
            control.observe_charge(work, u64::try_from(amount).expect("admitted work fits u64"))
        };
        observation.map_err(|reason| interrupted(reason, location))?;
    }
    admission
        .map(|_| ())
        .map_err(|error| work_failure(error, limits.max_work, location))
}

/// Preserve the source account's saturated attempted-demand diagnostic.
fn work_failure(
    error: zetesis_algebra::MeterWorkError,
    limit: u64,
    location: ProgramSite,
) -> FormulaFailure {
    let (observed, limit) = match error {
        zetesis_algebra::MeterWorkError::Limit { observed, limit } => (observed, limit),
        zetesis_algebra::MeterWorkError::Overflow => (u128::MAX, u128::from(limit)),
    };
    FormulaFailure::Limit {
        resource: FormulaResource::Work,
        observed,
        limit,
        location,
    }
}

/// A charge's count: record the local pending receipt, then count `amount`.
#[inline]
fn count_work(
    work: &mut u64,
    allowance: Option<&mut crate::constraint_allowance::Pending>,
    amount: u64,
) {
    if let Some(allowance) = allowance {
        allowance.work(amount);
    }
    *work += amount;
}

/// The failure of a charge stopped by cancellation or a deadline.
#[cold]
#[inline(never)]
fn interrupted(reason: zetesis_algebra::Stop, location: ProgramSite) -> FormulaFailure {
    FormulaFailure::Interrupted { reason, location }
}

#[cfg(test)]
pub(crate) fn build(
    catalog: SupportCatalog,
    prepared: &Prepared,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    fallback: ProgramSite,
) -> Result<CompletedCatalog, FormulaFailure> {
    build_with_executor(
        catalog,
        prepared,
        domains,
        budget,
        GroundingWork::new(limits, counters, fallback),
        None,
    )
}

pub(crate) fn build_with_executor(
    catalog: SupportCatalog,
    prepared: &Prepared,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    budget: &mut Budget,
    work: GroundingWork<'_>,
    executor: Option<&Executor>,
) -> Result<CompletedCatalog, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    counters.observe_work(Event::SupportConstructionWork, |counters| {
        let producers =
            producers::Producers::prepare(prepared, &catalog, limits, counters, location)?;
        complete_with_executor(
            catalog,
            prepared,
            producers,
            domains,
            budget,
            GroundingWork::new(limits, counters, location),
            executor,
        )
    })
}

#[cfg(test)]
fn complete(
    catalog: SupportCatalog,
    prepared: &Prepared,
    producers: producers::Producers<'_>,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    budget: &mut Budget,
    work: GroundingWork<'_>,
) -> Result<CompletedCatalog, FormulaFailure> {
    complete_with_executor(catalog, prepared, producers, domains, budget, work, None)
}

/// Install certainty and posting demand, publish the catalog, and let the
/// first round record each fact row's atom for grounding to read back.
fn prepare_rounds(
    catalog: &mut SupportCatalog,
    prepared: &Prepared,
    producers: &mut producers::Producers<'_>,
    (limits, counters): (&FormulaLimits, &mut Counters),
    fallback: ProgramSite,
) -> Result<(), FormulaFailure> {
    catalog.prepared_bytes(producers.bytes());
    // Certain predicates are read by assignment proposals from the first
    // round on, so they are installed before the first snapshot.
    if certain::every_predicate_assumed() {
        catalog.install_certainty(certain::Certainty::all(), limits, counters, fallback)?;
    } else if !certain::left_out()
        && let Some(certainty) =
            producers.certainty(prepared, catalog, limits, counters, fallback)?
    {
        catalog.install_certainty(certainty, limits, counters, fallback)?;
    }
    // Postings are kept only for columns a join can bind, decided before the
    // first relation is created, since a column cannot gain postings later.
    if let Some(view) = catalog.component_view(limits, counters, fallback)? {
        let demand = demand::Demand::of(prepared, view, limits, counters, fallback)?;
        catalog.install_demand(demand);
    }
    catalog.publish(limits, counters, fallback)?;
    let mut rows = 0_usize;
    for block in prepared.items().blocks() {
        counters.work(limits, block.location())?;
        rows = rows
            .checked_add(block.len())
            .ok_or(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Overflow,
                location: fallback,
            })?;
    }
    if rows > 0 {
        catalog.open_fact_rows(rows, limits, counters, fallback)?;
    } else {
        // Without fact blocks, a cell per fact rule for its head's atom.
        let mut facts = 0_usize;
        for rule in prepared.rules_without_facts() {
            counters.work(limits, rule.location)?;
            facts += usize::from(head_run::fact_head(rule).is_some());
        }
        if facts > 1 {
            catalog.open_head_rows(facts, limits, counters, fallback)?;
        }
    }
    Ok(())
}

fn complete_with_executor(
    mut catalog: SupportCatalog,
    prepared: &Prepared,
    mut producers: producers::Producers<'_>,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    budget: &mut Budget,
    work: GroundingWork<'_>,
    executor: Option<&Executor>,
) -> Result<CompletedCatalog, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location: fallback,
    } = work;
    prepare_rounds(
        &mut catalog,
        prepared,
        &mut producers,
        (limits, counters),
        fallback,
    )?;
    #[cfg(test)]
    postings::begin_support();
    // Persist interned ID columns for the hook across support rounds,
    // and drop them when this completion ends (PRIOR-ART.md).
    let _intern_cache = crate::formula_ground::algebra_join::InternCacheGuard::new();
    let mut rounds = 0_u64;
    // Rows are only appended, so each round extends the kept certificates.
    let mut retained = queries::Retained::default();
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
            let mut schedule = producers.schedule();
            let (mut facts, mut heads) = first_round(rounds, prepared);
            let mut selected = schedule.next(limits, counters, fallback)?;
            if selected.is_none() && producers.is_planned() && facts.is_none() {
                // Every zero-input producer ran at bootstrap. Every subsequent
                // new positive binding needs a changed predicate, whose complete
                // posting would have selected its original producer. Thus this
                // admitted round is unchanged without a new relation snapshot.
                return finish(
                    (catalog, producers, retained),
                    GroundingWork::new(limits, counters, fallback),
                );
            }
            counters.work(limits, fallback)?;
            counters.record(Event::SupportSnapshotPreparation);
            let (relations, mut delta) = counters
                .operator(GroundingOperator::SupportIndex, |counters| {
                    catalog.split(limits, counters, fallback)
                })?;
            let kept = std::mem::take(&mut retained);
            let support = counters.operator(GroundingOperator::SupportIndex, |counters| {
                Support::indexed_retaining(&relations, kept, limits, counters, fallback)
            })?;
            counters.operator(GroundingOperator::SupportJoin, |counters| {
                counters.observe_work(Event::SupportProductionWork, |counters| {
                    while let Some((index, scheduled)) = next_producer(
                        selected,
                        (&mut facts, &mut heads),
                        (&producers, &mut delta, &support),
                        (limits, counters, fallback),
                    )? {
                        visit_producer(
                            (index, scheduled, rounds),
                            (&mut facts, &mut heads),
                            prepared,
                            (&producers, &mut delta, &support, domains),
                            budget,
                            (limits, counters),
                            executor,
                        )?;
                        if !scheduled {
                            selected = schedule.next(limits, counters, fallback)?;
                        }
                    }
                    Ok::<_, FormulaFailure>(())
                })
            })?;
            // No join remains live. Release its query descriptor before the
            // pending selection borrows the catalog's semantic ordering index.
            retained = support.into_retained(fallback)?;
            advance_round(
                &mut delta,
                Some(&mut producers),
                GroundingWork::new(limits, counters, fallback),
                executor,
            )?
        };
        catalog.close_fact_rows();
        if !changed {
            // An unchanged round decides every component no waiting producer
            // still feeds, so the earliest waiting producer was traversed. A
            // producer left waiting would mean heads the support lacks, and
            // every decision read from it: refuse instead of publishing it.
            if producers.pending() {
                return Err(FormulaFailure::SourceActivity { location: fallback });
            }
            return finish(
                (catalog, producers, retained),
                GroundingWork::new(limits, counters, fallback),
            );
        }
        counters.operator(GroundingOperator::SupportPublish, |counters| {
            catalog.publish(limits, counters, fallback)
        })?;
    }
}

/// Run one selected producer of the round: admit the first round's facts or
/// heads before it, then derive its variants into the round's pending atoms.
fn visit_producer<'source>(
    (index, scheduled, rounds): (usize, bool, u64),
    first: (
        &mut Option<facts::FirstRound<'_>>,
        &mut Option<head_run::HeadRun>,
    ),
    prepared: &'source Prepared,
    (producers, delta, support, domains): (
        &producers::Producers<'_>,
        &mut SupportAppend<'source>,
        &Support<'source>,
        Option<&crate::formula_domains::Domains<'_>>,
    ),
    budget: &mut Budget,
    (limits, counters): (&FormulaLimits, &mut Counters),
    executor: Option<&Executor>,
) -> Result<(), FormulaFailure> {
    let rules = prepared.rules_without_facts();
    enter_rule(index, first, rules, (delta, support), (limits, counters))?;
    let rule = &rules[index];
    counters.work(limits, rule.location)?;
    counters.record(Event::SupportProducerVisit);
    let mut variants = if scheduled {
        counters.record(Event::SupportRichTraversal);
        delta::traversal(rule, support, true)
    } else {
        producers.variants(index, rule, support, rounds == 1, limits, counters)?
    };
    produce(
        (index, rule),
        &mut variants,
        (delta, support, domains),
        producers,
        budget,
        (limits, counters),
        executor,
    )
}

/// What the first round admits beside its producers. Rules alone are
/// scheduled: a fact block has no producer slot, and enters support in the
/// first round at its item position. Without fact blocks, the first round
/// admits runs of fact rules of one predicate as head blocks instead.
fn first_round(
    rounds: u64,
    prepared: &Prepared,
) -> (Option<facts::FirstRound<'_>>, Option<head_run::HeadRun>) {
    let facts = (rounds == 1 && prepared.items().blocks().next().is_some())
        .then(|| facts::FirstRound::new(prepared.items()));
    let heads = (rounds == 1 && facts.is_none()).then(head_run::HeadRun::default);
    (facts, heads)
}

/// Admit what the first round holds before the rule `index`: the fact blocks
/// that precede it, or the run of fact rules it opens or continues.
fn enter_rule<'source>(
    index: usize,
    (facts, heads): (
        &mut Option<facts::FirstRound<'_>>,
        &mut Option<head_run::HeadRun>,
    ),
    rules: &'source [crate::formula_ir::RuleIr],
    (delta, support): (&mut SupportAppend<'source>, &Support<'source>),
    (limits, counters): (&FormulaLimits, &mut Counters),
) -> Result<(), FormulaFailure> {
    if let Some(facts) = facts {
        facts.before_rule(index, delta, support, limits, counters)?;
    }
    if let Some(heads) = heads {
        heads.before_rule(index, rules, delta, support, limits, counters)?;
    }
    Ok(())
}

/// The round's next producer: the schedule's, then, once it is exhausted and
/// the facts are in, each producer the component schedule holds whose waited
/// components reached their fixed point, flagged so its caller traverses it
/// in full.
fn next_producer<'source>(
    selected: Option<usize>,
    (facts, heads): (
        &mut Option<facts::FirstRound<'_>>,
        &mut Option<head_run::HeadRun>,
    ),
    (producers, delta, support): (
        &producers::Producers<'_>,
        &mut SupportAppend<'source>,
        &Support<'source>,
    ),
    (limits, counters, location): (&FormulaLimits, &mut Counters, ProgramSite),
) -> Result<Option<(usize, bool)>, FormulaFailure> {
    if let Some(index) = selected {
        return Ok(Some((index, false)));
    }
    if let Some(facts) = facts.take() {
        facts.finish(delta, support, limits, counters)?;
    }
    // A producer the component schedule releases starts no head run.
    if let Some(mut heads) = heads.take() {
        heads.close(delta, support, limits, counters, location)?;
    }
    let ready = match producers.take_ready() {
        Some(index) => Some(index),
        None if producers.mark_ready(delta, limits, counters, location)? => producers.take_ready(),
        None => None,
    };
    Ok(ready.map(|index| (index, true)))
}

/// Derive one producer's variants into this round's pending atoms, under the
/// rule's domain guards and static totality when the analysis supplies them.
fn produce<'source>(
    (index, rule): (usize, &'source crate::formula_ir::RuleIr),
    variants: &mut delta::Variants<'_, '_>,
    (delta, support, domains): (
        &mut SupportAppend<'source>,
        &Support<'source>,
        Option<&crate::formula_domains::Domains<'_>>,
    ),
    producers: &producers::Producers<'_>,
    budget: &mut Budget,
    (limits, counters): (&FormulaLimits, &mut Counters),
    executor: Option<&Executor>,
) -> Result<(), FormulaFailure> {
    let mut computation = Computation::new(delta, support);
    let candidates = domains
        .map(|domains| domains.for_rule(index, rule))
        .transpose()?
        .flatten();
    let totality = domains
        .map(|domains| domains.total_body(index, rule))
        .transpose()?
        .flatten();
    let guards = match candidates {
        Some(candidates) => {
            support.domain_guards(rule, candidates, &computation, limits, counters)?
        }
        None => None,
    };
    derive_variants_with_executor(
        (index, rule),
        variants,
        (guards.as_ref(), totality),
        support,
        Some(producers),
        (budget, executor),
        Context::new(&mut computation, limits, counters, rule.location),
    )
}

/// Prepare the next wake set only after the complete pending population has its
/// publication order. Refusal leaves private round metadata, never completion.
fn advance_round(
    delta: &mut SupportAppend<'_>,
    producers: Option<&mut producers::Producers<'_>>,
    work: GroundingWork<'_>,
    executor: Option<&Executor>,
) -> Result<bool, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    if delta.is_empty() {
        return Ok(false);
    }
    counters.operator(GroundingOperator::SupportDedup, |counters| {
        counters.observe_work(Event::SupportOrderWork, |counters| {
            delta.order(limits, counters, location, executor)
        })
    })?;
    #[cfg(test)]
    facts::rounds::record(delta.atoms());
    if let Some(producers) = producers {
        counters.operator(GroundingOperator::SupportWake, |counters| {
            counters.observe_work(Event::SupportWakeWork, |counters| {
                producers.advance(delta.atoms(), limits, counters, location)
            })
        })?;
    }
    Ok(true)
}

/// The fixed point drops its scheduling metadata and the certificates its
/// rounds kept before final publication.
fn finish(
    (mut catalog, producers, retained): (
        SupportCatalog,
        producers::Producers<'_>,
        queries::Retained,
    ),
    work: GroundingWork<'_>,
) -> Result<CompletedCatalog, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    drop(producers);
    drop(retained);
    catalog.release_preparation();
    counters.operator(GroundingOperator::SupportPublish, |counters| {
        catalog.publish(limits, counters, location)
    })?;
    Ok(CompletedCatalog {
        catalog,
        completion: Completion(std::sync::Arc::new(())),
    })
}

/// What a conditional head's head stage holds under one condition row.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeadOutcome {
    /// An instance of the head is defined there.
    pub(crate) defined: bool,
    /// An instance of the head divides by zero there.
    pub(crate) zero: bool,
}

/// Visit one rule's ordered delta variants under the same admitted guards.
/// `index` is the rule's position among the producers.
#[cfg(test)]
fn derive_variants<'source>(
    (index, rule): (usize, &'source crate::formula_ir::RuleIr),
    variants: &mut delta::Variants<'_, '_>,
    (guards, totality): (
        Option<&queries::Guards<'_, 'source>>,
        Option<&crate::formula_domains::numeric_ir::TotalBody<'_>>,
    ),
    support: &Support<'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<(), FormulaFailure> {
    derive_variants_with_executor(
        (index, rule),
        variants,
        (guards, totality),
        support,
        None,
        (budget, None),
        context,
    )
}

fn derive_variants_with_executor<'source>(
    (index, rule): (usize, &'source crate::formula_ir::RuleIr),
    variants: &mut delta::Variants<'_, '_>,
    (guards, totality): (
        Option<&queries::Guards<'_, 'source>>,
        Option<&crate::formula_domains::numeric_ir::TotalBody<'_>>,
    ),
    support: &Support<'source>,
    producers: Option<&producers::Producers<'_>>,
    (budget, executor): (&mut Budget, Option<&Executor>),
    context: Context<'_, &mut Computation<'_, 'source>>,
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
    // Range-and-arithmetic and atom-bounded complete domains compile into
    // the same columnar Template formula instantiation uses. Completing
    // their heads from retained rows avoids walking failing pairs on the
    // Join cursor. Guards and static totality are Join-cursor certificates;
    // they must not suppress Bind when the compiler admits the body. The
    // compiler still declines free cross-product atoms, constraints, and
    // incomplete or recursive domains.
    if derive_columnar_variants(
        (index, rule),
        variants,
        support,
        (computation, limits, budget, counters),
        producers,
    )? {
        return Ok(());
    }
    // A body-less normal rule without variables has exactly one binding, the
    // empty one, under every variant: no join, selection or witness cursor
    // has anything to visit. Its head is derived from that binding as the
    // join's one row would derive it.
    if rule.body.is_empty()
        && rule.variables == 0
        && let HeadIr::Normal(Some(head)) = &rule.head
    {
        let mut next = variants.next(limits, counters)?;
        while next.is_some() {
            let binding = Binding::new(computation, limits, counters, rule.location)?;
            let mut derivation = Derivation {
                normal_head: None,
                computation: &mut *computation,
                limits,
                budget: &mut *budget,
                counters: &mut *counters,
            };
            derivation.normal_head(*head, &binding, rule.location)?;
            drop(derivation);
            next = variants.next(limits, counters)?;
        }
        return Ok(());
    }
    // A successful map belongs to this exact rule and immutable round. The
    // first cursor retains its ordinary mutable preparation; later variants
    // borrow it only after that cursor has finished and released its state.
    let mut preparation = None;
    let mut next = variants.next(limits, counters)?;
    while let Some(variant) = next {
        if let Some(preparation) = &preparation {
            let mut outer = multiples::RoundPreparation::rows(
                preparation,
                variant,
                guards,
                budget,
                Context::new(computation, limits, counters, rule.location),
            )?;
            outer.select_round_multiples(rule, computation, limits, counters)?;
            if let Some(totality) = totality {
                outer.select_static_total_body(rule, totality, limits, counters)?;
            }
            derive_rule_with_executor(
                (index, rule),
                &mut outer,
                support,
                budget,
                Context::new(computation, limits, counters, rule.location),
                executor,
            )?;
            drop(outer);
            next = variants.next(limits, counters)?;
            continue;
        }
        let mut outer = Join::variant_rule(
            Ir::Source(rule),
            variant,
            support,
            guards,
            budget,
            Context::new(computation, limits, counters, rule.location),
        )?;
        let total = outer.select_round_multiples(rule, computation, limits, counters)?;
        if let Some(totality) = totality {
            outer.select_static_total_body(rule, totality, limits, counters)?;
        }
        derive_rule_with_executor(
            (index, rule),
            &mut outer,
            support,
            budget,
            Context::new(computation, limits, counters, rule.location),
            executor,
        )?;
        // Preserve the next-variant lookup before admitting any new owner. A
        // final or only successful cursor has no reuse consumer.
        next = variants.next(limits, counters)?;
        if total && next.is_some() {
            preparation = Some(multiples::RoundPreparation::retain(
                Ir::Source(rule),
                outer,
                Context::new(computation, limits, counters, rule.location),
            )?);
        }
    }
    Ok(())
}

/// Whether every non-recursive positive body predicate is complete.
///
/// The hook consumes every variant of a visit after one snapshot join;
/// if a partner such as `open/2` can still grow (choice-dependent facts,
/// a later stratum), that join would miss later rows and could drop heads
/// the cursor would still derive — observed as unsat on network-repair
/// and equality-generalized-tsp. Extensional predicates (no schedule
/// component) are already complete. Recursive body atoms of the head stay
/// on the semi-naive path and are not required to be reached. When
/// `producers` is missing (unit-test wrapper), skip the gate so existing
/// tests keep the hook.
fn positive_partners_complete(
    rule: &crate::formula_ir::RuleIr,
    support: &Support<'_>,
    producers: Option<&producers::Producers<'_>>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    use crate::formula_support::producers::PredicateStage;
    let Some(producers) = producers else {
        return Ok(true);
    };
    let HeadIr::Normal(Some(head)) = &rule.head else {
        return Ok(false);
    };
    let Some(view) = support.components() else {
        return Ok(false);
    };
    let Some(head) = head.resolve(view) else {
        return Ok(false);
    };
    let head_predicate = head.predicate();
    for literal in &rule.body {
        let crate::formula_ir::LiteralIr::Atom(
            themelios_program::program::DefaultNegation::None,
            pattern,
        ) = literal
        else {
            continue;
        };
        let Some(pattern) = pattern.resolve(view) else {
            return Ok(false);
        };
        if pattern.predicate() == head_predicate {
            continue;
        }
        match producers.predicate_stage(pattern.predicate(), limits, counters, rule.location)? {
            PredicateStage::Reached | PredicateStage::Extensional => {}
            PredicateStage::Settled | PredicateStage::Open => return Ok(false),
        }
    }
    Ok(true)
}

fn derive_columnar_variants<'source>(
    (index, rule): (usize, &'source crate::formula_ir::RuleIr),
    variants: &mut delta::Variants<'_, '_>,
    support: &Support<'source>,
    (computation, limits, budget, counters): (
        &mut Computation<'_, 'source>,
        &FormulaLimits,
        &mut Budget,
        &mut Counters,
    ),
    producers: Option<&producers::Producers<'_>>,
) -> Result<bool, FormulaFailure> {
    if derive_algebra_variants(
        rule,
        variants,
        support,
        computation,
        limits,
        budget,
        counters,
    )? {
        return Ok(true);
    }
    derive_algebra_join_variants(
        (index, rule),
        variants,
        support,
        (computation, limits, budget, counters),
        producers,
    )
}

/// Derive heads with the columnar binary Join, consuming every variant of
/// this visit after a single snapshot enumeration, matching the Bind hook.
fn derive_algebra_join_variants<'source>(
    (index, rule): (usize, &'source crate::formula_ir::RuleIr),
    variants: &mut delta::Variants<'_, '_>,
    support: &Support<'source>,
    (computation, limits, budget, counters): (
        &mut Computation<'_, 'source>,
        &FormulaLimits,
        &mut Budget,
        &mut Counters,
    ),
    producers: Option<&producers::Producers<'_>>,
) -> Result<bool, FormulaFailure> {
    let HeadIr::Normal(Some(head)) = &rule.head else {
        return Ok(false);
    };
    if !crate::formula_ground::algebra_join::body_repeats_head(rule, support) {
        return Ok(false);
    }
    if !positive_partners_complete(rule, support, producers, limits, counters)? {
        return Ok(false);
    }
    // N1 and N2 both gold-diff clean vs 9a63, but both regress stratrec
    // t14 (N1 wall 1.117× / ground 1.261×; N2 1.134× / 1.351×) once the
    // hook admits `not`. Neither is kept. The hook declines default
    // negation; the partner-completeness gate above stays.
    let _ = index;
    let allow_negation = false;
    let pending: Vec<usize> = Vec::new();
    // The hook evaluates serially at every thread count. A partitioned
    // delta charged a different receipt at tN than at t1 (sg1k, closure10k
    // and agg10k by up to 3×), and running its fixed task blocks inline at
    // t1 cost 1.1–1.3× wall there; one evaluation keeps the receipt and the
    // t1 win.
    let Some(evaluated) = crate::formula_ground::algebra_join::evaluate(
        rule,
        support,
        limits,
        counters,
        rule.location,
        allow_negation,
        pending.as_slice(),
    )?
    else {
        return Ok(false);
    };
    let mut next = variants.next(limits, counters)?;
    let mut derivation = Derivation {
        normal_head: None,
        computation,
        limits,
        budget,
        counters,
    };
    let mut enumerated = false;
    while next.is_some() {
        if !enumerated {
            let mut assignment = Binding::new(
                derivation.computation,
                limits,
                derivation.counters,
                rule.location,
            )?;
            assignment.extend_scope(
                rule.variables,
                derivation.computation,
                limits,
                derivation.counters,
                rule.location,
            )?;
            let mut buffer = vec![0i32; evaluated.bindings.width()];
            let rows = evaluated.emission_order(rule, support);
            for row in rows {
                evaluated.bind_row(
                    (
                        derivation.computation,
                        limits,
                        derivation.counters,
                        rule.location,
                    ),
                    &mut assignment,
                    row,
                    &mut buffer,
                )?;
                derivation.normal_head(*head, &assignment, rule.location)?;
            }
            enumerated = true;
        }
        next = variants.next(limits, derivation.counters)?;
    }
    Ok(true)
}

/// Derive a range-and-arithmetic producer's heads with the columnar operators.
///
/// Returns `Ok(true)` when the rule compiled and every variant of this visit
/// was consumed. Enumeration runs once for the visit, including a delta
/// visit whose body domains are complete in the snapshot; later variants of
/// the same visit do not repeat it. Later rounds compile the same way and
/// emit nothing when no variant remains. Rules the compiler declines stay
/// on the Join cursor.
fn derive_algebra_variants<'source>(
    rule: &'source crate::formula_ir::RuleIr,
    variants: &mut delta::Variants<'_, '_>,
    support: &Support<'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    let Some((template, width)) =
        crate::formula_ground::algebra_rows::compile(rule, computation, limits, counters, support)?
    else {
        return Ok(false);
    };
    let HeadIr::Normal(Some(head)) = &rule.head else {
        return Ok(false);
    };
    let mut next = variants.next(limits, counters)?;
    let mut derivation = Derivation {
        normal_head: None,
        computation,
        limits,
        budget,
        counters,
    };
    let mut enumerated = false;
    while next.is_some() {
        if !enumerated {
            let bindings = crate::formula_ground::algebra_rows::enumerate(
                &template,
                limits,
                derivation.counters,
                rule.location,
            )?;
            let mut assignment = Binding::new(
                derivation.computation,
                limits,
                derivation.counters,
                rule.location,
            )?;
            assignment.extend_scope(
                rule.variables,
                derivation.computation,
                limits,
                derivation.counters,
                rule.location,
            )?;
            let mut buffer = vec![0i32; width];
            for row in bindings.row_range() {
                bindings.row_into(row, &mut buffer);
                crate::formula_ground::algebra_rows::bind_row(
                    derivation.computation,
                    limits,
                    derivation.counters,
                    &mut assignment,
                    &buffer,
                    rule.location,
                )?;
                derivation.normal_head(*head, &assignment, rule.location)?;
            }
            enumerated = true;
        }
        next = variants.next(limits, derivation.counters)?;
    }
    Ok(true)
}

/// Derive the heads of every complete row of `outer`, a join of the rule at
/// position `index` among the producers. A row under which a decided
/// default-negated atom fails derives nothing
/// ([`Derivation::negations_hold`]).
#[cfg(test)]
fn derive_rule<'source>(
    (index, rule): (usize, &'source crate::formula_ir::RuleIr),
    outer: &mut Join<'_, 'source>,
    support: &Support<'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    derive_rule_with_executor(
        (index, rule),
        outer,
        support,
        budget,
        Context::new(computation, limits, counters, rule.location),
        None,
    )
}

fn derive_rule_with_executor<'source>(
    (index, rule): (usize, &'source crate::formula_ir::RuleIr),
    outer: &mut Join<'_, 'source>,
    support: &Support<'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
    executor: Option<&Executor>,
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
    // Every cursor belongs to this immutable round, including each first-new
    // variant. Family-evidence and external-check cursors never enter here.
    outer.select_scalar_body(rule, computation, limits, counters)?;
    outer.select_numeric_probes(rule, limits, counters)?;
    if let Some(executor) = executor
        && row_blocks::coordinator::derive(
            executor,
            rule,
            outer,
            support,
            budget,
            Context::new(computation, limits, counters, location),
        )?
    {
        return Ok(());
    }
    if witnesses::derive(rule, outer, computation, limits, budget, counters)? {
        return Ok(());
    }
    let mut derivation = Derivation {
        normal_head: None,
        computation,
        limits,
        budget,
        counters,
    };
    // A body-less rule has one binding, so asking first whether its head is
    // already derived saves nothing: deriving it again finds the same atom.
    let projected = match &rule.head {
        HeadIr::Normal(head) if !rule.body.is_empty() => *head,
        _ => None,
    };
    let certainty = support.certainty();
    while let Some(binding) = derivation.next(outer, projected, rule.location)? {
        if let Some(certainty) = certainty
            && !derivation.negations_hold(rule, certainty.decided(index), &binding)?
        {
            continue;
        }
        match &rule.head {
            HeadIr::Normal(Some(head)) => {
                derivation.normal_head(*head, &binding, rule.location)?;
            }
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
                        Ir::Source(element.condition.as_slice()),
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

/// Whether every default-negated atom of `rule` at the body positions
/// `decided` has the value its literal requires under `binding`: `not a`
/// requires `a` outside the support `computation` reads, `not not a` inside.
/// The positions are those [`certain::Certainty::decided`] lists for the
/// rule, so each atom's predicate is certain and its relation complete: the
/// atom holds in every answer set when it is supported and in none otherwise.
/// Support completion and source activity both ask here, so they omit the
/// same rows. One unit per atom tested, with the pattern's read and the
/// support lookup.
pub(crate) fn decided_negations_hold(
    rule: &crate::formula_ir::RuleIr,
    decided: impl Iterator<Item = usize>,
    binding: &Binding<'_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    for occurrence in decided {
        let LiteralIr::Atom(negation, pattern) = &rule.body[occurrence] else {
            unreachable!("a decided occurrence is a default-negated flat atom");
        };
        counters.work(limits, rule.location)?;
        let pattern = computation.static_pattern(*pattern, limits, counters, rule.location)?;
        let supported =
            computation.contains_pattern(pattern, binding, limits, counters, rule.location)?;
        if supported != (*negation == DefaultNegation::NotNot) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A support round publishes newly derived heads after complete checked binding.
/// Existing support and the current delta share one borrowed atom identity.
struct Derivation<'a, 'source, 'round> {
    normal_head: Option<heads::Head<'round>>,
    computation: &'a mut Computation<'source, 'round>,
    limits: &'a FormulaLimits,
    budget: &'a mut Budget,
    counters: &'a mut Counters,
}

impl Derivation<'_, '_, '_> {
    /// The one normal head is prepared only after a complete selected binding.
    /// It borrows this round's immutable template, never a generated row.
    fn normal_head(
        &mut self,
        pattern: AtomPattern,
        binding: &Binding<'_>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.counters
            .observe_work(Event::SupportHeadWork, |counters| {
                if self.normal_head.is_none() {
                    self.normal_head = Some(heads::Head::new(
                        pattern,
                        self.computation,
                        self.limits,
                        counters,
                        location,
                    )?);
                }
                self.normal_head
                    .as_ref()
                    .expect("prepared normal head")
                    .derive(binding, self.computation, self.limits, counters, location)
            })
    }

    /// Whether every default-negated atom of `rule` at the body positions
    /// `decided` has the value its literal requires under `binding`. Each
    /// such atom's predicate is certain and its relation complete whenever
    /// this rule is traversed (`certain::Certainty::decided`), so the atom
    /// holds in every answer set when it is supported and in none otherwise:
    /// `not a` requires it unsupported, `not not a` supported. A row that
    /// fails derives no head, as no answer set satisfies its body. One unit
    /// per atom tested, with the pattern's read and the support lookup.
    fn negations_hold(
        &mut self,
        rule: &crate::formula_ir::RuleIr,
        decided: impl Iterator<Item = usize>,
        binding: &Binding<'_>,
    ) -> Result<bool, FormulaFailure> {
        decided_negations_hold(
            rule,
            decided,
            binding,
            self.computation,
            self.limits,
            self.counters,
        )
    }

    /// Select a complete support binding before the consumer borrows its frame.
    /// Attribution observes admitted operations; it adds no work charges.
    fn next<'join>(
        &mut self,
        join: &'join mut Join<'_, '_>,
        projected: Option<AtomPattern>,
        location: ProgramSite,
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

    fn choice<'source>(
        &mut self,
        group: &'source crate::formula_ir::ChoiceIr,
        binding: &Binding,
        support: &Support<'source>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        crate::formula_head_aggregate::validate_group(
            Ir::Source(group),
            binding,
            support,
            self.budget,
            Context::new(self.computation, self.limits, self.counters, location),
        )?;
        for element in &group.elements {
            let mut local = Join::element(
                Ir::Source(element),
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
        location: ProgramSite,
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
/// borrow a necessary positive-row condition; it never supplies family evidence.
pub(crate) struct Join<'a, 'source> {
    bindings: Option<&'a crate::formula_assignment_plan::Plan>,
    scalar_selection: scalar_selection::Preparation,
    literals: Ir<'a, 'source, [LiteralIr]>,
    generated: bool,
    /// The certificate of the last completed row, taken by its consumer.
    comparisons: Comparisons,
    /// The active proposal cursor keeps its own base certificate during lookahead.
    pending_comparisons: Comparisons,
    coverage: Coverage,
    /// Source-family traversal follows original body selection independently
    /// of a complemented comparison head, while still checking its arithmetic.
    source_evidence: bool,
    /// All flat constraint expressions succeeded over completed source domains.
    certified_total: bool,
    /// Explicit rule-instantiation permission for checked expression-key probes.
    expression_keys: Option<expression_keys::Eligible>,
    multiples: Option<multiples::Prepared<'a>>,
    range_selection: Option<RangeSelection<'a>>,
    family: family::Evidence,
    head_bounds: &'a [crate::formula_ir::AggregateGuard],
    checked_guard: Option<&'a crate::formula_guard::Guard>,
    /// What each prefix of the current order decides.
    plan: JoinPlan<'a>,
    /// The conjunction of the comparisons decided up to each depth.
    verdicts: Vec<bool>,
    /// The first evaluation failure on the current prefix and the depth that
    /// met it; released when that depth is undone.
    failure: Option<(usize, EvalError)>,
    evaluation: Evaluation,
    expression_memos: expression_memos::ExpressionMemos<'a>,
    pending: Option<crate::formula_binding_cursor::Cursor<'a, 'source>>,
    pending_head: Option<crate::formula_binding_cursor::Cursor<'a, 'source>>,
    head_slots: std::ops::Range<usize>,
    delta: Option<usize>,
    domains: Option<&'a queries::Guards<'a, 'source>>,
    // Installed only by filtered_rule before traversal; never replaced while
    // its authenticated source slots or held prefix can be retained.
    bind_restriction: Option<&'a dyn BindRestriction>,
    /// An admitted selected query's one original positive occurrence. Its
    /// immutable source row remains resolved across ordinary backtracking.
    anchor: Option<rows::Anchor<'a>>,
    /// Current matched positives are held, except at most one mapped open pivot.
    /// Enabled only for a borrowed non-generated filtered binding; undo
    /// truncates it with that binding. No truth survives a cursor/check.
    positive_prefix: Option<rows::PositivePrefix>,
    families: Option<std::rc::Rc<crate::formula_ground::families::Workspace<'a, 'source>>>,
    support: &'a Support<'source>,
    values: Binding<'static>,
    lease: StorageLease,
    trail_bytes: usize,
    slots: Vec<Slot>,
    positions: Vec<usize>,
    probes: Vec<Option<Probe<'a, 'source>>>,
    /// Completed necessary selections of short postings, owned by their fixed
    /// positive occurrence and used only with an immutable row condition.
    probe_selections: Vec<probe_selection::Receipt<'a>>,
    /// Borrowed relations are resolved once per occurrence in this snapshot.
    resolutions: Vec<Resolution<'source>>,
    changes: Vec<Vec<usize>>,
    /// Transactional captures borrow this join's fixed support snapshot. The
    /// same lease counts their retained capacity, including after refusal.
    pattern_captures: Vec<(usize, TermRef<'source>)>,
    structural_query: Option<structural::Scratch<'a>>,
    /// Immutable necessary rows, borrowed only by post-capture prepared joins.
    pattern_rows: Option<&'a prepared::PatternRows<'a>>,
    witnesses: Option<witnesses::Witnesses<'source>>,
    /// A passing completion found by an existence check and not yet lent:
    /// the next matched-row request lends it before the join advances.
    held: Option<rows::Frame>,
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
    Expression(queries::expression_keys::Rows<'a, 'source>),
    Range(Buffer<usize>),
    Owned(Buffer<usize>),
}

/// A probe that narrows an occurrence, if any, and the count to add its
/// replacement's rows to when a table selection was declined.
struct Narrowed<'a, 'source> {
    probe: Option<Probe<'a, 'source>>,
    declined: Option<usize>,
}

#[derive(Clone, Copy)]
enum Resolution<'source> {
    Unresolved,
    Resolved {
        rows: Option<&'source relations::RelationRows<'source>>,
        /// A slot in this join's row condition, authenticated for these exact
        /// source occurrences. Absent without a filter or an offered source.
        filter_source: Option<usize>,
    },
}

impl<'source> Resolution<'source> {
    fn rows(self) -> Option<&'source relations::RelationRows<'source>> {
        match self {
            Self::Unresolved => unreachable!("a prepared probe has resolved its relation"),
            Self::Resolved { rows, .. } => rows,
        }
    }

    fn filter_source(self) -> Option<usize> {
        match self {
            Self::Unresolved => unreachable!("a prepared probe has resolved its filter"),
            Self::Resolved { filter_source, .. } => filter_source,
        }
    }
}
impl Probe<'_, '_> {
    /// The rows this probe offers that a table selection could have spared
    /// the join, for the count of rows read in place of a declined one. A
    /// numeric index offers the rows its equality selects, which the table
    /// would offer too, and a table selection is never such a probe: both
    /// count none.
    fn offered(&self) -> usize {
        match self {
            Self::Indexed(rows) => rows.len(),
            Self::Owned(rows) | Self::Range(rows) => rows.slice().len(),
            Self::Expression(_) | Self::Table(_) => 0,
        }
    }

    fn next(
        &self,
        position: &mut usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<usize>, FormulaFailure> {
        let row = match self {
            Self::Indexed(rows) => {
                let row = rows.get(*position);
                if row.is_some() {
                    *position += 1;
                }
                row
            }
            Self::Owned(rows) => {
                let row = rows.slice().get(*position).copied();
                if row.is_some() {
                    *position += 1;
                }
                row
            }
            Self::Range(rows) => {
                counters.work(limits, location)?;
                let row = rows.slice().get(*position).copied();
                if row.is_some() {
                    *position += 1;
                    counters.record(Event::RangeProbeRow);
                }
                row
            }
            Self::Expression(rows) => {
                counters.work(limits, location)?;
                let row = rows.get(*position);
                if row.is_some() {
                    *position += 1;
                    counters.record(Event::ExpressionKeyRow);
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
        rule: impl Into<Ir<'a, 'source, crate::formula_ir::RuleIr>>,
        support: &'a Support<'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let source_rule = rule.into();
        let rule = source_rule.get();
        let mut join = Self::new(
            &rule.body,
            &Binding::new(computation, limits, counters, rule.location)?,
            rule.variables,
            support,
            budget,
            Context::new(computation, limits, counters, rule.location),
        )?;
        join.configure_rule(source_rule, limits, counters)?;
        Ok(join)
    }

    fn configure_rule(
        &mut self,
        rule: impl Into<Ir<'a, 'source, crate::formula_ir::RuleIr>>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let source_rule = rule.into();
        let rule = source_rule.get();
        self.literals = source_rule.map(|rule| rule.body.as_slice());
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
        rule: impl Into<Ir<'a, 'source, crate::formula_ir::RuleIr>>,
        support: &'a Support<'source>,
        filter: Option<&'a dyn BindRestriction>,
        prepared: Option<&'a PreparedRule<'a>>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<RestrictedRows<'a, 'source>, FormulaFailure> {
        let source_rule = rule.into();
        let rule = source_rule.get();
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location: _,
                },
        } = context;
        let mut join = if let Some(prepared) = prepared {
            if !std::ptr::eq(rule, prepared.rule) {
                return Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    location: rule.location,
                });
            }
            let mut join = Self::with_plan(
                &rule.body,
                &Binding::new(computation, limits, counters, rule.location)?,
                rule.variables,
                support,
                JoinPlan::Borrowed(&prepared.plan),
                prepared.total.as_ref(),
                Context::new(computation, limits, counters, rule.location),
            )?;
            join.configure_rule(source_rule, limits, counters)?;
            join.pattern_rows = prepared.pattern_rows.as_ref();
            if prepared.total.is_some() {
                join.certified_total = true;
                join.multiples = prepared.multiples;
                join.coverage = Coverage::Selected;
            }
            join
        } else {
            Self::rule(source_rule, support, computation, limits, budget, counters)?
        };
        join.bind_restriction = filter;
        join.prepare_selections(Context::new(computation, limits, counters, rule.location))?;
        Ok(RestrictedRows::new(join))
    }

    /// Attach necessary domains to their exact rule and support owner.
    pub(super) fn domain_rule(
        rule: impl Into<Ir<'a, 'source, crate::formula_ir::RuleIr>>,
        support: &'a Support<'source>,
        domains: Option<&'a queries::Guards<'a, 'source>>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let source_rule = rule.into();
        let rule = source_rule.get();
        Self::variant_rule(
            source_rule,
            delta::Variant::Full,
            support,
            domains,
            budget,
            Context::new(computation, limits, counters, rule.location),
        )
    }

    fn variant_rule(
        rule: impl Into<Ir<'a, 'source, crate::formula_ir::RuleIr>>,
        variant: delta::Variant,
        support: &'a Support<'source>,
        domains: Option<&'a queries::Guards<'a, 'source>>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let source_rule = rule.into();
        Self::variant_rule_with_total(
            source_rule,
            variant,
            support,
            domains,
            None,
            budget,
            context,
        )
    }

    /// A round owner authenticates any borrowed map to this rule and support.
    /// This changes expression reuse, never the cursor's coverage authority.
    fn variant_rule_with_total(
        rule: impl Into<Ir<'a, 'source, crate::formula_ir::RuleIr>>,
        variant: delta::Variant,
        support: &'a Support<'source>,
        domains: Option<&'a queries::Guards<'a, 'source>>,
        total: Option<&'a expression_memos::ExpressionMemoTable<'a>>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let source_rule = rule.into();
        let rule = source_rule.get();
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
            (variant, total),
        )?;
        join.configure_rule(source_rule, limits, counters)?;
        join.domains = domains;
        Ok(join)
    }

    /// A completed all-producer analysis can certify scalar totality before
    /// support completion. It permits prefix filtering in this rule's delta
    /// cursor, including delta variants, without computed-domain authority or
    /// source-family evidence.
    fn select_static_total_body(
        &mut self,
        rule: &crate::formula_ir::RuleIr,
        totality: &crate::formula_domains::numeric_ir::TotalBody<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        counters.work(limits, rule.location)?;
        if self.source_evidence
            || !totality.belongs_to(rule)
            || !std::ptr::eq(self.literals.get(), rule.body.as_slice())
        {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        }
        self.coverage = Coverage::Selected;
        Ok(())
    }

    /// A full-column proof is local to this immutable support snapshot. It
    /// permits selected support production but never exports family evidence.
    fn select_scalar_body(
        &mut self,
        rule: &'a crate::formula_ir::RuleIr,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        if !self.generated || self.certified_total || self.source_evidence {
            return Ok(());
        }
        counters.charge_work(rule.body.len() as u128, limits, rule.location)?;
        if !rule.body.iter().any(|literal| {
            matches!(literal, LiteralIr::Bind { target, .. } if *target < rule.body_variables)
        }) {
            return Ok(());
        }
        if self.expression_memos.total_body(
            rule,
            self.support,
            &mut self.evaluation,
            Context::new(computation, limits, counters, rule.location),
        )? {
            self.certified_total = true;
            self.coverage = Coverage::Selected;
        }
        self.evaluation.clear_zero_divisor();
        Ok(())
    }

    /// Install the selected view once, before any row or probe exists. Reorder
    /// the same source occurrences; delta ownership remains by original index.
    fn prepare_scalar_comparisons(
        &mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if !self.scalar_selection.is_pending()
            || !self.certified_total
            || !self.generated
            || self.source_evidence
        {
            return Ok(());
        }
        self.scalar_selection = scalar_selection::Preparation::Unavailable;
        let Some(view) = crate::formula_binding_plan::expanded::View::prepare(
            self.literals.get(),
            self.head_slots.start,
            Context::new(computation, limits, counters, location),
        )?
        else {
            return Ok(());
        };
        let domains = scalar_selection::Domains::prepare(
            &self.plan.patterns,
            self.head_slots.start,
            self.support,
            Context::new(computation, limits, counters, location),
        )?;
        let prefix = Binding::new(computation, limits, counters, location)?;
        let mut admit =
            |capacity| computation.preparation_capacity(capacity, limits, counters, location);
        let plan = order::Plan::with_comparisons(
            self.literals.get(),
            &prefix,
            self.values.len(),
            (
                order::SourceRows {
                    relations: self.support,
                    pivot: self.delta,
                },
                Some(&view),
            ),
            budget,
            location,
            Some(&mut admit),
        )?;
        let previous = self.lease.bytes();
        self.plan = JoinPlan::Owned(plan);
        self.scalar_selection = scalar_selection::Preparation::Ready {
            comparisons: view,
            domains,
        };
        self.lease.observe(self.storage_bytes(), location)?;
        computation.storage_observed(
            &self.lease,
            previous,
            self.storage_header(),
            limits,
            counters,
            location,
        )
    }

    /// After support completion and source-family validation, cover the full
    /// positive column domains before ordinary comparisons may select rows.
    /// Hybrid capture performs this same attempt with append-capable terms
    /// before a frozen checker prepares its retained immutable rule and carrier.
    /// This includes successful speculative values before a declined attempt;
    /// a missing frozen term remains an error. Evidence cursors stay complete.
    pub(crate) fn select_total_body(
        &mut self,
        rule: &'a crate::formula_ir::RuleIr,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        if (self.coverage != Coverage::Complete
            && !expression_memos::needs_totality(&rule.body)
            && !self.generated)
            || self.source_evidence
            || self.delta.is_some()
        {
            return Ok(false);
        }
        if !std::ptr::eq(self.literals.get(), rule.body.as_slice()) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        }
        let total = self.expression_memos.total_body(
            rule,
            self.support,
            &mut self.evaluation,
            Context::new(computation, limits, counters, rule.location),
        )?;
        self.evaluation.clear_zero_divisor();
        if total {
            let multiples = multiples::Prepared::new(
                rule,
                &self.plan,
                self.support,
                Context::new(&*computation, limits, counters, rule.location),
            )?;
            self.multiples = multiples;
            self.certified_total = true;
            self.coverage = Coverage::Selected;
        }
        Ok(total)
    }

    /// Prove a flat original-family fragment has no scalar or key failure over
    /// its completed positive source columns. This does not establish that a
    /// complete binding exists and changes neither coverage nor family evidence.
    /// Every normalized sibling must succeed before the caller omits its scan.
    /// Unsupported or partial expressions decline; resource/owner errors propagate.
    pub(super) fn total_family_fragment(
        &mut self,
        rule: &'a crate::formula_ir::RuleIr,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        counters.work(limits, rule.location)?;
        if self.source_evidence || self.delta.is_some() || !matches!(rule.head, HeadIr::Normal(_)) {
            return Ok(false);
        }
        if !std::ptr::eq(self.literals.get(), rule.body.as_slice()) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        }
        if !expression_memos::supports_column_totality(rule, limits, counters)?
            || !keys::covered_family(
                rule,
                &self.plan,
                &mut Context::new(&*computation, limits, counters, rule.location),
            )?
        {
            return Ok(false);
        }
        let total = self.expression_memos.total_family(
            rule,
            self.support,
            &mut self.evaluation,
            Context::new(computation, limits, counters, rule.location),
        );
        self.evaluation.clear_zero_divisor();
        total
    }

    #[cfg_attr(test, track_caller)]
    pub(super) fn element(
        element: impl Into<Ir<'a, 'source, crate::formula_ir::Element>>,
        prefix: &Binding,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let element = element.into();
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
            element.map(|element| element.condition.as_slice()),
            prefix,
            element.body_variables..element.variables,
            support,
            budget,
            Context::new(computation, limits, counters, location),
        )
    }

    /// Choice and conditional heads share one condition/head frame boundary.
    #[cfg_attr(test, track_caller)]
    pub(super) fn local_head(
        literals: impl Into<Ir<'a, 'source, [LiteralIr]>>,
        prefix: &Binding,
        head_slots: std::ops::Range<usize>,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let literals = literals.into();
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut join = Self::local(
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

    /// Stop after the condition stage: each passing condition row is
    /// returned once, with no head value generated for it.
    pub(crate) fn conditions_only(&mut self) {
        self.head_slots = self.head_slots.start..self.head_slots.start;
    }

    /// Whether this join's head stage has an instance under the condition
    /// row `frame`, and whether an instance of it there divides by zero.
    /// `frame` is a passing row of a condition stage over this join's
    /// condition and slots, its own or that of another alternative of the
    /// same condition. The head's generators run under that row alone, as
    /// the staged traversal runs them for each of its rows: the condition's
    /// own generators are not run again. One defined instance settles the
    /// answer, so later instances are not visited; a fatal failure reached
    /// before it is returned.
    pub(crate) fn head_outcome(
        &mut self,
        frame: Binding<'static>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<HeadOutcome, FormulaFailure> {
        let mut pending = crate::formula_binding_cursor::Cursor::new(
            self.literals,
            frame,
            self.support,
            self.bindings,
            self.head_slots.clone(),
            Context::new(computation, limits, counters, location),
        )?;
        if let Some(families) = &self.families {
            pending.share_families(std::rc::Rc::clone(families), limits, counters, location)?;
        }
        let mut outcome = HeadOutcome {
            defined: false,
            zero: false,
        };
        loop {
            match pending.next(
                &mut self.evaluation,
                computation,
                limits,
                budget,
                counters,
                location,
            ) {
                Ok(Some(_)) => {
                    outcome.defined = true;
                    return Ok(outcome);
                }
                Ok(None) => return Ok(outcome),
                Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. }))
                    if self.evaluation.zero_divisor() =>
                {
                    outcome.zero = true;
                    pending.reject(location)?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn stage_head(
        &mut self,
        slots: std::ops::Range<usize>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        objective: impl Into<Ir<'a, 'source, crate::formula_ir::ObjectiveIr>>,
        support: &'a Support<'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let source_objective = objective.into();
        let objective = source_objective.get();
        let mut join = Self::new(
            objective.condition.literals(),
            &Binding::new(computation, limits, counters, objective.location)?,
            objective.variables,
            support,
            budget,
            Context::new(computation, limits, counters, objective.location),
        )?;
        join.literals = source_objective.map(|objective| objective.condition.literals());
        if let crate::formula_ir::ObjectiveCondition::Body { bindings, .. } = &objective.condition {
            join.bindings = bindings.as_ref();
        }
        Ok(join)
    }

    /// Build a component cursor with fixed head values and unrelated slots excluded.
    /// `used` lists the component's variable slots in sorted, duplicate-free order.
    /// Every bound fixed slot is copied, even if this component does not read it.
    /// This execution state certifies neither body truth nor arithmetic totality
    /// across the complete head carrier.
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
        join.fix_component(
            fixed,
            used,
            None,
            Context::new(computation, limits, counters, location),
        )?;
        join.arrange_fixed(computation, limits, budget, counters, location)?;
        join.decide(computation, limits, budget, counters, location)?;
        Ok(join)
    }

    /// Reorder a fresh component cursor once its fixed slots are installed,
    /// as a prepared component plan is ordered: a fixed slot is bound.
    fn arrange_fixed(
        &mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let fixed = self.bound_slots(computation, limits, counters, location)?;
        let empty = Binding::new(computation, limits, counters, location)?;
        let mut admit =
            |capacity| computation.preparation_capacity(capacity, limits, counters, location);
        let plan = order::Plan::with_fixed(
            self.literals.get(),
            (&empty, fixed.slice()),
            fixed.len(),
            order::SourceRows {
                relations: self.support,
                pivot: None,
            },
            budget,
            location,
            Some(&mut admit),
        )?;
        *self.owned_plan() = plan;
        Ok(())
    }

    fn fix_component(
        &mut self,
        fixed: &Binding<'_>,
        used: &[usize],
        expected: Option<&[bool]>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation: _,
            work,
        } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        for slot in 0..fixed.len().min(self.values.len()) {
            counters.work(limits, location)?;
            let present = fixed
                .slots()
                .is_bound(slot)
                .map_err(|error| crate::formula_binding::assignment(error, location))?;
            if expected.is_some_and(|expected| expected[slot] != present) {
                return Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    location,
                });
            }
            if present {
                self.values
                    .copy_present(slot, fixed.slots(), slot, limits, counters, location)?;
            }
        }
        let mut used_cursor = 0;
        for (variable, slot) in self.slots.iter_mut().enumerate() {
            counters.work(limits, location)?;
            while used.get(used_cursor).is_some_and(|&used| used < variable) {
                counters.work(limits, location)?;
                used_cursor += 1;
            }
            if used.get(used_cursor) != Some(&variable) {
                *slot = Slot::Excluded;
            }
        }
        Ok(())
    }
    /// Return a join that [`ComponentPlans`] lent to its state just after it
    /// was lent, for another fixed binding of the same component. The lease,
    /// buffers and evaluation scratch stay, so successive head rows take no
    /// new storage handle or vocabulary witness. Every reset cell is charged
    /// as constructing it was, and the borrowed plan replaces any plan a
    /// scalar selection installed.
    pub(in crate::formula_support) fn rebind(
        &mut self,
        plan: &'a order::Plan<'a>,
        fixed: &Binding<'_>,
        used: &[usize],
        expected: Option<&[bool]>,
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
        self.plan = JoinPlan::Borrowed(plan);
        self.scalar_selection = scalar_selection::Preparation::Pending;
        self.comparisons = Comparisons::Deferred;
        self.pending_comparisons = Comparisons::Deferred;
        self.source_evidence = false;
        self.certified_total = false;
        self.expression_keys = None;
        self.multiples = None;
        self.range_selection = None;
        self.family = family::Evidence::default();
        self.delta = self.plan.pivot;
        self.failure = None;
        self.evaluation.clear_zero_divisor();
        self.pending = None;
        self.pending_head = None;
        self.anchor = None;
        self.positive_prefix = None;
        self.structural_query = None;
        self.held = None;
        self.depth = 0;
        self.traversal = Traversal::Searching;
        self.pattern_captures.clear();
        self.probe_selections.clear();
        if let Some(witnesses) = self.witnesses.as_mut() {
            witnesses.clear();
        }
        for slot in 0..self.values.len() {
            self.values.clear(slot, limits, counters, location)?;
        }
        for slot in &mut self.slots {
            counters.work(limits, location)?;
            *slot = Slot::Relational;
        }
        for verdict in &mut self.verdicts {
            counters.work(limits, location)?;
            *verdict = true;
        }
        for position in &mut self.positions {
            counters.work(limits, location)?;
            *position = 0;
        }
        for probe in &mut self.probes {
            counters.work(limits, location)?;
            *probe = None;
        }
        for resolution in &mut self.resolutions {
            counters.work(limits, location)?;
            *resolution = Resolution::Unresolved;
        }
        for changes in &mut self.changes {
            counters.work(limits, location)?;
            changes.clear();
        }
        for target in self
            .literals
            .get()
            .iter()
            .filter_map(crate::formula_binding_cursor::target)
        {
            self.slots[target] = Slot::Generated;
        }
        self.lease.observe(self.storage_bytes(), location)?;
        self.fix_component(
            fixed,
            used,
            expected,
            Context::new(computation, limits, counters, location),
        )
    }
    /// Share the existing final Builder's family stages. This is installed
    /// before traversal; the query borrow keeps the exact Support alive.
    pub(crate) fn share_families(
        &mut self,
        families: std::rc::Rc<crate::formula_ground::families::Workspace<'a, 'source>>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        families.authenticate(self.support, location)?;
        counters.work(limits, location)?;
        assert!(
            self.pending.is_none() && self.pending_head.is_none(),
            "family access is installed before cursor traversal"
        );
        self.families = Some(families);
        Ok(())
    }

    #[cfg_attr(test, track_caller)]
    pub(crate) fn local(
        literals: impl Into<Ir<'a, 'source, [LiteralIr]>>,
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        Self::local_inner::<false>(literals, prefix, variables, support, budget, context)
            .map(|(join, _)| join)
    }

    /// The same local constructor, optionally lending its already selected
    /// source plan to a completed-family occurrence certificate. Ordinary local
    /// callers instantiate RETAIN=false and perform no extra Rc clone.
    #[cfg_attr(test, track_caller)]
    pub(crate) fn local_certified(
        literals: impl Into<Ir<'a, 'source, [LiteralIr]>>,
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(Self, Option<LocalPlanCertificate<'source>>), FormulaFailure> {
        Self::local_inner::<true>(literals, prefix, variables, support, budget, context)
    }

    #[cfg_attr(test, track_caller)]
    fn local_inner<const RETAIN: bool>(
        literals: impl Into<Ir<'a, 'source, [LiteralIr]>>,
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(Self, Option<LocalPlanCertificate<'source>>), FormulaFailure> {
        let literals = literals.into();
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        counters.record(Event::LocalPlanRequest);
        #[cfg(test)]
        let _site = local_plans::tests::request(std::panic::Location::caller());
        #[cfg(test)]
        let enabled = local_plans::tests::enabled();
        #[cfg(not(test))]
        let enabled = true;
        let shared = if let Ir::Source(source) = literals
            && !source.is_empty()
            && enabled
        {
            support.local_plan(
                source,
                prefix,
                variables,
                budget,
                Context::new(computation, limits, counters, location),
            )?
        } else {
            None
        };
        let certificate = if RETAIN {
            if let Some(plan) = &shared {
                counters.work(limits, location)?;
                Some(LocalPlanCertificate::retain(plan))
            } else {
                None
            }
        } else {
            None
        };
        let mut join = if let Some(plan) = shared {
            Self::with_plan(
                literals.get(),
                prefix,
                variables,
                support,
                JoinPlan::Shared(plan),
                None,
                Context::new(computation, limits, counters, location),
            )?
        } else {
            Self::new(
                literals.get(),
                prefix,
                variables,
                support,
                budget,
                Context::new(computation, limits, counters, location),
            )?
        };
        join.literals = literals;
        Ok((join, certificate))
    }

    pub(crate) fn new(
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
            (delta::Variant::Full, None),
        )
    }

    fn new_with_rows(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        budget: &mut Budget,
        context: Context<'_, &Computation<'_, '_>>,
        (variant, total): (
            delta::Variant,
            Option<&'a expression_memos::ExpressionMemoTable<'a>>,
        ),
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
            JoinPlan::Owned(plan),
            total,
            Context::new(computation, limits, counters, location),
        )
    }

    fn with_plan(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        plan: JoinPlan<'a>,
        total: Option<&'a expression_memos::ExpressionMemoTable<'a>>,
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
        let mut lease = computation.lease();
        let plan_bytes = match &plan {
            JoinPlan::Owned(plan) => plan.retained_bytes(),
            JoinPlan::Borrowed(_) | JoinPlan::Shared(_) => 0,
        };
        let header = size_of::<Self>()
            - size_of::<Binding>()
            - size_of::<Evaluation>()
            - usize::from(total.is_none()) * size_of::<expression_memos::ExpressionMemoTable<'_>>();
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
            scalar_selection: scalar_selection::Preparation::Pending,
            literals: Ir::Temporary(literals),
            generated: literals
                .iter()
                .any(|literal| crate::formula_binding_cursor::target(literal).is_some()),
            pending: None,
            pending_head: None,
            head_slots: variables..variables,
            comparisons: Comparisons::Deferred,
            pending_comparisons: Comparisons::Deferred,
            coverage: if family::partial(literals) {
                Coverage::Complete
            } else {
                Coverage::Selected
            },
            source_evidence: false,
            certified_total: false,
            expression_keys: None,
            multiples: None,
            range_selection: None,
            family: family::Evidence::default(),
            head_bounds: &[],
            checked_guard: None,
            delta: plan.pivot,
            plan,
            verdicts: Vec::new(),
            failure: None,
            evaluation: Evaluation::default(),
            expression_memos: expression_memos::ExpressionMemos::for_cursor(
                total,
                &Context::new(computation, limits, counters, location),
            )?,
            domains: None,
            bind_restriction: None,
            anchor: None,
            positive_prefix: None,
            support,
            families: None,
            values,
            lease,
            trail_bytes: 0,
            slots: Vec::new(),
            positions: Vec::new(),
            probes: Vec::new(),
            probe_selections: Vec::new(),
            resolutions: Vec::new(),
            changes: Vec::new(),
            pattern_captures: Vec::new(),
            structural_query: None,
            pattern_rows: None,
            witnesses: None,
            held: None,
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
        join.classify_slots(prefix, limits, counters, location)?;
        Ok(join)
    }

    fn classify_slots(
        &mut self,
        prefix: &Binding,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        for target in self
            .literals
            .get()
            .iter()
            .filter_map(crate::formula_binding_cursor::target)
        {
            self.slots[target] = Slot::Generated;
        }
        for (index, slot) in self.slots.iter_mut().enumerate().take(prefix.len()) {
            counters.work(limits, location)?;
            if !prefix.is_bound(index, location)? {
                *slot = Slot::Excluded;
            }
        }
        Ok(())
    }

    fn storage_header(&self) -> usize {
        size_of::<Self>()
            - size_of::<Binding>()
            - size_of::<Evaluation>()
            - self.expression_memos.leased_header()
            - self.scalar_selection.leased_header()
            - self
                .structural_query
                .as_ref()
                .map_or(0, |_| structural::Scratch::leased_header())
            - usize::from(self.pending.is_some())
                * size_of::<crate::formula_binding_cursor::Cursor<'_, '_>>()
            - usize::from(self.pending_head.is_some())
                * size_of::<crate::formula_binding_cursor::Cursor<'_, '_>>()
    }
    fn storage_bytes(&self) -> usize {
        let plan = match &self.plan {
            JoinPlan::Owned(plan) => plan.retained_bytes() as usize,
            JoinPlan::Borrowed(_) | JoinPlan::Shared(_) => 0,
        };
        self.storage_header()
            + plan
            + self.trail_bytes
            + self.slots.capacity() * size_of::<Slot>()
            + self.verdicts.capacity() * size_of::<bool>()
            + self.positions.capacity() * size_of::<usize>()
            + self.probes.capacity() * size_of::<Option<Probe<'_, '_>>>()
            + self.probe_selections.capacity() * size_of::<probe_selection::Receipt<'_>>()
            + self.resolutions.capacity() * size_of::<Resolution<'_>>()
            + self.changes.capacity() * size_of::<Vec<usize>>()
            + self.pattern_captures.capacity() * size_of::<(usize, TermRef<'_>)>()
            + self
                .witnesses
                .as_ref()
                .map_or(0, witnesses::Witnesses::capacity_bytes)
    }
    fn initialize_storage(
        &mut self,
        variables: usize,
        count: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        location: ProgramSite,
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
    /// Evaluate `guard` beside every row that holds, so that a fatal failure
    /// in it refuses and a zero divisor is recorded. Such a row is still
    /// returned: the guard is the consequent of a conditional literal, and
    /// its consumer reads an undefined consequent as false. A row that fails
    /// a check, or is omitted, does not reach the guard: the condition
    /// comes before its consequent.
    pub(crate) fn check_guard(&mut self, guard: &'a crate::formula_guard::Guard) {
        if guard.expressions().any(family::expression) {
            self.coverage = Coverage::Complete;
        }
        self.checked_guard = Some(guard);
    }
    /// Traverse source families before head truth selects emitted constraints.
    /// The head still contributes arithmetic definedness and failure evidence.
    pub(crate) fn evidence(&mut self) {
        self.source_evidence = true;
        self.expression_keys = None;
        self.certified_total = false;
        self.multiples = None;
        self.range_selection = None;
        self.coverage = Coverage::Complete;
        self.domains = None;
    }
    pub(crate) fn take_family(&mut self) -> family::Evidence {
        std::mem::take(&mut self.family)
    }
    fn owned_plan(&mut self) -> &mut order::Plan<'a> {
        match &mut self.plan {
            JoinPlan::Owned(plan) => plan,
            JoinPlan::Borrowed(_) | JoinPlan::Shared(_) => {
                unreachable!("prepared plans are immutable")
            }
        }
    }
    fn bound_slots(
        &self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
    /// Fix, for the current order and prefix, the depth at which each
    /// comparison is decided and whether any check waits for the complete
    /// row. Called after every arrangement and after the prefix is fixed.
    fn decide(
        &mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let prefix = self.bound_slots(computation, limits, counters, location)?;
        let mut admit =
            |capacity| computation.preparation_capacity(capacity, limits, counters, location);
        let decisions = order::Decisions::checked(
            self.literals.get(),
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
    pub(crate) fn next(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
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
    /// Ordinary admission joins have no external row condition and visit the full family.
    pub(crate) fn next_row(
        &mut self,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<Row<'_>>, FormulaFailure> {
        let row = self.next_staged(
            Ownership::Lend,
            None,
            Advance::Base,
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
        location: ProgramSite,
    ) -> Result<Option<Row<'static>>, FormulaFailure> {
        let row = self.next_staged(
            Ownership::Own,
            None,
            Advance::Base,
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
        location: ProgramSite,
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
            Advance::Base,
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
    /// Negative gates and aggregate truth remain formulas here and select no
    /// row; a negation support completion decides omits its row afterwards
    /// (`decided_negations_hold`).
    fn next_staged(
        &mut self,
        ownership: Ownership,
        projected: Option<AtomPattern>,
        advance: Advance,
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
                advance,
                budget,
                Context::new(computation, limits, counters, location),
            )?
            else {
                return Ok(None);
            };
            if !row.passes || self.head_slots.is_empty() {
                return Ok(Some(row));
            }
            let mut pending = crate::formula_binding_cursor::Cursor::new(
                self.literals,
                row.frame.into_owned(),
                self.support,
                self.bindings,
                self.head_slots.clone(),
                Context::new(computation, limits, counters, location),
            )?;
            if let Some(families) = &self.families {
                pending.share_families(std::rc::Rc::clone(families), limits, counters, location)?;
            }
            self.pending_head = Some(pending);
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
            // The row holds: only the guard checked beside it divides by
            // zero, and its consumer gives that guard a value.
            filters::Selection::UndefinedGuard(error) => {
                self.family.zero.get_or_insert(error);
                Some(Staged {
                    frame,
                    passes: true,
                })
            }
        }
    }

    fn next_inner(
        &mut self,
        ownership: Ownership,
        projected: Option<AtomPattern>,
        advance: Advance,
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
                match pending.step(
                    &mut self.evaluation,
                    computation,
                    limits,
                    budget,
                    counters,
                    location,
                ) {
                    Ok(crate::formula_binding_cursor::Step::Frame(binding)) => {
                        let binding = Frame::Owned(binding);
                        let selection = self.filters(
                            &binding,
                            self.pending_comparisons.clone(),
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
                    Ok(crate::formula_binding_cursor::Step::Valueless(slot)) => {
                        self.valueless(slot, computation, limits, counters, location)?;
                        continue;
                    }
                    Ok(crate::formula_binding_cursor::Step::Finished) => {}
                    Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. })) => {
                        let zero = self.evaluation.zero_divisor();
                        let excluded = filters::excludes(
                            (
                                self.literals.get(),
                                &self.plan.decisions,
                                self.checked_guard,
                            ),
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
            if matches!(advance, Advance::Current) {
                return Ok(None);
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
            self.prepare_continuation(
                binding.into_owned(),
                projected,
                Context::new(computation, limits, counters, location),
            )?;
        }
    }
    /// The generator of `slot` in the proposal cursor has no value under the
    /// values of those before it: read the comparisons over that value, which
    /// no frame under them will reach (`filters::valueless`).
    fn valueless(
        &mut self,
        slot: usize,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let Some(pending) = &self.pending else {
            return Ok(());
        };
        filters::valueless(
            (
                self.literals.get(),
                &self.plan.decisions,
                self.checked_guard,
            ),
            slot,
            &mut self.evaluation,
            pending.binding(),
            Context::new(computation, limits, counters, location),
        )
    }
    /// Reuse only a complete support continuation; otherwise restart its
    /// existing frame or initialize the first cursor. Both live input frames
    /// remain admitted before comparison, and a hit adds no storage receipt.
    fn prepare_continuation(
        &mut self,
        binding: Binding<'static>,
        projected: Option<AtomPattern>,
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
        if let Some(pending) = &mut self.pending {
            // Only support production passes the normal head projection.
            // Formula lowering retains every positive activation. The
            // previous completed continuation already published its heads
            // and contributed its zero-divisor family evidence.
            if projected.is_some()
                && let Some(inputs) = self
                    .bindings
                    .and_then(|plan| plan.continuation_inputs.as_deref())
                && pending.completed_with(
                    &binding,
                    inputs,
                    Context::new(computation, limits, counters, location),
                )?
            {
                return Ok(());
            }
            self.pending_comparisons = self.comparisons.clone();
            pending.restart(
                binding,
                Context::new(computation, limits, counters, location),
            )?;
        } else {
            self.pending_comparisons = self.comparisons.clone();
            let mut pending = crate::formula_binding_cursor::Cursor::new(
                self.literals,
                binding,
                self.support,
                self.bindings,
                0..self.head_slots.start,
                Context::new(computation, limits, counters, location),
            )?;
            if let Some(families) = &self.families {
                pending.share_families(std::rc::Rc::clone(families), limits, counters, location)?;
            }
            pending.select_range(self.range_selection);
            self.pending = Some(pending);
        }
        self.lease.observe(self.storage_bytes(), location)?;
        Ok(())
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
        self.prepare_scalar_comparisons(computation, limits, budget, counters, location)?;
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
            let row = self.advance_probe(
                pattern,
                Context::new(computation, limits, counters, location),
            )?;
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
            let selected = self.permits_row(pattern, atom, limits, counters, location)?;
            let permitted = selected.permits()
                && self
                    .positive_prefix
                    .as_ref()
                    .is_none_or(|prefix| prefix.permits(selected));
            if let Some(selection) = self.probe_selections.get_mut(self.depth) {
                selection.record(self.positions[self.depth] - 1, permitted);
            }
            if !permitted {
                continue;
            }
            let matches = self.match_row(
                pattern.pattern,
                atom,
                budget,
                Context::new(computation, limits, counters, location),
            )?;
            if matches && self.filter_prefix(computation, limits, counters, location)? {
                if let Some(witnesses) = &mut self.witnesses {
                    counters.work(limits, location)?;
                    witnesses.push(self.depth, atom);
                }
                // Pattern matching and prefix scalars must succeed before a
                // mapped row's identity can describe this completed binding.
                if let Some(prefix) = &mut self.positive_prefix {
                    prefix.advance(self.depth, pattern.source, selected);
                }
                self.depth += 1;
            } else {
                self.undo_at(self.depth, limits, counters, location)?;
            }
        }
    }
    /// Authenticate this depth's relation and filter as one immutable pair.
    /// A refused authentication publishes neither half; backtracking reuses a
    /// successfully resolved pair independently of the chosen probe strategy.
    fn resolve_probe_source(
        &mut self,
        pattern: PatternOccurrence<'a>,
        work: GroundingWork<'_>,
    ) -> Result<(), FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        // Relation ownership is independent of how the row source was prepared.
        // A preselected probe still needs this snapshot's owner before row access.
        if matches!(self.resolutions[self.depth], Resolution::Unresolved) {
            let rows = self
                .support
                .resolve(pattern.atom(), limits, counters, location)?;
            let filter_source = match (self.bind_restriction, rows) {
                (Some(filter), Some(source)) if source.row_count() != 0 => {
                    Some(filter.resolve(source.atoms, limits, counters, location)?)
                }
                _ => None,
            };
            // Neither the source nor its filter slot is published after a
            // refused authentication. Backtracking keeps this immutable pair.
            self.resolutions[self.depth] = Resolution::Resolved {
                rows,
                filter_source,
            };
        }
        Ok(())
    }

    /// Resolve the selected source's probe before its cursor advances.
    fn prepare_probe(
        &mut self,
        pattern: PatternOccurrence<'a>,
        context: Context<'_, &mut Computation<'_, '_>>,
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
        self.resolve_probe_source(pattern, GroundingWork::new(limits, counters, location))?;
        if self.probes[self.depth].is_none() {
            let source = self.resolutions[self.depth].rows();
            if let Some(probe) = self.anchored_probe(
                source,
                pattern.source,
                GroundingWork::new(limits, counters, location),
            )? {
                self.probes[self.depth] = Some(probe);
                return Ok(());
            }
            if let structural::Selection::Posting(posting) = self.structural_selection(
                pattern.pattern,
                source,
                Context::new(computation, limits, counters, location),
            )? {
                self.probes[self.depth] = Some(self.indexed_probe(
                    posting,
                    source,
                    pattern.source,
                    limits,
                    counters,
                    location,
                )?);
                return Ok(());
            }
            let narrowed = self.narrowed_probe(
                pattern,
                source,
                Context::new(computation, limits, counters, location),
            )?;
            let probe = match narrowed.probe {
                Some(probe) => probe,
                None => self.broad_probe(
                    pattern,
                    source,
                    Context::new(computation, limits, counters, location),
                )?,
            };
            // The rows this probe offers are read in place of a selection
            // the snapshot declined, and count towards preparing its table.
            if let Some(ticket) = narrowed.declined {
                self.support.count_scanned(ticket, probe.offered());
            }
            self.probes[self.depth] = Some(probe);
        }
        Ok(())
    }

    /// The probe of a covered computed domain or, where its selection is
    /// absent or declined, of a numeric index over the occurrence.
    ///
    /// Covered computed domains may combine several restrictions, so their
    /// selection is kept before the numeric fallback, whose ordinary posting
    /// can otherwise replace it with a broader candidate set. A declined
    /// selection names the count that the probe taking its place adds to
    /// ([`Support::count_scanned`]).
    fn narrowed_probe(
        &mut self,
        pattern: PatternOccurrence<'a>,
        source: Option<&'source relations::RelationRows<'source>>,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Narrowed<'a, 'source>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let declined = match self.computed_probe(
            pattern.pattern,
            source,
            Context::new(computation, limits, counters, location),
        )? {
            expression_memos::Computed::Probe(probe) => {
                return Ok(Narrowed {
                    probe: Some(probe),
                    declined: None,
                });
            }
            expression_memos::Computed::Declined(ticket) => Some(ticket),
            expression_memos::Computed::Absent => None,
        };
        let probe = self.expression_key_probe(
            pattern,
            source,
            Context::new(&*computation, limits, counters, location),
        )?;
        Ok(Narrowed { probe, declined })
    }

    /// The probe of an occurrence that neither a computed domain nor a
    /// numeric index narrows: a table selection under the table strategy, a
    /// posting of multiples, or the indexed selection by its bound columns.
    fn broad_probe(
        &mut self,
        pattern: PatternOccurrence<'a>,
        source: Option<&'source relations::RelationRows<'source>>,
        context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<Probe<'a, 'source>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let binding = self
            .values
            .view(computation.read(), limits, counters, location)?;
        Ok(
            if let Some(rows) = self.support.select_at(
                source,
                pattern.pattern,
                binding,
                limits,
                counters,
                location,
            )? {
                Probe::Table(rows)
            } else if let Some(probe) = self.multiples_probe(
                source,
                Context::new(&*computation, limits, counters, location),
            )? {
                probe
            } else {
                match self.support.indexed_selection_at(
                    source,
                    pattern.pattern,
                    binding,
                    (self.delta, pattern.source),
                    GroundingWork::new(limits, counters, location),
                )? {
                    queries::IndexedSelection::Exact(range) => {
                        Probe::Indexed(delta::Rows::Interval(range))
                    }
                    queries::IndexedSelection::Posting(posting) => self.indexed_probe(
                        posting,
                        source,
                        pattern.source,
                        limits,
                        counters,
                        location,
                    )?,
                }
            },
        )
    }

    /// Resolve an explicitly anchored occurrence after source/filter admission.
    /// Its original row identity is retained across backtracking.
    fn anchored_probe(
        &mut self,
        source: Option<&'source relations::RelationRows<'source>>,
        occurrence: usize,
        work: GroundingWork<'_>,
    ) -> Result<Option<Probe<'a, 'source>>, FormulaFailure> {
        let Some(anchor) = self
            .anchor
            .as_mut()
            .filter(|anchor| anchor.occurrence == occurrence)
        else {
            return Ok(None);
        };
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        if matches!(anchor.row, rows::AnchorRow::Unresolved) {
            counters.work(limits, location)?;
            anchor.row = match source {
                Some(source) => self
                    .support
                    .lookup_at(source, anchor.atom, limits, counters, location)?
                    .map_or(rows::AnchorRow::Absent, rows::AnchorRow::Present),
                None => rows::AnchorRow::Absent,
            };
        }
        let range = match anchor.row {
            rows::AnchorRow::Absent => 0..0,
            rows::AnchorRow::Present(row) => row..row + 1,
            rows::AnchorRow::Unresolved => unreachable!("resolved above"),
        };
        Ok(Some(Probe::Indexed(delta::Rows::Interval(range))))
    }
    fn indexed_probe(
        &self,
        posting: Option<&'source [usize]>,
        source: Option<&'source relations::RelationRows<'source>>,
        occurrence: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Probe<'a, 'source>, FormulaFailure> {
        let selected = self
            .pattern_rows
            .map(|pattern_rows| {
                pattern_rows.posting(self.depth, source, limits, counters, location)
            })
            .transpose()?
            .flatten();
        // Both postings are necessary conditions over the same ordered source.
        // Keep the smaller one; the ordinary matcher checks their conjunction.
        let posting = match (posting, selected) {
            (Some(indexed), Some(selected)) if selected.len() < indexed.len() => Some(selected),
            (None, Some(selected)) => Some(selected),
            (indexed, _) => indexed,
        };
        let total = source.map_or(0, relations::RelationRows::row_count);
        Ok(Probe::Indexed(if self.delta.is_some() {
            let old = source.map_or(0, relations::RelationRows::old_rows);
            let range = delta::interval(self.delta, occurrence, old, total);
            delta::Rows::within(posting, range, limits, counters, location)?
        } else {
            delta::Rows::all(posting, total)
        }))
    }
    /// External admitted-source selection and ordinary necessary domains meet
    /// at the same pre-binding boundary. Complete arithmetic admission ignores
    /// domains as before; only an explicit filtered consumer has a row condition.
    fn permits_row(
        &self,
        pattern: PatternOccurrence<'_>,
        atom: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<RowSelection, FormulaFailure> {
        let selected = if let Some(filter) = self.bind_restriction {
            let source = self.resolutions[self.depth]
                .filter_source()
                .expect("an offered filtered row has an authenticated source");
            filter.select(source, pattern.source, atom, limits, counters, location)?
        } else {
            RowSelection::Possible
        };
        if !selected.permits() {
            return Ok(selected);
        }
        if self.coverage == Coverage::Selected
            && let Some(domains) = self.domains
            && !domains.permits(pattern.source, atom.position(), limits, counters, location)?
        {
            return Ok(RowSelection::Rejected);
        }
        Ok(selected)
    }
    fn match_row(
        &mut self,
        pattern: PositivePattern<'_>,
        atom: zetesis_core::relation::Row<'_, 'source>,
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
            let result = (|| {
                let other_bytes = self.storage_bytes()
                    - self.pattern_captures.capacity() * size_of::<(usize, TermRef<'_>)>();
                let view = self
                    .values
                    .view(computation.read(), limits, counters, location)?;
                if !pattern.matches(
                    atom,
                    view,
                    computation,
                    &mut self.pattern_captures,
                    &mut crate::formula_pattern::MatchContext {
                        work: GroundingWork::new(limits, counters, location),
                        budget,
                        lease: &mut self.lease,
                        other_bytes,
                    },
                )? {
                    return Ok(false);
                }
                self.reserve_trail(
                    self.pattern_captures.len(),
                    computation,
                    limits,
                    counters,
                    location,
                )?;
                for index in 0..self.pattern_captures.len() {
                    let (slot, value) = self.pattern_captures[index];
                    self.values
                        .set_term_with(slot, value, computation.read(), location, || {
                            counters.work(limits, location)
                        })?;
                    self.changes[self.depth].push(slot);
                }
                Ok(true)
            })();
            self.pattern_captures.clear();
            return result;
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
                    let old = self.changes[self.depth].capacity() * size_of::<usize>();
                    let other = self.storage_bytes() - old;
                    let trail = &mut self.changes[self.depth];
                    let lease = &mut self.lease;
                    let trail_bytes = &mut self.trail_bytes;
                    let mut reserve_before_set = true;
                    self.values.set_term_with(
                        variable,
                        value,
                        computation.read(),
                        location,
                        || {
                            // Source authentication precedes reservation; the
                            // next callback has only the ordinary set charge.
                            if std::mem::take(&mut reserve_before_set) {
                                let result = reserve(
                                    trail,
                                    1,
                                    lease,
                                    other,
                                    Context::new(&*computation, limits, counters, location),
                                );
                                *trail_bytes =
                                    *trail_bytes - old + trail.capacity() * size_of::<usize>();
                                result?;
                            }
                            counters.work(limits, location)
                        },
                    )?;
                    trail.push(variable);
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
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        if self.coverage == Coverage::Complete {
            return Ok(true);
        }
        if !self.scalar_selection.permits(
            self.literals.get(),
            &self.values,
            Context::new(&*computation, limits, counters, location),
        )? {
            self.verdicts[self.depth] = false;
            return Ok(false);
        }
        let depth = self.depth;
        let mut passes = depth == 0 || self.verdicts[depth - 1];
        for index in self.plan.decisions.decided_at(depth) {
            let compared = if let Some(operands) = self
                .scalar_selection
                .comparisons()
                .and_then(|view| view.operands(index))
            {
                // The selected operands have their own DAG. Original unary
                // expression-memo descriptors name generated slots and cannot serve it.
                self.evaluation.source_comparison(
                    operands,
                    &self.values,
                    Context::new(computation, limits, counters, location),
                )
            } else {
                let comparison =
                    comparison(&self.literals[index]).expect("a decided literal is a comparison");
                let decided = integer_comparisons::decide(
                    comparison,
                    &self.values,
                    Context::new(&*computation, limits, counters, location),
                )?;
                let ([left, right], relation) = comparison.operands();
                if let Some(holds) = decided {
                    Ok(holds)
                } else if self.certified_total {
                    self.expression_memos.compare(
                        index,
                        ([left, right], relation),
                        &self.values,
                        &mut self.evaluation,
                        Context::new(computation, limits, counters, location),
                    )
                } else {
                    self.evaluation.source_comparison(
                        ([left, right], relation),
                        &self.values,
                        Context::new(computation, limits, counters, location),
                    )
                }
            };
            match compared {
                Ok(false) if self.certified_total => {
                    // Complete source domains establish that later arithmetic
                    // cannot fail. False now excludes every extension without
                    // concealing a source diagnostic.
                    self.verdicts[depth] = false;
                    return Ok(false);
                }
                Ok(value) => passes &= value,
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
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if let Some(witnesses) = &self.witnesses {
            counters.charge_work(witnesses.removed_at(depth) as u128, limits, location)?;
        }
        self.values
            .clear_trail(&mut self.changes[depth], limits, counters, location)?;
        if let Some(witnesses) = &mut self.witnesses {
            witnesses.undo(depth);
        }
        if let Some(prefix) = &mut self.positive_prefix {
            prefix.undo(depth);
        }
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
        location: ProgramSite,
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
        location: ProgramSite,
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
        location: ProgramSite,
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
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if self.traversal == Traversal::PendingUndo {
            self.undo_at(self.depth - 1, limits, counters, location)?;
            self.depth -= 1;
            self.traversal = Traversal::Searching;
        }
        Ok(())
    }
}

/// The comparison a literal decides: a comparison literal or an argument
/// check. Captured arguments reuse comparison evaluation, never binding
/// inference.
fn comparison(literal: &LiteralIr) -> Option<&crate::formula_ir::Comparison> {
    match literal {
        LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison) => Some(comparison),
        _ => None,
    }
}

pub(crate) fn expression(
    expression: &Expression,
    assignment: &Binding,
    computation: &mut Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
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
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    if let Relation::Eq | Relation::Neq = relation {
        // ASP term order is structural: it equates two terms exactly when they
        // are the same typed value. Equality therefore needs no order, and two
        // terms of one canonical scope decide it by identity alone, without
        // walking either payload.
        let equal = left.equals_ref_with(right, || counters.work(limits, location))?;
        return Ok(equal == matches!(relation, Relation::Eq));
    }
    let order = left.compare_terms_with(right, || counters.work(limits, location))?;
    Ok(relation_holds(relation, order))
}

/// Whether `relation` holds between two values whose order is `order`.
pub(super) fn relation_holds(relation: Relation, order: std::cmp::Ordering) -> bool {
    match relation {
        Relation::Eq => order.is_eq(),
        Relation::Neq => !order.is_eq(),
        Relation::Lt => order.is_lt(),
        Relation::Le => !order.is_gt(),
        Relation::Gt => order.is_gt(),
        Relation::Ge => !order.is_lt(),
    }
}

pub(super) fn undefined(location: ProgramSite) -> FormulaFailure {
    ExpansionFailure::Evaluation {
        error: EvalError::Undefined,
        location,
    }
    .into()
}
