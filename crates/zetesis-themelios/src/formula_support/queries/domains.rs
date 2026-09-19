//! Rule-local candidates, resolved into the completed owner's dictionary.
//!
//! A variable's candidates are the meet of its argument domains, less every
//! value a comparison over that variable alone is defined and false at. The
//! candidates that remain are exactly the values the exclusion rule leaves for
//! the variable, decided before any row is read. A guard is prepared only
//! where the candidates are fewer than the
//! argument's domain, since a relation offers no value outside its domain.

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::mem::size_of;

use themelios_program::program::{DefaultNegation, Relation as Comparison};
use themelios_program::symbol::{Sign as SourceSign, Symbol};
use zetesis_core::relation::{Failure, Relation};
use zetesis_core::{AtomPattern, Sign, Term, Value};
use zetesis_domain::{Analysis, Domain};

use super::super::evaluation::Evaluation;
use super::{Counters, Event, FormulaFailure, FormulaLimits, Location, Support};
use crate::expansion::Budget;
use crate::formula_ir::{Expression, LiteralIr, RuleIr};
use crate::{ExpansionFailure, ExpansionResource};

type Values<'a> = Vec<&'a Symbol>;

/// One variable argument of a positive body atom: where it occurs, the
/// variable it names, and the size of the argument's finite domain when the
/// analysis knows one.
struct Occurrence<'a> {
    literal: usize,
    pattern: &'a AtomPattern,
    column: usize,
    slot: usize,
    width: Option<usize>,
}

/// A comparison that reads one variable alone, with that variable.
struct Unary<'a> {
    left: &'a Expression,
    relation: Comparison,
    right: &'a Expression,
    slot: usize,
}

struct Restriction<'a, 'source> {
    occurrence: usize,
    relation: &'a Relation<'source>,
    column: usize,
    ids: Vec<u32>,
}

/// Immutable restrictions for exactly one rule and one support owner, applied
/// in each completion round and in the final instantiation alike.
/// The lease includes live guards and preparation scratch beside all table masks.
pub(crate) struct Guards<'a, 'source> {
    support: &'a Support<'source>,
    rule: &'a RuleIr,
    restrictions: Vec<Restriction<'a, 'source>>,
    bytes: usize,
}

/// The narrowed candidates of one rule's variables: a property of the rule
/// and the analysis alone, prepared once and resolved into every completion
/// snapshot and the final one. Its storage is bounded by the analysis's value
/// entries, as the analysis heap is, and lies outside the support allowance.
pub(crate) struct Candidates<'source> {
    /// Each variable's candidates, in canonical order; `None` when no
    /// argument the variable occurs at has a finite domain, so every value
    /// remains.
    by_variable: Vec<Option<Values<'source>>>,
    occurrences: Vec<Occurrence<'source>>,
}

impl<'source> Candidates<'source> {
    pub(crate) fn prepare(
        rule: &'source RuleIr,
        analysis: &Analysis<'source>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let mut by_variable = Vec::new();
        for _ in 0..rule.variables {
            counters.work(limits, rule.location)?;
            by_variable.push(None);
        }
        let mut candidates = Self {
            by_variable,
            occurrences: Vec::new(),
        };
        for (literal, element) in rule.body.iter().enumerate() {
            // A comparison binds nothing and offers no row; it narrows the
            // candidates below, once every meet is taken.
            if matches!(element, LiteralIr::Compare(..)) {
                continue;
            }
            let LiteralIr::Atom(DefaultNegation::None, pattern) = element else {
                return Err(failure(Failure::Predicate, rule.location));
            };
            for (column, term) in pattern.terms().iter().enumerate() {
                counters.work(limits, rule.location)?;
                let Term::Variable(slot) = term else {
                    continue;
                };
                let domain = argument(
                    analysis,
                    pattern.predicate(),
                    column,
                    limits,
                    counters,
                    rule.location,
                )?;
                let variable_candidates = candidates.by_variable.get_mut(*slot).ok_or(
                    FormulaFailure::UnsafeVariable {
                        variable: *slot,
                        location: rule.location,
                    },
                )?;
                if let Some(domain) = domain {
                    meet(variable_candidates, domain, limits, counters, rule.location)?;
                }
                counters.work(limits, rule.location)?;
                candidates.occurrences.push(Occurrence {
                    literal,
                    pattern,
                    column,
                    slot: *slot,
                    width: domain.map(BTreeSet::len),
                });
            }
        }
        candidates.narrow(rule, limits, budget, counters)?;
        Ok(candidates)
    }

