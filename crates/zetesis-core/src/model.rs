//! Canonical interpretations sharing an immutable dense atom catalog.

use std::{cmp::Ordering, fmt, iter::FusedIterator, slice, sync::Arc};

use crate::{Atom, Value};

const LENGTH_BYTES: usize = std::mem::size_of::<u64>();
const TAG_BYTES: usize = 1;
const ATOM_HEADER_BYTES: usize = 2 * LENGTH_BYTES + TAG_BYTES;

/// Immutable typed atoms in their original dense index order.
///
/// The catalog does not establish truth or program membership. Equal atoms may
/// occupy different positions; indices retain their supplied meanings. Cloning
/// shares the vector and its payloads in constant time. Selections retain the
/// entire catalog, including unselected atoms, until the last owner is dropped.
#[derive(Clone, Debug)]
pub struct AtomCatalog(Arc<CatalogData>);

#[derive(Debug)]
struct CatalogData {
    atoms: Vec<Atom>,
    canonical_bytes: Option<usize>,
}

impl AtomCatalog {
    /// Retain an existing atom vector without copying or reordering its cells.
    /// Existing atom/value addresses and vector capacity are preserved. The Arc
    /// envelope allocation is infallible, not a typed allocation refusal. One
    /// traversal records a checked canonical payload size for later retention
    /// admission; this traverses atom/value descriptions but copies no payload.
    #[must_use]
    pub fn new(atoms: Vec<Atom>) -> Self {
        Self(Arc::new(CatalogData {
            canonical_bytes: canonical_bytes(&atoms),
            atoms,
        }))
    }

    /// Original dense-order atoms. Borrowing and indexing allocate nothing.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.0.atoms
    }

    /// Retained atom-vector capacity in cells, excluding the Arc envelope and
    /// nested payload allocations. Sharing does not multiply this capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.0.atoms.capacity()
    }

    /// Whether both handles retain the same catalog allocation. This is owner
    /// identity, not logical equality of atoms or interpretations.
    #[must_use]
    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Canonical payload of the entire catalog, including unselected atoms.
    /// Recorded by the constructor; returns `None` on arithmetic overflow.
    /// This portable measure excludes spare capacity, Arc and allocator overhead.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> Option<usize> {
        self.0.canonical_bytes
    }

    // Only a live retained handle makes this address an allocation identity.
    // The retention index stores that handle beside the key; it never exposes
    // addresses as logical identity or keeps a key after releasing its owner.
    pub(crate) fn owner_key(&self) -> usize {
        Arc::as_ptr(&self.0).addr()
    }
}

impl Default for AtomCatalog {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// u64 list lengths, a predicate-sign byte and typed logical value encodings.
// This portable admission measure is independent of allocator capacity or RSS.
fn canonical_bytes(atoms: &[Atom]) -> Option<usize> {
    let mut bytes = LENGTH_BYTES;
    for atom in atoms {
        bytes = bytes
            .checked_add(ATOM_HEADER_BYTES)?
            .checked_add(atom.predicate().name().len())?;
        for value in atom.values() {
            let payload = match value {
                Value::Infimum | Value::Supremum => 0,
                Value::Number(_) => std::mem::size_of::<i32>(),
                Value::Structured(value) => value.canonical_bytes().checked_sub(TAG_BYTES)?,
                Value::Symbol(text) | Value::String(text) => {
                    LENGTH_BYTES.checked_add(text.len())?
                }
            };
            bytes = bytes.checked_add(TAG_BYTES)?.checked_add(payload)?;
        }
    }
    Some(bytes)
}

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
    /// Coalesce supplied owned atoms into canonical set order.
    /// The iterator must terminate. Construction consumes the atoms, sorts and
    /// deduplicates them, then selects every remaining atom. It performs
    /// `O(n log n)` typed atom comparisons and allocates an atom vector plus an
    /// index vector. Collection and Arc allocation are infallible. Payloads are
    /// moved, not cloned; comparisons can inspect text and structured values.
    #[must_use]
    pub fn new(atoms: impl IntoIterator<Item = Atom>) -> Self {
        let mut atoms: Vec<_> = atoms.into_iter().collect();
        atoms.sort_unstable();
        atoms.dedup();
        let positions = (0..atoms.len()).collect();
        Self(Arc::new(Selected {
            catalog: AtomCatalog::new(atoms),
            positions,
        }))
    }

