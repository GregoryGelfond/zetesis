//! Finite query traversal with explicit ownership for each binding depth.
//!
//! Cursors enumerate relational alternatives and generated values. A depth owns
//! only its new bindings and expanded alternatives; inherited slots are borrowed.
//! Exhaustion, visitor stop and errors all release this query's ownership while
//! leaving keys retained by an enclosing aggregate scope untouched.

use super::{
    Atom, Binder, Bound, Error, ErrorKind, EvaluationError, Metric, Query, Resource, Symbol,
    Visitor, Work, complete, matches, patterns, scopes, values,
};

fn undo_slots(query: &Query, work: &Work<'_>) -> Result<Vec<Vec<usize>>, Error> {
    let mut undos = work.reserve(query.binders.len())?;
    for binder in &query.binders {
        let count = match binder {
            Binder::Atom(alternatives) => alternatives
                .iter()
                .map(|pattern| {
                    pattern.terms.iter().map(patterns::slots).sum::<usize>()
                        + usize::from(pattern.key.is_some())
                })
                .max()
                .unwrap_or(0),
            Binder::Assign(_, _) | Binder::Aggregate(_, _) => 1,
            Binder::Match { pattern, .. } => patterns::slots(pattern) + 1,
        };
        undos.push(work.reserve(count)?);
    }
    Ok(undos)
}

fn owned_binding(
    binder: &Binder,
    alternatives: Option<&values::Values>,
    cursor: usize,
    atoms: &[&Atom],
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<(usize, Symbol, Metric), Error> {
    match binder {
        Binder::Assign(slot, expression)
        | Binder::Match {
            complete: slot,
            value: expression,
            ..
        } => {
            let (value, metric) = if let Some(values) = alternatives {
                patterns::own(&values.values[cursor].0, work)?
            } else {
                work.own(expression, binding)?
            };
            Ok((*slot, value, metric))
        }
        Binder::Aggregate(slot, aggregate) => {
            let (value, metric) = scopes::aggregate(aggregate, atoms, binding, work)?;
            work.check(
                Resource::Nodes,
                metric.nodes as u128,
                work.limits.max_symbol_nodes as u128,
            )?;
            work.check(Resource::Depth, 1, work.limits.max_symbol_depth as u128)?;
            work.construction_check(metric)?;
            // Guard measures stay wide. A binding constructs an ordinary logical
            // value and therefore crosses the pinned scalar-width boundary here.
            let value = match value {
                scopes::Value::Integer(value) => {
                    Symbol::Number(i32::try_from(value).map_err(|_| {
                        work.error(ErrorKind::Evaluation(EvaluationError::Overflow))
                    })?)
                }
                scopes::Value::Symbol(value) => value,
            };
            work.check(
                Resource::LocalBytes,
                work.local_bytes + metric.payload(),
                work.limits.max_local_bytes as u128,
            )?;
            work.local_bytes += metric.payload();
            Ok((*slot, value, metric))
        }
        Binder::Atom(_) => unreachable!("relational bindings use borrowed atom matching"),
    }
}

fn choice_count(
    binder: &Binder,
    atom_count: usize,
    binding: &[Option<Bound<'_>>],
    alternatives: &mut Option<values::Values>,
    work: &mut Work<'_>,
) -> Result<usize, Error> {
    Ok(match binder {
        Binder::Atom(alternatives) => atom_count
            .checked_mul(alternatives.len())
            .ok_or_else(|| work.error(ErrorKind::Allocation))?,
        Binder::Aggregate(_, _) => 1,
        Binder::Assign(_, expression)
        | Binder::Match {
            value: expression, ..
        } => {
            if expression.multiple() && alternatives.is_none() {
                *alternatives = Some(values::collect(expression, binding, work)?);
            }
            alternatives
                .as_ref()
                .map_or(1, |values| values.values.len())
        }
    })
}

/// Retain a complete owned value even when matching reports a partial failure.
/// The common query unwind then owns every whole value and captured subvalue.
fn retain_binding(
    binder: &Binder,
    owned: (usize, Symbol, Metric),
    binding: &mut [Option<Bound<'_>>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    let (slot, value, metric) = owned;
    let matched = if let Binder::Match { pattern, .. } = binder {
        patterns::bind_symbol(pattern, &value, binding, undo, work)
    } else {
        Ok(true)
    };
    binding[slot] = Some(Bound::Owned(value, metric));
    undo.push(slot);
    matched
}

pub(super) fn visit<'a>(
    query: &Query,
    atoms: &[&'a Atom],
    outer: &'a [Option<Bound<'a>>],
    work: &mut Work<'_>,
    visitor: &mut Visitor<'_>,
) -> Result<bool, Error> {
    work.step(1 + query.variables as u128 + query.binders.len() as u128)?;
    let mut binding = work.reserve(query.variables)?;
    binding.extend(
        outer
            .iter()
            .take(query.variables)
            .map(|value| value.as_ref().map(|value| Bound::Borrowed(value.borrow()))),
    );
    binding.resize_with(query.variables, || None);
    let mut alternatives: Vec<Option<values::Values>> = work.reserve(query.binders.len())?;
    alternatives.resize_with(query.binders.len(), || None);
    let result = (|| {
        if query.binders.is_empty() {
            return complete(query, atoms, &mut binding, work, visitor);
        }
        let mut cursors = work.reserve(query.binders.len())?;
        cursors.resize(query.binders.len(), 0usize);
        let mut undos = undo_slots(query, work)?;
        let mut depth = 0;
        loop {
            work.step(1 + undos[depth].len() as u128)?;
            for slot in undos[depth].drain(..) {
                if let Some(Bound::Owned(_, metric)) = binding[slot].take() {
                    work.local_bytes -= metric.payload();
                }
            }
            let count = choice_count(
                &query.binders[depth],
                atoms.len(),
                &binding,
                &mut alternatives[depth],
                work,
            )?;
            if cursors[depth] == count {
                cursors[depth] = 0;
                if let Some(values) = alternatives[depth].take() {
                    work.local_bytes -= values.bytes;
                }
                if depth == 0 {
                    break;
                }
                depth -= 1;
                continue;
            }
            let cursor = cursors[depth];
            cursors[depth] += 1;
            match &query.binders[depth] {
                Binder::Atom(alternatives) => {
                    if !matches(
                        &alternatives[cursor / atoms.len()],
                        atoms[cursor % atoms.len()],
                        &mut binding,
                        &mut undos[depth],
                        true,
                        work,
                    )? {
                        continue;
                    }
                }
                binder
                @ (Binder::Aggregate(_, _) | Binder::Assign(_, _) | Binder::Match { .. }) => {
                    let owned = owned_binding(
                        binder,
                        alternatives[depth].as_ref(),
                        cursor,
                        atoms,
                        &binding,
                        work,
                    )?;
                    if !retain_binding(binder, owned, &mut binding, &mut undos[depth], work)? {
                        continue;
                    }
                }
            }
            if depth + 1 == query.binders.len() {
                if !complete(query, atoms, &mut binding, work, visitor)? {
                    return Ok(false);
                }
            } else {
                depth += 1;
            }
        }
        Ok(true)
    })();
    // The visitor may retain aggregate keys in its enclosing scope. Release only
    // this query's owned bindings, on success, early termination, and error alike.
    work.local_bytes -= alternatives
        .iter()
        .flatten()
        .map(|values| values.bytes)
        .sum::<u128>();
    work.local_bytes -= binding
        .iter()
        .filter_map(|bound| match bound {
            Some(Bound::Owned(_, metric)) => Some(metric.payload()),
            _ => None,
        })
        .sum::<u128>();
    result
}
