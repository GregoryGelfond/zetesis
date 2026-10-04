//! Helpers the evaluation's unit tests share across its modules.

use zetesis_cpu::Cancellation;

use super::{ConstructionLimits, Limits, Statistics, Work};

/// The work of an evaluation under the default limits, stopped by
/// `cancellation`.
pub(super) fn work(cancellation: &Cancellation) -> Work<'_> {
    Work {
        limits: Limits::default(),
        construction: ConstructionLimits::default(),
        cancellation,
        statistics: Statistics::default(),
        site: crate::ProgramSite::program(),
        local_bytes: 0,
    }
}
