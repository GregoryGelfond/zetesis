//! Logical-order and empty-extremum sources shared by solver qualification.

/// These are original aggregate heads, preserving their independent atom
/// permissions. CPU tests specify full answers; physical tests reuse the inputs.
pub const SOURCES: [&str; 6] = [
    include_str!("../fixtures/logical-extrema/min-symbols.lp"),
    include_str!("../fixtures/logical-extrema/max-symbols.lp"),
    include_str!("../fixtures/logical-extrema/value-classes.lp"),
    include_str!("../fixtures/logical-extrema/constructors.lp"),
    include_str!("../fixtures/logical-extrema/infimum.lp"),
    include_str!("../fixtures/logical-extrema/empty-supremum.lp"),
];
