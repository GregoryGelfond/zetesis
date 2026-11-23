use super::*;
use crate::formula_support::testing::Fixture;
use crate::grounding_observer::Work as Observer;
use crate::test_support::location;
use crate::{FormulaLimits, FormulaResource};
use std::num::NonZeroUsize;
use zetesis_algebra::threads::Pool;

const ROWS: usize = 4096;

fn live(computation: &Computation<'_, '_>, limits: &FormulaLimits) -> usize {
    let lease = computation.lease();
    limits.max_support_bytes - computation.allowance(&lease, limits, location()).unwrap()
}

struct Receipt {
    first_heads: Result<usize, FormulaFailure>,
    work: u64,
    peak: u64,
    retained: usize,
}

/// Fix the logical rows and insertion order; vary only the freeze pool. The
/// transport stays leased throughout, as the production worker envelope does.
fn freeze_sequence(workers: usize, maxima: &[Option<usize>]) -> Vec<Receipt> {
    let pool = Pool::new(NonZeroUsize::new(workers).unwrap(), |index| {
        format!("head-freeze-{index}")
    })
    .unwrap();
    Fixture::default().with(location(), |_, computation, counters| {
        let mut limits = FormulaLimits::default();
        let mut words = Buffer::new(computation, &limits, counters, location()).unwrap();
        words
            .reserve(ROWS * 2, computation, &limits, counters, location())
            .unwrap();
        for row in 0..ROWS {
            words
                .push(
                    u32::try_from(row / 2).unwrap(),
                    computation,
                    &limits,
                    counters,
                    location(),
                )
                .unwrap();
            words
                .push(7, computation, &limits, counters, location())
                .unwrap();
        }
        maxima
            .iter()
            .map(|&maximum| {
                limits.max_support_bytes = FormulaLimits::default().max_support_bytes;
                let before_live = live(computation, &limits);
                let before_work = counters.accounting.work;
                counters.observed = Observer::worker(true);
                if let Some(maximum) = maximum {
                    limits.max_support_bytes = maximum;
                }
                let result = freeze_keys(
                    || {
                        std::iter::once(KeyRows {
                            rows: ROWS,
                            width: 2,
                            words: Some(words.slice()),
                        })
                    },
                    Some(&pool),
                    Context::new(computation, &limits, counters, location()),
                );
                let work = counters.accounting.work - before_work;
                let peak = counters.observed.snapshot().support_peak_bytes.unwrap();
                let retained = live(computation, &FormulaLimits::default()) - before_live;
                let first_heads = result.map(|seen| {
                    let mut seen = seen.expect("nonempty keyed rows used the interner");
                    let mut first = 0;
                    for row in 0..ROWS {
                        let actual = seen.first(&limits, counters, location()).unwrap();
                        assert_eq!(actual, Some(row % 2 == 0));
                        first += usize::from(actual == Some(true));
                    }
                    first
                });
                // Success drops the generation and marks above; refusal drops every
                // temporary owner before returning. Neither path leaks a byte lease.
                assert_eq!(live(computation, &FormulaLimits::default()), before_live);
                Receipt {
                    first_heads,
                    work,
                    peak,
                    retained,
                }
            })
            .collect()
    })
}

fn freeze_report(workers: usize, maximum: Option<usize>) -> Receipt {
    freeze_sequence(workers, &[maximum]).pop().unwrap()
}

#[test]
fn head_freeze_accepts_its_exact_observed_peak() {
    let reference = freeze_report(1, None);
    let maximum = usize::try_from(reference.peak).unwrap();
    assert_eq!(reference.first_heads.unwrap(), ROWS / 2);
    assert!(reference.retained >= ROWS * size_of::<Option<Pending>>());
    for workers in [1, 2, 3, 4, 14] {
        let actual = freeze_report(workers, Some(maximum));
        assert_eq!(actual.first_heads.unwrap(), ROWS / 2, "workers={workers}");
        assert_eq!(actual.work, reference.work, "workers={workers}");
        assert_eq!(actual.peak, reference.peak, "workers={workers}");
        assert_eq!(actual.retained, reference.retained, "workers={workers}");
    }
}

#[test]
fn head_freeze_refuses_one_byte_below_its_peak() {
    let reference = freeze_report(1, None);
    let maximum = usize::try_from(reference.peak).unwrap() - 1;
    let mut refused = None;
    for workers in [1, 2, 3, 4, 14] {
        let actual = freeze_report(workers, Some(maximum));
        let FormulaFailure::Limit {
            resource,
            observed,
            limit,
            ..
        } = actual.first_heads.unwrap_err()
        else {
            panic!("expected the native support byte refusal");
        };
        assert_eq!(resource, FormulaResource::SupportBytes);
        assert_eq!(limit, maximum as u128);
        assert!(observed > limit);
        assert!(actual.peak <= maximum as u64);
        assert_eq!(actual.retained, 0);
        // Remap/table reservation follows content sorting: completed sorting
        // work must survive this storage refusal, beyond buffer initialization.
        assert!(actual.work > ROWS as u64 * 2);
        let receipt = (observed, actual.work, actual.peak);
        assert_eq!(
            *refused.get_or_insert(receipt),
            receipt,
            "workers={workers}"
        );
    }
}

