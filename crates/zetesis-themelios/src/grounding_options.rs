//! Execution choices independent of source meaning and finite resource limits.

/// Strategy for positive relational joins during eager formula grounding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JoinStrategy {
    /// Probe the shortest available equality posting and check complete rows.
    #[default]
    Indexed,
    /// Reuse finite-table indices for flat positive patterns over completed
    /// possible support. Structural patterns and support-growth rounds retain
    /// indexed matching. An exhausted table operation is an error, not fallback.
    ///
    /// This explicit strategy has no automatic size or speed threshold. Index
    /// construction may cost more work or storage than the rows it avoids.
    Table,
}

/// Execution policy for materializing a prepared formula source.
///
/// These choices change neither source meaning nor the retained admission
/// ceilings. They introduce no cancellation or timing contract. Grounding work
/// observation reports actual operations, including inapplicable table probes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroundingOptions {
    /// Positive-join execution strategy; independent of formula/reduct solving.
    pub joins: JoinStrategy,
}
