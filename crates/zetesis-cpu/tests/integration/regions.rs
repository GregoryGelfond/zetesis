//! The region traversal is the coverage tree of `Search.lean` walked by a
//! stack: a region is narrowed, refuted, decided outright, split on its
//! highest open atom with the cut branch first, or counted when its
//! narrowing decided nothing beyond the split. Every candidate of the root
//! lies in exactly one leaf or counted region visited, and the leaves come
//! in the counter's order.

use std::mem::size_of;

use zetesis_cpu::Stop;
use zetesis_cpu::regions::{Counting, Narrowing, Region, Traversal, Visit};

/// Every visit of a traversal, with a narrowing that never decides anything.
fn visits(mut traversal: Traversal) -> Vec<Visit> {
    let mut visits = Vec::new();
    while let Some(visit) = traversal
        .next(|_, ()| Ok::<_, Stop>(Narrowing::Fixed { changed: false }))
        .unwrap()
    {
        visits.push(visit);
    }
    visits
}

fn held(region: &Region) -> Vec<usize> {
    region.held().collect()
}

#[test]
fn region_retained_bytes_count_both_masks() {
    let empty = Region::all_open(0).retained_bytes();
    let region = Region::all_open(65); // two words per mask
    // Two masks (held and cut), two words each, eight bytes a word.
    assert_eq!(
        region.retained_bytes(),
        empty + 2 * 2 * size_of::<u64>() as u128
    );
}

#[test]
fn decisions_preserve_region_retained_bytes() {
    let mut region = Region::all_open(65);
    let before = region.retained_bytes();
    assert!(region.hold(11));
    assert!(region.cut(64));
    assert_eq!(region.retained_bytes(), before);
}

#[test]
fn open_atoms_match_the_ascending_universe() {
    for atoms in [0, 1, 63, 64, 65, 127, 128, 129] {
        let region = Region::all_open(atoms);
        assert_eq!(
            region.open().collect::<Vec<_>>(),
            (0..atoms).collect::<Vec<_>>()
        );
    }
}

#[test]
fn open_atoms_preserve_sparse_positions() {
    let atoms = 257;
    let expected = [0, 63, 64, 192, 255, 256];
    let mut region = Region::all_open(atoms);
    for atom in (0..atoms).filter(|atom| !expected.contains(atom)) {
        if atom % 2 == 0 {
            assert!(region.hold(atom));
        } else {
            assert!(region.cut(atom));
        }
    }
    assert_eq!(region.open().collect::<Vec<_>>(), expected);
}

#[test]
fn a_region_holds_cuts_or_leaves_each_atom_open() {
    let mut region = Region::all_open(3);
    assert!(region.is_open(0) && region.is_open(1) && region.is_open(2));
    assert!(region.hold(1));
    assert!(region.cut(2));
    assert!(region.is_held(1) && region.is_cut(2) && region.is_open(0));
    assert!(region.hold(1), "holding a held atom is idle");
    assert!(!region.cut(1), "cutting a held atom is refused");
    assert!(!region.hold(3), "an atom outside the region is refused");
    assert_eq!(region.decision(1), Some(true));
    assert_eq!(region.decision(0), None);
    assert_eq!(held(&region), vec![1]);
    assert_eq!(region.open().collect::<Vec<_>>(), vec![0]);
    assert_eq!(region.highest_open(), Some(0));
}

#[test]
fn a_region_reports_its_decisions_with_values_ascending() {
    let mut region = Region::all_open(3);
    assert!(region.hold(2));
    assert!(region.cut(0));
    assert!(region.hold(2), "an idle hold is not a decision");
    assert_eq!(
        region.decided().collect::<Vec<_>>(),
        vec![(0, false), (2, true)]
    );
    let (cut, held) = region.split(1);
    assert_eq!(
        cut.decided().collect::<Vec<_>>(),
        vec![(0, false), (1, false), (2, true)]
    );
    assert_eq!(
        held.decided().collect::<Vec<_>>(),
        vec![(0, false), (1, true), (2, true)]
    );
}

#[test]
fn regions_are_equal_by_their_decisions_whatever_their_order() {
    let mut region = Region::all_open(3);
    assert!(region.hold(2));
    assert!(region.cut(0));
    let mut other = Region::all_open(3);
    assert!(other.cut(0));
    assert!(other.hold(2));
    assert_eq!(other, region);
}

