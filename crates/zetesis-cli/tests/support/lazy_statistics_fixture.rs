//! Synthetic public statistics for CLI rendering; no physical execution is claimed.

use crate::{Backend, LazyExecutionStatistics};

pub(crate) fn lazy_statistics() -> LazyExecutionStatistics {
    LazyExecutionStatistics {
        requested_backend: Backend::Metal,
        adapter: "FORMAT FIXTURE: no physical execution".into(),
        backend: "Metal".into(),
        batches: 2,
        submitted_candidates: 7,
        completed_candidates: 4,
        stopped_candidates: 3,
        queued_results: 2,
        source_rounds: 3,
        source_work: 41,
        source_instances: 13,
        peak_catalog_atoms: 65,
        dispatches: 5,
        world_instances: 19,
        uploaded_bytes: 67,
        downloaded_bytes: 23,
        transport_allocations: 2,
        transport_reuses: 3,
        peak_transport_bytes: 1024,
        transport_replacements: crate::LazyTransportReplacements {
            initial: 1,
            records_growth: 1,
            result_shape: 1,
            ..Default::default()
        },
        transport_usage: crate::LazyTransportUsage {
            uniform: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            offsets: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            records: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 3,
            },
            snapshots: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            seeds: crate::LazyBufferUsage {
                allocations: 1,
                reuses: 4,
            },
            output: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 3,
            },
            readback: crate::LazyBufferUsage {
                allocations: 2,
                reuses: 3,
            },
            budget_releases: 0,
            accounting_overflow_releases: 0,
        },
        host_wait: std::time::Duration::from_nanos(123),
    }
}
