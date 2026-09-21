//! Exact finite-table selection and projection over an immutable typed relation.
//!
//! A surviving row is a tuple in the supplied table, not an ASP producer, a
//! true atom, or an answer set. Source completeness and reduct membership remain
//! separate obligations. Equality and aliasing use whole typed values.
//!
//! Preparation borrows the authoritative relation and builds value-to-row
//! bitsets. Every projection starts from the same immutable coherent rows;
//! domains may narrow, widen, or be restored in any order. No trail or previous
//! domain cardinality is trusted. Row masks borrow only the authoritative
//! relation; projected value domains also borrow the prepared table.

use std::{mem::size_of, ops::Range};

use zetesis_core::{Value, relation::Relation};

use crate::{Cancellation, Stop};

mod accounting;
mod selection;

pub use selection::{Domain, Selection};

use accounting::Work;

const WORD_BITS: usize = u32::BITS as usize;

/// Inclusive limits for one preparation, selection or projection operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum distinct variable/value support entries across this table.
    pub max_entries: usize,
    /// Live operation capacity, including the relation and prepared input.
    /// Borrowed source payload and other caller-owned results are excluded.
    pub max_bytes: usize,
    /// Charged cells, bitwise operations and typed-comparison payload bytes.
    pub max_work: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 1_048_576,
            max_bytes: 134_217_728,
            max_work: 100_000_000,
        }
    }
}

/// A finite table-operation resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Distinct variable/value entries, including the same value in different variables.
    Entries,
    /// Live operation-scoped capacity.
    Bytes,
    /// Charged logical operations and typed payload bytes.
    Work,
}

/// An incomplete operation, never a conclusion about program consistency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    /// Column variables were not canonical consecutive first-occurrence labels.
    Scope,
    /// The supplied domain count differs from the variable count.
    Domains,
    /// A trusted relation could not decode one of its own rows.
    Relation,
    /// A capacity or work sum cannot be represented.
    Overflow,
    /// A fallible vector reservation failed.
    Allocation,
    /// Cancellation or deadline expiry was observed.
    Interrupted(Stop),
    /// An inclusive finite ceiling was exceeded.
    Limit {
        /// Exhausted resource.
        resource: Resource,
        /// Proposed amount, including the refused operation.
        observed: u128,
        /// Inclusive allowance.
        limit: u128,
    },
}

/// Failure with the completed work prefix and conservative capacity peak.
/// No partial table or result is published. Existing borrowed objects remain valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Original cause, without converting an interruption into an empty table.
    pub cause: Cause,
    /// Work charged before the refused operation.
    pub work: u64,
    /// Largest accounted capacity, including conservatively coexisting reallocation buffers.
    pub peak_bytes: usize,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "finite-table operation failed: {:?}", self.cause)
    }
}
impl std::error::Error for Failure {}

/// Complete operation costs; these are logical counters and capacities, not RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Statistics {
    /// Charged work for this operation, excluding earlier preparation.
    pub work: u64,
    /// Returned object's owned capacity, excluding its borrowed inputs.
    pub retained_bytes: usize,
    /// Conservative largest live capacity including this operation's inputs and scratch.
    pub peak_bytes: usize,
}

/// Prepared support bitsets over one live immutable relation.
///
/// The scope labels argument columns by variable: `[0, 1, 0]` represents a
/// repeated first variable. Labels must be consecutive in first-occurrence
/// order. Rows disagreeing at aliased columns are excluded before projection.
/// Equal row occurrences remain distinct positions, including catalog aliases.
/// Value representatives are borrowed; no logical value or atom is cloned.
///
/// With R rows, V distinct variable/value pairs, and W=ceil(R/32), support
/// storage is V*W words. Preparation additionally sorts O(R) borrowed row cells
/// per variable; typed comparisons include their payload costs. Large finite
/// domains can make this index substantially larger than the original columns.
pub struct Table<'owner, 'source> {
    relation: &'owner Relation<'source>,
    scope: Vec<usize>,
    variables: Vec<Range<usize>>,
    values: Vec<&'source Value>,
    supports: Vec<u32>,
    coherent: Vec<u32>,
    statistics: Statistics,
}

