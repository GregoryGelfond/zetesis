//! Completed presence from facts and optional closed choices.
//!
//! Source carriers never assert independent realizability in an answer set.

mod carrier;
pub(super) use carrier::{Carrier, certify as carrier};

use super::Predicates;
use crate::formula::ceiling;
use crate::formula_ir::{AggregateIr, AggregateKey, HeadIr, LiteralIr, Prepared, RuleIr};
use crate::formula_support::{
    Computation, Counters,
    components::{Pattern, Term},
};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};
use std::collections::BTreeSet;
use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, DefaultNegation};
use themelios_program::symbol::Signature;
use zetesis_core::catalog::{PredicateRef, TermRef};
use zetesis_core::{PatternRef, TemplateTerm, ValueNodeRef};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Absent,
    Optional,
    Required,
}

struct Context<'a, 'terms, 'source> {
    computation: &'a mut Computation<'terms, 'source>,
    limits: &'a FormulaLimits,
    counters: &'a mut Counters,
    location: Location,
    entries: usize,
}
impl<'source> Context<'_, '_, 'source> {
    fn inspect(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }
    fn retain<T: Ord>(&mut self, set: &mut BTreeSet<T>, value: T) -> Result<(), FormulaFailure> {
        self.inspect()?;
        if set.contains(&value) {
            return Ok(());
        }
        self.reserve_entry()?;
        set.insert(value);
        Ok(())
    }
    fn retain_predicate(
        &mut self,
        set: &mut Predicates<'source>,
        value: PredicateRef<'source>,
    ) -> Result<(), FormulaFailure> {
        if set.contains(value, self.limits, self.counters, self.location)? {
            return Ok(());
        }
        self.reserve_entry()?;
        set.insert(value, self.limits, self.counters, self.location)?;
        Ok(())
    }
    fn reserve_entry(&mut self) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::ObjectivePresenceEntries,
            self.entries as u128 + 1,
            self.limits.max_objective_presence_entries as u128,
            self.location,
        )?;
        self.entries += 1;
        Ok(())
    }
    fn pattern(&mut self, atom: Pattern) -> Result<PatternRef<'source>, FormulaFailure> {
        self.computation
            .static_pattern(atom, self.limits, self.counters, self.location)
    }
    fn term(&mut self, term: Term) -> Result<TemplateTerm<'source>, FormulaFailure> {
        self.computation
            .static_term(term, self.limits, self.counters, self.location)
    }
    fn closed(&mut self, atom: PatternRef<'_>) -> Result<bool, FormulaFailure> {
        for column in 0..atom.terms().len() {
            self.inspect()?;
            if !matches!(atom.terms().at(column), Some(TemplateTerm::Constant(_))) {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn same_predicate(
        &mut self,
        left: PredicateRef<'_>,
        right: PredicateRef<'_>,
    ) -> Result<bool, FormulaFailure> {
        left.compare_ref_with(right, || self.inspect())
            .map(std::cmp::Ordering::is_eq)
    }
    fn same_atom(
        &mut self,
        left: PatternRef<'_>,
        right: PatternRef<'_>,
    ) -> Result<bool, FormulaFailure> {
        self.inspect()?;
        if !self.same_predicate(left.predicate(), right.predicate())? {
            return Ok(false);
        }
        for column in 0..left.terms().len() {
            self.inspect()?;
            let (left, right) = (
                left.terms().at(column).unwrap(),
                right.terms().at(column).unwrap(),
            );
            match (left, right) {
                (TemplateTerm::Variable(left), TemplateTerm::Variable(right)) if left == right => {}
                (TemplateTerm::Constant(left), TemplateTerm::Constant(right)) => {
                    if !left.compare_ref_with(right, || self.inspect())?.is_eq() {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }
    fn matches(
        &mut self,
        predicate: PredicateRef<'_>,
        signature: &Signature,
    ) -> Result<bool, FormulaFailure> {
        self.inspect()?;
        self.counters.charge_work(
            predicate.name().len().min(signature.name.as_str().len()) as u128,
            self.limits,
            self.location,
        )?;
        Ok(
            crate::coherence::source_sign(predicate.sign()) == signature.sign
                && predicate.name() == signature.name.as_str()
                && predicate.arity() == signature.arity as usize,
        )
    }
    fn relevant(
        &mut self,
        predicate: PredicateRef<'_>,
        cone: &BTreeSet<&Signature>,
    ) -> Result<bool, FormulaFailure> {
        for signature in cone {
            if self.matches(predicate, signature)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

pub(super) fn certify<'source>(
    prepared: &Prepared,
    rule: &RuleIr,
    aggregate: &AggregateIr,
    retained: usize,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<Predicates<'source>>, FormulaFailure> {
    let mut context = Context {
        computation,
        limits,
        counters,
        location: rule.location,
        entries: retained,
    };
    let Some(head) = assignment(rule, aggregate, &mut context)? else {
        return Ok(None);
    };
    let mut blocked = false;
    for element in &aggregate.elements {
        context.inspect()?;
        let AggregateKey::Tuple(tuple) = &element.key else {
            return Ok(None);
        };
        for term in tuple {
            context.inspect()?;
            if !matches!(term, Term::Constant(_)) {
                return Ok(None);
            }
        }
        let Some(activity) = activity(prepared, &element.condition, &mut context)? else {
            return Ok(None);
        };
        let Some(first) = tuple.first() else {
            return Ok(None);
        };
        let TemplateTerm::Constant(value) = context.term(*first)? else {
            return Ok(None);
        };
        context.inspect()?;
        blocked |= activity == Activity::Required && dominates_numbers(aggregate.function, value);
    }
    context.inspect()?;
    let Some(descendants) = transport(prepared, head.predicate(), &mut context)? else {
        return Ok(None);
    };
    Ok(Some(if blocked {
        descendants
    } else {
        Predicates::default()
    }))
}

fn assignment<'source>(
    rule: &RuleIr,
    aggregate: &AggregateIr,
    context: &mut Context<'_, '_, 'source>,
) -> Result<Option<PatternRef<'source>>, FormulaFailure> {
    context.inspect()?;
    let (HeadIr::Normal(Some(head)), [LiteralIr::Aggregate(only)]) =
        (&rule.head, rule.body.as_slice())
    else {
        return Ok(None);
    };
    let Some(target) = aggregate.binding else {
        return Ok(None);
    };
    let head = context.pattern(*head)?;
    context.inspect()?;
    Ok((only.id == aggregate.id
        && head.terms().len() == 1
        && head.terms().at(0) == Some(TemplateTerm::Variable(target)))
    .then_some(head))
}

fn dominates_numbers(function: AggregateFunction, value: TermRef<'_>) -> bool {
    matches!(
        (function, value.descriptor()),
        (AggregateFunction::Min, ValueNodeRef::Infimum)
            | (
                AggregateFunction::Max,
                ValueNodeRef::String(_)
                    | ValueNodeRef::Symbol(_)
                    | ValueNodeRef::Function { .. }
                    | ValueNodeRef::Tuple { .. }
                    | ValueNodeRef::Supremum
            )
    )
}

fn activity(
    prepared: &Prepared,
    condition: &[LiteralIr],
    context: &mut Context<'_, '_, '_>,
) -> Result<Option<Activity>, FormulaFailure> {
    context.inspect()?;
    let atom = match condition {
        [] => return Ok(Some(Activity::Required)),
        [LiteralIr::Atom(DefaultNegation::None, atom)] => context.pattern(*atom)?,
        _ => return Ok(None),
    };
    if !context.closed(atom)? {
        return Ok(None);
    }
    let mut activity = Activity::Absent;
    for producer in &prepared.rules {
        context.inspect()?;
        match &producer.head {
            HeadIr::Normal(Some(head)) => {
                let head = context.pattern(*head)?;
                context.inspect()?;
                if !context.same_predicate(head.predicate(), atom.predicate())? {
                    continue;
                }
                if !producer.body.is_empty() || !context.closed(head)? {
                    return Ok(None);
                }
                if context.same_atom(head, atom)? {
                    activity = Activity::Required;
                }
            }
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    context.inspect()?;
                    let Some(head) = element.head.positive_atom() else {
                        continue;
                    };
                    let head = context.pattern(*head)?;
                    context.inspect()?;
                    if !context.same_predicate(head.predicate(), atom.predicate())? {
                        continue;
                    }
                    if !producer.body.is_empty()
                        || !group.guards.is_empty()
                        || element.key.tuple().is_some()
                        || !element.condition.is_empty()
                        || !context.closed(head)?
                    {
                        return Ok(None);
                    }
                    if context.same_atom(head, atom)? && activity != Activity::Required {
                        activity = Activity::Optional;
                    }
                }
            }
            HeadIr::Disjunction(_) | HeadIr::ConditionalDisjunction { .. } => {
                for head in producer.head.disjuncts() {
                    if let Some(head) = head.positive_atom() {
                        let head = context.pattern(*head)?;
                        context.inspect()?;
                        if context.same_predicate(head.predicate(), atom.predicate())? {
                            return Ok(None);
                        }
                    }
                }
            }
            HeadIr::Normal(None) => {}
        }
    }
    Ok(Some(activity))
}

fn cone<'a>(
    prepared: &'a Prepared,
    context: &mut Context<'_, '_, '_>,
) -> Result<BTreeSet<&'a Signature>, FormulaFailure> {
    let graph = prepared.analysis.dependencies();
    let mut relevant = BTreeSet::new();
    for objective in &prepared.objectives {
        for atom in &objective.positive {
            let atom = context.pattern(*atom)?;
            for predicate in graph.predicates() {
                context.inspect()?;
                if context.matches(atom.predicate(), predicate)? {
                    context.retain(&mut relevant, predicate)?;
                }
            }
        }
    }
    loop {
        let previous = relevant.len();
        for predicate in graph.predicates() {
            context.inspect()?;
            if relevant.contains(predicate) {
                for (_, dependency) in graph.edges_from(predicate) {
                    context.retain(&mut relevant, dependency)?;
                }
            }
        }
        if relevant.len() == previous {
            return Ok(relevant);
        }
    }
}

fn transport<'source>(
    prepared: &Prepared,
    predicate: PredicateRef<'source>,
    context: &mut Context<'_, '_, 'source>,
) -> Result<Option<Predicates<'source>>, FormulaFailure> {
    if !unique(prepared, predicate, context)? {
        return Ok(None);
    }
    let relevant = cone(prepared, context)?;
    let mut reachable = Predicates::default();
    context.retain_predicate(&mut reachable, predicate)?;
    loop {
        let previous = reachable.len();
        for rule in &prepared.rules {
            context.inspect()?;
            let HeadIr::Normal(Some(head)) = &rule.head else {
                continue;
            };
            let head = context.pattern(*head)?;
            context.inspect()?;
            let predicate = head.predicate();
            if reachable.contains(
                predicate,
                context.limits,
                context.counters,
                context.location,
            )? || !context.relevant(predicate, &relevant)?
            {
                continue;
            }
            let [LiteralIr::Atom(DefaultNegation::None, body)] = rule.body.as_slice() else {
                continue;
            };
            let body = context.pattern(*body)?;
            context.inspect()?;
            let agrees = head.terms().len() == 1
                && body.terms().len() == 1
                && matches!((head.terms().at(0), body.terms().at(0)),
                    (Some(TemplateTerm::Variable(left)), Some(TemplateTerm::Variable(right))) if left == right);
            if !agrees
                || !reachable.contains(
                    body.predicate(),
                    context.limits,
                    context.counters,
                    context.location,
                )?
                || !unique(prepared, predicate, context)?
            {
                continue;
            }
            context.retain_predicate(&mut reachable, predicate)?;
        }
        if reachable.len() == previous {
            break;
        }
    }
    Ok(Some(reachable))
}

fn unique(
    prepared: &Prepared,
    predicate: PredicateRef<'_>,
    context: &mut Context<'_, '_, '_>,
) -> Result<bool, FormulaFailure> {
    let mut occurrences = 0;
    for rule in &prepared.rules {
        context.inspect()?;
        match &rule.head {
            HeadIr::Normal(Some(head)) => {
                let head = context.pattern(*head)?;
                context.inspect()?;
                if context.same_predicate(head.predicate(), predicate)? {
                    occurrences += 1;
                }
            }
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    if let Some(head) = element.head.positive_atom() {
                        let head = context.pattern(*head)?;
                        context.inspect()?;
                        if context.same_predicate(head.predicate(), predicate)? {
                            return Ok(false);
                        }
                    }
                }
            }
            HeadIr::Disjunction(_) | HeadIr::ConditionalDisjunction { .. } => {
                for head in rule.head.disjuncts() {
                    if let Some(head) = head.positive_atom() {
                        let head = context.pattern(*head)?;
                        context.inspect()?;
                        if context.same_predicate(head.predicate(), predicate)? {
                            return Ok(false);
                        }
                    }
                }
            }
            HeadIr::Normal(None) => {}
        }
    }
    Ok(occurrences == 1)
}
