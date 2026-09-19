//! A finite support upper bound and complete iterative relational joins.

mod evaluation;
mod delta;
mod order;
mod producers;
mod relations;
mod queries;
#[cfg(test)]
mod columnar;
#[cfg(test)]
mod membership;
#[cfg(test)]
mod postings;

use std::collections::BTreeSet;

use themelios_base::span::Location;
use themelios_program::program::{DefaultNegation, Relation};
use themelios_program::term::EvalError;
use zetesis_core::{Atom, AtomPattern, Value};

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Prepared, value_bytes};
use crate::grounding_observer::{Event, Work};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

pub(crate) use evaluation::Evaluation;
pub(crate) use queries::{Candidates, Support};
#[cfg(test)]
use relations::RelationRows;
pub(crate) use relations::{Relations, SupportCatalog};

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
}

/// An immutable relation view of one successfully completed support owner.
/// Intermediate round snapshots deliberately have only the `Relations` type.
pub(crate) struct CompletedSupport<'source> {
    relations: Relations<'source>,
}

impl CompletedCatalog {
    pub(crate) fn snapshot(
        &self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<CompletedSupport<'_>, FormulaFailure> {
        self.catalog
            .snapshot(limits, counters, location)
            .map(|relations| CompletedSupport { relations })
    }
}

impl CompletedSupport<'_> {
    /// Borrow the same typed rows used by final grounding and objective joins.
    pub(crate) fn queries(
        &self,
        strategy: crate::JoinStrategy,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<CompletedQueries<'_>, FormulaFailure> {
        Ok(CompletedQueries {
            support: Support::completed(&self.relations, strategy, limits, counters, location)?,
        })
    }
}

/// Queries over exactly one completed support certificate. Only its completed
/// snapshot constructs this workspace; growing relations cannot claim it.
pub(crate) struct CompletedQueries<'source> {
    support: Support<'source>,
}
impl<'source> CompletedQueries<'source> {
    pub(crate) fn support(&self) -> &Support<'source> {
        &self.support
    }
}

pub(crate) fn row_values<'source>(
    row: zetesis_core::relation::Row<'_, 'source>,
) -> impl ExactSizeIterator<Item = &'source Value> {
    (0..row.predicate().arity()).map(move |column| row.value(column).expect("checked row arity"))
}

#[derive(Default)]
pub(crate) struct Counters {
    pub work: u64,
    pub substitutions: u64,
    generated_values: BTreeSet<Value>,
    observed: Work,
}
impl Counters {
    pub(super) fn observed(observed: Work) -> Self {
        Self {
            observed,
            ..Self::default()
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
        ceiling(
            FormulaResource::Work,
            u128::from(self.work) + amount,
            u128::from(limits.max_work),
            location,
        )?;
        self.work += u64::try_from(amount).expect("charged work fits its u64 ceiling");
        Ok(())
    }
    pub(super) fn generated(
        &mut self,
        value: &Value,
        limits: &FormulaLimits,
        budget: &mut Budget,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if !self.generated_values.contains(value) {
            ceiling(
                FormulaResource::GeneratedValues,
                self.generated_values.len() as u128 + 1,
                limits.max_generated_values as u128,
                location,
            )?;
            self.generated_values.insert(copy(value, budget, location)?);
        }
        Ok(())
    }
    pub(super) fn substitution(
        &mut self,
        limits: &FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Substitutions,
            u128::from(self.substitutions) + 1,
            u128::from(limits.max_substitutions),
            location,
        )?;
        self.substitutions += 1;
        Ok(())
    }
}

pub(crate) fn build(
    prepared: &Prepared,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    fallback: Location,
) -> Result<CompletedCatalog, FormulaFailure> {
    let plan = producers::ProducerPlan::prepare(prepared, limits, counters, fallback)?;
    complete(prepared, plan, domains, limits, budget, counters, fallback)
}

