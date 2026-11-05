use super::*;

fn prepared(region: &Region) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let mut work = Meter::local(u64::MAX);
    snapshot.prepare(region.mask_words(), &mut work).unwrap();
    snapshot
}

fn saved(region: &Region) -> Snapshot {
    let mut snapshot = prepared(region);
    snapshot
        .publish(region, &mut Meter::local(u64::MAX))
        .unwrap();
    snapshot
}

#[test]
fn a_snapshot_preserves_both_polarities_across_a_word_boundary() {
    let mut region = Region::all_open(65);
    assert!(region.hold(63));
    assert!(region.cut(64));
    let snapshot = saved(&region);
    assert_eq!(snapshot.old_masks(), (&[1 << 63, 0][..], &[0, 1][..]));
    assert!(snapshot.initialized());
}

#[test]
fn ancestry_accepts_only_preserved_decision_polarities() {
    let mut parent = Region::all_open(65);
    assert!(parent.hold(0));
    assert!(parent.cut(64));
    let snapshot = saved(&parent);
    let mut child = parent.clone();
    assert!(child.hold(63));
    assert_eq!(
        snapshot.is_ancestor(&child, &mut Meter::local(u64::MAX)),
        Ok(true)
    );
    let mut reversed = Region::all_open(65);
    assert!(reversed.cut(0));
    assert!(reversed.cut(64));
    assert_eq!(
        snapshot.is_ancestor(&reversed, &mut Meter::local(u64::MAX)),
        Ok(false)
    );
    assert_eq!(
        snapshot.is_ancestor(&Region::all_open(65), &mut Meter::local(u64::MAX)),
        Ok(false)
    );
}

#[test]
fn a_fresh_snapshot_still_checks_the_declared_atom_universe() {
    let snapshot = Snapshot::default();
    let mut work = Meter::local(u64::MAX);
    // Both use one physical word; comparing word lengths alone is insufficient.
    assert_eq!(
        snapshot.validate(&Region::all_open(2), 1, &mut work),
        Err(Failure::Stopped(Stop::InvalidProgram))
    );
    assert_eq!(work.charged(), 1);
}

#[test]
fn detected_bad_lineage_is_a_stop_not_a_refutation() {
    let mut parent = Region::all_open(1);
    assert!(parent.hold(0));
    let snapshot = saved(&parent);
    assert_eq!(
        snapshot.validate(&Region::all_open(1), 1, &mut Meter::local(u64::MAX)),
        Err(Failure::Stopped(Stop::InvalidProgram))
    );
}

#[test]
fn publication_is_complete_only_after_the_final_flag_admission() {
    let mut region = Region::all_open(65);
    assert!(region.hold(64));
    assert!(region.cut(0));
    for limit in 0..=5 {
        let mut snapshot = prepared(&region);
        let mut work = Meter::local(limit);
        let result = snapshot.publish(&region, &mut work);
        assert_eq!(work.charged(), limit);
        assert_eq!(snapshot.initialized(), limit == 5);
        assert_eq!(
            result,
            if limit == 5 {
                Ok(())
            } else {
                Err(Failure::Stopped(Stop::WorkLimit))
            }
        );
        assert_eq!(region.decision(64), Some(true));
        assert_eq!(region.decision(0), Some(false));
    }
}

#[test]
fn the_empty_universe_publishes_without_a_mask_allocation() {
    let region = Region::all_open(0);
    let mut snapshot = prepared(&region);
    let mut work = Meter::local(1);
    snapshot.publish(&region, &mut work).unwrap();
    assert!(snapshot.initialized());
    assert_eq!(snapshot.old_masks(), (&[][..], &[][..]));
    assert_eq!(snapshot.allocated_bytes(), 0);
    assert_eq!(work.charged(), 1);
}

