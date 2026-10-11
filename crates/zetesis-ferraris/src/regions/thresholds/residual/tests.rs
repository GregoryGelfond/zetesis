//! Residual arithmetic preserves compatible models and consumes only closed sums.

use crate::regions::{Closure, NarrowingStatistics, Step, Width, Work};
use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaParts,
    FrozenSubject, Interpretation, Knowledge, Limits, Narrower, Narrowing, NarrowingQuota,
    NarrowingScratch, Node, NodeView, OriginalSubject, Region, RegionLimits, Theory, models,
};
use zetesis_cpu::{Cancellation, Stop};

use super::{gcd, misses};

fn interval(atoms: usize, terms: &[(usize, u64)], bounds: [i128; 2]) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..atoms).map(Node::atom).collect(), vec![]).unwrap());
    let (conditions, weights): (Vec<_>, Vec<_>) = terms.iter().copied().unzip();
    let guards = nodes
        .push_threshold_family(&conditions, &weights, &bounds, usize::MAX, usize::MAX)
        .unwrap();
    let mut append = nodes.transaction();
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let upper = append
        .push(NodeView::Implies(guards[1], falsum), usize::MAX, usize::MAX)
        .unwrap();
    append.commit();
    Theory::new(
        atoms,
        nodes.into_parts(),
        vec![guards[0], upper],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn grouped() -> Theory {
    let terms: Vec<_> = (0..8)
        .flat_map(|condition| [(condition, 2), (condition, 3)])
        .collect();
    interval(8, &terms, [17, 18])
}

fn residual() -> Theory {
    let terms: Vec<_> = std::iter::once((0, 1))
        .chain((1..=32).map(|condition| (condition, 6)))
        .chain((33..=64).map(|condition| (condition, 10)))
        .collect();
    interval(65, &terms, [17, 18])
}

fn close(
    theory: &Theory,
    index: &Narrower,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    index
        .narrow_known(
            OriginalSubject::new(theory, None),
            region,
            knowledge,
            scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
}

#[test]
fn grouped_divisibility_refutes_beyond_intervals() {
    let theory = grouped();
    let index = Narrower::new(&theory);
    // Eight coefficients of 5: total 40, forcing cutoffs 23 and 18. Neither
    // interval nor a single coefficient establishes the missing value 17.
    assert_eq!(index.thresholds.rows[0].terms.len(), 8);
    assert!(
        index.thresholds.rows[0]
            .terms
            .iter()
            .all(|&(_, weight)| weight == 5)
    );
    assert!(!index.thresholds.has_relations());
    let mut scratch = NarrowingScratch::default();
    let result = close(
        &theory,
        &index,
        &mut Region::all_open(8),
        &mut index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result.0, Narrowing::Refuted);
    assert!(scratch.threshold_rows.capacity() >= 1);
}

#[test]
fn condition_changes_recheck_inherited_residues() {
    let theory = residual();
    let index = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    let mut parent = index.knowledge();
    let mut root = Region::all_open(65);
    assert_ne!(
        close(&theory, &index, &mut root, &mut parent, &mut scratch).0,
        Narrowing::Refuted
    );
    assert!((0..65).all(|atom| root.is_open(atom)));
    // Cutting the weight 1 changes no guard truth, but every remaining sum is
    // even. A requirement-only notification policy misses this contradiction.
    for value in [false, true, false] {
        let mut child = root.clone();
        if value {
            assert!(child.hold(0));
        } else {
            assert!(child.cut(0));
        }
        let mut inherited = parent.clone();
        let mut fresh_region = child.clone();
        let inherited_result = close(&theory, &index, &mut child, &mut inherited, &mut scratch).0;
        let fresh_result = close(
            &theory,
            &index,
            &mut fresh_region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
        )
        .0;
        assert_eq!(inherited_result == Narrowing::Refuted, !value);
        assert_eq!(fresh_result == Narrowing::Refuted, !value);
    }
}

#[test]
fn gcd_one_does_not_claim_attainability() {
    let terms = [(0, 6), (1, 10), (2, 15), (3, 15)];
    let theory = interval(4, &terms, [17, 18]);
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(4);
    assert_ne!(
        close(
            &theory,
            &index,
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted,
    );
    assert!((0..4).all(|atom| region.is_open(atom)));
    // No subset makes 17, but gcd alone cannot establish this hole.
    assert!((0..16).all(|mask| {
        terms
            .iter()
            .filter(|&&(atom, _)| mask & (1 << atom) != 0)
            .map(|&(_, weight)| weight)
            .sum::<u64>()
            != 17
    }));
}

fn partial(mut code: usize, atoms: usize) -> Region {
    let mut region = Region::all_open(atoms);
    for atom in 0..atoms {
        match code % 3 {
            1 => assert!(region.hold(atom)),
            2 => assert!(region.cut(atom)),
            _ => {}
        }
        code /= 3;
    }
    region
}

fn agrees(region: &Region, mask: usize, atoms: usize) -> bool {
    (0..atoms).all(|atom| {
        region
            .decision(atom)
            .is_none_or(|value| value == (mask & (1 << atom) != 0))
    })
}

#[test]
fn every_small_satisfying_extension_is_preserved() {
    let cancellation = Cancellation::default();
    for terms in [
        vec![(0, 1), (1, 2), (2, 3), (3, 4)],
        vec![(0, 2), (1, 4), (2, 6), (3, 8)],
        vec![(0, 2), (0, 3), (1, 2), (1, 3), (2, 5), (3, 5)],
        vec![(0, 0), (1, 0), (2, 0), (3, 0)],
    ] {
        for target in [0, 1, 3, 5, 7, 9, 11, 17, 20, 21] {
            let theory = interval(4, &terms, [target, target + 1]);
            let index = Narrower::new(&theory);
            let expected: Vec<_> = (0..16)
                .filter(|&mask| {
                    let sum: i128 = terms
                        .iter()
                        .filter(|&&(atom, _)| mask & (1 << atom) != 0)
                        .map(|&(_, weight)| i128::from(weight))
                        .sum();
                    let candidate =
                        Interpretation::new(&theory, (0..4).filter(|atom| mask & (1 << atom) != 0))
                            .unwrap();
                    assert_eq!(
                        models(&theory, &candidate, Limits::default(), &cancellation).unwrap(),
                        sum == target
                    );
                    sum == target
                })
                .collect();
            let mut scratch = NarrowingScratch::default();
            for code in 0..81 {
                let before = partial(code, 4);
                let mut after = before.clone();
                let outcome = close(
                    &theory,
                    &index,
                    &mut after,
                    &mut index.knowledge(),
                    &mut scratch,
                )
                .0;
                for &mask in &expected {
                    if agrees(&before, mask, 4) {
                        assert_ne!(
                            outcome,
                            Narrowing::Refuted,
                            "target={target}, region={code}"
                        );
                        assert!(agrees(&after, mask, 4));
                    }
                }
            }
        }
    }
}

#[test]
fn correlated_conditions_keep_all_models() {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..3).map(Node::atom).collect(), vec![]).unwrap());
    let mut append = nodes.transaction();
    let both = append
        .push(NodeView::And(&[0, 1]), usize::MAX, usize::MAX)
        .unwrap();
    let either = append
        .push(NodeView::Or(&[0, 1]), usize::MAX, usize::MAX)
        .unwrap();
    let guards = append
        .push_threshold_family(
            &[both, both, either, 2],
            &[2, 3, 10, 1],
            &[6, 16],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let upper = append
        .push(NodeView::Implies(guards[1], falsum), usize::MAX, usize::MAX)
        .unwrap();
    append.commit();
    let theory = Theory::new(
        3,
        nodes.into_parts(),
        vec![guards[0], upper],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let cancellation = Cancellation::default();
    let models: Vec<_> = (0..8)
        .filter(|&mask| {
            let candidate =
                Interpretation::new(&theory, (0..3).filter(|atom| mask & (1 << atom) != 0))
                    .unwrap();
            models(&theory, &candidate, Limits::default(), &cancellation).unwrap()
        })
        .collect();
    assert!(!models.is_empty());
    for code in 0..27 {
        let before = partial(code, 3);
        let mut after = before.clone();
        let outcome = close(
            &theory,
            &index,
            &mut after,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
        )
        .0;
        for &mask in &models {
            if agrees(&before, mask, 3) {
                assert_ne!(outcome, Narrowing::Refuted);
                assert!(agrees(&after, mask, 3));
            }
        }
    }
}

#[test]
fn grouped_coefficients_retain_i128_width() {
    let terms: Vec<_> = (0..4)
        .flat_map(|condition| [(condition, u64::MAX), (condition, u64::MAX)])
        .collect();
    let coefficient = 2 * i128::from(u64::MAX);
    let target = coefficient + 1;
    let theory = interval(4, &terms, [target, target + 1]);
    let index = Narrower::new(&theory);
    assert_eq!(index.thresholds.rows[0].terms[0].1, coefficient);
    assert_eq!(
        close(
            &theory,
            &index,
            &mut Region::all_open(4),
            &mut index.knowledge(),
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted
    );
}

#[test]
fn lattice_distance_handles_extreme_bounds() {
    assert!(!misses(0, i128::MAX, i128::MAX, i128::MAX).unwrap());
    assert!(misses(0, i128::MAX - 1, i128::MAX - 1, i128::MAX).unwrap());
    assert!(!misses(i128::MAX - 3, i128::MAX - 1, i128::MAX, 2).unwrap());
    assert!(!misses(7, 7, 7, 0).unwrap());
    assert!(misses(7, 8, 9, 0).unwrap());
}

#[test]
fn gcd_refinement_preserves_interval_intersections() {
    // One further coefficient can only enlarge the congruence lattice. The
    // large base also checks the overflow-safe distance representation.
    for divisor in 1..=16 {
        for weight in 1..=16 {
            let refined = gcd(divisor, weight, &mut Work::new(u64::MAX)).unwrap();
            for base in [0, 7, i128::MAX - 64] {
                for offset in 0..=16 {
                    for width in 0..=4 {
                        let lower = base + offset;
                        let upper = lower + width;
                        if !misses(base, lower, upper, divisor).unwrap() {
                            assert!(!misses(base, lower, upper, refined).unwrap());
                        }
                    }
                }
            }
        }
    }
}

fn closed_residual_work(theory: &Theory, region: &mut Region) -> u64 {
    let index = Narrower::new(theory);
    assert_eq!(index.thresholds.rows.len(), 1);
    let mut knowledge = index.knowledge();
    assert_ne!(
        close(
            theory,
            &index,
            region,
            &mut knowledge,
            &mut NarrowingScratch::default(),
        )
        .0,
        Narrowing::Refuted
    );
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("the small residual fixture uses compact counters");
    };
    let mut work = Work::new(u64::MAX);
    let result = Closure {
        known: known.borrow(),
        lists: &mut NarrowingScratch::default(),
    }
    .threshold_residual(&index, 0, &mut work)
    .unwrap();
    assert!(result == Step::Unchanged);
    work.spent
}

#[test]
fn unit_coefficient_ends_residual_scan() {
    let theory = residual();
    let mut region = Region::all_open(65);
    // One row tick and one term read suffice. Reading from the large end
    // instead visits every undecided coefficient before finding the unit.
    assert_eq!(closed_residual_work(&theory, &mut region), 2);
    assert!((0..65).all(|atom| region.is_open(atom)));
}

#[test]
fn intersecting_residue_ends_scan_before_remaining_terms() {
    let theory = residual();
    let mut region = Region::all_open(65);
    assert!(region.hold(0));
    // The unit is fixed, so no remaining coefficient can give gcd 1. The first
    // 10 after the 6s gives gcd 2 and admits 17 = 1 + 16; the remaining 10s
    // cannot invalidate that lattice point. Each term read costs a tick, so a
    // receipt smaller than the row proves that the whole row was not scanned.
    assert!(closed_residual_work(&theory, &mut region) < 65);
    assert!(region.is_held(0));
    assert!((1..65).all(|atom| region.is_open(atom)));
}

#[test]
fn euclidean_steps_preserve_work_refusal() {
    for maximum in 0..3 {
        let mut work = Work::new(maximum);
        assert_eq!(gcd(8, 5, &mut work), Err(Stop::WorkLimit));
        assert_eq!(work.spent, maximum);
    }
    let mut work = Work::new(4);
    assert_eq!(gcd(8, 5, &mut work), Ok(1));
    assert_eq!(work.spent, 4);
}

#[test]
fn unit_rows_do_not_allocate_residual_queue() {
    let theory = interval(8, &(0..8).map(|atom| (atom, 1)).collect::<Vec<_>>(), [3, 4]);
    let index = Narrower::new(&theory);
    assert!(!index.thresholds.needs_row_queue());
    let mut scratch = NarrowingScratch::default();
    assert_ne!(
        close(
            &theory,
            &index,
            &mut Region::all_open(8),
            &mut index.knowledge(),
            &mut scratch
        )
        .0,
        Narrowing::Refuted
    );
    assert_eq!(scratch.threshold_rows.capacity(), 0);
    assert_eq!(scratch.pending_thresholds.capacity(), 0);
    assert_eq!(scratch.threshold_transfers.capacity(), 0);
}

#[test]
fn arithmetic_notifications_do_not_request_transport() {
    let mut scratch = NarrowingScratch::default();
    scratch.prepare_thresholds(130).unwrap();
    for row in [0, 63, 64, 129] {
        scratch.queue_residual(row);
        scratch.queue_residual(row);
    }
    scratch.queue_threshold(63);
    scratch.queue_residual(63);
    assert_eq!(scratch.next_threshold(), Some((129, false)));
    assert_eq!(scratch.next_threshold(), Some((64, false)));
    assert_eq!(scratch.next_threshold(), Some((63, true)));
    assert_eq!(scratch.next_threshold(), Some((0, false)));
    assert_eq!(scratch.next_threshold(), None);
    assert!(scratch.pending_thresholds.iter().all(|&word| word == 0));
    assert!(scratch.threshold_transfers.iter().all(|&word| word == 0));
    let retained = scratch.retained_bytes();
    scratch.queue_threshold(64);
    scratch.prepare(0);
    assert_eq!(scratch.next_threshold(), None);
    scratch.queue_residual(64);
    assert_eq!(scratch.next_threshold(), Some((64, false)));
    assert_eq!(scratch.retained_bytes(), retained);
}

#[test]
fn row_queue_receipt_includes_both_pending_masks() {
    let mut scratch = NarrowingScratch::default();
    scratch.prepare_thresholds(130).unwrap();
    assert!(scratch.threshold_transfers.capacity() >= 3);
    let entries = scratch.threshold_rows.capacity() as u128 * size_of::<usize>() as u128;
    let flags = (scratch.pending_thresholds.capacity() as u128
        + scratch.threshold_transfers.capacity() as u128)
        * size_of::<u64>() as u128;
    assert_eq!(scratch.retained_bytes(), entries + flags);
}

struct Allowance {
    remaining: u64,
    failure: Stop,
}

impl NarrowingQuota for Allowance {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let granted = wanted.min(self.remaining);
        if granted == 0 {
            return Err(self.failure);
        }
        self.remaining -= granted;
        Ok(granted)
    }
    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
    }
}

