//! Necessary integer equality/range postings for explicitly selected traversals.
//!
//! The query certificate covers every row of one source occurrence, not a
//! meet of argument domains or a guessed integer type. The opposite operand
//! must evaluate to an integer in the current prefix. A skipped row therefore
//! either cannot match the occurrence or makes a plan-decided comparison defined
//! and false. An ordering key is one column; equality may use a checked
//! expression of columns. The ordinary residual comparison remains in the join.
//!
//! Flat normal source bodies opt in during eager emission or possible-head
//! production. Family evidence, hybrid capture/checks and direct-source exact
//! activity never opt in. Generated bindings/heads and structural or nested
//! scopes retain their old traversal. Every round query uses the occurrence's
//! first-new interval against the one immutable snapshot index.

use themelios_program::program::Relation;

use super::queries::expression_keys::{buffer, column, evaluate, number, tick};
use super::relations::RelationRows;
use super::{Computation, Context, Join, PatternOccurrence, PositivePattern, Probe, delta};
use crate::FormulaFailure;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};

/// The outer body passed the explicit numeric-selection admission check.
/// Data totality is established separately when an index is built.
pub(super) struct Eligible;

#[cfg(test)]
mod tests;

impl<'a, 'source> Join<'a, 'source> {
    /// Called only by eager instantiation or possible-head production.
    /// No totality premise is borrowed from original-family admission: a
    /// defined false plan-decided comparison excludes that row under the
    /// ordinary filters contract, including independent deferred scalar faults.
    /// A source with generated bindings or structural/nested scopes retains
    /// its current traversal until that broader failure inventory is reviewed.
    pub(crate) fn select_numeric_probes(
        &mut self,
        rule: &RuleIr,
        limits: &crate::FormulaLimits,
        counters: &mut super::Counters,
    ) -> Result<(), FormulaFailure> {
        counters.work(limits, rule.location)?;
        if !std::ptr::eq(self.literals.get(), rule.body.as_slice())
            || !matches!(rule.head, HeadIr::Normal(_))
            || rule.variables != rule.body_variables
        {
            return Ok(());
        }
        let mut comparison_found = false;
        for literal in &rule.body {
            counters.work(limits, rule.location)?;
            match literal {
                LiteralIr::Compare(comparison) => {
                    comparison_found |= selected_relation(comparison.relation());
                }
                LiteralIr::Atom(..) | LiteralIr::TupleCompare(..) => {}
                _ => return Ok(()),
            }
        }
        self.expression_keys = comparison_found.then_some(Eligible);
        Ok(())
    }

    pub(super) fn expression_key_probe(
        &self,
        pattern: PatternOccurrence<'a>,
        source: Option<&'source RelationRows<'source>>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Probe<'a, 'source>>, FormulaFailure> {
        if self.expression_keys.is_none() || self.source_evidence {
            return Ok(None);
        }
        #[cfg(test)]
        if tests::disabled() {
            return Ok(None);
        }
        let (PositivePattern::Flat(flat), Some(source)) = (pattern.pattern, source) else {
            return Ok(None);
        };
        if source.row_count() == 0 {
            return Ok(None);
        }
        // Preserve expression-equality selection ahead of range selection:
        // a diagonal equality must not be displaced by an earlier R<S filter.
        for equality in [true, false] {
            for index in self.plan.decisions.decided_at(self.depth) {
                tick(&mut context)?;
                let Some(comparison) = super::comparison(&self.literals[index]) else {
                    continue;
                };
                let ([left, right], relation) = comparison.operands();
                if !selected_relation(relation) || (relation == Relation::Eq) != equality {
                    continue;
                }
                for (key, bound, relation) in
                    [(left, right, relation), (right, left, reversed(relation))]
                {
                    if let Some(probe) = self.numeric_candidate(
                        ([key, bound], relation),
                        flat,
                        source,
                        pattern.source,
                        &mut context,
                    )? {
                        return Ok(Some(probe));
                    }
                }
            }
        }
        Ok(None)
    }

    fn numeric_candidate(
        &self,
        comparison: ([&Expression; 2], Relation),
        pattern: zetesis_core::PatternRef<'_>,
        source: &'source RelationRows<'source>,
        occurrence: usize,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Probe<'a, 'source>>, FormulaFailure> {
        let ([key, bound], relation) = comparison;
        // Stage 2a indexes one column, while 2b allows a checked expression.
        if relation != Relation::Eq && !matches!(key.nodes.as_slice(), [Operation::Variable(_)]) {
            return Ok(None);
        }
        if !self.key_available(key, bound, pattern, context)? {
            return Ok(None);
        }
        let binding = self.values.view(
            context.computation.read(),
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        // The ordinary equality posting remains the first choice, including
        // its existing delta clipping and residual alias/constant checks.
        if let Some(posting) = self.support.probe_at(
            Some(source),
            pattern,
            binding,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )? {
            return self
                .indexed_probe(
                    Some(posting),
                    Some(source),
                    occurrence,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )
                .map(Some);
        }
        let mut scratch = buffer(context)?;
        let wanted = evaluate(&bound.nodes, &mut scratch, context, |slot, context| {
            tick(context)?;
            let value =
                self.values
                    .read(slot, context.computation.read(), context.work.location)?;
            Ok(number(value.descriptor()))
        })?;
        let Some(wanted) = wanted else {
            return Ok(None);
        };
        let range = delta::interval(
            self.delta,
            occurrence,
            source.old_rows(),
            source.row_count(),
        );
        let query = Context::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        );
        if relation == Relation::Eq {
            self.support
                .expression_key(source, key, pattern, wanted, range, query)
                .map(|rows| rows.map(Probe::Expression))
        } else {
            self.support
                .range_key(source, key, pattern, (relation, wanted), range, query)
                .map(|rows| rows.map(Probe::Range))
        }
    }

    fn key_available(
        &self,
        key: &Expression,
        bound: &Expression,
        pattern: zetesis_core::PatternRef<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        let mut unbound = false;
        for operation in &key.nodes {
            tick(context)?;
            match operation {
                Operation::Variable(slot) => {
                    if column(pattern, *slot, context)?.is_none() {
                        return Ok(false);
                    }
                    unbound |= !self.values.is_bound(*slot, context.work.location)?;
                }
                Operation::Constructor(_) => return Ok(false),
                _ => {}
            }
        }
        if !unbound {
            return Ok(false);
        }
        for operation in &bound.nodes {
            tick(context)?;
            match operation {
                Operation::Variable(slot)
                    if !self.values.is_bound(*slot, context.work.location)? =>
                {
                    return Ok(false);
                }
                Operation::Constructor(_) => return Ok(false),
                _ => {}
            }
        }
        Ok(true)
    }
}

fn selected_relation(relation: Relation) -> bool {
    matches!(
        relation,
        Relation::Eq | Relation::Lt | Relation::Le | Relation::Gt | Relation::Ge
    )
}

fn reversed(relation: Relation) -> Relation {
    match relation {
        Relation::Lt => Relation::Gt,
        Relation::Le => Relation::Ge,
        Relation::Gt => Relation::Lt,
        Relation::Ge => Relation::Le,
        relation => relation,
    }
}
