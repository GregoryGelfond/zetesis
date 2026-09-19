//! One appendable typed atom owner with stable, owner-local dense positions.
//!
//! A committed prefix may be borrowed during a synchronous source scan while a
//! disjoint appender interns newly encountered identities. New identities do not
//! enter that prefix until commit. Neither existence nor commitment asserts
//! truth. The AVL index contains only positions and links; payload occurs once.
//! This unique-builder contract does not alter [`crate::AtomCatalog::new`], which
//! preserves arbitrary original order, duplicate positions and input addresses.

mod query;

use std::cmp::Ordering;
use std::{collections::TryReserveError, fmt};

use crate::{Atom, AtomKey, Predicate, identity, ordered_index as index};
use index::{Directions, Index, Link, Node, Step, position};
use query::Query;

/// Bounds on this interner's population and named storage, independent of truth.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum distinct atoms, including the uncommitted append tail.
    pub max_atoms: usize,
    /// Owner header, atom-vector capacities, AVL nodes and reusable path capacity,
    /// including conservative old/new buffer overlap and temporary ordered IDs.
    /// Nested Atom/Value payload, temporary borrowed-view stack headers and the
    /// fixed entry direction record (two usize words plus a checked length),
    /// allocator bookkeeping, caller storage and RSS
    /// are excluded and must be admitted separately by the enclosing owner.
    pub max_bytes: u128,
}

impl Limits {
    /// Derive a finite named-storage envelope from an admitted population.
    ///
    /// Uses the actual Atom, AVL-node and path-step layouts: three n-cell atom
    /// buffers, two n-cell node buffers, two bounded AVL paths, one n-cell order
    /// and its header, plus the owner header. This conservatively includes
    /// geometric old/new-buffer overlap; it is not a requirement to allocate all
    /// those buffers. The AVL path bound is twice the population bit width plus
    /// one, including the planned leaf. Actual allocator slack is rechecked and
    /// can still refuse. Nested payload and caller storage remain excluded.
    #[must_use]
    pub fn for_atoms(max_atoms: usize) -> Self {
        Self {
            max_atoms,
            max_bytes: size_of::<AtomInterner>() as u128
                + 3 * cells::<Atom>(max_atoms)
                + 2 * cells::<Node>(max_atoms)
                + 2 * cells::<Step>(path_bound(max_atoms))
                + cells::<usize>(max_atoms)
                + size_of::<Vec<usize>>() as u128,
        }
    }
}

/// Refusal to complete an index operation; never absence or logical rejection.
#[derive(Debug)]
pub enum Failure<E> {
    /// The distinct population would exceed its ceiling.
    Atoms {
        /// Requested total population.
        required: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// A named storage envelope exceeds its ceiling.
    Bytes {
        /// Requested bytes, including a reservation's old/new overlap.
        required: u128,
        /// Configured maximum.
        limit: u128,
    },
    /// A vector reservation failed.
    Allocation(TryReserveError),
    /// A size or position calculation is not representable.
    Overflow,
    /// The caller stopped work before the next operation.
    Stopped(E),
}

impl<E: fmt::Display> fmt::Display for Failure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Atoms { required, limit } => write!(
                f,
                "atom interner requests {required} atoms, limit is {limit}"
            ),
            Self::Bytes { required, limit } => write!(
                f,
                "atom interner requests {required} storage bytes, limit is {limit}"
            ),
            Self::Allocation(error) => write!(f, "atom interner reservation: {error}"),
            Self::Overflow => f.write_str("atom interner size is not representable"),
            Self::Stopped(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for Failure<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Stopped(error) => Some(error),
            Self::Atoms { .. } | Self::Bytes { .. } | Self::Overflow => None,
        }
    }
}

/// Authoritative unique atoms in first-insertion order, plus one ID-only AVL
/// index per predicate, kept in predicate order.
///
/// A lookup compares the predicate once, finding its relation among the few
/// the program names by a checked binary search, and then compares arguments
/// only along that relation's tree. Searches use O(log n) node probes and
/// checked typed comparisons without allocating or changing retained scratch. Entry records the initial search's
/// directions in fixed local stack state. Only a vacant entry replays child
/// links to prepare its mutation path, without repeating typed comparisons.
/// Insertion uses O(log n) path operations and at most two rotations. Vector growth is
/// geometric (work admission includes possible relocation of live cells even
/// for in-place allocator growth); commit moves only the pending suffix. Canonical order comes from
/// the tree, without sorting historical atoms or shifting a sorted index.
/// Positions are local to this owner; equal atoms in unrelated owners need not
/// have equal positions. There is no process-global interning or shared truth.
#[derive(Default)]
pub struct AtomInterner {
    committed: Vec<Atom>,
    pending: Vec<Atom>,
    /// The shared node index. Its own root goes unused here: each relation
    /// keeps the root of its subtree, and the index publishes nodes alone.
    index: Index,
    relations: Vec<Relation>,
}

