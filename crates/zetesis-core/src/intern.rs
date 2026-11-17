//! The interner: one source of identity and order for interned content.
//!
//! Content is a string of `u32` words whose meaning belongs to the caller's
//! [`ContentOrder`]: a term's constructor and argument identities, an atom's
//! predicate and arguments, or a fact's tuple. The interner decides only
//! equality (equal words) and, through that one order, the order of identities.
//!
//! # Concurrency
//! Content lives in `S` shards chosen by the top bits of its content hash. Each
//! shard keeps its frozen entries in a [`hashbrown::HashTable`] of identities
//! that is immutable for the whole generation, so a lookup of frozen content
//! takes no lock. Content new in this generation enters its shard's pending
//! table under that one shard's lock; other shards stay available. A table
//! holds only `u32` identities and resolves equality against the shard's
//! append-only word arena, so a lookup that finds its content allocates
//! nothing: memory is allocated only on a miss.
//!
//! # Identities
//! A pending entry has a provisional [`Pending`] coordinate. [`Interner::freeze`]
//! ends the generation: it sorts the generation's new content once by the one
//! comparison and gives it dense final [`Id`]s in that order, after every
//! earlier generation's identities. The result depends only on the set of
//! content interned, never on which thread interned it first or in what order.
//!
//! # Order
//! [`Interner::compare`] is the only source of order between identities; [`Id`]
//! implements neither `Ord` nor `PartialOrd`. Within one generation identity
//! order equals the content order (the Lean law `IdOrderEqCompare` in
//! `proofs/Zetesis/ContentOrderInterning.lean`), so `compare` decides such a pair
//! by its identities alone; a pair from different generations compares content.
//!
//! # Work
//! Every operation adds deterministic counts to a caller-owned [`Work`]: words
//! hashed, table probes, content comparisons and words written. A generation's
//! totals depend only on the multiset of requests, so they are the same at
//! every worker count; [`freeze`](Interner::freeze) sorts fixed-size blocks and
//! merges them pairwise, so its comparisons do not depend on the worker count
//! either.

mod sort;
#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::sync::{Mutex, PoisonError};

use hashbrown::HashTable;
use zetesis_algebra::threads::Pool;

pub use sort::{BLOCK, sort_by_content};

/// The number of shards. A power of two, so a shard is a hash's top bits.
pub const SHARDS: usize = 64;
const SHARD_BITS: u32 = SHARDS.trailing_zeros();

/// A final identity: dense, in content order within its generation.
///
/// Deliberately not `Ord`: order between identities comes only from
/// [`Interner::compare`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(u32);

impl Id {
    /// The identity's dense index, for columns and remap tables.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The identity's packed word, for columnar storage.
    #[must_use]
    pub const fn word(self) -> u32 {
        self.0
    }

    /// The identity a packed word names. Valid only for words this interner
    /// produced with [`Self::word`].
    #[must_use]
    pub const fn from_word(word: u32) -> Self {
        Self(word)
    }
}

/// A provisional coordinate of content interned in the open generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pending {
    shard: u32,
    local: u32,
}

/// The identity an insert produced: final if the content was frozen in an
/// earlier generation, otherwise provisional until the next freeze.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    /// Content frozen in an earlier generation.
    Final(Id),
    /// Content new in the open generation.
    Pending(Pending),
}

/// Deterministic operation counts a caller charges against its work limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Content words folded into a hash.
    pub hashed: u64,
    /// Hash-table probes, one per table visited.
    pub probes: u64,
    /// Content comparisons: an equality check on a hash match, or an order
    /// comparison while freezing.
    pub compares: u64,
    /// Words appended to an arena or identities written.
    pub written: u64,
}

impl Work {
    /// The sum of all counts, one unit each.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.hashed + self.probes + self.compares + self.written
    }

    /// Add another tally to this one.
    pub const fn add(&mut self, other: Self) {
        self.hashed += other.hashed;
        self.probes += other.probes;
        self.compares += other.compares;
        self.written += other.written;
    }
}

