//! Optional totality over complete positive source-column domains.
//!
//! Original-family admission, final emission, hybrid capture and admitted hybrid checks share this gate
//! over the same completed source carrier. Each expression in an eligible flat normal body
//! is either a leaf, reads one source variable covered by an ordinary positive
//! atom column, or has checked arithmetic over several numeric column bounds.
//! Products, quotients and remainders extend sums and differences;
//! the bounds establish every intermediate and exclude zero divisors.
//! Successful evaluation on that entire
//! column domain covers every complete rule binding, including rows a scalar
//! equality later excludes. An arithmetic failure declines the optimization;
//! the original complete traversal remains responsible for its diagnostics.
//! Hybrid capture first performs this preparation with append-capable terms;
//! the first frozen check repeats it against the immutable rule and full source
//! columns. Successful maps remain immutable across later cursors, independently
//! of candidate selection. Missing frozen values remain errors.

use themelios_program::program::DefaultNegation;
use zetesis_core::TemplateTerm;

use super::{Projections, tick};
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::relations::{Postings, RelationRows};
use crate::formula_support::{Computation, Context, Counters, Evaluation, Support};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits};

mod numeric;
pub(in crate::formula_support) use numeric::positive_bounds;

/// Preparation serves either partial scalar families or total arithmetic whose
/// certificate admits incremental source checking. This does not redefine which
/// source families require a defined witness: that remains `family::partial`.
pub(in crate::formula_support) fn needed(literals: &[LiteralIr]) -> bool {
    let arithmetic = |expression: &Expression| expression.nodes.iter().any(numeric::arithmetic);
    crate::formula_support::family::partial(literals)
        || literals.iter().any(|literal| match literal {
            LiteralIr::Compare(left, _, right) => arithmetic(left) || arithmetic(right),
            LiteralIr::TupleCompare(left, _, right) => left.iter().chain(right).any(arithmetic),
            _ => false,
        })
}

