//! The fixed cell set on which a sequence of solver changes is measured.
//!
//! Each cell is one sealed workload: a generated family program at a size, a
//! constant-amended queens board, or an unchanged corpus entry. The sizes
//! were chosen, by measuring each cell's complete native JSON output when it
//! joined, to stay under [`CAPTURE_BYTES`], since the instrumented matrix
//! retains and checks every reported model, and so that the whole set runs in
//! minutes; the library does not check that property. Cells that a
//! change is meant to move (a refusal, a timeout) are kept as evidence rather
//! than dropped: their typed decisions are the observation.

mod view;

pub use view::{
    Breakdown, Cell, Comparison, Labelled, Native, Passed, PhaseTiming, ProfileRow, Provenance,
    Reference, Scoreboard, Timing, Verdict, ViewError, compare,
};

use super::families::Family;
use super::matrix::{ConstantAmendment, Workload, WorkloadLimits};
use super::{Case, Error, Limits};
use crate::answers::native_json;
use crate::examples;

/// Number of cells in the series: the generated programs, the amended
/// boards and the unchanged entries.
pub const CELLS: usize = GENERATED.len() + QUEENS.len() + ORIGINAL.len();

/// Per-invocation capture ceiling the cells were sized against, in bytes.
pub const CAPTURE_BYTES: usize = 16 * 1024 * 1024;

const GENERATED: [(Family, u32); 17] = [
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
    (Family::LatinSquare, 5),
    (Family::Planning, 14),
];

// Queens boards derived from the curated encodings' `#const n = 8`.
const QUEENS: [(Case, i32); 3] = [
    (Case::Queens01, 10),
    (Case::Queens01, 11),
    (Case::Queens04, 11),
];

const ORIGINAL: [Case; 2] = [Case::Send, Case::TaskAllocation];

/// The corpus entries the series takes, amended or unchanged: what the
/// matrix's series suite admits.
pub const CORPUS_CASES: [Case; 4] = [
    Case::Queens01,
    Case::Queens04,
    Case::Send,
    Case::TaskAllocation,
];

/// Campaign ceilings raised to what the cells need: one complete native
/// record per invocation up to [`CAPTURE_BYTES`], and cumulative capture and
/// report ceilings for two hundred such records. Larger ceilings in `base`
/// are kept; timeouts and deadlines are not touched.
#[must_use]
pub fn limits(mut base: Limits) -> Limits {
    base.process.max_output_bytes = base.process.max_output_bytes.max(CAPTURE_BYTES);
    base.max_total_capture_bytes = base.max_total_capture_bytes.max(200 * CAPTURE_BYTES);
    base.max_report_bytes = base.max_report_bytes.max(base.max_total_capture_bytes);
    base
}

/// Native record decoding ceilings raised to [`CAPTURE_BYTES`] per record.
#[must_use]
pub fn native_answers(mut base: native_json::Limits) -> native_json::Limits {
    base.report.max_input_bytes = base.report.max_input_bytes.max(CAPTURE_BYTES);
    base
}

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
            entry.path(),
            &[ConstantAmendment {
                source_path: entry.path(),
                name: "n",
                expected: 8,
                replacement: size,
            }],
            limits,
        )?);
    }
    for entry in ORIGINAL {
        cells.push(Workload::original(corpus, entry.path(), limits)?);
    }
    Ok(cells)
}
