//! Captured prepared CPU ownership facts, independent of answer membership.

use std::sync::Arc;
use zetesis_cpu::{BatchError, QueryStatistics};

/// Latest ordinary independent CPU query-cache observation.
///
/// The CPU producer remains the single counter authority. This snapshot copies
/// its typed receipt after each submitted batch; it does not sum counters again.
/// Unsupported executors expose no observation. A read failure retains the last
/// successful receipt and its original typed fault, without discarding checked
/// models or asserting candidate coverage.
#[derive(Clone, Debug, Default)]
pub struct QueryExecutionObservation {
    /// Most recent successful snapshot; absent before a batch was attempted or
    /// when the first snapshot failed. Activity counts describe the latest
    /// independent attempt that acquired the CPU admission owner; preparation
    /// builds are cumulative. These are owner counts, not completed checks.
    pub statistics: Option<QueryStatistics>,
    /// Original failure of the latest snapshot operation, if any. A prior
    /// successful receipt is not relabeled as current after this fault.
    pub fault: Option<Arc<BatchError>>,
}

impl QueryExecutionObservation {
    pub(crate) fn capture(&mut self, result: Result<QueryStatistics, BatchError>) {
        match result {
            Ok(statistics) => { self.statistics = Some(statistics); self.fault = None; }
            Err(error) => { self.fault = Some(Arc::new(error)); }
        }
    }
}
