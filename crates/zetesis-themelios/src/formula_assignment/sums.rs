//! Distinct finite assignment sums with leased sorted or packed execution.
//!
//! Every input occurrence remains independently optional. The returned `Numbers`
//! are always ascending, distinct i32 values; packed words are private scratch.
mod packed;

use crate::ProgramSite;
use crate::formula::ceiling;
use crate::formula_support::{Buffer, Computation, Context, Counters};
use crate::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};
use themelios_program::term::EvalError;

pub(crate) type Numbers = Buffer<i32>;

pub(crate) fn sums(
    weights: impl IntoIterator<Item = i32>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Numbers, FormulaFailure> {
    // Preserve the empty proposal's refusal before input, work or storage.
    ceiling(
        FormulaResource::AssignmentValues,
        1,
        limits.max_assignment_values as u128,
        location,
    )?;
    let mut context = Context::new(computation, limits, counters, location);
    let mut runs = Runs::new(&mut context)?;
    let mut weights = weights.into_iter();
    for weight in weights.by_ref() {
        tick(&mut context)?;
        if let Some(plan) = packed::Plan::for_runs(&runs.values, weight, &mut context)? {
            // This is the only entry into packed execution. A later signed or
            // sparse contribution returns to merging without trying it again.
            drop(runs.next);
            return packed::finish(runs.values, weight, weights, plan, &mut context);
        }
        runs.add(weight, &mut context)?;
    }
    Ok(runs.values)
}

fn tick(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}

fn overflow(location: ProgramSite) -> FormulaFailure {
    ExpansionFailure::Evaluation {
        error: EvalError::Overflow,
        location,
    }
    .into()
}

struct Runs {
    values: Numbers,
    next: Numbers,
}

impl Runs {
    fn new(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<Self, FormulaFailure> {
        let mut values = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let next = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        values.push(
            0,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        Ok(Self { values, next })
    }

    fn from_values(
        values: Numbers,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let next = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        Ok(Self { values, next })
    }

    fn finish(
        mut self,
        weights: impl IntoIterator<Item = i32>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Numbers, FormulaFailure> {
        for weight in weights {
            tick(context)?;
            self.add(weight, context)?;
        }
        Ok(self.values)
    }

    fn add(
        &mut self,
        weight: i32,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        // Adding a fixed integer preserves order. The caller paid the first
        // value's work/cancellation charge before choosing the physical route.
        // Check every addition before merging; an overflowing optional subset
        // remains a source refusal. Zero ensures this run is never empty.
        for (index, sum) in self.values.iter().enumerate() {
            if index != 0 {
                tick(context)?;
            }
            sum.checked_add(weight)
                .ok_or_else(|| overflow(context.work.location))?;
        }
        self.next.clear();
        let (mut original, mut shifted) = (0, 0);
        while original < self.values.len() || shifted < self.values.len() {
            tick(context)?;
            let left = self.values.slice().get(original).copied();
            let right = self.values.slice().get(shifted).map(|sum| sum + weight);
            let value = match (left, right) {
                (Some(left), Some(right)) => left.min(right),
                (Some(value), None) | (None, Some(value)) => value,
                (None, None) => unreachable!("one sorted run remains"),
            };
            if left == Some(value) {
                original += 1;
            }
            if right == Some(value) {
                shifted += 1;
            }
            ceiling(
                FormulaResource::AssignmentValues,
                self.next.len() as u128 + 1,
                context.work.limits.max_assignment_values as u128,
                context.work.location,
            )?;
            self.next.push(
                value,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
        }
        std::mem::swap(&mut self.values, &mut self.next);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_support::testing::Fixture;
    use crate::test_support::given_site;
    use std::collections::BTreeSet;
    #[test]
    fn merged_runs_equal_all_distinct_subset_sums() {
        for weights in [vec![2, -3, 2, 0], vec![-5, 9, 4], Vec::new()] {
            let expected: BTreeSet<_> = (0..1_usize << weights.len())
                .map(|mask| {
                    weights
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| mask & (1 << index) != 0)
                        .map(|(_, value)| value)
                        .sum::<i32>()
                })
                .collect();
            Fixture::default().with(given_site(), |_, computation, counters| {
                let actual = sums(
                    weights,
                    computation,
                    &FormulaLimits::default(),
                    counters,
                    given_site(),
                )
                .unwrap();
                assert_eq!(actual.slice(), expected.into_iter().collect::<Vec<_>>());
            });
        }
    }
    #[test]
    fn candidate_limit_counts_distinct_sums() {
        Fixture::default().with(given_site(), |_, computation, counters| {
            let limits = FormulaLimits {
                max_assignment_values: 3,
                ..Default::default()
            };
            assert_eq!(
                sums([1, 1], computation, &limits, counters, given_site())
                    .unwrap()
                    .slice(),
                &[0, 1, 2]
            );
            let limits = FormulaLimits {
                max_assignment_values: 2,
                ..limits
            };
            assert!(matches!(
                sums([1, 1], computation, &limits, counters, given_site()),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::AssignmentValues,
                    observed: 3,
                    limit: 2,
                    ..
                })
            ));
        });
    }
    #[test]
    fn any_overflowing_subset_refuses_the_carrier() {
        for weights in [[i32::MAX, 1], [i32::MIN, -1]] {
            Fixture::default().with(given_site(), |_, computation, counters| {
                assert!(matches!(
                    sums(
                        weights,
                        computation,
                        &FormulaLimits::default(),
                        counters,
                        given_site()
                    ),
                    Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                        error: EvalError::Overflow,
                        ..
                    }))
                ));
            });
        }
    }
    #[test]
    fn every_work_stop_releases_both_sum_runs() {
        let needed = Fixture::default().with(given_site(), |_, computation, counters| {
            let before = counters.accounting.work;
            sums(
                [2, -3, 5],
                computation,
                &FormulaLimits::default(),
                counters,
                given_site(),
            )
            .unwrap();
            counters.accounting.work - before
        });
        for cutoff in 0..needed {
            Fixture::default().with(given_site(), |_, computation, counters| {
                let limits = FormulaLimits::default();
                let observer = computation.lease();
                let before = computation
                    .allowance(&observer, &limits, given_site())
                    .unwrap();
                let bounded = FormulaLimits {
                    max_work: counters.accounting.work + cutoff,
                    ..limits
                };
                assert!(sums([2, -3, 5], computation, &bounded, counters, given_site()).is_err());
                assert_eq!(
                    computation
                        .allowance(&observer, &limits, given_site())
                        .unwrap(),
                    before
                );
            });
        }
    }
}
