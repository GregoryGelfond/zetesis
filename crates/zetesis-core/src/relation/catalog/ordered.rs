//! Explicit ordered access prepared once for one published tuple extent.

use std::mem::size_of;

use crate::{Atom, ordered_index::position};

use super::{Catalog, CatalogFailure, Failure, Limits, Storage, Work};

/// A prepared canonical row view borrowing the authoritative catalog.
///
/// Positions are typed storage-order ranks, not stable insertion IDs or ASP
/// term-order ranks. The borrow prevents append. Row lookup is constant time;
/// preparation's linear traversal and ID capacity are reported separately.
#[derive(Clone, Copy)]
pub struct OrderedRows<'a> {
    catalog: &'a Catalog,
    storage: Storage,
}

impl<'a> OrderedRows<'a> {
    /// Number of rows in this complete ordered view. Constant time.
    #[must_use]
    pub fn len(self) -> usize {
        self.catalog.ordered.len()
    }

    /// Whether this complete view contains no rows. Constant time.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.catalog.ordered.is_empty()
    }

    /// Stable catalog-local insertion row ID at an ordered position.
    ///
    /// This is one bounds-checked index read, with no lookup, allocation or
    /// payload comparison. A row ID belongs only to this catalog: append
    /// preserves it, while extraction invalidates it. Ordered positions can
    /// move after append. Out-of-range positions return `None`.
    #[must_use]
    pub fn row_id(self, position: usize) -> Option<usize> {
        self.catalog.ordered.get(position).copied()
    }

    /// Borrow one authoritative atom by ordered position. Constant time.
    #[must_use]
    pub fn get(self, position: usize) -> Option<&'a Atom> {
        self.row_id(position)
            .and_then(|row| self.catalog.atoms.get(row))
    }

    /// Current owner capacity and work of the preparation or reuse operation.
    #[must_use]
    pub const fn storage(self) -> Storage {
        self.storage
    }
}

