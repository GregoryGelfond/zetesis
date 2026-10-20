//! Complement deductions use one drained snapshot and ordinary propagation.

use super::*;
use crate::Knowledge;
use crate::regions::bit;

fn unopened_snapshot(index: &Narrower, bounds: [i128; 2]) -> Knowledge {
    let mut knowledge = index.knowledge();
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("the small forcing fixtures use compact counters");
    };
    // Initial condition masks are empty and initial numeric sums already match
    // them. Install the two requirements without processing any formula events.
    known.thresholds[0].lower = bounds[0];
    known.thresholds[0].upper = Some(bounds[1]);
    knowledge
}

#[test]
fn a_batch_keeps_its_processed_snapshot_until_events_drain() {
    for (target, expected) in [(15, [false, false, true]), (16, [true, true, false])] {
        let theory = interval(3, &[(0, 6), (1, 10), (2, 15)], [target, target + 1]);
        let index = Narrower::new(&theory);
        let mut knowledge = unopened_snapshot(&index, [target, target + 1]);
        let Width::Compact16(known) = &mut knowledge.width else {
            unreachable!();
        };
        let before = known.thresholds[0];
        let mut scratch = NarrowingScratch::default();
        let mut closure = Closure {
            known: known.borrow(),
            lists: &mut scratch,
        };
        assert!(matches!(
            closure.threshold_residual(&index, 0, &mut Work::new(u64::MAX)),
            Ok(Step::Changed)
        ));
        // All three complement results came from the same original masks and
        // sums, although each preceding publication immediately changed a bit.
        assert_eq!(closure.known.thresholds[0], before);
        assert_eq!(closure.lists.nodes.len(), 3);
        for (condition, truth) in expected.into_iter().enumerate() {
            assert_eq!(bit(closure.known.sure, condition), truth);
            assert_eq!(bit(closure.known.never, condition), !truth);
            assert!(closure.lists.nodes.contains(&(condition, truth)));
        }
        let mut region = Region::all_open(3);
        let mut knowledge = index.knowledge();
        assert_ne!(
            close(&theory, &index, &mut region, &mut knowledge, &mut scratch).0,
            Narrowing::Refuted
        );
        for (condition, truth) in expected.into_iter().enumerate() {
            assert_eq!(region.decision(condition), Some(truth));
        }
        let Width::Compact16(known) = &knowledge.width else {
            unreachable!();
        };
        assert_eq!(known.thresholds[0].known, target);
        assert_eq!(known.thresholds[0].possible, target);
        assert!(scratch.nodes.is_empty());
        assert!(scratch.residual_rows.is_empty());
    }
}

#[test]
fn one_small_coefficient_can_be_forced_and_two_open_choices_remain() {
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
    assert!(region.is_held(0));
    assert!(region.is_held(1));
    assert!(region.is_open(2));
    assert!(region.is_open(3));
    assert!(scratch.residual_suffix.capacity() >= 5);
    // A two-condition interval admitting both individual coefficients does not
    // force a choice merely because only one coefficient fits its width.
    let theory = interval(2, &[(0, 2), (1, 3)], [2, 4]);
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(2);
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
    assert!(region.is_open(0) && region.is_open(1));
}

#[test]
fn wide_intervals_skip_complement_storage() {
    let theory = interval(
        8,
        &(0..8).map(|atom| (atom, 5)).collect::<Vec<_>>(),
        [17, 22],
    );
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(8);
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
    assert!((0..8).all(|atom| region.is_open(atom)));
    assert_eq!(scratch.residual_suffix.capacity(), 0);
}

