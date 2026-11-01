use super::*;

#[test]
fn selection_keeps_its_original_borrowed_layout() {
    use std::mem::{align_of, size_of};

    enum Original<'a> {
        Prefix(usize),
        Mask { words: &'a [u64], count: usize },
    }
    assert_eq!(size_of::<Selection<'_>>(), size_of::<Original<'_>>());
    assert_eq!(align_of::<Selection<'_>>(), align_of::<Original<'_>>());
    let words = [1, 0, 2];
    let original = Original::Mask {
        words: &words,
        count: 2,
    };
    let selected = Selection::Mask {
        words: Words::new(&words[..]),
        count: 2,
    };
    let (
        Original::Mask {
            words: before,
            count: expected,
        },
        Selection::Mask {
            words: after,
            count,
        },
    ) = (&original, &selected)
    else {
        unreachable!("mask fixtures");
    };
    assert_eq!(
        (before.as_ptr(), before.len()),
        (after.as_ptr(), after.len())
    );
    assert_eq!(count, expected);
    assert_eq!(selected.len(), 2);
    assert_eq!(
        (0..192)
            .filter(|&bit| selected.contains(bit))
            .collect::<Vec<_>>(),
        [0, 129]
    );
    let Original::Prefix(count) = Original::Prefix(7) else {
        unreachable!("prefix fixture");
    };
    assert_eq!(Selection::Prefix(count).len(), 7);
}
