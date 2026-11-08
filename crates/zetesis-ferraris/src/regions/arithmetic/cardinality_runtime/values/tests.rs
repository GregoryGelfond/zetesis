use super::*;
use crate::regions::arithmetic::cardinality_source::{Kind, Source};

fn bound(support: u64, side: BoundSide, value: u64, guard: Vec<Literal>) -> Bound {
    Bound {
        support: Words::new(vec![support, 0]),
        cardinality: u64::from(support.count_ones()),
        guard,
        side,
        value,
        source: Source {
            kind: Kind::Asserted,
            root: 0,
            node: 0,
        },
    }
}

fn reference(bound: &Bound, held: u64, cut: u64) -> u64 {
    let selected = match bound.side {
        BoundSide::Lower => held,
        BoundSide::Upper => !cut,
    };
    let count: u64 = (0..3)
        .filter(|atom| bound.support[0] & selected & (1 << atom) != 0)
        .count()
        .try_into()
        .unwrap();
    let active = bound.guard.iter().all(|literal| {
        let bit = 1 << literal.atom;
        if literal.positive {
            held & bit != 0
        } else {
            cut & bit != 0
        }
    });
    match (bound.side, active) {
        (BoundSide::Lower, true) => count.max(bound.value),
        (BoundSide::Upper, true) => count.min(bound.value),
        (_, false) => count,
    }
}

fn guard_fixtures() -> Vec<Vec<Literal>> {
    let mut guards = vec![vec![]];
    for atom in 0..3 {
        for positive in [false, true] {
            let literal = Literal { atom, positive };
            guards.push(vec![literal]);
            guards.push(vec![literal, literal]);
            guards.push(vec![
                literal,
                Literal {
                    atom,
                    positive: !positive,
                },
            ]);
        }
    }
    guards
}