    /// Exclude from each variable's candidates every value a comparison over
    /// that variable alone is defined and false at. A candidate the comparison
    /// cannot evaluate is kept: nothing excludes it, so the join reaches it
    /// and refuses as the language reference requires.
    fn narrow(
        &mut self,
        rule: &RuleIr,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let mut evaluation = Evaluation::default();
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            let Some(comparison) = unary(literal) else {
                continue;
            };
            let Some(values) = self.by_variable.get_mut(comparison.slot).ok_or(
                FormulaFailure::UnsafeVariable {
                    variable: comparison.slot,
                    location: rule.location,
                },
            )?
            else {
                continue;
            };
            let mut kept = 0;
            for index in 0..values.len() {
                let symbol = values[index];
                counters.charge_work(1 + atomic_bytes(symbol) as u128, limits, rule.location)?;
                budget.charge(ExpansionResource::TermWork, 1, rule.location)?;
                let candidate = crate::compile::scalar(symbol, rule.location)?;
                if excludes(
                    &comparison,
                    &candidate,
                    &mut evaluation,
                    limits,
                    budget,
                    counters,
                    rule.location,
                )? {
                    counters.record(Event::DomainExcludedValue);
                } else {
                    values[kept] = symbol;
                    kept += 1;
                }
            }
            counters.work(limits, rule.location)?;
            values.truncate(kept);
        }
        Ok(())
    }
}

/// Intersect a variable's candidates with one more argument domain, in place.
fn meet<'p>(
    target: &mut Option<Values<'p>>,
    source: &BTreeSet<&'p Symbol>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<(), FormulaFailure> {
    let Some(values) = target else {
        let mut values = Vec::new();
        values
            .try_reserve_exact(source.len())
            .map_err(|_| failure(Failure::Allocation, location))?;
        for &value in source {
            counters.work(limits, location)?;
            values.push(value);
        }
        *target = Some(values);
        return Ok(());
    };
    let mut other = source.iter().copied().peekable();
    let mut kept = 0;
    for index in 0..values.len() {
        while let Some(&right) = other.peek() {
            compare_work(values[index], right, limits, counters, location)?;
            match values[index].cmp(right) {
                std::cmp::Ordering::Less => break,
                std::cmp::Ordering::Equal => {
                    counters.work(limits, location)?;
                    values[kept] = values[index];
                    kept += 1;
                    other.next();
                    break;
                }
                std::cmp::Ordering::Greater => {
                    other.next();
                }
            }
        }
    }
    counters.work(limits, location)?;
    values.truncate(kept);
    Ok(())
}

/// Whether the comparison, with its variable bound to `candidate`, is
/// defined and false. An evaluation failure excludes nothing; a resource
/// refusal is returned as it is.
fn excludes(
    comparison: &Unary<'_>,
    candidate: &Value,
    evaluation: &mut Evaluation,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    location: Location,
) -> Result<bool, FormulaFailure> {
    let left = evaluation.expression(
        comparison.left,
        |_| Ok(candidate),
        limits,
        budget,
        counters,
        location,
    );
    let sides = left.and_then(|left| {
        let right = evaluation.expression(
            comparison.right,
            |_| Ok(candidate),
            limits,
            budget,
            counters,
            location,
        )?;
        Ok((left, right))
    });
    match sides {
        Ok((left, right)) => Ok(!super::super::compare(&left, comparison.relation, &right)),
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })) => Ok(false),
        Err(error) => Err(error),
    }
}

impl<'source> Support<'source> {
    pub(crate) fn domain_guards<'a>(
        &'a self,
        rule: &'a RuleIr,
        candidates: &'a Candidates<'_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Option<Guards<'a, 'source>>, FormulaFailure> {
        let before = counters.work;
        let result = Guards::prepare(self, rule, candidates, limits, budget, counters);
        counters.record(Event::DomainPrepareWork(counters.work - before));
        result
    }
}

