//! Prefix discovery, original bound transport and isolation from frozen closure.

use zetesis_cpu::{Cancellation, Stop};

use crate::regions::{Known, NarrowingStatistics, Width};
use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaParts,
    FrozenSubject, Interpretation, Knowledge, Narrower, Narrowing, NarrowingQuota,
    NarrowingScratch, Node, NodeView, OriginalSubject, Region, RegionLimits, Theory, Threshold,
};

use super::super::{Row, State};
use super::{links, prepare};

fn row(conditions: &[usize], weights: &[u64]) -> Row {
    Row::new(Threshold {
        conditions,
        weights,
        bound: 0,
    })
    .unwrap()
}

#[test]
fn nearest_present_prefixes_form_a_forest() {
    let rows = [
        row(&[0, 1], &[1, 1]),
        row(&[0], &[1]),
        row(&[0, 1, 2], &[1, 1, 1]),
        row(&[0, 2], &[1, 1]),
        row(&[9], &[1]),
    ];
    assert_eq!(links(&rows).unwrap(), [(1, 0), (0, 2), (1, 3)]);
    let adjacency = prepare(&rows).unwrap().unwrap();
    assert_eq!(&adjacency[0], &[1, 2]);
    assert_eq!(&adjacency[1], &[0, 3]);
    assert_eq!(&adjacency[2], &[0]);
    assert_eq!(&adjacency[3], &[1]);
    assert!(adjacency[4].is_empty());
    assert_eq!(adjacency.iter().map(<[usize]>::len).sum::<usize>(), 6);
}

#[test]
fn grouped_equal_rows_form_a_linear_chain() {
    let rows = [
        row(&[1, 0, 0], &[1, 2, 3]),
        row(&[0, 1], &[5, 1]),
        row(&[1, 0], &[1, 5]),
        row(&[0, 1, 2], &[5, 1, 1]),
    ];
    assert_eq!(links(&rows).unwrap(), [(0, 1), (1, 2), (2, 3)]);
    let adjacency = prepare(&rows).unwrap().unwrap();
    assert_eq!(&adjacency[0], &[1]);
    assert_eq!(&adjacency[1], &[0, 2]);
    assert_eq!(&adjacency[2], &[1, 3]);
    assert_eq!(&adjacency[3], &[2]);
}

#[test]
fn empty_rows_are_zero_sum_prefixes() {
    let rows = [row(&[], &[]), row(&[99], &[0]), row(&[0], &[1])];
    assert_eq!(links(&rows).unwrap(), [(0, 1), (1, 2)]);
    assert!(prepare(&rows[..0]).unwrap().is_none());
    assert!(prepare(&rows[..1]).unwrap().is_none());
}

#[test]
fn distinct_condition_ids_remain_distinct() {
    let rows = [row(&[0, 1], &[1, 1]), row(&[0, 2], &[1, 1])];
    assert!(prepare(&rows).unwrap().is_none());
}

#[test]
fn weighted_subsets_without_exact_prefixes_are_unlinked() {
    let rows = [
        row(&[0], &[2]),
        row(&[0, 1], &[1, 1]),
        row(&[0, 1], &[1, 2]),
    ];
    assert!(prepare(&rows).unwrap().is_none());
}

/// Native rows have lengths 3, 4 and 5, with bounds 2, 3 and 2. The middle
/// row has no guard at 2. Atom 5 witnesses the propagated endpoint guard.
fn gap_theory(positive: bool) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..6).map(Node::atom).collect(), vec![]).unwrap());
    let mut guards = Vec::new();
    for (length, bound) in [(3, 2), (4, 3), (5, 2)] {
        guards.push(
            nodes
                .push_threshold_family(
                    &(0..length).collect::<Vec<_>>(),
                    &vec![1; length],
                    &[bound],
                    usize::MAX,
                    usize::MAX,
                )
                .unwrap()[0],
        );
    }
    let mut append = nodes.transaction();
    let requirement = if positive {
        guards[0]
    } else {
        let falsum = append
            .push(NodeView::False, usize::MAX, usize::MAX)
            .unwrap();
        append
            .push(NodeView::Implies(guards[2], falsum), usize::MAX, usize::MAX)
            .unwrap()
    };
    let consequence = if positive {
        NodeView::Implies(guards[2], 5)
    } else {
        NodeView::Implies(5, guards[0])
    };
    let consequence = append.push(consequence, usize::MAX, usize::MAX).unwrap();
    append.commit();
    Theory::new(
        6,
        nodes.into_parts(),
        vec![requirement, consequence],
        AdmissionLimits::default(),
    )
    .unwrap()
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

