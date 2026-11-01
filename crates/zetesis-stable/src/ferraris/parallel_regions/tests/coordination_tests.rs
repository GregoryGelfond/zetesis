//! Work-stealing coordination and termination for the region scheduler.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Instant;

use std::sync::TryLockError;

use super::*;
use crate::ferraris::region_frontier::Termination;

const WAIT: Duration = Duration::from_secs(2);

/// A two-branch theory: `Or(a0, not a0)` is a tautology, so its root region
/// decides nothing and splits on atom 0 into a cut and a held child. The
/// walk comes with the original index the enumeration would hand its workers.
fn search(workers: usize) -> (ParallelRegions, Arc<IndexedTheory>) {
    search_atoms(workers, 1)
}

/// A walk over `atoms` independent tautologies whose workers start at the
/// first proposal, which keeps the meaning of every test written before the
/// coordinator phase existed.
fn search_atoms(workers: usize, atoms: usize) -> (ParallelRegions, Arc<IndexedTheory>) {
    search_engaging(workers, atoms, Engagement::Immediate)
}

fn search_engaging(
    workers: usize,
    atoms: usize,
    engagement: Engagement,
) -> (ParallelRegions, Arc<IndexedTheory>) {
    use zetesis_ferraris::Node;

    let mut nodes = vec![Node::falsum()];
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let index = nodes.len();
        nodes.extend([
            Node::atom(atom),
            Node::implies(index, 0),
            Node::or_pair([index, index + 1]),
        ]);
        roots.push(index + 2);
    }
    let theory = Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = Cancellation::with_deadline(Instant::now() + 3 * WAIT).unwrap();
    let limits = Limits::default();
    let mut budget = Budget {
        quota: crate::budget::LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let search = ParallelRegions::new(
        crate::ferraris::regions::open(
            &theory,
            limits.max_reduct_bytes,
            limits.learning,
            &mut budget,
        )
        .unwrap(),
        NonZeroUsize::new(workers).unwrap(),
        engagement,
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap();
    (search, super::original(&theory))
}

/// Created-but-unresolved regions, as the workers read them.
fn outstanding(shared: &Shared) -> usize {
    shared.frontier.termination.unresolved()
}

fn budget(shared: &Shared) -> Budget<'_, WorkLease<'_>> {
    Budget {
        quota: shared.budget.lease(&shared.cancellation),
        limits: shared.limits.search,
        cancellation: &shared.cancellation,
        statistics: SearchStatistics::default(),
    }
}

/// The walker's idle wait: under the gate, the walk's end and a steal are
/// re-checked first.
fn wait_for_work(shared: &Shared, index: usize, timeout: Duration) -> Option<Entry> {
    shared
        .frontier
        .wait_for_work(timeout, |_| shared.frontier.steal(index))
}

/// The candidate region a worker took, where only candidate regions exist.
fn region(work: Work) -> Entry {
    match work {
        Work::Region(entry) => entry,
        Work::Published(..) => panic!("no query was published"),
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
    let (search, original) = search(2);
    let root = search.shared.frontier.take_local(0).unwrap();
    let mut report = report();
    let mut budget = budget(&search.shared);
    // The root is the one outstanding region before it splits.
    assert_eq!(outstanding(&search.shared), 1);
    let stepped = step(
        &search.shared,
        &original,
        (root, 0),
        &mut Workspaces::new(),
        &mut budget,
        &mut report,
        Timing::Inline,
    )
    .unwrap();
    assert!(matches!(stepped, Stepped::Split));
    // Two children replace the one parent: outstanding 1 -> 2, counted before
    // the pushes so no peer can observe a transient zero.
    assert_eq!(outstanding(&search.shared), 2);
    // Held is pushed last (on top of the LIFO deque), cut beneath it.
    let mut held = search.shared.frontier.take_local(0).unwrap();
    let mut cut = search.shared.frontier.take_local(0).unwrap();
    assert!(cut.region.is_cut(0) && held.region.is_held(0));
    let conditions = search.shared.restrictions.read().unwrap();
    // Narrowing the cut child learns that atom 0 is false. Its sibling must
    // retain independent knowledge and still admit the opposite decision.
    for entry in [&mut cut, &mut held] {
        assert_ne!(
            super::super::super::regions::narrow(
                (
                    search.shared.parts.subject(original.theory()).unwrap(),
                    original.narrower(),
                ),
                &conditions,
                entry,
                &mut zetesis_ferraris::NarrowingScratch::default(),
                &mut budget,
                &mut report.regions,
                None,
            )
            .unwrap(),
            Narrowing::Refuted,
        );
    }
    assert!(cut.region.is_cut(0) && held.region.is_held(0));
    assert!(search.shared.frontier.take_local(0).is_none());
}

#[test]
fn find_work_steals_a_peer_region_then_reports_done_at_quiescence() {
    let (search, _) = search(2);
    // The root starts on worker 0's deque; worker 1 steals it.
    let stolen = find_work(&search.shared, 1, IDLE_WAIT);
    assert!(stolen.is_some(), "an idle worker steals a peer's region");
    assert!(
        search.shared.frontier.take_local(0).is_none(),
        "the stolen region left the peer's deque"
    );
    // Resolving the stolen region leaves every deque empty and nothing
    // outstanding: the frontier is resolved.
    search.shared.frontier.termination.resolve();
    assert!(find_work(&search.shared, 1, IDLE_WAIT).is_none());
}

#[test]
fn a_split_overwrites_retired_knowledge() {
    use zetesis_ferraris::Node;
    use zetesis_theory_support::theories::theory;

    let (search, original) = search_atoms(2, 2);
    let mut workspaces = Workspaces::new();
    let mut report = report();
    let mut budget = budget(&search.shared);
    let root = search.shared.frontier.take_local(0).unwrap();
    assert!(matches!(
        step(
            &search.shared,
            &original,
            (root, 0),
            &mut workspaces,
            &mut budget,
            &mut report,
            Timing::Inline,
        )
        .unwrap(),
        Stepped::Split
    ));
    // Discover the actual split and its two children independently of either
    // atom preference or LIFO branch direction. Their inherited knowledge is
    // what this test exercises, not which policy selected the split.
    let first = search.shared.frontier.take_local(0).unwrap();
    let second = search.shared.frontier.take_local(0).unwrap();
    let split_atoms: Vec<_> = (0..2)
        .filter(|&atom| {
            first.region.is_cut(atom) && second.region.is_held(atom)
                || first.region.is_held(atom) && second.region.is_cut(atom)
        })
        .collect();
    assert_eq!(split_atoms.len(), 1);
    let atom = split_atoms[0];
    let (cut, held) = if first.region.is_cut(atom) {
        (first, second)
    } else {
        (second, first)
    };
    assert!(cut.region.is_cut(atom) && held.region.is_held(atom));
    // The cut child becomes contradictory after a new permanent restriction.
    let restriction = theory(2, vec![Node::atom(atom)], vec![0]);
    let narrower = Narrower::new(&restriction);
    search
        .shared
        .restrictions
        .write()
        .unwrap()
        .permanent
        .push(Arc::new((restriction, narrower)));
    assert!(matches!(
        step(
            &search.shared,
            &original,
            (cut, 0),
            &mut workspaces,
            &mut budget,
            &mut report,
            Timing::Inline,
        )
        .unwrap(),
        Stepped::Resolved(None)
    ));
    search.shared.frontier.termination.resolve();
    assert!(workspaces.retired_knowledge.is_some());

    assert!(matches!(
        step(
            &search.shared,
            &original,
            (held, 0),
            &mut workspaces,
            &mut budget,
            &mut report,
            Timing::Inline,
        )
        .unwrap(),
        Stepped::Split
    ));
    assert!(workspaces.retired_knowledge.is_none());
    // Both children must reach membership checking. Reusing the contradictory
    // conclusions would instead refute the copied child during narrowing.
    for _ in 0..2 {
        let child = search.shared.frontier.take_local(0).unwrap();
        assert!(matches!(
            step(
                &search.shared,
                &original,
                (child, 0),
                &mut workspaces,
                &mut budget,
                &mut report,
                Timing::Inline,
            )
            .unwrap(),
            Stepped::Resolved(_)
        ));
        search.shared.frontier.termination.resolve();
    }
    assert_eq!(report.regions.leaves, 2);
    assert_eq!(report.regions.refuted, 1);
    assert_eq!(outstanding(&search.shared), 0);
}

#[test]
fn find_work_returns_none_once_closed() {
    let (search, _) = search(2);
    search.shared.close();
    assert!(find_work(&search.shared, 1, IDLE_WAIT).is_none());
}

#[test]
fn the_first_stop_is_the_one_reported() {
    let (search, _) = search(2);
    search.shared.stop(Incomplete::WorkLimit);
    search.shared.stop(Incomplete::DecisionLimit);
    assert_eq!(search.shared.stopped(), Some(Incomplete::WorkLimit));
    assert!(search.shared.frontier.termination.is_closed());
}

#[test]
fn an_idle_worker_stops_on_cancellation_and_records_it() {
    let (search, _) = search(2);
    let _root = search.shared.frontier.take_local(0).unwrap();
    // Work is outstanding (the root held here) but no deque holds it,
    // so the idle worker waits — and observes the cancellation.
    assert!(outstanding(&search.shared) > 0);
    search.shared.cancellation.cancel();
    let result = find_work(&search.shared, 0, IDLE_WAIT);
    assert!(result.is_none());
    assert!(search.shared.frontier.termination.is_closed());
    assert_eq!(search.shared.stopped(), Some(Incomplete::Cancelled));
}

/// Long enough that a test which ends the walk fails at `WAIT`, rather than
/// passing late, when the idle worker it releases is not woken.
const IDLE: Duration = Duration::from_mins(1);

/// Hold the root in hand, so one region is outstanding and no deque holds
/// work, let a thief wait for the walk to end, then end it with `end`. The
/// thief must be back within `WAIT`, not after its `IDLE` wait.
fn idle_thief_after(end: impl FnOnce(&Shared)) -> Option<Entry> {
    let (search, _) = search(2);
    let _root = search.shared.frontier.take_local(0).unwrap();
    let shared = Arc::clone(&search.shared);
    let (done, result) = mpsc::sync_channel(1);
    // Detached: a thief that is never woken sleeps out `IDLE`, and the test
    // must fail at `WAIT` rather than wait for it. Its send can find the
    // receiver gone only after that failure, so the result is not needed.
    std::thread::spawn(move || {
        let _ = done.send(find_work(&shared, 1, IDLE).map(region));
    });
    // Time for the thief to spin out its rounds and start waiting, so the
    // walk ends while it waits; a thief that has not yet waited finds the
    // walk already ended.
    std::thread::sleep(Duration::from_millis(50));
    end(&search.shared);
    result
        .recv_timeout(WAIT)
        .expect("the idle worker was not woken")
}

#[test]
fn an_idle_worker_leaves_when_the_last_region_resolves() {
    assert!(idle_thief_after(|shared| shared.frontier.termination.resolve()).is_none());
}

#[test]
fn an_idle_worker_leaves_when_the_walk_closes() {
    assert!(idle_thief_after(Shared::close).is_none());
}

#[test]
fn an_idle_worker_leaves_when_a_peer_stops_the_walk() {
    assert!(idle_thief_after(|shared| shared.stop(Incomplete::WorkLimit)).is_none());
}

#[test]
fn an_idle_worker_takes_a_region_published_while_it_waits() {
    let (search, original) = search(2);
    let root = search.shared.frontier.take_local(0).unwrap();
    let shared = Arc::clone(&search.shared);
    let (done, result) = mpsc::sync_channel(1);
    // Detached, as in `idle_thief_after`: a thief that a publication does not
    // wake sleeps out `IDLE`, and the test fails at `WAIT`.
    std::thread::spawn(move || {
        let _ = done.send(find_work(&shared, 1, IDLE));
    });
    std::thread::sleep(Duration::from_millis(50));
    // Split the root for real: `step` publishes both children on deque 0.
    let stepped = step(
        &search.shared,
        &original,
        (root, 0),
        &mut Workspaces::new(),
        &mut budget(&search.shared),
        &mut report(),
        Timing::Inline,
    )
    .unwrap();
    assert!(matches!(stepped, Stepped::Split));
    let stolen = result
        .recv_timeout(WAIT)
        .expect("the idle worker was not woken by the publication");
    assert!(stolen.is_some());
}

#[test]
fn the_idle_wait_returns_at_once_after_the_walk_ended() {
    // The walk ends before the worker reaches the gate: the waiter must see
    // that under the gate and not sleep out its timeout.
    for end in [Termination::resolve as fn(&Termination), Termination::close] {
        let (search, _) = search(2);
        let _root = search.shared.frontier.take_local(0).unwrap();
        end(&search.shared.frontier.termination);
        let started = Instant::now();
        assert!(wait_for_work(&search.shared, 1, IDLE).is_none());
        assert!(started.elapsed() < WAIT);
    }
}

fn split_next(shared: &Shared, original: &IndexedTheory, index: usize) {
    let entry = shared.frontier.take_local(index).unwrap();
    let mut budget = budget(shared);
    let stepped = step(
        shared,
        original,
        (entry, index),
        &mut Workspaces::new(),
        &mut budget,
        &mut report(),
        Timing::Inline,
    )
    .unwrap();
    assert!(matches!(stepped, Stepped::Split));
}

#[test]
fn an_owner_and_thief_acquire_distinct_split_children() {
    let (search, original) = search(2);
    split_next(&search.shared, &original, 0);
    let start = std::sync::Barrier::new(2);
    let (local, stolen) = std::thread::scope(|scope| {
        let local = scope.spawn(|| {
            start.wait();
            search.shared.frontier.take_local(0).unwrap()
        });
        start.wait();
        let stolen = find_work(&search.shared, 1, IDLE_WAIT).map(region).unwrap();
        (local.join().unwrap(), stolen)
    });
    assert!(local.region.is_held(0));
    assert!(stolen.region.is_cut(0));
    assert!(search.shared.frontier.take_local(0).is_none());
    assert_eq!(outstanding(&search.shared), 2);
}

#[test]
fn a_refused_split_leaves_the_frontier_unpublished() {
    let (search, original) = search_atoms(2, 2);
    split_next(&search.shared, &original, 0);
    let parent = search.shared.frontier.take_local(0).unwrap();
    let prior = search
        .shared
        .frontier
        .queue(0)
        .front()
        .unwrap()
        .region
        .clone();
    let atom = parent.split_atom().unwrap();
    let interval = parent.region.clone();
    let preference = parent.split_atom();
    let knowledge_before = format!("{:?}", parent.state);
    let cut_knowledge = parent.state.clone();
    let mut pending = Some(parent);
    let error = search
        .shared
        .publish_split(0, &mut pending, atom, cut_knowledge, |queue, additional| {
            assert_eq!(additional, 2);
            // Exercise the actual reservation and error conversion, using
            // guaranteed capacity overflow rather than attempting host OOM.
            reserve_regions(queue, usize::MAX)
        })
        .unwrap_err();
    let parent = pending.take().expect("refusal retains the whole parent");
    assert_eq!(error, Incomplete::Allocation);
    assert_eq!(parent.region, interval);
    assert_eq!(parent.split_atom(), preference);
    assert_eq!(format!("{:?}", parent.state), knowledge_before);
    assert_eq!(outstanding(&search.shared), 2);
    assert_eq!(search.shared.frontier.queue(0).len(), 1);
    assert_eq!(search.shared.frontier.take_local(0).unwrap().region, prior);
    assert!(!search.exhausted);
}

#[test]
fn children_become_stealable_after_the_split_commit() {
    let (search, _) = search(2);
    let parent = search.shared.frontier.take_local(0).unwrap();
    let cut_knowledge = parent.state.clone();
    let (prepared, preparation) = mpsc::sync_channel(1);
    let (release, resume) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        let shared = &search.shared;
        let publisher = scope.spawn(move || {
            let mut pending = Some(parent);
            let outcome =
                shared.publish_split(0, &mut pending, 0, cut_knowledge, |queue, additional| {
                    reserve_regions(queue, additional)?;
                    prepared.send(()).unwrap();
                    resume.recv_timeout(WAIT).unwrap();
                    Ok(())
                });
            assert!(pending.is_none(), "an admitted split consumes the parent");
            outcome
        });
        preparation.recv_timeout(WAIT).unwrap();
        assert_eq!(outstanding(&search.shared), 1);
        assert!(matches!(
            search.shared.frontier.queues[0].try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        release.send(()).unwrap();
        let stolen = find_work(&search.shared, 1, IDLE_WAIT).map(region).unwrap();
        assert!(stolen.region.is_cut(0));
        assert_eq!(outstanding(&search.shared), 2);
        publisher.join().unwrap().unwrap();
    });
    assert!(
        search
            .shared
            .frontier
            .take_local(0)
            .unwrap()
            .region
            .is_held(0)
    );
    assert!(search.shared.frontier.take_local(0).is_none());
}

#[test]
fn a_busy_peer_does_not_block_an_available_steal() {
    let (search, _) = search(3);
    let root = search.shared.frontier.take_local(0).unwrap();
    reserve_regions(&mut search.shared.frontier.queue(1), 1).unwrap();
    search.shared.frontier.queue(1).push_back(root);
    let busy = search.shared.frontier.queue(0);
    let (done, result) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        scope.spawn(|| done.send(find_work(&search.shared, 2, IDLE_WAIT)).unwrap());
        let stolen = result.recv_timeout(WAIT);
        // Release before asserting so a blocking-lock mutation can exit too.
        drop(busy);
        assert!(stolen.unwrap().is_some());
    });
    assert!(search.shared.frontier.take_local(1).is_none());
}

