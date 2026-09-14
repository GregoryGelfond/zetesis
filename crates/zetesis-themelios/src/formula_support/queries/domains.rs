//! Rule-local necessary domains, resolved into the completed owner's dictionary.

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::mem::size_of;

use themelios_program::program::DefaultNegation;
use themelios_program::symbol::{Sign as SourceSign, Symbol};
use zetesis_core::relation::{Failure, Relation};
use zetesis_core::{Sign, Term, Value};
use zetesis_domain::{Analysis, Domain};

use super::{Counters, Event, FormulaFailure, FormulaLimits, Location, Support};
use crate::expansion::Budget;
use crate::formula_ir::{LiteralIr, RuleIr};
use crate::ExpansionResource;

type Values<'a> = Vec<&'a Symbol>;

struct Restriction<'a, 'source> {
    occurrence: usize,
    relation: &'a Relation<'source>,
    column: usize,
    ids: Vec<u32>,
}

/// Immutable restrictions for exactly one rule and one completed query owner.
/// The lease includes live guards and preparation scratch beside all table masks.
pub(crate) struct Guards<'a, 'source> {
    support: &'a Support<'source>,
    rule: &'a RuleIr,
    restrictions: Vec<Restriction<'a, 'source>>,
    bytes: usize,
}

impl<'source> Support<'source> {
    pub(crate) fn domain_guards<'a>(
        &'a self,
        rule: &'a RuleIr,
        analysis: &Analysis<'_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Option<Guards<'a, 'source>>, FormulaFailure> {
        let before = counters.work;
        let result = Guards::prepare(self, rule, analysis, limits, budget, counters);
        counters.record(Event::DomainPrepareWork(counters.work - before));
        result
    }
}

impl<'a, 'source> Guards<'a, 'source> {
    fn prepare(
        support: &'a Support<'source>,
        rule: &'a RuleIr,
        analysis: &Analysis<'_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure> {
        if rule.variables == 0 { return Ok(None); }
        let mut guards = Self { support, rule, restrictions: Vec::new(), bytes: 0 };
        guards.include(size_of::<Self>() + size_of::<Vec<Option<Values<'_>>>>()
            + size_of::<Restriction<'_, '_>>() + size_of::<Vec<Restriction<'_, '_>>>()
            + size_of::<Vec<u32>>() + size_of::<Vec<&Symbol>>() + size_of::<Value>(), limits, counters)?;
        let mut bounds = Vec::new();
        guards.reserve(&mut bounds, rule.variables, limits, counters)?;
        for _ in 0..rule.variables {
            counters.work(limits, rule.location)?;
            bounds.push(None);
        }
        let mut columns = 0_usize;
        for literal in &rule.body {
            let LiteralIr::Atom(DefaultNegation::None, pattern) = literal else {
                return Err(failure(Failure::Predicate, rule.location));
            };
            for (column, term) in pattern.terms().iter().enumerate() {
                counters.work(limits, rule.location)?;
                if let Term::Variable(slot) = term {
                    let bound = bounds.get_mut(*slot).ok_or(FormulaFailure::UnsafeVariable {
                        variable: *slot, location: rule.location,
                    })?;
                    columns = columns.checked_add(1).ok_or_else(|| failure(Failure::Overflow, rule.location))?;
                    if let Some(domain) = argument(analysis, pattern.predicate(), column, limits, counters, rule.location)? {
                        guards.meet(bound, domain, limits, counters)?;
                    }
                }
            }
        }
        let mut restrictions = Vec::new();
        guards.reserve(&mut restrictions, columns, limits, counters)?;
        for (occurrence, literal) in rule.body.iter().enumerate() {
            let LiteralIr::Atom(DefaultNegation::None, pattern) = literal else {
                return Err(failure(Failure::Predicate, rule.location));
            };
            let Some(relation) = support.relations.relation(pattern.predicate()) else { continue; };
            for (column, term) in pattern.terms().iter().enumerate() {
                counters.work(limits, rule.location)?;
                if relation.column(column).is_none() { return Err(failure(Failure::Column, rule.location)); }
                let Term::Variable(slot) = term else { continue; };
                let Some(values) = &bounds[*slot] else { continue; };
                let ids = guards.resolve(relation, column, values, limits, budget, counters)?;
                counters.work(limits, rule.location)?;
                restrictions.push(Restriction { occurrence, relation, column, ids });
            }
        }
        guards.restrictions = restrictions;
        counters.charge_work(bounds.len() as u128, limits, rule.location)?;
        let transient = transient_bytes(&bounds, bounds.capacity()).ok_or_else(|| failure(Failure::Overflow, rule.location))?;
        drop(bounds);
        guards.release(transient);
        Ok((!guards.restrictions.is_empty()).then_some(guards))
    }

