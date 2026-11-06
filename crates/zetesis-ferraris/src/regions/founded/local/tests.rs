//! Local replay matches fresh full closure, including every stopped prefix.

use super::*;

mod barriers;
mod coverage;
mod false_bodies;
mod heads;
mod publication;
use crate::test_support::{Allowance, theory};
use crate::{Node, RegionLimits};
use zetesis_algebra::Cancellation;

struct Example {
    owner: Theory,
    index: Narrower,
    producers: Producers,
    seeds: [usize; 3],
}

impl Example {
    /// Three independently seeded cycles; the first also reaches the second.
    /// Boolean bodies exercise shared projected occurrence replay.
    fn new(alternative: bool) -> Self {
        let mut nodes: Vec<_> = (0..9).map(Node::atom).collect();
        nodes.push(Node::falsum());
        let seeds = [10, 11, 12];
        for control in 6..9 {
            nodes.push(Node::implies(control, 9));
        }
        let mut roots = Vec::new();
        for (control, seed) in (6..9).zip(seeds) {
            roots.push(nodes.len());
            nodes.push(Node::or_pair([control, seed]));
        }
        for (body, head) in [
            (0, 1),
            (1, 0),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 4),
            (10, 0),
            (11, 2),
            (12, 4),
            (1, 2),
        ] {
            roots.push(nodes.len());
            nodes.push(Node::implies(body, head));
        }
        let conjunction = nodes.len();
        nodes.push(Node::and_pair([1, 10]));
        roots.push(nodes.len());
        nodes.push(Node::implies(conjunction, 2));
        let disjunction = nodes.len();
        nodes.push(Node::or_pair([0, 11]));
        roots.push(nodes.len());
        nodes.push(Node::implies(disjunction, 2));
        if alternative {
            roots.push(nodes.len());
            nodes.push(Node::implies(11, 0));
        }
        let owner = theory(9, nodes, roots);
        let index = Narrower::new(&owner);
        let producers = crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
            .unwrap()
            .producers
            .unwrap();
        Self {
            owner,
            index,
            producers,
            seeds,
        }
    }

    fn masks(&self, false_seeds: usize, cut_atoms: usize) -> (Vec<u64>, Vec<u64>) {
        let mut never = vec![0; flag_words(self.owner.nodes().len())];
        for (position, &seed) in self.seeds.iter().enumerate() {
            if false_seeds & (1 << position) != 0 {
                never[seed / 64] |= 1 << (seed % 64);
            }
        }
        for atom in 0..6 {
            if cut_atoms & (1 << atom) != 0 {
                for &node in &self.index.atom_nodes[atom] {
                    never[node / 64] |= 1 << (node % 64);
                }
            }
        }
        (never, vec![cut_atoms as u64])
    }

    fn complete<Q: zetesis_algebra::Quota<Error = Stop> + ?Sized>(
        &self,
        scratch: &mut Scratch,
        never: &[u64],
        cuts: &[u64],
        work: &mut Meter<'_, Q>,
    ) -> Result<(), Stop> {
        scratch
            .complete(&self.owner, &self.index, &self.producers, never, cuts, work)
            .map_err(zetesis_algebra::Failure::into_stop)
    }

    fn prepared(&self) -> Scratch {
        let mut scratch = Scratch::default();
        for _ in 0..2 {
            scratch
                .prepare_bodies(
                    &self.owner,
                    &self.index,
                    &self.producers,
                    &mut Meter::local(u64::MAX),
                )
                .map_err(zetesis_algebra::Failure::into_stop)
                .unwrap();
        }
        let (never, cuts) = self.masks(0, 0);
        self.complete(&mut scratch, &never, &cuts, &mut Meter::local(u64::MAX))
            .unwrap();
        assert_eq!(scratch.atoms[0] & 63, 63);
        scratch
    }

    fn start(&self, scratch: &mut Scratch, never: &[u64]) {
        scratch
            .begin_local(&self.owner, true, &mut Meter::local(u64::MAX))
            .unwrap();
        assert!(scratch.local.collecting);
        for node in 0..self.owner.nodes().len() {
            if bit(never, node) {
                scratch.record_false(node);
            }
        }
    }
}