#[test]
fn first_inspection_selects_every_row_in_ordinal_order() {
    let region = Region::all_open(0);
    let index = Index {
        words: Words::default(),
        row_words: 4,
        rows: 129,
        insertions: 0,
    };
    let mut snapshot = Snapshot::default();
    let mut scratch = ArithmeticScratch::default();
    snapshot
        .select(&region, &index, &mut scratch, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(&*scratch.touched, &[u64::MAX, u64::MAX, 1]);
}

#[test]
fn a_short_logical_column_keeps_the_padded_index_stride() {
    let mut region = Region::all_open(4);
    let index = Index {
        words: Words::new(vec![1, 0, 3, 0, 6, 0, 4, 0]),
        row_words: 2,
        rows: 3,
        insertions: 6,
    };
    let mut snapshot = saved(&region);
    let mut scratch = ArithmeticScratch::default();
    assert!(region.hold(1));
    snapshot
        .select(&region, &index, &mut scratch, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(&*scratch.touched, &[3]); // atom 1 participates in rows 0 and 1
    snapshot
        .publish(&region, &mut Meter::local(u64::MAX))
        .unwrap();
    assert!(region.cut(2));
    snapshot
        .select(&region, &index, &mut scratch, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(&*scratch.touched, &[6]); // atom 2 participates in rows 1 and 2
}

#[test]
fn a_three_word_column_uses_its_four_word_physical_stride() {
    let mut region = Region::all_open(2);
    // This is an index-layout fixture, not a claim of parity rank 129 on two atoms.
    let index = Index {
        words: Words::new(vec![1, 0, 0, 0, 0, 0, 1, 0]),
        row_words: 4,
        rows: 129,
        insertions: 2,
    };
    let mut snapshot = saved(&region);
    let mut scratch = ArithmeticScratch::default();
    assert!(region.hold(1));
    snapshot
        .select(&region, &index, &mut scratch, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(&*scratch.touched, &[0, 0, 1]);
}

#[test]
fn saving_the_input_leaves_batch_decisions_as_next_pass_triggers() {
    let mut region = Region::all_open(2);
    let index = Index {
        words: Words::new(vec![1, 0, 2, 0]),
        row_words: 2,
        rows: 2,
        insertions: 2,
    };
    let mut snapshot = saved(&region);
    // A caller publishes the inspected all-open input, then applies a consequence.
    assert!(region.hold(1));
    let mut scratch = ArithmeticScratch::default();
    snapshot
        .select(&region, &index, &mut scratch, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(&*scratch.touched, &[2]);
    snapshot
        .publish(&region, &mut Meter::local(u64::MAX))
        .unwrap();
    snapshot
        .select(&region, &index, &mut scratch, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(&*scratch.touched, &[0]);
}

#[test]
fn metered_snapshot_copy_reuses_capacity_without_aliasing() {
    let mut region = Region::all_open(1);
    assert!(region.hold(0));
    let source = saved(&region);
    let mut target = prepared(&region);
    let pointer = target.words.as_ptr();
    let capacity = target.words.capacity();
    let mut work = Meter::local(5);
    target.copy_from(&source, &mut work).unwrap();
    assert_eq!(target.words.as_ptr(), pointer);
    assert_eq!(target.words.capacity(), capacity);
    assert_ne!(target.words.as_ptr(), source.words.as_ptr());
    assert_eq!(target.old_masks(), source.old_masks());
    assert_eq!(work.charged(), 5);
    target.words[0] = 0;
    assert_eq!(source.old_masks().0, &[1]);
}

#[test]
fn refused_selection_preserves_knowledge_and_reusable_scratch() {
    let parent = Region::all_open(2);
    let mut child = parent.clone();
    assert!(child.cut(1));
    let index = Index {
        words: Words::new(vec![1, 0, 2, 0]),
        row_words: 2,
        rows: 2,
        insertions: 2,
    };
    let mut reference = saved(&parent);
    let mut reference_scratch = ArithmeticScratch::default();
    let mut full = Meter::local(u64::MAX);
    reference
        .select(&child, &index, &mut reference_scratch, &mut full)
        .unwrap();
    assert_eq!(&*reference_scratch.touched, &[2]);
    for limit in 0..full.charged() {
        let mut snapshot = saved(&parent);
        let mut scratch = ArithmeticScratch::default();
        let mut work = Meter::local(limit);
        assert_eq!(
            snapshot.select(&child, &index, &mut scratch, &mut work),
            Err(Failure::Stopped(Stop::WorkLimit))
        );
        assert!(work.charged() <= limit);
        assert_eq!(snapshot.old_masks(), parent.decision_words());
        assert!(snapshot.initialized());
        assert_eq!(child.decision(1), Some(false));
        snapshot
            .select(&child, &index, &mut scratch, &mut Meter::local(u64::MAX))
            .unwrap();
        assert_eq!(&*scratch.touched, &[2]);
        assert_eq!(snapshot.old_masks(), parent.decision_words());
    }
}

#[test]
fn ancestry_admits_exactly_the_performed_prefix() {
    for atoms in [0_usize, 1, 65, 129] {
        let region = Region::all_open(atoms);
        let snapshot = saved(&region);
        let needed = 4 + 6 * u64::try_from(atoms.div_ceil(64)).unwrap();
        for limit in 0..=needed {
            let mut work = Meter::local(limit);
            assert_eq!(
                snapshot.validate(&region, atoms, &mut work),
                if limit == needed {
                    Ok(())
                } else {
                    Err(Failure::Stopped(Stop::WorkLimit))
                }
            );
            assert_eq!(work.charged(), limit);
            assert!(snapshot.initialized());
            assert!(snapshot.words.iter().all(|word| *word == 0));
            assert_eq!(region.open().count(), atoms);
        }
    }
}

#[test]
fn bad_lineage_stops_at_its_first_missing_word() {
    for (held, atom, needed) in [
        (true, 0, 7),
        (true, 128, 13),
        (false, 0, 16),
        (false, 128, 22),
    ] {
        let mut prior = Region::all_open(129);
        assert!(if held {
            prior.hold(atom)
        } else {
            prior.cut(atom)
        });
        let snapshot = saved(&prior);
        let current = Region::all_open(129);
        for limit in 0..=needed {
            let mut work = Meter::local(limit);
            assert_eq!(
                snapshot.validate(&current, 129, &mut work),
                Err(Failure::Stopped(if limit == needed {
                    Stop::InvalidProgram
                } else {
                    Stop::WorkLimit
                }))
            );
            assert_eq!(work.charged(), limit);
            assert_eq!(snapshot.old_masks(), prior.decision_words());
        }
    }
}