    fn meet<'p>(&mut self, target: &mut Option<Values<'p>>, source: &BTreeSet<&'p Symbol>,
        limits: &FormulaLimits, counters: &mut Counters) -> Result<(), FormulaFailure>
    {
        let Some(values) = target else {
            let mut values = Vec::new();
            self.reserve(&mut values, source.len(), limits, counters)?;
            for &value in source {
                counters.work(limits, self.rule.location)?;
                values.push(value);
            }
            *target = Some(values);
            return Ok(());
        };
        let mut other = source.iter().copied().peekable();
        let mut kept = 0;
        for index in 0..values.len() {
            while let Some(&right) = other.peek() {
                compare_work(values[index], right, limits, counters, self.rule.location)?;
                match values[index].cmp(right) {
                    std::cmp::Ordering::Less => break,
                    std::cmp::Ordering::Equal => {
                        counters.work(limits, self.rule.location)?;
                        values[kept] = values[index];
                        kept += 1;
                        other.next();
                        break;
                    }
                    std::cmp::Ordering::Greater => { other.next(); }
                }
            }
        }
        counters.work(limits, self.rule.location)?;
        values.truncate(kept);
        Ok(())
    }

    fn resolve(&mut self, relation: &'a Relation<'source>, column: usize, values: &[&Symbol],
        limits: &FormulaLimits, budget: &mut Budget, counters: &mut Counters) -> Result<Vec<u32>, FormulaFailure>
    {
        let mut ids = Vec::new();
        self.reserve(&mut ids, values.len(), limits, counters)?;
        for &symbol in values {
            let payload = atomic_bytes(symbol);
            self.include(payload, limits, counters)?;
            counters.charge_work(1 + payload as u128, limits, self.rule.location)?;
            budget.charge(ExpansionResource::TermWork, 1, self.rule.location)?;
            let value = crate::compile::scalar(symbol, self.rule.location)?;
            let actual = match &value { Value::String(text) | Value::Symbol(text) => text.capacity(), _ => 0 };
            if actual > payload { self.include_actual(actual - payload, limits, counters)?; }
            let outer = self.support.live.get() - relation.storage().retained_bytes;
            let base = counters.work;
            let attempt = relation.query_attempt(&[(column, &value)], zetesis_core::relation::Limits {
                max_rows: limits.theory.max_atoms,
                max_columns: relation.predicate().arity(),
                max_values: limits.max_support_index_entries,
                max_bytes: limits.max_support_bytes - outer,
                max_work: limits.max_work - counters.work,
            });
            counters.charge_work(attempt.work, limits, self.rule.location)?;
            counters.record(Event::SupportPeakBytes(outer as u128 + attempt.peak_bytes as u128));
            let query = attempt.result.map_err(|error| super::super::relations::relation_failure(
                error, limits, base, outer, self.rule.location))?;
            let id = query.equalities().first().map(|equality| equality.value_id());
            drop(query);
            drop(value);
            self.release(actual.max(payload));
            if let Some(id) = id {
                let mut start = 0;
                let mut end = ids.len();
                while start < end {
                    counters.work(limits, self.rule.location)?;
                    let middle = start + (end - start) / 2;
                    if ids[middle] < id { start = middle + 1; } else { end = middle; }
                }
                counters.work(limits, self.rule.location)?;
                if ids.get(start) != Some(&id) {
                    counters.charge_work((ids.len() - start + 1) as u128, limits, self.rule.location)?;
                    ids.insert(start, id);
                }
            }
        }
        Ok(ids)
    }

    pub(crate) fn belongs_to(&self, rule: &RuleIr, support: &Support<'_>) -> bool {
        std::ptr::eq(self.rule, rule) && std::ptr::eq(self.support, support)
    }

    pub(crate) fn permits(&self, occurrence: usize, row: usize,
        limits: &FormulaLimits, counters: &mut Counters, location: Location) -> Result<bool, FormulaFailure>
    {
        counters.record(Event::DomainGuardRow);
        for restriction in &self.restrictions {
            counters.work(limits, location)?;
            if restriction.occurrence != occurrence { continue; }
            let id = restriction.relation.column(restriction.column).and_then(|column| column.get(row))
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
                    std::cmp::Ordering::Equal => { found = true; break; }
                }
            }
            if !found { counters.record(Event::DomainRejectedRow); return Ok(false); }
        }
        Ok(true)
    }

    fn include(&mut self, bytes: usize, limits: &FormulaLimits, counters: &Counters) -> Result<(), FormulaFailure> {
        let total = self.support.live.get().checked_add(bytes).ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        Support::admit(total, limits, counters, self.rule.location)?;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        self.support.live.set(total);
        Ok(())
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize,
        limits: &FormulaLimits, counters: &Counters) -> Result<(), FormulaFailure>
    {
        let proposed = count.checked_mul(size_of::<T>()).ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        Support::admit(self.support.live.get().checked_add(proposed).ok_or_else(|| failure(Failure::Overflow, self.rule.location))?,
            limits, counters, self.rule.location)?;
        values.try_reserve_exact(count).map_err(|_| failure(Failure::Allocation, self.rule.location))?;
        let actual = values.capacity().checked_mul(size_of::<T>()).ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        self.include_actual(actual, limits, counters)
    }

    fn include_actual(&mut self, bytes: usize, limits: &FormulaLimits, counters: &Counters) -> Result<(), FormulaFailure> {
        let owned = self.bytes.checked_add(bytes).ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
        let total = self.support.live.get().checked_add(bytes).ok_or_else(|| failure(Failure::Overflow, self.rule.location))?;
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
    fn drop(&mut self) { self.support.live.set(self.support.live.get() - self.bytes); }
}

