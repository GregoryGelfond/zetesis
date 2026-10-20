//! Capacity implications require original aliases, grouped unit rows and paid
//! condition events. Pending truth can weaken a bound but cannot strengthen it.

use crate::regions::{Closure, Known, Step, Work, bit};
use crate::test_support::{SinglePermits, close};
use crate::{
    AdmissionLimits, FormulaNodes, FormulaParts, Narrower, Narrowing, NarrowingScratch, Node,
    NodeView, Region, Theory,
};
use zetesis_cpu::Stop;

use super::push;

#[test]
fn disjunctive_antecedents_supply_necessary_aliases() {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..8).map(Node::atom).collect(), vec![]).unwrap());
    let falsum = push(&mut nodes, NodeView::False);
    let count = nodes
        .push_threshold_family(&[0, 1, 2, 3], &[1, 1, 1, 1], &[3], usize::MAX, usize::MAX)
        .unwrap()[0];
    let upper = push(&mut nodes, NodeView::Implies(count, falsum));
    let antecedent = push(&mut nodes, NodeView::Or(&[4, 5]));
    let alias = push(&mut nodes, NodeView::Implies(antecedent, 0));
    let other_alias = push(&mut nodes, NodeView::Implies(6, 1));
    let both = push(&mut nodes, NodeView::And(&[4, 6]));
    let alternative = push(&mut nodes, NodeView::Or(&[both, 7]));
    let theory = Theory::new(
        8,
        nodes.into_parts(),
        vec![upper, alias, other_alias, alternative],
        AdmissionLimits::default(),
    )
    .unwrap();
    let index = Narrower::new(&theory);
    let mut without = index.clone();
    without.thresholds.capacity = None;
    let mut initial = Region::all_open(8);
    initial.hold(2);
    let mut expected = initial.clone();
    assert_ne!(
        close(
            &theory,
            &without,
            &mut expected,
            &mut without.knowledge(),
            &mut NarrowingScratch::default(),
        )
        .0,
        Narrowing::Refuted
    );
    assert!(expected.is_open(7));
    let mut actual = initial;
    assert_ne!(
        close(
            &theory,
            &index,
            &mut actual,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
        )
        .0,
        Narrowing::Refuted
    );
    assert!(actual.is_held(7));
    assert!(
        (0..7)
            .filter(|&atom| atom != 2)
            .all(|atom| actual.is_open(atom))
    );
}

/// Two distinct conjunctions require two row conditions each. Holding condition
/// 2 makes either conjunction exceed the exclusive count bound of three.
fn direct(weights: [u64; 4]) -> (Theory, [usize; 2]) {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..4).map(Node::atom).collect(), vec![]).unwrap());
    let count = nodes
        .push_threshold_family(&[0, 1, 2, 3], &weights, &[3], usize::MAX, usize::MAX)
        .unwrap()[0];
    let falsum = push(&mut nodes, NodeView::False);
    let upper = push(&mut nodes, NodeView::Implies(count, falsum));
    let first = push(&mut nodes, NodeView::And(&[0, 1]));
    let second = push(&mut nodes, NodeView::And(&[0, 3]));
    (
        Theory::new(
            4,
            nodes.into_parts(),
            vec![upper],
            AdmissionLimits::default(),
        )
        .unwrap(),
        [first, second],
    )
}

