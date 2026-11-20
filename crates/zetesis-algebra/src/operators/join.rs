//! Indexed, columnar, set-at-a-time binary equijoin.
//!
//! Both realizations take two [`Atoms`] relations and a list of equality key
//! columns, and produce a [`Bindings`] table whose columns are the left
//! attributes followed by the right attributes. The output is sorted
//! lexicographically so the CPU path is deterministic at every thread count.
//! Failed charges return no partial table.
//!
//! [`JoinMethod::SortedMerge`] sorts both sides on the key and merges runs of
//! equal keys. [`JoinMethod::Hash`] builds an index from the smaller side and
//! probes the other, then sorts the result. The two methods are required to
//! produce identical bindings; the caller compares them on real work and
//! keeps one default. Loops walk column slices so the compiler can
//! auto-vectorize key compares and row copies. There is no hand-written SIMD
//! and no shipped GPU kernel.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::relation::{Atoms, Bindings, LayoutError};

/// How to realize a binary equality join.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum JoinMethod {
    /// Sort both sides on the join key and merge equal-key runs.
    #[default]
    SortedMerge,
    /// Hash the smaller side, probe the other, then sort the output.
    Hash,
}

/// Work recorded by a join, separate from the account callback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JoinStats {
    /// Key or lexicographic comparisons, including those used to sort.
    pub comparisons: u64,
    /// Probes of the hashed side, or merge steps that inspect a key.
    pub probes: u64,
    /// Rows written to the result.
    pub output_rows: u64,
}

/// Equijoin `left` and `right` on the given key columns.
///
/// `left_keys` and `right_keys` must have the same length and name columns
/// inside each relation. The result has width `left.width() + right.width()`,
/// with every matching pair appearing once, sorted lexicographically. Charges
/// one unit for each comparison or probe and one unit for each output row.
/// Empty inputs charge nothing and return an empty table of that width.
///
/// # Errors
/// Returns the first callback refusal. No partial bindings are provided.
/// Layout mistakes (a key out of range, or a length mismatch) are reported
/// as `Ok(Err(LayoutError))` and are not charged.
///
/// # Panics
/// Panics before any charge if a relation has width zero: atom keys have at
/// least one column.
pub fn join<E>(
    left: &Atoms,
    right: &Atoms,
    left_keys: &[usize],
    right_keys: &[usize],
    method: JoinMethod,
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut JoinStats,
) -> Result<Result<Bindings, LayoutError>, E> {
    assert!(
        left.width() > 0 && right.width() > 0,
        "atom keys have at least one column"
    );
    if left_keys.len() != right_keys.len() {
        return Ok(Err(LayoutError::LengthMismatch));
    }
    if left_keys.iter().any(|&index| index >= left.width())
        || right_keys.iter().any(|&index| index >= right.width())
    {
        return Ok(Err(LayoutError::EmptyWidth));
    }
    if left.is_empty() || right.is_empty() {
        return Ok(Bindings::from_columns(vec![
            Vec::new();
            left.width() + right.width()
        ]));
    }
    let pairs = match method {
        JoinMethod::SortedMerge => merge_pairs(left, right, left_keys, right_keys, account, stats)?,
        JoinMethod::Hash => hash_pairs(left, right, left_keys, right_keys, account, stats)?,
    };
    materialize(left, right, &pairs, account, stats)
}

