//! Appendable extensional tuple ownership with immutable relation views.

use std::mem::size_of;

use crate::{Atom, AtomKey, Predicate, Value};

use super::{
    Cell, Failure, Layout, LayoutOwner, Limits, Relation, Resource, Source, Storage, Work, ceiling,
    storage,
};

/// One signed predicate's unique typed atoms and appendable equality layout.
///
/// The catalog owns every atom once; dictionary entries are stable source-cell
/// positions. An immutable view borrows both owners, preventing mutation while
/// any query, row or selection is live. Existing rows and equality IDs survive
/// insertion; query identity remains the particular immutable Relation object.
///
/// Byte limits cover the catalog object, atom-vector cells, row index, equality
/// layout and operation scratch. The supplied atoms' nested payload allocations
/// remain the source admission caller's responsibility, as for borrowed views.
/// They are reported conservatively as referenced payload, not unique RSS.
/// Membership uses full signed predicate and typed tuple equality. Sorted row
/// and dictionary indexes move IDs only; their worst-case insertion work is
/// linear in existing rows/values. Column append has amortized linear total
/// copying across a growth sequence, separate from those sorted index shifts.
pub struct Catalog {
    predicate: Predicate,
    atoms: Vec<Atom>,
    rows: Vec<usize>,
    layout: Layout,
    payload: u128,
    construction: Storage,
}

/// The outcome and operation-scoped accounting of one complete insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insertion {
    /// Stable local row ID, including when the tuple was already present.
    pub row: usize,
    /// Whether a new tuple was published.
    pub inserted: bool,
    /// Current capacity and work performed by this operation only.
    pub storage: Storage,
}

/// Exact membership result with the work needed to establish it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lookup {
    /// Original row ID when a complete typed tuple is present.
    pub row: Option<usize>,
    /// Current capacity and operation-scoped work.
    pub storage: Storage,
}

/// A catalog operation failed after the reported completed work.
/// Logical membership is unchanged; existing owners may retain reserved capacity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFailure {
    /// Original shape, limit or allocation cause. A failed work charge retains
    /// its proposed count; `work` includes only work charged before that attempt.
    pub error: Failure,
    /// Completed charged work before failure.
    pub work: u128,
    /// Current retained owner capacity; zero when construction returns no owner.
    pub retained_bytes: usize,
}

impl std::fmt::Display for CatalogFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for CatalogFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

struct NewValue {
    id: u32,
    column: usize,
    position: usize,
}

struct Plan {
    ids: Vec<u32>,
    added: Vec<NewValue>,
    ordered: Vec<usize>,
    payload: u128,
}

impl Catalog {
    /// Create an empty signed relation owner, including nullary relations.
    ///
    /// # Errors
    /// Refuses shape, byte/work limits or allocation failure without an owner.
    pub fn new(predicate: Predicate, limits: Limits) -> Result<Self, CatalogFailure> {
        let mut work =
            Work::new(limits, size_of::<Self>() as u128).map_err(|error| CatalogFailure {
                error,
                work: 0,
                retained_bytes: 0,
            })?;
        let build = (|| {
            ceiling(
                Resource::Columns,
                predicate.arity() as u128,
                limits.max_columns as u128,
            )?;
            let mut columns = work.reserve(predicate.arity())?;
            for _ in 0..predicate.arity() {
                work.tick(1)?;
                columns.push(Vec::new());
            }
            Ok(Self {
                predicate,
                atoms: Vec::new(),
                rows: Vec::new(),
                layout: Layout {
                    dictionary: Vec::new(),
                    ordered: Vec::new(),
                    columns,
                },
                payload: 0,
                construction: Storage {
                    retained_bytes: work.live,
                    peak_construction_bytes: work.peak,
                    referenced_payload_bytes: 0,
                    borrowed_mapping_bytes: 0,
                    construction_work: work.used,
                },
            })
        })();
        build.map_err(|error| CatalogFailure {
            error,
            work: work.used,
            retained_bytes: 0,
        })
    }

    /// Initial empty-owner construction receipt. It does not change on insertion.
    #[must_use]
    pub const fn construction(&self) -> Storage {
        self.construction
    }

    /// Full signed predicate identity. Constant time.
    #[must_use]
    pub const fn predicate(&self) -> &Predicate {
        &self.predicate
    }

    /// Unique typed tuples in stable insertion order. Constant-time borrow.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    /// Borrow a tuple by ascending typed storage order in constant time.
    /// This sorted position differs from the stable insertion row ID and may
    /// change on append. Ordering is [`Value`](crate::Value)'s [`Ord`], not
    /// ASP term order.
    #[must_use]
    pub fn ordered_row(&self, position: usize) -> Option<&Atom> {
        self.rows.get(position).and_then(|&row| self.atoms.get(row))
    }

    /// Consume the owner without copying atoms or their nested payloads.
    /// The returned sequence retains insertion order.
    #[must_use]
    pub fn into_atoms(self) -> Vec<Atom> {
        self.atoms
    }

