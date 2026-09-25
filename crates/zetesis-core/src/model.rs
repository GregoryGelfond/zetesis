//! Canonical interpretations sharing an immutable dense occurrence catalog.

use std::{cmp::Ordering, fmt, iter::FusedIterator, slice, sync::Arc};

use crate::{
    Atom,
    catalog::{self, AtomRef, PredicateRef},
};

pub use crate::catalog::AtomCatalog;

const LENGTH_BYTES: usize = std::mem::size_of::<u64>();

/// A canonical true-atom set, also named [`Interpretation`]. Construction
/// establishes neither derivation, satisfaction nor answer-set membership.
///
/// One shared immutable catalog owns the atoms; one ordered index selection
/// denotes the true set. Clone shares both in constant time without allocation
/// or payload copying. Equality and ordering compare logical atoms, independently
/// of catalog identity or dense index order. No program or oracle is retained.
/// A sparse model can retain a much larger catalog than its selected true set.
#[derive(Clone)]
pub struct Model(Arc<Selected>);

struct Selected {
    catalog: AtomCatalog,
    positions: Vec<usize>,
}

impl Model {
    /// Named private selection header and position-buffer capacity, excluding
    /// the retained catalog, allocator metadata and Arc reference counters.
    /// Clones share this allocation; a family ledger counts it once per owner.
    #[must_use]
    pub fn selection_bytes(&self) -> u128 {
        size_of::<Selected>() as u128
            + self.0.positions.capacity() as u128 * size_of::<usize>() as u128
    }