#[test]
fn all_tiny_cuts_match_full_closure() {
    let example = Example::new(false);
    let mut actual = example.prepared();
    let mut expected = example.prepared();
    // Each case inherits the all-supported parent, while the worker retains
    // arbitrary previous sibling states. No ancestor body truth is restored.
    for false_seeds in 0..8 {
        for cut_atoms in 0..64 {
            let (never, cuts) = example.masks(false_seeds, cut_atoms);
            example
                .complete(&mut expected, &never, &cuts, &mut Meter::local(u64::MAX))
                .unwrap();
            example.start(&mut actual, &never);
            example
                .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
                .unwrap();
            assert_eq!(
                actual.atoms, expected.atoms,
                "seeds={false_seeds} cuts={cut_atoms}"
            );
            assert!(actual.local.touched.is_empty());
        }
    }
    assert!(actual.local.passes > 0);
    assert!(actual.local.passes < 512);
}

#[test]
fn unchanged_producer_support_is_recomputed() {
    let example = Example::new(true);
    let (never, cuts) = example.masks(1, 0);
    let mut actual = example.prepared();
    // Poison every mutable state, preserving only immutable selection. A
    // different sibling may have left precisely these inactive cells.
    for state in &mut actual.states {
        if !matches!(state, State::Outside) {
            *state = State::Inactive;
        }
    }
    example.start(&mut actual, &never);
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms[0] & 63, 63);
}

#[test]
fn a_false_seed_reaches_downstream_components() {
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    let mut actual = example.prepared();
    example.start(&mut actual, &never);
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms[0] & 63, 48);
}

#[test]
fn exact_owner_is_checked_after_history_collection() {
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    let mut actual = example.prepared();
    example.start(&mut actual, &never);
    // Cycle preparation or owner replacement can revoke authority after entry.
    actual.owner = None;
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 0);
    assert_eq!(actual.atoms[0] & 63, 48);
    let foreign = Example::new(false);
    actual
        .begin_local(&foreign.owner, true, &mut Meter::local(u64::MAX))
        .unwrap();
    assert!(!actual.local.collecting);
}

#[test]
fn bare_completion_does_not_rearm_local_replay() {
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    let mut actual = example.prepared();
    actual
        .begin_local(&example.owner, false, &mut Meter::local(u64::MAX))
        .unwrap();
    assert!(!actual.local.collecting);
    example.start(&mut actual, &never);
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms[0] & 63, 48);
}

