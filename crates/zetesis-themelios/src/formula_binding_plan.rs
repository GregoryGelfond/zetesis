//! Dependency scheduling for scalar and interval binding instructions.
//!
//! Positive relational occurrences supply the initial safe slots. A literal
//! may bind a new slot only after every value dependency is safe. Whole guards
//! can also propose scalar equalities and closed finite integer chains; their
//! complete original test remains. No global domain search or inversion is used.

use themelios_program::program::Relation;

use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
use crate::{ExpansionResource, FormulaFailure};

#[derive(Clone, Copy)]
enum Binding {
    Left(usize),
    Right(usize),
    Range(usize),
}

impl Compiler<'_> {
    /// Preserve ordinary literals and existing instructions, then append new
    /// binders in dependency order. Bound targets remain equality/membership
    /// tests. Unresolved dependencies remain for the existing safety diagnostic.
    /// The caller keeps previously generated targets in `variables.safe` when
    /// invoking this pass again on an already planned body.
    pub(super) fn bindings(
        &mut self,
        body: &mut Vec<LiteralIr>,
        variables: &mut Variables,
    ) -> Result<(), FormulaFailure> {
        self.budget.charge(
            ExpansionResource::TermWork,
            body.len() as u128,
            self.location,
        )?;
        let mut pending: Vec<_> = std::mem::take(body).into_iter().map(Some).collect();
        let mut generated = Vec::new();
        loop {
            let mut progress = false;
            for slot in &mut pending {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                let Some(literal) = slot.as_ref() else {
                    continue;
                };
                if let LiteralIr::TupleCompare(left, Relation::Eq, right) = literal
                    && let Some((target, instruction)) =
                        self.tuple_binding(left, right, variables)?
                {
                    variables.safe.insert(target);
                    generated.push(instruction);
                    progress = true;
                    continue;
                }
                if let LiteralIr::Guard(guard) = literal
                    && let Some((target, instruction)) = self.guard_binding(guard, variables)?
                {
                    variables.safe.insert(target);
                    generated.push(instruction);
                    progress = true;
                    continue;
                }
                let binding = self.binding(literal, variables)?;
                let Some(binding) = binding else {
                    continue;
                };
                let literal = slot.take().expect("inspected pending instruction");
                let (target, instruction) = match (binding, literal) {
                    (Binding::Left(target), LiteralIr::Compare(_, Relation::Eq, value))
                    | (Binding::Right(target), LiteralIr::Compare(value, Relation::Eq, _)) => {
                        (target, LiteralIr::Bind { target, value })
                    }
                    (Binding::Range(target), LiteralIr::Range { lower, upper, .. }) => (
                        target,
                        LiteralIr::Range {
                            target,
                            lower,
                            upper,
                            binder: true,
                        },
                    ),
                    _ => unreachable!("binding decision matches its inspected literal"),
                };
                variables.safe.insert(target);
                generated.push(instruction);
                progress = true;
            }
            if !progress {
                if variables.safe.len() == variables.count {
                    break;
                }
                let Some((target, instruction)) = self.conjunction_binding(&pending, variables)?
                else {
                    break;
                };
                variables.safe.insert(target);
                generated.push(instruction);
            }
        }
        for slot in pending {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if let Some(literal) = slot {
                body.push(literal);
            }
        }
        body.extend(generated);
        Ok(())
    }

    fn binding(
        &mut self,
        literal: &LiteralIr,
        variables: &Variables,
    ) -> Result<Option<Binding>, FormulaFailure> {
        match literal {
            LiteralIr::Compare(left, Relation::Eq, right) => {
                if let Some(target) = whole_variable(left)
                    && !variables.safe.contains(&target)
                    && self.binding_ready(right, variables)?
                {
                    return Ok(Some(Binding::Left(target)));
                }
                if let Some(target) = whole_variable(right)
                    && !variables.safe.contains(&target)
                    && self.binding_ready(left, variables)?
                {
                    return Ok(Some(Binding::Right(target)));
                }
                Ok(None)
            }
            LiteralIr::Range {
                target,
                lower,
                upper,
                binder: false,
            } if !variables.safe.contains(target)
                && self.binding_ready(lower, variables)?
                && self.binding_ready(upper, variables)? =>
            {
                Ok(Some(Binding::Range(*target)))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn binding_ready(
        &mut self,
        expression: &Expression,
        variables: &Variables,
    ) -> Result<bool, FormulaFailure> {
        for node in &expression.nodes {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if let Operation::Variable(variable) = node
                && !variables.safe.contains(variable)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

pub(super) fn whole_variable(expression: &Expression) -> Option<usize> {
    if let [Operation::Variable(variable)] = expression.nodes.as_slice() {
        Some(*variable)
    } else {
        None
    }
}