fn argument<'a, 'p>(analysis: &'a Analysis<'p>, predicate: &zetesis_core::Predicate, column: usize,
    limits: &FormulaLimits, counters: &mut Counters, location: Location) -> Result<Option<&'a BTreeSet<&'p Symbol>>, FormulaFailure>
{
    for (signature, index, argument) in analysis.arguments() {
        counters.charge_work(1 + signature.name.as_str().len() as u128 + predicate.name().len() as u128, limits, location)?;
        let sign = match predicate.sign() { Sign::Positive => SourceSign::Positive, Sign::Negative => SourceSign::Negative };
        if index == column && signature.sign == sign && signature.arity as usize == predicate.arity()
            && signature.name.as_str() == predicate.name()
        {
            return Ok(match argument.domain() { Domain::Unknown => None, Domain::Finite(values) => Some(values) });
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

fn compare_work(left: &Symbol, right: &Symbol, limits: &FormulaLimits, counters: &mut Counters, location: Location) -> Result<(), FormulaFailure> {
    counters.charge_work(1 + atomic_bytes(left) as u128 + atomic_bytes(right) as u128, limits, location)
}

fn failure(error: Failure, location: Location) -> FormulaFailure { FormulaFailure::SupportRelation { error, location } }

fn transient_bytes(bounds: &[Option<Values<'_>>], capacity: usize) -> Option<usize> {
    let arrays = bounds.iter().flatten().try_fold(0_usize, |total, values| {
        total.checked_add(values.capacity().checked_mul(size_of::<&Symbol>())?)
    })?;
    arrays.checked_add(capacity.checked_mul(size_of::<Option<Values<'_>>>())?)?
        .checked_add(size_of::<Vec<Option<Values<'_>>>>())?
        .checked_add(size_of::<Restriction<'_, '_>>())?
        .checked_add(size_of::<Vec<Restriction<'_, '_>>>())?
        .checked_add(size_of::<Vec<u32>>())?
        .checked_add(size_of::<Vec<&Symbol>>())?
        .checked_add(size_of::<Value>())
}