#[test]
fn stopped_residual_closures_do_not_poison_reuse() {
    let theory = grouped();
    let index = Narrower::new(&theory);
    let mut scratch = NarrowingScratch::default();
    let completed = close(
        &theory,
        &index,
        &mut Region::all_open(8),
        &mut index.knowledge(),
        &mut scratch,
    );
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for limit in 0..completed.1.work {
            let attempt = index.narrow_known_reserved(
                OriginalSubject::new(&theory, None),
                &mut Region::all_open(8),
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
            let actual = close(
                &theory,
                &index,
                &mut Region::all_open(8),
                &mut index.knowledge(),
                &mut scratch,
            );
            assert_eq!(actual.0, completed.0);
            assert_eq!(actual.1.work, completed.1.work);
            assert!(scratch.threshold_rows.is_empty());
            assert!(scratch.pending_thresholds.iter().all(|&word| word == 0));
            assert!(scratch.threshold_transfers.iter().all(|&word| word == 0));
        }
    }
}

#[test]
fn pre_cancelled_residual_check_mutates_nothing() {
    let theory = grouped();
    let index = Narrower::new(&theory);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut scratch = NarrowingScratch::default();
    let mut region = Region::all_open(8);
    let original = region.clone();
    assert_eq!(
        index
            .narrow_known_reserved(
                OriginalSubject::new(&theory, None),
                &mut region,
                &mut index.knowledge(),
                &mut scratch,
                &cancellation,
                &mut Allowance {
                    remaining: u64::MAX,
                    failure: Stop::WorkLimit
                },
            )
            .result,
        Err(Stop::Cancelled)
    );
    assert_eq!(region, original);
    assert_eq!(scratch.retained_bytes(), 0);
}