#[test]
fn absent_keys_do_not_allocate_a_publication() {
    Fixture::default().with(location(), |_, computation, counters| {
        let mut limits = FormulaLimits::default();
        limits.max_support_bytes = live(computation, &limits);
        let before = counters.accounting.work;
        counters.observed = Observer::worker(true);
        for rows in [0, 17] {
            let seen = freeze_keys(
                || {
                    std::iter::once(KeyRows {
                        rows,
                        width: 0,
                        words: None,
                    })
                },
                None,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            assert!(seen.is_none());
        }
        assert_eq!(counters.accounting.work, before);
        assert_eq!(counters.observed.snapshot().support_peak_bytes, Some(0));
        assert_eq!(live(computation, &limits), limits.max_support_bytes);
    });
}

#[test]
fn nullary_keys_retain_one_head() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut seen = freeze_keys(
            || {
                std::iter::once(KeyRows {
                    rows: 9,
                    width: 0,
                    words: Some(&[]),
                })
            },
            None,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            seen.first(&limits, counters, location()).unwrap(),
            Some(true)
        );
        for _ in 1..9 {
            assert_eq!(
                seen.first(&limits, counters, location()).unwrap(),
                Some(false)
            );
        }
    });
}

#[test]
fn unkeyed_rows_preserve_the_seen_cursor() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let batches = [
            KeyRows {
                rows: 1,
                width: 1,
                words: Some(&[9]),
            },
            KeyRows {
                rows: 2,
                width: 0,
                words: None,
            },
            KeyRows {
                rows: 1,
                width: 1,
                words: Some(&[9]),
            },
        ];
        let mut seen = freeze_keys(
            || batches.into_iter(),
            None,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap()
        .unwrap();
        for expected in [Some(true), None, None, Some(false)] {
            assert_eq!(seen.first(&limits, counters, location()).unwrap(), expected);
        }
    });
}

#[test]
fn refused_head_freeze_retries_the_same_projected_words() {
    let reference = freeze_report(1, None);
    let maximum = usize::try_from(reference.peak).unwrap();
    for workers in [1, 2, 3, 4, 14] {
        let attempts = freeze_sequence(workers, &[Some(maximum - 1), Some(maximum)]);
        let mut attempts = attempts.into_iter();
        let refused = attempts.next().unwrap();
        assert!(matches!(refused.first_heads, Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, limit, ..
        }) if limit == (maximum - 1) as u128));
        assert_eq!(refused.retained, 0, "workers={workers}");
        assert!(refused.work > 0, "workers={workers}");
        let retried = attempts.next().unwrap();
        assert_eq!(retried.first_heads.unwrap(), ROWS / 2, "workers={workers}");
        assert_eq!(retried.work, reference.work, "workers={workers}");
        assert_eq!(retried.peak, reference.peak, "workers={workers}");
        assert_eq!(retried.retained, reference.retained, "workers={workers}");
    }
}

#[test]
fn mixed_nullary_and_unkeyed_batches_keep_the_first_occurrence_on_every_pool() {
    // Empty key content denotes one real head. An unkeyed row has no key
    // content at all and must not consume that identity's first occurrence.
    let mut reference = None;
    for workers in [1, 2, 3, 4, 14] {
        let pool = Pool::new(NonZeroUsize::new(workers).unwrap(), |index| {
            format!("head-mixed-{index}")
        })
        .unwrap();
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let before = counters.accounting.work;
            let before_live = live(computation, &limits);
            counters.observed = Observer::worker(true);
            let batches = [
                KeyRows {
                    rows: 0,
                    width: 0,
                    words: Some(&[]),
                },
                KeyRows {
                    rows: 1,
                    width: 0,
                    words: None,
                },
                KeyRows {
                    rows: 2,
                    width: 0,
                    words: Some(&[]),
                },
                KeyRows {
                    rows: 2,
                    width: 0,
                    words: None,
                },
                KeyRows {
                    rows: 1,
                    width: 0,
                    words: Some(&[]),
                },
            ];
            let mut seen = freeze_keys(
                || batches.into_iter(),
                Some(&pool),
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
            for expected in [None, Some(true), Some(false), None, None, Some(false)] {
                assert_eq!(seen.first(&limits, counters, location()).unwrap(), expected);
            }
            let receipt = (
                counters.accounting.work - before,
                counters.observed.snapshot().support_peak_bytes,
                live(computation, &limits) - before_live,
            );
            assert_eq!(
                *reference.get_or_insert(receipt),
                receipt,
                "workers={workers}"
            );
            drop(seen);
            assert_eq!(live(computation, &limits), before_live, "workers={workers}");
        });
    }
}

#[test]
fn key_projection_header_refusal_releases_its_owner() {
    Fixture::default().with(location(), |_, computation, counters| {
        let defaults = FormulaLimits::default();
        let before_live = live(computation, &defaults);
        let header = size_of::<Keyer>() - size_of::<Buffer<RowColumn>>() - size_of::<Buffer<u32>>();
        let limits = FormulaLimits {
            max_support_bytes: before_live,
            ..defaults
        };
        let result = Keyer::new(Context::new(computation, &limits, counters, location()));
        assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, observed, limit, ..
        }) if limit == before_live as u128 && observed == (before_live + header) as u128));
        assert_eq!(live(computation, &defaults), before_live);
        // Retry under the original allowance after the failed header's lease
        // has been dropped. A successful projection owner also drops cleanly.
        let keyer = Keyer::new(Context::new(computation, &defaults, counters, location())).unwrap();
        assert_eq!(
            live(computation, &defaults),
            before_live + size_of::<Keyer>()
        );
        drop(keyer);
        assert_eq!(live(computation, &defaults), before_live);
    });
}