impl<'a, 'source> Guards<'a, 'source> {
    fn prepare(
        support: &'a Support<'source>,
        rule: &'a RuleIr,
        candidates: &'a Candidates<'_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure> {
        if rule.variables == 0 {
            return Ok(None);
        }
        let mut guards = Self {
            support,
            rule,
            restrictions: Vec::new(),
            bytes: 0,
        };
        guards.include(
            size_of::<Self>()
                + size_of::<Restriction<'_, '_>>()
                + size_of::<Vec<Restriction<'_, '_>>>()
                + size_of::<Vec<u32>>()
                + size_of::<Value>(),
            limits,
            counters,
        )?;
        let restrictions = guards.restrict(
            &candidates.occurrences,
            &candidates.by_variable,
            limits,
            budget,
            counters,
        )?;
        guards.restrictions = restrictions;
        Ok((!guards.restrictions.is_empty()).then_some(guards))
    }

    /// One restriction per variable argument whose candidates could reject a
    /// row of its relation, resolved into that relation's dictionary.
    fn restrict(
        &mut self,
        occurrences: &[Occurrence<'a>],
        candidates: &[Option<Values<'_>>],
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Vec<Restriction<'a, 'source>>, FormulaFailure> {
        let mut restrictions = Vec::new();
        self.reserve(&mut restrictions, occurrences.len(), limits, counters)?;
        for occurrence in occurrences {
            counters.work(limits, self.rule.location)?;
            let Some(relation) = self
                .support
                .relations
                .relation(occurrence.pattern.predicate())
            else {
                continue;
            };
            if relation.column(occurrence.column).is_none() {
                return Err(failure(Failure::Column, self.rule.location));
            }
            let Some(values) = &candidates[occurrence.slot] else {
                continue;
            };
            // Candidates as many as the argument's domain admit every value
            // the relation can offer; only fewer can reject a row.
            if occurrence.width.is_some_and(|width| values.len() >= width) {
                continue;
            }
            let ids = self.resolve(
                relation,
                occurrence.column,
                values,
                limits,
                budget,
                counters,
            )?;
            counters.work(limits, self.rule.location)?;
            restrictions.push(Restriction {
                occurrence: occurrence.literal,
                relation,
                column: occurrence.column,
                ids,
            });
        }
        Ok(restrictions)
    }

    /// Convert one borrowed source symbol to a value, charging its payload.
    /// The caller releases the returned bytes once the value is dropped.
    fn admit_value(
        &mut self,
        symbol: &Symbol,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<(Value, usize), FormulaFailure> {
        let payload = atomic_bytes(symbol);
        self.include(payload, limits, counters)?;
        counters.charge_work(1 + payload as u128, limits, self.rule.location)?;
        budget.charge(ExpansionResource::TermWork, 1, self.rule.location)?;
        let value = crate::compile::scalar(symbol, self.rule.location)?;
        let actual = match &value {
            Value::String(text) | Value::Symbol(text) => text.capacity(),
            _ => 0,
        };
        if actual > payload {
            self.include_actual(actual - payload, limits, counters)?;
        }
        Ok((value, actual.max(payload)))
    }

    fn resolve(
        &mut self,
        relation: &'a Relation<'source>,
        column: usize,
        values: &[&Symbol],
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Vec<u32>, FormulaFailure> {
        let mut ids = Vec::new();
        self.reserve(&mut ids, values.len(), limits, counters)?;
        for &symbol in values {
            let (value, charged) = self.admit_value(symbol, limits, budget, counters)?;
            let outer = self.support.live.get() - relation.storage().retained_bytes;
            let base = counters.work;
            let attempt = relation.query_attempt(
                &[(column, &value)],
                zetesis_core::relation::Limits {
                    max_rows: limits.theory.max_atoms,
                    max_columns: relation.predicate().arity(),
                    max_values: limits.max_support_index_entries,
                    max_bytes: limits.max_support_bytes - outer,
                    max_work: limits.max_work - counters.work,
                },
            );
            counters.charge_work(attempt.work, limits, self.rule.location)?;
            counters.record(Event::SupportPeakBytes(
                outer as u128 + attempt.peak_bytes as u128,
            ));
            let query = attempt.result.map_err(|error| {
                super::super::relations::relation_failure(
                    error,
                    limits,
                    base,
                    outer,
                    self.rule.location,
                )
            })?;
            let id = query
                .equalities()
                .first()
                .map(|equality| equality.value_id());
            drop(query);
            drop(value);
            self.release(charged);
            if let Some(id) = id {
                let mut start = 0;
                let mut end = ids.len();
                while start < end {
                    counters.work(limits, self.rule.location)?;
                    let middle = start + (end - start) / 2;
                    if ids[middle] < id {
                        start = middle + 1;
                    } else {
                        end = middle;
                    }
                }
                counters.work(limits, self.rule.location)?;
                if ids.get(start) != Some(&id) {
                    counters.charge_work(
                        (ids.len() - start + 1) as u128,
                        limits,
                        self.rule.location,
                    )?;
                    ids.insert(start, id);
                }
            }
        }
        Ok(ids)
    }

    pub(crate) fn belongs_to(&self, rule: &RuleIr, support: &Support<'source>) -> bool {
        std::ptr::eq(self.rule, rule) && std::ptr::eq(self.support, support)
    }

    pub(crate) fn permits(
        &self,
        occurrence: usize,
        row: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        counters.record(Event::DomainGuardRow);
        for restriction in &self.restrictions {
            counters.work(limits, location)?;
            if restriction.occurrence != occurrence {
                continue;
            }
            let id = restriction
                .relation
                .column(restriction.column)
                .and_then(|column| column.get(row))
                .ok_or_else(|| failure(Failure::Column, location))?;
            let mut start = 0;
            let mut end = restriction.ids.len();
            let mut found = false;
            while start < end {
                counters.work(limits, location)?;
                counters.record(Event::DomainGuardCheck);
                let middle = start + (end - start) / 2;
                match restriction.ids[middle].cmp(id) {
                    std::cmp::Ordering::Less => start = middle + 1,
                    std::cmp::Ordering::Greater => end = middle,
                    std::cmp::Ordering::Equal => {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                counters.record(Event::DomainRejectedRow);
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn include(
        &mut self,
        bytes: usize,
        limits: &FormulaLimits,
        counters: &Counters,
    ) -> Result<(), FormulaFailure> {
        let total = self
            .support
            .live
            .get()
            .checked_add(bytes)
            .ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        Support::admit(total, limits, counters, self.rule.location)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        self.support.live.set(total);
        Ok(())
    }

    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
        limits: &FormulaLimits,
        counters: &Counters,
    ) -> Result<(), FormulaFailure> {
        let proposed = count
            .checked_mul(size_of::<T>())
            .ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        Support::admit(
            self.support
                .live
                .get()
                .checked_add(proposed)
                .ok_or_else(|| failure(Failure::Overflow, self.rule.location))?,
            limits,
            counters,
            self.rule.location,
        )?;
        values
            .try_reserve_exact(count)
            .map_err(|_| failure(Failure::Allocation, self.rule.location))?;
        let actual = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        self.include_actual(actual, limits, counters)
    }

    fn include_actual(
        &mut self,
        bytes: usize,
        limits: &FormulaLimits,
        counters: &Counters,
    ) -> Result<(), FormulaFailure> {
        let owned = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        let total = self
            .support
            .live
            .get()
            .checked_add(bytes)
            .ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        self.bytes = owned;
        self.support.live.set(total);
        counters.record(Event::SupportPeakBytes(total as u128));
        Support::admit(total, limits, counters, self.rule.location)
    }

    fn release(&mut self, bytes: usize) {
        self.bytes -= bytes;
        self.support.live.set(self.support.live.get() - bytes);
    }
}

impl Drop for Guards<'_, '_> {
    fn drop(&mut self) {
        self.support.live.set(self.support.live.get() - self.bytes);
    }
}

/// The comparison as one over a single variable, when both sides together
/// read exactly one. A comparison over two variables, or over none, is
/// decided in the join.
fn unary(literal: &LiteralIr) -> Option<Unary<'_>> {
    let LiteralIr::Compare(left, relation, right) = literal else {
        return None;
    };
    let mut inputs = left.inputs().chain(right.inputs());
    let slot = inputs.next()?;
    inputs.all(|input| input == slot).then_some(Unary {
        left,
        relation: *relation,
        right,
        slot,
    })
}

fn argument<'a, 'p>(
    analysis: &'a Analysis<'p>,
    predicate: &zetesis_core::Predicate,
    column: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<Option<&'a BTreeSet<&'p Symbol>>, FormulaFailure> {
    for (signature, index, argument) in analysis.arguments() {
        counters.charge_work(
            1 + signature.name.as_str().len() as u128 + predicate.name().len() as u128,
            limits,
            location,
        )?;
        let sign = match predicate.sign() {
            Sign::Positive => SourceSign::Positive,
            Sign::Negative => SourceSign::Negative,
        };
        if index == column
            && signature.sign == sign
            && signature.arity as usize == predicate.arity()
            && signature.name.as_str() == predicate.name()
        {
            return Ok(match argument.domain() {
                Domain::Unknown => None,
                Domain::Finite(values) => Some(values),
            });
        }
    }
    Ok(None)
}

fn atomic_bytes(symbol: &Symbol) -> usize {
    match symbol {
        Symbol::String(text) => text.len(),
        Symbol::Function { name, .. } => name.as_str().len(),
        _ => 0,
    }
}

fn compare_work(
    left: &Symbol,
    right: &Symbol,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<(), FormulaFailure> {
    counters.charge_work(
        1 + atomic_bytes(left) as u128 + atomic_bytes(right) as u128,
        limits,
        location,
    )
}

fn failure(error: Failure, location: Location) -> FormulaFailure {
    FormulaFailure::SupportRelation { error, location }
}