#[test]
fn frozen_subjects_do_not_run_residual_checks() {
    let theory = interval(4, &[(0, 6), (1, 10), (2, 15), (3, 15)], [16, 18]);
    let index = Narrower::new(&theory);
    let cancellation = Cancellation::default();
    let mut workspace = EvaluationWorkspace::default();
    for mask in 0..16 {
        let original =
            Interpretation::new(&theory, (0..4).filter(|atom| mask & (1 << atom) != 0)).unwrap();
        let evaluation = workspace
            .evaluate(&original, EvaluationLimits::default(), &cancellation)
            .result
            .unwrap();
        let truth: Vec<_> = (0..theory.nodes().len())
            .map(|node| evaluation.node_truth(node).unwrap())
            .collect();
        for code in 0..81 {
            let mut scratch = NarrowingScratch::default();
            index
                .narrow_frozen_known(
                    FrozenSubject::new(&theory, &truth),
                    &mut partial(code, 4),
                    &mut index.knowledge(),
                    &mut scratch,
                    RegionLimits::default(),
                    &cancellation,
                )
                .unwrap();
            assert_eq!(scratch.threshold_rows.capacity(), 0);
            assert_eq!(scratch.pending_thresholds.capacity(), 0);
            assert_eq!(scratch.threshold_transfers.capacity(), 0);
        }
    }
}
