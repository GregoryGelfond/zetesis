//! Columnar atom sets and bindings.
//!
//! Both [`Atoms`] and [`Bindings`] store one densely packed `i32` column per
//! attribute, with every column the same length. Fields stay private so packing
//! can evolve. Rows of [`Atoms`] are unique and sorted lexicographically; rows
//! of [`Bindings`] keep enumeration order and may repeat until [`super::project`]
//! coalesces them.
//!
//! The layout is the default representation on the CPU path and the buffer
//! shape a later GPU backend should upload: little-endian `i32` cells, column
//! major, 4-byte aligned, with no hidden padding between cells.

use std::cmp::Ordering;
use std::ops::Range;

/// A columnar set of atoms, unique and sorted lexicographically.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Atoms {
    columns: Vec<Vec<i32>>,
}

/// A columnar collection of bindings, one integer column per slot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bindings {
    columns: Vec<Vec<i32>>,
    /// Row count when `width` is zero (the empty-body carrier is 0 or 1).
    empty_body_rows: usize,
}

impl Atoms {
    /// An empty set of `width` attributes.
    ///
    /// # Panics
    /// Panics if `width` is zero: an atom key must have at least one column.
    #[must_use]
    pub fn empty(width: usize) -> Self {
        assert!(width > 0, "atom keys have at least one column");
        Self {
            columns: vec![Vec::new(); width],
        }
    }

    /// Build a set from already unique, lexicographically sorted columns.
    ///
    /// # Errors
    /// Returns [`LayoutError`] if a column length differs, the width is zero,
    /// or a row is out of order or repeated.
    pub fn from_sorted_unique(columns: Vec<Vec<i32>>) -> Result<Self, LayoutError> {
        if columns.is_empty() {
            return Err(LayoutError::EmptyWidth);
        }
        check_columns(&columns)?;
        for index in 1..columns[0].len() {
            match cmp_row(&columns, index - 1, index) {
                Ordering::Less => {}
                Ordering::Equal => return Err(LayoutError::Duplicate),
                Ordering::Greater => return Err(LayoutError::Unsorted),
            }
        }
        Ok(Self { columns })
    }

    /// Sort, drop duplicates, and adopt `columns` as an atom set.
    ///
    /// Charges one comparison for each ordering decision in the sort and one
    /// comparison for each adjacent pair while uniquing. Empty input charges
    /// nothing.
    ///
    /// # Errors
    /// Returns the first callback refusal, leaving no partial set. Layout
    /// mistakes are reported as `Ok(Err(LayoutError))` and are not charged.
    pub fn from_unsorted<E>(
        columns: &[Vec<i32>],
        account: &mut impl FnMut(u64) -> Result<(), E>,
    ) -> Result<Result<Self, LayoutError>, E> {
        if columns.is_empty() {
            return Ok(Err(LayoutError::EmptyWidth));
        }
        if let Err(error) = check_columns(columns) {
            return Ok(Err(error));
        }
        let rows = columns[0].len();
        let mut order: Vec<usize> = (0..rows).collect();
        sort_order(columns, &mut order, account)?;
        let mut packed = vec![Vec::new(); columns.len()];
        if !order.is_empty() {
            push_row(&mut packed, columns, order[0]);
            for window in order.windows(2) {
                account(1)?;
                if cmp_row(columns, window[0], window[1]) != Ordering::Equal {
                    push_row(&mut packed, columns, window[1]);
                }
            }
        }
        Ok(Ok(Self { columns: packed }))
    }

    /// Number of attributes.
    #[must_use]
    pub fn width(&self) -> usize {
        self.columns.len()
    }

    /// Number of atoms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns.first().map_or(0, Vec::len)
    }

    /// Whether the set contains no atoms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow column `index`.
    ///
    /// # Panics
    /// Panics if `index` is outside `0..width`.
    #[must_use]
    pub fn column(&self, index: usize) -> &[i32] {
        &self.columns[index]
    }

    /// Owned copies of each row, in sort order.
    #[must_use]
    pub fn to_rows(&self) -> Vec<Vec<i32>> {
        copy_rows(&self.columns)
    }

    /// True when `row` is in the set. Uses binary search on the sorted keys.
    #[must_use]
    pub fn contains(&self, row: &[i32]) -> bool {
        if row.len() != self.width() || self.is_empty() {
            return false;
        }
        let mut low = 0;
        let mut high = self.len();
        while low < high {
            let mid = low + (high - low) / 2;
            match cmp_key(&self.columns, mid, row) {
                Ordering::Equal => return true,
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
            }
        }
        false
    }

    pub(super) fn from_checked(columns: Vec<Vec<i32>>) -> Self {
        debug_assert!(check_columns(&columns).is_ok());
        Self { columns }
    }
}

