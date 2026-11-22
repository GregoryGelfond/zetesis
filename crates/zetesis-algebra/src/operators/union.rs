//! Set union of columnar atom collections.

use super::relation::Atoms;

/// Union of two atom sets of equal width, in lexicographic order.
///
/// Equal keys keep the left representative. Charges one comparison for each
/// merge step (including end-of-input sentinels) and one unit for each output
/// atom. Empty inputs charge only the output atoms they contribute, if any.
///
/// # Errors
/// Returns the first callback refusal before the refused comparison or output
/// write. No partial set is returned.
///
/// # Panics
/// Panics before any charge if the widths differ.
pub fn union<E>(
    left: &Atoms,
    right: &Atoms,
    account: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<Atoms, E> {
    assert_eq!(left.width(), right.width(), "atom sets have equal width");
    let width = left.width();
    let mut columns = vec![Vec::new(); width];
    let mut i = 0;
    let mut j = 0;
    while i < left.len() && j < right.len() {
        account(1)?;
        match cmp_atoms(left, i, right, j) {
            std::cmp::Ordering::Less => {
                account(1)?;
                push(left, i, &mut columns);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                account(1)?;
                push(right, j, &mut columns);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                account(1)?;
                push(left, i, &mut columns);
                i += 1;
                j += 1;
            }
        }
    }
    while i < left.len() {
        account(1)?;
        push(left, i, &mut columns);
        i += 1;
    }
    while j < right.len() {
        account(1)?;
        push(right, j, &mut columns);
        j += 1;
    }
    Ok(Atoms::from_checked(columns))
}

fn cmp_atoms(left: &Atoms, i: usize, right: &Atoms, j: usize) -> std::cmp::Ordering {
    for column in 0..left.width() {
        match left.column(column)[i].cmp(&right.column(column)[j]) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

fn push(source: &Atoms, row: usize, dest: &mut [Vec<i32>]) {
    for (column, values) in dest.iter_mut().enumerate() {
        values.push(source.column(column)[row]);
    }
}
