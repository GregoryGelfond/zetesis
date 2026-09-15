//! Finite query traversal with explicit ownership for each binding depth.
//!
//! Cursors enumerate relational alternatives and generated values. A depth owns
//! only its new bindings and expanded alternatives; inherited slots are borrowed.
//! Exhaustion, visitor stop and errors all release this query's ownership while
//! leaving keys retained by an enclosing aggregate scope untouched.

use super::rows::AtomChoices;
use super::{
    Binder, Bound, Error, ErrorKind, EvaluationError, Metric, ModelRows, Query, Resource, Symbol,
    Visitor, Work, complete, matches, patterns, scopes, values,
};

enum Choice {
    Atom { alternative: usize, row: usize },
    Value(usize),
}

impl Choice {
    /// Apply one cursor choice while leaving every captured value in the
    /// current depth's undo owner, including when matching refuses or fails.
    fn bind<'a>(
        self,
        binder: &Binder,
        alternatives: Option<&values::Values>,
        atoms: &ModelRows<'a>,
        binding: &mut [Option<Bound<'a>>],
        undo: &mut Vec<usize>,
        work: &mut Work<'_>,
    ) -> Result<bool, Error> {
        match (binder, self) {
            (Binder::NumericMismatch(aggregate), Self::Value(_)) => {
                scopes::aggregate(aggregate, atoms, binding, work)?;
                Ok(false)
            }
            (Binder::Atom(alternatives), Self::Atom { alternative, row }) => matches(
                &alternatives[alternative],
                atoms.get(row),
                binding,
                undo,
                true,
                work,
            ),
            (
                binder @ (Binder::Aggregate(_, _) | Binder::Assign(_, _) | Binder::Match { .. }),
                Self::Value(cursor),
            ) => {
                let owned = owned_binding(binder, alternatives, cursor, atoms, binding, work)?;
                retain_binding(binder, owned, cursor, binding, undo, work)
            }
            _ => unreachable!("query cursors preserve binder kind"),
        }
    }
}

enum Cursor {
    Atoms(AtomChoices),
    Values(usize),
}

impl Cursor {
    fn next(
        &mut self,
        binder: &Binder,
        binding: &[Option<Bound<'_>>],
        alternatives: &mut Option<values::Values>,
        work: &mut Work<'_>,
    ) -> Result<Option<Choice>, Error> {
        match self {
            Self::Atoms(rows) => Ok(rows
                .next(work)?
                .map(|(alternative, row)| Choice::Atom { alternative, row })),
            Self::Values(cursor) => {
                let count = choice_count(binder, binding, alternatives, work)?;
                if *cursor == count {
                    *cursor = 0;
                    Ok(None)
                } else {
                    let choice = *cursor;
                    *cursor += 1;
                    Ok(Some(Choice::Value(choice)))
                }
            }
        }
    }
}

fn cursors(
    query: &Query,
    atoms: &ModelRows<'_>,
    work: &mut Work<'_>,
) -> Result<Vec<Cursor>, Error> {
    let mut cursors = work.reserve(query.binders.len())?;
    for binder in &query.binders {
        cursors.push(match binder {
            Binder::Atom(patterns) => Cursor::Atoms(AtomChoices::new(patterns, atoms, work)?),
            Binder::Assign(..)
            | Binder::Match { .. }
            | Binder::Aggregate(..)
            | Binder::NumericMismatch(_) => Cursor::Values(0),
        });
    }
    Ok(cursors)
}

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
            Binder::NumericMismatch(_) => 0,
            Binder::Match {
                patterns: alternatives,
                ..
            } => alternatives.iter().map(patterns::slots).max().unwrap_or(0) + 1,
        };
        undos.push(work.reserve(count)?);
    }
    Ok(undos)
}

fn owned_binding(
    binder: &Binder,
    alternatives: Option<&values::Values>,
    cursor: usize,
    atoms: &ModelRows<'_>,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<(usize, Symbol, Metric), Error> {
    let cursor = if let Binder::Match { patterns, .. } = binder {
        cursor / patterns.len()
    } else {
        cursor
    };
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
        Binder::Atom(_) | Binder::NumericMismatch(_) => {
            unreachable!("this binder does not retain a scalar")
        }
    }
}

fn choice_count(
    binder: &Binder,
    binding: &[Option<Bound<'_>>],
    alternatives: &mut Option<values::Values>,
    work: &mut Work<'_>,
) -> Result<usize, Error> {
    Ok(match binder {
        Binder::Atom(_) => unreachable!("atom cursors enumerate predicate ranges"),
        Binder::Aggregate(_, _) | Binder::NumericMismatch(_) => 1,
        Binder::Assign(_, expression)
        | Binder::Match {
            value: expression, ..
        } => {
            if expression.multiple() && alternatives.is_none() {
                *alternatives = Some(values::collect(expression, binding, work)?);
            }
            let values = alternatives
                .as_ref()
                .map_or(1, |values| values.values.len());
            let patterns = if let Binder::Match { patterns, .. } = binder {
                patterns.len()
            } else {
                1
            };
            values.checked_mul(patterns).ok_or_else(|| {
                work.error(ErrorKind::Limit {
                    resource: Resource::Bindings,
                    observed: (values as u128) * (patterns as u128),
                    limit: u128::from(work.limits.max_bindings),
                })
            })?
        }
    })
}

/// Retain a complete owned value even when matching reports a partial failure.
/// The common query unwind then owns every whole value and captured subvalue.
fn retain_binding(
    binder: &Binder,
    owned: (usize, Symbol, Metric),
    cursor: usize,
    binding: &mut [Option<Bound<'_>>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    let (slot, value, metric) = owned;
    let matched = if let Binder::Match {
        patterns: alternatives,
        ..
    } = binder
    {
        patterns::bind_symbol(
            &alternatives[cursor % alternatives.len()],
            &value,
            binding,
            undo,
            work,
        )
    } else {
        Ok(true)
    };
    binding[slot] = Some(Bound::Owned(value, metric));
    undo.push(slot);
    matched
}

pub(super) fn visit<'a>(
    query: &Query,
    atoms: &ModelRows<'a>,
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
        let mut cursors = cursors(query, atoms, work)?;
        let mut undos = undo_slots(query, work)?;
        let mut depth = 0;
        loop {
            work.step(1 + undos[depth].len() as u128)?;
            for slot in undos[depth].drain(..) {
                if let Some(Bound::Owned(_, metric)) = binding[slot].take() {
                    work.local_bytes -= metric.payload();
                }
            }
            let choice = cursors[depth].next(
                &query.binders[depth],
                &binding,
                &mut alternatives[depth],
                work,
            )?;
            let Some(choice) = choice else {
                if let Some(values) = alternatives[depth].take() {
                    work.local_bytes -= values.bytes;
                }
                if depth == 0 {
                    break;
                }
                depth -= 1;
                continue;
            };
            if !choice.bind(
                &query.binders[depth],
                alternatives[depth].as_ref(),
                atoms,
                &mut binding,
                &mut undos[depth],
                work,
            )? {
                continue;
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
