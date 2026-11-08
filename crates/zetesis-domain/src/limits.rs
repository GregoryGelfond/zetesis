//! Logical ceilings and accounting; these are not allocator or RSS measurements.

/// Limits on retained abstract data and inspected source structure.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Lowering, key inspection, transfer, certificates, finite comparison and copy steps.
    pub max_work: u64,
    /// Distinct signed predicate signatures, including zero-arity predicates.
    pub max_predicates: usize,
    /// Distinct predicate argument positions.
    pub max_positions: usize,
    /// Plan records, slots, bindings, guards, heads, provenance, keys, thresholds,
    /// integer-expression requests and their retained result cells.
    pub max_links: usize,
    /// Retained values in one argument; overflow widens only that argument.
    pub max_values_per_argument: usize,
    /// Live finite references across seeds, arguments and temporary transfer sets.
    pub max_value_entries: usize,
    /// Started synchronous image passes, including unchanged ascent and narrowing.
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
    /// Retained plan, key, threshold, provenance and certificate-result records.
    Links,
    /// Retained finite value references.
    ValueEntries,
    /// Started synchronous image passes.
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
/// Work is cumulative; finite value entries are live reservations, not a peak.
/// Failed temporary sets release their entries before a stopped-result receipt.
/// Plan/link reservations precede allocation and can remain in a failure receipt.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Statistics {
    /// Source/transfer/intersection/copy/value steps.
    pub work: u64,
    /// Registered signed predicate signatures.
    pub predicates: usize,
    /// Registered argument positions.
    pub positions: usize,
    /// Submitted plan, key, threshold, provenance and certificate-result records.
    pub links: usize,
    /// Live finite references at the receipt boundary; not peak storage or total allocations.
    pub value_entries: usize,
    /// Started synchronous image passes, including narrowing.
    pub rounds: u64,
    /// Inspected name/string payload.
    pub inspected_bytes: u64,
    /// Arguments widened to Unknown before a possible global fallback.
    pub widened: usize,
}

pub(crate) fn check(resource: Resource, observed: u128, limit: u128) -> Result<(), Stop> {
    zetesis_algebra::Meter::check_wide_count(observed, 0, limit)
        .map(|_| ())
        .map_err(|_| Stop {
            resource,
            limit,
            observed,
        })
}

/// Admit domain work without adding control polling or another stored counter.
pub(crate) fn charge_work(accepted: &mut u64, amount: u64, limit: u64) -> Result<(), Stop> {
    zetesis_algebra::Meter::charge_receipt(accepted, u128::from(amount), limit).map_err(|error| {
        match error {
            zetesis_algebra::MeterWorkError::Limit { observed, limit } => Stop {
                resource: Resource::Work,
                limit,
                observed,
            },
            // The receipt and request are both u64, so their sum fits u128.
            zetesis_algebra::MeterWorkError::Overflow => {
                unreachable!("two u64 work values fit u128")
            }
        }
    })
}