/// Every round joins each selected rule under its domain guards, when the
/// analysis prepared candidates: a row a guard rejects has no complete
/// continuation the exclusion rule keeps, in this round as in the final one.
fn complete(
    prepared: &Prepared,
    mut plan: Option<producers::ProducerPlan<'_>>,
    domains: Option<&crate::formula_domains::Domains<'_>>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    fallback: Location,
) -> Result<CompletedCatalog, FormulaFailure> {
    let mut catalog = SupportCatalog::with_prepared_bytes(
        plan.as_ref().map_or(0, producers::ProducerPlan::bytes),
    );
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
        let delta = {
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
                drop(plan);
                catalog.release_preparation();
                return Ok(CompletedCatalog { catalog });
            }
            counters.work(limits, fallback)?;
            counters.record(Event::SupportSnapshotPreparation);
            let relations = catalog.snapshot(limits, counters, fallback)?;
            let support = Support::indexed(&relations, limits, counters, fallback)?;
            let mut delta = BTreeSet::new();
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
                let guards = match domains {
                    Some(domains) => support.domain_guards(
                        rule,
                        domains.for_rule(index, rule)?,
                        limits,
                        budget,
                        counters,
                    )?,
                    None => None,
                };
                while let Some(variant) = variants.next(limits, counters)? {
                    let mut outer = Join::domain_rule(rule, &support, guards.as_ref(), budget)?;
                    outer.partition(variant, budget, rule.location)?;
                    derive_rule(
                        rule, &mut outer, &support, &mut delta, limits, budget, counters,
                    )?;
                }
                selected = schedule.next(limits, counters, fallback)?;
            }
            delta
        };
        if delta.is_empty() {
            drop(plan);
            catalog.release_preparation();
            return Ok(CompletedCatalog { catalog });
        }
        if let Some(plan) = &mut plan {
            plan.advance(&delta, limits, counters, fallback)?;
        }
        catalog.advance(limits, counters, fallback)?;
        for atom in delta {
            catalog = catalog.insert(atom, limits, counters, fallback)?;
        }
    }
}

fn derive_rule(
    rule: &crate::formula_ir::RuleIr,
    outer: &mut Join<'_, '_>,
    support: &Support<'_>,
    delta: &mut BTreeSet<Atom>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    let mut derivation = Derivation {
        support,
        delta,
        limits,
        budget,
        counters,
    };
    while let Some(binding) = match &rule.head {
        HeadIr::Normal(Some(head)) => outer.next_support(
            head,
            derivation.delta,
            limits,
            derivation.budget,
            derivation.counters,
            rule.location,
        )?,
        _ => outer.next(
            limits,
            derivation.budget,
            derivation.counters,
            rule.location,
        )?,
    } {
        match &rule.head {
            HeadIr::Normal(Some(head)) => derivation.head(head, &binding, rule.location)?,
            HeadIr::Disjunction(heads) => {
                // Only positive occurrences can produce possible atoms.
                // Neither default-negation mode supplies support.
                for head in heads.iter().filter_map(|head| head.positive_atom()) {
                    derivation.head(head, &binding, rule.location)?;
                }
            }
            HeadIr::ConditionalDisjunction { ordinary, elements } => {
                for head in ordinary.iter().filter_map(|head| head.positive_atom()) {
                    derivation.head(head, &binding, rule.location)?;
                }
                for element in elements {
                    let mut local = Join::local_head(
                        &element.condition,
                        &binding.prefix(element.outer_variables),
                        element.body_variables..element.variables,
                        support,
                        derivation.budget,
                        rule.location,
                    )?;
                    while let Some(binding) = local.next(
                        limits,
                        derivation.budget,
                        derivation.counters,
                        rule.location,
                    )? {
                        if let Some(head) = element.head.positive_atom() {
                            derivation.head(head, &binding, rule.location)?;
                        }
                    }
                }
            }
            HeadIr::Choice(group) => {
                crate::formula_head_aggregate::validate_group(
                    group,
                    &binding,
                    support,
                    limits,
                    derivation.budget,
                    derivation.counters,
                    rule.location,
                )?;
                for element in &group.elements {
                    let mut local = Join::element(
                        element,
                        &binding,
                        support,
                        derivation.budget,
                        rule.location,
                    )?;
                    while let Some(binding) = local.next(
                        limits,
                        derivation.budget,
                        derivation.counters,
                        rule.location,
                    )? {
                        if let Some(head) = element.head.positive_atom() {
                            derivation.head(head, &binding, rule.location)?;
                        }
                    }
                }
            }
            HeadIr::Normal(None) => unreachable!("constraints never produce support"),
        }
    }
    Ok(())
}

/// A support round publishes newly derived heads after complete checked binding.
/// Existing support and the current delta share one borrowed atom identity.
struct Derivation<'a, 'source> {
    support: &'a Support<'source>,
    delta: &'a mut BTreeSet<Atom>,
    limits: &'a FormulaLimits,
    budget: &'a mut Budget,
    counters: &'a mut Counters,
}

