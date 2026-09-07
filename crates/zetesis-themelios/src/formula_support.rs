//! A finite support upper bound and complete iterative relational joins.

use std::collections::{BTreeMap, BTreeSet};

use themelios_base::span::Location;
use themelios_program::program::{DefaultNegation, Relation};
use themelios_program::symbol::Symbol;
use themelios_program::term::{EvalError, Term};
use zetesis_core::{Atom, AtomPattern, Predicate, Value};

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, Prepared, value_bytes};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource};

#[derive(Default)]
pub(crate) struct Counters {
    pub work: u64,
    pub substitutions: u64,
    generated_values: BTreeSet<Value>,
}
impl Counters {
    pub fn work(
        &mut self,
        limits: FormulaLimits,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::Work,
            u128::from(self.work) + 1,
            u128::from(limits.max_work),
            location,
        )?;
        self.work += 1;
        Ok(())
    }
    pub(super) fn generated(
        &mut self,
        value: &Value,
        limits: FormulaLimits,
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
        limits: FormulaLimits,
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

#[derive(Default)]
pub(crate) struct Support {
    present: BTreeSet<Atom>,
    rows: BTreeMap<Predicate, RelationRows>,
    indexed_entries: usize,
}
#[derive(Default)]
struct RelationRows {
    atoms: Vec<Atom>,
    columns: Vec<BTreeMap<Value, Vec<usize>>>,
}
impl Support {
    pub(super) fn rows(&self, predicate: &Predicate) -> &[Atom] {
        self.rows
            .get(predicate)
            .map_or(&[], |relation| relation.atoms.as_slice())
    }
    fn insert(
        &mut self,
        atom: Atom,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::SupportIndexEntries,
            self.indexed_entries as u128 + atom.values().len() as u128,
            limits.max_support_index_entries as u128,
            location,
        )?;
        let relation = self.rows.entry(atom.predicate().clone()).or_default();
        if relation.columns.is_empty() {
            relation.columns = (0..atom.values().len()).map(|_| BTreeMap::new()).collect();
        }
        let row = relation.atoms.len();
        for (column, value) in relation.columns.iter_mut().zip(atom.values()) {
            counters.work(limits, location)?;
            if !column.contains_key(value) {
                column.insert(copy(value, budget, location)?, Vec::new());
            }
            column.get_mut(value).expect("index key inserted").push(row);
        }
        self.indexed_entries += atom.values().len();
        relation.atoms.push(atom.clone());
        self.present.insert(atom);
        Ok(())
    }
    fn probe(
        &self,
        pattern: &AtomPattern,
        values: &[Option<Value>],
        limits: FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<&[usize]>, FormulaFailure> {
        let Some(relation) = self.rows.get(pattern.predicate()) else {
            return Ok(Some(&[]));
        };
        let mut selected: Option<&[usize]> = None;
        for (column, term) in relation.columns.iter().zip(pattern.terms()) {
            counters.work(limits, location)?;
            let value = match term {
                zetesis_core::Term::Constant(value) => Some(value),
                zetesis_core::Term::Variable(variable) => values[*variable].as_ref(),
            };
            if let Some(value) = value {
                let rows = column.get(value).map_or(&[][..], Vec::as_slice);
                if selected.is_none_or(|selected| rows.len() < selected.len()) {
                    selected = Some(rows);
                }
            }
        }
        Ok(selected)
    }
}

pub(crate) fn build(
    prepared: &Prepared,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    fallback: Location,
) -> Result<Support, FormulaFailure> {
    let mut support = Support::default();
    let mut rounds = 0_u64;
    loop {
        ceiling(
            FormulaResource::SupportRounds,
            u128::from(rounds) + 1,
            u128::from(limits.max_support_rounds),
            fallback,
        )?;
        rounds += 1;
        let mut delta = BTreeSet::new();
        for rule in &prepared.rules {
            if matches!(rule.head, HeadIr::Normal(None)) {
                continue;
            }
            let mut outer = Join::new(
                &rule.body,
                &[],
                rule.variables,
                &support,
                budget,
                rule.location,
            )?;
            while let Some(binding) = match &rule.head {
                HeadIr::Normal(Some(head)) => {
                    outer.next_support(head, &delta, limits, budget, counters, rule.location)?
                }
                _ => outer.next(limits, budget, counters, rule.location)?,
            } {
                match &rule.head {
                    HeadIr::Normal(Some(head)) => derive(
                        head,
                        &binding,
                        &support,
                        &mut delta,
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
                                &support,
                                &mut delta,
                                limits,
                                budget,
                                rule.location,
                            )?;
                        }
                    }
                    HeadIr::Choice { elements, .. } => {
                        crate::formula_count_head::validate_group(
                            elements,
                            &binding,
                            &support,
                            limits,
                            budget,
                            counters,
                            rule.location,
                        )?;
                        for element in elements {
                            let mut local = Join::new(
                                &element.condition,
                                &binding,
                                element.variables,
                                &support,
                                budget,
                                rule.location,
                            )?;
                            while let Some(binding) =
                                local.next(limits, budget, counters, rule.location)?
                            {
                                derive(
                                    &element.head,
                                    &binding,
                                    &support,
                                    &mut delta,
                                    limits,
                                    budget,
                                    rule.location,
                                )?;
                            }
                        }
                    }
                    HeadIr::Normal(None) => unreachable!("constraints never produce support"),
                }
            }
        }
        if delta.is_empty() {
            return Ok(support);
        }
        for atom in delta {
            support.insert(atom, limits, budget, counters, fallback)?;
        }
    }
}

