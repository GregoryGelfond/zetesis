//! A one-tuple transaction containing only equality IDs and new values.
//!
//! A new value is staged with its content hash; a later column of the same
//! tuple that repeats it reuses its tentative ID. No row, dictionary
//! representative or content chain changes before final admission.
//! The staged state depends on this tuple's width, never on the historical
//! dictionary.

use crate::catalog::{AtomRef, Atoms, storage::TermId};

use super::super::{
    Cell, DictionaryIndex, Failure, Layout, Resource, Work, ceiling,
    dictionary::{Content, Probe},
};

pub(super) struct Added {
    pub cell: Cell,
    pub term: TermId,
    /// The value's semantic content hash.
    pub hash: u64,
}

#[derive(Default)]
pub(super) struct Plan {
    pub ids: Vec<u32>,
    pub added: Vec<Added>,
    pub encoding_bytes: u128,
}

/// Temporary scalar scratch is admitted only after a novel row is established.
/// A session includes its retained header and capacities at operation entry.
#[derive(Clone, Copy)]
pub(super) enum Admission {
    Temporary,
    Retained,
}

impl Plan {
    pub(super) fn capacity_bytes(&self) -> usize {
        super::vector_bytes(&self.ids) + super::vector_bytes(&self.added)
    }

    pub(super) fn clear(&mut self) {
        self.ids.clear();
        self.added.clear();
        self.encoding_bytes = 0;
    }

    /// Only this tuple's unpublished distinct values are scanned. Historical
    /// dictionary hits use the complete canonical inverse instead.
    fn staged(
        &self,
        term: TermId,
        start: usize,
        work: &mut Work,
    ) -> Result<Option<usize>, Failure> {
        for (offset, added) in self.added.iter().enumerate() {
            work.tick(1)?;
            if added.term == term {
                return start.checked_add(offset).map(Some).ok_or(Failure::Overflow);
            }
        }
        Ok(None)
    }
}

pub(super) fn values(
    layout: &mut Layout,
    atoms: Atoms<'_>,
    atom: AtomRef<'_>,
    encoding_bytes: u128,
    plan: &mut Plan,
    work: &mut Work,
) -> Result<(), Failure> {
    let Layout {
        dictionary, index, ..
    } = layout;
    let DictionaryIndex::Append(index) = index else {
        return Err(Failure::Dictionary);
    };
    if index.terms {
        return terms(index, atoms, atom, encoding_bytes, plan, work);
    }
    plan.encoding_bytes = encoding_bytes;
    work.grow(&mut plan.ids, atom.values().len())?;
    work.grow(&mut plan.added, atom.values().len())?;
    for (column, value) in atom.values().iter().enumerate() {
        work.tick(1)?;
        let bytes = value.canonical_bytes_with(|| work.tick(1))?;
        plan.encoding_bytes = plan
            .encoding_bytes
            .checked_add(bytes as u128)
            .ok_or(Failure::Overflow)?;
        let (term, found) = match index
            .identities
            .probe_with(atoms.read(), value, &mut || work.tick(1))?
        {
            Probe::Local { term, equality } => (term, equality.map(|id| id as usize)),
            Probe::Unavailable(error) => return Err(Failure::Read(error)),
        };
        let found = match found {
            Some(id) => Some(id),
            None => plan.staged(term, dictionary.len(), work)?,
        };
        let id = if let Some(id) = found {
            id
        } else {
            let id = dictionary
                .len()
                .checked_add(plan.added.len())
                .ok_or(Failure::Overflow)?;
            ceiling(
                Resource::Values,
                id as u128 + 1,
                (work.limits.max_values as u128).min(u128::from(u32::MAX) + 1),
            )?;
            // A new value is chained under its content hash, which reads
            // every payload word once.
            work.tick(1 + Content::hash_work(value.expanded_nodes(), bytes))?;
            plan.added.push(Added {
                cell: Cell {
                    row: atoms.len(),
                    column,
                },
                term,
                hash: Content::hash(value),
            });
            id
        };
        work.tick(1)?;
        plan.ids
            .push(u32::try_from(id).map_err(|_| Failure::Overflow)?);
    }
    Ok(())
}

/// An interned owner's IDs are the values' own term identities: a value new
/// to this owner is staged once for its presence entry, with no content hash,
/// representative or renumbering.
fn terms(
    index: &super::super::dictionary::Append,
    atoms: Atoms<'_>,
    atom: AtomRef<'_>,
    encoding_bytes: u128,
    plan: &mut Plan,
    work: &mut Work,
) -> Result<(), Failure> {
    plan.encoding_bytes = encoding_bytes;
    work.grow(&mut plan.ids, atom.values().len())?;
    work.grow(&mut plan.added, atom.values().len())?;
    for (column, value) in atom.values().iter().enumerate() {
        work.tick(1)?;
        let bytes = value.canonical_bytes_with(|| work.tick(1))?;
        plan.encoding_bytes = plan
            .encoding_bytes
            .checked_add(bytes as u128)
            .ok_or(Failure::Overflow)?;
        let (term, found) = match index
            .identities
            .probe_with(atoms.read(), value, &mut || work.tick(1))?
        {
            Probe::Local { term, equality } => (term, equality.is_some()),
            Probe::Unavailable(error) => return Err(Failure::Read(error)),
        };
        if !found && plan.staged(term, 0, work)?.is_none() {
            plan.added.push(Added {
                cell: Cell {
                    row: atoms.len(),
                    column,
                },
                term,
                hash: 0,
            });
        }
        plan.ids.push(term.word());
    }
    Ok(())
}