impl Derivation<'_, '_> {
    fn head(
        &mut self,
        pattern: &AtomPattern,
        binding: &Binding,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let key = pattern
            .key(binding.slots())
            .map_err(|error| FormulaFailure::UnsafeVariable {
                variable: error.variable,
                location,
            })?;
        if !self
            .support
            .contains(&key, self.limits, self.counters, location)?
            && key.get(self.delta).is_none()
        {
            ceiling(
                FormulaResource::Atoms,
                self.support.len() as u128 + self.delta.len() as u128 + 1,
                self.limits.theory.max_atoms as u128,
                location,
            )?;
            // A new atom's copied payload, index entry and catalog cell: the
            // cumulative allowance counts each atom once, not each proposal.
            let bytes = pattern.predicate().name().len() as u128
                + pattern
                    .terms()
                    .iter()
                    .map(|term| binding.resolve(term, location).map(value_bytes))
                    .sum::<Result<u128, _>>()?;
            self.budget.charge(
                ExpansionResource::ScalarBytes,
                bytes.saturating_mul(3),
                location,
            )?;
            self.delta.insert(key.to_atom());
        }
        Ok(())
    }
}

/// Validation evidence for one complete positive binding. A comparison that
/// is defined and false excludes the substitution before this certificate is
/// issued, so a certificate says at most that every comparison was decided
/// at its depth and passed. An arithmetic failure is retained until the
/// positive join has a complete extension no comparison excludes; an
/// incomplete prefix alone does not require evaluating a ground source
/// instance.
enum Comparisons {
    /// Some check waits for the complete row.
    Deferred,
    Failed(ExpansionFailure),
    /// Every comparison was decided at its depth, and passed.
    Verified,
}

// The same whole-argument index serves flat atoms and structural captures.
#[derive(Clone, Copy)]
enum PositivePattern<'a> {
    Flat(&'a AtomPattern),
    Structural(&'a crate::formula_pattern::PatternAtom),
}
impl PositivePattern<'_> {
    fn atom(&self) -> &AtomPattern {
        match self {
            Self::Flat(atom) => atom,
            Self::Structural(pattern) => &pattern.atom,
        }
    }
}