fn derive(
    pattern: &AtomPattern,
    binding: &[Value],
    support: &Support,
    delta: &mut BTreeSet<Atom>,
    limits: FormulaLimits,
    budget: &mut Budget,
    location: Location,
) -> Result<(), FormulaFailure> {
    let bytes = pattern.predicate().name().len() as u128
        + pattern
            .terms()
            .iter()
            .map(|term| value_bytes(term.resolve(binding).expect("safe variable assigned")))
            .sum::<u128>();
    budget.charge(
        ExpansionResource::ScalarBytes,
        bytes.saturating_mul(3),
        location,
    )?;
    let atom = pattern
        .instantiate(binding)
        .expect("safe variable assigned");
    if !support.present.contains(&atom) && !delta.contains(&atom) {
        ceiling(
            FormulaResource::Atoms,
            support.present.len() as u128 + delta.len() as u128 + 1,
            limits.theory.max_atoms as u128,
            location,
        )?;
        delta.insert(atom);
    }
    Ok(())
}

/// Outcome of all ordinary comparisons on one partial binding. Deferred scalar
/// errors and unbound arguments cannot certify the final immutable binding.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Comparisons {
    Rejected,
    Deferred,
    Verified,
}

/// The cursor owns only its current assignment and undo trails. Negative gates
/// never restrict this upper relation; the emitted formulas still retain them.
pub(crate) struct Join<'a> {
    literals: &'a [LiteralIr],
    generated: bool,
    comparisons: Comparisons,
    pending: Option<crate::formula_binding_cursor::Cursor<'a>>,
    patterns: Vec<&'a AtomPattern>,
    support: &'a Support,
    values: Vec<Option<Value>>,
    generated_slots: Vec<bool>,
    positions: Vec<usize>,
    probes: Vec<Option<&'a [usize]>>,
    probed: Vec<bool>,
    changes: Vec<Vec<usize>>,
    depth: usize,
    empty_yielded: bool,
    finished: bool,
}
impl<'a> Join<'a> {
    pub(super) fn component(
        literals: &'a [LiteralIr],
        variables: usize,
        used: &BTreeSet<usize>,
        fixed: &[Option<Value>],
        support: &'a Support,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let mut join = Self::new(literals, &[], variables, support, budget, location)?;
        for (slot, fixed) in join.values.iter_mut().zip(fixed) {
            if let Some(value) = fixed {
                *slot = Some(copy(value, budget, location)?);
            }
        }
        for (variable, placeholder) in join.generated_slots.iter_mut().enumerate() {
            if !used.contains(&variable) {
                *placeholder = true;
            }
        }
        Ok(join)
    }
    pub fn new(
        literals: &'a [LiteralIr],
        prefix: &[Value],
        variables: usize,
        support: &'a Support,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        // Universals have separate local scopes. Their conditions cannot bind
        // outer variables, and truth/vacuity over an incomplete support round
        // cannot prune possible heads. Only ordinary positive atoms join here.
        let mut patterns: Vec<_> = literals
            .iter()
            .filter_map(|literal| match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => Some(atom),
                _ => None,
            })
            .collect();
        patterns.sort_by_key(|pattern| support.rows(pattern.predicate()).len());
        let count = patterns.len();
        let mut values = vec![None; variables];
        let mut generated_slots = vec![false; variables];
        for target in literals
            .iter()
            .filter_map(crate::formula_binding_cursor::target)
        {
            generated_slots[target] = true;
        }
        for (slot, value) in values.iter_mut().zip(prefix) {
            *slot = Some(copy(value, budget, location)?);
        }
        Ok(Self {
            literals,
            generated: literals
                .iter()
                .any(|literal| crate::formula_binding_cursor::target(literal).is_some()),
            pending: None,
            comparisons: Comparisons::Deferred,
            patterns,
            support,
            values,
            generated_slots,
            positions: vec![0; count],
            probes: vec![None; count],
            probed: vec![false; count],
            changes: vec![Vec::new(); count],
            depth: 0,
            empty_yielded: false,
            finished: false,
        })
    }
    pub fn next(
        &mut self,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Vec<Value>>, FormulaFailure> {
        self.next_inner(None, limits, budget, counters, location)
    }
    fn next_support(
        &mut self,
        head: &AtomPattern,
        delta: &BTreeSet<Atom>,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Vec<Value>>, FormulaFailure> {
        self.next_inner(Some((head, delta)), limits, budget, counters, location)
    }
    fn next_inner(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Vec<Value>>, FormulaFailure> {
        if !self.generated {
            while let Some(binding) =
                self.next_base(projected, limits, budget, counters, location)?
            {
                if filters(
                    self.literals,
                    &binding,
                    self.comparisons,
                    limits,
                    budget,
                    counters,
                    location,
                )? {
                    return Ok(Some(binding));
                }
            }
            return Ok(None);
        }
        loop {
            if let Some(pending) = &mut self.pending {
                while let Some(binding) = pending.next(limits, budget, counters, location)? {
                    if filters(
                        self.literals,
                        &binding,
                        Comparisons::Deferred,
                        limits,
                        budget,
                        counters,
                        location,
                    )? {
                        return Ok(Some(binding));
                    }
                }
            }
            let Some(binding) = self.next_base(projected, limits, budget, counters, location)?
            else {
                return Ok(None);
            };
            self.pending = Some(crate::formula_binding_cursor::Cursor::new(
                self.literals,
                binding,
                self.support,
            ));
        }
    }
    fn next_base(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Vec<Value>>, FormulaFailure> {
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
                if !self.filter_prefix(limits, budget, counters, location)? {
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
            if !self.probed[self.depth] {
                self.probes[self.depth] =
                    self.support
                        .probe(pattern, &self.values, limits, counters, location)?;
                self.probed[self.depth] = true;
            }
            let position = self.positions[self.depth];
            let row =
                self.probes[self.depth].map_or(Some(position), |rows| rows.get(position).copied());
            let atom = row.and_then(|row| self.support.rows(pattern.predicate()).get(row));
            let Some(atom) = atom else {
                self.positions[self.depth] = 0;
                self.probed[self.depth] = false;
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
            let mut matches = true;
            for (term, value) in pattern.terms().iter().zip(atom.values()) {
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
            if matches && self.filter_prefix(limits, budget, counters, location)? {
                self.depth += 1;
            } else {
                self.undo();
            }
        }
    }
    fn filter_prefix(
        &mut self,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        self.comparisons = partial_filters(
            self.literals,
            &self.values,
            limits,
            budget,
            counters,
            location,
        )?;
        Ok(self.comparisons != Comparisons::Rejected)
    }
    fn undo(&mut self) {
        for variable in self.changes[self.depth].drain(..) {
            self.values[variable] = None;
        }
    }
    fn skip_derived(
        &mut self,
        projected: Option<(&AtomPattern, &BTreeSet<Atom>)>,
        limits: FormulaLimits,
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
                self.probed[self.depth] = false;
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
        limits: FormulaLimits,
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
                && self.values[*variable].is_none()
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
        Ok(self.support.present.contains(&atom) || delta.contains(&atom))
    }
    fn complete(
        &self,
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Vec<Value>>, FormulaFailure> {
        counters.substitution(limits, location)?;
        let values = self
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if let Some(value) = value {
                    copy(value, budget, location)
                } else {
                    assert!(self.generated_slots[index], "positive binders cover scope");
                    Ok(Value::Number(0))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(values))
    }
}

fn partial_filters(
    literals: &[LiteralIr],
    assignment: &[Option<Value>],
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Comparisons, FormulaFailure> {
    let mut result = Comparisons::Verified;
    for literal in literals {
        if let LiteralIr::Compare(left, relation, right) = literal {
            if !bound(left, assignment, limits, counters, location)?
                || !bound(right, assignment, limits, counters, location)?
            {
                result = Comparisons::Deferred;
                continue;
            }
            let Some(left) = partial_value(left, assignment, limits, budget, counters, location)?
            else {
                result = Comparisons::Deferred;
                continue;
            };
            let Some(right) = partial_value(right, assignment, limits, budget, counters, location)?
            else {
                result = Comparisons::Deferred;
                continue;
            };
            if !compare(&left, *relation, &right) {
                return Ok(Comparisons::Rejected);
            }
        }
    }
    Ok(result)
}

fn partial_value(
    expression: &Expression,
    assignment: &[Option<Value>],
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Option<Value>, FormulaFailure> {
    match expression_from(
        expression,
        |variable| assignment[variable].as_ref().expect("ready expression"),
        limits,
        budget,
        counters,
        location,
    ) {
        Ok(value) => Ok(Some(value)),
        // An invalid prefix can have no complete relational extension. Only
        // final-row evaluation may turn scalar undefinedness into a refusal.
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })) => Ok(None),
        Err(error) => Err(error),
    }
}

fn bound(
    expression: &Expression,
    assignment: &[Option<Value>],
    limits: FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    for operation in &expression.nodes {
        counters.work(limits, location)?;
        if let Operation::Variable(variable) = operation
            && assignment[*variable].is_none()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn filters(
    literals: &[LiteralIr],
    assignment: &[Value],
    comparisons: Comparisons,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    let mut passes = true;
    for literal in literals {
        if let LiteralIr::Compare(left, relation, right) = literal
            && comparisons != Comparisons::Verified
        {
            let left = expression(left, assignment, limits, budget, counters, location)?;
            let right = expression(right, assignment, limits, budget, counters, location)?;
            passes &= compare(&left, *relation, &right);
        } else if let LiteralIr::TupleCompare(left, relation, right) = literal {
            let mut equal = left.len() == right.len();
            for (left, right) in left.iter().zip(right) {
                let left = expression(left, assignment, limits, budget, counters, location)?;
                let right = expression(right, assignment, limits, budget, counters, location)?;
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
            let lower = expression(lower, assignment, limits, budget, counters, location)?;
            let upper = expression(upper, assignment, limits, budget, counters, location)?;
            let (Value::Number(lower), Value::Number(upper)) = (lower, upper) else {
                passes = false;
                continue;
            };
            passes &= matches!(&assignment[*target], Value::Number(value) if *value >= lower && *value <= upper);
        }
    }
    Ok(passes)
}
pub(crate) fn expression(
    expression: &Expression,
    assignment: &[Value],
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Value, FormulaFailure> {
    expression_from(
        expression,
        |variable| &assignment[variable],
        limits,
        budget,
        counters,
        location,
    )
}

fn expression_from<'a>(
    expression: &Expression,
    variable: impl Fn(usize) -> &'a Value,
    limits: FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<Value, FormulaFailure> {
    let mut values: Vec<Value> = Vec::new();
    for node in &expression.nodes {
        counters.work(limits, location)?;
        let value = match *node {
            Operation::Constant(ref value) => copy(value, budget, location)?,
            Operation::Variable(index) => copy(variable(index), budget, location)?,
            Operation::Unary(operator, argument) => evaluate(
                &Term::UnaryOperation {
                    operator,
                    argument: Box::new(numeric(&values[argument], location)?),
                },
                location,
            )?,
            Operation::Binary(operator, left, right) => evaluate(
                &Term::BinaryOperation {
                    operator,
                    left: Box::new(numeric(&values[left], location)?),
                    right: Box::new(numeric(&values[right], location)?),
                },
                location,
            )?,
            Operation::Absolute(argument) => evaluate(
                &Term::Absolute(Box::new(numeric(&values[argument], location)?)),
                location,
            )?,
        };
        if let Value::Structured(structure) = &value {
            for _ in 0..structure.payload_bytes() {
                counters.work(limits, location)?;
            }
        }
        values.push(value);
    }
    Ok(values.pop().expect("expression has root"))
}
pub(super) fn copy(
    value: &Value,
    budget: &mut Budget,
    location: Location,
) -> Result<Value, FormulaFailure> {
    budget.charge(ExpansionResource::ScalarBytes, value_bytes(value), location)?;
    Ok(value.clone())
}
fn numeric(value: &Value, location: Location) -> Result<Term, FormulaFailure> {
    match value {
        Value::Number(value) => Ok(Term::Symbolic(Symbol::Number(*value))),
        _ => Err(undefined(location)),
    }
}
fn evaluate(term: &Term, location: Location) -> Result<Value, FormulaFailure> {
    let value = term
        .evaluate()
        .map_err(|error| ExpansionFailure::Evaluation { error, location })?;
    crate::compile::scalar(&value, location).map_err(Into::into)
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
