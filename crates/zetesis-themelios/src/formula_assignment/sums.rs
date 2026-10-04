//! Merge sorted subset-sum runs under the caller's shared storage allowance.
use crate::ProgramSite;
use crate::formula::ceiling;
use crate::formula_support::Buffer;
use crate::formula_support::{Computation, Counters};
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
    ceiling(
        FormulaResource::AssignmentValues,
        1,
        limits.max_assignment_values as u128,
        location,
    )?;
    let mut sums = Buffer::new(computation, limits, counters, location)?;
    let mut next = Buffer::new(computation, limits, counters, location)?;
    sums.push(0_i32, computation, limits, counters, location)?;
    for weight in weights {
        // Adding a fixed integer preserves order. Check every addition before
        // merging so an overflowing possible subset remains a source refusal.
        for sum in sums.iter() {
            counters.work(limits, location)?;
            sum.checked_add(weight)
                .ok_or(ExpansionFailure::Evaluation {
                    error: EvalError::Overflow,
                    location,
                })?;
        }
        next.clear();
        let (mut original, mut shifted) = (0, 0);
        while original < sums.len() || shifted < sums.len() {
            counters.work(limits, location)?;
            let left = sums.slice().get(original).copied();
            let right = sums.slice().get(shifted).map(|sum| sum + weight);
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
                next.len() as u128 + 1,
                limits.max_assignment_values as u128,
                location,
            )?;
            next.push(value, computation, limits, counters, location)?;
        }
        std::mem::swap(&mut sums, &mut next);
    }
    Ok(sums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_support::testing::Fixture;
    use std::collections::BTreeSet;
    use themelios_base::{
        source::SourceId,
        span::{ByteOffset, Span},
    };
    fn location() -> ProgramSite {
        ProgramSite::source(themelios_base::span::Location {
            source: SourceId::new(89),
            span: Span::empty(ByteOffset::new(0)),
        })
    }

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
            Fixture::default().with(location(), |_, computation, counters| {
                let actual = sums(
                    weights,
                    computation,
                    &FormulaLimits::default(),
                    counters,
                    location(),
                )
                .unwrap();
                assert_eq!(actual.slice(), expected.into_iter().collect::<Vec<_>>());
            });
        }
    }
    #[test]
    fn candidate_limit_counts_distinct_sums() {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits {
                max_assignment_values: 3,
                ..Default::default()
            };
            assert_eq!(
                sums([1, 1], computation, &limits, counters, location())
                    .unwrap()
                    .slice(),
                &[0, 1, 2]
            );
            let limits = FormulaLimits {
                max_assignment_values: 2,
                ..limits
            };
            assert!(matches!(
                sums([1, 1], computation, &limits, counters, location()),
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
            Fixture::default().with(location(), |_, computation, counters| {
                assert!(matches!(
                    sums(
                        weights,
                        computation,
                        &FormulaLimits::default(),
                        counters,
                        location()
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
        let needed = Fixture::default().with(location(), |_, computation, counters| {
            let before = counters.accounting.work;
            sums(
                [2, -3, 5],
                computation,
                &FormulaLimits::default(),
                counters,
                location(),
            )
            .unwrap();
            counters.accounting.work - before
        });
        for cutoff in 0..needed {
            Fixture::default().with(location(), |_, computation, counters| {
                let limits = FormulaLimits::default();
                let observer = computation.lease();
                let before = computation
                    .allowance(&observer, &limits, location())
                    .unwrap();
                let bounded = FormulaLimits {
                    max_work: counters.accounting.work + cutoff,
                    ..limits
                };
                assert!(sums([2, -3, 5], computation, &bounded, counters, location()).is_err());
                assert_eq!(
                    computation
                        .allowance(&observer, &limits, location())
                        .unwrap(),
                    before
                );
            });
        }
    }
}
