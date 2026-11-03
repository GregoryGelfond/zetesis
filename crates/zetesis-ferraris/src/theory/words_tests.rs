use std::mem::{align_of, offset_of, size_of};

use super::{Interpretation, Theory};

#[test]
fn interpretation_keeps_the_original_owner_and_word_layout() {
    struct Original {
        theory: Theory,
        words: Vec<u64>,
    }
    assert_eq!(size_of::<Interpretation>(), size_of::<Original>());
    assert_eq!(align_of::<Interpretation>(), align_of::<Original>());
    assert_eq!(
        offset_of!(Interpretation, theory),
        offset_of!(Original, theory)
    );
    assert_eq!(
        offset_of!(Interpretation, words),
        offset_of!(Original, words)
    );
}
