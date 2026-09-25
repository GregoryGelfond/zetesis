//! Directed finite integer envelopes for an existing binding scope.
//!
//! Each inequality has the mathematical form `sum(a_i * X_i) <= bound`.
//! An endpoint is proposed only from the required endpoints of every other
//! nonzero term. Each endpoint is installed once; at most twice the number of
//! variables can be installed. This is a finite coverage analysis, not integer
//! constraint solving. The original whole guard filters the proposed rows.

use crate::formula_support::GroundingWork;
mod affine;

use std::collections::BTreeSet;

use themelios_base::span::Location;
use themelios_program::program::{DefaultNegation, Relation};

use crate::expansion::Budget;
use crate::formula_guard::{Guard, GuardComparison};
use crate::formula_ir::{Expression, LiteralIr};
use crate::{ExpansionResource, FormulaFailure, FormulaResource};
use affine::{Affine, Reader, coefficient};

struct Inequality {
    coefficients: Vec<i64>,
    bound: i64,
}

#[derive(Clone, Copy, Default)]
struct Interval {
    lower: Option<i64>,
    upper: Option<i64>,
}

#[derive(Clone, Copy)]
enum Endpoint {
    Lower(i64),
    Upper(i64),
}

pub(super) fn binding<'a>(
    literals: impl Iterator<Item = &'a LiteralIr>,
    variables: usize,
    safe: &BTreeSet<usize>,
    source: &crate::formula_support::components::Admission<'_>,
    budget: &mut Budget,
    work: GroundingWork<'_>,
) -> Result<Option<(usize, i32, i32)>, FormulaFailure> {
    let GroundingWork {
        limits,
        counters,
        location,
    } = work;
    let mut capture = Capture {
        reader: Reader {
            source,
            limits,
            counters,
            variables,
            budget,
            location,
        },
        inequalities: Vec::new(),
        capacity: None,
    };
    for literal in literals {
        capture.literal(literal)?;
    }
    capture.reader.budget.charge(
        ExpansionResource::ScalarBytes,
        2 * variables as u128 * std::mem::size_of::<Interval>() as u128,
        location,
    )?;
    let mut intervals = vec![Interval::default(); variables];
    let mut proposals = vec![Interval::default(); variables];
    // Each successful round installs an absent endpoint. Numeric tightening
    // never controls termination, even on contradictory cyclic comparisons.
    for _ in 0..variables.saturating_mul(2) {
        capture.reader.work(variables as u128)?;
        proposals.fill(Interval::default());
        for inequality in &capture.inequalities {
            for (target, proposal) in proposals.iter_mut().enumerate() {
                capture.reader.work(1)?;
                match inequality.derive(target, &intervals, &mut capture.reader) {
                    Ok(Some(endpoint)) => {
                        // All proposals in a round read the same earlier
                        // endpoints. Select the strongest simultaneous bound,
                        // independently of source comparison order.
                        proposal.include(endpoint);
                    }
                    Ok(None) => {}
                    Err(error) if capacity_failure(&error) => capture.capacity = Some(error),
                    Err(error) => return Err(error),
                }
            }
        }
        let mut progress = false;
        for (interval, proposal) in intervals.iter_mut().zip(&proposals) {
            capture.reader.work(1)?;
            if let Some(lower) = proposal.lower {
                interval.lower = Some(lower);
                progress = true;
            }
            if let Some(upper) = proposal.upper {
                interval.upper = Some(upper);
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    for (target, interval) in intervals.iter().enumerate() {
        capture.reader.work(1)?;
        if !safe.contains(&target)
            && let Some((lower, upper)) = interval.range()
        {
            return Ok(Some((target, lower, upper)));
        }
    }
    if let Some(error) = capture.capacity {
        return Err(error);
    }
    Ok(None)
}

struct Capture<'a, 'source> {
    reader: Reader<'a, 'source>,
    inequalities: Vec<Inequality>,
    capacity: Option<FormulaFailure>,
}

impl Capture<'_, '_> {
    fn literal(&mut self, literal: &LiteralIr) -> Result<(), FormulaFailure> {
        self.reader.work(1)?;
        match literal {
            LiteralIr::Compare(left, relation, right) => self.comparison(left, *relation, right)?,
            LiteralIr::Guard(Guard::Comparisons {
                negation: DefaultNegation::None | DefaultNegation::NotNot,
                comparisons,
            }) => {
                for comparison in comparisons {
                    if let GuardComparison::Scalar(left, relation, right) = comparison {
                        self.comparison(left, *relation, right)?;
                    }
                }
            }
            LiteralIr::Bind { target, value } => self.generated(*target, value, value)?,
            LiteralIr::Range {
                target,
                lower,
                upper,
                binder: true,
            } => self.generated(*target, lower, upper)?,
            _ => {}
        }
        Ok(())
    }

    fn comparison(
        &mut self,
        left: &Expression,
        relation: Relation,
        right: &Expression,
    ) -> Result<(), FormulaFailure> {
        if relation == Relation::Neq {
            return Ok(());
        }
        let result = self.normalized(left, relation, right);
        match result {
            Err(error) if capacity_failure(&error) => self.capacity = Some(error),
            other => other?,
        }
        Ok(())
    }

    fn normalized(
        &mut self,
        left: &Expression,
        relation: Relation,
        right: &Expression,
    ) -> Result<(), FormulaFailure> {
        let left = self.reader.expression(left)?;
        let right = self.reader.expression(right)?;
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(());
        };
        match relation {
            Relation::Le | Relation::Lt => {
                self.inequality(&left, &right, relation == Relation::Lt)?;
            }
            Relation::Ge | Relation::Gt => {
                self.inequality(&right, &left, relation == Relation::Gt)?;
            }
            Relation::Eq => {
                self.inequality(&left, &right, false)?;
                self.inequality(&right, &left, false)?;
            }
            Relation::Neq => {}
        }
        Ok(())
    }

    fn inequality(
        &mut self,
        left: &Affine,
        right: &Affine,
        strict: bool,
    ) -> Result<(), FormulaFailure> {
        self.reader.work(1 + self.reader.variables as u128)?;
        self.reader.budget.charge(
            ExpansionResource::ScalarBytes,
            std::mem::size_of::<Inequality>() as u128
                + self.reader.variables as u128 * std::mem::size_of::<i64>() as u128,
            self.reader.location,
        )?;
        let difference = self.reader.combine(left, right, -1)?;
        let bound = coefficient(
            -i128::from(difference.constant) - i128::from(strict),
            self.reader.location,
        )?;
        self.inequalities.push(Inequality {
            coefficients: difference.coefficients,
            bound,
        });
        Ok(())
    }

    fn generated(
        &mut self,
        target: usize,
        lower: &Expression,
        upper: &Expression,
    ) -> Result<(), FormulaFailure> {
        // Existing generators retain their own dependency contract. Only closed
        // numeric results are reused here as static endpoint evidence; a bound
        // relational variable is not a closed source endpoint.
        if lower.inputs().next().is_some() || upper.inputs().next().is_some() {
            return Ok(());
        }
        let lower = self.reader.expression(lower)?;
        let upper = self.reader.expression(upper)?;
        let (Some(lower), Some(upper)) = (lower, upper) else {
            return Ok(());
        };
        let variable = self.reader.variable(target)?;
        self.inequality(&lower, &variable, false)?;
        self.inequality(&variable, &upper, false)
    }
}

impl Inequality {
    fn derive(
        &self,
        target: usize,
        intervals: &[Interval],
        reader: &mut Reader<'_, '_>,
    ) -> Result<Option<Endpoint>, FormulaFailure> {
        let coefficient = self.coefficients[target];
        if coefficient == 0 {
            return Ok(None);
        }
        let lower = coefficient < 0;
        if (if lower {
            intervals[target].lower
        } else {
            intervals[target].upper
        })
        .is_some()
        {
            return Ok(None);
        }
        let mut remainder = i128::from(self.bound);
        for (index, (&weight, interval)) in self.coefficients.iter().zip(intervals).enumerate() {
            reader.work(1)?;
            if index == target || weight == 0 {
                continue;
            }
            let Some(endpoint) = (if weight > 0 {
                interval.lower
            } else {
                interval.upper
            }) else {
                return Ok(None);
            };
            remainder = remainder
                .checked_sub(i128::from(weight) * i128::from(endpoint))
                .ok_or_else(|| bound_capacity(reader.location))?;
        }
        let quotient = inverse_bound(remainder, coefficient, reader.location)?;
        let endpoint = if lower {
            quotient.clamp(i128::from(i32::MIN), i128::from(i32::MAX) + 1)
        } else {
            quotient.clamp(i128::from(i32::MIN) - 1, i128::from(i32::MAX))
        };
        let endpoint =
            i64::try_from(endpoint).expect("clamped i32 endpoint or adjacent empty boundary");
        Ok(Some(if lower {
            Endpoint::Lower(endpoint)
        } else {
            Endpoint::Upper(endpoint)
        }))
    }
}

/// Solve `coefficient * x <= remainder` over mathematical integers. The caller
/// supplies a nonzero coefficient and chooses the indicated lower/upper side.
fn inverse_bound(
    remainder: i128,
    coefficient: i64,
    location: Location,
) -> Result<i128, FormulaFailure> {
    let divisor = i128::from(coefficient);
    if coefficient < 0 {
        // ceil(r / a), a < 0, equals -floor(r / -a).
        remainder
            .div_euclid(-divisor)
            .checked_neg()
            .ok_or_else(|| bound_capacity(location))
    } else {
        Ok(remainder.div_euclid(divisor))
    }
}

impl Interval {
    fn include(&mut self, endpoint: Endpoint) {
        match endpoint {
            Endpoint::Lower(value) => {
                self.lower = Some(self.lower.map_or(value, |previous| previous.max(value)));
            }
            Endpoint::Upper(value) => {
                self.upper = Some(self.upper.map_or(value, |previous| previous.min(value)));
            }
        }
    }

    fn range(self) -> Option<(i32, i32)> {
        let (lower, upper) = (self.lower?, self.upper?);
        Some(if lower > upper {
            (1, 0)
        } else {
            (
                i32::try_from(lower).expect("nonempty lower endpoint is i32"),
                i32::try_from(upper).expect("nonempty upper endpoint is i32"),
            )
        })
    }
}

fn capacity_failure(error: &FormulaFailure) -> bool {
    matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::BindingCoefficientBits | FormulaResource::BindingBoundBits,
            ..
        }
    )
}

