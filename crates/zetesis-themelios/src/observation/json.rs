//! Pure model-record JSON encoding vocabulary, independent of a CLI envelope.
//!
//! A model *record* inside a document spells only the atoms the document has
//! not spelled before, as typed atoms with preorder term nodes, and refers to
//! every atom by its index in the document's [`AtomTable`], the atoms in the
//! order the document spelled them; the shown channels are separate, and the
//! costs are descending priority/value pairs (schema 2). All integers are
//! exact decimal JSON numbers; consumers must preserve integers beyond
//! JavaScript's exact Number range. The version is supplied out of band; it
//! adds no field to the record.

use std::collections::HashMap;
use std::fmt;

use zetesis_core::{Atom, Model};

pub use super::view::{ViewError as Error, ViewLimits as Limits};

/// Version of the model-record representation: a record refers to its atoms
/// by index into the document's [`AtomTable`] and spells only the new ones.
pub const RECORD_SCHEMA_VERSION: u32 = 2;

/// The atoms a document has spelled, in the order it spelled them. A record
/// encoded against the table spells the atoms it adds and refers to all of
/// its atoms by index, so a document spells each atom once. The table is
/// bounded by a ceiling on distinct atoms; every entry refers to its atom in
/// the model that spelled it, sharing that model's catalog rather than
/// copying the atom.
#[derive(Debug)]
pub struct AtomTable {
    /// Placed by the standard library's randomized hasher: the program's
    /// author spells the atoms, and their placement is not his to drive
    /// into collisions.
    indices: HashMap<Entry, usize>,
    /// The first record's model, whose atoms hold the indices `0..len` in
    /// model order and are indexed only when a second record asks.
    deferred: Option<Model>,
    max_atoms: usize,
}

/// An atom by its position in the model that spelled it. It hashes and
/// compares as the atom does, so a lookup by atom finds it.
#[derive(Debug)]
struct Entry {
    model: Model,
    position: usize,
}
impl Entry {
    fn atom(&self) -> &Atom {
        self.model
            .atoms()
            .at(self.position)
            .expect("an entry refers to a position of the model that spelled it")
    }
}
impl std::borrow::Borrow<Atom> for Entry {
    fn borrow(&self) -> &Atom {
        self.atom()
    }
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.atom() == other.atom()
    }
}
impl Eq for Entry {}
impl std::hash::Hash for Entry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.atom().hash(state);
    }
}

impl AtomTable {
    /// An empty table admitting at most `max_atoms` distinct atoms.
    #[must_use]
    pub fn new(max_atoms: usize) -> Self {
        Self {
            indices: HashMap::default(),
            deferred: None,
            max_atoms,
        }
    }
    /// Distinct atoms spelled so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.indices.len()
            + self
                .deferred
                .as_ref()
                .map_or(0, |model| model.atoms().len())
    }
    /// Whether no atom has been spelled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The atom's index when the document has spelled it. The first
    /// record's atoms are indexed on the first lookup after it, so a document
    /// of one record never indexes at all.
    ///
    /// # Errors
    /// Returns [`Error::Allocation`] when the deferred record cannot be indexed.
    pub fn index(&mut self, atom: &Atom) -> Result<Option<usize>, Error> {
        self.flush()?;
        Ok(self.indices.get(atom).copied())
    }
    /// Take the whole first record as the table: its atoms hold the indices
    /// `0..len` in model order, without indexing them.
    pub(super) fn defer(&mut self, model: &Model) -> Result<(), Error> {
        debug_assert!(self.is_empty(), "only the first record is deferred");
        if model.atoms().len() > self.max_atoms {
            return Err(Error::Table);
        }
        self.deferred = Some(model.clone());
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Error> {
        let Some(model) = self.deferred.take() else {
            return Ok(());
        };
        self.indices
            .try_reserve(model.atoms().len())
            .map_err(|_| Error::Allocation)?;
        for position in 0..model.atoms().len() {
            self.indices.insert(
                Entry {
                    model: model.clone(),
                    position,
                },
                position,
            );
        }
        Ok(())
    }
    /// Enter the atom at `position` of `model`, which the document is about
    /// to spell, at the next index.
    ///
    /// # Errors
    /// Returns [`Error::Table`] at the ceiling and [`Error::Allocation`] when
    /// the entry cannot be retained; the table is unchanged either way.
    pub(super) fn enter(&mut self, model: &Model, position: usize) -> Result<usize, Error> {
        self.flush()?;
        let index = self.indices.len();
        if index >= self.max_atoms {
            return Err(Error::Table);
        }
        self.indices.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.indices.insert(
            Entry {
                model: model.clone(),
                position,
            },
            index,
        );
        Ok(index)
    }
    /// Withdraw what a refused record gave the table, so it is as it was
    /// before the record: the record's deferral when it was the first
    /// record, else the atoms it entered. A later record refused before its
    /// first lookup gave nothing, and the first record's deferral stands.
    pub(super) fn retract(&mut self, atoms: &[&Atom], deferred: bool) {
        if deferred {
            self.deferred = None;
            return;
        }
        for atom in atoms {
            self.indices.remove(*atom);
        }
    }
}

/// Charged encoding work, distinct from observation evaluation and publication.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Successful work charges; the refused next charge is excluded.
    pub work: u64,
    /// UTF-8 bytes buffered before success or refusal. On failure these were
    /// discarded, and must never be interpreted as published bytes.
    pub buffered_bytes: usize,
}

/// One complete private model record with its encoding accounting.
#[derive(Debug)]
pub struct Encoded {
    text: String,
    statistics: Statistics,
}
impl Encoded {
    pub(super) fn new(text: String, statistics: Statistics) -> Self {
        Self { text, statistics }
    }
    /// Complete record text; no external bytes have been written.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Consume the record without copying its text.
    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }
    /// Charged work for this encoding alone.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}

/// Typed refusal and retained accounting; no incomplete record escapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    cause: Error,
    statistics: Statistics,
}
impl Failure {
    pub(super) const fn new(cause: Error, statistics: Statistics) -> Self {
        Self { cause, statistics }
    }
    /// Authoritative typed encoding cause.
    #[must_use]
    pub const fn cause(&self) -> Error {
        self.cause
    }
    /// Charged work and discarded private bytes before refusal.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for Failure {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::hash::RandomState;

    /// The program's author spells the atoms, so the table places them by
    /// the standard library's randomized hasher, which he cannot drive into
    /// collisions; this compiles only while it does.
    #[test]
    fn the_atom_table_places_spelled_atoms_by_the_randomized_hasher() {
        let table = super::AtomTable::new(1);
        let _: &HashMap<super::Entry, usize, RandomState> = &table.indices;
    }
}