#[test]
fn decision_words_represent_the_exact_region() {
    for atoms in [0_usize, 1, 63, 64, 65, 127, 128, 129] {
        let mut region = Region::all_open(atoms);
        for atom in 0..atoms {
            match atom % 3 {
                0 => assert!(region.hold(atom)),
                1 => assert!(region.cut(atom)),
                _ => {}
            }
        }
        let (held, cut) = region.decision_words();
        assert_eq!(held.len(), atoms.div_ceil(64));
        assert_eq!(cut.len(), held.len());
        for atom in 0..atoms {
            let flag = 1_u64 << (atom % 64);
            assert_eq!(held[atom / 64] & flag != 0, atom % 3 == 0);
            assert_eq!(cut[atom / 64] & flag != 0, atom % 3 == 1);
        }
        assert!(held.iter().zip(cut).all(|(held, cut)| held & cut == 0));
        let tail = atoms % 64;
        if tail != 0 {
            let unused = !((1_u64 << tail) - 1);
            assert_eq!((held[held.len() - 1] | cut[cut.len() - 1]) & unused, 0);
        }
    }
}

#[test]
fn a_split_partitions_a_region_on_one_atom_cut_first() {
    let region = Region::all_open(2);
    let (cut, held) = region.split(1);
    assert!(cut.is_cut(1) && cut.is_open(0));
    assert!(held.is_held(1) && held.is_open(0));
}

#[test]
#[should_panic(expected = "one of its open atoms")]
fn splitting_an_already_decided_atom_fails_fast() {
    // Deciding an already-decided atom would leave a reader's seen-mask out of
    // step with the region, so it fails fast rather than desynchronising silently.
    let mut region = Region::all_open(4);
    assert!(region.hold(2));
    let _ = region.split(2);
}

#[test]
#[should_panic(expected = "one of its open atoms")]
fn splitting_an_out_of_range_atom_fails_fast() {
    // Atom 100 is beyond the 65-atom universe but inside the top word; it must
    // fail fast rather than set a phantom tail bit.
    let region = Region::all_open(65);
    let _ = region.split(100);
}

#[test]
fn tail_bits_beyond_the_atom_count_are_never_open() {
    // 65 atoms need two 64-bit words; the high bits of the second word are not
    // atoms and must never surface as open.
    let mut region = Region::all_open(65);
    for atom in 0..65 {
        assert!(region.hold(atom));
    }
    assert_eq!(region.highest_open(), None);
    assert_eq!(region.split_atom(), None);
    assert_eq!(region.open().count(), 0);
    assert_eq!(region.len(), 65);
}

#[test]
fn iteration_reports_only_real_atoms_in_a_partial_word() {
    let mut region = Region::all_open(63); // a single partial word
    assert!(region.hold(62));
    assert_eq!(region.held().collect::<Vec<_>>(), vec![62]);
    assert_eq!(region.open().count(), 62);
    assert!(region.open().all(|atom| atom < 63));
    assert_eq!(region.highest_open(), Some(61));
}

#[test]
fn decided_since_reports_new_decisions_with_values_ascending() {
    let mut region = Region::all_open(70); // two words
    region.hold(3);
    region.cut(65);
    let mut seen = vec![0u64; 2];
    assert_eq!(
        region.decided_since(&seen).collect::<Vec<_>>(),
        vec![(3, true), (65, false)]
    );
    region.snapshot_decided(&mut seen);
    // Nothing is new after the snapshot.
    assert_eq!(region.decided_since(&seen).count(), 0);
    // A further decision is the only new one.
    region.hold(10);
    assert_eq!(
        region.decided_since(&seen).collect::<Vec<_>>(),
        vec![(10, true)]
    );
}

#[test]
fn decided_since_with_empty_seen_reports_every_decision() {
    let mut region = Region::all_open(65);
    region.cut(64);
    assert_eq!(
        region.decided_since(&[]).collect::<Vec<_>>(),
        vec![(64, false)]
    );
}

#[test]
fn a_short_snapshot_records_only_its_available_words() {
    let mut region = Region::all_open(130);
    assert!(region.hold(0));
    assert!(region.cut(63));
    assert!(region.hold(64));
    assert!(region.cut(129));
    let mut seen = [u64::MAX; 2];
    region.snapshot_decided(&mut seen);
    assert_eq!(seen, [1 | (1 << 63), 1]);
    assert_eq!(
        region.decided_since(&seen).collect::<Vec<_>>(),
        vec![(129, false)]
    );
}