    /// Borrow the current layout without rebuilding its dictionary or columns.
    /// The source lifetime prevents append while this immutable view is borrowed.
    /// View accounting includes only its own object: the catalog's retained
    /// capacity remains the caller's separate owner charge.
    #[must_use]
    pub fn view(&self) -> Relation<'_> {
        Relation {
            predicate: &self.predicate,
            source: Source::Atoms(&self.atoms),
            layout: LayoutOwner::Borrowed(&self.layout),
            storage: Storage {
                retained_bytes: size_of::<Relation<'_>>(),
                peak_construction_bytes: size_of::<Relation<'_>>(),
                referenced_payload_bytes: self.payload,
                borrowed_mapping_bytes: 0,
                construction_work: 0,
            },
        }
    }

    /// Check exact typed tuple membership without constructing another atom.
    ///
    /// # Errors
    /// Refuses foreign predicates or work/byte ceilings. Comparisons include
    /// nested typed-value payload costs and do not rely on equality IDs alone.
    pub fn lookup(&self, atom: &Atom, limits: Limits) -> Result<Lookup, CatalogFailure> {
        let mut work = self.work(limits).map_err(|error| self.failed(error, 0))?;
        self.position(atom, &mut work)
            .map(|position| Lookup {
                row: position.ok(),
                storage: self.receipt(&work),
            })
            .map_err(|error| self.failed(error, work.used))
    }

    /// Look up a borrowed substitution with the same comparison and accounting
    /// as [`Self::lookup`]. No atom or value payload is copied.
    ///
    /// # Errors
    /// Refuses foreign predicates or work/byte ceilings, preserving completed
    /// comparison work in the failure receipt.
    pub fn lookup_key(&self, key: &AtomKey<'_>, limits: Limits) -> Result<Lookup, CatalogFailure> {
        let mut work = self.work(limits).map_err(|error| self.failed(error, 0))?;
        self.position_values(key.predicate(), |column| key.argument(column), &mut work)
            .map(|position| Lookup {
                row: position.ok(),
                storage: self.receipt(&work),
            })
            .map_err(|error| self.failed(error, work.used))
    }

    /// Insert an atom only after every required reservation and check succeeds.
    ///
    /// # Errors
    /// Refuses predicate mismatch, inclusive limits, overflow or allocation
    /// failure. Logical contents and all published row/equality IDs remain
    /// unchanged on failure; successful reservations may retain spare capacity.
    /// Subsequent views and admissions include that actual retained capacity.
    pub fn insert(&mut self, atom: Atom, limits: Limits) -> Result<Insertion, CatalogFailure> {
        let mut work = self.work(limits).map_err(|error| self.failed(error, 0))?;
        self.insert_inner(atom, &mut work)
            .map_err(|error| self.failed(error, work.used))
    }

    fn insert_inner(&mut self, atom: Atom, work: &mut Work) -> Result<Insertion, Failure> {
        let position = match self.position(&atom, work)? {
            Ok(row) => {
                return Ok(Insertion {
                    row,
                    inserted: false,
                    storage: self.receipt(work),
                });
            }
            Err(position) => position,
        };
        ceiling(
            Resource::Rows,
            self.atoms.len() as u128 + 1,
            work.limits.max_rows as u128,
        )?;
        let plan = self.plan(&atom, work)?;
        self.reserve(&plan, position, work)?;
        Ok(self.publish(atom, position, plan, work))
    }

    fn plan(&self, atom: &Atom, work: &mut Work) -> Result<Plan, Failure> {
        let mut ids = work.reserve(self.predicate.arity())?;
        let mut added: Vec<NewValue> = work.reserve(self.predicate.arity())?;
        let mut ordered: Vec<usize> = work.reserve(self.predicate.arity())?;
        let source = Source::Atoms(&self.atoms);
        let mut payload = self.payload;
        for (column, value) in atom.values().iter().enumerate() {
            work.tick(1 + value.payload_bytes() as u128)?;
            payload = payload
                .checked_add(value.payload_bytes() as u128)
                .ok_or(Failure::Overflow)?;
            let id = match storage::lookup(&self.layout, &source, value, work)? {
                Ok(id) => id,
                Err(insertion) => {
                    let mut previous = None;
                    for (index, entry) in added.iter().enumerate() {
                        if work.compare(&atom.values()[entry.column], value)?.is_eq() {
                            previous = Some(index);
                            break;
                        }
                    }
                    let index = if let Some(index) = previous {
                        index
                    } else {
                        let index = added.len();
                        let mut rank = ordered.len();
                        for (rank_index, &other) in ordered.iter().enumerate() {
                            if work
                                .compare(value, &atom.values()[added[other].column])?
                                .is_lt()
                            {
                                rank = rank_index;
                                break;
                            }
                        }
                        work.tick((ordered.len() - rank + 1) as u128)?;
                        ordered.insert(rank, index);
                        added.push(NewValue {
                            id: u32::try_from(
                                self.layout
                                    .dictionary
                                    .len()
                                    .checked_add(index)
                                    .ok_or(Failure::Overflow)?,
                            )
                            .map_err(|_| Failure::Overflow)?,
                            column,
                            position: insertion,
                        });
                        index
                    };
                    added[index].id
                }
            };
            work.tick(1)?;
            ids.push(id);
        }
        Ok(Plan {
            ids,
            added,
            ordered,
            payload,
        })
    }

    fn reserve(&mut self, plan: &Plan, position: usize, work: &mut Work) -> Result<(), Failure> {
        let Plan {
            ids,
            added,
            ordered,
            ..
        } = plan;
        ceiling(
            Resource::Values,
            self.layout.dictionary.len() as u128 + added.len() as u128,
            (work.limits.max_values as u128).min(u128::from(u32::MAX) + 1),
        )?;
        work.grow(&mut self.atoms, 1)?;
        work.grow(&mut self.rows, 1)?;
        work.grow(&mut self.layout.dictionary, added.len())?;
        work.grow(&mut self.layout.ordered, added.len())?;
        for column in &mut self.layout.columns {
            work.grow(column, 1)?;
        }
        // Charge every impending mutation before publishing any logical state.
        let mut writes =
            (self.rows.len() - position + 1) as u128 + ids.len() as u128 + added.len() as u128 + 1;
        for (offset, &index) in ordered.iter().enumerate() {
            let insertion = added[index].position + offset;
            writes += (self.layout.ordered.len() + offset - insertion + 1) as u128;
        }
        work.tick(writes)?;
        Ok(())
    }

    fn publish(&mut self, atom: Atom, position: usize, plan: Plan, work: &mut Work) -> Insertion {
        let Plan {
            ids,
            added,
            ordered,
            payload,
        } = plan;
        let row = self.atoms.len();
        for value in &added {
            self.layout.dictionary.push(Cell {
                row,
                column: value.column,
            });
        }
        for (offset, &index) in ordered.iter().enumerate() {
            self.layout
                .ordered
                .insert(added[index].position + offset, added[index].id);
        }
        for (column, id) in self.layout.columns.iter_mut().zip(ids.iter().copied()) {
            column.push(id);
        }
        self.rows.insert(position, row);
        self.atoms.push(atom);
        self.payload = payload;
        work.release(ids);
        work.release(added);
        work.release(ordered);
        Insertion {
            row,
            inserted: true,
            storage: self.receipt(work),
        }
    }

    fn position(&self, atom: &Atom, work: &mut Work) -> Result<Result<usize, usize>, Failure> {
        self.position_values(atom.predicate(), |column| &atom.values()[column], work)
    }

    fn position_values<'value>(
        &self,
        predicate: &Predicate,
        value: impl Fn(usize) -> &'value Value,
        work: &mut Work,
    ) -> Result<Result<usize, usize>, Failure> {
        work.tick(1 + self.predicate.name().len() as u128 + predicate.name().len() as u128)?;
        if predicate != &self.predicate {
            return Err(Failure::Predicate);
        }
        let mut start = 0;
        let mut end = self.rows.len();
        while start < end {
            let middle = start + (end - start) / 2;
            let row = self.rows[middle];
            let mut order = std::cmp::Ordering::Equal;
            for (column, left) in self.atoms[row].values().iter().enumerate() {
                order = work.compare(left, value(column))?;
                if !order.is_eq() {
                    break;
                }
            }
            match order {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Equal => return Ok(Ok(row)),
                std::cmp::Ordering::Greater => end = middle,
            }
        }
        Ok(Err(start))
    }

    fn work(&self, limits: Limits) -> Result<Work, Failure> {
        ceiling(
            Resource::Rows,
            self.atoms.len() as u128,
            limits.max_rows as u128,
        )?;
        ceiling(
            Resource::Columns,
            self.predicate.arity() as u128,
            limits.max_columns as u128,
        )?;
        ceiling(
            Resource::Values,
            self.layout.dictionary.len() as u128,
            limits.max_values as u128,
        )?;
        Work::new(limits, self.retained_bytes() as u128)
    }

    fn failed(&self, error: Failure, work: u128) -> CatalogFailure {
        CatalogFailure {
            error,
            work,
            retained_bytes: self.retained_bytes(),
        }
    }

    /// Current owner capacity, including reservations retained after a refusal.
    /// Inspects one capacity per column; logical values and rows are not visited.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.atoms.capacity() * size_of::<Atom>()
            + self.rows.capacity() * size_of::<usize>()
            + self.layout.dictionary.capacity() * size_of::<Cell>()
            + self.layout.ordered.capacity() * size_of::<u32>()
            + self.layout.columns.capacity() * size_of::<Vec<u32>>()
            + self
                .layout
                .columns
                .iter()
                .map(|column| column.capacity() * size_of::<u32>())
                .sum::<usize>()
    }

    fn receipt(&self, work: &Work) -> Storage {
        Storage {
            retained_bytes: self.retained_bytes(),
            peak_construction_bytes: work.peak,
            referenced_payload_bytes: self.payload,
            borrowed_mapping_bytes: 0,
            construction_work: work.used,
        }
    }
}

#[cfg(test)]
mod tests;
