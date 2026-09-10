//! Bounded integer-affine interpretation of the existing expression plan.
//!
//! Coefficients describe mathematical integer expressions. They do not replace
//! the original checked scalar evaluator. Unsupported operations yield no
//! binding evidence; finite-width exhaustion has its own located limit.

use themelios_base::span::Location;
use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::Value;

use crate::expansion::Budget;
use crate::formula_ir::{Expression, Operation};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure, FormulaResource};

pub(super) struct Affine {
    pub coefficients: Vec<i64>,
    pub constant: i64,
    ground: bool,
}

pub(super) struct Reader<'a> {
    pub variables: usize,
    pub budget: &'a mut Budget,
    pub location: Location,
}

impl Reader<'_> {
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
        form.ground = false;
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
            // Constant detection and one coefficient-producing operation each
            // traverse at most this many entries of the dense scoped frame.
            self.work(1 + 6 * self.variables as u128)?;
            let form = match *node {
                Operation::Constant(Value::Number(number)) => Some(self.number(number)),
                Operation::Variable(variable) => {
                    let mut form = self.number(0);
                    form.coefficients[variable] = 1;
                    form.ground = false;
                    Some(form)
                }
                Operation::Unary(operator, index) => match &forms[index] {
                    Some(form) if form.ground => {
                        let value = crate::scalar_arithmetic::unary(
                            operator,
                            i32::try_from(form.constant).expect("ground arithmetic retains i32"),
                        )
                        .map_err(|error| ExpansionFailure::Evaluation {
                            error,
                            location: self.location,
                        })?;
                        Some(self.number(value))
                    }
                    Some(form) if operator == UnaryOp::Negate => Some(self.scale(form, -1)?),
                    _ => None,
                },
                Operation::Absolute(index) => match &forms[index] {
                    Some(form) if form.ground => {
                        let value = crate::scalar_arithmetic::absolute(
                            i32::try_from(form.constant).expect("ground arithmetic retains i32"),
                        )
                        .map_err(|error| ExpansionFailure::Evaluation {
                            error,
                            location: self.location,
                        })?;
                        Some(self.number(value))
                    }
                    _ => None,
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
            ground: true,
        }
    }

    fn binary(
        &self,
        operator: BinaryOp,
        left: &Affine,
        right: &Affine,
    ) -> Result<Option<Affine>, FormulaFailure> {
        if left.ground && right.ground {
            let result = crate::scalar_arithmetic::binary(
                operator,
                i32::try_from(left.constant).expect("ground arithmetic retains i32"),
                i32::try_from(right.constant).expect("ground arithmetic retains i32"),
            )
            .map_err(|error| ExpansionFailure::Evaluation {
                error,
                location: self.location,
            })?;
            return Ok(Some(self.number(result)));
        }
        let result = match operator {
            BinaryOp::Add => self.combine(left, right, 1)?,
            BinaryOp::Sub => self.combine(left, right, -1)?,
            BinaryOp::Mul if left.ground => self.scale(right, left.constant)?,
            BinaryOp::Mul if right.ground => self.scale(left, right.constant)?,
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
            ground: left.ground && right.ground,
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
            ground: form.ground,
        })
    }

    pub fn work(&mut self, amount: u128) -> Result<(), FormulaFailure> {
        self.budget
            .charge(ExpansionResource::TermWork, amount, self.location)
            .map_err(Into::into)
    }
}

pub(super) fn coefficient(value: i128, location: Location) -> Result<i64, FormulaFailure> {
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