fn merge_pairs<E>(
    left: &Atoms,
    right: &Atoms,
    left_keys: &[usize],
    right_keys: &[usize],
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut JoinStats,
) -> Result<Vec<(usize, usize)>, E> {
    let mut left_order: Vec<usize> = (0..left.len()).collect();
    let mut right_order: Vec<usize> = (0..right.len()).collect();
    sort_by_key(left, left_keys, &mut left_order, account, stats)?;
    sort_by_key(right, right_keys, &mut right_order, account, stats)?;
    let mut pairs = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < left_order.len() && j < right_order.len() {
        account(1)?;
        stats.comparisons = stats.comparisons.saturating_add(1);
        stats.probes = stats.probes.saturating_add(1);
        match cmp_keys(
            left,
            left_order[i],
            left_keys,
            right,
            right_order[j],
            right_keys,
        ) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                let i0 = i;
                let j0 = j;
                i += 1;
                j += 1;
                while i < left_order.len() {
                    account(1)?;
                    stats.comparisons = stats.comparisons.saturating_add(1);
                    if cmp_keys(
                        left,
                        left_order[i],
                        left_keys,
                        left,
                        left_order[i0],
                        left_keys,
                    ) != Ordering::Equal
                    {
                        break;
                    }
                    i += 1;
                }
                while j < right_order.len() {
                    account(1)?;
                    stats.comparisons = stats.comparisons.saturating_add(1);
                    if cmp_keys(
                        right,
                        right_order[j],
                        right_keys,
                        right,
                        right_order[j0],
                        right_keys,
                    ) != Ordering::Equal
                    {
                        break;
                    }
                    j += 1;
                }
                for &left_row in &left_order[i0..i] {
                    for &right_row in &right_order[j0..j] {
                        pairs.push((left_row, right_row));
                    }
                }
            }
        }
    }
    Ok(pairs)
}

fn hash_pairs<E>(
    left: &Atoms,
    right: &Atoms,
    left_keys: &[usize],
    right_keys: &[usize],
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut JoinStats,
) -> Result<Vec<(usize, usize)>, E> {
    // Hash the smaller side so the probe stream is the longer one.
    let build_left = left.len() <= right.len();
    if build_left {
        hash_and_probe(left, left_keys, right, right_keys, true, account, stats)
    } else {
        hash_and_probe(right, right_keys, left, left_keys, false, account, stats)
    }
}

fn hash_and_probe<E>(
    build: &Atoms,
    build_keys: &[usize],
    probe: &Atoms,
    probe_keys: &[usize],
    build_is_left: bool,
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut JoinStats,
) -> Result<Vec<(usize, usize)>, E> {
    let mut index: HashMap<Vec<i32>, Vec<usize>> = HashMap::new();
    let mut key = vec![0i32; build_keys.len()];
    for row in 0..build.len() {
        account(1)?;
        stats.probes = stats.probes.saturating_add(1);
        write_key(build, row, build_keys, &mut key);
        index.entry(key.clone()).or_default().push(row);
    }
    let mut pairs = Vec::new();
    let mut probe_key = vec![0i32; probe_keys.len()];
    for probe_row in 0..probe.len() {
        account(1)?;
        stats.probes = stats.probes.saturating_add(1);
        write_key(probe, probe_row, probe_keys, &mut probe_key);
        if let Some(hits) = index.get(&probe_key) {
            for &build_row in hits {
                if build_is_left {
                    pairs.push((build_row, probe_row));
                } else {
                    pairs.push((probe_row, build_row));
                }
            }
        }
    }
    Ok(pairs)
}

fn materialize<E>(
    left: &Atoms,
    right: &Atoms,
    pairs: &[(usize, usize)],
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut JoinStats,
) -> Result<Result<Bindings, LayoutError>, E> {
    let width = left.width() + right.width();
    let mut columns = vec![Vec::with_capacity(pairs.len()); width];
    for &(left_row, right_row) in pairs {
        account(1)?;
        stats.output_rows = stats.output_rows.saturating_add(1);
        append_row(left, left_row, &mut columns, 0);
        append_row(right, right_row, &mut columns, left.width());
    }
    // Sort so hash and merge agree, and so any later parallel probe order
    // cannot change the CPU path's bindings.
    if columns[0].len() > 1 {
        let mut order: Vec<usize> = (0..columns[0].len()).collect();
        sort_columns(&columns, &mut order, account, stats)?;
        let mut packed = vec![Vec::with_capacity(order.len()); width];
        for index in order {
            for (dest, source) in packed.iter_mut().zip(&columns) {
                dest.push(source[index]);
            }
        }
        columns = packed;
    }
    match Bindings::from_columns(columns) {
        Ok(bindings) => Ok(Ok(bindings)),
        Err(error) => Ok(Err(error)),
    }
}

fn write_key(atoms: &Atoms, row: usize, keys: &[usize], out: &mut [i32]) {
    for (slot, &column) in keys.iter().enumerate() {
        out[slot] = atoms.column(column)[row];
    }
}