#[test]
fn every_stopped_prefix_requires_full_retry() {
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    let mut expected = example.prepared();
    example
        .complete(&mut expected, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    let mut measured = example.prepared();
    example.start(&mut measured, &never);
    let mut completed = Meter::local(u64::MAX);
    example
        .complete(&mut measured, &never, &cuts, &mut completed)
        .unwrap();
    assert_eq!(measured.local.passes, 1);
    for failure in [
        Stop::WorkLimit,
        Stop::Cancelled,
        Stop::Deadline,
        Stop::Allocation,
    ] {
        for ceiling in 0..completed.charged() {
            let mut actual = example.prepared();
            example.start(&mut actual, &never);
            let mut allowance = Allowance {
                remaining: ceiling,
                failure,
            };
            let mut work = Meter::reserved(&mut allowance);
            assert_eq!(
                example.complete(&mut actual, &never, &cuts, &mut work),
                Err(failure)
            );
            assert_eq!(actual.local.passes, 0);
            assert!(!actual.local.collecting);
            work.refund();
            example
                .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
                .unwrap();
            assert_eq!(actual.local.passes, 0);
            assert_eq!(actual.atoms, expected.atoms);
            assert!(actual.local.touched.is_empty());
        }
    }
}

#[test]
fn membership_size_overflow_is_typed() {
    assert_eq!(
        Replay::default()
            .prepare(usize::MAX, 1, 0, &[], &mut Meter::local(u64::MAX))
            .map_err(zetesis_algebra::Failure::into_stop),
        Err(Stop::Allocation)
    );
}

#[test]
fn capture_reads_the_complete_current_atom_result() {
    use super::super::explanations::{Input, Limits, collect};
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    let mut actual = example.prepared();
    example.start(&mut actual, &never);
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    let atom_sure = [1 << 6 | 1 << 7];
    let input = Input {
        theory: &example.owner,
        index: &example.index,
        producers: &example.producers,
        completed: &actual,
        atom_sure: atom_sure.as_slice().into(),
        atom_never: cuts.as_slice().into(),
    };
    let limits = Limits {
        atoms: 32,
        literals: 32,
        visits: 100_000,
        bytes: 100_000,
    };
    let reason = collect(input, 0, limits, &mut Meter::local(u64::MAX), &mut 0)
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap()
        .unwrap();
    assert_eq!(reason.unsupported, vec![0, 1]);
    assert_eq!(reason.conditions, vec![(6, true)]);
    assert!(
        collect(input, 4, limits, &mut Meter::local(u64::MAX), &mut 0)
            .map_err(zetesis_algebra::Failure::into_stop)
            .unwrap()
            .is_none()
    );
}

#[test]
fn original_false_events_collect_the_whole_interval() {
    use crate::{Narrowing, NarrowingScratch, OriginalSubject, Region};
    let example = Example::new(false);
    let mut parent = Region::all_open(example.owner.atom_count());
    let mut known = example.index.knowledge();
    let mut scratch = NarrowingScratch::default();
    let learning = crate::test_support::learning(&example.owner);
    let close =
        |region: &mut Region, known: &mut crate::Knowledge, scratch: &mut NarrowingScratch| {
            example
                .index
                .narrow_known(
                    OriginalSubject::carrier(&learning, Some(&example.producers)),
                    region,
                    known,
                    scratch,
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap()
        };
    assert!(matches!(
        close(&mut parent, &mut known, &mut scratch),
        Narrowing::Fixed { .. }
    ));
    scratch
        .common
        .founded
        .prepare_bodies(
            &example.owner,
            &example.index,
            &example.producers,
            &mut Meter::local(u64::MAX),
        )
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    for controls in [[6, 7], [7, 6]] {
        let mut child = parent.clone();
        let mut inherited = known.clone();
        for control in controls {
            assert!(child.hold(control));
        }
        let before = scratch.common.founded.local.passes;
        assert!(matches!(
            close(&mut child, &mut inherited, &mut scratch),
            Narrowing::Fixed { .. }
        ));
        // One changed replay publishes the cuts, then one local confirmation
        // consumes the complete publication baseline with no further false input.
        assert_eq!(scratch.common.founded.local.passes, before + 2);
        assert!((0..4).all(|atom| child.is_cut(atom)));
        assert!(child.is_open(4) && child.is_open(5));
    }
}

#[test]
fn a_refuted_siblings_marks_are_reset_before_inherited_collection() {
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    let mut expected = example.prepared();
    example
        .complete(&mut expected, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    let mut actual = example.prepared();
    // A sibling records its false inputs, then is refuted before its pass.
    let (sibling, _) = example.masks(4, 0);
    example.start(&mut actual, &sibling);
    assert!(!actual.local.touched.is_empty());
    actual
        .begin_local(&example.owner, true, &mut Meter::local(u64::MAX))
        .unwrap();
    assert!(actual.local.collecting);
    assert!(actual.local.touched.is_empty());
    assert!(actual.local.marks.iter().all(|&mark| mark == 0));
    for node in 0..example.owner.nodes().len() {
        if bit(&never, node) {
            actual.record_false(node);
        }
    }
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms, expected.atoms);
    assert_eq!(actual.atoms[0] & 63, 48);
}

#[test]
fn resetting_stale_marks_charges_one_operation_per_mark() {
    let example = Example::new(false);
    let mut actual = example.prepared();
    let (sibling, _) = example.masks(7, 63);
    example.start(&mut actual, &sibling);
    let stale = actual.local.touched.len();
    assert!(stale > 0);
    let mut reset = Meter::local(u64::MAX);
    actual
        .begin_local(&example.owner, true, &mut reset)
        .unwrap();
    assert_eq!(reset.charged(), stale as u64);
    let mut clean = Meter::local(u64::MAX);
    actual
        .begin_local(&example.owner, true, &mut clean)
        .unwrap();
    assert!(actual.local.collecting);
    assert_eq!(clean.charged(), 0);
}

#[test]
fn a_refused_reset_keeps_every_remaining_mark_and_grants_no_authority() {
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    let mut expected = example.prepared();
    example
        .complete(&mut expected, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    let (sibling, _) = example.masks(7, 63);
    for failure in [
        Stop::WorkLimit,
        Stop::Cancelled,
        Stop::Deadline,
        Stop::Allocation,
    ] {
        let mut stale = None;
        for ceiling in 0.. {
            let mut actual = example.prepared();
            example.start(&mut actual, &sibling);
            let recorded = actual.local.touched.len();
            assert_eq!(*stale.get_or_insert(recorded), recorded);
            if ceiling == recorded {
                break;
            }
            let mut allowance = Allowance {
                remaining: u64::try_from(ceiling).unwrap(),
                failure,
            };
            let mut work = Meter::reserved(&mut allowance);
            assert_eq!(
                actual
                    .begin_local(&example.owner, true, &mut work)
                    .map_err(zetesis_algebra::Failure::into_stop),
                Err(failure)
            );
            work.refund();
            assert!(!actual.local.collecting);
            assert_eq!(actual.local.touched.len(), recorded - ceiling);
            assert!(
                actual
                    .local
                    .touched
                    .iter()
                    .all(|&vertex| actual.local.marks[vertex] != 0)
            );
            assert_eq!(
                actual.local.marks.iter().filter(|&&mark| mark != 0).count(),
                actual.local.touched.len()
            );
            // A later reset continues from the remaining marks.
            actual
                .begin_local(&example.owner, true, &mut Meter::local(u64::MAX))
                .unwrap();
            assert!(actual.local.collecting && actual.local.touched.is_empty());
            for node in 0..example.owner.nodes().len() {
                if bit(&never, node) {
                    actual.record_false(node);
                }
            }
            example
                .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
                .unwrap();
            assert_eq!(actual.local.passes, 1);
            assert_eq!(actual.atoms, expected.atoms);
        }
    }
}

#[test]
fn a_refuted_sibling_does_not_force_the_next_full_pass() {
    use crate::{Narrowing, NarrowingScratch, OriginalSubject, Region};
    let example = Example::new(false);
    let learning = crate::test_support::learning(&example.owner);
    let close =
        |region: &mut Region, known: &mut crate::Knowledge, scratch: &mut NarrowingScratch| {
            example
                .index
                .narrow_known(
                    OriginalSubject::carrier(&learning, Some(&example.producers)),
                    region,
                    known,
                    scratch,
                    RegionLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap()
        };
    let mut parent = Region::all_open(example.owner.atom_count());
    let mut known = example.index.knowledge();
    let mut scratch = NarrowingScratch::default();
    assert!(matches!(
        close(&mut parent, &mut known, &mut scratch),
        Narrowing::Fixed { .. }
    ));
    scratch
        .common
        .founded
        .prepare_bodies(
            &example.owner,
            &example.index,
            &example.producers,
            &mut Meter::local(u64::MAX),
        )
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    // Cutting atom 0 and holding atom 1 refutes the first child through the
    // rule 1 -> 0 during ordinary propagation, after atom 0's false
    // occurrences were recorded and before any founded pass consumed them.
    let mut refuted = parent.clone();
    assert!(refuted.cut(0) && refuted.hold(1));
    assert!(matches!(
        close(&mut refuted, &mut known.clone(), &mut scratch),
        Narrowing::Refuted
    ));
    assert!(!scratch.common.founded.local.touched.is_empty());
    // Its sibling inherits the same completed parent and replays locally: one
    // changed replay publishes the cuts, then one local confirmation.
    let mut sibling = parent.clone();
    assert!(sibling.hold(6));
    let local = scratch.common.founded.local.passes;
    let passes = scratch.common.founded.passes;
    assert!(matches!(
        close(&mut sibling, &mut known.clone(), &mut scratch),
        Narrowing::Fixed { .. }
    ));
    assert_eq!(scratch.common.founded.passes - passes, 2);
    assert_eq!(scratch.common.founded.local.passes - local, 2);
    assert!(sibling.is_cut(0) && sibling.is_cut(1));
    assert!(sibling.is_open(4) && sibling.is_open(5));
}
