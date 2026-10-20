//! A sufficient utility filter must preserve every useful complement deduction.

use super::*;

#[test]
fn coprime_complements_need_no_suffix_storage() {
    let theory = interval(3, &[(0, 1), (1, 2), (2, 3)], [3, 4]);
    let index = Narrower::new(&theory);
    let mut knowledge = unopened_snapshot(&index, [3, 4]);
    let Width::Compact16(known) = &mut knowledge.width else {
        unreachable!();
    };
    let before = known.thresholds[0];
    let mut scratch = NarrowingScratch::default();
    let mut work = Work::new(8);
    let mut closure = Closure {
        known: known.borrow(),
        lists: &mut scratch,
    };
    assert!(matches!(
        closure.threshold_residual(&index, 0, &mut work),
        Ok(Step::Unchanged)
    ));
    // One whole-row witness, three ascending reads, and gcd(2,3).
    assert_eq!(work.spent, 8);
    assert_eq!(closure.known.thresholds[0], before);
    assert!(closure.known.sure.iter().all(|&word| word == 0));
    assert!(closure.known.never.iter().all(|&word| word == 0));
    assert!(scratch.nodes.is_empty());
    assert_eq!(scratch.residual_suffix.capacity(), 0);
    assert_eq!(scratch.retained_bytes(), 0);
}

#[test]
fn a_partial_gcd_witness_avoids_the_unread_tail() {
    let terms: Vec<_> = (0..30).map(|atom| (atom, atom as u64 + 1)).collect();
    let theory = interval(30, &terms, [100, 101]);
    let index = Narrower::new(&theory);
    let mut knowledge = unopened_snapshot(&index, [100, 101]);
    let Width::Compact16(known) = &mut knowledge.width else {
        unreachable!();
    };
    let mut scratch = NarrowingScratch::default();
    let mut work = Work::new(8);
    assert!(matches!(
        Closure {
            known: known.borrow(),
            lists: &mut scratch,
        }
        .threshold_residual(&index, 0, &mut work),
        Ok(Step::Unchanged)
    ));
    // The same allowance as the three-term case cannot read the other 27
    // conditions, allocate a suffix, or perform the complement sweep.
    assert_eq!(work.spent, 8);
    assert_eq!(scratch.residual_suffix.capacity(), 0);
    assert_eq!(scratch.retained_bytes(), 0);
}

#[test]
fn a_large_complement_gcd_retains_productive_forcing() {
    let theory = interval(4, &[(0, 1), (1, 6), (2, 10), (3, 10)], [17, 18]);
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(4);
    let mut scratch = NarrowingScratch::default();
    assert_ne!(
        close(
            &theory,
            &index,
            &mut region,
            &mut index.knowledge(),
            &mut scratch
        )
        .0,
        Narrowing::Refuted
    );
    assert!(region.is_held(0) && region.is_held(1));
    assert!(region.is_open(2) && region.is_open(3));
    assert!(scratch.residual_suffix.capacity() >= 5);
}

#[test]
fn decided_coefficients_supply_neither_utility_witness() {
    for held in [false, true] {
        // A decided unit cannot stand in for an open fitting coefficient.
        let target = 16 + i128::from(held);
        let theory = interval(4, &[(0, 1), (1, 6), (2, 10), (3, 15)], [target, target + 1]);
        let index = Narrower::new(&theory);
        let mut region = Region::all_open(4);
        assert!(if held { region.hold(0) } else { region.cut(0) });
        let mut scratch = NarrowingScratch::default();
        assert_ne!(
            close(
                &theory,
                &index,
                &mut region,
                &mut index.knowledge(),
                &mut scratch
            )
            .0,
            Narrowing::Refuted
        );
        assert!(region.is_held(1) && region.is_held(2) && region.is_cut(3));
        assert!(scratch.residual_suffix.capacity() >= 5);

        // A decided odd coefficient cannot reduce the open even complement's
        // gcd from 2 to 1 and conceal the forced open unit.
        let target = 17 + if held { 3 } else { 0 };
        let theory = interval(
            5,
            &[(0, 1), (1, 3), (2, 6), (3, 10), (4, 10)],
            [target, target + 1],
        );
        let index = Narrower::new(&theory);
        let mut region = Region::all_open(5);
        assert!(if held { region.hold(1) } else { region.cut(1) });
        let mut scratch = NarrowingScratch::default();
        assert_ne!(
            close(
                &theory,
                &index,
                &mut region,
                &mut index.knowledge(),
                &mut scratch
            )
            .0,
            Narrowing::Refuted
        );
        assert!(region.is_held(0) && region.is_held(2));
        assert!(region.is_open(3) && region.is_open(4));
        assert!(scratch.residual_suffix.capacity() >= 6);
    }
}