impl Bindings {
    /// No rows, `width` slots. A width of zero is the empty-body binding's
    /// carrier: one later [`Self::push_empty`] row has no columns.
    #[must_use]
    pub fn empty(width: usize) -> Self {
        Self {
            columns: vec![Vec::new(); width],
            empty_body_rows: 0,
        }
    }

    /// Adopt `columns` as bindings.
    ///
    /// # Errors
    /// Returns [`LayoutError`] if a column length differs.
    pub fn from_columns(columns: Vec<Vec<i32>>) -> Result<Self, LayoutError> {
        check_columns(&columns)?;
        Ok(Self {
            columns,
            empty_body_rows: 0,
        })
    }

    /// Number of slots.
    #[must_use]
    pub fn width(&self) -> usize {
        self.columns.len()
    }

    /// Number of rows. A width-zero binding uses an explicit row count of
    /// zero or one, represented as having no columns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns.first().map_or(self.empty_body_rows, Vec::len)
    }

    /// Whether there are no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow column `index`.
    ///
    /// # Panics
    /// Panics if `index` is outside `0..width`.
    #[must_use]
    pub fn column(&self, index: usize) -> &[i32] {
        &self.columns[index]
    }

    /// Inclusive range of row indices.
    #[must_use]
    pub fn row_range(&self) -> Range<usize> {
        0..self.len()
    }

    /// Copy one row into `out`, which must already have length `width`.
    ///
    /// # Panics
    /// Panics if `row` is out of range or `out` has the wrong length.
    pub fn row_into(&self, row: usize, out: &mut [i32]) {
        assert_eq!(out.len(), self.width(), "row buffer matches width");
        assert!(row < self.len(), "row index is in range");
        for (slot, column) in self.columns.iter().enumerate() {
            out[slot] = column[row];
        }
    }

    /// Collect the rows as owned tuples, in enumeration order.
    #[must_use]
    pub fn to_rows(&self) -> Vec<Vec<i32>> {
        if self.width() == 0 {
            return vec![Vec::new(); self.len()];
        }
        copy_rows(&self.columns)
    }

    pub(super) fn push_row(&mut self, row: &[i32]) {
        debug_assert_eq!(row.len(), self.width());
        if self.width() == 0 {
            self.empty_body_rows = 1;
            return;
        }
        for (column, value) in self.columns.iter_mut().zip(row) {
            column.push(*value);
        }
    }

    pub(super) fn push_empty(&mut self) {
        debug_assert_eq!(self.width(), 0);
        self.empty_body_rows = 1;
    }

    pub(super) fn retain_mask(&mut self, mask: &[u8]) {
        if self.width() == 0 {
            if self.empty_body_rows == 1 && mask.first().copied().unwrap_or(0) == 0 {
                self.empty_body_rows = 0;
            }
            return;
        }
        let rows = self.len();
        debug_assert_eq!(mask.len(), rows);
        for column in &mut self.columns {
            let mut write = 0;
            for row in 0..rows {
                if mask[row] != 0 {
                    column[write] = column[row];
                    write += 1;
                }
            }
            column.truncate(write);
        }
    }
}

/// A columnar layout that is not a relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// No attribute columns were supplied for an atom set.
    EmptyWidth,
    /// Two columns had different lengths.
    LengthMismatch,
    /// A row was not strictly after its predecessor.
    Unsorted,
    /// A row occurred twice.
    Duplicate,
}

fn check_columns(columns: &[Vec<i32>]) -> Result<(), LayoutError> {
    let Some(first) = columns.first() else {
        return Ok(());
    };
    if columns.iter().any(|column| column.len() != first.len()) {
        return Err(LayoutError::LengthMismatch);
    }
    Ok(())
}

fn cmp_row(columns: &[Vec<i32>], left: usize, right: usize) -> Ordering {
    for column in columns {
        match column[left].cmp(&column[right]) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

fn cmp_key(columns: &[Vec<i32>], index: usize, key: &[i32]) -> Ordering {
    for (column, value) in columns.iter().zip(key) {
        match column[index].cmp(value) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

fn push_row(dest: &mut [Vec<i32>], source: &[Vec<i32>], row: usize) {
    for (column, values) in dest.iter_mut().zip(source) {
        column.push(values[row]);
    }
}

fn copy_rows(columns: &[Vec<i32>]) -> Vec<Vec<i32>> {
    let rows = columns.first().map_or(0, Vec::len);
    let width = columns.len();
    let mut out = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut item = Vec::with_capacity(width);
        for column in columns {
            item.push(column[row]);
        }
        out.push(item);
    }
    out
}

fn sort_order<E>(
    columns: &[Vec<i32>],
    order: &mut [usize],
    account: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<(), E> {
    let mut refusal = None;
    order.sort_by(|&left, &right| {
        if refusal.is_some() {
            return Ordering::Equal;
        }
        match account(1) {
            Ok(()) => cmp_row(columns, left, right),
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
