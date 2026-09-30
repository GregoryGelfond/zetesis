//! Data several crates' tests read, embedded.

/// The phase timings section of zetesis's statistics, for one fixed run.
pub const PHASE_STATISTICS: &str = include_str!("../fixtures/phase_statistics.txt");

/// The stage timings section of zetesis's statistics, for the same run as
/// [`PHASE_STATISTICS`].
pub const STAGE_STATISTICS: &str = include_str!("../fixtures/stage_statistics.txt");

/// A program one of whose rule instances divides by zero, which admission
/// reports as a zero-divisor warning.
pub const ZERO_DIVISOR: &str = "d(0..2).\np(X) :- d(X), 1/X=1.\n";

/// Comparison-generator programs, one JSON case per line, each with its
/// expected admission and its complete answer sets.
pub const COMPARISON_GENERATORS: &str = include_str!("../fixtures/comparison-generators.jsonl");

/// Programs whose formula heads hold negative occurrences, as a JSON array of
/// cases, each with its roots, its support and its complete answer sets.
pub const NEGATIVE_HEADS: &str = include_str!("../fixtures/negative-heads.json");

/// Programs whose heads are negative singletons, as a JSON array of cases, each
/// with its roots, its support and its complete answer sets.
pub const SINGLETON_HEADS: &str = include_str!("../fixtures/singleton-heads.json");