    /// Accept an already ordered unique catalog as one interpretation.
    /// Every adjacent pair is checked in semantic atom order; no sorting or
    /// payload import is repeated. The selection allowance covers this model's
    /// header and dense position buffer, separately from its retained catalog.
    /// Callback admission precedes reservations, occurrence resolution,
    /// comparisons and publication.
    ///
    /// # Errors
    /// Refuses unordered/duplicate atoms, selection capacity or callback work.
    /// No interpretation is returned after a partial check.
    ///
    /// # Panics
    /// Panics if the immutable catalog iterator violates its exact length.
    pub fn from_ordered_catalog_with<E>(
        catalog: AtomCatalog,
        max_selection_bytes: usize,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, ModelFailure<E>> {
        Self::publish_ordered_catalog_with(catalog, max_selection_bytes, before)
            .map_err(|failure| failure.into_parts().0)
    }

    /// Publish an already ordered unique catalog, retaining an actual selection
    /// allocation receipt when the attempt is refused. The checks, allowance
    /// and callback order are those of [`Self::from_ordered_catalog_with`].
    ///
    /// The successful attempt's peak is exactly [`Self::selection_bytes`]: one
    /// position buffer is moved into the final selection without copying. On
    /// refusal, the receipt counts only the buffer capacity actually allocated;
    /// the final selection header has not been allocated. Rejected reservation
    /// proposals are not observations. The retained catalog, caller frames, Arc
    /// counters and allocator bookkeeping are excluded; this is not process RSS.
    ///
    /// Vector reservation is fallible. The final Arc envelope allocation uses
    /// stable Rust's infallible boundary and follows the last callback permit.
    ///
    /// # Errors
    /// Returns the original typed refusal and the actual named allocation peak.
    /// No partial interpretation is returned; earlier catalog clones stay valid.
    ///
    /// # Panics
    /// Panics if the immutable catalog iterator violates its exact length.
    /// A callback panic returns no receipt; earlier catalog clones stay valid.
    pub fn publish_ordered_catalog_with<E>(
        catalog: AtomCatalog,
        max_selection_bytes: usize,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, ModelPublicationFailure<E>> {
        let mut positions = Vec::new();
        prepare_ordered_selection(&catalog, &mut positions, max_selection_bytes, before)?;
        Ok(Self(Arc::new(Selected { catalog, positions })))
    }

    /// Import supplied atoms and coalesce them into canonical set order.
    /// The iterator must terminate. Construction consumes the descriptions,
    /// sorts and deduplicates them, and imports their typed payload into one
    /// canonical catalog. It performs `O(n log n)` typed atom comparisons;
    /// import work includes admitted term structure and exact interning probes.
    /// Input and selection vector growth is fallible. Arc envelope allocation
    /// remains infallible on stable Rust. Input addresses are not retained.
    ///
    /// # Errors
    /// Returns a vector reservation or canonical catalog admission refusal.
    /// No partial interpretation is returned.
    pub fn new(atoms: impl IntoIterator<Item = Atom>) -> Result<Self, ModelError> {
        let mut admitted = Vec::new();
        for atom in atoms {
            admitted
                .try_reserve(1)
                .map_err(|_| ModelError::Allocation)?;
            admitted.push(atom);
        }
        admitted.sort_unstable();
        admitted.dedup();
        Self::from_ordered(admitted)
    }

    /// Import atoms already in canonical order without sorting or deduplication.
    /// Canonical payload is admitted once; only occurrence and selection indices
    /// are retained beside it. Input addresses and capacities are not adopted.
    ///
    /// The atoms must be strictly increasing in canonical order; a debug build
    /// checks this, a release build trusts the producer, and a violated
    /// precondition would make membership queries wrong, not merely slow.
    ///
    /// # Errors
    /// Returns a selection reservation or canonical catalog admission refusal.
    /// No partial interpretation is returned.
    pub fn from_ordered(atoms: Vec<Atom>) -> Result<Self, ModelError> {
        debug_assert!(
            atoms.windows(2).all(|pair| pair[0] < pair[1]),
            "atoms imported as a model are strictly increasing"
        );
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(atoms.len())
            .map_err(|_| ModelError::Allocation)?;
        positions.extend(0..atoms.len());
        Ok(Self(Arc::new(Selected {
            catalog: AtomCatalog::new(atoms).map_err(ModelError::Catalog)?,
            positions,
        })))
    }

    /// Select original catalog positions as a canonical interpretation.
    ///
    /// Every index is checked before access. Duplicate indices and equal logical
    /// atoms coalesce. For `m` input positions, construction uses `O(m log m)`
    /// typed atom comparisons and `O(m)` index storage; no atom is cloned.
    /// Comparisons include immutable segment resolution and term traversal. The
    /// iterator must terminate. Index-vector growth is fallible, while the final
    /// Arc envelope allocation is infallible. The whole catalog remains live.
    ///
    /// # Errors
    /// Returns the first outside-catalog position or an index-vector reservation
    /// failure. No partial model is returned and the catalog remains unchanged.
    pub fn from_positions(
        catalog: &AtomCatalog,
        positions: impl IntoIterator<Item = usize>,
    ) -> Result<Self, ModelError> {
        let mut selected = Vec::new();
        for position in positions {
            if position >= catalog.atoms().len() {
                return Err(ModelError::Position {
                    position,
                    atoms: catalog.atoms().len(),
                });
            }
            selected
                .try_reserve(1)
                .map_err(|_| ModelError::Allocation)?;
            selected.push(position);
        }
        let atoms = catalog.atoms();
        selected.sort_unstable_by(|&left, &right| atoms.compare_positions(left, right));
        selected.dedup_by(|left, right| atoms.compare_positions(*left, *right).is_eq());
        Ok(Self(Arc::new(Selected {
            catalog: catalog.clone(),
            positions: selected,
        })))
    }

    /// Borrow the true atoms in canonical logical order. Creating this semantic
    /// collection view takes constant time and allocates nothing. It exposes no
    /// mutable collection and owns no payload; cloning the view only copies borrows.
    #[must_use]
    pub fn atoms(&self) -> ModelAtoms<'_> {
        ModelAtoms {
            atoms: self.0.catalog.atoms(),
            positions: &self.0.positions,
        }
    }

    /// Exact membership with a logarithmic number of typed atom comparisons.
    #[must_use]
    pub fn contains<'query>(&self, atom: impl Into<AtomRef<'query>>) -> bool {
        self.atoms().contains(atom)
    }

    /// Borrow checked predicate/key lookup over the existing canonical selection.
    /// Constant time, no allocation or payload copying. Predicate rows remain
    /// in canonical atom order; returned positions address the original catalog,
    /// and unselected catalog atoms never participate in either search.
    #[must_use]
    pub fn lookup(&self) -> crate::AtomLookup<'_, '_> {
        self.atoms().lookup()
    }

