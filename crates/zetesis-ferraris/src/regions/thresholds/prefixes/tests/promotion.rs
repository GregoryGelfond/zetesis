//! Residual notifications do not change mandatory transport order.
//! Both pending schedules must reach the same closed facts and split score.

use crate::regions::{Closure, NarrowingStatistics, Step, Subject, Width, Work, bit};
use crate::{
    AdmissionLimits, FormulaNodes, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingScratch,
    Node, NodeView, Region, Theory,
};

use super::{assert_closed_equivalent, close as narrow};

struct Fixture {
    theory: Theory,
    index: Narrower,
    region: Region,
    knowledge: Knowledge,
    endpoints: [usize; 2],
}

impl Fixture {
    /// A=[2a,2b,2c], B=A+[d], C=B+[e]. Require 3<=A<6 and C<6;
    /// C>=3 implies z. B has no guard at 3, so numeric transport must cross it.
    fn new() -> Self {
        let mut nodes =
            FormulaNodes::new(FormulaParts::new((0..6).map(Node::atom).collect(), vec![]).unwrap());
        let mut guards = Vec::new();
        for (conditions, weights, bounds) in [
            (&[0, 1, 2][..], &[2, 2, 2][..], &[3, 6][..]),
            (&[0, 1, 2, 3][..], &[2, 2, 2, 1][..], &[4][..]),
            (&[0, 1, 2, 3, 4][..], &[2, 2, 2, 1, 1][..], &[3, 6][..]),
        ] {
            guards.push(
                nodes
                    .push_threshold_family(conditions, weights, bounds, usize::MAX, usize::MAX)
                    .unwrap(),
            );
        }
        let mut append = nodes.transaction();
        let falsum = append
            .push(NodeView::False, usize::MAX, usize::MAX)
            .unwrap();
        let mut roots = vec![guards[0][0]];
        for guard in [guards[0][1], guards[2][1]] {
            roots.push(
                append
                    .push(NodeView::Implies(guard, falsum), usize::MAX, usize::MAX)
                    .unwrap(),
            );
        }
        roots.push(
            append
                .push(NodeView::Implies(guards[2][0], 5), usize::MAX, usize::MAX)
                .unwrap(),
        );
        append.commit();
        let theory = Theory::new(6, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
        let index = Narrower::new(&theory);
        let endpoints = [
            index.thresholds.row(&theory, guards[0][0]),
            index.thresholds.row(&theory, guards[2][0]),
        ];
        assert_eq!(index.thresholds.rows.len(), 3);
        assert!(index.thresholds.has_relations());
        let mut without = index.clone();
        without.thresholds.relations = None;
        let mut region = Region::all_open(6);
        let mut knowledge = index.knowledge();
        assert_ne!(
            narrow(
                &theory,
                &without,
                &mut region,
                &mut knowledge,
                &mut NarrowingScratch::default(),
            )
            .0,
            Narrowing::Refuted
        );
        assert!((0..6).all(|atom| region.is_open(atom)));
        Self {
            theory,
            index,
            region,
            knowledge,
            endpoints,
        }
    }

    fn queued(&self, promoted: bool) -> NarrowingScratch {
        let mut scratch = NarrowingScratch::default();
        scratch.prepare(self.theory.atom_count());
        scratch
            .prepare_thresholds(self.index.thresholds.rows.len())
            .unwrap();
        let [initial, extended] = self.endpoints;
        if promoted {
            scratch
                .prepare_residuals(self.index.thresholds.rows.len())
                .unwrap();
            scratch.queue_residual(initial);
        }
        scratch.queue_threshold(extended);
        scratch.queue_threshold(initial);
        scratch
    }

    /// Enter the existing closure directly: the public narrowing wrapper clears
    /// scratch first and would erase the pending schedules this test compares.
    fn finish(&self, promoted: bool, region: &mut Region, knowledge: &mut Knowledge) -> bool {
        let [initial, extended] = self.endpoints;
        let Width::Compact16(before) = &knowledge.width else {
            panic!("the small prefix fixture uses compact counters");
        };
        assert!(before.thresholds[initial].has_residual_interval());
        let order = [initial, extended];
        let mut scratch = self.queued(promoted);
        for row in order {
            assert_eq!(scratch.next_threshold(), Some(row));
        }
        assert_eq!(scratch.next_threshold(), None);
        assert_eq!(scratch.next_residual(), promoted.then_some(initial));
        assert_eq!(scratch.next_residual(), None);
        scratch = self.queued(promoted);
        finish_pending(&self.theory, &self.index, region, knowledge, &mut scratch)
    }
}

/// Drain one already authenticated fixed-state schedule, then publish its
/// learned atom decisions exactly as the ordinary narrowing wrapper does.
fn finish_pending(
    theory: &Theory,
    index: &Narrower,
    region: &mut Region,
    knowledge: &mut Knowledge,
    scratch: &mut NarrowingScratch,
) -> bool {
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("the small prefix fixture uses compact counters");
    };
    let mut closure = Closure {
        known: known.borrow(),
        lists: scratch,
    };
    let outcome = closure
        .close(
            Subject {
                theory,
                producers: None,
                frozen: None,
                choices: &crate::regions::NO_CHOICES,
                generator_order: None,
            },
            index,
            region,
            &mut Work::new(u64::MAX),
            &mut NarrowingStatistics::default(),
        )
        .unwrap();
    if outcome == Step::Contradiction {
        return true;
    }
    for atom in closure.lists.learned.drain(..) {
        assert!(if bit(closure.known.atom_sure, atom) {
            region.hold(atom)
        } else {
            region.cut(atom)
        });
    }
    region.snapshot_decided(closure.known.seen);
    assert!(closure.lists.threshold_rows.is_empty());
    assert!(closure.lists.residual_rows.is_empty());
    assert!(
        closure
            .lists
            .pending_thresholds
            .iter()
            .all(|&word| word == 0)
    );
    assert!(
        closure
            .lists
            .pending_residuals
            .iter()
            .all(|&word| word == 0)
    );
    false
}

