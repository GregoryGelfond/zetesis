//! Explicit scope checks for finite aggregate equality binders and consumers.

use themelios_program::program::{
    Aggregate, AggregateFunction, BodyElement, DefaultNegation, Relation, Rule,
};
use zetesis_core::{AtomPattern, Term};

use crate::diagnostic::unsupported;
use crate::formula_ir::{
    AggregateElementIr, AggregateGuard, AggregateIr, AggregateKey, Compiler, Expression, LiteralIr,
    Operation, Variables,
};
use crate::{ExpansionResource, FormulaFailure, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn assignment_targets(
        &self,
        rule: &Rule,
        guards: &[Vec<AggregateGuard>],
        variables: &mut Variables,
    ) -> Result<Vec<Option<usize>>, FormulaFailure> {
        let mut result = Vec::new();
        for (element, guards) in rule
            .body()
            .get()
            .elements()
            .filter(|element| matches!(element.get(), BodyElement::Aggregate { .. }))
            .zip(guards)
        {
            let [guard] = guards.as_slice() else {
                result.push(None);
                continue;
            };
            let [Operation::Variable(target)] = guard.bound.nodes.as_slice() else {
                result.push(None);
                continue;
            };
            if guard.relation != Relation::Eq || variables.safe.contains(target) {
                result.push(None);
                continue;
            }
            let BodyElement::Aggregate {
                negation: DefaultNegation::None,
                aggregate: Aggregate::Function(aggregate),
            } = element.get()
            else {
                result.push(None);
                continue;
            };
            if !matches!(
                aggregate.function(),
                AggregateFunction::Count
                    | AggregateFunction::Sum
                    | AggregateFunction::SumPlus
                    | AggregateFunction::Min
                    | AggregateFunction::Max
            ) || !variables.named.values().any(|variable| variable == target)
            {
                return Err(unsupported(ProfileFeature::AggregateAssignment, self.location).into());
            }
            variables.safe.insert(*target);
            result.push(Some(*target));
        }
        Ok(result)
    }
    pub(super) fn assignment_scope(
        &mut self,
        aggregate: &AggregateIr,
    ) -> Result<(), FormulaFailure> {
        let Some(target) = aggregate.binding else {
            return Ok(());
        };
        for element in &aggregate.elements {
            if self.element_uses(element, target)? {
                return Err(FormulaFailure::UnsafeVariable {
                    variable: target,
                    location: self.location,
                });
            }
        }
        Ok(())
    }
    pub(super) fn assignment_context(
        &mut self,
        body: &[LiteralIr],
        choice_guards: &[AggregateGuard],
        aggregate_values: &[bool],
    ) -> Result<bool, FormulaFailure> {
        let mut consumed = false;
        for (target, dependent) in aggregate_values.iter().enumerate() {
            self.scope_work(1)?;
            if !dependent {
                continue;
            }
            for guard in choice_guards {
                // A head bound consumes the completed proposal row. The
                // original aggregate equality still guards the entire choice;
                // neither a proposal nor its cardinality establishes support.
                consumed |= self.expression_uses(&guard.bound, target)?;
            }
            for literal in body {
                self.scope_work(1)?;
                // Outer body consumers share one contract across all heads:
                // consume completed proposal values while retaining the
                // original equality and body activation. Local element scopes
                // are compiled separately from this completed outer frame.
                if matches!(
                    literal,
                    LiteralIr::Atom(DefaultNegation::Not | DefaultNegation::NotNot, _)
                        | LiteralIr::ProjectedAtom(
                            DefaultNegation::Not | DefaultNegation::NotNot,
                            _,
                        )
                        | LiteralIr::Compare(..)
                        | LiteralIr::ArgumentCheck { .. }
                        | LiteralIr::TupleCompare(..)
                        | LiteralIr::Guard(_)
                        | LiteralIr::Bind { .. }
                        | LiteralIr::Range { .. }
                        | LiteralIr::Conditional(_)
                ) {
                    // Negative gates read completed values without binding or
                    // pruning possible rows. Grounding retains their original
                    // polarity, including the existential projection formula.
                    // Range generators follow the checked plan; a previously
                    // bound range target remains a completed-row membership test.
                    // Universal conditions seed their local joins with this
                    // complete outer frame. Their implications and consequent
                    // alternatives remain formulas, never support-row filters.
                    consumed |= self.literal_uses(literal, target)?;
                    continue;
                }
                if let LiteralIr::Aggregate(other) = literal {
                    // Its own equality guard supplies the target; scope checks
                    // forbid reading that target inside its own elements.
                    // Another producer may read it only after the checked plan
                    // completes its inputs. Such a dependency is still a value
                    // consumer for the objective-observer admission contract.
                    if let Some(produced) = other.binding {
                        if produced != target {
                            consumed |= self.literal_uses(literal, target)?;
                        }
                        continue;
                    }
                }
                // Non-binding comparisons retain their established completed-
                // row element scopes. Other consumers keep their restrictions.
                if self.literal_uses(literal, target)? {
                    return Err(
                        unsupported(ProfileFeature::AggregateAssignment, self.location).into(),
                    );
                }
            }
        }
        Ok(consumed)
    }

    pub(super) fn scope_work(&mut self, count: usize) -> Result<(), FormulaFailure> {
        self.budget
            .charge(ExpansionResource::TermWork, count as u128, self.location)?;
        Ok(())
    }

    pub(super) fn expression_uses(
        &mut self,
        expression: &Expression,
        variable: usize,
    ) -> Result<bool, FormulaFailure> {
        self.scope_work(expression.nodes.len())?;
        Ok(expression.inputs().any(|input| input == variable))
    }

    fn pattern_uses(
        &mut self,
        atom: &AtomPattern,
        variable: usize,
    ) -> Result<bool, FormulaFailure> {
        self.scope_work(atom.terms().len())?;
        Ok(atom.terms().contains(&Term::Variable(variable)))
    }

    pub(super) fn element_uses(
        &mut self,
        element: &AggregateElementIr,
        variable: usize,
    ) -> Result<bool, FormulaFailure> {
        self.scope_work(1)?;
        let key_uses = match &element.key {
            AggregateKey::Tuple(terms) => {
                self.scope_work(terms.len())?;
                terms.contains(&Term::Variable(variable))
            }
            AggregateKey::Atom(atom) => self.pattern_uses(atom, variable)?,
        };
        if key_uses {
            return Ok(true);
        }
        for literal in &element.condition {
            if self.literal_uses(literal, variable)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn literal_uses(
        &mut self,
        literal: &LiteralIr,
        variable: usize,
    ) -> Result<bool, FormulaFailure> {
        self.scope_work(1)?;
        Ok(match literal {
            LiteralIr::Atom(_, atom) => self.pattern_uses(atom, variable)?,
            LiteralIr::PatternAtom(pattern) => {
                self.scope_work(pattern.node_count())?;
                pattern.slots().any(|slot| slot == variable)
            }
            LiteralIr::ProjectedAtom(_, projection) => {
                self.scope_work(projection.terms.len())?;
                projection.terms.contains(&Some(Term::Variable(variable)))
            }
            LiteralIr::Compare(left, _, right)
            | LiteralIr::ArgumentCheck {
                captured: left,
                value: right,
            } => self.expression_uses(left, variable)? || self.expression_uses(right, variable)?,
            LiteralIr::TupleCompare(left, _, right) => {
                for term in left.iter().chain(right) {
                    if self.expression_uses(term, variable)? {
                        return Ok(true);
                    }
                }
                false
            }
            LiteralIr::Guard(guard) => {
                for expression in guard.expressions() {
                    if self.expression_uses(expression, variable)? {
                        return Ok(true);
                    }
                }
                false
            }
            LiteralIr::Conditional(conditional) => {
                let consequent_uses = match &conditional.consequent {
                    crate::formula_conditional_ir::Consequent::Atoms(_, alternatives) => {
                        let mut uses = false;
                        for alternative in alternatives {
                            uses |= self.pattern_uses(&alternative.atom, variable)?;
                            for literal in &alternative.bindings {
                                uses |= self.literal_uses(literal, variable)?;
                            }
                        }
                        uses
                    }
                    crate::formula_conditional_ir::Consequent::Guard(guard) => {
                        let mut uses = false;
                        for expression in guard.expressions() {
                            uses |= self.expression_uses(expression, variable)?;
                        }
                        uses
                    }
                };
                if consequent_uses {
                    return Ok(true);
                }
                for literal in &conditional.condition {
                    if self.literal_uses(literal, variable)? {
                        return Ok(true);
                    }
                }
                false
            }
            LiteralIr::Aggregate(aggregate) => {
                for guard in &aggregate.guards {
                    if self.expression_uses(&guard.bound, variable)? {
                        return Ok(true);
                    }
                }
                if aggregate.binding.is_some() {
                    for element in &aggregate.elements {
                        if self.element_uses(element, variable)? {
                            return Ok(true);
                        }
                    }
                }
                false
            }
            LiteralIr::Bind { target, value } => {
                *target == variable || self.expression_uses(value, variable)?
            }
            LiteralIr::Range {
                target,
                lower,
                upper,
                ..
            } => {
                *target == variable
                    || self.expression_uses(lower, variable)?
                    || self.expression_uses(upper, variable)?
            }
        })
    }
}
