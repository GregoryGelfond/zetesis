//! Finite query traversal with explicit ownership for each binding depth.
//!
//! Cursors enumerate relational alternatives and generated values. A depth owns
//! only its new bindings and expanded alternatives; inherited slots are borrowed.
//! Exhaustion, visitor stop and errors release this query's logical ownership.

use zetesis_core::catalog::TermKey;

use super::rows::AtomChoices;
use super::{
    Binder, Binding, Error, ErrorKind, EvaluationError, Interpreter, Metric, ModelRows, Query,
    Resource, Visitor, Work, anonymous, complete, matches, patterns, scopes, values,
};

enum Alternatives {
    Terms(values::Values),
    Keys(anonymous::Alternatives),
}
impl Alternatives {
    fn bytes(&self) -> u128 {
        match self {
            Self::Terms(values) => values.bytes,
            Self::Keys(values) => values.bytes,
        }
    }
}

enum Choice {
    Atom { alternative: usize, row: usize },
    Value(usize),
}
impl Choice {
    /// Every installed value belongs to the depth's undo owner, including when
    /// later matching refuses or fails.
    fn bind<'input>(
        self,
        binder: &Binder,
        alternatives: Option<&Alternatives>,
        atoms: &ModelRows<'input>,
        binding: &mut Binding<'input>,
        undo: &mut Vec<usize>,
        context: &mut Interpreter<'input, '_, '_>,
    ) -> Result<bool, Error> {
        match (binder, self) {
            (Binder::NumericMismatch(aggregate), Self::Value(_)) => {
                scopes::aggregate(aggregate, atoms, binding, context)?;
                Ok(false)
            }
            (Binder::Atom(alternatives), Self::Atom { alternative, row }) => matches(
                &alternatives[alternative],
                atoms.get(row),
                binding,
                undo,
                true,
                context,
            ),
            (Binder::AtomKey(slot, _), Self::Value(cursor)) => {
                let Some(Alternatives::Keys(values)) = alternatives else {
                    unreachable!("atom-key cursor prepares typed alternatives")
                };
                context.work.step(1)?;
                let (key, metric) = values.values[cursor];
                admit_local(metric, context.work)?;
                binding.bind_pattern(*slot, key, metric, context.work);
                undo.push(*slot);
                Ok(true)
            }
            (
                binder @ (Binder::Aggregate(_, _) | Binder::Assign(_, _) | Binder::Match { .. }),
                Self::Value(cursor),
            ) => {
                let terms = match alternatives {
                    Some(Alternatives::Terms(values)) => Some(values),
                    None => None,
                    Some(Alternatives::Keys(_)) => {
                        unreachable!("scalar cursor retains term alternatives")
                    }
                };
                let owned = owned_binding(binder, terms, cursor, atoms, binding, context)?;
                retain_binding(binder, owned, cursor, binding, undo, context)
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
    fn next<'input>(
        &mut self,
        binder: &Binder,
        binding: &Binding<'input>,
        alternatives: &mut Option<Alternatives>,
        context: &mut Interpreter<'input, '_, '_>,
    ) -> Result<Option<Choice>, Error> {
        match self {
            Self::Atoms(rows) => Ok(rows
                .next(context.work)?
                .map(|(alternative, row)| Choice::Atom { alternative, row })),
            Self::Values(cursor) => {
                let count = choice_count(binder, binding, alternatives, context)?;
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
fn cursors<'input>(
    query: &Query,
    atoms: &ModelRows<'input>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<Vec<Cursor>, Error> {
    let mut cursors = context.work.reserve(query.binders.len())?;
    for binder in &query.binders {
        cursors.push(match binder {
            Binder::Atom(patterns) => Cursor::Atoms(AtomChoices::new(
                patterns,
                atoms,
                context.metadata,
                context.work,
            )?),
            Binder::Assign(..)
            | Binder::AtomKey(..)
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
            Binder::Assign(..) | Binder::AtomKey(..) | Binder::Aggregate(..) => 1,
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
fn admit_local(metric: Metric, work: &Work<'_>) -> Result<(), Error> {
    work.construction_check(metric)?;
    work.check(
        Resource::LocalBytes,
        work.local_bytes + metric.payload(),
        work.limits.max_local_bytes as u128,
    )
}
fn owned_binding<'input>(
    binder: &Binder,
    alternatives: Option<&values::Values>,
    cursor: usize,
    atoms: &ModelRows<'input>,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<(usize, TermKey, Metric), Error> {
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
            let (key, metric) = if let Some(values) = alternatives {
                context.work.step(1)?;
                let metric = values.metric(cursor);
                admit_local(metric, context.work)?;
                (values.key(cursor, context)?, metric)
            } else {
                context.own(expression, binding)?
            };
            Ok((*slot, key, metric))
        }
        Binder::Aggregate(slot, aggregate) => {
            let (value, metric) = scopes::aggregate(aggregate, atoms, binding, context)?;
            context.work.check(
                Resource::Nodes,
                metric.nodes as u128,
                context.work.limits.max_symbol_nodes as u128,
            )?;
            context.work.check(
                Resource::Depth,
                1,
                context.work.limits.max_symbol_depth as u128,
            )?;
            admit_local(metric, context.work)?;
            // Guard arithmetic stays wide. A retained logical scalar crosses
            // the pinned width boundary only when this binder constructs it.
            let key = match value {
                scopes::Value::Integer(value) => {
                    context.number(i32::try_from(value).map_err(|_| {
                        context
                            .work
                            .error(ErrorKind::Evaluation(EvaluationError::Overflow))
                    })?)?
                }
                scopes::Value::Term(key) => key,
            };
            Ok((*slot, key, metric))
        }
        Binder::Atom(_) | Binder::AtomKey(..) | Binder::NumericMismatch(_) => {
            unreachable!("this binder does not retain a scalar")
        }
    }
}
fn choice_count<'input>(
    binder: &Binder,
    binding: &Binding<'input>,
    alternatives: &mut Option<Alternatives>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<usize, Error> {
    Ok(match binder {
        Binder::Atom(_) => unreachable!("atom cursors enumerate predicate ranges"),
        Binder::Aggregate(_, _) | Binder::NumericMismatch(_) => 1,
        Binder::AtomKey(_, template) => {
            if alternatives.is_none() {
                *alternatives = Some(Alternatives::Keys(anonymous::collect(
                    template, binding, context,
                )?));
            }
            let Some(Alternatives::Keys(values)) = alternatives else {
                unreachable!("typed atom-key alternatives")
            };
            values.values.len()
        }
        Binder::Assign(_, expression)
        | Binder::Match {
            value: expression, ..
        } => {
            if expression.multiple() && alternatives.is_none() {
                *alternatives = Some(Alternatives::Terms(values::collect(
                    expression, binding, context,
                )?));
            }
            let values = match alternatives {
                Some(Alternatives::Terms(values)) => values.len(),
                None => 1,
                Some(Alternatives::Keys(_)) => unreachable!("typed scalar alternatives"),
            };
            let patterns = if let Binder::Match { patterns, .. } = binder {
                patterns.len()
            } else {
                1
            };
            values.checked_mul(patterns).ok_or_else(|| {
                context.work.error(ErrorKind::Limit {
                    resource: Resource::Bindings,
                    observed: (values as u128) * (patterns as u128),
                    limit: u128::from(context.work.limits.max_bindings),
                })
            })?
        }
    })
}
fn retain_binding<'input>(
    binder: &Binder,
    owned: (usize, TermKey, Metric),
    cursor: usize,
    binding: &mut Binding<'input>,
    undo: &mut Vec<usize>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    let (slot, key, metric) = owned;
    // Match's complete slot is freshly allocated by the compiler; its pattern
    // cannot refer to it. Install ownership before the matcher can fail, so the
    // same unwind releases both the whole term and any partial captures.
    binding.bind_term(slot, &key, metric, context.work)?;
    undo.push(slot);
    if let Binder::Match {
        patterns: alternatives,
        ..
    } = binder
    {
        patterns::bind_key(
            &alternatives[cursor % alternatives.len()],
            &key,
            binding,
            undo,
            context,
        )
    } else {
        Ok(true)
    }
}

pub(super) fn visit<'input>(
    query: &Query,
    atoms: &ModelRows<'input>,
    outer: Option<&Binding<'input>>,
    context: &mut Interpreter<'input, '_, '_>,
    visitor: &mut Visitor<'input, '_>,
) -> Result<bool, Error> {
    context
        .work
        .step(1 + query.variables as u128 + query.binders.len() as u128)?;
    let mut binding = Binding::new(
        query.variables,
        outer,
        context.terms.read(),
        context.work.limits.max_term_storage_bytes,
        context.work,
    )?;
    let mut alternatives: Vec<Option<Alternatives>> = context.work.reserve(query.binders.len())?;
    alternatives.resize_with(query.binders.len(), || None);
    let result = (|| {
        if query.binders.is_empty() {
            return complete(query, atoms, &mut binding, context, visitor);
        }
        let mut cursors = cursors(query, atoms, context)?;
        let mut undos = undo_slots(query, context.work)?;
        let mut depth = 0;
        loop {
            context.work.step(1 + undos[depth].len() as u128)?;
            for slot in undos[depth].drain(..) {
                binding.clear(slot, context.work);
            }
            let choice = cursors[depth].next(
                &query.binders[depth],
                &binding,
                &mut alternatives[depth],
                context,
            )?;
            let Some(choice) = choice else {
                if let Some(values) = alternatives[depth].take() {
                    context.work.local_bytes -= values.bytes();
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
                context,
            )? {
                continue;
            }
            if depth + 1 == query.binders.len() {
                if !complete(query, atoms, &mut binding, context, visitor)? {
                    return Ok(false);
                }
            } else {
                depth += 1;
            }
        }
        Ok(true)
    })();
    // The visitor may retain aggregate keys in an enclosing scope. Release only
    // this query's charges, on exhaustion, early termination and errors alike.
    context.work.local_bytes -= alternatives
        .iter()
        .flatten()
        .map(Alternatives::bytes)
        .sum::<u128>();
    binding.release(context.work);
    result
}