#[test]
fn a_snapshot_leaves_excess_destination_words_unchanged() {
    let mut region = Region::all_open(65);
    assert!(region.hold(64));
    let mut seen = [u64::MAX; 3];
    region.snapshot_decided(&mut seen);
    assert_eq!(seen, [0, 1, u64::MAX]);
}

#[test]
fn an_empty_snapshot_destination_leaves_every_decision_unseen() {
    let mut region = Region::all_open(65);
    assert!(region.cut(64));
    let mut seen = [];
    region.snapshot_decided(&mut seen);
    assert_eq!(
        region.decided_since(&seen).collect::<Vec<_>>(),
        vec![(64, false)]
    );
}

#[test]
fn decided_since_ignores_nonatom_seen_bits() {
    let mut region = Region::all_open(65);
    assert!(region.hold(0));
    assert!(region.cut(64));
    // The partial word's tail and a whole excess word cannot hide either atom.
    let seen = [0, !1, u64::MAX];
    assert_eq!(
        region.decided_since(&seen).collect::<Vec<_>>(),
        vec![(0, true), (64, false)]
    );
}

#[test]
fn a_fully_decided_region_has_no_open_atom_to_split() {
    let mut region = Region::all_open(3);
    assert_eq!(region.highest_open(), Some(2));
    assert!(region.hold(2));
    assert_eq!(region.highest_open(), Some(1));
    assert!(region.hold(1));
    assert!(region.cut(0));
    assert_eq!(region.highest_open(), None);
    assert_eq!(region.split_atom(), None);
}

#[test]
fn a_split_decides_the_atom_both_ways_and_drops_the_preference() {
    let mut region = Region::all_open(4);
    region.prefer(1);
    assert_eq!(region.split_atom(), Some(1));
    let (cut, held) = region.split(3);
    assert!(cut.is_cut(3) && held.is_held(3));
    // Neither child inherits the parent's preference; each falls back to its
    // highest open atom.
    assert_eq!(cut.split_atom(), Some(2));
    assert_eq!(held.split_atom(), Some(2));
}

#[test]
fn never_counting_visits_every_leaf_in_counter_order() {
    let traversal = Traversal::new(Region::all_open(2), Counting::Never);
    let leaves: Vec<Vec<usize>> = visits(traversal)
        .iter()
        .map(|visit| match visit {
            Visit::Leaf(region, ()) => held(region),
            Visit::Counted(..) => panic!("never counted"),
        })
        .collect();
    // Empty, {0}, {1}, {0,1}: atom 0 is the low bit of the counter.
    assert_eq!(leaves, vec![vec![], vec![0], vec![1], vec![0, 1]]);
}

#[test]
fn the_root_is_split_even_when_its_narrowing_changes_nothing() {
    let traversal = Traversal::new(Region::all_open(2), Counting::Unchanged);
    let visits = visits(traversal);
    // The root splits on atom 1; each child, unchanged by narrowing, is counted.
    assert_eq!(visits.len(), 2);
    assert!(matches!(&visits[0], Visit::Counted(region, ()) if region.is_cut(1)));
    assert!(matches!(&visits[1], Visit::Counted(region, ()) if region.is_held(1)));
}

#[test]
fn a_narrowing_that_decides_an_atom_keeps_splitting_below_it() {
    // Narrowing cuts atom 0 whenever atom 2 is held: the held branch then
    // has one open atom left and is split, not counted.
    let mut traversal = Traversal::new(Region::all_open(3), Counting::Unchanged);
    let mut seen = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            if region.is_held(2) && region.is_open(0) {
                assert!(region.cut(0));
                Ok::<_, Stop>(Narrowing::Fixed { changed: true })
            } else {
                Ok(Narrowing::Fixed { changed: false })
            }
        })
        .unwrap()
    {
        seen.push(visit);
    }
    assert_eq!(seen.len(), 3);
    assert!(matches!(&seen[0], Visit::Counted(region, ()) if region.is_cut(2)));
    assert!(matches!(&seen[1], Visit::Leaf(region, ()) if held(region) == vec![2]));
    assert!(matches!(&seen[2], Visit::Leaf(region, ()) if held(region) == vec![1, 2]));
}

