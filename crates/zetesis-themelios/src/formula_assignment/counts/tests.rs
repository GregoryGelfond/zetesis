use super::counts;
use crate::formula_assignment::sums;
use crate::formula_support::testing::Fixture;
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};
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
fn count_proposals_equal_unit_subset_sums() {
    for size in 0..=16 {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let actual = counts(
                std::iter::repeat_n(1, size),
                computation,
                &limits,
                counters,
                location(),
            )
            .unwrap();
            let reference = sums(
                std::iter::repeat_n(1, size),
                computation,
                &limits,
                counters,
                location(),
            )
            .unwrap();
            assert_eq!(actual.slice(), reference.slice());
        });
    }
}

#[test]
fn count_construction_avoids_subset_merge_work() {
    let (direct, merged) = Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let before = counters.accounting.work;
        drop(counts([1; 64], computation, &limits, counters, location()).unwrap());
        let direct = counters.accounting.work - before;
        let before = counters.accounting.work;
        drop(sums([1; 64], computation, &limits, counters, location()).unwrap());
        (direct, counters.accounting.work - before)
    });
    assert!(
        direct < merged,
        "{direct} direct steps versus {merged} merged steps"
    );
}

#[test]
fn zero_value_limit_precedes_other_refusals() {
    Fixture::default().with(location(), |_, computation, counters| {
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        *counters = std::mem::take(counters).with_cancellation(Some(&cancellation));
        let before = (counters.accounting.work, counters.workspace_bytes());
        let limits = FormulaLimits {
            max_assignment_values: 0,
            max_support_bytes: 0,
            max_work: 0,
            ..FormulaLimits::default()
        };
        let unseen = std::iter::from_fn(|| -> Option<i32> {
            panic!("the empty candidate's limit precedes input consumption")
        });
        assert!(matches!(
            counts(unseen, computation, &limits, counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AssignmentValues,
                observed: 1,
                limit: 0,
                ..
            })
        ));
        assert_eq!(
            (counters.accounting.work, counters.workspace_bytes()),
            before
        );
    });
}

#[test]
fn value_limit_includes_the_zero_proposal() {
    for limit in 1_usize..=5 {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits {
                max_assignment_values: limit,
                ..FormulaLimits::default()
            };
            let fitting = counts(
                std::iter::repeat_n(1, limit - 1),
                computation,
                &limits,
                counters,
                location(),
            )
            .unwrap();
            assert_eq!(fitting.len(), limit);
            drop(fitting);
            assert!(matches!(
                counts(
                    std::iter::repeat_n(1, limit),
                    computation,
                    &limits,
                    counters,
                    location()
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::AssignmentValues,
                    observed,
                    limit: actual_limit,
                    ..
                }) if observed == limit as u128 + 1 && actual_limit == limit as u128
            ));
        });
    }
}

#[test]
fn every_work_refusal_releases_count_storage() {
    let needed = Fixture::default().with(location(), |_, computation, counters| {
        let before = counters.accounting.work;
        drop(
            counts(
                [1; 8],
                computation,
                &FormulaLimits::default(),
                counters,
                location(),
            )
            .unwrap(),
        );
        counters.accounting.work - before
    });
    for cutoff in 0..needed {
        Fixture::default().with(location(), |_, computation, counters| {
            let before = counters.workspace_bytes();
            let limits = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..FormulaLimits::default()
            };
            assert!(matches!(
                counts([1; 8], computation, &limits, counters, location()),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert!(counters.accounting.work <= limits.max_work);
            assert_eq!(counters.workspace_bytes(), before);
        });
    }
}

#[test]
fn storage_refusal_releases_the_count_header() {
    Fixture::default().with(location(), |_, computation, counters| {
        let before = counters.workspace_bytes();
        let limits = FormulaLimits {
            max_support_bytes: 0,
            ..FormulaLimits::default()
        };
        assert!(matches!(
            counts([1; 4], computation, &limits, counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert_eq!(counters.workspace_bytes(), before);
    });
}

#[test]
fn cancellation_between_contributions_releases_storage() {
    Fixture::default().with(location(), |_, computation, counters| {
        let cancellation = zetesis_cpu::Cancellation::default();
        *counters = std::mem::take(counters).with_cancellation(Some(&cancellation));
        let before = counters.workspace_bytes();
        let mut offered = 0;
        let weights = [1; 4].into_iter().inspect(|_| {
            offered += 1;
            if offered == 2 {
                cancellation.cancel();
            }
        });
        assert!(matches!(
            counts(
                weights,
                computation,
                &FormulaLimits::default(),
                counters,
                location()
            ),
            Err(FormulaFailure::Interrupted {
                reason: zetesis_cpu::Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(offered, 2);
        assert_eq!(counters.workspace_bytes(), before);
    });
}

#[test]
#[should_panic(expected = "count contributions are unit weights")]
fn nonunit_count_contribution_is_an_invariant_failure() {
    Fixture::default().with(location(), |_, computation, counters| {
        let _ = counts(
            [1, 2],
            computation,
            &FormulaLimits::default(),
            counters,
            location(),
        );
    });
}
