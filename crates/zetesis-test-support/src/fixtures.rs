//! Data several crates' tests read, embedded.

/// A retained schema-1 phase section, for compatibility checks.
pub const PHASE_STATISTICS: &str = include_str!("../fixtures/phase_statistics.txt");

/// The current phase protocol, with one measured admission attempt.
/// Reader tests use its fixed elapsed values; the CLI guard substitutes only
/// those values from a real recorder snapshot before comparing the full text.
pub const CURRENT_PHASE_STATISTICS: &str = include_str!("../fixtures/current_phase_statistics.txt");

/// The stage timings section of zetesis's statistics, for the same run as
/// [`PHASE_STATISTICS`].
pub const STAGE_STATISTICS: &str = include_str!("../fixtures/stage_statistics.txt");

/// The statistics validation qualifies from a GPU formula run, as the command
/// line renders them: the exhaustion, route, completion, batching, device and
/// countermodel lines. The search counters come from a real solve of `a | b.`;
/// the device counters are authored to agree with them under a format-fixture
/// adapter, and claim no physical execution. The command line's tests fail
/// when its rendering leaves this text, and validation's tests fail when it
/// cannot qualify it, so neither side can change alone.
pub const FORMULA_GPU_STATISTICS: &str = include_str!("../fixtures/formula_gpu_statistics.txt");

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