#[test]
fn effective_bounds_match_partial_interpretations() {
    for support in 0u64..8 {
        for side in [BoundSide::Lower, BoundSide::Upper] {
            for value in 0..=u64::from(support.count_ones()) {
                for guard in guard_fixtures() {
                    let bound = bound(support, side, value, guard);
                    for held in 0u64..8 {
                        for cut in 0u64..8 {
                            if held & cut != 0 {
                                continue;
                            }
                            let mut work = Meter::local(u64::MAX);
                            assert_eq!(
                                effective(&bound, (&[held], &[cut]), &mut work).unwrap(),
                                reference(&bound, held, cut)
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn effective_bounds_tighten_along_descendants() {
    for support in 0u64..8 {
        for side in [BoundSide::Lower, BoundSide::Upper] {
            for value in 0..=u64::from(support.count_ones()) {
                for guard in guard_fixtures() {
                    let bound = bound(support, side, value, guard);
                    for held in 0u64..8 {
                        for cut in 0u64..8 {
                            if held & cut != 0 {
                                continue;
                            }
                            for next_held in 0u64..8 {
                                for next_cut in 0u64..8 {
                                    if next_held & next_cut != 0
                                        || held & !next_held != 0
                                        || cut & !next_cut != 0
                                    {
                                        continue;
                                    }
                                    let mut work = Meter::local(u64::MAX);
                                    let old =
                                        effective(&bound, (&[held], &[cut]), &mut work).unwrap();
                                    let next =
                                        effective(&bound, (&[next_held], &[next_cut]), &mut work)
                                            .unwrap();
                                    match side {
                                        BoundSide::Lower => assert!(old <= next),
                                        BoundSide::Upper => assert!(old >= next),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn ordered_guards_charge_only_the_reached_prefix() {
    let positive = Literal {
        atom: 0,
        positive: true,
    };
    let negative = Literal {
        atom: 0,
        positive: false,
    };
    for (guard, held, cut, expected, charged) in [
        (vec![], 0, 0, true, 1),
        (vec![positive, positive], 0, 0, false, 7),
        (vec![positive, positive], 1, 0, true, 13),
        (vec![positive, negative], 1, 0, false, 13),
        (vec![negative, positive], 1, 0, false, 7),
        (vec![negative, negative], 0, 1, true, 13),
    ] {
        let mut work = Meter::local(u64::MAX);
        assert_eq!(
            super::guard(&guard, (&[held], &[cut]), &mut work).unwrap(),
            expected
        );
        assert_eq!(work.charged(), charged);
    }
}

#[test]
fn effective_refusals_preserve_their_exact_admitted_prefix() {
    let bound = bound(
        7,
        BoundSide::Upper,
        1,
        vec![Literal {
            atom: 2,
            positive: true,
        }],
    );
    let mut full = Meter::local(u64::MAX);
    assert_eq!(effective(&bound, (&[4], &[1]), &mut full).unwrap(), 1);
    let total = full.charged();
    for limit in 0..total {
        let mut work = Meter::local(limit);
        assert_eq!(
            effective(&bound, (&[4], &[1]), &mut work),
            Err(Failure::Stopped(Stop::WorkLimit))
        );
        assert!(work.charged() <= limit);
        assert_eq!(&bound.support[..], &[7, 0]);
        assert_eq!(
            bound.guard,
            [Literal {
                atom: 2,
                positive: true
            }]
        );
    }
}

#[test]
fn pair_replacement_commits_before_semantic_comparison() {
    for limit in 0..=8 {
        let mut sums = PairSums { lower: 1, upper: 1 };
        let mut work = Meter::local(limit);
        let result = replace(&mut sums, BoundSide::Lower, 1, 2, &mut work);
        if limit < 8 {
            assert_eq!(result, Err(Failure::Stopped(Stop::WorkLimit)));
        } else {
            assert_eq!(result, Ok(true));
        }
        assert_eq!(sums.lower, if limit < 6 { 1 } else { 2 });
        assert_eq!(sums.upper, 1);
        assert_eq!(
            work.charged(),
            match limit {
                0 => 0,
                1 | 2 => 1,
                3 | 4 => 3,
                5 => 5,
                6 | 7 => 6,
                _ => 8,
            }
        );
    }
}

#[test]
fn invalid_pair_arithmetic_is_not_a_refutation() {
    for (initial, old, next, charged) in [(0, 1, 0, 3), (u64::MAX, 0, 1, 5)] {
        let mut sums = PairSums {
            lower: initial,
            upper: u64::MAX,
        };
        let mut work = Meter::local(u64::MAX);
        assert_eq!(
            replace(&mut sums, BoundSide::Lower, old, next, &mut work),
            Err(Failure::Stopped(Stop::InvalidProgram))
        );
        assert_eq!(sums.lower, initial);
        assert_eq!(sums.upper, u64::MAX);
        assert_eq!(work.charged(), charged);
    }
}

#[test]
fn logical_support_width_excludes_authenticated_padding() {
    let mut bound = Bound {
        support: Words::new(vec![1, 2, 1, 0]),
        cardinality: 3,
        guard: vec![],
        side: BoundSide::Lower,
        value: 1,
        source: Source {
            kind: Kind::Asserted,
            root: 0,
            node: 0,
        },
    };
    let mut lower_work = Meter::local(u64::MAX);
    assert_eq!(
        effective(&bound, (&[1, 0, 1], &[0, 0, 0]), &mut lower_work).unwrap(),
        2
    );
    // Three logical words: counted intersection10; direction/conversion/
    // dispatch3; empty guard result/test2; direction/clamp/value3. No fourth-word reads.
    assert_eq!(lower_work.charged(), 18);
    bound.side = BoundSide::Upper;
    bound.value = 2;
    let mut upper_work = Meter::local(u64::MAX);
    assert_eq!(
        effective(&bound, (&[0, 0, 0], &[1, 2, 0]), &mut upper_work).unwrap(),
        1
    );
    assert_eq!(upper_work.charged(), 20);
}
