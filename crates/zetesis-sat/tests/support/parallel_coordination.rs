//! Ownership transfers and termination at the native region pool boundary.

use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use super::*;

const WAIT: Duration = Duration::from_secs(2);

fn search(workers: usize) -> ParallelRegions {
    // One independent choice requires a split and admits both children.
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

/// Observe the actual registration, not an elapsed interval that might
/// merely mean the worker has not run. Callers close and join on failure.
fn observe_idle(shared: &Shared) -> bool {
    let deadline = Instant::now() + WAIT;
    loop {
        if shared.lock().idle == 1 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::yield_now();
    }
}

#[test]
fn local_take_avoids_the_pool_lock() {
    let search = search(2);
    let mut local = vec![search.shared.lock().pending.pop().unwrap()];
    let shared = Arc::clone(&search.shared);
    let pool = search.shared.lock();
    let (sent, received) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        sent.send((take(&shared, &mut local), local)).unwrap();
    });
    // Success must be received while the guard is still held. A timeout
    // fails only after releasing the lock and joining, so the old locking
    // implementation fails this assertion without stranding a worker.
    let result = received.recv_timeout(WAIT);
    drop(pool);
    worker.join().unwrap();
    let (taken, remaining) = result.expect("local take waited for the pool lock");
    assert!(taken.unwrap().0.is_open(0));
    assert!(remaining.is_empty());
}

#[test]
fn closed_take_preserves_unvisited_local_work() {
    let search = search(2);
    let mut local = vec![search.shared.lock().pending.pop().unwrap()];
    search.shared.close();
    assert!(take(&search.shared, &mut local).is_none());
    assert_eq!(local.len(), 1);
    assert!(local[0].0.is_open(0));
}

#[test]
fn local_take_precedes_pending_work() {
    let search = search(1);
    let (root, knowledge) = search.shared.lock().pending.pop().unwrap();
    let (cut, held) = root.split(0);
    search.shared.lock().pending.push((held, knowledge.clone()));
    let mut local = vec![(cut, knowledge)];
    assert!(take(&search.shared, &mut local).unwrap().0.is_cut(0));
    assert_eq!(search.shared.lock().pending.len(), 1);
    assert!(take(&search.shared, &mut local).unwrap().0.is_held(0));
    assert!(!search.shared.closed.load(Ordering::Acquire));
    assert!(take(&search.shared, &mut local).is_none());
    assert!(search.shared.closed.load(Ordering::Acquire));
    assert_eq!(search.shared.lock().idle, 0);
}

#[test]
fn an_idle_peer_cannot_exhaust_owned_local_work() {
    let search = search(2);
    let mut local = vec![search.shared.lock().pending.pop().unwrap()];
    let waiting = Arc::clone(&search.shared);
    let waiter = thread::spawn(move || take(&waiting, &mut Vec::new()));
    let idle = observe_idle(&search.shared);
    let mut took_local = false;
    let mut closed_while_owned = true;
    let mut final_take_empty = false;
    if idle {
        let taken = take(&search.shared, &mut local);
        // The taken region remains owned by this active worker. The other
        // worker's empty stack cannot establish coverage on its behalf.
        closed_while_owned = search.shared.closed.load(Ordering::Acquire);
        took_local = taken.is_some();
        // Retire that work before asking for another region, as the worker
        // loop does after processing it. Both workers can now become idle.
        drop(taken);
        final_take_empty = take(&search.shared, &mut local).is_none();
    } else {
        search.shared.close();
    }
    let result = waiter.join().unwrap();
    assert!(idle, "the peer never registered its wait");
    assert!(took_local);
    assert!(!closed_while_owned);
    assert!(final_take_empty);
    assert!(result.is_none());
    assert!(search.shared.closed.load(Ordering::Acquire));
    let pool = search.shared.lock();
    assert_eq!(pool.idle, 0);
    assert_eq!(pool.stopped, None);
}

