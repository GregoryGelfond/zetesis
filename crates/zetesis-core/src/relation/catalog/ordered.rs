//! Explicit ordered access prepared once for one published tuple extent.

use std::mem::size_of;

use crate::{Atom, ordered_index::position};

use super::{Catalog, CatalogFailure, Limits, Storage};

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

    /// Borrow one authoritative atom by ordered position. Constant time.
    #[must_use]
    pub fn get(self, position: usize) -> Option<&'a Atom> {
        self.catalog
            .ordered
            .get(position)
            .and_then(|&row| self.catalog.atoms.get(row))
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
    /// A changed extent requires O(n) ID traversal plus admitted vector growth.
    /// A prepared extent is reused with zero traversal work. The cache and any
    /// allocator slack stay in this owner's retained capacity. Only successful
    /// insertion invalidates it; a refused insertion preserves a prepared view.
    ///
    /// # Errors
    /// Current shape, bytes and work must fit `limits`. Refused preparation does
    /// not publish a partial ordered view or change atom/equality identity.
    pub fn prepare_ordered(&mut self, limits: Limits) -> Result<OrderedRows<'_>, CatalogFailure> {
        let mut work = self.work(limits)?;
        if !self.ordered_valid {
            let prepare = (|| {
                let additional = self.atoms.len().saturating_sub(self.ordered.len());
                work.grow(&mut self.ordered, additional)?;
                work.include(size_of::<Vec<usize>>())?;
                let mut path = work.reserve::<usize>(0)?;
                self.ordered.clear();
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
                self.ordered_valid = true;
                Ok(())
            })();
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
        self.ordered_valid.then(|| OrderedRows {
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
}
