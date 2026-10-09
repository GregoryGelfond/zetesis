//! Cancellation settles leases before the native iterator exposes its final receipt.

use super::*;
use crate::ferraris::{Proposer, StableModels};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

#[test]
fn cancellation_preserves_settled_interruption() {
    let theory = Theory::new(
        0,
        zetesis_ferraris::FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let mut models = StableModels::with_region_workers(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        Limits::default(),
        cancellation.clone(),
    )
    .unwrap();
    // Prepare the empty index before cancellation so the next pull reaches
    // the coordinator's cancellation check, not index construction.
    models.index.ensure(|_| Ok(())).unwrap();
    let before = models.statistics.search;
    let Proposer::Parallel(search) = &mut models.proposer else {
        panic!("parallel region workers were not selected")
    };
    search.synchronize_budget(before).unwrap();
    search.started = true;
    let shared = Arc::clone(&search.shared);
    let settled = Arc::new(AtomicBool::new(false));
    let worker_settled = Arc::clone(&settled);
    let (ready, entered) = mpsc::sync_channel(1);
    search.handles.push(std::thread::spawn(move || {
        contain_worker(&shared, || {
            let lease = shared.budget.lease(&shared.cancellation);
            lease.take(37).unwrap();
            lease.decide().unwrap();
            Live::add(&shared.live.work, 37);
            ready.send(()).unwrap();
            // No model is delivered and the sender remains owned by the
            // coordinator. Only terminal cleanup can settle this prefix.
            while !shared.termination.is_closed() {
                std::thread::yield_now();
            }
            drop(lease);
            // This later cleanup failure must not replace cancellation.
            shared.stop(Incomplete::WorkLimit);
            worker_settled.store(true, Ordering::Release);
            WorkerReport {
                regions: RegionCounts::default(),
                statistics: Statistics::default(),
            }
        })
    }));
    let entered = entered.recv_timeout(Duration::from_secs(2));
    cancellation.cancel();
    let result = models.next();
    // Unblock even a regression before asserting, so a failed test cannot
    // leave its injected worker spinning or hang while dropping the iterator.
    let joined_before_return = settled.load(Ordering::Acquire);
    let observed = models.statistics();
    let _ = models.stop();
    entered.unwrap();
    assert!(matches!(result, Some(Err(Incomplete::Cancelled))));
    assert!(joined_before_return);
    assert_eq!(observed.search.work, before.work + 37);
    assert_eq!(observed.search.decisions, before.decisions + 1);
    assert!(observed.regions.unwrap().counts.work <= observed.search.work);
    assert_eq!(observed.stable_models, 0);
    assert!(!models.exhausted());
    assert!(models.next().is_none());
}
