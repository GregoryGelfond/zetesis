//! Work-stealing coordination and termination for the region scheduler.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Instant;

use super::*;

const WAIT: Duration = Duration::from_secs(2);

/// A two-branch theory: `Or(a0, not a0)` is a tautology, so its root region
/// decides nothing and splits on atom 0 into a cut and a held child.
fn search(workers: usize) -> ParallelRegions {
    search_atoms(workers, 1)
}

fn search_atoms(workers: usize, atoms: usize) -> ParallelRegions {
    use zetesis_ferraris::Node;

    let mut nodes = vec![Node::False];
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let index = nodes.len();
        nodes.extend([
            Node::Atom(atom),
            Node::Implies(index, 0),
            Node::Or(index, index + 1),
        ]);
        roots.push(index + 2);
    }
    let theory = Theory::new(
        atoms,
        nodes,
        roots,
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
fn a_split_preserves_both_children_and_counts_the_net_gain() {
    let search = search(2);
    let root = search.shared.take_local(0).unwrap();
    let mut membership =
        crate::prepared_reduct::State::with_index(Arc::clone(&search.shared.index));
    let mut report = report();
    let mut budget = budget(&search.shared);
    // The root is the one outstanding region before it splits.
    assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 1);
    let stepped = step(
        &search.shared,
        root,
        0,
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
    let (cut, cut_knowledge) = search.shared.take_local(0).unwrap();
    let (held, held_knowledge) = search.shared.take_local(0).unwrap();
    assert!(cut.is_cut(0) && held.is_held(0));
    assert_eq!(cut_knowledge.len(), 1);
    assert_eq!(held_knowledge.len(), 1);
    assert!(search.shared.take_local(0).is_none());
}

#[test]
fn find_work_steals_a_peer_region_then_reports_done_at_quiescence() {
    let search = search(2);
    // The root starts on worker 0's deque; worker 1 steals it.
    let stolen = find_work(&search.shared, 1);
    assert!(stolen.is_some(), "an idle worker steals a peer's region");
    assert!(
        search.shared.take_local(0).is_none(),
        "the stolen region left the peer's deque"
    );
    // Every deque empty and nothing outstanding: the frontier is resolved.
    search.shared.outstanding.store(0, Ordering::Release);
    assert!(find_work(&search.shared, 1).is_none());
}

#[test]
fn find_work_returns_none_once_closed() {
    let search = search(2);
    search.shared.close();
    assert!(find_work(&search.shared, 1).is_none());
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
    let _root = search.shared.take_local(0).unwrap();
    // Work is outstanding (the root held here) but no deque holds it,
    // so the idle worker waits — and observes the cancellation.
    assert!(search.shared.outstanding.load(Ordering::Acquire) > 0);
    search.shared.cancellation.cancel();
    let result = find_work(&search.shared, 0);
    assert!(result.is_none());
    assert!(search.shared.closed.load(Ordering::Acquire));
    assert_eq!(search.shared.stopped(), Some(Incomplete::Cancelled));
}

fn split_next(shared: &Shared, index: usize) {
    let entry = shared.take_local(index).unwrap();
    let mut membership = crate::prepared_reduct::State::with_index(Arc::clone(&shared.index));
    let mut budget = budget(shared);
    let stepped = step(
        shared,
        entry,
        index,
        &mut budget,
        &mut membership,
        &mut report(),
        &mut None,
    )
    .unwrap();
    assert!(matches!(stepped, Stepped::Split));
}

#[test]
fn an_owner_and_thief_acquire_distinct_split_children() {
    let search = search(2);
    split_next(&search.shared, 0);
    let start = std::sync::Barrier::new(2);
    let (local, stolen) = std::thread::scope(|scope| {
        let local = scope.spawn(|| {
            start.wait();
            search.shared.take_local(0).unwrap()
        });
        start.wait();
        let stolen = find_work(&search.shared, 1).unwrap();
        (local.join().unwrap(), stolen)
    });
    assert!(local.0.is_cut(0));
    assert!(stolen.0.is_held(0));
    assert!(search.shared.take_local(0).is_none());
    assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 2);
}

