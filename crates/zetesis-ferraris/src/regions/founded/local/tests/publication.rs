//! Whole publication supplies a new baseline; later false events remain required.

use super::*;

/// The fixture has exactly six cyclic heads. Publish precisely those missing
/// from the complete result, changing only their atom and occurrence masks.
fn publish(example: &Example, scratch: &Scratch, never: &mut [u64], cuts: &mut [u64]) {
    assert_eq!(scratch.cyclic_atoms[0] & 63, 63);
    for atom in 0..6 {
        if !bit(&scratch.atoms, atom) {
            cuts[0] |= 1 << atom;
            for &node in &example.index.atom_nodes[atom] {
                never[node / 64] |= 1 << (node % 64);
            }
        }
    }
}

#[test]
fn direct_publication_preserves_the_complete_result() {
    let example = Example::new(false);
    let mut scratch = example.prepared();
    for false_seeds in 0..8 {
        for cut_atoms in 0..64 {
            let (mut never, mut cuts) = example.masks(false_seeds, cut_atoms);
            example
                .complete(&mut scratch, &never, &cuts, &mut Meter::local(u64::MAX))
                .unwrap();
            let completed = scratch.atoms.clone();
            publish(&example, &scratch, &mut never, &mut cuts);
            // No replay authority is requested: this is an independent full
            // pass after the exact direct atom-occurrence publication.
            example
                .complete(&mut scratch, &never, &cuts, &mut Meter::local(u64::MAX))
                .unwrap();
            assert_eq!(scratch.local.passes, 0);
            assert_eq!(
                scratch.atoms, completed,
                "seeds={false_seeds} cuts={cut_atoms}"
            );
            assert_eq!(scratch.cyclic_atoms[0] & !cuts[0] & !scratch.atoms[0], 0);
        }
    }
}

fn published_then_invalidated(example: &Example) -> (Scratch, Vec<u64>, Vec<u64>) {
    let (mut never, mut cuts) = example.masks(1, 0);
    let mut scratch = example.prepared();
    example
        .complete(&mut scratch, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(scratch.atoms[0] & 63, 60);
    publish(example, &scratch, &mut never, &mut cuts);
    scratch.begin_after_publication(&example.owner);
    assert!(scratch.local.collecting);
    let seed = example.seeds[1];
    never[seed / 64] |= 1 << (seed % 64);
    scratch.record_false(seed);
    (scratch, never, cuts)
}

#[test]
fn postpublication_replay_matches_a_fresh_full_pass() {
    let example = Example::new(false);
    let (mut actual, never, cuts) = published_then_invalidated(&example);
    let mut expected = example.prepared();
    example
        .complete(&mut expected, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(expected.local.passes, 0);
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms, expected.atoms);
    assert_eq!(actual.atoms[0] & 63, 48);
}

#[test]
fn stopped_postpublication_replay_requires_full_retry() {
    let example = Example::new(false);
    let (mut measured, never, cuts) = published_then_invalidated(&example);
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
            let (mut actual, never, cuts) = published_then_invalidated(&example);
            let mut allowance = Allowance {
                remaining: ceiling,
                failure,
            };
            let mut work = Meter::reserved(&mut allowance);
            assert_eq!(
                example.complete(&mut actual, &never, &cuts, &mut work),
                Err(failure)
            );
            assert!(!actual.local.collecting);
            assert_eq!(actual.local.passes, 0);
            work.refund();
            example
                .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
                .unwrap();
            assert_eq!(actual.local.passes, 0);
            assert_eq!(actual.atoms, measured.atoms);
        }
    }
}

#[test]
fn partial_publication_does_not_rearm_replay() {
    use crate::{Knowledge, Narrowing, NarrowingScratch, OriginalSubject, Region};
    fn publication_flags(known: &mut Knowledge) -> (bool, bool, bool) {
        fn flags<C>(known: &mut crate::regions::Known<C>) -> (bool, bool, bool) {
            let view = known.borrow();
            (
                view.atom_never.contains(0),
                view.atom_never.contains(1),
                *view.founded_closed,
            )
        }
        match &mut known.width {
            crate::regions::Width::Compact16(inner) => flags(inner),
            crate::regions::Width::Compact32(inner) => flags(inner),
            crate::regions::Width::Native(inner) => flags(inner),
        }
    }
    let example = Example::new(false);
    let learning = crate::test_support::learning(&example.owner);
    let close =
        |region: &mut Region, known: &mut Knowledge, scratch: &mut NarrowingScratch, ceiling| {
            example.index.narrow_known(
                OriginalSubject::carrier(&learning, Some(&example.producers)),
                region,
                known,
                scratch,
                RegionLimits { max_work: ceiling },
                &Cancellation::default(),
            )
        };
    let mut parent = Region::all_open(example.owner.atom_count());
    let mut inherited = example.index.knowledge();
    assert!(matches!(
        close(
            &mut parent,
            &mut inherited,
            &mut NarrowingScratch::default(),
            u64::MAX
        )
        .result
        .unwrap(),
        Narrowing::Fixed { .. }
    ));
    let child = || {
        let mut region = parent.clone();
        assert!(region.hold(6) && region.hold(7));
        region
    };
    let prepared = || NarrowingScratch {
        common: crate::regions::CommonScratch {
            founded: example.prepared(),
            ..crate::regions::CommonScratch::default()
        },
        ..NarrowingScratch::default()
    };
    let mut expected = child();
    let completed = close(
        &mut expected,
        &mut inherited.clone(),
        &mut prepared(),
        u64::MAX,
    );
    assert!(matches!(completed.result.unwrap(), Narrowing::Fixed { .. }));
    let mut prefixes = 0;
    for ceiling in 0..completed.statistics.work {
        let mut actual = child();
        let mut known = inherited.clone();
        let mut scratch = prepared();
        let stopped = close(&mut actual, &mut known, &mut scratch, ceiling);
        assert_eq!(stopped.result, Err(Stop::WorkLimit));
        let (first_cut, second_cut, closed) = publication_flags(&mut known);
        if first_cut && !second_cut {
            prefixes += 1;
            assert!(!closed);
            assert!(!scratch.common.founded.local.collecting);
            // Retry from the actual inherited parent with this retained worker
            // scratch; the partial publication itself contributes no authority.
            let mut retry = child();
            assert!(matches!(
                close(&mut retry, &mut inherited.clone(), &mut scratch, u64::MAX)
                    .result
                    .unwrap(),
                Narrowing::Fixed { .. }
            ));
            assert_eq!(retry, expected);
        }
    }
    assert!(
        prefixes > 0,
        "a charged boundary must stop inside cut publication"
    );
}

