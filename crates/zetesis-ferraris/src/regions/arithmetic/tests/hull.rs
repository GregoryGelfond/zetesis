use super::super::hull::{Hull, LocalEquation, propose};

fn table(support_len: usize, allowed: &[usize]) -> [u64; 4] {
    let mut forbidden = [0; 4];
    for pattern in 0..1usize << support_len {
        if !allowed.contains(&pattern) {
            forbidden[pattern / 64] |= 1 << (pattern % 64);
        }
    }
    forbidden
}

fn words(support_len: usize) -> usize {
    2 * (1usize << support_len).div_ceil(128)
}

fn proposal(support_len: usize, allowed: &[usize]) -> Hull {
    propose(
        support_len,
        &table(support_len, allowed)[..words(support_len)],
        &mut |_| Ok::<_, ()>(()),
    )
    .unwrap()
}

fn holds(equation: LocalEquation, pattern: usize, support_len: usize) -> bool {
    let true_terms = (0..support_len)
        .filter(|&position| {
            equation.coefficients & (1 << position) != 0 && pattern & (1 << position) != 0
        })
        .count();
    true_terms % 2 == usize::from(equation.constant)
}

fn models(support_len: usize, equations: &[LocalEquation]) -> Vec<usize> {
    (0..1usize << support_len)
        .filter(|&pattern| {
            equations
                .iter()
                .all(|row| holds(*row, pattern, support_len))
        })
        .collect()
}

/// Enumerate every possible equation, retaining exactly those true on all
/// supplied patterns. This uses truth tables, not elimination or a basis.
fn reference_hull(support_len: usize, allowed: &[usize]) -> Vec<usize> {
    let mut result: Vec<_> = (0..1usize << support_len).collect();
    for coefficients in (0..=u8::MAX).take(1usize << support_len) {
        for constant in [false, true] {
            let row = LocalEquation {
                coefficients,
                constant,
            };
            if allowed
                .iter()
                .all(|&pattern| holds(row, pattern, support_len))
            {
                result.retain(|&pattern| holds(row, pattern, support_len));
            }
        }
    }
    result
}

#[test]
fn every_small_pattern_set_has_its_affine_hull() {
    for support_len in 0..=3 {
        let patterns = 1usize << support_len;
        for selected in 0..1usize << patterns {
            let allowed: Vec<_> = (0..patterns)
                .filter(|&pattern| selected & (1 << pattern) != 0)
                .collect();
            let hull = proposal(support_len, &allowed);
            assert_eq!(
                models(support_len, hull.equations()),
                reference_hull(support_len, &allowed),
                "support={support_len}, selected={selected}"
            );
            assert!(
                hull.equations()
                    .iter()
                    .all(|row| { usize::from(row.coefficients) < patterns })
            );
        }
    }
}

#[test]
fn exactness_means_the_original_pattern_set() {
    for support_len in 0..=3 {
        let patterns = 1usize << support_len;
        for selected in 0..1usize << patterns {
            let allowed: Vec<_> = (0..patterns)
                .filter(|&pattern| selected & (1 << pattern) != 0)
                .collect();
            let hull = proposal(support_len, &allowed);
            assert_eq!(hull.exact, models(support_len, hull.equations()) == allowed);
        }
    }
}

#[test]
fn an_inexact_set_is_not_reported_as_exact() {
    let hull = proposal(2, &[0, 1, 2]);
    assert!(!hull.exact);
    assert_eq!(models(2, hull.equations()), [0, 1, 2, 3]);
}

#[test]
fn full_allowed_tables_need_no_equation() {
    for support_len in 0..=8 {
        let allowed: Vec<_> = (0..1usize << support_len).collect();
        let hull = proposal(support_len, &allowed);
        assert!(hull.equations().is_empty());
        assert!(hull.exact);
    }
}

#[test]
fn empty_allowed_tables_emit_a_contradiction() {
    for support_len in 0..=8 {
        let hull = proposal(support_len, &[]);
        assert_eq!(
            hull.equations(),
            [LocalEquation {
                coefficients: 0,
                constant: true
            }]
        );
        assert!(hull.exact);
    }
}

#[test]
fn affine_cosets_cross_all_forbidden_table_words() {
    let allowed = [1, 64, 128, 193];
    let hull = proposal(8, &allowed);
    assert!(hull.exact);
    assert_eq!(models(8, hull.equations()), allowed);
    assert_eq!(models(8, hull.equations()), reference_hull(8, &allowed));
}

#[derive(Debug, PartialEq, Eq)]
struct Refused {
    chunk: usize,
    work: u64,
}