#[test]
fn a_split_donates_its_held_child_to_an_idle_worker() {
    let search = search(2);
    let (region, knowledge) = search.shared.lock().pending.pop().unwrap();
    let waiting = Arc::clone(&search.shared);
    let waiter = thread::spawn(move || take(&waiting, &mut Vec::new()));
    let idle = observe_idle(&search.shared);
    let mut local = Vec::new();
    let mut membership =
        crate::prepared_reduct::State::with_index(Arc::clone(&search.shared.index));
    let mut report = WorkerReport {
        regions: RegionCounts::default(),
        statistics: Statistics::default(),
    };
    let mut budget = Budget {
        quota: search.shared.budget.lease(&search.shared.cancellation),
        limits: search.shared.limits.search,
        cancellation: &search.shared.cancellation,
        statistics: SearchStatistics::default(),
    };
    let result = if idle {
        step(
            &search.shared,
            (region, knowledge),
            &mut local,
            &mut budget,
            &mut membership,
            &mut report,
            &mut None,
        )
    } else {
        Err(Incomplete::Deadline)
    };
    drop(budget);
    if result.is_err() {
        search.shared.close();
    }
    let donated = waiter.join().unwrap();
    assert!(idle, "the peer never registered its wait");
    assert!(result.unwrap().is_none(), "the root must split");
    let (held, held_knowledge) = donated.expect("the donated child was lost");
    let (cut, cut_knowledge) = local.pop().expect("the local child was lost");
    assert!(held.is_held(0));
    assert!(cut.is_cut(0));
    assert_eq!(held_knowledge.len(), 1);
    assert_eq!(cut_knowledge.len(), 1);
    assert!(local.is_empty());
    assert!(!search.shared.closed.load(Ordering::Acquire));
    let pool = search.shared.lock();
    assert!(pool.pending.is_empty());
    assert_eq!(pool.idle, 0);
}

#[test]
fn closure_before_worker_errors_keeps_the_first_error() {
    let search = search(2);
    search.shared.lock().pending.clear();
    let waiting = Arc::clone(&search.shared);
    let waiter = thread::spawn(move || take(&waiting, &mut Vec::new()));
    let idle = observe_idle(&search.shared);
    search.shared.close();
    search.shared.stop(Incomplete::WorkLimit);
    search.shared.stop(Incomplete::DecisionLimit);
    let result = waiter.join().unwrap();
    assert!(idle, "the peer never registered its wait");
    assert!(result.is_none());
    let pool = search.shared.lock();
    assert_eq!(pool.stopped, Some(Incomplete::WorkLimit));
    assert_eq!(pool.idle, 0);
}

#[test]
fn cancellation_releases_idle_registration() {
    let search = search(2);
    search.shared.lock().pending.clear();
    let waiting = Arc::clone(&search.shared);
    let waiter = thread::spawn(move || take(&waiting, &mut Vec::new()));
    let idle = observe_idle(&search.shared);
    search.shared.cancellation.cancel();
    let result = waiter.join().unwrap();
    assert!(idle, "the peer never registered its wait");
    assert!(result.is_none());
    assert!(search.shared.closed.load(Ordering::Acquire));
    assert_eq!(search.shared.lock().idle, 0);
    // Cancellation remains an incomplete outcome at the consumer boundary.
    assert_eq!(
        search.shared.cancellation.poll().map_err(Incomplete::from),
        Err(Incomplete::Cancelled)
    );
}

#[test]
fn idle_cancellation_preserves_unfinished_coverage() {
    let search = search(2);
    let mut local = vec![search.shared.lock().pending.pop().unwrap()];
    let waiting = Arc::clone(&search.shared);
    let waiter = thread::spawn(move || take(&waiting, &mut Vec::new()));
    let idle = observe_idle(&search.shared);
    search.shared.cancellation.cancel();
    let result = waiter.join().unwrap();
    assert!(idle, "the peer never registered its wait");
    assert!(result.is_none());
    assert!(take(&search.shared, &mut local).is_none());
    assert_eq!(local.len(), 1, "the unvisited region remains unfinished");
    assert!(local[0].0.is_open(0));
    // A coordinator whose earlier poll succeeded relies on this receipt
    // when the last worker's sender disconnects, not on another control poll.
    assert_eq!(search.shared.lock().stopped, Some(Incomplete::Cancelled));
}

#[test]
fn idle_deadline_preserves_unfinished_coverage() {
    let mut search = search(2);
    let mut local = vec![search.shared.lock().pending.pop().unwrap()];
    // An already expired deadline has no timer-thread or scheduling race.
    // The two-worker pool still requires this empty worker to enter its
    // normal timed idle wait before it observes that deadline.
    Arc::get_mut(&mut search.shared).unwrap().cancellation =
        Cancellation::with_deadline(Instant::now()).unwrap();
    let waiting = Arc::clone(&search.shared);
    let waiter = thread::spawn(move || take(&waiting, &mut Vec::new()));
    let result = waiter.join().unwrap();
    assert!(result.is_none());
    assert!(take(&search.shared, &mut local).is_none());
    assert_eq!(local.len(), 1, "the unvisited region remains unfinished");
    assert!(local[0].0.is_open(0));
    assert_eq!(search.shared.lock().stopped, Some(Incomplete::Deadline));
}