    /// Shared original catalog, including its unselected atoms. Access does not
    /// change the true set. The catalog can outlive the source or solve session.
    #[must_use]
    pub fn catalog(&self) -> &AtomCatalog {
        &self.0.catalog
    }

    /// True catalog positions in canonical atom order, with one representative
    /// per logical atom. These integers have meaning only in [`Self::catalog`].
    #[must_use]
    pub fn positions(&self) -> &[usize] {
        &self.0.positions
    }

    /// Retained selection-vector capacity in cells, excluding shared catalog
    /// storage and the Arc envelope. Model clones share this same vector.
    #[must_use]
    pub fn selection_capacity(&self) -> usize {
        self.0.positions.capacity()
    }

    /// Portable encoded payload of every catalog occurrence, including repeats
    /// and unselected atoms, plus a u64 selection length and u64 positions.
    /// Constant time after catalog construction. Returns `None` on size overflow.
    ///
    /// Counting this per retained model conservatively recounts shared catalogs.
    /// [`crate::retention::ModelRetention`] accounts shared catalogs once instead.
    /// This is a portable admission measure, not allocated bytes or RSS: spare
    /// vector capacity, Arc envelopes and allocator bookkeeping are excluded,
    /// as are canonical identities retained outside the occurrence map.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> Option<usize> {
        self.catalog()
            .retained_payload_bytes()?
            .checked_add(self.selection_payload_bytes()?)
    }

    /// Canonical selected-position record: a u64 length and u64 per position.
    /// Constant time; excludes the catalog, spare capacity and Arc overhead.
    /// Returns `None` on arithmetic overflow. Retention accounts this per model
    /// entry, even when a cloned model physically shares its selection vector.
    #[must_use]
    pub fn selection_payload_bytes(&self) -> Option<usize> {
        LENGTH_BYTES.checked_add(self.0.positions.len().checked_mul(LENGTH_BYTES)?)
    }
}

impl Default for Model {
    fn default() -> Self {
        Self(Arc::new(Selected {
            catalog: AtomCatalog::default(),
            positions: Vec::new(),
        }))
    }
}
impl fmt::Debug for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Model")
            .field("atoms", &self.atoms())
            .finish()
    }
}
impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.atoms() == other.atoms()
    }
}
impl Eq for Model {}
impl PartialOrd for Model {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Model {
    fn cmp(&self, other: &Self) -> Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            Ordering::Equal
        } else {
            self.atoms().cmp(&other.atoms())
        }
    }
}

/// A finite total interpretation represented by its true atoms. Every absent
/// atom is false. This raw value establishes no satisfaction or stable membership.
pub type Interpretation = Model;

/// Read-only canonical atom-set view. All returned references retain the model's
/// lifetime. Order is signed predicate and typed value storage order, not ASP
/// arithmetic term order. No allocation or atom copying occurs during iteration.
#[derive(Clone, Copy)]
pub struct ModelAtoms<'a> {
    atoms: catalog::Atoms<'a>,
    positions: &'a [usize],
}

impl<'a> ModelAtoms<'a> {
    /// Borrow the checked lookup over this selection without allocating.
    /// Only selected occurrences participate; returned row positions address
    /// the original catalog. The existing semantic order serves both identity
    /// and predicate searches.
    #[must_use]
    pub fn lookup(self) -> crate::AtomLookup<'a, 'a> {
        crate::AtomLookup {
            atoms: crate::atom_lookup::Source::Canonical(self.atoms),
            keys: self.positions,
            rows: self.positions,
        }
    }

    /// Number of distinct true atoms. Constant time.
    #[must_use]
    pub const fn len(self) -> usize {
        self.positions.len()
    }