#[test]
fn busy_peers_do_not_postpone_cancellation() {
    let (search, _) = search(2);
    let busy = search.shared.frontier.queue(0);
    search.shared.cancellation.cancel();
    let (done, result) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        scope.spawn(|| done.send(find_work(&search.shared, 1, IDLE_WAIT)).unwrap());
        let stopped = result.recv_timeout(WAIT);
        drop(busy);
        assert!(stopped.unwrap().is_none());
    });
    assert_eq!(search.shared.stopped(), Some(Incomplete::Cancelled));
    assert!(search.shared.frontier.termination.is_closed());
}

#[test]
fn a_deep_local_walk_retains_one_sibling_per_level() {
    const ATOMS: usize = 80;
    let (search, original) = search_atoms(2, ATOMS);
    for depth in 0..ATOMS {
        split_next(&search.shared, &original, 0);
        assert_eq!(search.shared.frontier.queue(0).len(), depth + 2);
        assert!(search.shared.frontier.queue(0).len() <= ATOMS + 1);
    }
    assert_eq!(outstanding(&search.shared), ATOMS + 1);
}

#[test]
fn a_refused_worker_launch_joins_the_started_subset() {
    let (mut search, _) = search(2);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = Budget {
        quota: crate::budget::LocalQuota,
        limits: search.shared.limits.search,
        cancellation: &cancellation,
        statistics: search.search_statistics(),
    };
    search.begin(None, false, &mut budget).unwrap();
    let before = search.search_statistics().work;
    let finished = Arc::new(AtomicBool::new(false));
    let (ready, running) = mpsc::sync_channel(1);
    let result = search.engage_with(&mut budget, |shared, index, _sender| {
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
                while !shared.frontier.termination.is_closed() {
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
    assert!(matches!(
        search.record,
        WalkRecord::Engaged { launched: 1, .. }
    ));
    assert!(finished.load(Ordering::Acquire));
    assert!(search.handles.is_empty());
    assert!(search.receiver.is_none());
    assert!(!search.exhausted);
    let mut spent = SearchStatistics::default();
    search.shared.budget.record(&mut spent);
    assert_eq!(spent.work, before + 1);
    for queue in &search.shared.frontier.queues {
        let queue = queue.lock().unwrap();
        assert!(queue.is_empty());
        assert_eq!(queue.capacity(), 0);
    }
}

#[test]
fn stopping_releases_the_pending_frontier_storage() {
    let (mut search, original) = search(2);
    split_next(&search.shared, &original, 0);
    assert_eq!(search.shared.frontier.queue(0).len(), 2);
    search.stop().unwrap();
    assert!(!search.exhausted);
    for queue in &search.shared.frontier.queues {
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
    // Under `After(Duration::MAX)` the refused child is the coordinator's
    // step, and its pull releases the frontier before it returns the stop.
    for engagement in [Engagement::Immediate, Engagement::After(Duration::MAX)] {
        allocation_stop_closes_and_settles_the_frontier(engagement);
    }
}

fn allocation_stop_closes_and_settles_the_frontier(engagement: Engagement) {
    let (mut search, original) = search_engaging(2, 1, engagement);
    search
        .set_filter(crate::region_filter::Filter::new(Arc::new(RefuseChildren(
            original.theory().clone(),
        ))))
        .unwrap();
    let cancellation = search.shared.cancellation.clone();
    let mut budget = Budget {
        quota: crate::budget::LocalQuota,
        limits: search.shared.limits.search,
        cancellation: &cancellation,
        statistics: search.search_statistics(),
    };
    // The existing filter boundary injects the typed stop after a real root
    // split. This tests worker/coordinator composition, not allocator failure;
    // a_refused_split_leaves_the_frontier_unpublished covers reservation itself.
    assert!(matches!(
        search.propose(&original, None, false, &mut budget),
        Err(Incomplete::Allocation)
    ));
    if engagement == Engagement::Immediate {
        assert!(matches!(
            search.record,
            WalkRecord::Engaged { launched: 2, .. }
        ));
    } else {
        assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
    }
    assert_eq!(outstanding(&search.shared), 2);
    assert_eq!(search.shared.stopped(), Some(Incomplete::Allocation));
    assert!(search.shared.frontier.termination.is_closed());
    assert!(!search.exhausted);
    assert_eq!(search.statistics().counts.leaves, 0);
    assert!(search.handles.is_empty());
    for queue in &search.shared.frontier.queues {
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

#[test]
fn coordinator_observations_saturate_with_worker_receipts() {
    let (mut search, _) = search(2);
    search.statistics.counts.propagations = u64::MAX;
    search.statistics.counts.held = u64::MAX;
    search.statistics.counts.cut = u64::MAX;
    Live::add(&search.shared.live.propagations, 1);
    Live::add(&search.shared.live.held, 1);
    Live::add(&search.shared.live.cut, 1);
    let counts = search.statistics().counts;
    assert_eq!(counts.propagations, u64::MAX);
    assert_eq!(counts.held, u64::MAX);
    assert_eq!(counts.cut, u64::MAX);
}

/// Busy-wait for `turns` spin hints: a head start for one side of a race.
fn spin(turns: u32) {
    for _ in 0..turns {
        std::hint::spin_loop();
    }
}

/// A protocol harness for the idle wake: a waiter whose idle wait is a full
/// minute must take a region a publisher pushes within a deadline far below
/// that minute, across many interleavings of the two sides. Only the waiter
/// can take the region (its owner never pops it before the deadline), so a
/// lost wakeup fails the deadline. This checks the translation of the
/// protocol; its ordering argument is the design's.
#[test]
fn a_waiting_worker_never_misses_a_publication() {
    const ROUNDS: u32 = 5000;
    const IDLE: Duration = Duration::from_mins(1);
    let (search, _) = search(2);
    let shared = &search.shared;
    let root = shared.frontier.take_local(0).unwrap();
    for round in 0..ROUNDS {
        // A deterministic spread of head starts for either side.
        let waiter_start = round.wrapping_mul(7919) % 3000;
        let publisher_start = round.wrapping_mul(104_729) % 3000;
        let (taken, done) = mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                spin(waiter_start);
                loop {
                    if shared.frontier.steal(1).is_some() {
                        let _ = taken.send(());
                        break;
                    }
                    if wait_for_work(shared, 1, IDLE).is_some() {
                        let _ = taken.send(());
                        break;
                    }
                    if shared.frontier.termination.has_ended() {
                        break;
                    }
                }
            });
            spin(publisher_start);
            shared
                .publish_split(
                    0,
                    &mut Some(root.clone()),
                    0,
                    root.state.clone(),
                    reserve_regions,
                )
                .unwrap();
            if done.recv_timeout(WAIT).is_err() {
                // End the walk so the stranded waiter returns at once.
                shared.close();
                panic!("round {round}: the waiting worker missed the publication");
            }
        });
        // The owner takes the other child, so each round starts empty.
        assert!(shared.frontier.take_local(0).is_some());
    }
}

/// The number of registered idle waiters, read under the gate that guards
/// its changes.
fn waiting(search: &ParallelRegions) -> usize {
    let termination = &search.shared.frontier.termination;
    let _gate = termination
        .gate
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    termination.idle_waiters.load(Ordering::Relaxed)
}

#[test]
fn a_waiter_finding_a_region_at_its_recheck_deregisters() {
    let (search, _) = search(2);
    let shared = &search.shared;
    let root = shared.frontier.take_local(0).unwrap();
    let cut_knowledge = root.state.clone();
    shared
        .publish_split(0, &mut Some(root), 0, cut_knowledge, reserve_regions)
        .unwrap();
    assert!(wait_for_work(shared, 1, WAIT).is_some());
    assert_eq!(waiting(&search), 0);
}

#[test]
fn a_waiter_timing_out_deregisters() {
    let (search, _) = search(2);
    let _root = search.shared.frontier.take_local(0).unwrap();
    assert!(wait_for_work(&search.shared, 1, Duration::from_millis(1)).is_none());
    assert_eq!(waiting(&search), 0);
}

#[test]
fn a_waiter_woken_without_a_region_deregisters() {
    let (search, _) = search(2);
    let shared = &search.shared;
    let _root = shared.frontier.take_local(0).unwrap();
    std::thread::scope(|scope| {
        let waiter = scope.spawn(|| wait_for_work(shared, 1, Duration::from_mins(1)));
        while waiting(&search) == 0 {
            std::hint::spin_loop();
        }
        // The waiter registers holding the gate and releases it only by
        // waiting: once the count reads one, it waits.
        shared.frontier.termination.wake_one();
        assert!(waiter.join().unwrap().is_none());
    });
    assert_eq!(waiting(&search), 0);
}

#[test]
fn a_waiter_released_by_the_end_of_the_walk_deregisters() {
    let (search, _) = search(2);
    let shared = &search.shared;
    let _root = shared.frontier.take_local(0).unwrap();
    std::thread::scope(|scope| {
        let waiter = scope.spawn(|| wait_for_work(shared, 1, Duration::from_mins(1)));
        while waiting(&search) == 0 {
            std::hint::spin_loop();
        }
        shared.close();
        assert!(waiter.join().unwrap().is_none());
    });
    assert_eq!(waiting(&search), 0);
}

#[test]
fn a_worker_splits_the_root_on_the_original_choice_head() {
    let theory = crate::ferraris::fixtures::choice_beside_a_loop();
    let cancellation = Cancellation::with_deadline(Instant::now() + 3 * WAIT).unwrap();
    let limits = Limits::default();
    let search = ParallelRegions::new(
        crate::ferraris::regions::open(
            &theory,
            limits.max_reduct_bytes,
            limits.learning,
            &mut Budget {
                quota: crate::budget::LocalQuota,
                limits: limits.search,
                cancellation: &cancellation,
                statistics: SearchStatistics::default(),
            },
        )
        .unwrap(),
        NonZeroUsize::new(1).unwrap(),
        Engagement::Immediate,
        limits,
        cancellation.clone(),
        &mut Budget {
            quota: crate::budget::LocalQuota,
            limits: limits.search,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        },
    )
    .unwrap();
    let original = super::original(&theory);
    let root = search.shared.frontier.take_local(0).unwrap();
    let stepped = step(
        &search.shared,
        &original,
        (root, 0),
        &mut Workspaces::new(),
        &mut budget(&search.shared),
        &mut report(),
        Timing::Inline,
    )
    .unwrap();
    assert!(matches!(stepped, Stepped::Split));
    // The children differ only in the split atom: held on top, cut beneath.
    let held = search.shared.frontier.take_local(0).unwrap();
    let cut = search.shared.frontier.take_local(0).unwrap();
    let split: Vec<_> = (0..theory.atom_count())
        .filter(|&atom| cut.region.is_cut(atom) && held.region.is_held(atom))
        .collect();
    assert_eq!(split, [0]);
}
