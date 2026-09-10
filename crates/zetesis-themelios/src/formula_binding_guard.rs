//! Finite binding candidates extracted without removing the original whole guard.
//!
//! Scalar equality may supply a value once all its dependencies are safe. Integer
//! chains require closed numeric endpoints: relationally bound endpoints are not
//! sufficient for the source comparison-domain safety contract. Single default
//! negation supplies no generator. Directed integer-affine envelopes can cover
//! several unresolved variables after established generators stall. Every
//! extracted candidate still passes the unchanged complete guard.

mod envelope;

use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::Value;

use crate::formula_binding_plan::whole_variable;
use crate::formula_guard::{Guard, GuardComparison};
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
use crate::formula_support::copy;
use crate::{ExpansionResource, FormulaFailure};

impl Compiler<'_> {
    /// Recover a finite affine envelope only after established generators stall.
    /// The caller retains every original guard and all earlier binding evidence.
    pub(super) fn chain_binding(
        &mut self,
        pending: &[Option<LiteralIr>],
        generated: &[LiteralIr],
        variables: &Variables,
    ) -> Result<Option<(usize, LiteralIr)>, FormulaFailure> {
        let range = envelope::binding(
            pending.iter().flatten().chain(generated),
            variables.count,
            &variables.safe,
            self.budget,
            self.location,
        )?;
        Ok(range.map(|(target, lower, upper)| {
            (
                target,
                LiteralIr::Range {
                    target,
                    lower: constant(lower),
                    upper: constant(upper),
                    binder: true,
                },
            )
        }))
    }

    pub(super) fn guard_binding(
        &mut self,
        guard: &Guard,
        variables: &Variables,
    ) -> Result<Option<(usize, LiteralIr)>, FormulaFailure> {
        let Guard::Comparisons {
            negation: DefaultNegation::None | DefaultNegation::NotNot,
            comparisons,
        } = guard
        else {
            return Ok(None);
        };
        for comparison in comparisons {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            let binding = match comparison {
                GuardComparison::Scalar(left, Relation::Eq, right) => {
                    self.equality_binding(left, right, variables)?
                }
                GuardComparison::Tuple(left, Relation::Eq, right) => {
                    self.tuple_binding(left, right, variables)?
                }
                GuardComparison::Range(value, lower, upper) => {
                    self.range_binding(value, lower, upper, variables)?
                }
                _ => None,
            };
            if binding.is_some() {
                return Ok(binding);
            }
        }
        for comparison in comparisons {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            let Some((target, _, _)) = numeric_bound(comparison) else {
                continue;
            };
            if variables.safe.contains(&target) || !self.only_missing(guard, target, variables)? {
                continue;
            }
            if let Some((lower, upper)) = self.integer_bounds(comparisons, target)? {
                self.budget
                    .charge(ExpansionResource::TermWork, 2, self.location)?;
                return Ok(Some((
                    target,
                    LiteralIr::Range {
                        target,
                        lower: constant(lower),
                        upper: constant(upper),
                        binder: true,
                    },
                )));
            }
        }
        Ok(None)
    }

    fn equality_binding(
        &mut self,
        left: &Expression,
        right: &Expression,
        variables: &Variables,
    ) -> Result<Option<(usize, LiteralIr)>, FormulaFailure> {
        for (target, value) in [(left, right), (right, left)] {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if let Some(target) = whole_variable(target)
                && !variables.safe.contains(&target)
                && self.binding_ready(value, variables)?
            {
                return Ok(Some((
                    target,
                    LiteralIr::Bind {
                        target,
                        value: self.binding_expression(value)?,
                    },
                )));
            }
        }
        Ok(None)
    }

    pub(super) fn tuple_binding(
        &mut self,
        left: &[Expression],
        right: &[Expression],
        variables: &Variables,
    ) -> Result<Option<(usize, LiteralIr)>, FormulaFailure> {
        // A whole opposite tuple must already be bound. Extracting independent
        // pairs first would incorrectly make (X,Y)=(Y,1) safe by bootstrapping Y.
        // Repeated targets still pass the retained complete tuple comparison.
        if left.len() != right.len() {
            return Ok(None);
        }
        for (targets, values) in [(left, right), (right, left)] {
            let mut ready = true;
            for value in values {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                ready &= self.binding_ready(value, variables)?;
            }
            if !ready {
                continue;
            }
            for (target, value) in targets.iter().zip(values) {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                if let Some(target) = whole_variable(target)
                    && !variables.safe.contains(&target)
                {
                    return Ok(Some((
                        target,
                        LiteralIr::Bind {
                            target,
                            value: self.binding_expression(value)?,
                        },
                    )));
                }
            }
        }
        Ok(None)
    }

    fn range_binding(
        &mut self,
        value: &Expression,
        lower: &Expression,
        upper: &Expression,
        variables: &Variables,
    ) -> Result<Option<(usize, LiteralIr)>, FormulaFailure> {
        if let Some(target) = whole_variable(value)
            && !variables.safe.contains(&target)
            && self.binding_ready(lower, variables)?
            && self.binding_ready(upper, variables)?
        {
            return Ok(Some((
                target,
                LiteralIr::Range {
                    target,
                    lower: self.binding_expression(lower)?,
                    upper: self.binding_expression(upper)?,
                    binder: true,
                },
            )));
        }
        Ok(None)
    }

    /// Combine independently closed bounds from conjunctive body literals.
    /// Single default negation and unresolved neighbors supply no domain.
    pub(super) fn conjunction_binding(
        &mut self,
        pending: &[Option<LiteralIr>],
        variables: &Variables,
    ) -> Result<Option<(usize, LiteralIr)>, FormulaFailure> {
        for target in 0..variables.count {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if variables.safe.contains(&target) {
                continue;
            }
            let mut bounds = IntegerBounds::default();
            for literal in pending.iter().flatten() {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                match literal {
                    LiteralIr::Compare(left, relation, right) => {
                        bounds.include(target, numeric_parts(left, *relation, right));
                    }
                    LiteralIr::Guard(
                        guard @ Guard::Comparisons {
                            negation: DefaultNegation::None | DefaultNegation::NotNot,
                            comparisons,
                        },
                    ) if self.only_missing(guard, target, variables)? => {
                        for comparison in comparisons {
                            self.budget
                                .charge(ExpansionResource::TermWork, 1, self.location)?;
                            bounds.include(target, numeric_bound(comparison));
                        }
                    }
                    _ => {}
                }
            }
            if let Some((lower, upper)) = bounds.interval() {
                self.budget
                    .charge(ExpansionResource::TermWork, 2, self.location)?;
                return Ok(Some((
                    target,
                    LiteralIr::Range {
                        target,
                        lower: constant(lower),
                        upper: constant(upper),
                        binder: true,
                    },
                )));
            }
        }
        Ok(None)
    }

    fn binding_expression(&mut self, value: &Expression) -> Result<Expression, FormulaFailure> {
        let mut nodes = Vec::new();
        for node in &value.nodes {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            nodes.push(match *node {
                Operation::Constant(ref value) => {
                    Operation::Constant(copy(value, self.budget, self.location)?)
                }
                Operation::Constructor(ref constructor) => {
                    Operation::Constructor(constructor.copy(self.budget, self.location)?)
                }
                Operation::Variable(variable) => Operation::Variable(variable),
                Operation::Unary(operator, operand) => Operation::Unary(operator, operand),
                Operation::Binary(operator, left, right) => {
                    Operation::Binary(operator, left, right)
                }
                Operation::Absolute(operand) => Operation::Absolute(operand),
            });
        }
        Ok(Expression { nodes })
    }

    fn only_missing(
        &mut self,
        guard: &Guard,
        target: usize,
        variables: &Variables,
    ) -> Result<bool, FormulaFailure> {
        for expression in guard.expressions() {
            for node in &expression.nodes {
                self.budget
                    .charge(ExpansionResource::TermWork, 1, self.location)?;
                if let Operation::Variable(variable) = node
                    && *variable != target
                    && !variables.safe.contains(variable)
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn integer_bounds(
        &mut self,
        comparisons: &[GuardComparison],
        target: usize,
    ) -> Result<Option<(i32, i32)>, FormulaFailure> {
        let mut bounds = IntegerBounds::default();
        for comparison in comparisons {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            bounds.include(target, numeric_bound(comparison));
        }
        Ok(bounds.interval())
    }
}

#[derive(Default)]
struct IntegerBounds {
    lower: Option<i64>,
    upper: Option<i64>,
}

impl IntegerBounds {
    fn include(&mut self, target: usize, bound: Option<(usize, bool, i64)>) {
        if let Some((variable, is_lower, value)) = bound
            && variable == target
        {
            if is_lower {
                self.lower = Some(self.lower.map_or(value, |previous| previous.max(value)));
            } else {
                self.upper = Some(self.upper.map_or(value, |previous| previous.min(value)));
            }
        }
    }

    fn interval(self) -> Option<(i32, i32)> {
        let (lower, upper) = (self.lower?, self.upper?);
        // Strict i32 endpoint comparisons can make an empty integer domain.
        // Calculate in i64 so that emptiness cannot wrap into a nonempty range.
        Some(if lower > upper {
            (1, 0)
        } else {
            (
                i32::try_from(lower).expect("nonempty lower endpoint lies inside i32"),
                i32::try_from(upper).expect("nonempty upper endpoint lies inside i32"),
            )
        })
    }
}

fn constant(value: i32) -> Expression {
    Expression {
        nodes: vec![Operation::Constant(Value::Number(value))],
    }
}

fn numeric_bound(comparison: &GuardComparison) -> Option<(usize, bool, i64)> {
    let GuardComparison::Scalar(left, relation, right) = comparison else {
        return None;
    };
    numeric_parts(left, *relation, right)
}

fn numeric_parts(
    left: &Expression,
    relation: Relation,
    right: &Expression,
) -> Option<(usize, bool, i64)> {
    let (target, relation, number) = if let Some(target) = whole_variable(left) {
        let [Operation::Constant(Value::Number(number))] = right.nodes.as_slice() else {
            return None;
        };
        (target, relation, *number)
    } else {
        let target = whole_variable(right)?;
        let [Operation::Constant(Value::Number(number))] = left.nodes.as_slice() else {
            return None;
        };
        let relation = match relation {
            Relation::Lt => Relation::Gt,
            Relation::Le => Relation::Ge,
            Relation::Gt => Relation::Lt,
            Relation::Ge => Relation::Le,
            Relation::Eq | Relation::Neq => return None,
        };
        (target, relation, *number)
    };
    let number = i64::from(number);
    match relation {
        Relation::Gt => Some((target, true, number + 1)),
        Relation::Ge => Some((target, true, number)),
        Relation::Lt => Some((target, false, number - 1)),
        Relation::Le => Some((target, false, number)),
        Relation::Eq | Relation::Neq => None,
    }
}
