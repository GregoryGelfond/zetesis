//! A coalesced residual scan consumes its first open lattice witness once.

use super::*;

fn fives(bounds: [i128; 2]) -> Theory {
    interval(8, &(0..8).map(|atom| (atom, 5)).collect::<Vec<_>>(), bounds)
}

#[test]
fn fitting_coefficient_ends_after_one_charged_term() {
    let theory = fives([17, 22]);
    let mut region = Region::all_open(8);
    // The width is exactly 5. The lattice contains 20; reading one coefficient
    // proves intersection without a modulo test or visiting the other seven.
    assert_eq!(closed_refutation_work(&theory, &mut region), 2);
    assert!((0..8).all(|atom| region.is_open(atom)));
}

#[test]
fn narrower_interval_retains_the_exact_progression_test() {
    let theory = fives([17, 21]);
    let mut region = Region::all_open(8);
    // Width 4 does not satisfy the sufficient test, but 20 still lies inside
    // the interval. The existing charged remainder test supplies this witness.
    assert_eq!(closed_refutation_work(&theory, &mut region), 3);
    assert!((0..8).all(|atom| region.is_open(atom)));
}

#[test]
fn the_exclusive_endpoint_does_not_supply_a_witness() {
    for upper in [18, 20] {
        let theory = fives([17, upper]);
        let index = Narrower::new(&theory);
        // In particular 20 is excluded from [17,20), so the neighboring
        // progression point cannot make this required interval consistent.
        assert_eq!(
            close(
                &theory,
                &index,
                &mut Region::all_open(8),
                &mut index.knowledge(),
                &mut NarrowingScratch::default(),
            )
            .0,
            Narrowing::Refuted,
        );
    }
}

#[test]
fn a_decided_unit_cannot_hide_a_residual_contradiction() {
    let terms: Vec<_> = std::iter::once((0, 1))
        .chain((1..=8).map(|atom| (atom, 5)))
        .collect();
    let theory = interval(9, &terms, [17, 18]);
    let index = Narrower::new(&theory);
    for value in [false, true] {
        let mut region = Region::all_open(9);
        if value {
            assert!(region.hold(0));
        } else {
            assert!(region.cut(0));
        }
        // Mandatory closure must consume the decision before arithmetic:
        // neither 5*k nor 1+5*k can be 17. The decided unit is no witness.
        assert_eq!(
            close(
                &theory,
                &index,
                &mut region,
                &mut index.knowledge(),
                &mut NarrowingScratch::default(),
            )
            .0,
            Narrowing::Refuted,
        );
    }
}

#[test]
fn the_first_open_coefficient_after_a_decided_tail_can_finish() {
    let terms: Vec<_> = std::iter::once((0, 1))
        .chain((1..=8).map(|atom| (atom, 5)))
        .collect();
    let theory = interval(9, &terms, [17, 22]);
    for value in [false, true] {
        let mut region = Region::all_open(9);
        if value {
            assert!(region.hold(0));
        } else {
            assert!(region.cut(0));
        }
        // Charge the row, the decided unit, and one open coefficient. No
        // separate tail probe precedes this same ordered term traversal.
        assert_eq!(closed_refutation_work(&theory, &mut region), 3);
        assert_eq!(region.decision(0), Some(value));
        assert!((1..9).all(|atom| region.is_open(atom)));
    }
}

#[test]
fn grouped_unit_occurrences_do_not_supply_a_unit_witness() {
    let terms: Vec<_> = (0..8).flat_map(|atom| [(atom, 1), (atom, 1)]).collect();
    let theory = interval(8, &terms, [7, 8]);
    let index = Narrower::new(&theory);
    assert!(
        index.thresholds.rows[0]
            .terms
            .iter()
            .all(|&(_, weight)| weight == 2)
    );
    assert_eq!(
        close(
            &theory,
            &index,
            &mut Region::all_open(8),
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
        )
        .0,
        Narrowing::Refuted,
    );
}

#[test]
fn fitting_grouped_coefficients_keep_i128_width() {
    let terms: Vec<_> = (0..4)
        .flat_map(|atom| [(atom, u64::MAX), (atom, u64::MAX)])
        .collect();
    let weight = 2 * i128::from(u64::MAX);
    let theory = interval(4, &terms, [weight + 1, 2 * weight + 1]);
    let mut region = Region::all_open(4);
    assert_eq!(closed_refutation_work(&theory, &mut region), 2);
    assert!((0..4).all(|atom| region.is_open(atom)));
}

#[test]
fn negative_lower_requirements_remain_with_mandatory_closure() {
    let theory = fives([-2, 3]);
    let mut region = Region::all_open(8);
    // Nonnegative contributions already satisfy the negative lower bound.
    // The ordinary upper requirement excludes every weight-5 contribution;
    // arithmetic has no residual interval and visits no coefficient.
    assert_eq!(closed_refutation_work(&theory, &mut region), 1);
    assert!((0..8).all(|atom| region.is_cut(atom)));
}

#[test]
fn a_stopped_width_witness_preserves_closed_state_and_can_retry() {
    let theory = fives([17, 22]);
    let index = Narrower::new(&theory);
    let mut knowledge = index.knowledge();
    let mut region = Region::all_open(8);
    assert_ne!(
        close(
            &theory,
            &index,
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default(),
        )
        .0,
        Narrowing::Refuted,
    );
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("the small residual fixture uses compact counters");
    };
    let closure = Closure {
        known: known.borrow(),
        lists: &mut NarrowingScratch::default(),
    };
    let state = closure.known.thresholds[0];
    let masks = (closure.known.sure.to_vec(), closure.known.never.to_vec());
    for cutoff in 0..2 {
        let mut work = Work::new(cutoff);
        assert!(matches!(
            closure.threshold_residual_check(&index, 0, &mut work),
            Err(Stop::WorkLimit),
        ));
        assert_eq!(work.spent, cutoff);
        assert_eq!(closure.known.thresholds[0], state);
        assert_eq!(&*closure.known.sure, masks.0.as_slice());
        assert_eq!(&*closure.known.never, masks.1.as_slice());
    }
    let mut work = Work::new(2);
    assert!(matches!(
        closure.threshold_residual_check(&index, 0, &mut work),
        Ok(Step::Unchanged),
    ));
    assert_eq!(work.spent, 2);
}