impl Catalog {
    /// Prepare complete typed-order row access without cloning tuple payload.
    ///
    /// A prepared extent is reused with zero traversal work. The first
    /// preparation of a nonempty extent traverses the row index once, O(n).
    /// A later preparation merges the `d` rows appended since the previous
    /// one: it sorts them by typed comparison, O(d log d), locates each among
    /// the previous view by binary search, O(d log n) comparisons, and writes
    /// the merged view in one linear pass of n + d ID copies with no
    /// comparison. The previous view and the sorted run stay borrowable
    /// through [`Self::ordered_runs`]. The two views and the run are this
    /// owner's retained capacity; a temporary scratch buffer of `d` IDs is
    /// released before return. Only successful insertion invalidates a view; a
    /// refused insertion or a refused preparation preserves the previous one.
    ///
    /// # Errors
    /// Current shape, bytes and work must fit `limits`. Refused preparation does
    /// not publish a partial ordered view or change atom/equality identity.
    pub fn prepare_ordered(&mut self, limits: Limits) -> Result<OrderedRows<'_>, CatalogFailure> {
        let mut work = self.work(limits)?;
        if self.ordered.len() != self.atoms.len() {
            let prepare = if self.ordered.is_empty() {
                // A refused traversal leaves no partial view: the vector is
                // either a complete prepared extent or empty.
                self.traverse(&mut work)
                    .inspect_err(|_| self.ordered.clear())
            } else {
                self.merge(&mut work)
            };
            prepare.map_err(|error| self.failed(error, &work))?;
        }
        Ok(OrderedRows {
            catalog: self,
            storage: self.receipt(&work),
        })
    }

    /// Borrow an already prepared extent without allocating or traversing rows.
    /// `None` means a successful append requires a new preparation. It does not
    /// mean this relation has no tuples. A new empty catalog is already prepared.
    #[must_use]
    pub fn ordered(&self) -> Option<OrderedRows<'_>> {
        (self.ordered.len() == self.atoms.len()).then(|| OrderedRows {
            catalog: self,
            storage: Storage {
                retained_bytes: self.retained_bytes(),
                peak_construction_bytes: self.retained_bytes(),
                referenced_payload_bytes: self.payload,
                borrowed_mapping_bytes: 0,
                construction_work: 0,
            },
        })
    }

    /// The two runs of the last merging preparation, each in canonical order:
    /// the row IDs of the view it started from, and the IDs it merged in.
    /// Together they are the prepared extent. The first preparation starts
    /// from nothing, so its run is the whole view. `None` exactly when
    /// [`Self::ordered`] is `None`.
    #[must_use]
    pub fn ordered_runs(&self) -> Option<(&[usize], &[usize])> {
        self.ordered().map(|_| {
            if self.previous.is_empty() {
                (&[][..], &self.ordered[..])
            } else {
                (&self.previous[..], &self.run[..])
            }
        })
    }

    /// Build the first view by an in-order traversal of the row index.
    fn traverse(&mut self, work: &mut Work) -> Result<(), Failure> {
        let additional = self.atoms.len().saturating_sub(self.ordered.len());
        work.grow(&mut self.ordered, additional)?;
        work.include(size_of::<Vec<usize>>())?;
        let mut path = work.reserve::<usize>(0)?;
        self.ordered.clear();
        self.previous.clear();
        self.run.clear();
        let mut cursor = self.rows.root;
        loop {
            while let Some(link) = cursor {
                work.tick(1)?;
                let id = position(link);
                work.grow(&mut path, 1)?;
                work.tick(1)?;
                path.push(id);
                cursor = self.rows.nodes[id].children[0];
            }
            let Some(id) = path.pop() else {
                break;
            };
            work.tick(1)?;
            self.ordered.push(id);
            cursor = self.rows.nodes[id].children[1];
        }
        work.release(path);
        work.live -= size_of::<Vec<usize>>();
        Ok(())
    }

    /// Merge the rows appended since the last preparation into the view.
    /// Every fallible step precedes the swap that publishes the new view, so a
    /// refusal leaves the previous view, its runs and the atom count coherent.
    fn merge(&mut self, work: &mut Work) -> Result<(), Failure> {
        let appended = self.ordered.len()..self.atoms.len();
        let pending = appended.len();
        self.run.clear();
        work.grow(&mut self.run, pending)?;
        self.run.extend(appended);
        work.tick(pending as u128)?;
        let mut scratch = work.reserve::<usize>(pending)?;
        self.sort_run(&mut scratch, work)?;
        // Insertion points into the previous view, nondecreasing along the
        // sorted run, so one forward pass places every run ID.
        scratch.clear();
        for index in 0..pending {
            let id = self.run[index];
            scratch.push(self.insertion_point(id, work)?);
        }
        let merged = self.ordered.len() + pending;
        self.previous.clear();
        work.grow(&mut self.previous, merged)?;
        work.tick(merged as u128)?;
        let mut copied = 0;
        for (id, &point) in self.run.iter().zip(&scratch) {
            self.previous
                .extend_from_slice(&self.ordered[copied..point]);
            self.previous.push(*id);
            copied = point;
        }
        self.previous.extend_from_slice(&self.ordered[copied..]);
        work.release(scratch);
        std::mem::swap(&mut self.ordered, &mut self.previous);
        Ok(())
    }

    /// Bottom-up merge sort of the run by typed row comparison; `scratch` has
    /// capacity for the whole run. O(d log d) comparisons, each charged.
    fn sort_run(&mut self, scratch: &mut Vec<usize>, work: &mut Work) -> Result<(), Failure> {
        let length = self.run.len();
        let mut width = 1;
        while width < length {
            scratch.clear();
            let mut start = 0;
            while start < length {
                let middle = (start + width).min(length);
                let end = (start + 2 * width).min(length);
                let (mut left, mut right) = (start, middle);
                while left < middle && right < end {
                    if self.order(self.run[right], self.run[left], work)?.is_lt() {
                        scratch.push(self.run[right]);
                        right += 1;
                    } else {
                        scratch.push(self.run[left]);
                        left += 1;
                    }
                }
                scratch.extend_from_slice(&self.run[left..middle]);
                scratch.extend_from_slice(&self.run[right..end]);
                start = end;
            }
            work.tick(length as u128)?;
            std::mem::swap(&mut self.run, scratch);
            width *= 2;
        }
        Ok(())
    }

    /// Position in the previous view before which row `id` sorts.
    fn insertion_point(&self, id: usize, work: &mut Work) -> Result<usize, Failure> {
        let (mut low, mut high) = (0, self.ordered.len());
        while low < high {
            let middle = low + (high - low) / 2;
            if self.order(self.ordered[middle], id, work)?.is_lt() {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }

    /// Typed identity order of two rows, charged per compared value.
    fn order(
        &self,
        left: usize,
        right: usize,
        work: &mut Work,
    ) -> Result<std::cmp::Ordering, Failure> {
        work.tick(1)?;
        for (left, right) in self.atoms[left]
            .values()
            .iter()
            .zip(self.atoms[right].values())
        {
            let order = work.compare(left, right)?;
            if !order.is_eq() {
                return Ok(order);
            }
        }
        Ok(std::cmp::Ordering::Equal)
    }
}
