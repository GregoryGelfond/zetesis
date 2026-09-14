//! Independent admission and receipts for exact candidate exclusion history.

/// Finite ceilings for the retained exact semantic-projection trie.
/// These do not consume the candidate or reduct CNF's admission population.
#[derive(Clone, Copy, Debug)]
pub struct ProjectionLimits {
    /// Maximum distinct excluded keys. Repeating a key consumes no new entry.
    /// A zero-width projection has one possible key, which uses no trie nodes.
    pub max_entries: u64,
    /// Maximum logical trie elements, including root and terminal elements.
    /// Spare vector capacity is separately admitted by `max_bytes`.
    pub max_nodes: usize,
    /// Maximum admitted history header and node-vector capacity in bytes,
    /// including conservative old/new capacity overlap during growth.
    /// Actual capacity is checked after reservation as well as before it.
    /// A refused insertion may retain enlarged capacity but publishes no key.
    /// Allocator metadata, stack locals and the rest of SAT storage are excluded;
    /// this is neither process RSS nor an operating-system allocation quota.
    pub max_bytes: usize,
}

// Preserve the historic scale of twelve million exclusion bits plus a root,
// independently of any caller-selected CNF admission limits.
const DEFAULT_MAX_NODES: usize = 12_582_912 + 1;

impl ProjectionLimits {
    /// Default finite history population, independent of CNF admission limits.
    pub const DEFAULT: Self = Self {
        max_entries: 1_000_000,
        max_nodes: DEFAULT_MAX_NODES,
        max_bytes: 128 * 1024 * 1024,
    };
}

impl Default for ProjectionLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Independently bounded projection-history resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionResource {
    /// Distinct complete semantic keys.
    Entries,
    /// Logical allocated trie elements, not vector capacity.
    Nodes,
    /// Named header and actual node-vector capacity, including growth overlap.
    Bytes,
}

/// Cumulative receipt from the single exclusion-history owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectionStatistics {
    /// Successfully published distinct keys, including the zero-width key.
    pub entries: u64,
    /// Currently retained logical trie elements. Failed suffixes are removed.
    pub nodes: usize,
    /// Current history header plus actual node-vector capacity in bytes.
    pub retained_bytes: u128,
    /// Greatest conservative header plus old/new vector-capacity overlap.
    /// Repeated failed attempts take a maximum, not a sum. This is not RSS.
    pub peak_bytes: u128,
    /// Successfully charged trie insertion and lookup steps, including failed
    /// attempts. This is a subset of `Statistics::search.work`, not additional
    /// work or a second allowance. Outer cursor validation is excluded.
    pub work: u64,
}