#[test]
fn refusal_preserves_each_executed_charge_prefix() {
    for (support_len, allowed) in [
        (0, vec![]),
        (0, vec![0]),
        (3, vec![]),
        (3, vec![0]),
        (3, vec![0, 1, 2]),
        (3, (0..8).collect()),
        (8, vec![1, 64, 128, 193]),
    ] {
        let forbidden = table(support_len, &allowed);
        let forbidden = &forbidden[..words(support_len)];
        let mut trace = Vec::new();
        propose(support_len, forbidden, &mut |work| {
            assert!(work > 0);
            trace.push(work);
            Ok::<_, ()>(())
        })
        .unwrap();
        for stop in 0..trace.len() {
            let mut attempts = 0;
            let mut admitted = 0;
            let result = propose(support_len, forbidden, &mut |work| {
                let chunk = attempts;
                attempts += 1;
                assert_eq!(work, trace[chunk]);
                if chunk == stop {
                    Err(Refused { chunk, work })
                } else {
                    admitted += work;
                    Ok(())
                }
            });
            assert_eq!(
                result.unwrap_err(),
                Refused {
                    chunk: stop,
                    work: trace[stop]
                }
            );
            assert_eq!(attempts, stop + 1);
            assert_eq!(admitted, trace[..stop].iter().sum());
        }
    }
}

#[test]
fn small_tables_charge_the_declared_operations() {
    // These totals include fixed scratch/result initialization. The two-point
    // set on two variables also requires an occupied-pivot substitution.
    for (support_len, allowed, expected) in [
        (0, vec![], 39),
        (0, vec![0], 43),
        (1, vec![], 44),
        (1, vec![0], 61),
        (1, vec![0, 1], 62),
        (2, vec![0], 86),
        (2, vec![1, 2], 92),
        (2, vec![0, 1, 2, 3], 90),
    ] {
        let forbidden = table(support_len, &allowed);
        let mut work = 0;
        propose(
            support_len,
            &forbidden[..words(support_len)],
            &mut |chunk| {
                work += chunk;
                Ok::<_, ()>(())
            },
        )
        .unwrap();
        assert_eq!(work, expected, "support={support_len}, allowed={allowed:?}");
    }
}

#[test]
fn a_complete_span_does_not_reeliminate_later_patterns() {
    let forbidden = table(3, &(0..8).collect::<Vec<_>>());
    let mut work = 0;
    let hull = propose(3, &forbidden[..words(3)], &mut |chunk| {
        work += chunk;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert!(hull.equations().is_empty());
    assert!(hull.exact);
    // Rank reaches three at pattern four. Patterns five through seven still
    // update the count, but no longer reduce against the completed basis.
    // The former full elimination costs 192 operations on this same table.
    assert_eq!(work, 148);
}

#[test]
fn a_completed_span_still_admits_the_exactness_count() {
    let forbidden = table(3, &(0..8).collect::<Vec<_>>());
    let mut work = 0;
    let result = propose(3, &forbidden[..words(3)], &mut |chunk| {
        if work + chunk > 105 {
            return Err(chunk);
        }
        work += chunk;
        Ok(())
    });
    // The last independent pivot has been installed; the next pattern test
    // is still required. A refusal there must not publish an exact Hull.
    assert_eq!(result.unwrap_err(), 5);
    assert_eq!(work, 105);
}

#[test]
fn actual_work_stays_within_the_input_bound() {
    for support_len in 0..=8 {
        let patterns = 1usize << support_len;
        for allowed in [
            vec![],
            vec![0],
            (0..patterns).collect(),
            (0..patterns).step_by(3).collect(),
        ] {
            let forbidden = table(support_len, &allowed);
            let mut work = 0;
            propose(
                support_len,
                &forbidden[..words(support_len)],
                &mut |chunk| {
                    work += chunk;
                    Ok::<_, ()>(())
                },
            )
            .unwrap();
            let bound = 3 * support_len
                + 6
                + patterns * (5 * support_len + 8)
                + (8 - support_len)
                + 18
                + 4.max(3 + support_len + support_len * (7 * support_len + 8));
            assert!(work <= u64::try_from(bound).unwrap());
        }
    }
}

#[test]
#[should_panic(expected = "local support has at most eight variables")]
fn oversized_support_is_rejected_before_work() {
    let _ = propose(9, &[0; 4], &mut |_| -> Result<(), ()> {
        panic!("unexpected work")
    });
}

#[test]
#[should_panic(expected = "padded table length")]
fn unpadded_tables_are_rejected_before_work() {
    let _ = propose(0, &[0], &mut |_| -> Result<(), ()> {
        panic!("unexpected work")
    });
}