/// Source occurrence identity survives join reordering.
#[derive(Clone, Copy)]
struct PatternOccurrence<'a> {
    pattern: PositivePattern<'a>,
    source: usize,
}
impl PatternOccurrence<'_> {
    fn atom(&self) -> &AtomPattern {
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

/// The cursor owns its current assignment, undo trails and bounded expression
/// storage. Negative gates never restrict this upper relation; the emitted
/// formulas still retain them.
pub(crate) struct Join<'a, 'source> {
    bindings: Option<&'a crate::formula_assignment_plan::Plan>,
    literals: &'a [LiteralIr],
    generated: bool,
    /// The certificate of the last completed row, taken by its consumer.
    comparisons: Comparisons,
    /// What each prefix of the current order decides.
    decisions: order::Decisions,
    /// The conjunction of the comparisons decided up to each depth.
    verdicts: Vec<bool>,
    /// The first evaluation failure on the current prefix and the depth that
    /// met it; released when that depth is undone.
    failure: Option<(usize, ExpansionFailure)>,
    evaluation: Evaluation,
    pending: Option<crate::formula_binding_cursor::Cursor<'a, 'source>>,
    pending_head: Option<crate::formula_binding_cursor::Cursor<'a, 'source>>,
    head_slots: std::ops::Range<usize>,
    patterns: Vec<PatternOccurrence<'a>>,
    delta: Option<usize>,
    domains: Option<&'a queries::Guards<'a, 'source>>,
    support: &'a Support<'source>,
    values: Vec<Option<Value>>,
    slots: Vec<Slot>,
    positions: Vec<usize>,
    probes: Vec<Option<Probe<'a, 'source>>>,
    changes: Vec<Vec<usize>>,
    depth: usize,
    empty_yielded: bool,
    finished: bool,
}
/// One complete body binding and its scalar selection result. Selected rule
/// rows also contain their evaluated head suffix; rejected rows contain only
/// the genuine body frame needed by scoped source validation.
pub(crate) struct Row {
    pub values: Binding<'static>,
    pub passes: bool,
}

/// A row source retains its original occurrence indices. Indexed positions
/// count posting entries; table positions are the next source row to inspect.
enum Probe<'a, 'source> {
    Indexed(delta::Rows<'a>),
    Table(queries::Rows<'a, 'source>),
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
        budget: &mut Budget,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(
            &rule.body,
            &Binding::default(),
            rule.variables,
            support,
            budget,
            rule.location,
        )?;
        join.bindings = rule.bindings.as_ref();
        join.stage_head(rule.body_variables..rule.variables);
        Ok(join)
    }

    /// Attach necessary domains to their exact rule and support owner.
    pub(super) fn domain_rule(
        rule: &'a crate::formula_ir::RuleIr,
        support: &'a Support<'source>,
        domains: Option<&'a queries::Guards<'a, 'source>>,
        budget: &mut Budget,
    ) -> Result<Self, FormulaFailure> {
        if domains.is_some_and(|guards| !guards.belongs_to(rule, support)) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            });
        }
        let mut join = Self::rule(rule, support, budget)?;
        join.domains = domains;
        Ok(join)
    }

    pub(super) fn element(
        element: &'a crate::formula_ir::Element,
        prefix: &Binding,
        support: &'a Support<'source>,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        Self::local_head(
            &element.condition,
            prefix,
            element.body_variables..element.variables,
            support,
            budget,
            location,
        )
    }

    /// Choice and conditional heads share one condition/head frame boundary.
    pub(super) fn local_head(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        head_slots: std::ops::Range<usize>,
        support: &'a Support<'source>,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(literals, prefix, head_slots.end, support, budget, location)?;
        join.stage_head(head_slots);
        Ok(join)
    }

    fn stage_head(&mut self, slots: std::ops::Range<usize>) {
        self.values.truncate(slots.start);
        self.slots.truncate(slots.start);
        self.generated = self
            .slots
            .iter()
            .any(|slot| matches!(slot, Slot::Generated));
        self.head_slots = slots;
    }

    pub(crate) fn objective(
        objective: &'a crate::formula_ir::ObjectiveIr,
        support: &'a Support<'source>,
        budget: &mut Budget,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(
            objective.condition.literals(),
            &Binding::default(),
            objective.variables,
            support,
            budget,
            objective.location,
        )?;
        if let crate::formula_ir::ObjectiveCondition::Body { bindings, .. } = &objective.condition {
            join.bindings = bindings.as_ref();
        }
        Ok(join)
    }

    pub(super) fn component(
        literals: &'a [LiteralIr],
        variables: usize,
        used: &BTreeSet<usize>,
        fixed: &[Option<Value>],
        support: &'a Support<'source>,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(
            literals,
            &Binding::default(),
            variables,
            support,
            budget,
            location,
        )?;
        for (slot, fixed) in join.values.iter_mut().zip(fixed) {
            if let Some(value) = fixed {
                *slot = Some(copy(value, budget, location)?);
            }
        }
        for (variable, slot) in join.slots.iter_mut().enumerate() {
            if !used.contains(&variable) {
                *slot = Slot::Excluded;
            }
        }
        join.decide();
        Ok(join)
    }
    pub fn new(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'source>,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        // Universals have separate local scopes. Their conditions cannot bind
        // outer variables, and truth/vacuity over an incomplete support round
        // cannot prune possible heads. Only ordinary positive atoms join here.
        let mut patterns: Vec<_> = literals
            .iter()
            .enumerate()
            .filter_map(|(source, literal)| {
                let pattern = match literal {
                    LiteralIr::Atom(DefaultNegation::None, atom) => PositivePattern::Flat(atom),
                    LiteralIr::PatternAtom(pattern) => PositivePattern::Structural(pattern),
                    _ => return None,
                };
                Some(PatternOccurrence { pattern, source })
            })
            .collect();
        for pattern in &patterns {
            for term in pattern.atom().terms() {
                budget.charge(ExpansionResource::TermWork, 1, location)?;
                if let zetesis_core::Term::Variable(variable) = term
                    && prefix.slots().get(*variable).is_some_and(Option::is_none)
                {
                    return Err(FormulaFailure::UnsafeVariable {
                        variable: *variable,
                        location,
                    });
                }
            }
        }
        let mut bound: Vec<bool> = prefix.slots().iter().map(Option::is_some).collect();
        bound.resize(variables, false);
        order::arrange(
            &mut patterns,
            literals,
            &mut bound,
            |pattern| support.row_count(pattern.atom().predicate()),
            budget,
            location,
        )?;
        let count = patterns.len();
        let mut values = vec![None; variables];
        let mut slots = vec![Slot::Relational; variables];
        for target in literals
            .iter()
            .filter_map(crate::formula_binding_cursor::target)
        {
            slots[target] = Slot::Generated;
        }
        for (index, (slot, value)) in values.iter_mut().zip(prefix.slots()).enumerate() {
            *slot = value
                .as_ref()
                .map(|value| copy(value, budget, location))
                .transpose()?;
            if value.is_none() {
                // An enclosing generator's unavailable output is not a local
                // binding obligation. Any actual read still fails explicitly.
                slots[index] = Slot::Excluded;
            }
        }
        let prefix: Vec<bool> = values.iter().map(Option::is_some).collect();
        let decisions = order::Decisions::of(literals, &patterns, &prefix);
        Ok(Self {
            bindings: None,
            literals,
            generated: literals
                .iter()
                .any(|literal| crate::formula_binding_cursor::target(literal).is_some()),
            pending: None,
            pending_head: None,
            head_slots: variables..variables,
            comparisons: Comparisons::Deferred,
            decisions,
            verdicts: vec![true; count.max(1)],
            failure: None,
            evaluation: Evaluation::default(),
            patterns,
            delta: None,
            domains: None,
            support,
            values,
            slots,
            positions: vec![0; count],
            probes: std::iter::repeat_with(|| None).take(count).collect(),
            changes: vec![Vec::new(); count],
            depth: 0,
            empty_yielded: false,
            finished: false,
        })
    }
    /// Fix, for the current order and prefix, the depth at which each
    /// comparison is decided and whether any check waits for the complete
    /// row. Called after every arrangement and after the prefix is fixed.
    fn decide(&mut self) {
        let prefix: Vec<bool> = self.values.iter().map(Option::is_some).collect();
        self.decisions = order::Decisions::of(self.literals, &self.patterns, &prefix);
    }
    /// Restrict this round's join to the rows `variant` offers each source
    /// occurrence and order the join by those counts: the pivot occurrence
    /// offers only the round's new rows, so it is joined first when they are
    /// the fewest.
    fn partition(
        &mut self,
        variant: delta::Variant,
        budget: &mut Budget,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        self.delta = match variant {
            delta::Variant::Full => None,
            delta::Variant::Delta(pivot) => Some(pivot),
        };
        let mut bound: Vec<bool> = self.values.iter().map(Option::is_some).collect();
        let (support, delta) = (self.support, self.delta);
        order::arrange(
            &mut self.patterns,
            self.literals,
            &mut bound,
            |pattern| {
                let predicate = pattern.atom().predicate();
                let (old, total) = (support.old_rows(predicate), support.row_count(predicate));
                delta::interval(delta, pattern.source, old, total).len()
            },
            budget,
            location,
        )?;
        self.decide();
        Ok(())
    }
    pub fn next(
        &mut self,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        self.next_selected(None, limits, budget, counters, location)
    }
    /// Return every complete positive row for validation before selection.
    pub(crate) fn next_row(
        &mut self,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Row>, FormulaFailure> {
        self.next_staged(None, limits, budget, counters, location)
    }
    fn next_support(
        &mut self,
        head: &AtomPattern,
        delta: &BTreeSet<Atom>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        self.next_selected(Some((head, delta)), limits, budget, counters, location)
    }
    fn next_selected(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        while let Some(row) = self.next_staged(projected, limits, budget, counters, location)? {
            if row.passes {
                return Ok(Some(row.values));
            }
        }
        Ok(None)
    }
    /// Body data and scalar/range filters complete before head-only generation.
    /// Negative gates and aggregate truth remain formulas, never row selection.
    fn next_staged(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Row>, FormulaFailure> {
        loop {
            if let Some(pending) = &mut self.pending_head
                && let Some(values) =
                    pending.next(&mut self.evaluation, limits, budget, counters, location)?
            {
                return Ok(Some(Row {
                    values,
                    passes: true,
                }));
            }
            self.pending_head = None;
            let Some(row) = self.next_inner(projected, limits, budget, counters, location)? else {
                return Ok(None);
            };
            if !row.passes || self.head_slots.is_empty() {
                return Ok(Some(row));
            }
            self.pending_head = Some(crate::formula_binding_cursor::Cursor::new(
                self.literals,
                row.values,
                self.support,
                self.bindings,
                self.head_slots.clone(),
            ));
        }
    }
    fn next_inner(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Row>, FormulaFailure> {
        if !self.generated {
            if let Some(binding) = self.next_base(projected, limits, budget, counters, location)? {
                let comparisons = std::mem::replace(&mut self.comparisons, Comparisons::Deferred);
                let passes =
                    self.filters(&binding, comparisons, limits, budget, counters, location)?;
                return Ok(Some(Row {
                    values: binding,
                    passes,
                }));
            }
            return Ok(None);
        }
        loop {
            if let Some(pending) = &mut self.pending
                && let Some(binding) =
                    pending.next(&mut self.evaluation, limits, budget, counters, location)?
            {
                let passes = self.filters(
                    &binding,
                    Comparisons::Deferred,
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                return Ok(Some(Row {
                    values: binding,
                    passes,
                }));
            }
            let Some(binding) = self.next_base(projected, limits, budget, counters, location)?
            else {
                return Ok(None);
            };
            self.pending = Some(crate::formula_binding_cursor::Cursor::new(
                self.literals,
                binding,
                self.support,
                self.bindings,
                0..self.head_slots.start,
            ));
        }
    }
    fn next_base(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        self.comparisons = Comparisons::Deferred;
        if self.finished {
            return Ok(None);
        }
        loop {
            counters.work(limits, location)?;
            if self.skip_derived(projected, limits, counters, location)? {
                if self.finished {
                    return Ok(None);
                }
                continue;
            }
            if self.patterns.is_empty() {
                if self.empty_yielded {
                    self.finished = true;
                    return Ok(None);
                }
                self.empty_yielded = true;
                if !self.filter_prefix(limits, budget, counters, location)? {
                    continue;
                }
                self.comparisons = self.certificate();
                if let Some(binding) = self.complete(limits, budget, counters, location)? {
                    return Ok(Some(binding));
                }
                continue;
            }
            if self.depth == self.patterns.len() {
                // The certificate belongs to the returned binding snapshot,
                // even though completing that snapshot undoes the last row.
                self.comparisons = self.certificate();
                let complete = self.complete(limits, budget, counters, location)?;
                self.depth -= 1;
                self.undo();
                if complete.is_some() {
                    return Ok(complete);
                }
                continue;
            }
            let pattern = self.patterns[self.depth];
            if self.probes[self.depth].is_none() {
                self.probes[self.depth] = Some(
                    if let Some(rows) = self.support.select(
                        pattern.pattern,
                        &self.values,
                        limits,
                        counters,
                        location,
                    )? {
                        Probe::Table(rows)
                    } else {
                        let posting = self.support.probe(
                            pattern.atom(),
                            &self.values,
                            limits,
                            counters,
                            location,
                        )?;
                        let total = self.support.row_count(pattern.atom().predicate());
                        Probe::Indexed(if self.delta.is_some() {
                            let old = self.support.old_rows(pattern.atom().predicate());
                            let range = delta::interval(self.delta, pattern.source, old, total);
                            delta::Rows::within(posting, range, limits, counters, location)?
                        } else {
                            delta::Rows::all(posting, total)
                        })
                    },
                );
            }
            let row = self.probes[self.depth]
                .as_ref()
                .expect("prepared probe")
                .next(&mut self.positions[self.depth], limits, counters, location)?;
            let atom = row.and_then(|row| self.support.row(pattern.atom().predicate(), row));
            let Some(atom) = atom else {
                self.positions[self.depth] = 0;
                self.probes[self.depth] = None;
                if self.depth == 0 {
                    self.finished = true;
                    return Ok(None);
                }
                self.depth -= 1;
                self.undo();
                continue;
            };
            counters.record(Event::JoinRow);
            if let Some(domains) = self.domains
                && !domains.permits(pattern.source, atom.position(), limits, counters, location)?
            {
                continue;
            }
            let matches =
                self.match_row(pattern.pattern, atom, limits, budget, counters, location)?;
            if matches && self.filter_prefix(limits, budget, counters, location)? {
                self.depth += 1;
            } else {
                self.undo();
            }
        }
    }
    fn match_row(
        &mut self,
        pattern: PositivePattern<'_>,
        atom: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let mut matches = true;
        if let PositivePattern::Structural(pattern) = pattern {
            let delta = pattern.matches(
                atom,
                &self.values,
                &mut crate::formula_pattern::MatchContext {
                    limits,
                    budget,
                    counters,
                    location,
                },
            )?;
            if let Some(delta) = delta {
                crate::formula_pattern::reserve(
                    &mut self.changes[self.depth],
                    delta.len(),
                    budget,
                    location,
                )?;
                for (slot, value) in delta {
                    self.values[slot] = Some(value);
                    self.changes[self.depth].push(slot);
                }
            } else {
                matches = false;
            }
        } else {
            for (column, term) in pattern.atom().terms().iter().enumerate() {
                let value = atom.value(column).expect("checked pattern arity");
                let payload = match value {
                    Value::Structured(value) => value.payload_bytes(),
                    _ => 0,
                };
                counters.charge_work(1 + payload as u128, limits, location)?;
                match term {
                    zetesis_core::Term::Constant(constant) => matches &= constant == value,
                    zetesis_core::Term::Variable(variable) => {
                        if let Some(bound) = &self.values[*variable] {
                            matches &= bound == value;
                        } else {
                            self.values[*variable] = Some(copy(value, budget, location)?);
                            self.changes[self.depth].push(*variable);
                        }
                    }
                }
                if !matches {
                    break;
                }
            }
        }
        Ok(matches)
    }
    /// Evaluate the comparisons the current depth decides, once, and fold
    /// them into the prefix's verdict. A comparison that is defined and false
    /// excludes every substitution of the prefix, so the prefix is pruned and
    /// nothing beneath it is reached; an evaluation failure is retained, and
    /// becomes a refusal only if no comparison of the complete substitution
    /// excludes it.
    fn filter_prefix(
        &mut self,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let depth = self.depth;
        let mut passes = depth == 0 || self.verdicts[depth - 1];
        for index in self.decisions.decided_at(depth) {
            let (left, relation, right) =
                comparison(&self.literals[index]).expect("a decided literal is a comparison");
            let values = (|| {
                let left = partial_value(
                    left,
                    &self.values,
                    &mut self.evaluation,
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                let right = partial_value(
                    right,
                    &self.values,
                    &mut self.evaluation,
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                Ok((left, right))
            })();
            match values {
                Ok((left, right)) => passes &= compare(&left, relation, &right),
                Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. })) => {
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
    fn certificate(&mut self) -> Comparisons {
        if let Some((_, error)) = self.failure.take() {
            return Comparisons::Failed(error);
        }
        let depth = self.patterns.len().saturating_sub(1);
        if self.decisions.certifies(depth) {
            Comparisons::Verified
        } else {
            Comparisons::Deferred
        }
    }
    fn undo(&mut self) {
        for variable in self.changes[self.depth].drain(..) {
            self.values[variable] = None;
        }
        if self
            .failure
            .as_ref()
            .is_some_and(|(at, _)| *at >= self.depth)
        {
            self.failure = None;
        }
    }
    fn skip_derived(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        if !self.already_derived(projected, limits, counters, location)? {
            return Ok(false);
        }
        if self.depth == 0 {
            self.finished = true;
        } else {
            if self.depth < self.patterns.len() {
                self.positions[self.depth] = 0;
                self.probes[self.depth] = None;
            }
            self.depth -= 1;
            self.undo();
        }
        Ok(true)
    }
    fn already_derived(
        &self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let Some((head, delta)) = projected else {
            return Ok(false);
        };
        for term in head.terms() {
            counters.work(limits, location)?;
            if let zetesis_core::Term::Variable(variable) = term
                && self.values.get(*variable).is_none_or(Option::is_none)
            {
                return Ok(false);
            }
        }
        let key =
            head.key(self.values.as_slice())
                .map_err(|error| FormulaFailure::UnsafeVariable {
                    variable: error.variable,
                    location,
                })?;
        counters.work(limits, location)?;
        Ok(self.support.contains(&key, limits, counters, location)? || key.get(delta).is_some())
    }
    fn complete(
        &self,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        counters.substitution(limits, location)?;
        for (variable, (value, slot)) in self.values.iter().zip(&self.slots).enumerate() {
            if value.is_none() && matches!(slot, Slot::Relational) {
                return Err(FormulaFailure::UnsafeVariable { variable, location });
            }
        }
        let values = Binding::copy_slots(&self.values, limits, counters, budget, location)?;
        counters.record(Event::BindingSnapshot);
        Ok(Some(values))
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

fn partial_value(
    expression: &Expression,
    assignment: &[Option<Value>],
    evaluation: &mut Evaluation,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Value, FormulaFailure> {
    evaluation.expression(
        expression,
        |variable| {
            assignment[variable]
                .as_ref()
                .ok_or(FormulaFailure::UnsafeVariable { variable, location })
        },
        limits,
        budget,
        counters,
        location,
    )
}

impl Join<'_, '_> {
    fn filters(
        &mut self,
        assignment: &Binding,
        comparisons: Comparisons,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let mut passes = match comparisons {
            Comparisons::Verified | Comparisons::Deferred => true,
            Comparisons::Failed(error) => return Err(error.into()),
        };
        // Falsehood does not discharge another expression's validation duty.
        // These complete-row checks deliberately continue after a false filter.
        for (index, literal) in self.literals.iter().enumerate() {
            if crate::formula_binding_cursor::target(literal)
                .is_some_and(|target| target >= assignment.len())
            {
                continue;
            }
            if let Some((left, relation, right)) = comparison(literal) {
                // A comparison a prefix decided was evaluated there, once,
                // and passed, or the row would not be complete.
                if self.decisions.decides(index) {
                    continue;
                }
                let left = self.evaluation.expression(
                    left,
                    |variable| assignment.read(variable, location),
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                let right = self.evaluation.expression(
                    right,
                    |variable| assignment.read(variable, location),
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                passes &= compare(&left, relation, &right);
            } else if let LiteralIr::TupleCompare(left, relation, right) = literal {
                let mut equal = left.len() == right.len();
                // Unequal arities determine truth, not whether an existing
                // component's arithmetic must be validated.
                for index in 0..left.len().max(right.len()) {
                    let left = left
                        .get(index)
                        .map(|value| {
                            self.evaluation.expression(
                                value,
                                |variable| assignment.read(variable, location),
                                limits,
                                budget,
                                counters,
                                location,
                            )
                        })
                        .transpose()?;
                    let right = right
                        .get(index)
                        .map(|value| {
                            self.evaluation.expression(
                                value,
                                |variable| assignment.read(variable, location),
                                limits,
                                budget,
                                counters,
                                location,
                            )
                        })
                        .transpose()?;
                    equal &= left == right;
                }
                passes &= equal == (*relation == Relation::Eq);
            } else if let LiteralIr::Guard(guard) = literal {
                passes &= guard.evaluate(assignment, limits, budget, counters, location)?;
            } else if let LiteralIr::Range {
                target,
                lower,
                upper,
                binder: false,
            } = literal
            {
                let lower = self.evaluation.expression(
                    lower,
                    |variable| assignment.read(variable, location),
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                let upper = self.evaluation.expression(
                    upper,
                    |variable| assignment.read(variable, location),
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                let (Value::Number(lower), Value::Number(upper)) = (lower, upper) else {
                    passes = false;
                    continue;
                };
                passes &= matches!(assignment.read(*target, location)?, Value::Number(value) if *value >= lower && *value <= upper);
            }
        }
        Ok(passes)
    }
}
pub(crate) fn expression(
    expression: &Expression,
    assignment: &Binding,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Value, FormulaFailure> {
    Evaluation::default().expression(
        expression,
        |variable| assignment.read(variable, location),
        limits,
        budget,
        counters,
        location,
    )
}

pub(super) fn copy(
    value: &Value,
    budget: &mut Budget,
    location: Location,
) -> Result<Value, FormulaFailure> {
    budget.charge(ExpansionResource::ScalarBytes, value_bytes(value), location)?;
    Ok(value.clone())
}
fn numeric(value: &Value, location: Location) -> Result<i32, FormulaFailure> {
    match value {
        Value::Number(value) => Ok(*value),
        _ => Err(undefined(location)),
    }
}
fn scalar_value(
    result: Result<i32, EvalError>,
    location: Location,
) -> Result<Value, FormulaFailure> {
    let value = result.map_err(|error| ExpansionFailure::Evaluation { error, location })?;
    Ok(Value::Number(value))
}
pub(super) fn compare(left: &Value, relation: Relation, right: &Value) -> bool {
    let order = left.compare_terms(right);
    match relation {
        Relation::Eq => order.is_eq(),
        Relation::Neq => !order.is_eq(),
        Relation::Lt => order.is_lt(),
        Relation::Le => !order.is_gt(),
        Relation::Gt => order.is_gt(),
        Relation::Ge => !order.is_lt(),
    }
}

pub(super) fn undefined(location: Location) -> FormulaFailure {
    ExpansionFailure::Evaluation {
        error: EvalError::Undefined,
        location,
    }
    .into()
}