#[test]
fn a_refused_split_leaves_the_frontier_unpublished() {
    let search = search_atoms(2, 2);
    split_next(&search.shared, 0);
    let (parent, knowledge) = search.shared.take_local(0).unwrap();
    let prior = search.shared.queue(0).front().unwrap().0.clone();
    let atom = parent.split_atom().unwrap();
    let (cut, held) = parent.split(atom);
    let result = search.shared.publish_split(
        0,
        (held, knowledge.clone()),
        (cut, knowledge),
        |queue, additional| {
            assert_eq!(additional, 2);
            // Exercise the actual reservation and error conversion, using
            // guaranteed capacity overflow rather than attempting host OOM.
            reserve_regions(queue, usize::MAX)
        },
    );
    assert_eq!(result, Err(Incomplete::Allocation));
    assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 2);
    assert_eq!(search.shared.queue(0).len(), 1);
    assert_eq!(search.shared.take_local(0).unwrap().0, prior);
    assert!(!search.exhausted);
}

#[test]
fn children_become_stealable_after_the_split_commit() {
    let search = search(2);
    let (parent, knowledge) = search.shared.take_local(0).unwrap();
    let (cut, held) = parent.split(0);
    let (prepared, preparation) = mpsc::sync_channel(1);
    let (release, resume) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        let shared = &search.shared;
        let publisher = scope.spawn(move || {
            shared.publish_split(
                0,
                (held, knowledge.clone()),
                (cut, knowledge),
                |queue, additional| {
                    reserve_regions(queue, additional)?;
                    prepared.send(()).unwrap();
                    resume.recv_timeout(WAIT).unwrap();
                    Ok(())
                },
            )
        });
        preparation.recv_timeout(WAIT).unwrap();
        assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 1);
        assert!(matches!(
            search.shared.queues[0].try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        release.send(()).unwrap();
        let stolen = find_work(&search.shared, 1).unwrap();
        assert!(stolen.0.is_held(0));
        assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 2);
        publisher.join().unwrap().unwrap();
    });
    assert!(search.shared.take_local(0).unwrap().0.is_cut(0));
    assert!(search.shared.take_local(0).is_none());
}

#[test]
fn a_busy_peer_does_not_block_an_available_steal() {
    let search = search(3);
    let root = search.shared.take_local(0).unwrap();
    reserve_regions(&mut search.shared.queue(1), 1).unwrap();
    search.shared.queue(1).push_back(root);
    let busy = search.shared.queue(0);
    let (done, result) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        scope.spawn(|| done.send(find_work(&search.shared, 2)).unwrap());
        let stolen = result.recv_timeout(WAIT);
        // Release before asserting so a blocking-lock mutation can exit too.
        drop(busy);
        assert!(stolen.unwrap().is_some());
    });
    assert!(search.shared.take_local(1).is_none());
}

#[test]
fn busy_peers_do_not_postpone_cancellation() {
    let search = search(2);
    let busy = search.shared.queue(0);
    search.shared.cancellation.cancel();
    let (done, result) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        scope.spawn(|| done.send(find_work(&search.shared, 1)).unwrap());
        let stopped = result.recv_timeout(WAIT);
        drop(busy);
        assert!(stopped.unwrap().is_none());
    });
    assert_eq!(search.shared.stopped(), Some(Incomplete::Cancelled));
    assert!(search.shared.closed.load(Ordering::Acquire));
}

#[test]
fn a_deep_local_walk_retains_one_sibling_per_level() {
    const ATOMS: usize = 80;
    let search = search_atoms(2, ATOMS);
    for depth in 0..ATOMS {
        split_next(&search.shared, 0);
        assert_eq!(search.shared.queue(0).len(), depth + 2);
        assert!(search.shared.queue(0).len() <= ATOMS + 1);
    }
    assert_eq!(search.shared.outstanding.load(Ordering::Acquire), ATOMS + 1);
}