#[test]
fn a_refuted_region_is_skipped_with_its_whole_subtree() {
    let mut traversal = Traversal::new(Region::all_open(3), Counting::Never);
    let mut leaves = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            if region.is_held(2) {
                Ok::<_, Stop>(Narrowing::Refuted)
            } else {
                Ok(Narrowing::Fixed { changed: false })
            }
        })
        .unwrap()
    {
        if let Visit::Leaf(region, ()) = visit {
            leaves.push(held(&region));
        }
    }
    assert_eq!(leaves, vec![vec![], vec![0], vec![1], vec![0, 1]]);
    let statistics = traversal.statistics();
    assert_eq!(statistics.refuted, 1);
    assert_eq!(statistics.leaves, 4);
    assert_eq!(statistics.counted, 0);
    // The root, its two children, and the cut child's subtree of six.
    assert_eq!(statistics.regions, 1 + 2 + 6);
}

#[test]
fn a_decided_root_is_one_leaf() {
    let mut root = Region::all_open(2);
    assert!(root.hold(0) && root.cut(1));
    let visits = visits(Traversal::new(root, Counting::Unchanged));
    assert_eq!(visits.len(), 1);
    assert!(matches!(&visits[0], Visit::Leaf(region, ()) if held(region) == vec![0]));
}

#[test]
fn a_stopped_narrowing_stops_the_traversal_and_keeps_the_region() {
    let mut traversal = Traversal::new(Region::all_open(2), Counting::Never);
    assert_eq!(
        traversal.next(|_, ()| Err(Stop::WorkLimit)).unwrap_err(),
        Stop::WorkLimit
    );
    // The region whose narrowing stopped is visited again on the next call.
    let visits = visits(traversal);
    assert_eq!(visits.len(), 4);
}

#[test]
fn a_narrowing_may_choose_the_split_atom() {
    // Preferring atom 0 at every region visits the leaves with atom 0 as
    // the high bit of the order: empty, {1}, {0}, {0,1}.
    let mut traversal = Traversal::new(Region::all_open(2), Counting::Never);
    let mut leaves = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            region.prefer(0);
            Ok::<_, Stop>(Narrowing::Fixed { changed: false })
        })
        .unwrap()
    {
        if let Visit::Leaf(region, ()) = visit {
            leaves.push(held(&region));
        }
    }
    assert_eq!(leaves, vec![vec![], vec![1], vec![0], vec![0, 1]]);
    let mut region = Region::all_open(2);
    region.prefer(1);
    assert!(region.hold(1));
    assert_eq!(region.split_atom(), Some(0), "a decided preference lapses");
}

#[test]
fn each_leaf_carries_the_state_of_its_own_lineage() {
    // A split clones the state into both children. The state counts the
    // regions on its own lineage: each narrowing adds one, so a leaf at
    // depth two has seen three regions, and siblings do not see each
    // other's count.
    let mut traversal = Traversal::with_state(Region::all_open(2), Counting::Never, 0usize);
    let mut depths = Vec::new();
    while let Some(visit) = traversal
        .next(|_, seen: &mut usize| {
            *seen += 1;
            Ok::<_, Stop>(Narrowing::Fixed { changed: false })
        })
        .unwrap()
    {
        if let Visit::Leaf(_, seen) = visit {
            depths.push(seen);
        }
    }
    assert_eq!(depths, vec![3, 3, 3, 3]);
}

#[test]
fn a_traversal_from_a_narrowed_root_does_not_narrow_it_again() {
    // The caller narrowed the root before the traversal began: the root is
    // split without a narrowing, and every region below it is narrowed
    // once, the counting never counting the root as a flat interval.
    let mut traversal = Traversal::with_narrowed_root(Region::all_open(2), Counting::Unchanged, ());
    let mut narrowed = Vec::new();
    while let Some(visit) = traversal
        .next(|region, ()| {
            narrowed.push(region.decided().collect::<Vec<_>>());
            Ok::<_, Stop>(Narrowing::Fixed { changed: false })
        })
        .unwrap()
    {
        assert!(matches!(visit, Visit::Counted(..)));
    }
    // The root's two children, cut then held, each narrowed once and counted.
    assert_eq!(narrowed, vec![vec![(1, false)], vec![(1, true)]]);
    assert_eq!(traversal.statistics().regions, 3);
    assert_eq!(traversal.statistics().counted, 2);
}