/// One predicate's atoms: the root of its subtree in the shared node index.
/// Relations are kept in the predicate's canonical identity order, so the
/// trees in relation order give the canonical order of all atoms.
struct Relation {
    predicate: Predicate,
    root: Link,
}

/// Find the relation of `predicate`, or the position that keeps the relations
/// ordered if none exists, comparing predicates with the charged identity
/// comparator at each probe.
fn relation_of<E>(
    relations: &[Relation],
    predicate: &Predicate,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Result<usize, usize>, E> {
    let (mut start, mut end) = (0, relations.len());
    while start < end {
        let middle = start + (end - start) / 2;
        before()?;
        match identity::predicate(&relations[middle].predicate, predicate, before)? {
            Ordering::Less => start = middle + 1,
            Ordering::Greater => end = middle,
            Ordering::Equal => return Ok(Ok(middle)),
        }
    }
    Ok(Err(start))
}

impl AtomInterner {
    /// Create an empty owner without allocating.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Distinct committed and pending atom count. Constant time.
    #[must_use]
    pub fn len(&self) -> usize {
        self.committed.len() + self.pending.len()
    }

    /// Whether no identity has been inserted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow a local position, including its pending identity when present.
    #[must_use]
    pub fn get(&self, id: usize) -> Option<&Atom> {
        get(&self.committed, &self.pending, id)
    }

    /// Find an owned atom's local position in committed and pending identities.
    ///
    /// Borrows the owner immutably, allocates nothing and changes no payload,
    /// index, scratch or capacity observation. Calls `before` before each visited
    /// AVL node and each compared typed descriptor/text prefix. Search is
    /// logarithmic in node probes; payload comparison cost is additional.
    ///
    /// # Errors
    /// Current population and storage must satisfy `limits` before probing.
    /// Returns the first callback refusal without publishing an ID or absence.
    pub fn find_atom_with<E>(
        &self,
        atom: &Atom,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.find(Query::Atom(atom), limits, before)
    }

    /// Find a checked borrowed substitution without materializing an atom.
    /// Work, ownership and capacity contracts match [`Self::find_atom_with`].
    ///
    /// # Errors
    /// Same admission and callback failures as [`Self::find_atom_with`].
    pub fn find_key_with<E>(
        &self,
        key: AtomKey<'_>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        self.find(Query::Key(key), limits, before)
    }

    fn find<E>(
        &self,
        query: Query<'_>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<usize>, Failure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        let mut checked = || before().map_err(Failure::Stopped);
        let Ok(relation) = relation_of(&self.relations, query.predicate(), &mut checked)? else {
            return Ok(None);
        };
        query.search(
            &self.committed,
            &self.pending,
            &self.index.nodes,
            self.relations[relation].root,
            &mut checked,
            |_| {},
        )
    }

    /// Current named header and vector capacities, excluding nested payload.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        storage(
            self.committed.capacity(),
            self.pending.capacity(),
            self.index.nodes.capacity(),
            self.index.path.capacity(),
            self.relations.capacity(),
        )
    }

    /// Greatest admitted live capacity or actual reservation-overlap envelope.
    /// A preflighted proposal stopped before allocation does not increase it. Allocator slack
    /// acquired before a later refusal remains reflected here and in capacity.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.index.peak.max(self.storage_bytes())
    }

    /// Borrow the current committed prefix without mutating the owner.
    #[must_use]
    pub fn committed(&self) -> CommittedAtoms<'_> {
        CommittedAtoms {
            atoms: &self.committed,
        }
    }

    /// Split committed rows from a disjoint append capability. Neither borrow
    /// can survive commit. Preparing row/membership selections before this call
    /// lets a synchronous scanner coexist with append and index growth.
    #[must_use]
    pub fn split(&mut self) -> (CommittedAtoms<'_>, AtomAppender<'_>) {
        let capacity = self.committed.capacity();
        (
            CommittedAtoms {
                atoms: &self.committed,
            },
            AtomAppender {
                committed: &self.committed,
                committed_capacity: capacity,
                pending: &mut self.pending,
                index: &mut self.index,
                relations: &mut self.relations,
            },
        )
    }

    /// Borrow an appender without retaining a separate committed view.
    #[must_use]
    pub fn appender(&mut self) -> AtomAppender<'_> {
        self.split().1
    }

    /// Find or prepare an entry from an already owned atom, without copying it.
    /// Entry records each descent in fixed local stack state without changing
    /// retained scratch. The charged node operation includes that constant-size
    /// recording; typed comparison work is separate. Only a vacant entry replays
    /// the recorded child links to prepare its insertion path. The two target-
    /// sized words and checked length live only through this call.
    ///
    /// # Errors
    /// Returns the first work, storage or reservation refusal. Vacant-path
    /// preparation may retain grown scratch capacity; membership is unchanged.
    pub fn entry_atom_with<'owner, 'key, E>(
        &'owner mut self,
        atom: &'key Atom,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.appender().entry(Query::Atom(atom), limits, before)
    }

    /// Find or prepare an entry using a borrowed checked substitution.
    ///
    /// # Errors
    /// Same failure and retained-capacity contract as [`Self::entry_atom_with`].
    pub fn entry_key_with<'owner, 'key, E>(
        &'owner mut self,
        key: AtomKey<'key>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.appender().entry(Query::Key(key), limits, before)
    }

    /// Return committed positions in exact typed Atom order. Owns only the
    /// positions; no Atom is copied. Iterative inorder traversal visits O(n)
    /// index nodes, including any pending identities, and filters the prefix.
    /// Reuses the interner's checked path scratch. The returned vector's capacity
    /// is included in the operation's peak and must be retained by its consumer.
    ///
    /// # Errors
    /// Refuses work, storage or reservation without returning a partial order.
    /// Existing atoms and index remain unchanged; scratch capacity may grow.
    pub fn ordered_ids_with<E>(
        &mut self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<usize>, Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        let mut output = Vec::new();
        let count = self.committed.len();
        population(self.len(), limits)?;
        let current = self.storage_bytes() + size_of::<Vec<usize>>() as u128;
        reserve(
            &mut output,
            count,
            count,
            current,
            &mut self.index.peak,
            limits,
            &mut checked,
        )?;
        self.index.path.clear();
        // Relations are in predicate order and each tree in argument order,
        // so this visits every atom in canonical order.
        for relation in 0..self.relations.len() {
            checked()?;
            let mut cursor = self.relations[relation].root;
            loop {
                while let Some(next) = cursor {
                    checked()?;
                    let id = position(next);
                    let node = self.index.nodes[id];
                    let live = self.storage_bytes()
                        + size_of::<Vec<usize>>() as u128
                        + cells::<usize>(output.capacity());
                    let bound = path_bound(self.len());
                    reserve(
                        &mut self.index.path,
                        1,
                        bound,
                        live,
                        &mut self.index.peak,
                        limits,
                        &mut checked,
                    )?;
                    checked()?;
                    self.index.path.push(Step {
                        id,
                        node,
                        right: false,
                        changed: false,
                    });
                    cursor = node.children[0];
                }
                let Some(step) = self.index.path.pop() else {
                    break;
                };
                checked()?;
                if step.id < count {
                    checked()?;
                    output.push(step.id);
                }
                cursor = step.node.children[1];
            }
        }
        Ok(output)
    }

    /// Move the pending suffix into the committed vector, preserving every local
    /// position. Borrowing prevents this while a scan or appender is retained.
    /// Each constant-size Atom move is pre-admitted before the indivisible move;
    /// nested allocations are transferred, never cloned. An empty committed prefix
    /// takes the pending vector in one admitted owner swap without allocation.
    ///
    /// # Errors
    /// On refusal no membership or prefix changes. A successful capacity growth
    /// may remain. Bounds cover old/new committed buffers and the live tail.
    pub fn commit_with<E>(
        &mut self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        let live = self.storage_bytes();
        admit(live, limits)?;
        population(self.len(), limits)?;
        let additional = self.pending.len();
        if additional == 0 {
            return Ok(());
        }
        if self.committed.is_empty() {
            checked()?;
            std::mem::swap(&mut self.committed, &mut self.pending);
            return Ok(());
        }
        reserve(
            &mut self.committed,
            additional,
            limits.max_atoms,
            live,
            &mut self.index.peak,
            limits,
            &mut checked,
        )?;
        for _ in &self.pending {
            checked()?;
        }
        self.committed.append(&mut self.pending);
        Ok(())
    }

    /// Commit and transfer the one dense vector into a caller's final owner.
    /// Index/scratch/tail storage is dropped. Original insertion order survives.
    ///
    /// # Errors
    /// A commit refusal consumes this builder and drops its owned data. Use
    /// [`Self::commit_with`] first when failure recovery needs the builder.
    pub fn into_atoms_with<E>(
        mut self,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<Atom>, Failure<E>> {
        self.commit_with(limits, before)?;
        Ok(self.committed)
    }
}