/// The one comparison of contents, a total order whose equality is equality
/// of words. Content words that name identities name final identities of
/// earlier generations, compared through `ids`.
pub trait ContentOrder: Sync {
    /// Compare two contents.
    fn compare(&self, ids: Frozen<'_>, left: &[u32], right: &[u32]) -> Ordering;
}

/// Lexicographic word order: the content order of flat words with no
/// identities inside, such as packed constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct Words;

impl ContentOrder for Words {
    fn compare(&self, _: Frozen<'_>, left: &[u32], right: &[u32]) -> Ordering {
        left.cmp(right)
    }
}

/// A refusal to intern; never absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The identity space or a content length does not fit `u32`.
    Exhausted,
    /// A reservation failed.
    Allocation,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Exhausted => "interner identity space exhausted",
            Self::Allocation => "interner reservation failed",
        })
    }
}

impl std::error::Error for Refusal {}

/// Content words with their span in an arena.
#[derive(Debug, Default)]
struct Arena {
    words: Vec<u32>,
    /// Start of each entry; entry `i` ends where `i + 1` starts.
    starts: Vec<u32>,
}

impl Arena {
    fn get(&self, entry: usize) -> &[u32] {
        let start = self.starts[entry] as usize;
        let end = self
            .starts
            .get(entry + 1)
            .map_or(self.words.len(), |&end| end as usize);
        &self.words[start..end]
    }

    fn len(&self) -> usize {
        self.starts.len()
    }

    fn push(&mut self, content: &[u32]) -> Result<u32, Refusal> {
        let entry = u32::try_from(self.starts.len()).map_err(|_| Refusal::Exhausted)?;
        let start = u32::try_from(self.words.len()).map_err(|_| Refusal::Exhausted)?;
        u32::try_from(self.words.len() + content.len()).map_err(|_| Refusal::Exhausted)?;
        self.starts
            .try_reserve(1)
            .map_err(|_| Refusal::Allocation)?;
        self.words
            .try_reserve(content.len())
            .map_err(|_| Refusal::Allocation)?;
        self.starts.push(start);
        self.words.extend_from_slice(content);
        Ok(entry)
    }

    fn bytes(&self) -> u128 {
        (self.words.capacity() as u128 + self.starts.capacity() as u128) * 4
    }
}

/// A shard's open-generation entries: provisional locals over its own arena.
#[derive(Debug, Default)]
struct Open {
    arena: Arena,
    table: HashTable<u32>,
    hashes: Vec<u64>,
}

#[derive(Debug, Default)]
struct Shard {
    /// Final identities of frozen content whose hash falls in this shard.
    frozen: HashTable<u32>,
    open: Mutex<Open>,
}

/// A read-only view of frozen content and its order, for [`ContentOrder`].
#[derive(Clone, Copy)]
pub struct Frozen<'a> {
    arena: &'a Arena,
    generations: &'a [u32],
    order: &'a dyn ContentOrder,
}

impl Frozen<'_> {
    /// The content of a final identity.
    #[must_use]
    pub fn content(&self, id: Id) -> &[u32] {
        self.arena.get(id.index())
    }

    /// The generation that froze `id`.
    fn generation(&self, id: Id) -> usize {
        self.generations.partition_point(|&start| start <= id.0) - 1
    }

    /// Compare two final identities: by identity within one generation (the
    /// law), by content across generations.
    #[must_use]
    pub fn compare(&self, left: Id, right: Id) -> Ordering {
        if left == right {
            return Ordering::Equal;
        }
        if self.generation(left) == self.generation(right) {
            return left.0.cmp(&right.0);
        }
        self.order
            .compare(*self, self.content(left), self.content(right))
    }
}

/// The interner. See the [module documentation](self).
pub struct Interner<O> {
    order: O,
    shards: Box<[Shard]>,
    /// Content of every final identity, in identity order.
    arena: Arena,
    /// First identity of each completed generation, then of the open one;
    /// nondecreasing, starting with 0.
    generations: Vec<u32>,
}