#[test]
fn a_refused_worker_launch_joins_the_started_subset() {
    let mut search = search(2);
    let before = search.search_statistics().work;
    let finished = Arc::new(AtomicBool::new(false));
    let (ready, running) = mpsc::sync_channel(1);
    let result = search.start_with(None, false, |shared, index, _sender| {
        if index == 1 {
            running.recv_timeout(WAIT).unwrap();
            return Err(std::io::Error::other("injected thread creation refusal"));
        }
        let finished = Arc::clone(&finished);
        let ready = ready.clone();
        std::thread::Builder::new().spawn(move || {
            contain_worker(&shared, || {
                let lease = shared.budget.lease(&shared.cancellation);
                lease.tick().unwrap();
                ready.send(()).unwrap();
                while !shared.closed.load(Ordering::Acquire) {
                    shared.cancellation.poll().unwrap();
                    std::thread::yield_now();
                }
                drop(lease);
                finished.store(true, Ordering::Release);
                report()
            })
        })
    });
    assert_eq!(result, Err(Incomplete::Allocation));
    assert!(finished.load(Ordering::Acquire));
    assert!(search.handles.is_empty());
    assert!(search.receiver.is_none());
    assert!(!search.exhausted);
    let mut spent = SearchStatistics::default();
    search.shared.budget.record(&mut spent);
    assert_eq!(spent.work, before + 1);
    for queue in &search.shared.queues {
        let queue = queue.lock().unwrap();
        assert!(queue.is_empty());
        assert_eq!(queue.capacity(), 0);
    }
}

#[test]
fn stopping_releases_the_pending_frontier_storage() {
    let mut search = search(2);
    split_next(&search.shared, 0);
    assert_eq!(search.shared.queue(0).len(), 2);
    search.stop().unwrap();
    assert!(!search.exhausted);
    for queue in &search.shared.queues {
        let queue = queue.lock().unwrap();
        assert!(queue.is_empty());
        assert_eq!(queue.capacity(), 0);
    }
}

#[derive(Debug)]
struct RefuseChildren(Theory);

struct RefuseChildrenWorker<'a>(&'a Theory);

impl crate::RegionFilter for RefuseChildren {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn crate::RegionFilterWorker + '_>, Incomplete> {
        cancellation.poll()?;
        if !theory.same_instance(&self.0) {
            return Err(Incomplete::WrongTheory);
        }
        Ok(Box::new(RefuseChildrenWorker(&self.0)))
    }
}

impl crate::RegionFilterWorker for RefuseChildrenWorker<'_> {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<crate::RegionFeasibility, Incomplete> {
        cancellation.poll()?;
        assert!(theory.same_instance(self.0));
        if region.decision(0).is_some() {
            return Err(Incomplete::Allocation);
        }
        Ok(crate::RegionFeasibility::NotRefuted)
    }
}

#[test]
fn an_allocation_stop_closes_and_settles_the_frontier() {
    let mut search = search(2);
    search
        .set_filter(crate::region_filter::Filter::new(Arc::new(RefuseChildren(
            search.shared.index.theory().clone(),
        ))))
        .unwrap();
    let cancellation = search.shared.cancellation.clone();
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: search.shared.limits.search,
        cancellation: &cancellation,
        statistics: search.search_statistics(),
    };
    // The existing filter boundary injects the typed stop after a real root
    // split. This tests worker/coordinator composition, not allocator failure;
    // a_refused_split_leaves_the_frontier_unpublished covers reservation itself.
    assert!(matches!(
        search.propose(None, false, &mut budget),
        Err(Incomplete::Allocation)
    ));
    assert_eq!(search.shared.outstanding.load(Ordering::Acquire), 2);
    assert_eq!(search.shared.stopped(), Some(Incomplete::Allocation));
    assert!(search.shared.closed.load(Ordering::Acquire));
    assert!(!search.exhausted);
    assert_eq!(search.statistics().counts.leaves, 0);
    assert!(search.handles.is_empty());
    for queue in &search.shared.queues {
        let queue = queue.lock().unwrap();
        assert!(queue.is_empty());
        assert_eq!(queue.capacity(), 0);
    }
    let mut settled = SearchStatistics::default();
    search.shared.budget.record(&mut settled);
    // No leaf ran: all charged work is region preparation/narrowing and the
    // one decision tick. Every failed child prefix remains accounted.
    assert_eq!(settled.decisions, 1);
    assert_eq!(settled.work, search.statistics().counts.work + 1);
    assert_eq!(budget.statistics, settled);
    assert_eq!(search.stop(), Err(Incomplete::Allocation));
    assert!(!search.exhausted);
    // Prove that no unused grant remains held, even without debug assertions:
    // charging the entire unspent allowance must succeed after joined cleanup.
    search
        .shared
        .budget
        .charge(search.shared.limits.search.max_work - settled.work)
        .unwrap();
}
