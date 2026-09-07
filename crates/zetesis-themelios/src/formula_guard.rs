//! Complete ground truth tests, retained when a separate plan supplies bindings.
//!
//! A default-negated chain complements the conjunction of adjacent comparisons.
//! These tests contain no semantic atoms. Complete evaluation therefore supplies
//! the same constant in the original formula and every frozen reduct. Undefined
//! arithmetic remains a located failure, including under negation or a false
//! neighboring comparison; no intermediate truth result can conceal that failure.

use themelios_base::span::Location;
use themelios_program::program::{Comparison, DefaultNegation, Relation};
use themelios_program::term::Term;
use zetesis_core::Value;

use crate::expansion::Budget;
use crate::formula_ir::{Compiler, Expression, LiteralIr, Variables};
use crate::formula_support::{Counters, compare, expression};
use crate::{ExpansionResource, FormulaFailure, FormulaLimits};

pub(crate) enum Guard {
    Boolean(bool),
    Comparisons {
        negation: DefaultNegation,
        comparisons: Vec<GuardComparison>,
    },
}

pub(crate) enum GuardComparison {
    Scalar(Expression, Relation, Expression),
    Tuple(Vec<Expression>, Relation, Vec<Expression>),
    /// Complete scalar membership for one double-negated interval equality.
    Range(Expression, Expression, Expression),
}

impl Guard {
    pub(super) fn expressions(&self) -> impl Iterator<Item = &Expression> {
        let comparisons = match self {
            Self::Boolean(_) => &[][..],
            Self::Comparisons { comparisons, .. } => comparisons.as_slice(),
        };
        comparisons.iter().flat_map(|comparison| {
            let (left, right, last) = match comparison {
                GuardComparison::Scalar(left, _, right) => (
                    std::slice::from_ref(left),
                    std::slice::from_ref(right),
                    None,
                ),
                GuardComparison::Tuple(left, _, right) => (left.as_slice(), right.as_slice(), None),
                GuardComparison::Range(value, lower, upper) => (
                    std::slice::from_ref(value),
                    std::slice::from_ref(lower),
                    Some(upper),
                ),
            };
            left.iter().chain(right).chain(last)
        })
    }

    pub(super) fn evaluate(
        &self,
        assignment: &[Value],
        limits: FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        counters.work(limits, location)?;
        let (negation, comparisons) = match self {
            Self::Boolean(value) => return Ok(*value),
            Self::Comparisons {
                negation,
                comparisons,
            } => (negation, comparisons),
        };
        let mut conjunction = true;
        for comparison in comparisons {
            counters.work(limits, location)?;
            conjunction &= match comparison {
                GuardComparison::Scalar(left, relation, right) => {
                    let left = expression(left, assignment, limits, budget, counters, location)?;
                    let right = expression(right, assignment, limits, budget, counters, location)?;
                    compare(&left, *relation, &right)
                }
                GuardComparison::Tuple(left, relation, right) => {
                    let mut equal = left.len() == right.len();
                    // A length mismatch decides equality but does not make an
                    // undefined term in an unmatched tail admissible.
                    for index in 0..left.len().max(right.len()) {
                        let left = left
                            .get(index)
                            .map(|value| {
                                expression(value, assignment, limits, budget, counters, location)
                            })
                            .transpose()?;
                        let right = right
                            .get(index)
                            .map(|value| {
                                expression(value, assignment, limits, budget, counters, location)
                            })
                            .transpose()?;
                        equal &= left == right;
                    }
                    equal == (*relation == Relation::Eq)
                }
                GuardComparison::Range(value, lower, upper) => {
                    let value = expression(value, assignment, limits, budget, counters, location)?;
                    let lower = expression(lower, assignment, limits, budget, counters, location)?;
                    let upper = expression(upper, assignment, limits, budget, counters, location)?;
                    matches!((value, lower, upper), (Value::Number(value), Value::Number(lower), Value::Number(upper)) if lower <= value && value <= upper)
                }
            };
        }
        Ok(conjunction != (*negation == DefaultNegation::Not))
    }
}

impl Compiler<'_> {
    pub(super) fn comparison_guard(
        &mut self,
        comparison: &Comparison,
        negation: DefaultNegation,
        variables: &mut Variables,
    ) -> Result<LiteralIr, FormulaFailure> {
        if negation == DefaultNegation::NotNot
            && let Some(range) = self.interval_equality(comparison, variables)?
        {
            return Ok(LiteralIr::Guard(Guard::Comparisons {
                negation,
                comparisons: vec![range],
            }));
        }
        let mut comparisons = Vec::new();
        let mut left = comparison.first();
        for (relation, right) in comparison.steps() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            let test = match self.comparison(left, relation, right, variables)? {
                LiteralIr::Compare(left, relation, right) => {
                    GuardComparison::Scalar(left, relation, right)
                }
                LiteralIr::TupleCompare(left, relation, right) => {
                    GuardComparison::Tuple(left, relation, right)
                }
                _ => unreachable!("comparison produces a scalar or tuple test"),
            };
            comparisons.push(test);
            left = right;
        }
        Ok(LiteralIr::Guard(Guard::Comparisons {
            negation,
            comparisons,
        }))
    }

    fn interval_equality(
        &mut self,
        comparison: &Comparison,
        variables: &mut Variables,
    ) -> Result<Option<GuardComparison>, FormulaFailure> {
        let mut steps = comparison.steps();
        let Some((Relation::Eq, right)) = steps.next() else {
            return Ok(None);
        };
        if steps.next().is_some() {
            return Ok(None);
        }
        let ((value, Term::Interval { lower, upper }) | (Term::Interval { lower, upper }, value)) =
            (comparison.first(), right)
        else {
            return Ok(None);
        };
        self.budget
            .charge(ExpansionResource::TermWork, 1, self.location)?;
        Ok(Some(GuardComparison::Range(
            self.expression(value, variables)?,
            self.expression(lower, variables)?,
            self.expression(upper, variables)?,
        )))
    }
}
