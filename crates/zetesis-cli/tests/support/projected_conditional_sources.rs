//! Original sources distinguishing projection from signed witness alternatives.

/// The CPU contract supplies independent full answers; physical checks reuse
/// these exact sources to exercise ordinary formula admission and execution.
pub const SOURCES: [&str; 4] = [
    include_str!("../fixtures/projected-conditionals/absent.lp"),
    include_str!("../fixtures/projected-conditionals/present.lp"),
    include_str!("../fixtures/projected-conditionals/negative-cycle.lp"),
    include_str!("../fixtures/projected-conditionals/double-negative-cycle.lp"),
];