fn assert_same_choice(index: &Narrower, region: &Region, left: &Knowledge, right: &Knowledge) {
    assert_closed_equivalent(index, left, right);
    let (Width::Compact16(left_known), Width::Compact16(right_known)) = (&left.width, &right.width)
    else {
        panic!("the small prefix fixture uses compact counters");
    };
    let left_scores: Vec<_> = (0..index.atom_nodes.len())
        .map(|atom| left_known.unknown.get(atom))
        .collect();
    let right_scores: Vec<_> = (0..index.atom_nodes.len())
        .map(|atom| right_known.unknown.get(atom))
        .collect();
    assert_eq!(left_scores, right_scores);
    let preferred = left
        .preferred_atom(
            region,
            &crate::ChoiceHeads::default(),
            &mut Work::new(u64::MAX),
        )
        .unwrap();
    assert_eq!(preferred.is_some(), region.open().next().is_some());
    assert_eq!(
        preferred,
        right
            .preferred_atom(
                region,
                &crate::ChoiceHeads::default(),
                &mut Work::new(u64::MAX)
            )
            .unwrap()
    );
}

#[test]
fn promoted_residual_keeps_closed_split_knowledge() {
    let fixture = Fixture::new();
    let mut left = fixture.knowledge.clone();
    let mut right = fixture.knowledge.clone();
    let mut left_region = fixture.region.clone();
    let mut right_region = fixture.region.clone();
    assert!(!fixture.finish(true, &mut left_region, &mut left));
    assert!(!fixture.finish(false, &mut right_region, &mut right));
    assert_eq!(left_region, right_region);
    assert_eq!(left_region.decision(5), Some(true));
    assert!((0..5).all(|atom| left_region.is_open(atom)));
    assert_same_choice(&fixture.index, &left_region, &left, &right);

    // Different forcing cursors must also remain interchangeable after a split.
    assert!(left_region.cut(0));
    assert!(right_region.cut(0));
    for (region, knowledge) in [
        (&mut left_region, &mut left),
        (&mut right_region, &mut right),
    ] {
        assert_ne!(
            narrow(
                &fixture.theory,
                &fixture.index,
                region,
                knowledge,
                &mut NarrowingScratch::default()
            )
            .0,
            Narrowing::Refuted
        );
        assert_eq!(region.decision(1), Some(true));
        assert_eq!(region.decision(2), Some(true));
    }
    assert_eq!(left_region, right_region);
    assert_same_choice(&fixture.index, &left_region, &left, &right);
}

#[test]
fn promoted_residual_keeps_transfer_contradictions() {
    let fixture = Fixture::new();
    let mut region = fixture.region.clone();
    assert!(region.cut(5));
    // No single-row forcing establishes the conflict. The complete prefix
    // relation must carry A's lower 3 to C, or C's upper 3 back to A.
    let mut without = fixture.index.clone();
    without.thresholds.relations = None;
    let mut knowledge = fixture.knowledge.clone();
    assert_ne!(
        narrow(
            &fixture.theory,
            &without,
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted
    );
    assert!((0..5).all(|atom| region.is_open(atom)));
    for promoted in [false, true] {
        assert!(fixture.finish(promoted, &mut region.clone(), &mut knowledge.clone()));
    }
}

mod generated;