fn contradictory_conditions() -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap());
    let mut append = nodes.transaction();
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let negative = append
        .push(NodeView::Implies(0, falsum), usize::MAX, usize::MAX)
        .unwrap();
    let guards = append
        .push_threshold_family(
            &[0, negative, 1],
            &[6, 10, 15],
            &[16, 17],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let upper = append
        .push(NodeView::Implies(guards[1], falsum), usize::MAX, usize::MAX)
        .unwrap();
    append.commit();
    Theory::new(
        2,
        nodes.into_parts(),
        vec![guards[0], upper],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn correlated_forced_conditions_return_through_the_boolean_queues() {
    let theory = contradictory_conditions();
    let index = Narrower::new(&theory);
    let mut knowledge = unopened_snapshot(&index, [16, 17]);
    let Width::Compact16(known) = &mut knowledge.width else {
        unreachable!();
    };
    let before = known.thresholds[0];
    let mut scratch = NarrowingScratch::default();
    let mut closure = Closure {
        known: known.borrow(),
        lists: &mut scratch,
    };
    assert!(matches!(
        closure.threshold_residual(&index, 0, &mut Work::new(u64::MAX)),
        Ok(Step::Changed)
    ));
    // Atom 0 and its negation (node 3) are distinct conditions. Arithmetic
    // publishes both; it does not pretend their Boolean consequences ran.
    assert!(closure.lists.nodes.contains(&(0, true)));
    assert!(closure.lists.nodes.contains(&(3, true)));
    assert!(closure.lists.nodes.contains(&(1, false)));
    assert_eq!(closure.known.thresholds[0], before);
    assert_eq!(
        close(
            &theory,
            &index,
            &mut Region::all_open(2),
            &mut index.knowledge(),
            &mut scratch
        )
        .0,
        Narrowing::Refuted
    );
    let cancellation = Cancellation::default();
    assert!((0..4).all(|mask| {
        !models(
            &theory,
            &crate::test_support::world(&theory, mask),
            Limits::default(),
            &cancellation,
        )
        .unwrap()
    }));
}

fn choices_with_interval(atoms: usize, terms: &[(usize, u64)], target: i128) -> Theory {
    let interval = interval(atoms, terms, [target, target + 1]);
    let mut nodes = FormulaNodes::new(interval.parts().try_clone().unwrap());
    let mut roots = interval.roots().to_vec();
    let mut append = nodes.transaction();
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    for atom in 0..atoms {
        let negative = append
            .push(NodeView::Implies(atom, falsum), usize::MAX, usize::MAX)
            .unwrap();
        roots.push(
            append
                .push(NodeView::Or(&[atom, negative]), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    append.commit();
    Theory::new(atoms, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

#[test]
fn inherited_enumeration_matches_independent_complete_families() {
    // The expected families use raw input occurrences, not grouped rows or
    // divisibility. Choice roots make every satisfying selection stable, checked
    // by the exhaustive reduct oracle. Traversal inherits real knowledge at splits.
    for terms in [
        vec![(0, 6), (1, 10), (2, 15)],
        vec![(0, 2), (0, 4), (1, 10), (2, 15)],
        vec![(0, 1), (1, 6), (2, 10), (3, 10)],
        vec![(0, 6), (1, 10), (2, 15), (3, 7)],
    ] {
        let atoms = terms.iter().map(|&(atom, _)| atom).max().unwrap() + 1;
        let total: i128 = terms.iter().map(|&(_, weight)| i128::from(weight)).sum();
        for target in 0..=total + 1 {
            let theory = choices_with_interval(atoms, &terms, target);
            let index = Narrower::new(&theory);
            let expected: Vec<_> = (0..1 << atoms)
                .filter(|&mask| {
                    terms
                        .iter()
                        .filter(|&&(atom, _)| mask & (1 << atom) != 0)
                        .map(|&(_, weight)| i128::from(weight))
                        .sum::<i128>()
                        == target
                })
                .collect();
            let mut pending = vec![(Region::all_open(atoms), index.knowledge())];
            let mut scratch = NarrowingScratch::default();
            let mut actual = Vec::new();
            while let Some((mut region, mut knowledge)) = pending.pop() {
                if close(&theory, &index, &mut region, &mut knowledge, &mut scratch).0
                    == Narrowing::Refuted
                {
                    continue;
                }
                if let Some(atom) = (0..atoms).find(|&atom| region.is_open(atom)) {
                    let mut held = region.clone();
                    assert!(held.hold(atom));
                    assert!(region.cut(atom));
                    pending.push((region, knowledge.clone()));
                    pending.push((held, knowledge));
                } else {
                    let mask = (0..atoms)
                        .filter(|&atom| region.is_held(atom))
                        .fold(0, |mask, atom| mask | (1 << atom));
                    assert!(
                        crate::check(
                            &theory,
                            &crate::test_support::world(&theory, mask),
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap()
                        .accepted()
                    );
                    actual.push(mask);
                }
            }
            actual.sort_unstable();
            assert_eq!(actual, expected, "terms={terms:?}, target={target}");
        }
    }
}

#[test]
fn stopped_forcing_batches_leave_only_reusable_scratch_capacity() {
    let theory = interval(3, &[(0, 6), (1, 10), (2, 15)], [16, 17]);
    let index = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    let completed = close(
        &theory,
        &index,
        &mut Region::all_open(3),
        &mut index.knowledge(),
        &mut scratch,
    );
    assert_ne!(completed.0, Narrowing::Refuted);
    assert!(scratch.residual_suffix.capacity() >= 4);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for limit in 0..completed.1.work {
            let attempt = index.narrow_known_reserved(
                OriginalSubject::new(&theory, None),
                &mut Region::all_open(3),
                &mut index.knowledge(),
                &mut scratch,
                &Cancellation::default(),
                &mut Allowance {
                    remaining: limit,
                    failure,
                },
            );
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.statistics.work, limit);
            let mut region = Region::all_open(3);
            let retried = close(
                &theory,
                &index,
                &mut region,
                &mut index.knowledge(),
                &mut scratch,
            );
            assert_eq!(retried, completed);
            assert!(region.is_held(0) && region.is_held(1) && region.is_cut(2));
            assert!(scratch.nodes.is_empty() && scratch.residual_rows.is_empty());
            assert!(scratch.pending_residuals.iter().all(|&word| word == 0));
        }
    }
}

#[test]
fn suffix_storage_is_metered_and_does_not_cross_owners_or_siblings() {
    let theory = interval(3, &[(0, 6), (1, 10), (2, 15)], [16, 17]);
    let index = Narrower::new(&theory);
    let mut knowledge = unopened_snapshot(&index, [16, 17]);
    let Width::Compact16(known) = &mut knowledge.width else {
        unreachable!()
    };
    let mut scratch = NarrowingScratch::default();
    assert!(matches!(
        Closure {
            known: known.borrow(),
            lists: &mut scratch
        }
        .threshold_residual(&index, 0, &mut Work::new(u64::MAX)),
        Ok(Step::Changed)
    ));
    let expected = scratch.nodes.capacity() as u128 * size_of::<(usize, bool)>() as u128
        + scratch.residual_suffix.capacity() as u128 * size_of::<i128>() as u128;
    assert_eq!(scratch.retained_bytes(), expected);
    scratch.prepare(0);
    assert!(scratch.residual_suffix.is_empty());
    assert_eq!(scratch.retained_bytes(), expected);
    for target in [15, 16, 15] {
        let theory = interval(3, &[(0, 6), (1, 10), (2, 15)], [target, target + 1]);
        let index = Narrower::new(&theory);
        let mut region = Region::all_open(3);
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
        assert_eq!(region.is_held(2), target == 15);
        assert_eq!(region.is_held(0), target == 16);
        assert_eq!(region.is_held(1), target == 16);
    }
}

#[test]
fn two_fitting_coefficients_leave_every_complement_a_lattice_witness() {
    // Exhaust the small arithmetic statement independently of the runtime gcd,
    // clipping helper and skip implementation. Bounds satisfy the mandatory
    // single-coefficient rules before the residual gate is consulted.
    for a in 1_i128..=5 {
        for b in 1_i128..=5 {
            for c in 1_i128..=5 {
                let weights = [a, b, c];
                let total = a + b + c;
                for low in 1..total {
                    for high in low + 1..=total {
                        if weights
                            .iter()
                            .any(|&weight| weight > total - low || weight >= high)
                            || weights
                                .iter()
                                .filter(|&&weight| weight <= high - low)
                                .count()
                                < 2
                        {
                            continue;
                        }
                        for (selected, &weight) in weights.iter().enumerate() {
                            let complement: Vec<_> = weights
                                .iter()
                                .enumerate()
                                .filter_map(|(position, &weight)| {
                                    (position != selected).then_some(weight)
                                })
                                .collect();
                            // Trial divisors provide an independent tiny oracle.
                            let divisor = (1..=*complement.iter().min().unwrap())
                                .filter(|divisor| {
                                    complement.iter().all(|weight| weight % divisor == 0)
                                })
                                .max()
                                .unwrap();
                            for base in [0, 7] {
                                for truth in [false, true] {
                                    let start = base + if truth { weight } else { 0 };
                                    let maximum = base + total - if truth { 0 } else { weight };
                                    assert!((start..=maximum)
                                        .step_by(usize::try_from(divisor).unwrap())
                                        .any(|point| base + low <= point && point < base + high),
                                        "weights={weights:?}, bounds={low}..{high}, selected={selected}, truth={truth}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

mod utility;