/// A complete finite-table projection for one supplied domain family.
///
/// Rows retain their original relation positions. A value is retained exactly
/// when a surviving complete row carries it. Empty rows mean that this finite
/// table/domain intersection is empty, not that an ASP program is inconsistent.
pub struct Projection<'table, 'owner, 'source> {
    table: &'table Table<'owner, 'source>,
    rows: Vec<u32>,
    supported: Vec<bool>,
    statistics: Statistics,
}

#[derive(Clone, Copy)]
struct RowValue<'source> {
    row: usize,
    value: &'source Value,
}

impl<'owner, 'source> Table<'owner, 'source> {
    /// Prepare a borrowed relation for repeated exact domain projection.
    ///
    /// # Errors
    /// Refuses malformed scope, finite limits, allocation failure or interrupted
    /// control. The relation is unchanged; no partial index is returned.
    ///
    /// # Examples
    /// ```
    /// use zetesis_core::{Atom, Predicate, Value, relation::Relation};
    /// use zetesis_cpu::{Cancellation, table::{Limits, Table}};
    /// let predicate = Predicate::new("pair", 2)?;
    /// let rows = [Atom::new(predicate.clone(), vec![Value::Number(1), Value::Number(2)])?];
    /// let relation = Relation::from_atoms(&predicate, &rows, Default::default())?;
    /// let table = Table::prepare(&relation, &[0, 1], Limits::default(), &Cancellation::default())?;
    /// let first = [Value::Number(1), Value::Number(3)];
    /// let second = [Value::Number(2)];
    /// let projected = table.project(&[&first, &second], Limits::default(), &Cancellation::default())?;
    /// assert!(projected.contains(0));
    /// assert_eq!(projected.domain(0).unwrap().collect::<Vec<_>>(), vec![&Value::Number(1)]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn prepare(
        relation: &'owner Relation<'source>,
        scope: &[usize],
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Self, Failure> {
        let scratch_headers = 2 * size_of::<Vec<RowValue<'_>>>() + size_of::<Vec<usize>>();
        let external = relation.storage().retained_bytes;
        let mut work = Work::new(
            limits,
            cancellation,
            external,
            size_of::<Self>() + scratch_headers,
        )?;
        let result = (|| {
            if scope.len() != relation.predicate().arity() {
                return Err(Cause::Scope);
            }
            let mut copied_scope = work.reserve(scope.len())?;
            let mut first_columns = work.reserve(scope.len())?;
            for (column, &variable) in scope.iter().enumerate() {
                work.tick(1)?;
                if variable > first_columns.len() {
                    return Err(Cause::Scope);
                }
                if variable == first_columns.len() {
                    work.tick(1)?;
                    first_columns.push(column);
                }
                copied_scope.push(variable);
            }
            let words = relation.row_count().div_ceil(WORD_BITS);
            let mut coherent = work.zeros(words)?;
            for row in 0..relation.row_count() {
                if coherent_row(relation, scope, &first_columns, row, &mut work)? {
                    work.tick(1)?;
                    coherent[row / WORD_BITS] |= 1 << (row % WORD_BITS);
                }
            }
            let mut table = Self {
                relation,
                scope: copied_scope,
                variables: work.reserve(first_columns.len())?,
                values: Vec::new(),
                supports: Vec::new(),
                coherent,
                statistics: Statistics {
                    work: 0,
                    retained_bytes: 0,
                    peak_bytes: 0,
                },
            };
            for &column in &first_columns {
                table.prepare_variable(column, &mut work)?;
            }
            work.release(first_columns);
            work.release_bytes(scratch_headers);
            table.statistics = work.statistics(external);
            Ok(table)
        })();
        result.map_err(|cause| work.failure(cause))
    }

    /// The exact authoritative relation borrowed by this prepared index.
    #[must_use]
    pub const fn relation(&self) -> &'owner Relation<'source> {
        self.relation
    }

    /// Canonical variable label for every argument column, including aliases.
    #[must_use]
    pub fn scope(&self) -> &[usize] {
        &self.scope
    }

    /// Number of independently supplied domains.
    #[must_use]
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Number of distinct variable/value supports, excluding incoherent rows.
    /// Equal values in different variables have separate support entries.
    #[must_use]
    pub fn support_entries(&self) -> usize {
        self.values.len()
    }

    /// Preparation work and retained support-index capacity.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }

