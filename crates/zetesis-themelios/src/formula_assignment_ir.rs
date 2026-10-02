//! Explicit scope checks for finite aggregate equality binders and consumers.

use crate::formula_support::components::{Pattern as AtomPattern, Term};
use themelios_program::program::{
    Aggregate, AggregateFunction, Body, BodyElement, DefaultNegation, Relation,
};
use zetesis_core::TemplateTerm;

use crate::diagnostic::unsupported;
use crate::formula_conditional_ir::{Consequent, ConsequentOperand};
use crate::formula_ir::{
    AggregateElementIr, AggregateGuard, AggregateIr, AggregateKey, Compiler, Expression, LiteralIr,
    Operation, Projection, Variables,
};
use crate::{ExpansionResource, FormulaFailure, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn assignment_targets(
        &self,
        source: &Body,
        guards: &[Vec<AggregateGuard>],
        variables: &mut Variables,
    ) -> Result<Vec<Option<usize>>, FormulaFailure> {
        let mut result = Vec::new();
        for (element, guards) in source
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
                        | LiteralIr::HeadGuard(_)
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
                    // A nonbinding aggregate reads guards, element keys and
                    // element conditions from the complete proposal row, with
                    // independently scoped local witnesses. Its comparison
                    // and default negation remain original formula operations;
                    // the guard neither generates values nor filters support.
                    if other.binding != Some(target) {
                        consumed |= self.literal_uses(literal, target)?;
                    }
                    continue;
                }
                // Other consumers keep their established restrictions.
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

    fn pattern_uses(&mut self, atom: AtomPattern, variable: usize) -> Result<bool, FormulaFailure> {
        let atom = self
            .source
            .pattern_ref(atom, self.limits, self.counters, self.location)?;
        self.budget.charge(
            ExpansionResource::TermWork,
            atom.terms().len() as u128,
            self.location,
        )?;
        Ok(atom
            .terms()
            .iter()
            .any(|term| matches!(term, TemplateTerm::Variable(slot) if slot == variable)))
    }

    fn projection_uses(
        &mut self,
        projection: &Projection,
        variable: usize,
    ) -> Result<bool, FormulaFailure> {
        match projection {
            Projection::Arguments { terms, .. } => {
                self.scope_work(terms.len())?;
                Ok(terms.contains(&Some(Term::Variable(variable))))
            }
            Projection::Witnesses {
                bindings, inputs, ..
            } => {
                if variable >= *inputs {
                    return Ok(false);
                }
                for literal in bindings {
                    if self.literal_uses(literal, variable)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    pub(super) fn element_uses(
        &mut self,
        element: &AggregateElementIr,
        variable: usize,
    ) -> Result<bool, FormulaFailure> {
        // Callers supply an outer-frame slot. Element-local slots extend that
        // frame, so matching an inherited index retains correlation without
        // treating local witnesses or later synthetic head slots as inputs.
        self.scope_work(1)?;
        let key_uses = match &element.key {
            AggregateKey::Tuple(terms) => {
                self.scope_work(terms.len())?;
                terms.contains(&Term::Variable(variable))
            }
            AggregateKey::Atom(atom) => self.pattern_uses(*atom, variable)?,
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
            LiteralIr::Atom(_, atom) => self.pattern_uses(*atom, variable)?,
            LiteralIr::PatternAtom(pattern) => {
                let flat = self.source.pattern_ref(
                    pattern.atom,
                    self.limits,
                    self.counters,
                    self.location,
                )?;
                let pattern = pattern.bind(flat);
                self.budget.charge(
                    ExpansionResource::TermWork,
                    pattern.node_count() as u128,
                    self.location,
                )?;
                pattern.slots().any(|slot| slot == variable)
            }
            LiteralIr::ProjectedAtom(_, projection) => {
                self.projection_uses(projection, variable)?
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
            LiteralIr::Guard(guard) | LiteralIr::HeadGuard(guard) => {
                for expression in guard.expressions() {
                    if self.expression_uses(expression, variable)? {
                        return Ok(true);
                    }
                }
                false
            }
            LiteralIr::Conditional(conditional) => {
                if self.consequent_uses(&conditional.consequent, variable)? {
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
                // Whether the aggregate supplies an output does not change
                // its element reads. In particular, a comparison aggregate
                // can consume another aggregate's completed proposal.
                for element in &aggregate.elements {
                    if self.element_uses(element, variable)? {
                        return Ok(true);
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

    /// Visit every local alternative before the enclosing condition scope is read.
    fn consequent_uses(
        &mut self,
        consequent: &Consequent,
        variable: usize,
    ) -> Result<bool, FormulaFailure> {
        let mut uses = false;
        match consequent {
            Consequent::Atoms(_, alternatives) => {
                for alternative in alternatives {
                    uses |= match &alternative.operand {
                        ConsequentOperand::Atom(atom) => self.pattern_uses(*atom, variable)?,
                        ConsequentOperand::Projection(projection) => {
                            self.projection_uses(projection, variable)?
                        }
                    };
                    for literal in &alternative.bindings {
                        uses |= self.literal_uses(literal, variable)?;
                    }
                }
            }
            Consequent::Guards(alternatives) => {
                for alternative in alternatives {
                    for expression in alternative.guard.expressions() {
                        uses |= self.expression_uses(expression, variable)?;
                    }
                    for literal in &alternative.bindings {
                        uses |= self.literal_uses(literal, variable)?;
                    }
                }
            }
            Consequent::Guard(guard) => {
                for expression in guard.expressions() {
                    uses |= self.expression_uses(expression, variable)?;
                }
            }
        }
        Ok(uses)
    }
}
