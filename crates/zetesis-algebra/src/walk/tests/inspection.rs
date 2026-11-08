//! Shared classification preserves root counting and the entry's split policy.

use super::super::{Counting, Step, inspect};
use crate::region::{Entry, Narrowing, Region};

fn region(mut ternary: usize, atoms: usize) -> Region {
    let mut region = Region::all_open(atoms);
    for atom in 0..atoms {
        match ternary % 3 {
            0 => {}
            1 => assert!(region.cut(atom)),
            2 => assert!(region.hold(atom)),
            _ => unreachable!(),
        }
        ternary /= 3;
    }
    region
}

#[test]
fn refutation_precedes_every_region_shape() {
    for atoms in 0_usize..=3 {
        for code in 0..3_usize.pow(u32::try_from(atoms).unwrap()) {
            let entry = Entry::new(region(code, atoms), ());
            for counting in [Counting::Never, Counting::Unchanged] {
                for root in [false, true] {
                    assert_eq!(
                        inspect(&entry, Narrowing::Refuted, counting, root),
                        Step::Refuted
                    );
                }
            }
        }
    }
}

#[test]
fn only_unchanged_open_descendants_can_be_counted() {
    for code in 0..27 {
        let entry = Entry::new(region(code, 3), ());
        let open: Vec<_> = (0..3).filter(|&atom| entry.region.is_open(atom)).collect();
        for counting in [Counting::Never, Counting::Unchanged] {
            for root in [false, true] {
                for changed in [false, true] {
                    let actual = inspect(&entry, Narrowing::Fixed { changed }, counting, root);
                    if open.is_empty() {
                        assert_eq!(actual, Step::Leaf);
                    } else if counting == Counting::Unchanged && !root && !changed {
                        assert_eq!(actual, Step::Counted);
                    } else {
                        assert_eq!(actual, Step::Split(*open.last().unwrap()));
                    }
                }
            }
        }
    }
}

#[test]
fn inspection_uses_the_existing_open_preference() {
    let mut entry = Entry::new(Region::all_open(5), ());
    assert!(entry.region.cut(3));
    entry.prefer(1);
    for root in [false, true] {
        for changed in [false, true] {
            assert_eq!(
                inspect(&entry, Narrowing::Fixed { changed }, Counting::Never, root),
                Step::Split(1)
            );
        }
    }
    // Deciding the preference changes the interval, not the policy's fallback.
    assert!(entry.region.hold(1));
    assert_eq!(
        inspect(
            &entry,
            Narrowing::Fixed { changed: true },
            Counting::Never,
            false
        ),
        Step::Split(4)
    );
}
