//! Completed presence from facts and optional closed choices.
//!
//! Shared optional conditions and constraints never erase possible witnesses.
//! Only a required value dominating all numbers excludes numeric presence.

use std::collections::BTreeSet;

use themelios_analysis::depend::DependencyGraph;
use themelios_base::span::Location;
use themelios_program::program::{AggregateFunction, DefaultNegation};
use themelios_program::symbol::Signature;
use zetesis_core::{AtomPattern, Predicate, Term, Value};

use crate::formula::ceiling;
use crate::formula_ir::{AggregateIr, AggregateKey, HeadIr, LiteralIr, Prepared, RuleIr};
use crate::formula_support::Counters;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Absent,
    Optional,
    Required,
}

/// These sets share one lifetime with no intermediate reclamation. Entries
/// count simultaneous borrowed slots, including caller-held exclusions. No
/// names or values are cloned. Every insertion is checked before allocation.
struct Context<'a> {
    limits: &'a FormulaLimits,
    counters: &'a mut Counters,
    location: Location,
    entries: usize,
}
impl Context<'_> {
    fn inspect(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }
    fn retain<T: Ord>(&mut self, set: &mut BTreeSet<T>, value: T) -> Result<(), FormulaFailure> {
        self.inspect()?;
        if set.contains(&value) {
            return Ok(());
        }
        ceiling(
            FormulaResource::ObjectivePresenceEntries,
            self.entries as u128 + 1,
            self.limits.max_objective_presence_entries as u128,
            self.location,
        )?;
        set.insert(value);
        self.entries += 1;
        Ok(())
    }
    fn closed(&mut self, atom: &AtomPattern) -> Result<bool, FormulaFailure> {
        for term in atom.terms() {
            self.inspect()?;
            if !matches!(term, Term::Constant(_)) {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn matches(
        &mut self,
        predicate: &Predicate,
        signature: &Signature,
    ) -> Result<bool, FormulaFailure> {
        self.inspect()?;
        Ok(
            crate::coherence::source_sign(predicate.sign()) == signature.sign
                && predicate.name() == signature.name.as_str()
                && predicate.arity() == signature.arity as usize,
        )
    }
    fn relevant(
        &mut self,
        predicate: &Predicate,
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

/// None denotes an unqualified carrier. An empty set denotes a qualified
/// carrier whose possible numeric witnesses remain retained.
pub(super) fn certify<'a>(
    prepared: &'a Prepared,
    rule: &'a RuleIr,
    aggregate: &AggregateIr,
    retained: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<BTreeSet<&'a Predicate>>, FormulaFailure> {
    let mut context = Context {
        limits,
        counters,
        location: rule.location,
        entries: retained,
    };
    context.inspect()?;
    let (HeadIr::Normal(Some(head)), [LiteralIr::Aggregate(only)]) =
        (&rule.head, rule.body.as_slice())
    else {
        return Ok(None);
    };
    let Some(target) = aggregate.binding else {
        return Ok(None);
    };
    if only.id != aggregate.id || head.terms() != [Term::Variable(target)] {
        return Ok(None);
    }
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
        let Some(Term::Constant(value)) = tuple.first() else {
            return Ok(None);
        };
        blocked |= activity == Activity::Required && dominates_numbers(aggregate.function, value);
    }
    let Some(descendants) = transport(prepared, head.predicate(), &mut context)? else {
        return Ok(None);
    };
    Ok(Some(if blocked {
        descendants
    } else {
        BTreeSet::new()
    }))
}

fn dominates_numbers(function: AggregateFunction, value: &Value) -> bool {
    matches!(
        (function, value),
        (AggregateFunction::Min, Value::Infimum)
            | (
                AggregateFunction::Max,
                Value::String(_) | Value::Symbol(_) | Value::Structured(_) | Value::Supremum
            )
    )
}

fn activity(
    prepared: &Prepared,
    condition: &[LiteralIr],
    context: &mut Context<'_>,
) -> Result<Option<Activity>, FormulaFailure> {
    context.inspect()?;
    let atom = match condition {
        [] => return Ok(Some(Activity::Required)),
        [LiteralIr::Atom(DefaultNegation::None, atom)] if context.closed(atom)? => atom,
        _ => return Ok(None),
    };
    let mut activity = Activity::Absent;
    for producer in &prepared.rules {
        context.inspect()?;
        match &producer.head {
            HeadIr::Normal(Some(head)) if head.predicate() == atom.predicate() => {
                if !producer.body.is_empty() || !context.closed(head)? {
                    return Ok(None);
                }
                if head == atom {
                    activity = Activity::Required;
                }
            }
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    context.inspect()?;
                    let Some(head) = element.head.positive_atom() else {
                        continue;
                    };
                    if head.predicate() != atom.predicate() {
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
                    if head == atom && activity != Activity::Required {
                        activity = Activity::Optional;
                    }
                }
            }
            // Objective dependency admission has already excluded every signed
            // or default-negated disjunctive producer in this observed cone.
            HeadIr::Disjunction(heads) => {
                for head in heads {
                    context.inspect()?;
                    if head
                        .positive_atom()
                        .is_some_and(|head| head.predicate() == atom.predicate())
                    {
                        return Ok(None);
                    }
                }
            }
            HeadIr::Normal(_) => {}
        }
    }
    Ok(Some(activity))
}

/// Borrow the objective dependency cone rather than cloning graph names.
fn cone<'a>(
    prepared: &'a Prepared,
    context: &mut Context<'_>,
) -> Result<BTreeSet<&'a Signature>, FormulaFailure> {
    let graph = prepared.analysis.dependencies();
    let mut relevant = BTreeSet::new();
    for objective in &prepared.objectives {
        context.inspect()?;
        for atom in objective.template.positive() {
            context.inspect()?;
            for predicate in graph.predicates() {
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

fn depends(
    graph: &DependencyGraph,
    head: &Predicate,
    reachable: &BTreeSet<&Predicate>,
    context: &mut Context<'_>,
) -> Result<bool, FormulaFailure> {
    for predicate in graph.predicates() {
        if !context.matches(head, predicate)? {
            continue;
        }
        for (_, dependency) in graph.edges_from(predicate) {
            context.inspect()?;
            for ancestor in reachable {
                if context.matches(ancestor, dependency)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Each successful round adds a source predicate. Only unique unary renamings
/// inherit the completed carrier; finite scans never recurse.
fn transport<'a>(
    prepared: &'a Prepared,
    predicate: &'a Predicate,
    context: &mut Context<'_>,
) -> Result<Option<BTreeSet<&'a Predicate>>, FormulaFailure> {
    let relevant = cone(prepared, context)?;
    let graph = prepared.analysis.dependencies();
    let mut reachable = BTreeSet::new();
    context.retain(&mut reachable, predicate)?;
    loop {
        let previous = reachable.len();
        for rule in &prepared.rules {
            context.inspect()?;
            let HeadIr::Normal(Some(head)) = &rule.head else {
                continue;
            };
            if !context.relevant(head.predicate(), &relevant)?
                || !depends(graph, head.predicate(), &reachable, context)?
            {
                continue;
            }
            let [LiteralIr::Atom(DefaultNegation::None, body)] = rule.body.as_slice() else {
                return Ok(None);
            };
            context.inspect()?;
            if !matches!((head.terms(), body.terms()), ([Term::Variable(left)], [Term::Variable(right)]) if left == right)
                || !reachable.contains(body.predicate())
            {
                return Ok(None);
            }
            context.retain(&mut reachable, head.predicate())?;
        }
        if reachable.len() == previous {
            break;
        }
    }
    for predicate in &reachable {
        if !unique(prepared, predicate, context)? {
            return Ok(None);
        }
    }
    Ok(Some(reachable))
}

fn unique(
    prepared: &Prepared,
    predicate: &Predicate,
    context: &mut Context<'_>,
) -> Result<bool, FormulaFailure> {
    let mut occurrences = 0;
    for rule in &prepared.rules {
        context.inspect()?;
        match &rule.head {
            HeadIr::Normal(Some(head)) if head.predicate() == predicate => occurrences += 1,
            HeadIr::Choice(group) => {
                for element in &group.elements {
                    context.inspect()?;
                    if element
                        .head
                        .positive_atom()
                        .is_some_and(|head| head.predicate() == predicate)
                    {
                        return Ok(false);
                    }
                }
            }
            HeadIr::Disjunction(heads) => {
                for head in heads {
                    context.inspect()?;
                    if head
                        .positive_atom()
                        .is_some_and(|head| head.predicate() == predicate)
                    {
                        return Ok(false);
                    }
                }
            }
            HeadIr::Normal(_) => {}
        }
    }
    Ok(occurrences == 1)
}
