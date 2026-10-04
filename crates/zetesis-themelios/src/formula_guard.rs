//! Complete ground truth tests, retained when a separate plan supplies bindings.
//!
//! A default-negated chain complements the conjunction of adjacent comparisons.
//! These tests contain no semantic atoms. Complete evaluation therefore supplies
//! the same constant in the original formula and every frozen reduct. Undefined
//! arithmetic remains a located failure, including under negation or a false
//! neighboring comparison; no intermediate truth result can conceal that failure.

use crate::formula_binding::Binding;

use crate::ProgramSite;
use themelios_program::program::{Comparison, DefaultNegation, Relation};
use themelios_program::term::Term;
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::{AssignmentError, CatalogRead, TermKey, TermRef};

use crate::formula_ir::{Compiler, Expression, LiteralIr, Variables};
use crate::formula_support::{Computation, Counters, Evaluation, Failures, compare};
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
        assignment: &Binding,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        self.evaluate_in(
            assignment,
            &mut Evaluation::default(),
            computation,
            limits,
            counters,
            location,
        )
    }

    pub(super) fn evaluate_in(
        &self,
        assignment: &Binding,
        evaluation: &mut Evaluation,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        let mut failures = Failures::default();
        for comparison in comparisons {
            counters.work(limits, location)?;
            let result = match comparison {
                GuardComparison::Scalar(left, relation, right) => evaluation
                    .source_values(
                        [left, right],
                        |variable| assignment.key(variable, location),
                        computation,
                        limits,
                        counters,
                        location,
                    )
                    .and_then(|[left, right]| {
                        let read = computation.read();
                        let left = resolve(read, &left, limits, counters, location)?;
                        let right = resolve(read, &right, limits, counters, location)?;
                        compare(left, *relation, right, limits, counters, location)
                    }),
                GuardComparison::Tuple(left, relation, right) => evaluation
                    .source_tuple(
                        (left, right),
                        |variable| assignment.key(variable, location),
                        computation,
                        limits,
                        counters,
                        location,
                    )
                    .map(|equal| equal == (*relation == Relation::Eq)),
                GuardComparison::Range(value, lower, upper) => evaluation
                    .source_values(
                        [value, lower, upper],
                        |variable| assignment.key(variable, location),
                        computation,
                        limits,
                        counters,
                        location,
                    )
                    .and_then(|values| {
                        let read = computation.read();
                        let mut numbers = [None; 3];
                        for (number, value) in numbers.iter_mut().zip(&values) {
                            let value = resolve(read, value, limits, counters, location)?;
                            counters.work(limits, location)?;
                            *number = match value.descriptor() {
                                ValueNodeRef::Number(value) => Some(value),
                                _ => None,
                            };
                        }
                        Ok(matches!(numbers, [Some(value), Some(lower), Some(upper)]
                        if lower <= value && value <= upper))
                    }),
            };
            if let Some(value) = failures.value(result, evaluation.zero_divisor())? {
                conjunction &= value;
            }
        }
        failures.finish(evaluation)?;
        Ok(conjunction != (*negation == DefaultNegation::Not))
    }
}

/// Resolution checks the computation's current prefix before borrowing payload.
fn resolve<'read>(
    read: CatalogRead<'read>,
    key: &TermKey,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<TermRef<'read>, FormulaFailure> {
    counters.work(limits, location)?;
    read.term(key)
        .map_err(|error| crate::formula_binding::assignment(AssignmentError::Read(error), location))
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

    /// Data-valued alternatives are generated before their truth is tested.
    /// Source variables remain inputs: these comparisons never bind a name.
    pub(super) fn ranged_guard(
        &mut self,
        comparison: &Comparison,
        negation: DefaultNegation,
        variables: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<Guard, FormulaFailure> {
        let mut steps = comparison.steps();
        let (relation, right) = steps.next().expect("comparison step");
        if negation == DefaultNegation::NotNot
            && relation == Relation::Eq
            && steps.next().is_none()
            && let (value, Term::Interval { lower, upper })
            | (Term::Interval { lower, upper }, value) = (comparison.first(), right)
        {
            let value = self.ranged_expression(value, variables, bindings)?;
            let lower = self.ranged_expression(lower, variables, bindings)?;
            let upper = self.ranged_expression(upper, variables, bindings)?;
            self.budget.charge(
                ExpansionResource::ScalarBytes,
                std::mem::size_of::<GuardComparison>() as u128,
                self.location,
            )?;
            return Ok(Guard::Comparisons {
                negation,
                comparisons: vec![GuardComparison::Range(value, lower, upper)],
            });
        }
        let mut left = self.generated_term(comparison.first(), variables, bindings)?;
        let mut comparisons = Vec::new();
        for (relation, right) in comparison.steps() {
            let right = self.generated_term(right, variables, bindings)?;
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            self.budget.charge(
                ExpansionResource::ScalarBytes,
                (std::mem::size_of::<GuardComparison>()
                    + 2 * std::mem::size_of::<crate::formula_ir::Operation>())
                    as u128,
                self.location,
            )?;
            comparisons.push(GuardComparison::Scalar(
                Self::scalar_expression(&left),
                relation,
                Self::scalar_expression(&right),
            ));
            left = right;
        }
        Ok(Guard::Comparisons {
            negation,
            comparisons,
        })
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
