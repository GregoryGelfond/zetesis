//! Stable iterative index sorting; the caller supplies the comparison contract.

use std::cmp::Ordering;

/// Both buffers have the same initialized length. The comparator owns permits
/// for reads/comparisons; this primitive permits each output write. On refusal
/// neither buffer promises a complete order. Successful output is in `values`.
/// Each pass merges adjacent sorted runs of `width` cells and doubles that
/// width; the final pass covers the whole finite occurrence extent. Equal keys
/// retain their input order because the left run wins an equal comparison.
pub(crate) fn sort<E, F: FnMut() -> Result<(), E>>(
    values: &mut Vec<usize>,
    scratch: &mut Vec<usize>,
    mut compare: impl FnMut(&[usize], usize, usize, &mut F) -> Result<Ordering, E>,
    before: &mut F,
) -> Result<(), E> {
    let count = values.len();
    let mut width = 1;
    while width < count {
        let mut start = 0;
        while start < count {
            let middle = start.saturating_add(width).min(count);
            let end = middle.saturating_add(width).min(count);
            let (mut left, mut right) = (start, middle);
            for output in &mut scratch[start..end] {
                let take_left = if right == end {
                    true
                } else if left == middle {
                    false
                } else {
                    !compare(values, left, right, before)?.is_gt()
                };
                before()?;
                *output = if take_left {
                    let value = values[left];
                    left += 1;
                    value
                } else {
                    let value = values[right];
                    right += 1;
                    value
                };
            }
            start = end;
        }
        std::mem::swap(values, scratch);
        width = width.saturating_mul(2);
    }
    Ok(())
}
