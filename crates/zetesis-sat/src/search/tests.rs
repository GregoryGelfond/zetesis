use super::*;

/// A search budget under the default limits, drawing on a local quota.
fn budget(cancellation: &Cancellation) -> Budget<'_> {
    Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation,
        statistics: SearchStatistics::default(),
    }
}

mod workspace_tests;
mod finish_tests;
mod watch_tests;
mod watch_traces;
mod binary_watch_tests;
mod ternary_watch_tests;
