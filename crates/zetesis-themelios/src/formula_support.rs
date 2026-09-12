//! A finite support upper bound and complete iterative relational joins.

mod evaluation;
mod delta;
mod relations;
#[cfg(test)]
mod columnar;
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
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, Prepared, value_bytes};
use crate::grounding_observer::{Event, Work};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

pub(crate) use evaluation::Evaluation;
#[cfg(test)]
use relations::RelationRows;
pub(crate) use relations::{Support, SupportCatalog};

/// Possible atoms after a complete support round added no new head.
///
/// Construction is private to `build`: reaching a resource ceiling never
/// produces this owner. Its finite fixed point is an observed result, not a
/// promise that recursive value generation terminates for every source.
pub(crate) struct CompletedCatalog {
    catalog: SupportCatalog,
}

/// An immutable relation view of one successfully completed support owner.
/// Intermediate round snapshots deliberately have only the `Support` type.
pub(crate) struct CompletedSupport<'source> {
    relations: Support<'source>,
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

impl<'source> CompletedSupport<'source> {
    /// Borrow the same typed rows used by final grounding and objective joins.
    pub(crate) fn relations(&self) -> &Support<'source> {
        &self.relations
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
                FormulaResource::AssignmentValues,
                self.generated_values.len() as u128 + 1,
                limits.max_assignment_values as u128,
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
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    fallback: Location,
) -> Result<CompletedCatalog, FormulaFailure> {
    let mut catalog = SupportCatalog::default();
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
        let support = catalog.snapshot(limits, counters, fallback)?;
        let mut delta = BTreeSet::new();
        for rule in &prepared.rules {
            if matches!(rule.head, HeadIr::Normal(None)) {
                continue;
            }
            let mut variants = delta::variants(rule, &support, rounds == 1, limits, counters)?;
            while let Some(variant) = variants.next(limits, counters)? {
                let mut outer = Join::rule(rule, &support, budget)?;
                outer.delta = match variant {
                    delta::Variant::Full => None,
                    delta::Variant::Delta(pivot) => Some(pivot),
                };
                derive_rule(
                    rule, &mut outer, &support, &mut delta, limits, budget, counters,
                )?;
            }
        }
        drop(support);
        if delta.is_empty() {
            return Ok(CompletedCatalog { catalog });
        }
        catalog.advance(limits, counters, fallback)?;
        for atom in delta {
            catalog = catalog.insert(atom, limits, counters, fallback)?;
        }
    }
}

