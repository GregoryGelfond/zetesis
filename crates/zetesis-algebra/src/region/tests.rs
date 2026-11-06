mod subject_step;

use std::mem::{align_of, offset_of, size_of};

use super::Region;

#[test]
fn resetting_a_region_reuses_equal_length_masks_and_replaces_the_universe() {
    let mut source = Region::all_open(65);
    assert!(source.hold(64) && source.cut(2));
    let mut target = Region::all_open(127);
    assert!(target.hold(120));
    let held = target.held.as_ptr();
    let cut = target.cut.as_ptr();

    target.clone_from(&source);
    assert_eq!(target, source);
    assert_eq!(target.held.as_ptr(), held);
    assert_eq!(target.cut.as_ptr(), cut);

    let smaller = Region::all_open(1);
    target.clone_from(&smaller);
    assert_eq!(target, smaller);
}

#[test]
fn region_keeps_the_original_two_mask_record_layout() {
    struct Original {
        held: Box<[u64]>,
        cut: Box<[u64]>,
        atoms: usize,
    }
    assert_eq!(size_of::<Region>(), size_of::<Original>());
    assert_eq!(align_of::<Region>(), align_of::<Original>());
    assert_eq!(offset_of!(Region, held), offset_of!(Original, held));
    assert_eq!(offset_of!(Region, cut), offset_of!(Original, cut));
    assert_eq!(offset_of!(Region, atoms), offset_of!(Original, atoms));
}