fn append_row(source: &Atoms, row: usize, dest: &mut [Vec<i32>], offset: usize) {
    for column in 0..source.width() {
        dest[offset + column].push(source.column(column)[row]);
    }
}

fn cmp_keys(
    left: &Atoms,
    left_row: usize,
    left_keys: &[usize],
    right: &Atoms,
    right_row: usize,
    right_keys: &[usize],
) -> Ordering {
    for (&left_column, &right_column) in left_keys.iter().zip(right_keys) {
        match left.column(left_column)[left_row].cmp(&right.column(right_column)[right_row]) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

fn sort_by_key<E>(
    atoms: &Atoms,
    keys: &[usize],
    order: &mut [usize],
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut JoinStats,
) -> Result<(), E> {
    let mut refusal = None;
    order.sort_by(|&left, &right| {
        if refusal.is_some() {
            return Ordering::Equal;
        }
        match account(1) {
            Ok(()) => {
                stats.comparisons = stats.comparisons.saturating_add(1);
                cmp_keys(atoms, left, keys, atoms, right, keys)
            }
            Err(error) => {
                refusal = Some(error);
                Ordering::Equal
            }
        }
    });
    match refusal {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn sort_columns<E>(
    columns: &[Vec<i32>],
    order: &mut [usize],
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut JoinStats,
) -> Result<(), E> {
    let mut refusal = None;
    order.sort_by(|&left, &right| {
        if refusal.is_some() {
            return Ordering::Equal;
        }
        match account(1) {
            Ok(()) => {
                stats.comparisons = stats.comparisons.saturating_add(1);
                cmp_rows(columns, left, right)
            }
            Err(error) => {
                refusal = Some(error);
                Ordering::Equal
            }
        }
    });
    match refusal {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn cmp_rows(columns: &[Vec<i32>], left: usize, right: usize) -> Ordering {
    for column in columns {
        match column[left].cmp(&column[right]) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::{JoinMethod, JoinStats, join};
    use crate::operators::relation::Atoms;

    fn atoms<const WIDTH: usize>(rows: &[[i32; WIDTH]]) -> Atoms {
        if rows.is_empty() {
            return Atoms::empty(WIDTH.max(1));
        }
        let width = WIDTH;
        let mut columns = vec![Vec::with_capacity(rows.len()); width];
        for row in rows {
            for (column, &value) in columns.iter_mut().zip(row.iter()) {
                column.push(value);
            }
        }
        Atoms::from_unsorted(&columns, &mut |n| {
            let _ = n;
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap()
    }

    fn run(
        left: &Atoms,
        right: &Atoms,
        left_keys: &[usize],
        right_keys: &[usize],
        method: JoinMethod,
    ) -> (Vec<Vec<i32>>, JoinStats) {
        let mut stats = JoinStats::default();
        let bindings = join(
            left,
            right,
            left_keys,
            right_keys,
            method,
            &mut |_| Ok::<(), ()>(()),
            &mut stats,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            u64::try_from(bindings.len()).expect("row count fits in u64"),
            stats.output_rows
        );
        (bindings.to_rows(), stats)
    }

    fn both(
        left: &Atoms,
        right: &Atoms,
        left_keys: &[usize],
        right_keys: &[usize],
    ) -> Vec<Vec<i32>> {
        let (merge, _) = run(left, right, left_keys, right_keys, JoinMethod::SortedMerge);
        let (hash, _) = run(left, right, left_keys, right_keys, JoinMethod::Hash);
        assert_eq!(merge, hash, "hash and merge must agree");
        assert!(
            merge.windows(2).all(|pair| pair[0] <= pair[1]),
            "output is sorted"
        );
        merge
    }

    #[test]
    fn empty_sides_yield_empty_sorted_schema() {
        let left = Atoms::empty(2);
        let right = atoms(&[[1, 2], [3, 4]]);
        assert!(both(&left, &right, &[0], &[0]).is_empty());
        assert!(both(&right, &left, &[0], &[0]).is_empty());
    }

    #[test]
    fn one_to_one_key_preserves_payloads() {
        let left = atoms(&[[1, 10], [2, 20], [3, 30]]);
        let right = atoms(&[[2, 200], [3, 300], [4, 400]]);
        assert_eq!(
            both(&left, &right, &[0], &[0]),
            vec![vec![2, 20, 2, 200], vec![3, 30, 3, 300]]
        );
    }

    #[test]
    fn many_to_many_enumerates_the_product_of_each_key_run() {
        let left = atoms(&[[1, 10], [1, 11], [2, 20]]);
        let right = atoms(&[[1, 100], [1, 101], [3, 300]]);
        assert_eq!(
            both(&left, &right, &[0], &[0]),
            vec![
                vec![1, 10, 1, 100],
                vec![1, 10, 1, 101],
                vec![1, 11, 1, 100],
                vec![1, 11, 1, 101],
            ]
        );
    }

    #[test]
    fn join_on_a_non_prefix_column() {
        let edge = atoms(&[[1, 2], [1, 3], [2, 3], [4, 5]]);
        let anc = atoms(&[[2, 8], [3, 9], [3, 10], [6, 7]]);
        assert_eq!(
            both(&edge, &anc, &[1], &[0]),
            vec![
                vec![1, 2, 2, 8],
                vec![1, 3, 3, 9],
                vec![1, 3, 3, 10],
                vec![2, 3, 3, 9],
                vec![2, 3, 3, 10],
            ]
        );
    }

    #[test]
    fn two_column_keys_match_only_on_both() {
        let left = atoms(&[[1, 2, 10], [1, 3, 11], [2, 2, 12]]);
        let right = atoms(&[[1, 2, 20], [1, 4, 21], [2, 2, 22]]);
        assert_eq!(
            both(&left, &right, &[0, 1], &[0, 1]),
            vec![vec![1, 2, 10, 1, 2, 20], vec![2, 2, 12, 2, 2, 22]]
        );
    }

    #[test]
    fn charge_refusal_returns_no_partial_table() {
        let left = atoms(&[[1, 10], [2, 20], [3, 30], [4, 40]]);
        let right = atoms(&[[2, 200], [3, 300], [4, 400], [5, 500]]);
        for method in [JoinMethod::SortedMerge, JoinMethod::Hash] {
            let mut budget = 0u64;
            let result = join(
                &left,
                &right,
                &[0],
                &[0],
                method,
                &mut |_| {
                    budget += 1;
                    if budget > 3 { Err("stop") } else { Ok(()) }
                },
                &mut JoinStats::default(),
            );
            assert_eq!(result, Err("stop"), "{method:?} refuses without a table");
        }
    }

    #[test]
    fn key_width_mismatch_is_a_layout_error() {
        let left = atoms(&[[1, 2]]);
        let right = atoms(&[[1, 2]]);
        let result = join(
            &left,
            &right,
            &[0, 1],
            &[0],
            JoinMethod::SortedMerge,
            &mut |_| Ok::<(), ()>(()),
            &mut JoinStats::default(),
        )
        .unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn same_generation_shape_filters_outside_the_join() {
        let edge = atoms(&[[1, 9], [2, 9], [3, 9], [4, 8]]);
        let rows = both(&edge, &edge, &[1], &[1]);
        let retained: Vec<_> = rows
            .into_iter()
            .filter(|row| row[0] != row[2])
            .map(|row| vec![row[0], row[2]])
            .collect();
        assert_eq!(
            retained,
            vec![
                vec![1, 2],
                vec![1, 3],
                vec![2, 1],
                vec![2, 3],
                vec![3, 1],
                vec![3, 2],
            ]
        );
    }

    #[test]
    fn modest_one_column_join_is_a_contiguous_slice_walk() {
        let left_rows: Vec<[i32; 2]> = (0..64).map(|i| [i, i * 2]).collect();
        let right_rows: Vec<[i32; 2]> =
            (0..64).filter(|i| i % 2 == 0).map(|i| [i, i * 3]).collect();
        let left = atoms(&left_rows);
        let right = atoms(&right_rows);
        let rows = both(&left, &right, &[0], &[0]);
        assert_eq!(rows.len(), 32);
        assert_eq!(rows[0], vec![0, 0, 0, 0]);
        assert_eq!(rows[31], vec![62, 124, 62, 186]);
    }
}
