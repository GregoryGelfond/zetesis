//! Bounded construction descriptions, consumed at the program admission door.

use std::ops::Range;

use crate::catalog::Error;
use crate::{Predicate, Value};

/// One closed argument alternative. An integer interval is inclusive; a
/// descending interval contributes no values. Segment order is source order.
#[derive(Clone, Debug)]
pub enum FactSegment {
    /// One already constructed closed logical value.
    Value(Value),
    /// Consecutive signed integers without an expanded input vector.
    Interval {
        /// Inclusive first integer.
        first: i32,
        /// Inclusive last integer.
        last: i32,
    },
}

impl FactSegment {
    pub(super) fn len(&self) -> Result<usize, Error> {
        match self {
            Self::Value(_) => Ok(1),
            Self::Interval { first, last } if first <= last => {
                usize::try_from(i64::from(*last) - i64::from(*first) + 1)
                    .map_err(|_| Error::Overflow)
            }
            Self::Interval { .. } => Ok(0),
        }
    }
}

/// Source-ordered fact construction descriptions, separate from rule templates.
/// Each block is anchored immediately before a template position. Products keep
/// compact intervals; adjacent rows of one predicate and anchor share column
/// vectors. The program consumes these descriptions and retains one canonical
/// atom store plus occurrence positions, not these owned values.
///
/// Caller input is outside `AdmissionLimits::max_bytes`, as with `Template`.
/// Builder vector reservations are fallible. A failed push poisons this value;
/// admission then refuses it rather than publishing an accepted prefix.
#[derive(Default, Debug)]
pub struct FactTables {
    pub(in crate::program) blocks: Vec<Block>,
    len: usize,
    pub(in crate::program) failed: Option<Error>,
}

#[derive(Debug)]
pub(in crate::program) struct Block {
    pub(in crate::program) before: usize,
    pub(in crate::program) predicate: Predicate,
    pub(in crate::program) first: usize,
    pub(in crate::program) len: usize,
    pub(in crate::program) columns: Columns,
}

#[derive(Debug)]
pub(in crate::program) enum Columns {
    Product(Vec<Vec<FactSegment>>),
    Run(Vec<Vec<Value>>),
}

impl FactTables {
    /// Original fact occurrences, including duplicate atoms.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether no fact occurrence has been submitted.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append one closed row at `before`, the count of preceding templates.
    /// Returns its original fact-occurrence position. Equal atoms remain distinct
    /// occurrences; compatible adjacent rows share one column-major input run.
    /// Moves O(arity) owned value handles; growing columns can move their prior
    /// handles. Amortized work is O(arity) per row, excluding predicate comparison
    /// and allocator behavior. Values are neither canonicalized nor expanded here.
    ///
    /// # Errors
    /// Refuses an arity mismatch, decreasing anchor, index overflow, allocation,
    /// or an earlier failed push. A refusal poisons this construction owner.
    pub fn push_row(
        &mut self,
        before: usize,
        predicate: Predicate,
        values: Vec<Value>,
    ) -> Result<usize, Error> {
        let result = self.row(before, predicate, values);
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }

    fn row(
        &mut self,
        before: usize,
        predicate: Predicate,
        values: Vec<Value>,
    ) -> Result<usize, Error> {
        self.check(before, &predicate, values.len())?;
        let next = self.len.checked_add(1).ok_or(Error::Overflow)?;
        if let Some(Block {
            before: previous,
            predicate: prior,
            len,
            columns: Columns::Run(columns),
            ..
        }) = self.blocks.last_mut()
            && *previous == before
            && prior == &predicate
        {
            // Every nested reservation precedes any row write.
            for column in columns.iter_mut() {
                column.try_reserve(1).map_err(|_| Error::Allocation)?;
            }
            for (column, value) in columns.iter_mut().zip(values) {
                column.push(value);
            }
            *len += 1;
        } else {
            self.blocks.try_reserve(1).map_err(|_| Error::Allocation)?;
            let mut columns = Vec::new();
            columns
                .try_reserve_exact(values.len())
                .map_err(|_| Error::Allocation)?;
            for value in values {
                let mut column = Vec::new();
                column.try_reserve_exact(1).map_err(|_| Error::Allocation)?;
                column.push(value);
                columns.push(column);
            }
            self.blocks.push(Block {
                before,
                predicate,
                first: self.len,
                len: 1,
                columns: Columns::Run(columns),
            });
        }
        let first = self.len;
        self.len = next;
        Ok(first)
    }

    /// Append a Cartesian product in last-argument-fastest order. Columns contain
    /// their alternatives in source order; one empty column makes an empty
    /// product, while a zero-arity product contains the one nullary fact.
    /// Returns the original occurrence range for parallel source evidence.
    /// Checks O(segments + arity) metadata and retains the supplied columns;
    /// interval width and Cartesian products are checked without enumerating
    /// scalar values or rows. `Program::new` performs that admitted enumeration.
    ///
    /// # Errors
    /// Same structural and allocation refusals as [`Self::push_row`], plus an
    /// overflowing alternative or product count. A refusal poisons this owner.
    pub fn push_product(
        &mut self,
        before: usize,
        predicate: Predicate,
        columns: Vec<Vec<FactSegment>>,
    ) -> Result<Range<usize>, Error> {
        let result = self.product(before, predicate, columns);
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }

    fn product(
        &mut self,
        before: usize,
        predicate: Predicate,
        columns: Vec<Vec<FactSegment>>,
    ) -> Result<Range<usize>, Error> {
        self.check(before, &predicate, columns.len())?;
        let width = |column: &[FactSegment]| {
            column.iter().try_fold(0_usize, |sum, segment| {
                sum.checked_add(segment.len()?).ok_or(Error::Overflow)
            })
        };
        let empty = columns.iter().try_fold(false, |empty, column| {
            let count = width(column)?;
            Ok::<_, Error>(empty || count == 0)
        })?;
        if empty {
            return Ok(self.len..self.len);
        }
        let count = columns.iter().try_fold(1_usize, |rows, column| {
            rows.checked_mul(width(column)?).ok_or(Error::Overflow)
        })?;
        let next = self.len.checked_add(count).ok_or(Error::Overflow)?;
        self.blocks.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.blocks.push(Block {
            before,
            predicate,
            first: self.len,
            len: count,
            columns: Columns::Product(columns),
        });
        let range = self.len..next;
        self.len = next;
        Ok(range)
    }

    fn check(&self, before: usize, predicate: &Predicate, arity: usize) -> Result<(), Error> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if predicate.arity() != arity
            || self
                .blocks
                .last()
                .is_some_and(|block| block.before > before)
        {
            return Err(Error::Shape);
        }
        Ok(())
    }

    /// Signed signatures and their first original occurrence, in input block
    /// order. Coherence scans each block once without expanding its rows.
    pub fn predicates(&self) -> impl Iterator<Item = (&Predicate, usize)> {
        self.blocks
            .iter()
            .map(|block| (&block.predicate, block.first))
    }
}