fn snapshot(knowledge: &Knowledge) -> (Vec<u64>, Vec<State>) {
    fn read<C>(known: &Known<C>) -> (Vec<u64>, Vec<State>) {
        (
            known
                .masks
                .slices()
                .into_iter()
                .flatten()
                .copied()
                .collect(),
            known.thresholds.to_vec(),
        )
    }
    match &knowledge.width {
        Width::Compact16(known) => read(known),
        Width::Compact32(known) => read(known),
        Width::Native(known) => read(known),
    }
}

/// Closed truth and numeric requirements are independent of event order.
/// Term cursors may differ: a fresh row can already meet its lower bound when
/// that requirement arrives and return before skipping decided conditions.
fn assert_closed_equivalent(index: &Narrower, left: &Knowledge, right: &Knowledge) {
    fn check_cursors<C>(index: &Narrower, known: &Known<C>) {
        let [sure, never, ..] = known.masks.slices();
        for (row, state) in index.thresholds.rows.iter().zip(&known.thresholds) {
            // Every passed position was either already decided or forced.
            // Slice bounds also require both cursors to stay within the row.
            for &(condition, _) in &row.terms[..state.true_term.max(state.false_term)] {
                assert!(
                    crate::regions::bit(sure, condition) || crate::regions::bit(never, condition)
                );
            }
        }
    }
    for knowledge in [left, right] {
        match &knowledge.width {
            Width::Compact16(known) => check_cursors(index, known),
            Width::Compact32(known) => check_cursors(index, known),
            Width::Native(known) => check_cursors(index, known),
        }
    }
    let (left_masks, left_rows) = snapshot(left);
    let (right_masks, right_rows) = snapshot(right);
    assert_eq!(left_masks, right_masks);
    assert_eq!(left_rows.len(), index.thresholds.rows.len());
    assert_eq!(left_rows.len(), right_rows.len());
    for (left, right) in left_rows.iter().zip(&right_rows) {
        assert_eq!(
            (left.known, left.possible, left.lower, left.upper),
            (right.known, right.possible, right.lower, right.upper)
        );
        assert_eq!(
            (left.sure_guard, left.never_guard),
            (right.sure_guard, right.never_guard)
        );
    }
}

fn receipt(statistics: NarrowingStatistics) -> [u64; 4] {
    [
        statistics.work,
        statistics.propagations,
        statistics.held,
        statistics.cut,
    ]
}

#[test]
fn requirements_cross_a_missing_middle_guard() {
    for positive in [false, true] {
        let theory = gap_theory(positive);
        let index = Narrower::new(&theory);
        assert!(index.thresholds.has_relations());
        let mut without = index.clone();
        without.thresholds.relations = None;
        let mut baseline = Region::all_open(6);
        close(
            &theory,
            &without,
            &mut baseline,
            &mut without.knowledge(),
            &mut NarrowingScratch::default(),
        );
        assert!(
            baseline.is_open(5),
            "the endpoint needs cross-row transport"
        );
        let mut region = Region::all_open(6);
        let mut knowledge = index.knowledge();
        assert_eq!(
            close(
                &theory,
                &index,
                &mut region,
                &mut knowledge,
                &mut NarrowingScratch::default(),
            )
            .0,
            Narrowing::Fixed { changed: true }
        );
        assert!(!region.is_open(5));
        assert_eq!(region.is_held(5), positive);
        for state in snapshot(&knowledge).1 {
            if positive {
                assert_eq!(state.lower, 2);
            } else {
                assert_eq!(state.upper, Some(2));
            }
        }
    }
}

fn decide_shared(region: &mut Region, mut code: usize) {
    for atom in 0..3 {
        match code % 3 {
            1 => assert!(region.hold(atom)),
            2 => assert!(region.cut(atom)),
            _ => {}
        }
        code /= 3;
    }
}

