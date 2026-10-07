//! An exclusive append session retains one tuple transaction's metadata.
//!
//! Every call commits at most one row. Earlier successful calls remain published
//! after a later refusal; dropping the session neither commits nor rolls back.

use std::mem::size_of;

use crate::catalog::AtomRef;

use super::{Catalog, CatalogFailure, Insertion, Limits, plan};

/// Reusable insertion preparation over one existing relation catalog.
///
/// This owns only transaction metadata and exclusively borrows the catalog.
/// It retains no canonical payload or second identity index. Tuple lookup and
/// publication use the same engine as [`Catalog::insert`]. Work remains charged
/// per insertion; reusing allocations does not erase comparisons or writes.
/// Dropping the appender frees scratch but preserves every completed insertion.
pub struct Appender<'catalog> {
    catalog: &'catalog mut Catalog,
    plan: plan::Plan,
}

/// One complete insertion and its temporarily borrowed equality coordinates.
///
/// The borrow prevents another session insertion until these coordinates have
/// been consumed. Equality IDs belong to this catalog, never to another owner.
#[derive(Debug)]
pub struct Appended<'step> {
    /// Same stable row identity and insertion flag as scalar insertion.
    pub insertion: Insertion,
    /// The newly published row's columns, in argument order. A duplicate has
    /// no new columns; a newly inserted nullary tuple has `Some(&[])`.
    pub equality_ids: Option<&'step [u32]>,
    /// Session header and actual scratch capacity, excluding the catalog.
    pub scratch_bytes: usize,
}

impl Catalog {
    /// Borrow this catalog for repeated checked, individually atomic insertion.
    ///
    /// The session retains its tuple-plan vectors between calls. `limits`
    /// include both the catalog and the session header. Each subsequent call
    /// admits its current complete envelope against its supplied limits, so a
    /// caller can account for other owners growing between insertions.
    ///
    /// # Errors
    /// Refuses current population or byte limits before creating the session.
    /// No row, equality ID or retained catalog capacity changes.
    pub fn appender(&mut self, limits: Limits) -> Result<Appender<'_>, CatalogFailure> {
        self.work_with(limits, size_of::<Appender<'_>>())
            .map_err(|mut error| {
                // The proposed session has no retained header on refusal.
                error.peak_construction_bytes = error.retained_bytes;
                error
            })?;
        Ok(Appender {
            catalog: self,
            plan: plan::Plan::default(),
        })
    }
}

impl Appender<'_> {
    /// Current session header and scratch capacities, excluding catalog storage.
    /// Empty logical scratch retains its allocation, including after refusal.
    #[must_use]
    pub fn scratch_bytes(&self) -> usize {
        size_of::<Self>() + self.plan.capacity_bytes()
    }

    /// Insert one atom using the existing canonical owner and transaction engine.
    ///
    /// Limits and operation peak include catalog plus session scratch and actual
    /// growth overlap. The result's retained catalog bytes exclude this session;
    /// `scratch_bytes` reports its separate retained component. Work is for this
    /// call only. Repeated calls must receive the caller's remaining work budget.
    ///
    /// # Errors
    /// Returns the same typed owner, shape, allocation, overflow and limit errors
    /// as scalar insertion. Only this call's logical insertion is atomic: earlier
    /// successful rows remain present. Catalog and scratch reservations may remain
    /// after refusal. Inspect [`Self::scratch_bytes`] on an error; the error's
    /// retained bytes name only the catalog and its peak includes both owners.
    pub fn insert(
        &mut self,
        atom: AtomRef<'_>,
        limits: Limits,
    ) -> Result<Appended<'_>, CatalogFailure> {
        let mut work = match self.catalog.work_with(limits, self.scratch_bytes()) {
            Ok(work) => work,
            Err(error) => {
                self.plan.clear();
                return Err(error);
            }
        };
        let insertion = match self.catalog.insert_inner(
            atom,
            &mut self.plan,
            plan::Admission::Retained,
            &mut work,
        ) {
            Ok(insertion) => insertion,
            Err(error) => {
                self.plan.clear();
                return Err(self.catalog.failed(error, &work));
            }
        };
        Ok(Appended {
            insertion,
            equality_ids: insertion.inserted.then_some(self.plan.ids.as_slice()),
            scratch_bytes: self.scratch_bytes(),
        })
    }
}

#[cfg(test)]
mod tests;
