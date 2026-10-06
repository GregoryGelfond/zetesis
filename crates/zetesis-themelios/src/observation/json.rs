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
    Atom, Model, ValueLimits,
    catalog::{AtomIdentityMap, AtomRef},
};

pub use super::view::{ViewError as Error, ViewLimits as Limits};

/// Version of the model-record representation: a record refers to its atoms
/// by index into the document's [`AtomTable`] and spells only the new ones.
pub const RECORD_SCHEMA_VERSION: u32 = 2;

/// The atoms a document has spelled, in the order it spelled them. A record
/// encoded against the table spells the atoms it adds and refers to all of
/// its atoms by index, so a document spells each atom once. The table is
/// bounded by a ceiling on distinct atoms; every entry owns a copy of its
/// atom, made once when the document spells it, so the table retains no
/// answer's model or catalog. Structural equality decides an atom's index; a
/// canonical atom found once is afterwards answered by its owner-scoped
/// identity while anything outside the table still holds its owner.
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
    /// Indices of canonical atoms already found or entered by the record
    /// encoder, by owner-scoped identity, so a repeated atom costs an identity
    /// hash rather than a structural one. Only the encoder's lookup records
    /// identities. Each record begins by dropping the owners nothing outside
    /// the cache holds any longer: no atom of theirs can be presented again.
    /// Interleaved workers' live owners keep their identities across records,
    /// and runs whose every answer has its own owner, such as reconstructed
    /// terminal definitions, keep only the owners of answers still alive, so
    /// the map's linear owner scan does not grow with the number of records.
    identities: AtomIdentityMap<usize>,
    /// Owners the cache held after its last prune. A prune walks every owner,
    /// so it runs only once the owners have doubled since: amortized constant
    /// work per record, whoever holds the answers.
    held: usize,
    /// Prunes run, for the amortization tests.
    #[cfg(test)]
    prunes: usize,
    /// The first record's model, whose atoms hold the indices `0..len` in
    /// model order and are indexed only when a second record asks.
    deferred: Option<Model>,
    max_atoms: usize,
}