/// Whether a flat rule uses the source-column certificate's arithmetic grammar.
/// This gate changes no binding schedule and supplies no numeric or definedness
/// premise. Only successful preparation over the completed source does that.
pub(crate) fn supports_column_totality(
    rule: &RuleIr,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    for literal in &rule.body {
        counters.work(limits, rule.location)?;
        let expressions = match literal {
            LiteralIr::Atom(..) => continue,
            LiteralIr::Compare(left, _, right) => std::slice::from_ref(left)
                .iter()
                .chain(std::slice::from_ref(right)),
            LiteralIr::TupleCompare(left, _, right) => left.iter().chain(right),
            _ => return Ok(false),
        };
        for expression in expressions {
            for node in &expression.nodes {
                counters.work(limits, rule.location)?;
                if !numeric::supports(node) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

struct Domain<'source> {
    rows: &'source RelationRows<'source>,
    column: usize,
}

impl<'a> Projections<'a> {
    /// Use completed, candidate-independent source columns. Success establishes
    /// scalar totality for complete positive bindings and permits post-admission
    /// selection. A separately covered flat original family may omit its scan
    /// only when all normalized siblings succeed. Evidence cursors themselves
    /// still traverse complete families; this proof supplies no defined witness.
    pub(in crate::formula_support) fn total_body(
        &mut self,
        rule: &'a RuleIr,
        support: &Support<'_>,
        evaluation: &mut Evaluation,
        mut context: Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        tick(&mut context)?;
        context
            .computation
            .allowance(&self.lease, context.work.limits, context.work.location)?;
        if !eligible(rule, support, &mut context)? {
            return Ok(false);
        }
        self.prepare(
            &rule.body,
            Context::new(
                &*context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            ),
        )?;
        let mut binding = Binding::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        binding.extend_scope(
            rule.body_variables,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let mut numeric = numeric::Preparation::new(rule, support);
        for (index, literal) in rule.body.iter().enumerate() {
            tick(&mut context)?;
            match literal {
                LiteralIr::Compare(left, _, right) => {
                    for (side, expression) in [left, right].into_iter().enumerate() {
                        if !self.total_expression(
                            (Some((index, side)), expression),
                            &mut numeric,
                            &mut binding,
                            evaluation,
                            &mut context,
                        )? {
                            return Ok(false);
                        }
                    }
                }
                LiteralIr::TupleCompare(left, _, right) => {
                    for expression in left.iter().chain(right) {
                        if !self.total_expression(
                            (None, expression),
                            &mut numeric,
                            &mut binding,
                            evaluation,
                            &mut context,
                        )? {
                            return Ok(false);
                        }
                    }
                }
                LiteralIr::Guard(guard) | LiteralIr::HeadGuard(guard) => {
                    for expression in guard.expressions() {
                        if !self.total_expression(
                            (None, expression),
                            &mut numeric,
                            &mut binding,
                            evaluation,
                            &mut context,
                        )? {
                            return Ok(false);
                        }
                    }
                }
                LiteralIr::Atom(..) => {}
                _ => unreachable!("eligible flat normal body checked above"),
            }
        }
        let Self::Local(values) = self else {
            unreachable!("totality preparation owns its projection values");
        };
        for projection in &mut values.values {
            tick(&mut context)?;
            projection.covered = Some(projection.results.len());
        }
        Ok(true)
    }

    fn total_expression(
        &mut self,
        (source, expression): (Option<(usize, usize)>, &Expression),
        numeric: &mut numeric::Preparation<'_, '_>,
        binding: &mut Binding<'static>,
        evaluation: &mut Evaluation,
        context: &mut Context<'_, &mut Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        context.work.counters.charge_work(
            expression.nodes.len() as u128,
            context.work.limits,
            context.work.location,
        )?;
        // Constructor admission has its own failure variants. Its operational
        // totality is not established by this numeric expression certificate.
        if expression
            .nodes
            .iter()
            .any(|node| matches!(node, Operation::Constructor(_)))
        {
            return Ok(false);
        }
        if let [Operation::Constant(_)] = expression.nodes.as_slice() {
            return admitted(evaluation.source_expression(
                expression,
                |slot| binding.key(slot, context.work.location),
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            ));
        }
        let mut inputs = expression.inputs();
        let Some(slot) = inputs.next() else {
            return Ok(false);
        };
        if !inputs.all(|input| input == slot) {
            return numeric.total(expression, context);
        }
        let Some(domain) = numeric.domain(slot, context)? else {
            return Ok(false);
        };
        if matches!(expression.nodes.as_slice(), [Operation::Variable(_)]) {
            // Complete source columns already contain admitted canonical terms.
            // A direct variable leaf performs no partial arithmetic.
            return Ok(true);
        }
        let Postings::Indexed(postings) = domain.rows.postings(domain.column) else {
            unreachable!("a domain column keeps postings");
        };
        for posting in postings.values() {
            tick(context)?;
            let position = *posting.first().expect("a distinct input has a source row");
            let row = domain
                .rows
                .row(position)
                .expect("retained source row position");
            let value = row.value(domain.column).expect("checked source column");
            let input = context
                .computation
                .read()
                .term_key(value)
                .map_err(|error| {
                    crate::formula_binding::assignment(error.into(), context.work.location)
                })?;
            binding.set(
                slot,
                &input,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
            let result = if let Some(source) = source {
                self.value(source, expression, binding, evaluation, context)
            } else {
                evaluation.source_expression(
                    expression,
                    |slot| binding.key(slot, context.work.location),
                    context.computation,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )
            };
            if !admitted(result)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// The finite column argument covers only a flat normal body over ordinary
/// positive source rows. Empty relations need no speculative arithmetic.
fn eligible(
    rule: &RuleIr,
    support: &Support<'_>,
    context: &mut Context<'_, &mut Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    if !matches!(rule.head, HeadIr::Normal(_)) || rule.variables != rule.body_variables {
        return Ok(false);
    }
    for literal in &rule.body {
        tick(context)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, atom) => {
                let pattern = context.computation.static_pattern(
                    *atom,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )?;
                let rows = support.resolve(
                    pattern,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )?;
                if rows.is_none_or(|rows| rows.row_count() == 0) {
                    return Ok(false);
                }
            }
            LiteralIr::Atom(..)
            | LiteralIr::Compare(..)
            | LiteralIr::TupleCompare(..)
            | LiteralIr::Guard(_)
            | LiteralIr::HeadGuard(_) => {}
            // No generated bindings, structural captures, projections or
            // nested families are certified by a flat column domain.
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// A complete input domain may overapproximate actual complete bindings.
/// Arithmetic failure there is therefore not a source diagnostic. Resource,
/// owner, prefix and other typed refusals remain failures of this operation.
fn admitted<T>(result: Result<T, FormulaFailure>) -> Result<bool, FormulaFailure> {
    match result {
        Ok(_) => Ok(true),
        Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })) => Ok(false),
        Err(error) => Err(error),
    }
}

fn domain<'source>(
    slot: usize,
    literals: &[LiteralIr],
    support: &Support<'source>,
    context: &mut Context<'_, &mut Computation<'_, '_>>,
) -> Result<Option<Domain<'source>>, FormulaFailure> {
    let mut selected: Option<Domain<'source>> = None;
    for literal in literals {
        tick(context)?;
        let LiteralIr::Atom(DefaultNegation::None, atom) = literal else {
            continue;
        };
        let pattern = context.computation.static_pattern(
            *atom,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let terms = pattern.terms();
        for column in 0..terms.len() {
            tick(context)?;
            if !matches!(terms.at(column), Some(TemplateTerm::Variable(input)) if input == slot) {
                continue;
            }
            let Some(rows) = support.resolve(
                pattern,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?
            else {
                return Ok(None);
            };
            if rows.row_count() == 0 {
                return Ok(None);
            }
            // A column without postings has no recorded distinct values: it is
            // declined, never read as an empty domain.
            let Postings::Indexed(values) = rows.postings(column) else {
                continue;
            };
            if selected.as_ref().is_none_or(|previous| {
                let Postings::Indexed(known) = previous.rows.postings(previous.column) else {
                    unreachable!("a selected domain column keeps postings");
                };
                values.len() < known.len()
            }) {
                selected = Some(Domain { rows, column });
            }
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests;