    /// Intersect complete rows with the supplied domains and project their values.
    ///
    /// Each domain is a finite list of whole typed values; duplicates have set
    /// meaning and absent values have no support. Every call recomputes from the
    /// immutable base, including after domain widening. For K variables, D
    /// supplied domain values, V indexed entries, A allowed entries and W row
    /// words, work is O(K + (K+1)*W + V + D*(1+log(V+1)+W) + A*W), plus typed
    /// comparison payload costs. The base-mask copy remains even for a nullary
    /// relation; each variable clears and intersects W words even if its domain
    /// is empty.
    ///
    /// # Errors
    /// Refuses a wrong domain count, finite limits, allocation failure or
    /// interrupted control. Existing projections and the table remain unchanged.
    pub fn project(
        &self,
        domains: &[&[Value]],
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Projection<'_, 'owner, 'source>, Failure> {
        self.project_inner(
            domains.iter().map(|domain| Domain::Finite(domain)),
            limits,
            cancellation,
        )
    }

    /// Project borrowed finite, singleton or unrestricted domains.
    ///
    /// This shares row restriction and value projection with [`Self::project`].
    /// The result borrows the prepared table, not the supplied domain storage.
    /// An unrestricted variable permits every indexed value; a singleton needs
    /// one typed lookup and no union scratch. Finite lists retain set meaning.
    ///
    /// # Errors
    /// Refuses invalid domain count, finite limits, allocation failure or
    /// interrupted control without publishing a partial projection.
    pub fn project_domains(
        &self,
        domains: &[Domain<'_>],
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Projection<'_, 'owner, 'source>, Failure> {
        self.project_inner(domains.iter().copied(), limits, cancellation)
    }

    fn project_inner<'domain>(
        &self,
        domains: impl ExactSizeIterator<Item = Domain<'domain>>,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Projection<'_, 'owner, 'source>, Failure> {
        let external = self.retained_inputs()?;
        let scratch_header = size_of::<Vec<u32>>();
        let mut work = Work::new(
            limits,
            cancellation,
            external,
            size_of::<Projection<'_, '_, '_>>() + scratch_header,
        )?;
        let result = (|| {
            self.check_domains(domains.len(), &work)?;
            let mut supported = work.reserve(self.values.len())?;
            work.tick(self.values.len())?;
            supported.resize(self.values.len(), false);
            let rows = self.restrict_rows(domains, Some(&mut supported), &mut work)?;
            for (entry, allowed) in supported.iter_mut().enumerate() {
                work.tick(1)?;
                if *allowed {
                    *allowed = intersects(&rows, self.support(entry), &mut work)?;
                }
            }
            work.release_bytes(scratch_header);
            Ok(Projection {
                table: self,
                rows,
                supported,
                statistics: work.statistics(external),
            })
        })();
        result.map_err(|cause| work.failure(cause))
    }

    fn retained_inputs(&self) -> Result<usize, Failure> {
        self.relation
            .storage()
            .retained_bytes
            .checked_add(self.statistics.retained_bytes)
            .ok_or(Failure {
                cause: Cause::Overflow,
                work: 0,
                peak_bytes: usize::MAX,
            })
    }

    fn check_domains(&self, count: usize, work: &Work<'_>) -> Result<(), Cause> {
        work.entries(self.values.len())?;
        if count == self.variable_count() {
            Ok(())
        } else {
            Err(Cause::Domains)
        }
    }

    fn prepare_variable(&mut self, column: usize, work: &mut Work<'_>) -> Result<(), Cause> {
        let mut rows = work.reserve(self.relation.row_count())?;
        for row in 0..self.relation.row_count() {
            work.tick(1)?;
            if self.coherent[row / WORD_BITS] & (1 << (row % WORD_BITS)) != 0 {
                let value = self
                    .relation
                    .row(row)
                    .and_then(|row| row.value(column))
                    .ok_or(Cause::Relation)?;
                rows.push(RowValue { row, value });
            }
        }
        sort(&mut rows, work)?;
        let start = self.values.len();
        let mut previous = None;
        for row in &rows {
            let distinct = match previous {
                Some(value) => work.compare(value, row.value)? != std::cmp::Ordering::Equal,
                None => true,
            };
            if distinct {
                let entries = self.values.len().checked_add(1).ok_or(Cause::Overflow)?;
                work.entries(entries)?;
                work.grow(&mut self.values, 1)?;
                work.tick(1)?;
                self.values.push(row.value);
                work.grow(&mut self.supports, self.coherent.len())?;
                for _ in 0..self.coherent.len() {
                    work.tick(1)?;
                    self.supports.push(0);
                }
            }
            work.tick(1)?;
            let entry = self.values.len() - 1;
            self.supports[entry * self.coherent.len() + row.row / WORD_BITS] |=
                1 << (row.row % WORD_BITS);
            previous = Some(row.value);
        }
        work.tick(1)?;
        self.variables.push(start..self.values.len());
        work.release(rows);
        Ok(())
    }

    fn support(&self, entry: usize) -> &[u32] {
        let start = entry * self.coherent.len();
        &self.supports[start..start + self.coherent.len()]
    }

    fn lookup(
        &self,
        variable: usize,
        value: &Value,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Cause> {
        let mut range = self.variables[variable].clone();
        while range.start < range.end {
            let middle = range.start + (range.end - range.start) / 2;
            match work.compare(self.values[middle], value)? {
                std::cmp::Ordering::Less => range.start = middle + 1,
                std::cmp::Ordering::Greater => range.end = middle,
                std::cmp::Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
    }
}

impl<'table, 'owner, 'source> Projection<'table, 'owner, 'source> {
    /// Prepared owner whose row positions and value references this result uses.
    #[must_use]
    pub const fn table(&self) -> &'table Table<'owner, 'source> {
        self.table
    }

    /// Low-bit-first surviving original row occurrences, with zero unused tail bits.
    #[must_use]
    pub fn words(&self) -> &[u32] {
        &self.rows
    }

    /// Whether one in-range original occurrence survives.
    #[must_use]
    pub fn contains(&self, row: usize) -> bool {
        row < self.table.relation.row_count()
            && self.rows[row / WORD_BITS] & (1 << (row % WORD_BITS)) != 0
    }

    /// Borrow supported typed values for one variable, in typed value order.
    /// Unknown variable indices are explicit absence. This allocates no storage.
    #[must_use]
    pub fn domain(&self, variable: usize) -> Option<impl Iterator<Item = &'source Value> + '_> {
        self.table.variables.get(variable).map(|range| {
            range
                .clone()
                .filter(|&index| self.supported[index])
                .map(|index| self.table.values[index])
        })
    }

    /// Projection work and retained result capacity, excluding prepared inputs.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}

fn coherent_row(
    relation: &Relation<'_>,
    scope: &[usize],
    first_columns: &[usize],
    row: usize,
    work: &mut Work<'_>,
) -> Result<bool, Cause> {
    for (column, &variable) in scope.iter().enumerate() {
        work.tick(1)?;
        let previous = first_columns[variable];
        if previous != column {
            work.tick(1)?;
            if relation.column(column).ok_or(Cause::Relation)?[row]
                != relation.column(previous).ok_or(Cause::Relation)?[row]
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn intersects(left: &[u32], right: &[u32], work: &mut Work<'_>) -> Result<bool, Cause> {
    for (&left, &right) in left.iter().zip(right) {
        work.tick(1)?;
        if left & right != 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn sort(rows: &mut Vec<RowValue<'_>>, work: &mut Work<'_>) -> Result<(), Cause> {
    let mut scratch = work.reserve(rows.len())?;
    for &row in rows.iter() {
        work.tick(1)?;
        scratch.push(row);
    }
    let mut width = 1;
    while width < rows.len() {
        let mut start = 0;
        while start < rows.len() {
            let middle = start.saturating_add(width).min(rows.len());
            let end = middle.saturating_add(width).min(rows.len());
            let mut left = start;
            let mut right = middle;
            for output in &mut scratch[start..end] {
                work.tick(1)?;
                let take_left = left < middle
                    && (right == end
                        || work.compare(rows[left].value, rows[right].value)?
                            != std::cmp::Ordering::Greater);
                *output = rows[if take_left {
                    let index = left;
                    left += 1;
                    index
                } else {
                    let index = right;
                    right += 1;
                    index
                }];
            }
            start = end;
        }
        std::mem::swap(rows, &mut scratch);
        width = width.saturating_mul(2);
    }
    work.release(scratch);
    Ok(())
}

#[cfg(test)]
mod tests;
