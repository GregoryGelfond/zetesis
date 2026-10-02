//! Logical ceilings and accounting; these are not allocator or RSS measurements.

/// Limits on retained abstract data and inspected source structure.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Source, transfer-input, intersection-probe, snapshot-copy and value-merge steps.
    pub max_work: u64,
    /// Distinct signed predicate signatures, including zero-arity predicates.
    pub max_predicates: usize,
    /// Distinct predicate argument positions.
    pub max_positions: usize,
    /// Transfer records, producer references, and dependency-column references.
    pub max_links: usize,
    /// Retained values in one argument; overflow widens only that argument.
    pub max_values_per_argument: usize,
    /// Retained finite value references across all arguments.
    pub max_value_entries: usize,
    /// Complete passes, including the final unchanged pass.
    pub max_rounds: u64,
    /// Cumulative name/string bytes inspected before comparison or copying.
    pub max_inspected_bytes: u64,
    /// Nodes in one finite symbol; larger values widen their output argument.
    pub max_symbol_nodes: usize,
    /// Nested symbol depth, with an atomic symbol at depth one.
    pub max_symbol_depth: usize,
    /// Name/string payload in one finite symbol.
    pub max_symbol_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 1_000_000,
            max_predicates: 4_096,
            max_positions: 16_384,
            max_links: 131_072,
            max_values_per_argument: 256,
            max_value_entries: 1_000_000,
            max_rounds: 1_024,
            max_inspected_bytes: 8_388_608,
            max_symbol_nodes: 256,
            max_symbol_depth: 64,
            max_symbol_bytes: 16_384,
        }
    }
}

/// A global ceiling whose exhaustion prevents publishing any finite domains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// Source/transfer/intersection/copy/merge steps.
    Work,
    /// Signed predicate signatures.
    Predicates,
    /// Argument positions.
    Positions,
    /// Retained transfer and provenance/dependency references.
    Links,
    /// Retained finite value references.
    ValueEntries,
    /// Complete fixed-point passes.
    Rounds,
    /// Inspected name/string bytes.
    InspectedBytes,
}

/// The exact logical ceiling that stopped analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stop {
    /// Exhausted resource.
    pub resource: Resource,
    /// Configured inclusive maximum.
    pub limit: u128,
    /// Attempted count; the failed operation is not committed.
    pub observed: u128,
}
impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} ceiling {} exceeded by {}",
            self.resource, self.limit, self.observed
        )
    }
}
impl std::error::Error for Stop {}

/// Charged logical counts before completion or an explicit conservative stop.
/// Reservations precede allocation; a later failure can leave a reservation unused.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Statistics {
    /// Source/transfer/intersection/copy/value steps.
    pub work: u64,
    /// Registered signed predicate signatures.
    pub predicates: usize,
    /// Registered argument positions.
    pub positions: usize,
    /// Submitted transfer and producer/dependency references.
    pub links: usize,
    /// Finite value references before any global fallback clears results.
    pub value_entries: usize,
    /// Started fixed-point passes.
    pub rounds: u64,
    /// Inspected name/string payload.
    pub inspected_bytes: u64,
    /// Arguments widened to Unknown before a possible global fallback.
    pub widened: usize,
}

pub(crate) fn check(resource: Resource, observed: u128, limit: u128) -> Result<(), Stop> {
    if observed > limit {
        Err(Stop {
            resource,
            limit,
            observed,
        })
    } else {
        Ok(())
    }
}