fn derive_rule(
    rule: &crate::formula_ir::RuleIr,
    outer: &mut Join<'_>,
    support: &Support<'_>,
    delta: &mut BTreeSet<Atom>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<(), FormulaFailure> {
    while let Some(binding) = match &rule.head {
        HeadIr::Normal(Some(head)) => {
            outer.next_support(head, delta, limits, budget, counters, rule.location)?
        }
        _ => outer.next(limits, budget, counters, rule.location)?,
    } {
        match &rule.head {
            HeadIr::Normal(Some(head)) => derive(
                head,
                &binding,
                support,
                delta,
                limits,
                budget,
                rule.location,
            )?,
            HeadIr::Disjunction(heads) => {
                // Only positive occurrences can produce possible atoms.
                // Neither default-negation mode supplies support.
                for head in heads.iter().filter_map(|head| head.positive_atom()) {
                    derive(
                        head,
                        &binding,
                        support,
                        delta,
                        limits,
                        budget,
                        rule.location,
                    )?;
                }
            }
            HeadIr::Choice(group) => {
                crate::formula_head_aggregate::validate_group(
                    group,
                    &binding,
                    support,
                    limits,
                    budget,
                    counters,
                    rule.location,
                )?;
                for element in &group.elements {
                    let mut local =
                        Join::element(element, &binding, support, budget, rule.location)?;
                    while let Some(binding) = local.next(limits, budget, counters, rule.location)? {
                        if let Some(head) = element.head.positive_atom() {
                            derive(
                                head,
                                &binding,
                                support,
                                delta,
                                limits,
                                budget,
                                rule.location,
                            )?;
                        }
                    }
                }
            }
            HeadIr::Normal(None) => unreachable!("constraints never produce support"),
        }
    }
    Ok(())
}

fn derive(
    pattern: &AtomPattern,
    binding: &Binding,
    support: &Support,
    delta: &mut BTreeSet<Atom>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    location: Location,
) -> Result<(), FormulaFailure> {
    let bytes = pattern.predicate().name().len() as u128
        + pattern
            .terms()
            .iter()
            .map(|term| binding.resolve(term, location).map(value_bytes))
            .sum::<Result<u128, _>>()?;
    budget.charge(
        ExpansionResource::ScalarBytes,
        bytes.saturating_mul(3),
        location,
    )?;
    let atom = binding.instantiate(pattern, location)?;
    if !support.contains(&atom) && !delta.contains(&atom) {
        ceiling(
            FormulaResource::Atoms,
            support.len() as u128 + delta.len() as u128 + 1,
            limits.theory.max_atoms as u128,
            location,
        )?;
        delta.insert(atom);
    }
    Ok(())
}

/// Validation evidence for one partial positive binding. A false comparison
/// can prune only when every scalar check is defined. An arithmetic failure is
/// retained until the positive join has a complete extension; an incomplete
/// prefix alone does not require evaluating a ground source instance.
enum Comparisons {
    Deferred,
    Failed(ExpansionFailure),
    Verified(bool),
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

/// Source occurrence identity survives cardinality-based join reordering.
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
pub(crate) struct Join<'a> {
    bindings: Option<&'a crate::formula_assignment_plan::Plan>,
    literals: &'a [LiteralIr],
    generated: bool,
    comparisons: Comparisons,
    evaluation: Evaluation,
    pending: Option<crate::formula_binding_cursor::Cursor<'a>>,
    pending_head: Option<crate::formula_binding_cursor::Cursor<'a>>,
    head_slots: std::ops::Range<usize>,
    patterns: Vec<PatternOccurrence<'a>>,
    delta: Option<usize>,
    support: &'a Support<'a>,
    values: Vec<Option<Value>>,
    slots: Vec<Slot>,
    positions: Vec<usize>,
    probes: Vec<Option<delta::Rows<'a>>>,
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

impl<'a> Join<'a> {
    pub(super) fn rule(
        rule: &'a crate::formula_ir::RuleIr,
        support: &'a Support<'a>,
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

    pub(super) fn element(
        element: &'a crate::formula_ir::Element,
        prefix: &Binding,
        support: &'a Support<'a>,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(
            &element.condition,
            prefix,
            element.variables,
            support,
            budget,
            location,
        )?;
        join.stage_head(element.body_variables..element.variables);
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
        support: &'a Support<'a>,
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
        support: &'a Support<'a>,
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
        Ok(join)
    }
    pub fn new(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &'a Support<'a>,
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
        patterns.sort_by_key(|pattern| support.row_count(pattern.atom().predicate()));
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
            evaluation: Evaluation::default(),
            patterns,
            delta: None,
            support,
            values,
            slots,
            positions: vec![0; count],
            probes: vec![None; count],
            changes: vec![Vec::new(); count],
            depth: 0,
            empty_yielded: false,
            finished: false,
        })
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
        self.next_staged(None, true, limits, budget, counters, location)
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
        while let Some(row) =
            self.next_staged(projected, false, limits, budget, counters, location)?
        {
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
        retain_rejected: bool,
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
            let Some(row) = self.next_inner(
                projected,
                retain_rejected,
                limits,
                budget,
                counters,
                location,
            )?
            else {
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
        retain_rejected: bool,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Row>, FormulaFailure> {
        if !self.generated {
            if let Some(binding) = self.next_base(
                projected,
                retain_rejected,
                limits,
                budget,
                counters,
                location,
            )? {
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
            let Some(binding) = self.next_base(
                projected,
                retain_rejected,
                limits,
                budget,
                counters,
                location,
            )?
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
        retain_rejected: bool,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        // This certificate belongs to the next returned binding snapshot, even
        // though completing that snapshot undoes the last mutable join row.
        self.comparisons = Comparisons::Deferred;
        if self.finished {
            return Ok(None);
        }
        loop {
            counters.work(limits, location)?;
            if self.skip_derived(projected, limits, budget, counters, location)? {
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
                if !self.filter_prefix(limits, budget, counters, location)? && !retain_rejected {
                    continue;
                }
                if let Some(binding) = self.complete(limits, budget, counters, location)? {
                    return Ok(Some(binding));
                }
                continue;
            }
            if self.depth == self.patterns.len() {
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
                let posting =
                    self.support
                        .probe(pattern.atom(), &self.values, limits, counters, location)?;
                let total = self.support.row_count(pattern.atom().predicate());
                self.probes[self.depth] = Some(if self.delta.is_some() {
                    let old = self.support.old_rows(pattern.atom().predicate());
                    let range = delta::interval(self.delta, pattern.source, old, total);
                    delta::Rows::within(posting, range, limits, counters, location)?
                } else {
                    delta::Rows::all(posting, total)
                });
            }
            let position = self.positions[self.depth];
            let row = self.probes[self.depth]
                .as_ref()
                .and_then(|rows| rows.get(position));
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
            self.positions[self.depth] += 1;
            counters.record(Event::JoinRow);
            let matches =
                self.match_row(pattern.pattern, atom, limits, budget, counters, location)?;
            if matches
                && (self.filter_prefix(limits, budget, counters, location)? || retain_rejected)
            {
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
                counters.work(limits, location)?;
                if let Value::Structured(value) = value {
                    for _ in 0..value.payload_bytes() {
                        counters.work(limits, location)?;
                    }
                }
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
    fn filter_prefix(
        &mut self,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        self.comparisons = partial_filters(
            self.literals,
            &self.values,
            &mut self.evaluation,
            limits,
            budget,
            counters,
            location,
        )?;
        Ok(!matches!(self.comparisons, Comparisons::Verified(false)))
    }
    fn undo(&mut self) {
        for variable in self.changes[self.depth].drain(..) {
            self.values[variable] = None;
        }
    }
    fn skip_derived(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        if !self.already_derived(projected, limits, budget, counters, location)? {
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
        budget: &mut Budget,
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
        budget.charge(
            ExpansionResource::ScalarBytes,
            head.predicate().name().len() as u128,
            location,
        )?;
        let mut values = Vec::new();
        for term in head.terms() {
            let value = match term {
                zetesis_core::Term::Constant(value) => value,
                zetesis_core::Term::Variable(variable) => self.values[*variable]
                    .as_ref()
                    .expect("head variables ready"),
            };
            values.push(copy(value, budget, location)?);
        }
        let atom = Atom::new(head.predicate().clone(), values).expect("validated head arity");
        counters.work(limits, location)?;
        Ok(self.support.contains(&atom) || delta.contains(&atom))
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

fn partial_filters(
    literals: &[LiteralIr],
    assignment: &[Option<Value>],
    evaluation: &mut Evaluation,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Comparisons, FormulaFailure> {
    let mut deferred = false;
    let mut passes = true;
    for literal in literals {
        if crate::formula_binding_cursor::target(literal)
            .is_some_and(|target| target >= assignment.len())
        {
            continue;
        }
        if let Some((left, relation, right)) = comparison(literal) {
            if !bound(left, assignment, limits, counters, location)?
                || !bound(right, assignment, limits, counters, location)?
            {
                deferred = true;
                continue;
            }
            let values = (|| {
                let left = partial_value(
                    left, assignment, evaluation, limits, budget, counters, location,
                )?;
                let right = partial_value(
                    right, assignment, evaluation, limits, budget, counters, location,
                )?;
                Ok((left, right))
            })();
            match values {
                Ok((left, right)) => passes &= compare(&left, relation, &right),
                Err(FormulaFailure::Expansion(error @ ExpansionFailure::Evaluation { .. })) => {
                    return Ok(Comparisons::Failed(error));
                }
                Err(error) => return Err(error),
            }
        } else if !matches!(
            literal,
            LiteralIr::Atom(..) | LiteralIr::PatternAtom(_) | LiteralIr::ProjectedAtom(..)
        ) {
            // Tuple/whole guards, range checks and generated values are validated
            // by their complete-row operations. An ordinary comparison cannot
            // certify that these independent checks have succeeded.
            deferred = true;
        }
    }
    Ok(if deferred {
        Comparisons::Deferred
    } else {
        Comparisons::Verified(passes)
    })
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

fn bound(
    expression: &Expression,
    assignment: &[Option<Value>],
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    for operation in &expression.nodes {
        counters.work(limits, location)?;
        counters.record(Event::ReadinessNode);
        if let Operation::Variable(variable) = operation
            && assignment[*variable].is_none()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

impl Join<'_> {
    fn filters(
        &mut self,
        assignment: &Binding,
        comparisons: Comparisons,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let (verified, mut passes) = match comparisons {
            Comparisons::Verified(passes) => (true, passes),
            Comparisons::Deferred => (false, true),
            Comparisons::Failed(error) => return Err(error.into()),
        };
        // Falsehood does not discharge another expression's validation duty.
        // These complete-row checks deliberately continue after a false filter.
        for literal in self.literals {
            if crate::formula_binding_cursor::target(literal)
                .is_some_and(|target| target >= assignment.len())
            {
                continue;
            }
            if let Some((left, relation, right)) = comparison(literal)
                && !verified
            {
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