fn bound_capacity(location: Location) -> FormulaFailure {
    FormulaFailure::Limit {
        resource: FormulaResource::BindingBoundBits,
        limit: u128::from(i128::BITS),
        observed: u128::from(i128::BITS) + 1,
        location,
    }
}

#[cfg(test)]
mod tests {
    use super::inverse_bound;
    use crate::{FormulaFailure, FormulaResource};
    use themelios_base::source::SourceId;
    use themelios_base::span::{ByteOffset, Location, Span};

    fn location() -> Location {
        Location {
            source: SourceId::new(1),
            span: Span::empty(ByteOffset::new(0)),
        }
    }

    #[test]
    fn inverse_bounds_match_integer_inequalities() {
        for coefficient in [-7_i64, -3, -1, 1, 3, 7] {
            for remainder in -20_i128..=20 {
                let bound = inverse_bound(remainder, coefficient, location()).unwrap();
                for value in -24_i128..=24 {
                    assert_eq!(
                        i128::from(coefficient) * value <= remainder,
                        if coefficient < 0 {
                            value >= bound
                        } else {
                            value <= bound
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn unrepresentable_inverse_bound_reports_capacity() {
        assert!(matches!(
            inverse_bound(i128::MIN, -1, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::BindingBoundBits,
                limit: 128,
                observed: 129,
                ..
            })
        ));
    }
}