impl<O: ContentOrder> Interner<O> {
    /// An empty interner whose one order is `order`.
    #[must_use]
    pub fn new(order: O) -> Self {
        Self {
            order,
            shards: (0..SHARDS).map(|_| Shard::default()).collect(),
            arena: Arena::default(),
            generations: vec![0],
        }
    }

    /// The one comparison: a view of frozen content and its order.
    #[must_use]
    pub fn frozen(&self) -> Frozen<'_> {
        Frozen {
            arena: &self.arena,
            generations: &self.generations,
            order: &self.order,
        }
    }

    /// Compare two final identities. The only source of identity order.
    #[must_use]
    pub fn compare(&self, left: Id, right: Id) -> Ordering {
        self.frozen().compare(left, right)
    }

    /// The content of a final identity.
    #[must_use]
    pub fn content(&self, id: Id) -> &[u32] {
        self.arena.get(id.index())
    }

    /// Final identities so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Whether nothing is frozen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.arena.len() == 0
    }

    /// Completed generations.
    #[must_use]
    pub fn generations(&self) -> usize {
        self.generations.len() - 1
    }

    /// The identity range a completed generation assigned.
    #[must_use]
    pub fn generation_range(&self, generation: usize) -> std::ops::Range<usize> {
        let start = self.generations[generation] as usize;
        let end = self
            .generations
            .get(generation + 1)
            .map_or(self.arena.len(), |&end| end as usize);
        start..end
    }

    fn hash(content: &[u32], work: &mut Work) -> u64 {
        work.hashed += content.len() as u64 + 1;
        hash_of(content)
    }

    fn shard(hash: u64) -> usize {
        usize::try_from(hash >> (64 - SHARD_BITS)).expect("a shard index fits usize")
    }

    /// The final identity of frozen content, without a lock or an allocation.
    #[must_use]
    pub fn lookup(&self, content: &[u32], work: &mut Work) -> Option<Id> {
        let hash = Self::hash(content, work);
        self.lookup_hashed(content, hash, work)
    }

    fn lookup_hashed(&self, content: &[u32], hash: u64, work: &mut Work) -> Option<Id> {
        work.probes += 1;
        let arena = &self.arena;
        self.shards[Self::shard(hash)]
            .frozen
            .find(hash, |&id| {
                work.compares += 1;
                arena.get(id as usize) == content
            })
            .map(|&id| Id(id))
    }

    /// Intern `content`: its final identity if frozen, else its provisional
    /// coordinate, added under its shard's lock if new.
    ///
    /// # Errors
    /// Refuses when the identity space or a reservation is exhausted; the
    /// interner is unchanged.
    ///
    /// # Panics
    /// Never: the shard count fits `u32`.
    pub fn intern(&self, content: &[u32], work: &mut Work) -> Result<Handle, Refusal> {
        let hash = Self::hash(content, work);
        if let Some(id) = self.lookup_hashed(content, hash, work) {
            return Ok(Handle::Final(id));
        }
        let shard = Self::shard(hash);
        let mut open = self.shards[shard]
            .open
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Open {
            arena,
            table,
            hashes,
        } = &mut *open;
        work.probes += 1;
        let found = table.find(hash, |&local| {
            work.compares += 1;
            arena.get(local as usize) == content
        });
        let shard = u32::try_from(shard).expect("shard count fits u32");
        if let Some(&local) = found {
            return Ok(Handle::Pending(Pending { shard, local }));
        }
        hashes.try_reserve(1).map_err(|_| Refusal::Allocation)?;
        table
            .try_reserve(1, |&local| hashes[local as usize])
            .map_err(|_| Refusal::Allocation)?;
        let local = arena.push(content)?;
        hashes.push(hash);
        table.insert_unique(hash, local, |&local| hashes[local as usize]);
        work.written += content.len() as u64 + 1;
        Ok(Handle::Pending(Pending { shard, local }))
    }

    /// The content of a provisional coordinate in the open generation.
    ///
    /// # Panics
    /// If the coordinate is not from this interner's open generation.
    #[must_use]
    pub fn pending_content<T>(&self, pending: Pending, read: impl FnOnce(&[u32]) -> T) -> T {
        let open = self.shards[pending.shard as usize]
            .open
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        read(open.arena.get(pending.local as usize))
    }

    /// Entries waiting for the next freeze.
    #[must_use]
    pub fn pending_len(&mut self) -> usize {
        self.shards
            .iter_mut()
            .map(|shard| {
                shard
                    .open
                    .get_mut()
                    .unwrap_or_else(PoisonError::into_inner)
                    .arena
                    .len()
            })
            .sum()
    }

    /// End the open generation: sort its content once by the one order and
    /// give it dense final identities in that order, after all earlier ones.
    /// `pool` runs the sort's blocks and the shards' table updates; `None`
    /// runs them on the calling thread with identical results and work.
    ///
    /// # Errors
    /// Refuses when the identity space or a reservation is exhausted. On
    /// refusal the open generation is unchanged.
    ///
    /// # Panics
    /// Never: identities are checked against `u32` before assignment.
    pub fn freeze(&mut self, pool: Option<&Pool>, work: &mut Work) -> Result<Generation, Refusal> {
        let shards: Vec<Open> = self
            .shards
            .iter_mut()
            .map(|shard| {
                std::mem::take(shard.open.get_mut().unwrap_or_else(PoisonError::into_inner))
            })
            .collect();
        let count: usize = shards.iter().map(|open| open.arena.len()).sum();
        let base = self.arena.len();
        if count == 0 {
            self.restore(shards);
            return Ok(Generation {
                remap: Vec::new(),
                base,
            });
        }
        if u32::try_from(base + count).is_err() {
            self.restore(shards);
            return Err(Refusal::Exhausted);
        }
        let mut entries: Vec<Pending> = Vec::new();
        if entries.try_reserve_exact(count).is_err() {
            self.restore(shards);
            return Err(Refusal::Allocation);
        }
        for (shard, open) in shards.iter().enumerate() {
            let shard = u32::try_from(shard).expect("shard count fits u32");
            entries.extend((0..open.arena.len()).map(|local| Pending {
                shard,
                local: u32::try_from(local).expect("arena entries fit u32"),
            }));
        }
        let frozen = self.frozen();
        let content =
            |entry: &Pending| shards[entry.shard as usize].arena.get(entry.local as usize);
        let hash = |entry: &Pending| shards[entry.shard as usize].hashes[entry.local as usize];
        // Arrival order within a shard depends on thread timing. Start the
        // content sort from the hash order instead, so the comparisons it
        // makes depend only on the generation's content. Equal hashes are
        // distinct contents; their content order breaks the tie.
        entries.sort_unstable_by(|left, right| {
            hash(left)
                .cmp(&hash(right))
                .then_with(|| self.order.compare(frozen, content(left), content(right)))
        });
        work.written += count as u64;
        work.compares += sort_by_content(&mut entries, pool, |left, right| {
            self.order.compare(frozen, content(left), content(right))
        });
        // Remap tables: each shard's locals to their final identities.
        let mut remap: Vec<Vec<Id>> = shards
            .iter()
            .map(|open| vec![Id(0); open.arena.len()])
            .collect();
        let total_words: usize = shards.iter().map(|open| open.arena.words.len()).sum();
        if self.arena.words.try_reserve(total_words).is_err()
            || self.arena.starts.try_reserve(count).is_err()
        {
            self.restore(shards);
            return Err(Refusal::Allocation);
        }
        for (offset, entry) in entries.iter().enumerate() {
            let id = Id(u32::try_from(base + offset).expect("checked identity space"));
            let pushed = self.arena.push(content(entry)).expect("reserved arena");
            debug_assert_eq!(pushed, id.0);
            remap[entry.shard as usize][entry.local as usize] = id;
            work.written += 1;
        }
        // Each shard's frozen table gains its own new identities; shards are
        // independent, so they update in parallel.
        let arena = &self.arena;
        let update = |(shard, (open, remap)): (&mut Shard, (&Open, &Vec<Id>))| {
            shard
                .frozen
                .reserve(open.arena.len(), |&id| hash_of(arena.get(id as usize)));
            for (local, id) in remap.iter().enumerate() {
                shard.frozen.insert_unique(open.hashes[local], id.0, |&id| {
                    hash_of(arena.get(id as usize))
                });
            }
        };
        let pairs = self.shards.iter_mut().zip(shards.iter().zip(remap.iter()));
        match pool {
            Some(pool) => pool.scope(|scope| {
                let update = &update;
                for pair in pairs {
                    scope.spawn(move |_| update(pair));
                }
            }),
            None => pairs.for_each(update),
        }
        work.written += count as u64;
        // The next generation starts after this one's identities.
        self.generations
            .push(u32::try_from(base + count).expect("checked identity space"));
        Ok(Generation { remap, base })
    }

    fn restore(&mut self, shards: Vec<Open>) {
        for (shard, open) in self.shards.iter_mut().zip(shards) {
            *shard.open.get_mut().unwrap_or_else(PoisonError::into_inner) = open;
        }
    }

    /// Every final identity in content order: the generations' sorted runs
    /// merged by the one comparison.
    ///
    /// # Panics
    /// Never: every identity fits `u32`.
    #[must_use]
    pub fn ordered(&self, work: &mut Work) -> Vec<Id> {
        let frozen = self.frozen();
        let mut runs: Vec<Vec<Id>> = (0..self.generations())
            .map(|generation| {
                self.generation_range(generation)
                    .map(|index| Id(u32::try_from(index).expect("identity fits u32")))
                    .collect()
            })
            .collect();
        while runs.len() > 1 {
            let mut merged = Vec::with_capacity(runs.len().div_ceil(2));
            let mut pairs = runs.into_iter();
            while let Some(left) = pairs.next() {
                match pairs.next() {
                    Some(right) => {
                        merged.push(sort::merge(&left, &right, &mut work.compares, |a, b| {
                            frozen.compare(*a, *b)
                        }));
                    }
                    None => merged.push(left),
                }
            }
            runs = merged;
        }
        runs.pop().unwrap_or_default()
    }

    /// Bytes retained by tables and arenas, by capacity.
    #[must_use]
    pub fn bytes(&mut self) -> u128 {
        let shards: u128 = self
            .shards
            .iter_mut()
            .map(|shard| {
                let open = shard.open.get_mut().unwrap_or_else(PoisonError::into_inner);
                table_bytes(&shard.frozen)
                    + table_bytes(&open.table)
                    + open.arena.bytes()
                    + open.hashes.capacity() as u128 * 8
            })
            .sum();
        shards + self.arena.bytes() + self.generations.capacity() as u128 * 4
    }
}

