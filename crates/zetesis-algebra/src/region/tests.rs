mod subject_step;

use std::mem::{align_of, offset_of, size_of};

use super::Region;

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
