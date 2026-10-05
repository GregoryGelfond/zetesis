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

use hashbrown::HashMap;
use std::fmt;

use zetesis_core::{
    AtomCatalog, Model,
    catalog::{AtomIdentityMap, AtomRef},
};

pub use super::view::{ViewError as Error, ViewLimits as Limits};

/// Version of the model-record representation: a record refers to its atoms
/// by index into the document's [`AtomTable`] and spells only the new ones.
pub const RECORD_SCHEMA_VERSION: u32 = 2;

/// The atoms a document has spelled, in the order it spelled them. A record
/// encoded against the table spells the atoms it adds and refers to all of
/// its atoms by index, so a document spells each atom once. The table is
/// bounded by a ceiling on distinct atoms; every entry refers to its atom in
/// the model that spelled it, sharing that model's catalog rather than
/// copying the atom. Structural equality decides an atom's index; a
/// canonical atom found once is afterwards answered by its owner-scoped
/// identity, at most once per distinct atom and owner.
#[derive(Debug)]
pub struct AtomTable {
    /// Placed by the crate's fixed word hash. The program's author spells
    /// the atoms and could choose them to collide; a collision costs a
    /// lookup a scan of the table, bounded by its ceiling of distinct
    /// atoms, and the solving his program commands already costs him more
    /// than any table could. The randomized hasher was measured at 1.09 to
    /// 1.16 of the cell time on the series' large-model cells, hashing
    /// every atom of every model on the way out (the observations record).
    indices: HashMap<Entry, usize, std::hash::BuildHasherDefault<crate::word_hash::WordHasher>>,
    /// Indices of canonical atoms already found or entered, by owner-scoped
    /// identity, so a repeated atom costs an identity hash rather than a
    /// structural one. An atom of an owner not yet seen is found structurally.
    /// Identities are recorded only while a record is encoded, and only for
    /// the record's own catalog: a record's atoms all come from its model's
    /// one catalog, and a record of another catalog empties the cache, so it
    /// holds one owner. Runs whose models share one catalog keep their
    /// identities across records; runs whose every answer has its own
    /// catalog, such as reconstructed terminal definitions, would otherwise
    /// add an owner per record to the map's linear owner scan.
    identities: AtomIdentityMap<usize>,
    /// The catalog of the record being, or last, encoded.
    catalog: Option<AtomCatalog>,
    /// Whether a record is being encoded: only then are identities used.
    recording: bool,
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
    fn atom(&self) -> AtomRef<'_> {
        self.model
            .atoms()
            .at(self.position)
            .expect("an entry refers to a position of the model that spelled it")
    }
}
impl hashbrown::Equivalent<Entry> for AtomRef<'_> {
    fn equivalent(&self, entry: &Entry) -> bool {
        *self == entry.atom()
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
            identities: AtomIdentityMap::default(),
            catalog: None,
            recording: false,
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
    /// of one record never indexes at all. Within a record, a canonical atom
    /// found once is answered by its identity afterwards; outside a record the
    /// lookup is structural and records no identity.
    ///
    /// # Errors
    /// Returns [`Error::Allocation`] when the deferred record cannot be indexed.
    pub fn index<'a>(&mut self, atom: impl Into<AtomRef<'a>>) -> Result<Option<usize>, Error> {
        self.flush()?;
        let atom = atom.into();
        if self.recording
            && let Some(index) = self.identities.get(atom)
        {
            return Ok(Some(index));
        }
        let index = self.indices.get(&atom).copied();
        if self.recording
            && let Some(index) = index
        {
            let _ = self.identities.insert(atom, index);
        }
        Ok(index)
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
        let Some(model) = self.deferred.as_ref() else {
            return Ok(());
        };
        self.indices
            .try_reserve(model.atoms().len())
            .map_err(|_| Error::Allocation)?;
        let model = self.deferred.take().expect("reserved deferred model");
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
    /// Start encoding a record whose atoms come from `catalog`. A record of
    /// another catalog than the last empties the identity cache.
    pub(super) fn begin_record(&mut self, catalog: &AtomCatalog) {
        if !self
            .catalog
            .as_ref()
            .is_some_and(|last| last.same_owner(catalog))
        {
            self.identities = AtomIdentityMap::default();
            self.catalog = Some(catalog.clone());
        }
        self.recording = true;
    }
    /// The record has been encoded or refused.
    pub(super) fn end_record(&mut self) {
        self.recording = false;
    }
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
        let atom = model
            .atoms()
            .at(position)
            .expect("an entered position belongs to its model");
        if self.recording {
            let _ = self.identities.insert(atom, index);
        }
        Ok(index)
    }
    /// Withdraw what a refused record gave the table, so it is as it was
    /// before the record: the record's deferral when it was the first
    /// record, else the atoms it entered. A later record refused before its
    /// first lookup gave nothing, and the first record's deferral stands.
    pub(super) fn retract(&mut self, atoms: &[AtomRef<'_>], deferred: bool) {
        if deferred {
            // Only the first record defers, and no lookup precedes it, so no
            // identity was recorded.
            debug_assert!(self.identities.is_empty());
            self.deferred = None;
            return;
        }
        for atom in atoms {
            self.indices.remove(atom);
        }
        // The withdrawn atoms held the last indices; no identity keeps one.
        let len = self.indices.len();
        self.identities.retain(|index| index < len);
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
    use hashbrown::HashMap;
    use std::hash::BuildHasherDefault;
    use zetesis_core::{AtomCatalog, Model, Sign};
    use zetesis_test_support::programs::signed_numbered as atom;

    /// Index every atom of `model` as `ModelView::encode_record` does: look
    /// it up, and enter it when the table has not spelled it.
    fn record(table: &mut super::AtomTable, model: &Model) -> Vec<usize> {
        table.begin_record(model.catalog());
        let indices = (0..model.atoms().len())
            .map(|position| {
                let atom = model.atoms().at(position).unwrap();
                match table.index(atom).unwrap() {
                    Some(index) => index,
                    None => table.enter(model, position).unwrap(),
                }
            })
            .collect();
        table.end_record();
        indices
    }

    /// Records whose models each have their own catalog, as reconstructed
    /// answers do, sharing `q(0)` and each adding its own `p(n)`.
    fn separate_catalogs(records: i32) -> Vec<Model> {
        (0..records)
            .map(|n| {
                Model::new([
                    atom("q", Sign::Positive, &[0]),
                    atom("p", Sign::Positive, &[n]),
                ])
                .unwrap()
            })
            .collect()
    }

    #[test]
    fn separate_catalogs_keep_one_owner_in_the_identity_cache() {
        let mut table = super::AtomTable::new(1 << 20);
        for model in separate_catalogs(200) {
            record(&mut table, &model);
        }
        // One record's atoms: the identities of earlier catalogs are gone.
        assert!(table.identities.len() <= 2, "{}", table.identities.len());
    }

    #[test]
    fn indices_match_a_structural_numbering_across_catalogs() {
        let mut table = super::AtomTable::new(1 << 20);
        let mut reference: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (n, model) in separate_catalogs(200).iter().enumerate() {
            let indices = record(&mut table, model);
            // Model order is canonical: `p(n)` precedes `q(0)`.
            for (spelled, index) in [format!("p({n})"), "q(0)".to_owned()]
                .into_iter()
                .zip(indices)
            {
                let next = reference.len();
                assert_eq!(*reference.entry(spelled).or_insert(next), index);
            }
        }
    }

    #[test]
    fn one_catalog_keeps_its_identities_across_records() {
        let catalog = AtomCatalog::new(vec![
            atom("q", Sign::Positive, &[0]),
            atom("p", Sign::Positive, &[1]),
        ])
        .unwrap();
        let first = Model::from_positions(&catalog, [0, 1]).unwrap();
        let second = Model::from_positions(&catalog, [0, 1]).unwrap();
        let mut table = super::AtomTable::new(16);
        record(&mut table, &first);
        let known = table.identities.len();
        assert_eq!(known, 2);
        record(&mut table, &second);
        assert_eq!(table.identities.len(), known);
    }

    #[test]
    fn a_lookup_outside_a_record_records_no_identity() {
        let model = Model::new([atom("q", Sign::Positive, &[0])]).unwrap();
        let mut table = super::AtomTable::new(16);
        table.begin_record(model.catalog());
        table.enter(&model, 0).unwrap();
        table.end_record();
        let other = Model::new([atom("q", Sign::Positive, &[0])]).unwrap();
        let before = table.identities.len();
        assert_eq!(table.index(other.atoms().at(0).unwrap()).unwrap(), Some(0));
        assert_eq!(table.identities.len(), before);
    }

    /// The table places the atoms by the crate's fixed word hash, the
    /// placement an author could drive into collisions being bounded by the
    /// table's ceiling and dwarfed by the solving his program already
    /// commands; this compiles only while it does.
    #[test]
    fn the_atom_table_places_spelled_atoms_by_the_word_hash() {
        let table = super::AtomTable::new(1);
        let _: &HashMap<super::Entry, usize, BuildHasherDefault<crate::word_hash::WordHasher>> =
            &table.indices;
    }
}