#[test]
fn pending_truth_waits_for_its_counted_event() {
    let (theory, [first, _]) = direct([1; 4]);
    let index = Narrower::new(&theory);
    assert_eq!(index.thresholds.rows.len(), 1);
    let mut known = index.root_known::<u16>();
    known.thresholds[0].upper = Some(3);
    let mut lists = NarrowingScratch::default();
    lists.prepare_thresholds(1).unwrap();
    let mut closure = Closure {
        known: known.borrow(),
        lists: &mut lists,
    };
    assert!(closure.sure(2) == Step::Changed);
    assert_eq!(closure.lists.nodes.pop(), Some((2, true)));
    assert!(
        closure
            .threshold_condition(&index, 2, true, None, &mut Work::new(u64::MAX))
            .is_ok()
    );
    assert_eq!(closure.known.thresholds[0].known, 1);
    assert!(closure.sure(0) == Step::Changed);
    assert_eq!(closure.lists.next_threshold(), Some(0));
    assert!(
        closure
            .threshold_capacity(&index, 0, &mut Work::new(u64::MAX))
            .unwrap()
            == Step::Unchanged
    );
    assert!(!bit(closure.known.never, first));
    assert_eq!(closure.lists.next_threshold(), None);
    // The pending true bit is not a counted contribution yet. Processing its
    // event must request another row visit, even though the upper did not move.
    assert_eq!(closure.lists.nodes.pop(), Some((0, true)));
    assert!(
        closure
            .threshold_condition(&index, 0, true, None, &mut Work::new(u64::MAX))
            .is_ok()
    );
    assert_eq!(closure.known.thresholds[0].known, 2);
    assert_eq!(closure.lists.next_threshold(), Some(0));
    assert!(
        closure
            .threshold_capacity(&index, 0, &mut Work::new(u64::MAX))
            .unwrap()
            == Step::Changed
    );
    assert!(bit(closure.known.never, first));
}

struct Attempt {
    result: Result<Step, Stop>,
    known: Known<u16>,
    events: Vec<(usize, bool)>,
}

fn attempt(index: &Narrower, held: Option<usize>, work: &mut Work<'_>) -> Attempt {
    let mut known = index.root_known::<u16>();
    known.thresholds[0].known = 1;
    known.thresholds[0].upper = Some(3);
    let mut lists = NarrowingScratch::default();
    let mut closure = Closure {
        known: known.borrow(),
        lists: &mut lists,
    };
    assert!(closure.sure(2) == Step::Changed);
    if let Some(node) = held {
        assert!(closure.sure(node) == Step::Changed);
    }
    closure.lists.nodes.clear();
    let result = closure.threshold_capacity(index, 0, work);
    Attempt {
        result,
        known,
        events: lists.nodes,
    }
}

#[test]
fn interrupted_capacity_keeps_only_paid_deductions() {
    let (theory, heads) = direct([1; 4]);
    let index = Narrower::new(&theory);
    for held in [None, Some(heads[1])] {
        let mut full_work = Work::new(u64::MAX);
        let full = attempt(&index, held, &mut full_work);
        assert!(
            full.result
                == Ok(if held.is_some() {
                    Step::Contradiction
                } else {
                    Step::Changed
                })
        );
        assert_eq!(full.events.first(), Some(&(heads[0], false)));
        // Each deduction needs a footprint visit and two distinct conditions.
        assert_eq!(full_work.spent, 6);
        for stop in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
            for limit in 0..=full_work.spent {
                let mut quota = SinglePermits::new(limit, stop);
                let mut work = Work::reserved(&mut quota);
                let partial = attempt(&index, held, &mut work);
                assert_eq!(work.spent, limit);
                if limit == full_work.spent {
                    assert!(partial.result == full.result);
                    assert_eq!(partial.events, full.events);
                    assert_eq!(partial.known.masks.slices(), full.known.masks.slices());
                } else {
                    assert!(partial.result == Err(stop));
                    assert!(full.events.starts_with(&partial.events));
                    assert_eq!(partial.events.len(), usize::from(limit >= 3));
                    assert!(!bit(partial.known.masks.slices()[1], heads[1]));
                }
                work.refund();
                assert_eq!(quota.remaining, 0);
            }
        }
    }
}

#[test]
fn nonunit_rows_retain_no_capacity_projection() {
    let (unit, _) = direct([1; 4]);
    let unit = Narrower::new(&unit);
    assert!(unit.capacity_index_bytes() > 0);
    // A positive non-unit term disqualifies the entire row, even where this
    // particular conjunction mentions only unit terms. It cannot be counted
    // by the unit-row formula without a separately justified weighted rule.
    for weights in [[2, 1, 1, 1], [1, 1, 2, 1]] {
        let (theory, _) = direct(weights);
        let index = Narrower::new(&theory);
        assert!(index.thresholds.capacity.is_none());
        assert_eq!(index.capacity_index_bytes(), 0);
    }
}
