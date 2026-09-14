//! Transactional canonical payload accounting for retained interpretations.
//!
//! Catalogs are charged once per allocation, selections once per retained entry,
//! and caller-described associated payload separately. This is not an allocator
//! or RSS bound: vector/hash capacity, index entries, Arc envelopes, subjects and
//! transient candidates remain outside this portable measure.

use std::{collections::HashMap, fmt};

use crate::{AtomCatalog, Model};

/// Canonical payload admitted by a [`ModelRetention`] ledger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetainedPayload {
    /// Distinct catalog allocations, including an empty catalog if retained.
    pub catalogs: usize,
    /// Whole canonical catalogs, including unselected atoms, counted once each.
    pub catalog_bytes: usize,
    /// Canonical position records, counted separately for every retained entry.
    pub selection_bytes: usize,
    /// Caller-supplied canonical payload, such as objective score records.
    pub associated_bytes: usize,
    /// Checked sum of the three byte subtotals.
    pub bytes: usize,
}

/// Owner-aware accounting for an append-only retained family or its replacement.
///
/// The private hash index holds the actual catalog handle beside its allocation
/// identity. Equal-content distinct owners remain distinct; a live handle keeps
/// its address from being reused. No atom payload is cloned. Expected amortized
/// lookup is constant time, with hash-table worst-case linear lookup; metadata
/// space is linear in distinct retained owners. Replacement/clear releases each
/// old handle and costs linear time in retained index capacity.
///
/// This ledger does not retain model selections or establish answer-set
/// membership. The consumer must retain the corresponding model entries and
/// describe associated payload honestly. Prepare an admission, reserve the
/// consumer's entry slot, then commit and publish without another fallible step.
/// A refused or abandoned admission leaves the old payload and owner set intact;
/// successful reservation can still grow uncharged index capacity.
#[derive(Debug, Default)]
pub struct ModelRetention {
    owners: HashMap<usize, AtomCatalog>,
    payload: RetainedPayload,
}

impl ModelRetention {
    /// Current committed accounting. Pending admissions are not included.
    #[must_use]
    pub const fn payload(&self) -> RetainedPayload {
        self.payload
    }

    /// Prepare one additional retained entry without publishing its charge.
    /// `associated_bytes` is the additional caller-owned canonical payload.
    /// The ceiling applies to the complete resulting payload, including when
    /// the caller lowers it below a previously admitted total.
    ///
    /// # Errors
    /// Returns checked overflow, the complete prospective byte-limit refusal,
    /// or owner-index allocation failure. No partial admission is committed.
    pub fn admit(
        &mut self,
        model: &Model,
        associated_bytes: usize,
        max_bytes: usize,
    ) -> Result<RetentionAdmission<'_>, RetentionError> {
        self.prepare(model, associated_bytes, max_bytes, false)
    }

    /// Prepare replacement of the complete retained family by one model entry.
    /// The new owner's full catalog and supplied associated payload are charged
    /// against an empty ledger; old owners remain live until commit. The byte
    /// limit describes the new retained payload, not old/new transient overlap.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::admit`], preserving the old family
    /// accounting on every refusal and when the returned admission is dropped.
    pub fn replace(
        &mut self,
        model: &Model,
        associated_bytes: usize,
        max_bytes: usize,
    ) -> Result<RetentionAdmission<'_>, RetentionError> {
        self.prepare(model, associated_bytes, max_bytes, true)
    }

    /// Release all catalog handles and reset accounting, retaining index capacity.
    /// The caller must also release or transfer the associated retained entries.
    pub fn clear(&mut self) {
        self.owners.clear();
        self.payload = RetainedPayload::default();
    }

    fn prepare(
        &mut self,
        model: &Model,
        associated_bytes: usize,
        max_bytes: usize,
        replacement: bool,
    ) -> Result<RetentionAdmission<'_>, RetentionError> {
        let key = model.catalog().owner_key();
        let new_owner = replacement || !self.owners.contains_key(&key);
        let mut payload = if replacement {
            RetainedPayload::default()
        } else {
            self.payload
        };
        if new_owner {
            payload.catalogs = add(payload.catalogs, 1)?;
            payload.catalog_bytes = add(
                payload.catalog_bytes,
                model
                    .catalog()
                    .retained_payload_bytes()
                    .ok_or(RetentionError::Overflow)?,
            )?;
        }
        payload.selection_bytes = add(
            payload.selection_bytes,
            model
                .selection_payload_bytes()
                .ok_or(RetentionError::Overflow)?,
        )?;
        payload.associated_bytes = add(payload.associated_bytes, associated_bytes)?;
        payload.bytes = add(
            add(payload.catalog_bytes, payload.selection_bytes)?,
            payload.associated_bytes,
        )?;
        if payload.bytes > max_bytes {
            return Err(RetentionError::Bytes {
                required: payload.bytes,
                limit: max_bytes,
            });
        }
        // Replacement can reuse any existing slot, but an empty index still
        // needs one. No reserve or fallible operation remains after commit starts.
        self.owners
            .try_reserve(payload.catalogs.saturating_sub(self.owners.len()))
            .map_err(|_| RetentionError::Allocation)?;
        Ok(RetentionAdmission {
            ledger: self,
            owner: new_owner.then(|| model.catalog().clone()),
            payload,
            replacement,
        })
    }
}

fn add(left: usize, right: usize) -> Result<usize, RetentionError> {
    left.checked_add(right).ok_or(RetentionError::Overflow)
}

/// Exclusive pending admission; dropping it abandons publication.
/// The borrow prevents intervening ledger changes from making its charge stale.
#[derive(Debug)]
#[must_use = "commit only after all consumer admission steps succeed"]
pub struct RetentionAdmission<'a> {
    ledger: &'a mut ModelRetention,
    owner: Option<AtomCatalog>,
    payload: RetainedPayload,
    replacement: bool,
}

impl RetentionAdmission<'_> {
    /// Complete prospective accounting, available before publication.
    #[must_use]
    pub const fn payload(&self) -> RetainedPayload {
        self.payload
    }

    /// Publish the pre-admitted owner set and payload. Allocates no storage and
    /// invokes no caller callback; the consumer can now publish its reserved entry.
    pub fn commit(self) {
        if self.replacement {
            self.ledger.owners.clear();
        }
        if let Some(owner) = self.owner {
            self.ledger.owners.insert(owner.owner_key(), owner);
        }
        self.ledger.payload = self.payload;
    }
}

/// A retention request failed before publishing any owner or payload change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetentionError {
    /// The complete proposed canonical payload exceeds the requested ceiling.
    Bytes {
        /// Complete prospective payload, not only the new entry's increment.
        required: usize,
        /// Requested ceiling; zero is a real limit.
        limit: usize,
    },
    /// Canonical payload/count arithmetic cannot be represented by `usize`.
    Overflow,
    /// The distinct-owner index could not reserve storage.
    Allocation,
}

impl fmt::Display for RetentionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bytes { required, limit } => write!(
                f,
                "retained payload needs {required} bytes; limit is {limit}"
            ),
            Self::Overflow => f.write_str("retained payload arithmetic overflowed"),
            Self::Allocation => f.write_str("retained catalog index could not reserve storage"),
        }
    }
}

impl std::error::Error for RetentionError {}
