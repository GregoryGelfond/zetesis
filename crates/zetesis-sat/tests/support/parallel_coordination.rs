//! Work-stealing coordination and termination for the region scheduler.

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::*;

const WAIT: Duration = Duration::from_secs(2);

/// A two-branch theory: `Or(a0, not a0)` is a tautology, so its root region
/// decides nothing and splits on atom 0 into a cut and a held child.
fn search(workers: usize) -> ParallelRegions {
    let theory = Theory::new(
        1,
        vec![
            zetesis_ferraris::Node::False,
            zetesis_ferraris::Node::Atom(0),
            zetesis_ferraris::Node::Implies(1, 0),
            zetesis_ferraris::Node::Or(1, 2),
        ],
        vec![3],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::with_deadline(Instant::now() + 3 * WAIT).unwrap();
    let limits = Limits::default();
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    ParallelRegions::new(
        &theory,
        NonZeroUsize::new(workers).unwrap(),
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap()
}

fn budget(shared: &Shared) -> Budget<'_, WorkLease<'_>> {
    Budget {
        quota: shared.budget.lease(&shared.cancellation),
        limits: shared.limits.search,
        cancellation: &shared.cancellation,
        statistics: SearchStatistics::default(),
    }
}

fn report() -> WorkerReport {
    WorkerReport {
        regions: RegionCounts::default(),
        statistics: Statistics::default(),
    }
}

#[test]
fn a_split_pushes_both_children_onto_the_deque_and_counts_the_net_gain() {
    let mut search = search(2);
    let root = search.root.take().unwrap();
    let deque: Worker<Entry> = Worker::new_lifo();
    let mut membership =
        crate::prepared_reduct::State::with_index(Arc::clone(&search.shared.index));
    let mut report = report();
    let mut budget = budget(&search.shared);
    // The root is the one outstanding region before it splits.
    assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 1);
    let stepped = step(
        &search.shared,
        root,
        &deque,
        &mut budget,
        &mut membership,
        &mut report,
        &mut None,
    )
    .unwrap();
    assert!(matches!(stepped, Stepped::Split));
    // Two children replace the one parent: outstanding 1 -> 2, counted before
    // the pushes so no peer can observe a transient zero.
    assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 2);
    // Cut is pushed last (on top of the LIFO deque), held beneath it.
    let (cut, cut_knowledge) = deque.pop().unwrap();
    let (held, held_knowledge) = deque.pop().unwrap();
    assert!(cut.is_cut(0) && held.is_held(0));
    assert_eq!(cut_knowledge.len(), 1);
    assert_eq!(held_knowledge.len(), 1);
    assert!(deque.pop().is_none());
}

#[test]
fn find_work_steals_a_peer_region_then_reports_done_at_quiescence() {
    let search = search(2);
    let mine: Worker<Entry> = Worker::new_lifo();
    let peer: Worker<Entry> = Worker::new_lifo();
    let stealers = [mine.stealer(), peer.stealer()];
    // A region on the peer's deque; worker 0 steals from worker 1.
    peer.push((
        Region::all_open(1),
        vec![search.shared.index.narrower().knowledge()],
    ));
    let stolen = find_work(&search.shared, &stealers, 0);
    assert!(stolen.is_some(), "an idle worker steals a peer's region");
    assert!(
        peer.pop().is_none(),
        "the stolen region left the peer's deque"
    );
    // Every deque empty and nothing outstanding: the frontier is resolved.
    search.shared.outstanding.store(0, Ordering::Release);
    assert!(find_work(&search.shared, &stealers, 0).is_none());
}

#[test]
fn find_work_returns_none_once_closed() {
    let search = search(2);
    let mine: Worker<Entry> = Worker::new_lifo();
    let peer: Worker<Entry> = Worker::new_lifo();
    let stealers = [mine.stealer(), peer.stealer()];
    search.shared.close();
    assert!(find_work(&search.shared, &stealers, 0).is_none());
}

#[test]
fn the_first_stop_is_the_one_reported() {
    let search = search(2);
    search.shared.stop(Incomplete::WorkLimit);
    search.shared.stop(Incomplete::DecisionLimit);
    assert_eq!(search.shared.stopped(), Some(Incomplete::WorkLimit));
    assert!(search.shared.closed.load(Ordering::Acquire));
}

#[test]
fn an_idle_worker_stops_on_cancellation_and_records_it() {
    let search = search(2);
    let mine: Worker<Entry> = Worker::new_lifo();
    let peer: Worker<Entry> = Worker::new_lifo();
    let stealers = [mine.stealer(), peer.stealer()];
    // Work is outstanding (the root, never seeded here) but no deque holds it,
    // so the idle worker waits — and observes the cancellation.
    assert!(search.shared.outstanding.load(Ordering::Acquire) > 0);
    search.shared.cancellation.cancel();
    let result = find_work(&search.shared, &stealers, 0);
    assert!(result.is_none());
    assert!(search.shared.closed.load(Ordering::Acquire));
    assert_eq!(search.shared.stopped(), Some(Incomplete::Cancelled));
}
