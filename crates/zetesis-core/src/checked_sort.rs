//! Stable iterative index sorting; the caller supplies the comparison contract.

use std::cmp::Ordering;

/// Both buffers have the same initialized length. The comparator owns permits
/// for reads/comparisons; this primitive permits each output write. On refusal
/// neither buffer promises a complete order. Successful output is in `values`.
/// Each pass merges adjacent sorted runs of `width` cells and doubles that
/// width; the final pass covers the whole finite occurrence extent. Equal keys
/// retain their input order because the left run wins an equal comparison.
///
/// Two runs of more than one cell whose last and first cells are already in
/// order are copied without another comparison: every cell of the left run
/// is at most its last, and every cell of the right run at least its first,
/// so their concatenation is the stable merge. An input that is already in
/// order therefore costs one comparison for each pair of runs, fewer than
/// its cells in all, where merging them compared half the cells in every
/// pass. An input in no order pays one comparison more for each merge of
/// longer runs, fewer than half its cells in all. Runs of one cell are merged
/// directly: the merge's one comparison is the test.
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
            let ordered =
                width > 1 && middle < end && !compare(values, middle - 1, middle, before)?.is_gt();
            let (mut left, mut right) = (start, middle);
            for output in &mut scratch[start..end] {
                let take_left = if right == end {
                    true
                } else if left == middle {
                    false
                } else {
                    ordered || !compare(values, left, right, before)?.is_gt()
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

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::convert::Infallible;

    use super::sort;

    /// The positions `0..keys.len()` sorted by their keys, and the number of
    /// comparisons the sort asked for.
    fn sorted(keys: &[usize]) -> (Vec<usize>, usize) {
        let mut values: Vec<usize> = (0..keys.len()).collect();
        let mut scratch = vec![0; keys.len()];
        let mut comparisons = 0;
        let mut permit = || Ok::<(), Infallible>(());
        let result: Result<(), Infallible> = sort(
            &mut values,
            &mut scratch,
            |cells, left, right, _| {
                comparisons += 1;
                Ok::<Ordering, Infallible>(keys[cells[left]].cmp(&keys[cells[right]]))
            },
            &mut permit,
        );
        result.unwrap();
        (values, comparisons)
    }

    /// Whether `order` lists every position once, by ascending key, and equal
    /// keys by ascending position.
    fn stable(keys: &[usize], order: &[usize]) -> bool {
        let mut seen = vec![false; keys.len()];
        order
            .iter()
            .all(|&position| !std::mem::replace(&mut seen[position], true))
            && order.len() == keys.len()
            && order
                .windows(2)
                .all(|pair| (keys[pair[0]], pair[0]) < (keys[pair[1]], pair[1]))
    }

    /// A fixed permutation-like sequence of `count` keys with repetitions.
    fn scattered(count: usize) -> Vec<usize> {
        (0..count).map(|index| (index * 7919 + 13) % 257).collect()
    }

    #[test]
    fn an_input_in_no_order_is_sorted_stably() {
        for count in [0, 1, 2, 3, 5, 8, 100, 1000] {
            let keys = scattered(count);
            let (order, _) = sorted(&keys);
            assert!(stable(&keys, &order), "{count}");
        }
    }

    #[test]
    fn an_ordered_input_costs_fewer_comparisons_than_cells() {
        // One comparison for each pair of runs: 512 pairs of single cells,
        // then 256, 128 and so on down to the last pair.
        let keys: Vec<usize> = (0..1024).collect();
        let (order, comparisons) = sorted(&keys);
        assert_eq!(order, keys);
        assert_eq!(comparisons, 1023);
    }

    #[test]
    fn equal_keys_count_as_ordered() {
        let keys = vec![4; 1024];
        let (order, comparisons) = sorted(&keys);
        assert_eq!(order, (0..1024).collect::<Vec<_>>());
        assert_eq!(comparisons, 1023);
    }

    #[test]
    fn runs_of_one_cell_are_merged_by_one_comparison() {
        // Reversed: no two runs are in order. Four merges of single cells
        // take one comparison each; a test before them would double that.
        // The two merges of pairs and the last of fours each pay their test
        // and then merge: 4 + 2 * (1 + 2) + (1 + 4) = 15.
        let keys: Vec<usize> = (0..8).rev().collect();
        let (order, comparisons) = sorted(&keys);
        assert!(stable(&keys, &order));
        assert_eq!(comparisons, 15);
    }

    #[test]
    fn runs_are_tested_by_the_last_cell_of_the_left_one() {
        // The left run begins below the right one and ends above its start:
        // only its last cell shows that the runs must be merged.
        let keys = [1, 5, 2, 6];
        let (order, _) = sorted(&keys);
        assert_eq!(order, [0, 2, 1, 3]);
    }
}