    /// Whether the true set is empty. Constant time.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.positions.is_empty()
    }

    /// The atom at a position of the model's canonical order. Resolving its
    /// canonical payload searches the retained immutable segment ranges.
    #[must_use]
    pub fn at(self, position: usize) -> Option<AtomRef<'a>> {
        self.positions
            .get(position)
            .and_then(|&index| self.atoms.at(index))
    }

    /// Canonical double-ended iteration with an exact remaining length.
    #[must_use]
    pub fn iter(self) -> ModelIter<'a> {
        ModelIter {
            atoms: self.atoms,
            positions: self.positions.iter(),
        }
    }

    /// Least true atom in canonical storage order, resolving its retained segment.
    #[must_use]
    pub fn first(self) -> Option<AtomRef<'a>> {
        self.positions
            .first()
            .and_then(|&position| self.atoms.at(position))
    }

    /// Greatest true atom in canonical storage order, resolving its retained segment.
    #[must_use]
    pub fn last(self) -> Option<AtomRef<'a>> {
        self.positions
            .last()
            .and_then(|&position| self.atoms.at(position))
    }

    /// The true atoms of one signed predicate, a contiguous sub-view found by
    /// two logarithmic searches; empty when the predicate has none.
    #[must_use]
    pub fn of_predicate<'query>(
        self,
        predicate: impl Into<PredicateRef<'query>>,
    ) -> ModelAtoms<'a> {
        let predicate = predicate.into();
        let start = self.positions.partition_point(|&position| {
            self.selected(position).predicate().cmp(&predicate).is_lt()
        });
        let end = start
            + self.positions[start..].partition_point(|&position| {
                self.selected(position).predicate().cmp(&predicate).is_eq()
            });
        ModelAtoms {
            atoms: self.atoms,
            positions: &self.positions[start..end],
        }
    }

    /// Borrow the original matching atom using logarithmic typed comparisons.
    #[must_use]
    pub fn get<'query>(self, atom: impl Into<AtomRef<'query>>) -> Option<AtomRef<'a>> {
        let atom = atom.into();
        self.positions
            .binary_search_by(|&position| self.selected(position).cmp(&atom))
            .ok()
            .map(|index| self.selected(self.positions[index]))
    }

    fn selected(self, position: usize) -> AtomRef<'a> {
        self.atoms
            .at(position)
            .expect("model occurrence is checked")
    }

    /// Exact membership using logarithmic typed comparisons.
    #[must_use]
    pub fn contains<'query>(self, atom: impl Into<AtomRef<'query>>) -> bool {
        self.get(atom).is_some()
    }
}
impl fmt::Debug for ModelAtoms<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
impl PartialEq for ModelAtoms<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl Eq for ModelAtoms<'_> {}
impl PartialOrd for ModelAtoms<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ModelAtoms<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}
impl<'a> IntoIterator for ModelAtoms<'a> {
    type Item = AtomRef<'a>;
    type IntoIter = ModelIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Borrowed canonical traversal of a model's selected true atoms. Cloning copies
/// only cursor state. Every yielded atom is borrowed from the original catalog.
#[derive(Clone, Debug)]
pub struct ModelIter<'a> {
    atoms: catalog::Atoms<'a>,
    positions: slice::Iter<'a, usize>,
}
impl<'a> Iterator for ModelIter<'a> {
    type Item = AtomRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.positions.next().map(|&position| {
            self.atoms
                .at(position)
                .expect("model occurrence is checked")
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.positions.size_hint()
    }
}
impl DoubleEndedIterator for ModelIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.positions.next_back().map(|&position| {
            self.atoms
                .at(position)
                .expect("model occurrence is checked")
        })
    }
}
impl ExactSizeIterator for ModelIter<'_> {}
impl FusedIterator for ModelIter<'_> {}

/// Interpretation construction failed, without a partial true set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// A purportedly ordered unique catalog repeats or reverses an atom.
    Order {
        /// The second position of the first non-increasing pair.
        position: usize,
    },
    /// Named selection header and index capacity exceed the allowance.
    Bytes {
        /// Requested or actual named capacity.
        required: u128,
        /// Inclusive selection-storage allowance.
        limit: usize,
    },
    /// Canonical payload admission or representation failed.
    Catalog(catalog::Error),
    /// A supplied index has no atom in the retained catalog.
    Position {
        /// Refused dense position.
        position: usize,
        /// Number of atoms in this catalog.
        atoms: usize,
    },
    /// Input or selection-vector reservation failed.
    Allocation,
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Order { position } => write!(
                f,
                "model catalog is not strictly ordered at position {position}"
            ),
            Self::Bytes { required, limit } => write!(
                f,
                "model selection requires {required} bytes, allowance is {limit}"
            ),
            Self::Catalog(error) => error.fmt(f),
            Self::Position { position, atoms } => {
                write!(
                    f,
                    "model position {position} is outside a catalog of {atoms} atoms"
                )
            }
            Self::Allocation => f.write_str("model construction storage could not be reserved"),
        }
    }
}
impl std::error::Error for ModelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Catalog(error) => Some(error),
            Self::Position { .. } | Self::Order { .. } | Self::Bytes { .. } | Self::Allocation => {
                None
            }
        }
    }
}