impl<O> std::fmt::Debug for Interner<O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interner")
            .field("final", &self.arena.len())
            .field("generations", &(self.generations.len() - 1))
            .finish_non_exhaustive()
    }
}

/// One table's identity cells and control bytes, by capacity.
fn table_bytes(table: &HashTable<u32>) -> u128 {
    table.capacity() as u128 * (size_of::<u32>() as u128 + 1)
}

/// The content hash without a work tally, for table growth.
fn hash_of(content: &[u32]) -> u64 {
    content
        .iter()
        .fold(crate::mix_word(0, content.len() as u64), |state, &word| {
            crate::mix_word(state, u64::from(word))
        })
}

/// A completed generation's remap from provisional coordinates to identities.
#[derive(Debug)]
pub struct Generation {
    remap: Vec<Vec<Id>>,
    base: usize,
}

impl Generation {
    /// The final identity a provisional coordinate of this generation received.
    #[must_use]
    pub fn resolve(&self, pending: Pending) -> Id {
        self.remap[pending.shard as usize][pending.local as usize]
    }

    /// A handle's final identity: unchanged if already final.
    #[must_use]
    pub fn finalize(&self, handle: Handle) -> Id {
        match handle {
            Handle::Final(id) => id,
            Handle::Pending(pending) => self.resolve(pending),
        }
    }

    /// The first identity of this generation.
    #[must_use]
    pub const fn base(&self) -> usize {
        self.base
    }
}
