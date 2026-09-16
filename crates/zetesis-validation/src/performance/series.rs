//! The fixed cell set on which a sequence of solver changes is measured.
//!
//! Each cell is one sealed workload: a generated family program at a size, a
//! constant-amended queens board, or an unchanged corpus entry. The sizes are
//! chosen so that every cell's complete native JSON output stays under
//! [`CAPTURE_BYTES`], since the instrumented matrix retains and checks every
//! reported model, and so that the whole set runs in minutes. Cells that a
//! change is meant to move (a refusal, a timeout) are kept as evidence rather
//! than dropped: their typed decisions are the observation.

use super::Error;
use super::families::Family;
use super::matrix::{ConstantAmendment, Workload, WorkloadLimits};
use crate::examples;

/// Number of cells in the series.
pub const CELLS: usize = 20;

/// Per-invocation capture ceiling the cells were sized against, in bytes.
pub const CAPTURE_BYTES: usize = 16 * 1024 * 1024;

const GENERATED: [(Family, u32); 15] = [
    (Family::IndependentChoice, 12),
    (Family::IndependentChoice, 16),
    (Family::IndependentNegation, 8),
    (Family::IndependentNegation, 10),
    (Family::IndependentNegationAggregate, 16),
    (Family::Disjunction, 12),
    (Family::Ties, 50),
    (Family::TransitivePath, 100),
    (Family::TransitivePath, 200),
    (Family::TransitiveDense, 40),
    (Family::Chain, 1000),
    (Family::Chain, 2000),
    (Family::ChainArithmetic, 1000),
    (Family::Stratified, 16),
    (Family::ProducerChain, 700),
];

// Queens boards derived from the curated encodings' `#const n = 8`.
const QUEENS: [(&str, i32); 3] = [
    ("standalone/n-queens/variant-01.lp", 10),
    ("standalone/n-queens/variant-01.lp", 11),
    ("standalone/n-queens/variant-04.lp", 11),
];

const ORIGINAL: [&str; 2] = [
    "standalone/send-money/send-money.lp",
    "scenarios/task-allocation/variant-04/05-larger-mix.lp",
];

/// The cells, in schedule order.
///
/// # Errors
/// Refuses when a corpus entry is missing or a workload exceeds `limits`.
pub fn workloads(
    corpus: &examples::Corpus,
    limits: WorkloadLimits,
) -> Result<Vec<Workload>, Error> {
    let mut cells = Vec::with_capacity(CELLS);
    for (family, size) in GENERATED {
        cells.push(Workload::generated(family, size, limits)?);
    }
    for (entry, size) in QUEENS {
        cells.push(Workload::amended(
            corpus,
            entry,
            &[ConstantAmendment {
                source_path: entry,
                name: "n",
                expected: 8,
                replacement: size,
            }],
            limits,
        )?);
    }
    for entry in ORIGINAL {
        cells.push(Workload::original(corpus, entry, limits)?);
    }
    Ok(cells)
}