/// Prepare a fresh, unallocated position buffer through the final publication
/// permit. The caller retains it on every return and moves it into Selected only
/// on success. One reservation is the only capacity change, so its retained
/// capacity is also the refused attempt's peak; no Selected envelope exists yet.
fn prepare_ordered_selection<E>(
    catalog: &AtomCatalog,
    positions: &mut Vec<usize>,
    max_selection_bytes: usize,
    mut before: impl FnMut() -> Result<(), E>,
) -> Result<(), ModelPublicationFailure<E>> {
    let result = (|| {
        let mut checked = || before().map_err(ModelFailure::Stopped);
        let count = catalog.atoms().len();
        selection_allowance(count, max_selection_bytes).map_err(ModelFailure::Model)?;
        checked()?;
        positions
            .try_reserve_exact(count)
            .map_err(|_| ModelFailure::Model(ModelError::Allocation))?;
        selection_allowance(positions.capacity(), max_selection_bytes)
            .map_err(ModelFailure::Model)?;
        let mut previous: Option<AtomRef<'_>> = None;
        let mut atoms = catalog.atoms().iter();
        for index in 0..count {
            checked()?;
            let atom = atoms
                .next()
                .expect("immutable catalog iterator has exact length");
            if let Some(left) = previous
                && !left.compare_ref_with(atom, &mut checked)?.is_lt()
            {
                return Err(ModelFailure::Model(ModelError::Order { position: index }));
            }
            previous = Some(atom);
            positions.push(index);
        }
        checked()
    })();
    // Observe the same still-live buffer after every normal refusal, including
    // reservation or actual-capacity refusal. Requested bytes are not a receipt.
    result.map_err(|failure| ModelPublicationFailure {
        failure,
        peak_bytes: positions.capacity() as u128 * size_of::<usize>() as u128,
    })
}

fn selection_allowance(capacity: usize, limit: usize) -> Result<(), ModelError> {
    let required = size_of::<Selected>() as u128 + capacity as u128 * size_of::<usize>() as u128;
    if required > limit as u128 {
        Err(ModelError::Bytes { required, limit })
    } else {
        Ok(())
    }
}

/// A model construction stopped without publishing an interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelFailure<E> {
    /// Typed ordering, shape or selection-storage refusal.
    Model(ModelError),
    /// The caller refused the next operation.
    Stopped(E),
}
impl<E: fmt::Display> fmt::Display for ModelFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ModelFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Model(error) => error,
            Self::Stopped(error) => error,
        })
    }
}

/// A selected-model publication refused after observing its actual allocations.
/// The consumed catalog handle is not returned; other catalog clones stay valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelPublicationFailure<E> {
    failure: ModelFailure<E>,
    peak_bytes: u128,
}

impl<E> ModelPublicationFailure<E> {
    /// Original ordering, selection-storage, allocation or caller refusal.
    #[must_use]
    pub const fn failure(&self) -> &ModelFailure<E> {
        &self.failure
    }

    /// Actual allocated position-buffer capacity during the refused attempt.
    /// The unallocated final selection header, retained catalog, caller frames,
    /// Arc counters, allocator bookkeeping and rejected proposals are excluded.
    #[must_use]
    pub const fn peak_bytes(&self) -> u128 {
        self.peak_bytes
    }

    /// Recover the original typed refusal together with its attempt receipt.
    #[must_use]
    pub fn into_parts(self) -> (ModelFailure<E>, u128) {
        (self.failure, self.peak_bytes)
    }
}

impl<E: fmt::Display> fmt::Display for ModelPublicationFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(f)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ModelPublicationFailure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

#[cfg(test)]
#[path = "model/publication_tests.rs"]
mod publication_tests;