#[test]
fn interrupted_utility_checks_publish_no_state() {
    let terms: Vec<_> = (0..30).map(|atom| (atom, atom as u64 + 1)).collect();
    let theory = interval(30, &terms, [100, 101]);
    let index = Narrower::new(&theory);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for cutoff in 0..8 {
            let mut knowledge = unopened_snapshot(&index, [100, 101]);
            let Width::Compact16(known) = &mut knowledge.width else {
                unreachable!();
            };
            let before = known.thresholds[0];
            let mut scratch = NarrowingScratch::default();
            let mut closure = Closure {
                known: known.borrow(),
                lists: &mut scratch,
            };
            let mut allowance = Allowance {
                remaining: cutoff,
                failure,
            };
            let mut work = Work::reserved(&mut allowance);
            assert!(
                matches!(closure.threshold_residual(&index, 0, &mut work), Err(stop) if stop == failure)
            );
            assert_eq!(work.spent, cutoff);
            assert_eq!(closure.known.thresholds[0], before);
            assert!(closure.known.sure.iter().all(|&word| word == 0));
            assert!(closure.known.never.iter().all(|&word| word == 0));
            assert!(closure.lists.nodes.is_empty());
            assert_eq!(closure.lists.residual_suffix.capacity(), 0);
            assert!(matches!(
                closure.threshold_residual(&index, 0, &mut Work::new(8)),
                Ok(Step::Unchanged)
            ));
        }
    }
}

#[test]
fn a_fitting_weight_and_small_complement_leave_every_branch_a_witness() {
    // Trial divisors and explicit progression points are independent of the
    // production gcd, clipped-interval helper and utility filter. Only bounds
    // already satisfying the ordinary single-condition rules enter the claim.
    let mut single_fitting = 0;
    for a in 1_i128..=6 {
        for b in 1_i128..=6 {
            for c in 1_i128..=6 {
                let weights = [a, b, c];
                let total = a + b + c;
                for low in 1..total {
                    for high in low + 1..=total {
                        if weights
                            .iter()
                            .any(|&weight| weight > total - low || weight >= high)
                        {
                            continue;
                        }
                        let width = high - low;
                        let useful = weights.iter().enumerate().any(|(selected, &weight)| {
                            if weight > width {
                                return false;
                            }
                            let others: Vec<_> = weights
                                .iter()
                                .enumerate()
                                .filter_map(|(position, &value)| {
                                    (position != selected).then_some(value)
                                })
                                .collect();
                            let divisor = (1..=*others.iter().min().unwrap())
                                .filter(|divisor| others.iter().all(|value| value % divisor == 0))
                                .max()
                                .unwrap();
                            divisor <= width
                        });
                        if !useful {
                            continue;
                        }
                        single_fitting += usize::from(
                            weights.iter().filter(|&&weight| weight <= width).count() == 1,
                        );
                        for (selected, &weight) in weights.iter().enumerate() {
                            let others: Vec<_> = weights
                                .iter()
                                .enumerate()
                                .filter_map(|(position, &value)| {
                                    (position != selected).then_some(value)
                                })
                                .collect();
                            let divisor = (1..=*others.iter().min().unwrap())
                                .filter(|divisor| others.iter().all(|value| value % divisor == 0))
                                .max()
                                .unwrap();
                            for base in [0, 7] {
                                for held in [false, true] {
                                    let start = base + if held { weight } else { 0 };
                                    let maximum = base + total - if held { 0 } else { weight };
                                    assert!((start..=maximum).step_by(usize::try_from(divisor).unwrap())
                                        .any(|point| base + low <= point && point < base + high),
                                        "weights={weights:?}, bounds={low}..{high}, selected={selected}, held={held}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        single_fitting > 0,
        "the stronger gate exceeds the two-fitting criterion"
    );
}