#[test]
fn inherited_bounds_are_closed_and_branch_local() {
    for positive in [false, true] {
        let theory = gap_theory(positive);
        let index = Narrower::new(&theory);
        let mut parent = index.knowledge();
        let mut root = Region::all_open(6);
        let mut scratch = NarrowingScratch::default();
        close(&theory, &index, &mut root, &mut parent, &mut scratch);
        let before = snapshot(&parent);
        let mut child = parent.clone();
        for code in 0..27 {
            child.clone_from(&parent);
            let mut region = root.clone();
            decide_shared(&mut region, code);
            let mut fresh_region = region.clone();
            let mut fresh = index.knowledge();
            let inherited = close(&theory, &index, &mut region, &mut child, &mut scratch).0;
            let expected = close(
                &theory,
                &index,
                &mut fresh_region,
                &mut fresh,
                &mut NarrowingScratch::default(),
            )
            .0;
            // A fresh closure rediscovers the already incorporated root fact.
            assert_eq!(
                matches!(inherited, Narrowing::Refuted),
                matches!(expected, Narrowing::Refuted)
            );
            assert_eq!(region, fresh_region);
            if !matches!(inherited, Narrowing::Refuted) {
                assert_closed_equivalent(&index, &child, &fresh);
            }
            assert_eq!(snapshot(&parent), before);
        }
    }
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

fn assert_pending_rows(scratch: &NarrowingScratch, rows: usize) {
    for row in 0..rows {
        let queued = scratch
            .threshold_rows
            .iter()
            .filter(|&&pending| pending == row)
            .count();
        let pending = scratch
            .pending_thresholds
            .get(row / 64)
            .is_some_and(|word| word & (1_u64 << (row % 64)) != 0);
        assert_eq!(queued, usize::from(pending));
    }
}

#[test]
fn every_refused_prefix_discards_scratch_authority() {
    let theory = gap_theory(false);
    let index = Narrower::new(&theory);
    let mut expected_region = Region::all_open(6);
    let mut expected = index.knowledge();
    let completed = close(
        &theory,
        &index,
        &mut expected_region,
        &mut expected,
        &mut NarrowingScratch::default(),
    );
    let mut scratch = NarrowingScratch::default();
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for ceiling in 0..completed.1.work {
            let attempt = index.narrow_known_reserved(
                OriginalSubject::new(&theory, None),
                &mut Region::all_open(6),
                &mut index.knowledge(),
                &mut scratch,
                &Cancellation::default(),
                &mut Allowance {
                    remaining: ceiling,
                    failure,
                },
            );
            assert_eq!(attempt.result, Err(failure));
            assert_eq!(attempt.statistics.work, ceiling);
            assert_pending_rows(&scratch, index.thresholds.rows.len());
            let mut region = Region::all_open(6);
            let mut knowledge = index.knowledge();
            let actual = close(&theory, &index, &mut region, &mut knowledge, &mut scratch);
            assert_eq!(actual.0, completed.0);
            assert_eq!(receipt(actual.1), receipt(completed.1));
            assert_eq!(region, expected_region);
            assert_eq!(snapshot(&knowledge), snapshot(&expected));
            assert!(scratch.threshold_rows.is_empty());
            assert!(scratch.pending_thresholds.iter().all(|&word| word == 0));
            assert!(scratch.threshold_transfers.iter().all(|&word| word == 0));
        }
    }
}

#[test]
fn frozen_closure_never_consumes_prefix_relations() {
    for positive in [false, true] {
        let theory = gap_theory(positive);
        let index = Narrower::new(&theory);
        assert!(index.thresholds.has_relations());
        let mut without = index.clone();
        without.thresholds.relations = None;
        let mut evaluation = EvaluationWorkspace::default();
        for candidate in 0_usize..64 {
            let interpretation =
                Interpretation::new(&theory, (0..6).filter(|atom| candidate & (1 << atom) != 0))
                    .unwrap();
            let evaluated = evaluation
                .evaluate(
                    &interpretation,
                    EvaluationLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap();
            let truth: Vec<_> = (0..theory.nodes().len())
                .map(|node| evaluated.node_truth(node).unwrap())
                .collect();
            for code in 0..27 {
                let mut region = Region::all_open(6);
                decide_shared(&mut region, code);
                let mut baseline_region = region.clone();
                let mut knowledge = index.knowledge();
                let mut baseline = without.knowledge();
                let mut scratch = NarrowingScratch::default();
                let run = |index: &Narrower,
                           region: &mut Region,
                           knowledge: &mut Knowledge,
                           scratch: &mut NarrowingScratch| {
                    index
                        .narrow_frozen_known(
                            FrozenSubject::new(&theory, &truth),
                            region,
                            knowledge,
                            scratch,
                            RegionLimits::default(),
                            &Cancellation::default(),
                        )
                        .unwrap()
                };
                let actual = run(&index, &mut region, &mut knowledge, &mut scratch);
                let expected = run(
                    &without,
                    &mut baseline_region,
                    &mut baseline,
                    &mut NarrowingScratch::default(),
                );
                assert_eq!(actual.0, expected.0);
                assert_eq!(receipt(actual.1), receipt(expected.1));
                assert_eq!(region, baseline_region);
                assert_eq!(snapshot(&knowledge), snapshot(&baseline));
                assert_eq!(scratch.threshold_rows.capacity(), 0);
                assert_eq!(scratch.pending_thresholds.capacity(), 0);
            }
        }
    }
}