/// Immutable committed prefix, independent of the disjoint mutable append tail.
#[derive(Clone, Copy)]
pub struct CommittedAtoms<'a> {
    atoms: &'a [Atom],
}
impl<'a> CommittedAtoms<'a> {
    /// Prefix length; later pending identities are excluded.
    #[must_use]
    pub const fn len(self) -> usize {
        self.atoms.len()
    }
    /// Whether the prefix has no identities.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.atoms.is_empty()
    }
    /// Borrow an admitted prefix position, or return absence outside that prefix.
    #[must_use]
    pub fn get(self, id: usize) -> Option<&'a Atom> {
        self.atoms.get(id)
    }
    /// Borrow the exact committed dense sequence without copying.
    #[must_use]
    pub const fn as_slice(self) -> &'a [Atom] {
        self.atoms
    }
}

/// Scoped append capability that cannot mutate committed Atom payloads.
pub struct AtomAppender<'a> {
    committed: &'a [Atom],
    committed_capacity: usize,
    pending: &'a mut Vec<Atom>,
    index: &'a mut Index,
    relations: &'a mut Vec<Relation>,
}
impl AtomAppender<'_> {
    /// Distinct committed plus pending count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.committed.len() + self.pending.len()
    }
    /// Whether no identities exist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Borrow one local committed or pending identity.
    #[must_use]
    pub fn get(&self, id: usize) -> Option<&Atom> {
        get(self.committed, self.pending, id)
    }
    /// Current named capacity, including prior insertion/order scratch.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        storage(
            self.committed_capacity,
            self.pending.capacity(),
            self.index.nodes.capacity(),
            self.index.path.capacity(),
            self.relations.capacity(),
        )
    }
    /// Peak named conservative capacity; nested payload remains caller-owned accounting.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.index.peak.max(self.storage_bytes())
    }

    fn reborrow(&mut self) -> AtomAppender<'_> {
        AtomAppender {
            committed: self.committed,
            committed_capacity: self.committed_capacity,
            pending: self.pending,
            index: self.index,
            relations: self.relations,
        }
    }

    /// Search without copying an owned atom; prepare a path only when vacant.
    /// Node work includes fixed local direction recording. Occupied lookup
    /// changes no retained scratch; a miss replays links without comparing keys.
    ///
    /// # Errors
    /// Refusal changes no membership. Vacant-path preparation may retain
    /// admitted scratch capacity.
    pub fn entry_atom_with<'owner, 'key, E>(
        &'owner mut self,
        atom: &'key Atom,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.reborrow().entry(Query::Atom(atom), limits, before)
    }
    /// Search a checked borrowed substitution without materialization.
    ///
    /// # Errors
    /// Same failure contract as [`Self::entry_atom_with`].
    pub fn entry_key_with<'owner, 'key, E>(
        &'owner mut self,
        key: AtomKey<'key>,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'owner, 'key>, Failure<E>> {
        self.reborrow().entry(Query::Key(key), limits, before)
    }
}
impl<'a> AtomAppender<'a> {
    fn entry<'key, E>(
        mut self,
        query: Query<'key>,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<AtomEntry<'a, 'key>, Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        let relation = relation_of(self.relations, query.predicate(), &mut checked)?;
        let mut directions = Directions::default();
        let found = match relation {
            Ok(relation) => query.search(
                self.committed,
                self.pending,
                &self.index.nodes,
                self.relations[relation].root,
                &mut checked,
                |right| directions.push(right).expect("AVL height fits two words"),
            )?,
            Err(_) => None,
        };
        if found.is_none() {
            let root = relation.map_or(None, |relation| self.relations[relation].root);
            self.prepare_path(root, &directions, limits, &mut checked)?;
        }
        Ok(AtomEntry {
            appender: self,
            query,
            relation,
            found,
        })
    }

    fn prepare_path<E>(
        &mut self,
        root: Link,
        directions: &Directions,
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), Failure<E>>,
    ) -> Result<(), Failure<E>> {
        let bound = path_bound(self.len());
        let fixed = storage(
            self.committed_capacity,
            self.pending.capacity(),
            self.index.nodes.capacity(),
            0,
            self.relations.capacity(),
        );
        let Index {
            nodes, path, peak, ..
        } = &mut *self.index;
        path.clear();
        // The exclusive entry borrow has prevented any node/link change since
        // this route reached absence. Inductively, replay starts at the same
        // root and each recorded direction reaches the same next node. Thus it
        // ends at that absent child; no second typed comparison is required.
        // Refusal changes only disposable path scratch, never published links.
        let mut cursor = root;
        for offset in 0..directions.len() {
            before()?; // Replayed node and its constant-size direction decode.
            let right = directions.get(offset).expect("recorded direction");
            let id = position(cursor.expect("vacant route retains its nodes"));
            let node = nodes[id];
            let live = fixed + cells::<Step>(path.capacity());
            reserve(path, 1, bound, live, peak, limits, before)?;
            before()?;
            path.push(Step {
                id,
                node,
                right,
                changed: false,
            });
            cursor = node.children[usize::from(right)];
        }
        assert!(cursor.is_none(), "exclusive vacant route reaches absence");
        Ok(())
    }
}

