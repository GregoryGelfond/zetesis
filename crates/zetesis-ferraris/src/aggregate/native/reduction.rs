//! Direct reductions of occurrence masks, independent of formula acquisition.

use std::cmp::Ordering;

use zetesis_core::Value as Term;
use zetesis_cpu::Control;

use super::{Bound, Error, ErrorKind, Function, Group, Phase, Statistics, Work, add};
use crate::{AggregateComparison, AggregateExtremum};

/// Exact result borrowed from a retained group or an explicit empty endpoint.
/// Numeric aggregates use checked i128 arithmetic, including numerical extrema.
#[derive(Clone, Copy, Debug)]
pub enum Value<'a> {
    /// Wide mathematical integer within the checked execution representation.
    Integer(i128),
    /// Borrowed ordered ASP term. Group-produced numerical values use `Integer`;
    /// empty extrema use genuine ASP endpoints.
    Term(&'a Term),
}

impl Value<'_> {
    /// Compare without narrowing integers or confusing term storage order with
    /// ASP order. This standalone comparison does not charge a work budget;
    /// group guard evaluation charges both admitted carriers before calling it.
    /// Cost is linear in the compared flat value carriers, with constant extra
    /// storage; no recursive logical-tree traversal occurs.
    #[must_use]
    pub fn compare(self, other: Self) -> Ordering {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left.cmp(&right),
            (Self::Integer(left), Self::Term(right)) => match right {
                Term::Number(right) => left.cmp(&i128::from(*right)),
                Term::Infimum => Ordering::Greater,
                Term::Supremum | Term::String(_) | Term::Symbol(_) | Term::Structured(_) => {
                    Ordering::Less
                }
            },
            (Self::Term(left), Self::Integer(right)) => {
                Self::Integer(right).compare(Self::Term(left)).reverse()
            }
            (Self::Term(left), Self::Term(right)) => left.compare_terms(right),
        }
    }
}

/// Independent reduction/guard-evaluation work ceiling. Reduction allocates no
/// heap storage; transferred group storage and supplied masks remain borrowed.
#[derive(Clone, Copy, Debug)]
pub struct ReductionLimits {
    /// Mask visits, selected contributions, guards and comparison-carrier work.
    pub max_work: u64,
}
impl Default for ReductionLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
        }
    }
}

/// One computed measure with the conjunction of all evaluated guard occurrences.
#[derive(Clone, Copy, Debug)]
pub struct Evaluation<'a> {
    value: Value<'a>,
    holds: bool,
}
impl<'a> Evaluation<'a> {
    /// Complete native measure, including wide integers or empty extrema.
    #[must_use]
    pub const fn value(self) -> Value<'a> {
        self.value
    }

    /// True exactly when every guard holds of this computed measure.
    #[must_use]
    pub const fn holds(self) -> bool {
        self.holds
    }
}

/// Completed direct operation over the supplied occurrence masks.
/// No field asserts that the observations came from a particular M/J pair.
#[derive(Clone, Copy, Debug)]
pub struct Reduction<'a> {
    original: Evaluation<'a>,
    frozen: Option<Evaluation<'a>>,
    statistics: Statistics,
}
impl<'a> Reduction<'a> {
    /// Computed measure and guards over the original eligibility observation.
    #[must_use]
    pub const fn original(self) -> Evaluation<'a> {
        self.original
    }

    /// Computed measure and guards over the supplied frozen eligibility, if any.
    /// The aggregate reduct additionally requires the original guards to hold.
    #[must_use]
    pub const fn frozen(self) -> Option<Evaluation<'a>> {
        self.frozen
    }

    /// Aggregate-reduct truth: original guard truth AND frozen guard truth.
    /// Its semantic interpretation requires actual original/frozen eligibility.
    #[must_use]
    pub fn reduct_truth(self) -> Option<bool> {
        self.frozen
            .map(|frozen| self.original.holds && frozen.holds)
    }

    /// Completed direct reduction/guard accounting; no acquisition work included.
    #[must_use]
    pub const fn statistics(self) -> Statistics {
        self.statistics
    }
}

