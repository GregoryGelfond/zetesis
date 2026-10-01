//! Execution choices independent of source meaning and finite resource limits.

/// Strategy for positive relational joins during eager formula grounding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JoinStrategy {
    /// Probe the shortest available equality posting and check complete rows.
    /// A completed flat constraint with a finite arithmetic-totality certificate
    /// may compose a computed equality domain with the shared table selector.
    /// This special query keeps the original rows and residual checks; other
    /// probes remain indexed even after that table has been prepared.
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

/// Logical bounds on an optional conservative domain-analysis attempt.
///
/// These are distinct from the named support/query byte ceiling. Analysis uses
/// bounded standard collections and has no allocation-failure or caller-control
/// API; it is an uninterruptible operation inside existing eager materialization.
pub use zetesis_domain::{Limits as DomainLimits, Stop as DomainStop};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Execution {
    pub joins: JoinStrategy,
    pub domains: Option<DomainLimits>,
}
