//! Bounded integer-affine interpretation of normalized source expression plans.
//!
//! Source normalization evaluates closed arithmetic before plan construction.
//! Admitted pool selections preserve that property; consequent intervals become
//! variable slots. Remaining arithmetic operators therefore contain variables,
//! which this reader treats symbolically even when another instruction binds them.
//! Coefficients describe mathematical integers, while the retained whole guard
//! still uses checked source arithmetic. Unsupported operations yield no binding
//! evidence; finite-width exhaustion has its own located limit.

use crate::ProgramSite;
use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::ValueNodeRef;

use crate::expansion::Budget;
use crate::formula_ir::{Expression, Operation};
use crate::{ExpansionResource, FormulaFailure, FormulaResource};

pub(super) struct Affine {
    pub coefficients: Vec<i64>,
    pub constant: i64,
    /// Syntactic absence of variable inputs, never inferred from cancellation.
    closed: bool,
}

pub(super) struct Reader<'a, 'source> {
    pub source: &'a crate::formula_support::components::Admission<'source>,
    pub limits: &'a crate::FormulaLimits,
    pub counters: &'a mut crate::formula_support::Counters,
    pub variables: usize,
    pub budget: &'a mut Budget,
    pub location: ProgramSite,
}

impl Reader<'_, '_> {
    pub fn variable(&mut self, target: usize) -> Result<Affine, FormulaFailure> {
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            std::mem::size_of::<Affine>() as u128
                + self.variables as u128 * std::mem::size_of::<i64>() as u128,
            self.location,
        )?;
        self.work(1 + self.variables as u128)?;
        let mut form = self.number(0);
        form.coefficients[target] = 1;
        form.closed = false;
        Ok(form)
    }

    pub fn expression(
        &mut self,
        expression: &Expression,
    ) -> Result<Option<Affine>, FormulaFailure> {
        let cells = expression.nodes.len() as u128;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            cells
                * (std::mem::size_of::<Option<Affine>>() as u128
                    + self.variables as u128 * std::mem::size_of::<i64>() as u128),
            self.location,
        )?;
        let mut forms: Vec<Option<Affine>> = Vec::with_capacity(expression.nodes.len());
        for node in &expression.nodes {
            // Preserve the conservative dense-frame allowance for each node's
            // coefficient construction and arithmetic.
            self.work(1 + 6 * self.variables as u128)?;
            let form = match *node {
                Operation::Constant(scalar) => {
                    let value = self.source.scalar_ref(
                        scalar,
                        self.limits,
                        self.counters,
                        self.location,
                    )?;
                    match value.descriptor() {
                        ValueNodeRef::Number(number) => Some(self.number(number)),
                        _ => None,
                    }
                }
                Operation::Variable(variable) => {
                    let mut form = self.number(0);
                    form.coefficients[variable] = 1;
                    form.closed = false;
                    Some(form)
                }
                Operation::Unary(UnaryOp::Negate, index) => match &forms[index] {
                    Some(form) => Some(self.scale(form, -1)?),
                    None => None,
                },
                Operation::Binary(operator, left, right) => match (&forms[left], &forms[right]) {
                    (Some(left), Some(right)) => self.binary(operator, left, right)?,
                    _ => None,
                },
                _ => None,
            };
            forms.push(form);
        }
        Ok(forms.pop().flatten())
    }

    fn number(&self, number: i32) -> Affine {
        Affine {
            coefficients: vec![0; self.variables],
            constant: i64::from(number),
            closed: true,
        }
    }

    fn binary(
        &self,
        operator: BinaryOp,
        left: &Affine,
        right: &Affine,
    ) -> Result<Option<Affine>, FormulaFailure> {
        let result = match operator {
            BinaryOp::Add => self.combine(left, right, 1)?,
            BinaryOp::Sub => self.combine(left, right, -1)?,
            BinaryOp::Mul if left.closed => self.scale(right, left.constant)?,
            BinaryOp::Mul if right.closed => self.scale(left, right.constant)?,
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    pub fn combine(
        &self,
        left: &Affine,
        right: &Affine,
        factor: i64,
    ) -> Result<Affine, FormulaFailure> {
        let combine = |left, right| {
            coefficient(
                i128::from(left) + i128::from(factor) * i128::from(right),
                self.location,
            )
        };
        Ok(Affine {
            coefficients: left
                .coefficients
                .iter()
                .zip(&right.coefficients)
                .map(|(&left, &right)| combine(left, right))
                .collect::<Result<_, _>>()?,
            constant: combine(left.constant, right.constant)?,
            closed: left.closed && right.closed,
        })
    }

    pub fn scale(&self, form: &Affine, factor: i64) -> Result<Affine, FormulaFailure> {
        Ok(Affine {
            coefficients: form
                .coefficients
                .iter()
                .map(|&value| coefficient(i128::from(value) * i128::from(factor), self.location))
                .collect::<Result<_, _>>()?,
            constant: coefficient(
                i128::from(form.constant) * i128::from(factor),
                self.location,
            )?,
            closed: form.closed,
        })
    }

    pub fn work(&mut self, amount: u128) -> Result<(), FormulaFailure> {
        self.budget
            .charge(ExpansionResource::TermWork, amount, self.location)
            .map_err(Into::into)
    }
}

pub(super) fn coefficient(value: i128, location: ProgramSite) -> Result<i64, FormulaFailure> {
    i64::try_from(value).map_err(|_| {
        let magnitude = if value < 0 {
            value.unsigned_abs() - 1
        } else {
            value.unsigned_abs()
        };
        FormulaFailure::Limit {
            resource: FormulaResource::BindingCoefficientBits,
            limit: u128::from(i64::BITS),
            observed: u128::from(u128::BITS - magnitude.leading_zeros() + 1),
            location,
        }
    })
}