    /// Select original catalog positions as a canonical interpretation.
    ///
    /// Every index is checked before access. Duplicate indices and equal logical
    /// atoms coalesce. For `m` input positions, construction uses `O(m log m)`
    /// typed atom comparisons and `O(m)` index storage; no atom is cloned. The
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
        selected
            .sort_unstable_by(|&left, &right| catalog.atoms()[left].cmp(&catalog.atoms()[right]));
        selected.dedup_by(|left, right| catalog.atoms()[*left] == catalog.atoms()[*right]);
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
    pub fn contains(&self, atom: &Atom) -> bool {
        self.atoms().contains(atom)
    }

    /// Borrow checked predicate/key lookup over the existing canonical selection.
    /// Constant time, no allocation or payload copying. Predicate rows remain
    /// in canonical atom order; returned positions address the original catalog,
    /// and unselected catalog atoms never participate in either search.
    #[must_use]
    pub fn lookup(&self) -> crate::AtomLookup<'_, '_> {
        crate::AtomLookup {
            atoms: self.0.catalog.atoms(),
            keys: &self.0.positions,
            rows: &self.0.positions,
        }
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

    /// Canonical retained payload size: every catalog atom, including unselected
    /// atoms, plus a u64 selection length and u64 positions. Constant time after
    /// the catalog's initial traversal. Returns `None` on size overflow.
    ///
    /// Counting this per retained model conservatively recounts shared catalogs.
    /// [`crate::retention::ModelRetention`] accounts shared catalogs once instead.
    /// This is a portable admission measure, not allocated bytes or RSS: spare
    /// vector capacity, Arc envelopes and allocator bookkeeping are excluded.
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
        Self::new([])
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
    atoms: &'a [Atom],
    positions: &'a [usize],
}

impl<'a> ModelAtoms<'a> {
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

    /// Canonical double-ended iteration with an exact remaining length.
    #[must_use]
    pub fn iter(self) -> ModelIter<'a> {
        ModelIter {
            atoms: self.atoms,
            positions: self.positions.iter(),
        }
    }

    /// Least true atom in canonical storage order. Constant time.
    #[must_use]
    pub fn first(self) -> Option<&'a Atom> {
        self.positions
            .first()
            .map(|&position| &self.atoms[position])
    }

    /// Greatest true atom in canonical storage order. Constant time.
    #[must_use]
    pub fn last(self) -> Option<&'a Atom> {
        self.positions.last().map(|&position| &self.atoms[position])
    }

    /// Borrow the original matching atom using logarithmic typed comparisons.
    #[must_use]
    pub fn get(self, atom: &Atom) -> Option<&'a Atom> {
        self.positions
            .binary_search_by(|&position| self.atoms[position].cmp(atom))
            .ok()
            .map(|index| &self.atoms[self.positions[index]])
    }

    /// Exact membership using logarithmic typed comparisons.
    #[must_use]
    pub fn contains(self, atom: &Atom) -> bool {
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
    type Item = &'a Atom;
    type IntoIter = ModelIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Borrowed canonical traversal of a model's selected true atoms. Cloning copies
/// only cursor state. Every yielded atom is borrowed from the original catalog.
#[derive(Clone, Debug)]
pub struct ModelIter<'a> {
    atoms: &'a [Atom],
    positions: slice::Iter<'a, usize>,
}
impl<'a> Iterator for ModelIter<'a> {
    type Item = &'a Atom;
    fn next(&mut self) -> Option<Self::Item> {
        self.positions.next().map(|&position| &self.atoms[position])
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.positions.size_hint()
    }
}
impl DoubleEndedIterator for ModelIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.positions
            .next_back()
            .map(|&position| &self.atoms[position])
    }
}
impl ExactSizeIterator for ModelIter<'_> {}
impl FusedIterator for ModelIter<'_> {}

/// Interpretation selection failed, without a partial true set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// A supplied index has no atom in the retained catalog.
    Position {
        /// Refused dense position.
        position: usize,
        /// Number of atoms in this catalog.
        atoms: usize,
    },
    /// Selection-vector reservation failed.
    Allocation,
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Position { position, atoms } => {
                write!(
                    f,
                    "model position {position} is outside a catalog of {atoms} atoms"
                )
            }
            Self::Allocation => f.write_str("model selection storage could not be reserved"),
        }
    }
}
impl std::error::Error for ModelError {}