/// One checked lookup, holding the exclusive append path until insertion/drop.
/// The key is borrowed. Occupied lookup never copies payload or mutation scratch.
pub struct AtomEntry<'owner, 'key> {
    appender: AtomAppender<'owner>,
    query: Query<'key>,
    /// The predicate's relation, or where a new one keeps the relations ordered.
    relation: Result<usize, usize>,
    found: Option<usize>,
}
impl AtomEntry<'_, '_> {
    /// Existing local position, or a vacant insertion capability.
    #[must_use]
    pub const fn position(&self) -> Option<usize> {
        self.found
    }
    /// Current named storage after possible vacant-path preparation.
    #[must_use]
    pub fn storage_bytes(&self) -> u128 {
        self.appender.storage_bytes()
    }
    /// Current peak including actual retained vacant-path capacity.
    #[must_use]
    pub fn storage_peak_bytes(&self) -> u128 {
        self.appender.storage_peak_bytes()
    }

    /// Return the existing position or publish one new authoritative atom.
    ///
    /// The caller separately admits nested payload. Copy admission charges the
    /// predicate and each value descriptor, plus
    /// copied predicate/string/symbol bytes, before the one Atom clone. Structural
    /// values retain their existing shared payload and constant-size clone cost.
    /// Owned Atom/Value nested allocations retain their existing infallible
    /// contract. Metadata planning and all publication writes are
    /// pre-admitted before an indivisible update. A refused write admission can
    /// therefore count charged work without publishing that update.
    ///
    /// # Errors
    /// Refusal publishes neither a new atom nor partial AVL links. Actual vector
    /// capacity acquired before a later refusal is retained and observable.
    pub fn insert_with<E>(
        self,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        population(self.appender.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        if let Some(id) = self.found {
            return Ok(id);
        }
        let id = self.appender.len();
        let required = id.checked_add(1).ok_or(Failure::Overflow)?;
        population(required, limits)?;
        let live = self.storage_bytes();
        reserve(
            self.appender.pending,
            1,
            limits.max_atoms,
            live,
            &mut self.appender.index.peak,
            limits,
            &mut checked,
        )?;
        let live = self.storage_bytes();
        reserve(
            &mut self.appender.index.nodes,
            1,
            limits.max_atoms,
            live,
            &mut self.appender.index.peak,
            limits,
            &mut checked,
        )?;
        let live = self.storage_bytes();
        reserve(
            &mut self.appender.index.path,
            1,
            path_bound(required),
            live,
            &mut self.appender.index.peak,
            limits,
            &mut checked,
        )?;
        if self.relation.is_err() {
            let live = self.storage_bytes();
            reserve(
                self.appender.relations,
                1,
                limits.max_atoms,
                live,
                &mut self.appender.index.peak,
                limits,
                &mut checked,
            )?;
        }
        checked()?;
        self.appender.index.path.push(Step {
            id,
            node: Node::default(),
            right: false,
            changed: true,
        });
        let previous = self
            .relation
            .map_or(None, |relation| self.appender.relations[relation].root);
        let root = self.appender.index.plan_from(previous, id, &mut checked)?;
        self.query.prepare_copy(&mut checked)?;
        if let Err(at) = self.relation {
            // The relation's own predicate copy, and each later relation moved.
            for _ in self.query.predicate().name().as_bytes() {
                checked()?;
            }
            for _ in at..self.appender.relations.len() {
                checked()?;
            }
        }
        checked()?; // Authoritative atom-vector write.
        checked()?; // New AVL node.
        for step in &self.appender.index.path[..self.appender.index.path.len() - 1] {
            if step.changed {
                checked()?;
            }
        }
        checked()?; // Root publication.
        let mut atom = self.query.to_atom();
        self.appender.index.publish_nodes();
        match self.relation {
            Ok(relation) => {
                // Every atom of the relation refers to the relation's one name.
                atom.share_predicate(self.appender.relations[relation].predicate.clone());
                self.appender.relations[relation].root = root;
            }
            Err(at) => self.appender.relations.insert(
                at,
                Relation {
                    predicate: atom.predicate().clone(),
                    root,
                },
            ),
        }
        self.appender.pending.push(atom);
        Ok(id)
    }
}

fn get<'a>(committed: &'a [Atom], pending: &'a [Atom], id: usize) -> Option<&'a Atom> {
    if id < committed.len() {
        committed.get(id)
    } else {
        pending.get(id - committed.len())
    }
}

fn storage(committed: usize, pending: usize, nodes: usize, path: usize, relations: usize) -> u128 {
    size_of::<AtomInterner>() as u128
        + cells::<Atom>(committed)
        + cells::<Atom>(pending)
        + cells::<Node>(nodes)
        + cells::<Step>(path)
        + cells::<Relation>(relations)
}
fn cells<T>(count: usize) -> u128 {
    count as u128 * size_of::<T>() as u128
}
fn admit<E>(required: u128, limits: Limits) -> Result<(), Failure<E>> {
    if required > limits.max_bytes {
        Err(Failure::Bytes {
            required,
            limit: limits.max_bytes,
        })
    } else {
        Ok(())
    }
}
fn population<E>(required: usize, limits: Limits) -> Result<(), Failure<E>> {
    if required > limits.max_atoms {
        Err(Failure::Atoms {
            required,
            limit: limits.max_atoms,
        })
    } else {
        Ok(())
    }
}
fn path_bound(count: usize) -> usize {
    2 * (usize::BITS - count.leading_zeros()) as usize + 1
}

fn reserve<T, E>(
    values: &mut Vec<T>,
    additional: usize,
    maximum: usize,
    live: u128,
    peak: &mut u128,
    limits: Limits,
    before: &mut impl FnMut() -> Result<(), Failure<E>>,
) -> Result<(), Failure<E>> {
    admit(live, limits)?;
    *peak = (*peak).max(live);
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(Failure::Overflow)?;
    if required <= values.capacity() {
        return Ok(());
    }
    let capacity = values
        .capacity()
        .checked_mul(2)
        .unwrap_or(maximum)
        .max(4)
        .min(maximum)
        .max(required);
    let envelope = live + cells::<T>(capacity);
    admit(envelope, limits)?;
    before()?;
    // Admit possible relocation of every live cell before requesting growth.
    // This remains a conservative charge when an allocator grows in place.
    for _ in values.iter() {
        before()?;
    }
    let old = values.capacity();
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(Failure::Allocation)?;
    let actual = live + cells::<T>(values.capacity());
    *peak = (*peak).max(actual);
    // `live` already includes the old buffer. Keeping it in this envelope is
    // conservative even when the allocator grew that buffer in place.
    admit(actual, limits)?;
    debug_assert!(values.capacity() >= old);
    Ok(())
}

#[cfg(test)]
mod tests;