/// A spelled atom, owned by the table. It hashes and compares as the
/// canonical atom does, so a lookup by atom finds it.
#[derive(Debug)]
struct Entry(Atom);
impl Entry {
    /// Copy a spelled atom. The atom was admitted already, so only its own
    /// size bounds the copy; a copy that cannot be allocated is refused.
    fn copy(atom: AtomRef<'_>) -> Result<Self, Error> {
        let unbounded = ValueLimits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        };
        atom.to_atom(unbounded)
            .map(Self)
            .map_err(|_| Error::Allocation)
    }
    fn atom(&self) -> AtomRef<'_> {
        AtomRef::from(&self.0)
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
            held: 0,
            #[cfg(test)]
            prunes: 0,
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
    /// The atom's index when the document has spelled it, by structural
    /// lookup; no identity is recorded. The first record's atoms are indexed on
    /// the first lookup after it, so a document of one record never indexes at
    /// all.
    ///
    /// # Errors
    /// Returns [`Error::Allocation`] when the deferred record cannot be indexed.
    pub fn index<'a>(&mut self, atom: impl Into<AtomRef<'a>>) -> Result<Option<usize>, Error> {
        self.flush()?;
        Ok(self.indices.get(&atom.into()).copied())
    }
    /// The record encoder's lookup of an atom of the record begun last: as
    /// [`Self::index`], answering a canonical atom found once by its identity
    /// afterwards.
    pub(super) fn find(&mut self, atom: AtomRef<'_>) -> Result<Option<usize>, Error> {
        if let Some(index) = self.identities.get(atom) {
            return Ok(Some(index));
        }
        let index = self.index(atom)?;
        if let Some(index) = index {
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
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(model.atoms().len())
            .map_err(|_| Error::Allocation)?;
        for atom in model.atoms() {
            entries.push(Entry::copy(atom)?);
        }
        self.deferred = None;
        for (position, entry) in entries.into_iter().enumerate() {
            self.indices.insert(entry, position);
        }
        Ok(())
    }
    /// Start encoding a record. Once the cache's owners have doubled since
    /// its last prune, forget the identities of owners nothing outside the
    /// cache holds any longer, before this record's atoms are looked up.
    pub(super) fn begin_record(&mut self) {
        if self.identities.owners() > 2 * self.held.max(1) {
            self.identities.retain_held();
            self.held = self.identities.owners();
            #[cfg(test)]
            {
                self.prunes += 1;
            }
        }
    }
    /// Enter `atom`, of the record begun last, which the document is about to
    /// spell, at the next index.
    ///
    /// # Errors
    /// Returns [`Error::Table`] at the ceiling and [`Error::Allocation`] when
    /// the entry cannot be retained; the table is unchanged either way.
    pub(super) fn enter(&mut self, atom: AtomRef<'_>) -> Result<usize, Error> {
        self.flush()?;
        let index = self.indices.len();
        if index >= self.max_atoms {
            return Err(Error::Table);
        }
        self.indices.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.indices.insert(Entry::copy(atom)?, index);
        let _ = self.identities.insert(atom, index);
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
    use std::convert::Infallible;
    use std::hash::BuildHasherDefault;
    use zetesis_core::catalog::interner::{AtomInterner, Limits};
    use zetesis_core::{AtomCatalog, Model, Sign};
    use zetesis_test_support::programs::signed_numbered as atom;

    /// Index every atom of `model` as `ModelView::encode_record` does: defer
    /// the document's first record whole, else look each atom up and enter
    /// it when the table has not spelled it.
    fn record(table: &mut super::AtomTable, model: &Model) -> Vec<usize> {
        table.begin_record();
        if table.is_empty() {
            table.defer(model).unwrap();
            return (0..model.atoms().len()).collect();
        }
        (0..model.atoms().len())
            .map(|position| {
                let atom = model.atoms().at(position).unwrap();
                match table.find(atom).unwrap() {
                    Some(index) => index,
                    None => table.enter(atom).unwrap(),
                }
            })
            .collect()
    }

    /// Records whose models each have their own atom owner, as reconstructed
    /// answers do, sharing `q(0)` and each adding its own `p(n)`.
    fn separate_owners(records: i32) -> Vec<Model> {
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

    /// One writer's selections: distinct catalogs of one atom owner, as the
    /// lazy closure publishes each answer of a worker's workspace.
    fn one_owner_selections(selections: &[&[usize]]) -> Vec<Model> {
        let limits = Limits::for_atoms(32, 1 << 20);
        let mut owner = AtomInterner::new();
        for number in [2, 1, 3] {
            owner
                .entry_atom_with(&atom("p", Sign::Positive, &[number]), limits, || {
                    Ok::<_, Infallible>(())
                })
                .unwrap()
                .insert_with(limits, || Ok::<_, Infallible>(()))
                .unwrap();
        }
        selections
            .iter()
            .map(|positions| {
                let catalog = owner
                    .publish_selection_with(positions, limits, || Ok::<_, Infallible>(()))
                    .unwrap();
                Model::from_positions(&catalog, 0..positions.len()).unwrap()
            })
            .collect()
    }

    /// A live writer holding one atom, as a closure worker's workspace does.
    fn writer(number: i32) -> AtomInterner {
        let limits = Limits::for_atoms(32, 1 << 20);
        let mut owner = AtomInterner::new();
        owner
            .entry_atom_with(&atom("p", Sign::Positive, &[number]), limits, || {
                Ok::<_, Infallible>(())
            })
            .unwrap()
            .insert_with(limits, || Ok::<_, Infallible>(()))
            .unwrap();
        owner
    }

    fn selection(owner: &mut AtomInterner) -> Model {
        let limits = Limits::for_atoms(32, 1 << 20);
        let catalog = owner
            .publish_selection_with(&[0], limits, || Ok::<_, Infallible>(()))
            .unwrap();
        Model::from_positions(&catalog, [0]).unwrap()
    }

    #[test]
    fn interleaved_live_writers_keep_their_identities() {
        // Two workers' writers stay alive while their records alternate; each
        // record's model is its own selection, dropped once encoded.
        let mut left = writer(1);
        let mut right = writer(2);
        let mut table = super::AtomTable::new(16);
        for _ in 0..4 {
            record(&mut table, &selection(&mut left));
            record(&mut table, &selection(&mut right));
        }
        let model = selection(&mut left);
        table.begin_record();
        let atom = model.atoms().at(0).unwrap();
        assert!(table.identities.get(atom).is_some());
        assert_eq!(table.identities.get(atom), table.index(atom).unwrap());
    }

    #[test]
    fn answers_dropped_after_their_records_leave_the_identity_cache() {
        let mut table = super::AtomTable::new(1 << 20);
        // Each model is dropped after its record, as a reconstructed answer
        // is once published, so pruning keeps the cache to a few owners.
        for model in separate_owners(50) {
            record(&mut table, &model);
        }
        assert!(
            table.identities.owners() <= 3,
            "{}",
            table.identities.owners()
        );
    }

    #[test]
    fn live_writers_keep_their_identities_through_a_prune() {
        // Three workers' writers stay alive; their records interleave until a
        // prune runs, which must keep every live owner.
        let mut writers = [writer(1), writer(2), writer(3)];
        let mut table = super::AtomTable::new(16);
        let mut models = Vec::new();
        while table.prunes == 0 {
            for owner in &mut writers {
                let model = selection(owner);
                record(&mut table, &model);
                models.push(model);
                if table.prunes != 0 {
                    break;
                }
            }
        }
        assert_eq!(table.identities.owners(), 3);
        for owner in &mut writers {
            let model = selection(owner);
            let atom = model.atoms().at(0).unwrap();
            assert!(table.identities.get(atom).is_some());
        }
    }

    #[test]
    fn retained_answers_are_pruned_a_logarithmic_number_of_times() {
        // A consumer that keeps every answer keeps every owner held, so no
        // prune can drop one; prunes stay amortized all the same.
        let models = separate_owners(256);
        let mut table = super::AtomTable::new(1 << 20);
        for model in &models {
            record(&mut table, model);
        }
        assert_eq!(table.identities.owners(), 255);
        assert!(table.prunes <= 9, "{}", table.prunes);
    }

    #[test]
    fn indices_match_a_structural_numbering_across_owners() {
        let mut table = super::AtomTable::new(1 << 20);
        let mut reference: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (n, model) in separate_owners(8).iter().enumerate() {
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
        let models: Vec<_> = (0..3)
            .map(|_| Model::from_positions(&catalog, [0, 1]).unwrap())
            .collect();
        let mut table = super::AtomTable::new(16);
        // The first record is deferred; the second records both identities.
        record(&mut table, &models[0]);
        record(&mut table, &models[1]);
        table.begin_record();
        // The third record's atoms are answered by those identities.
        for position in 0..2 {
            let atom = models[2].atoms().at(position).unwrap();
            assert_eq!(table.identities.get(atom), table.index(atom).unwrap());
            assert!(table.identities.get(atom).is_some());
        }
    }

    #[test]
    fn selections_of_one_owner_keep_their_identities_across_records() {
        // Every record is its own catalog of the one owner.
        let models = one_owner_selections(&[&[0], &[0, 1], &[0, 1, 2]]);
        let mut table = super::AtomTable::new(16);
        record(&mut table, &models[0]);
        record(&mut table, &models[1]);
        table.begin_record();
        // The next selection's repeated atoms are answered by the identities
        // the previous one recorded.
        for position in 0..2 {
            let atom = models[2].atoms().at(position).unwrap();
            assert_eq!(table.identities.get(atom), table.index(atom).unwrap());
            assert!(table.identities.get(atom).is_some());
        }
    }

    #[test]
    fn the_public_index_records_no_identity() {
        let catalog = AtomCatalog::new(vec![atom("q", Sign::Positive, &[0])]).unwrap();
        let model = Model::from_positions(&catalog, [0]).unwrap();
        let mut table = super::AtomTable::new(16);
        record(&mut table, &model);
        // An equal atom of another owner is a structural hit whose identity
        // the table has not recorded; the public lookup leaves it so.
        let other = Model::new([atom("q", Sign::Positive, &[0])]).unwrap();
        let other = other.atoms().at(0).unwrap();
        assert_eq!(table.index(other).unwrap(), Some(0));
        assert_eq!(table.identities.get(other), None);
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