impl Group {
    /// Reduce ordered eligibility occurrences and evaluate every guard.
    ///
    /// Bare masks are observations supplied by the caller, not certified formula
    /// truth. Their lengths must equal the exact retained tuple count. For actual
    /// original/frozen truth, use [`Self::eligibility`] and its bound record.
    /// No aggregate is lowered and no formula node or source is evaluated here.
    ///
    /// Each mask cell is visited once. Selected tuples are reduced in retained
    /// order using checked integer addition or ASP term comparisons. Empty and
    /// nonnumeric tuples contribute nothing to sums; empty tuples contribute
    /// nothing to extrema; count includes every selected key. Sum-plus retains
    /// neutral keys but ignores zero/negative contributions. Every guard is
    /// evaluated even after an earlier false guard. Comparisons charge the full
    /// admitted term carriers, although lexicographic comparison may stop early.
    ///
    /// # Errors
    /// Refuses mask shape, arithmetic/work overflow, cancellation or deadline;
    /// no partially evaluated reduction is returned.
    pub fn reduce(
        &self,
        original: &[bool],
        frozen: Option<&[bool]>,
        limits: ReductionLimits,
        control: &Control,
    ) -> Result<Reduction<'_>, Error> {
        let mut work = Work {
            maximum: limits.max_work,
            control,
            statistics: Statistics::default(),
        };
        let result = self.reduce_checked(original, frozen, &mut work);
        result
            .map(|(original, frozen)| Reduction {
                original,
                frozen,
                statistics: work.statistics,
            })
            .map_err(|kind| work.failure(kind))
    }

    fn reduce_checked(
        &self,
        original: &[bool],
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<(Evaluation<'_>, Option<Evaluation<'_>>), ErrorKind> {
        work.poll()?;
        self.mask_length(original, Phase::Original)?;
        if let Some(frozen) = frozen {
            self.mask_length(frozen, Phase::Frozen)?;
        }
        let original = self.evaluate(original, work)?;
        let frozen = frozen.map(|mask| self.evaluate(mask, work)).transpose()?;
        work.poll()?;
        Ok((original, frozen))
    }

    fn mask_length(&self, mask: &[bool], phase: Phase) -> Result<(), ErrorKind> {
        if mask.len() != self.tuples.len() {
            return Err(ErrorKind::Mask {
                phase,
                expected: self.tuples.len(),
                actual: mask.len(),
            });
        }
        Ok(())
    }

    fn evaluate(&self, mask: &[bool], work: &mut Work<'_>) -> Result<Evaluation<'_>, ErrorKind> {
        let (value, cost) = match self.function {
            Function::Count => (Value::Integer(self.numeric(mask, work, |_| Some(1))?), 1),
            Function::Sum => (Value::Integer(self.numeric(mask, work, number)?), 1),
            Function::SumPlus => (
                Value::Integer(self.numeric(mask, work, positive_number)?),
                1,
            ),
            Function::Min => self.extremum(AggregateExtremum::Min, mask, work)?,
            Function::Max => self.extremum(AggregateExtremum::Max, mask, work)?,
        };
        let mut holds = true;
        for (guard, bound_cost) in self.guards.iter().zip(&self.guard_costs) {
            work.charge(1)?;
            work.charge(add(cost, *bound_cost)?)?;
            let bound = match &guard.bound {
                Bound::Integer(value) => Value::Integer(*value),
                Bound::Term(value) => term(value),
            };
            holds &= comparison(guard.comparison, value.compare(bound));
        }
        Ok(Evaluation { value, holds })
    }

    fn numeric(
        &self,
        mask: &[bool],
        work: &mut Work<'_>,
        contribution: impl Fn(Option<&Term>) -> Option<i128>,
    ) -> Result<i128, ErrorKind> {
        let mut total = 0_i128;
        for (tuple, selected) in self.tuples.iter().zip(mask) {
            work.charge(1)?;
            if !selected {
                continue;
            }
            work.charge(1)?;
            if let Some(value) = contribution(tuple.key.first()) {
                total = total.checked_add(value).ok_or(ErrorKind::Overflow)?;
            }
        }
        Ok(total)
    }

    fn extremum(
        &self,
        kind: AggregateExtremum,
        mask: &[bool],
        work: &mut Work<'_>,
    ) -> Result<(Value<'_>, u64), ErrorKind> {
        let mut value = match kind {
            AggregateExtremum::Min => Value::Term(&Term::Supremum),
            AggregateExtremum::Max => Value::Term(&Term::Infimum),
        };
        let mut cost = 1;
        for ((tuple, selected), tuple_cost) in self.tuples.iter().zip(mask).zip(&self.first_costs) {
            work.charge(1)?;
            if !selected {
                continue;
            }
            let Some(first) = tuple.key.first() else {
                continue;
            };
            work.charge(add(cost, *tuple_cost)?)?;
            let candidate = term(first);
            let order = candidate.compare(value);
            let replace = match kind {
                AggregateExtremum::Min => order.is_lt(),
                AggregateExtremum::Max => order.is_gt(),
            };
            if replace {
                value = candidate;
                cost = *tuple_cost;
            }
        }
        Ok((value, cost))
    }
}

fn term(value: &Term) -> Value<'_> {
    match value {
        Term::Number(value) => Value::Integer(i128::from(*value)),
        _ => Value::Term(value),
    }
}

fn number(value: Option<&Term>) -> Option<i128> {
    match value {
        Some(Term::Number(value)) => Some(i128::from(*value)),
        _ => None,
    }
}

fn positive_number(value: Option<&Term>) -> Option<i128> {
    number(value).filter(|value| *value > 0)
}

fn comparison(comparison: AggregateComparison, order: Ordering) -> bool {
    match comparison {
        AggregateComparison::Eq => order.is_eq(),
        AggregateComparison::Ne => !order.is_eq(),
        AggregateComparison::Lt => order.is_lt(),
        AggregateComparison::Le => !order.is_gt(),
        AggregateComparison::Gt => order.is_gt(),
        AggregateComparison::Ge => !order.is_lt(),
    }
}