/// Holding control 6 disables the first cycle's seed. Its later published cut
/// of atom 0 makes the opaque body (6 -> 0) false, invalidating the second cycle.
/// A third cycle remains supported, so both invalidations have a local boundary.
fn cascading_example() -> Example {
    let mut nodes: Vec<_> = (0..8).map(Node::atom).collect();
    nodes.push(Node::falsum());
    nodes.push(Node::implies(6, 8));
    nodes.push(Node::implies(6, 0));
    nodes.push(Node::implies(7, 8));
    let mut roots = Vec::new();
    for (control, negative) in [(6, 9), (7, 11)] {
        roots.push(nodes.len());
        nodes.push(Node::or_pair([control, negative]));
    }
    for (body, head) in [
        (0, 1),
        (1, 0),
        (2, 3),
        (3, 2),
        (4, 5),
        (5, 4),
        (9, 0),
        (10, 2),
        (11, 4),
    ] {
        roots.push(nodes.len());
        nodes.push(Node::implies(body, head));
    }
    let owner = theory(8, nodes, roots);
    let index = Narrower::new(&owner);
    let producers = crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    Example {
        owner,
        index,
        producers,
        seeds: [9, 10, 11],
    }
}

#[test]
fn opaque_events_after_publication_preserve_completions() {
    use crate::{Narrowing, NarrowingScratch, OriginalSubject, Region};
    let example = cascading_example();
    let learning = crate::test_support::learning(&example.owner);
    let close =
        |region: &mut Region, known: &mut crate::Knowledge, scratch: &mut NarrowingScratch| {
            example.index.narrow_known(
                OriginalSubject::carrier(&learning, Some(&example.producers)),
                region,
                known,
                scratch,
                RegionLimits::default(),
                &Cancellation::default(),
            )
        };
    let mut endpoints = Vec::new();
    for force_full in [false, true] {
        let mut region = Region::all_open(example.owner.atom_count());
        let mut known = example.index.knowledge();
        let mut scratch = NarrowingScratch::default();
        assert!(matches!(
            close(&mut region, &mut known, &mut scratch).result.unwrap(),
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
        scratch.common.founded.local.force_full = force_full;
        assert!(region.hold(6));
        let before = scratch.common.founded.local.passes;
        let result = close(&mut region, &mut known, &mut scratch);
        assert!(matches!(result.result.unwrap(), Narrowing::Fixed { .. }));
        // Two changed local passes, separated by real ordinary propagation,
        // and one unchanged confirmation. The independent control uses none.
        assert_eq!(
            scratch.common.founded.local.passes - before,
            if force_full { 0 } else { 3 }
        );
        assert!((0..4).all(|atom| region.is_cut(atom)));
        assert!(region.is_open(4) && region.is_open(5));
        endpoints.push(region);
    }
    assert_eq!(endpoints[0], endpoints[1]);
    // Exhaust the entire authored eight-atom universe with the maintained
    // reduct reference kernel, retaining the complete family below hold(6).
    // This checks membership separately from either founded replay route.
    let answers: Vec<_> = (0..1 << example.owner.atom_count())
        .filter(|mask| mask & (1 << 6) != 0)
        .filter(|&mask| {
            crate::check(
                &example.owner,
                &crate::test_support::world(&example.owner, mask),
                crate::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        })
        .collect();
    assert!(!answers.is_empty());
    for region in &endpoints {
        let retained: Vec<_> = answers
            .iter()
            .copied()
            .filter(|mask| {
                (0..example.owner.atom_count()).all(|atom| {
                    region
                        .decision(atom)
                        .is_none_or(|held| held == (mask & (1 << atom) != 0))
                })
            })
            .collect();
        assert_eq!(retained, answers);
    }
}
