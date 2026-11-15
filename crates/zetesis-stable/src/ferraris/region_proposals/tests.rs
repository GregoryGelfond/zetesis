use crate::region_filter::tests::injection::{Source as InjectedSource, Worker as InjectedWorker};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{
    Attempt, CandidateKnowledge, Entry, FrontierOrder, Mutex, Outcome, RegionProposals, SlateFill,
    Step, Termination, Workspace, reserve_regions,
};
use crate::budget::{Budget, LocalQuota, SharedBudget};
use crate::{
    Cancellation, Incomplete, Limits, RegionFeasibility, SearchLimits, SearchStatistics,
    StableModels,
};
use zetesis_ferraris::{AdmissionLimits, Interpretation, Narrower, Node, Region, Theory};

impl RegionProposals {
    pub(in crate::ferraris) fn enable_region_checks(&mut self) {
        let statistics = self.statistics();
        self.parts.enable_region_checks(&statistics);
    }
}

fn nonzero(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).unwrap()
}

/// `atoms` independent tautologies `a or not a`: no reading decides an atom,
/// so every region splits until all are decided, and every leaf is a
/// candidate.
fn tautologies(atoms: usize) -> Theory {
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
    Theory::new(
        atoms,
        zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// A theory whose root reading refutes the whole candidate space.
fn falsum() -> Theory {
    Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![Node::falsum()], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn proposals(theory: &Theory, producers: usize) -> RegionProposals {
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    RegionProposals::new(
        crate::ferraris::regions::open(
            theory,
            u64::MAX,
            zetesis_ferraris::LearningLimits::DEFAULT,
            &mut budget,
        )
        .unwrap(),
        nonzero(producers),
        u64::MAX,
    )
    .unwrap()
}

/// One slate of `maximum` candidates through `fill`, with the produced
/// candidates.
fn fill(
    proposals: &mut RegionProposals,
    theory: &Theory,
    maximum: usize,
) -> (super::Produced, Vec<Interpretation>) {
    let cancellation = Cancellation::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let narrower = Narrower::new(theory);
    let mut output = Vec::with_capacity(maximum);
    let produced = proposals.fill(
        (theory, &narrower),
        maximum,
        u64::MAX,
        &mut budget,
        &mut output,
        false,
    );
    (produced, output)
}

/// A slate whose transitions a test calls one at a time: `proposals`'
/// frontier with its own allowance and no filter.
struct Driver {
    theory: Theory,
    narrower: Narrower,
    proposals: RegionProposals,
    allowance: SharedBudget,
    cancellation: Cancellation,
}

fn driver(theory: Theory, producers: usize) -> Driver {
    Driver {
        narrower: Narrower::new(&theory),
        proposals: proposals(&theory, producers),
        allowance: SharedBudget::new(SearchLimits::default(), SearchStatistics::default()),
        cancellation: Cancellation::default(),
        theory,
    }
}

impl Driver {
    /// A driver over a stream's own producers and frontier, resuming them as
    /// each proposal does, so its slates take the order the stream proposes
    /// with.
    fn of_stream(search: StableModels) -> Self {
        let theory = search.theory.clone();
        let super::super::Proposer::Proposals(proposals) = search.proposer else {
            panic!("a stream with region producers proposes through them");
        };
        let mut proposals = *proposals;
        proposals.frontier.termination =
            Termination::resuming(proposals.frontier.termination.unresolved());
        Self {
            narrower: Narrower::new(&theory),
            proposals,
            allowance: SharedBudget::new(SearchLimits::default(), SearchStatistics::default()),
            cancellation: Cancellation::default(),
            theory,
        }
    }

    fn slate(&self, maximum: usize) -> SlateFill<'_, '_> {
        self.slate_in(maximum, FrontierOrder::PerProducer)
    }

    /// The deque on which producer `index` publishes its first split, in a
    /// slate taking the stream's own order. Leaves it takes first are emitted.
    fn first_publication(&self, index: usize, producers: usize) -> usize {
        let slate = self.slate_in(64, self.proposals.order);
        let lease = self.allowance.lease(&self.cancellation);
        loop {
            let mut entry = slate
                .admit(index, Attempt::Outside, &lease)
                .expect("a queued region remains");
            match step(&slate, &mut entry) {
                Step::Split(atom) => {
                    let mut published = entry.region.clone();
                    assert!(published.cut(atom));
                    let _hand = slate
                        .copy_and_hand_over(index, &mut Some(entry), atom, reserve_regions)
                        .unwrap();
                    return (0..producers)
                        .find(|&deque| {
                            slate
                                .frontier
                                .queue(deque)
                                .back()
                                .is_some_and(|newest| newest.region == published)
                        })
                        .expect("the published child is queued");
                }
                Step::Candidate(candidate) => slate.emit(candidate),
                Step::Refuted => panic!("tautologies refute no region"),
                Step::Deferred => panic!("a slate of 64 has room for the leaves taken first"),
            }
        }
    }

    fn slate_in(&self, maximum: usize, order: FrontierOrder) -> SlateFill<'_, '_> {
        SlateFill {
            theory: &self.theory,
            filter: None,
            parts: &self.proposals.parts,
            narrower: &self.narrower,
            restrictions: &self.proposals.restrictions,
            allowance: &self.allowance,
            limits: SearchLimits::default(),
            cancellation: &self.cancellation,
            frontier: &self.proposals.frontier,
            maximum,
            remaining: u64::MAX,
            hand_limit: super::slate_hands(maximum, self.proposals.pool.current_num_threads()),
            hands: AtomicUsize::new(0),
            output_slots: AtomicUsize::new(0),
            outcome: Mutex::new(Outcome {
                candidates: Vec::with_capacity(maximum),
                stopped: None,
            }),
            order,
        }
    }
}

/// One step of `entry` under a fresh lease of the slate's allowance.
fn step(slate: &SlateFill<'_, '_>, entry: &mut Entry) -> Step {
    let mut budget = Budget {
        quota: slate.allowance.lease(slate.cancellation),
        limits: slate.limits,
        cancellation: slate.cancellation,
        statistics: SearchStatistics::default(),
    };
    slate
        .step(
            entry,
            &mut Workspace::default(),
            &mut budget,
            &mut super::RegionCounts::default(),
        )
        .unwrap()
}

/// Existing transition tests isolate deque publication after a real admitted
/// copy. Production receipt assertions use `produce` in the copy tests below.
impl SlateFill<'_, '_> {
    fn copy_and_hand_over(
        &self,
        index: usize,
        pending: &mut Option<Entry>,
        atom: usize,
        reserve: impl FnOnce(&mut std::collections::VecDeque<Entry>, usize) -> Result<(), Incomplete>,
    ) -> Result<Entry, Incomplete> {
        let mut budget = Budget {
            quota: self.allowance.lease(self.cancellation),
            limits: self.limits,
            cancellation: self.cancellation,
            statistics: SearchStatistics::default(),
        };
        let copied = super::regions::copy_knowledge(
            &pending
                .as_ref()
                .expect("test publication has a parent")
                .state,
            None,
            &mut budget,
        );
        self.hand_over(index, pending, atom, copied.result?, reserve)
    }
}

fn open(atoms: usize) -> Entry {
    Entry::new(Region::all_open(atoms), CandidateKnowledge::default())
}

fn slots(slate: &SlateFill<'_, '_>) -> usize {
    slate.output_slots.load(Ordering::Acquire)
}

fn hands(slate: &SlateFill<'_, '_>) -> usize {
    slate.hands.load(Ordering::Acquire)
}

fn stopped(slate: &SlateFill<'_, '_>) -> Option<Incomplete> {
    slate.outcome().stopped
}

fn emitted(slate: &SlateFill<'_, '_>) -> usize {
    slate.outcome().candidates.len()
}

/// A slate of one candidate that a producer filled with the leaf of an
/// atomless theory.
fn filled(driver: &Driver) -> SlateFill<'_, '_> {
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Candidate(candidate) = step(&slate, &mut root) else {
        panic!("an atomless region is a leaf");
    };
    slate.emit(candidate);
    slate
}

#[test]
fn admission_never_exceeds_the_slates_hands() {
    let driver = driver(tautologies(3), 3);
    let slate = driver.slate(2);
    for index in 1..3 {
        slate
            .frontier
            .requeue(index, open(3), reserve_regions)
            .unwrap();
    }
    let lease = driver.allowance.lease(&driver.cancellation);
    let admitted = (0..3)
        .filter_map(|index| slate.admit(index, Attempt::Outside, &lease))
        .count();
    assert_eq!(admitted, 2);
    assert_eq!(hands(&slate), 2);
}

#[test]
fn a_slate_of_more_candidates_than_producers_admits_every_producer() {
    let driver = driver(tautologies(3), 3);
    let slate = driver.slate(64);
    for index in 1..3 {
        slate
            .frontier
            .requeue(index, open(3), reserve_regions)
            .unwrap();
    }
    let lease = driver.allowance.lease(&driver.cancellation);
    let admitted = (0..3)
        .filter_map(|index| slate.admit(index, Attempt::Outside, &lease))
        .count();
    assert_eq!(admitted, 3);
}

/// At a slate's end, admission does not wait for output slots: three of four
/// are reserved, as three leaf steps leave them, and both producers still take
/// a region.
#[test]
fn admission_ignores_the_output_slots_already_reserved() {
    let driver = driver(tautologies(3), 2);
    let slate = driver.slate(4);
    for _ in 0..3 {
        assert!(slate.reserve_output_slot());
    }
    slate.frontier.requeue(1, open(3), reserve_regions).unwrap();
    let lease = driver.allowance.lease(&driver.cancellation);
    let admitted = (0..2)
        .filter_map(|index| slate.admit(index, Attempt::Outside, &lease))
        .count();
    assert_eq!(admitted, 2);
}

/// A slate of one candidate is one walk: while one producer holds its one
/// hand no peer takes a region, and the hand passes once it is released.
#[test]
fn a_slate_of_one_candidate_admits_one_producer_at_a_time() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    slate.frontier.requeue(1, open(1), reserve_regions).unwrap();
    let lease = driver.allowance.lease(&driver.cancellation);
    let _root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    assert!(slate.admit(1, Attempt::Outside, &lease).is_none());
    slate.release_hand();
    assert!(slate.admit(1, Attempt::Outside, &lease).is_some());
}

#[test]
fn the_emission_that_fills_the_slate_closes_it_without_a_stop() {
    let driver = driver(tautologies(0), 2);
    let slate = filled(&driver);
    assert!(slate.frontier.termination.is_closed());
    assert!(stopped(&slate).is_none());
    // Every output slot is a candidate's. The slate's one hand was its
    // emitter's, so no other producer holds a region.
    assert_eq!(slots(&slate), emitted(&slate));
}

/// A leaf step that finds the output spoken for defers its leaf: it neither
/// emits nor resolves its region, and counts no leaf.
#[test]
fn a_leaf_finding_the_output_spoken_for_stays_unresolved() {
    let driver = driver(tautologies(0), 2);
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    // Another leaf step reserved the slate's one output slot.
    assert!(slate.reserve_output_slot());
    let mut counts = super::RegionCounts::default();
    let mut budget = Budget {
        quota: slate.allowance.lease(slate.cancellation),
        limits: slate.limits,
        cancellation: slate.cancellation,
        statistics: SearchStatistics::default(),
    };
    let result = slate
        .step(
            &mut root,
            &mut Workspace::default(),
            &mut budget,
            &mut counts,
        )
        .unwrap();
    assert!(matches!(result, Step::Deferred));
    assert_eq!(counts.leaves, 0);
    assert_eq!(slate.frontier.termination.unresolved(), 1);
    assert_eq!(emitted(&slate), 0);
}

/// Once the output is spoken for the slate is about to close, and a region
/// taken before the close is returned unstepped.
#[test]
fn a_region_taken_once_the_output_is_spoken_for_is_returned_unstepped() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let root = slate.frontier.take_local(0).unwrap();
    assert!(slate.reserve_output_slot());
    assert!(!slate.frontier.termination.is_closed());
    assert!(slate.ready(root).is_break());
}

#[test]
fn a_deferred_leaf_is_emitted_by_the_next_slate() {
    let mut driver = driver(tautologies(0), 2);
    {
        let slate = driver.slate(1);
        let lease = driver.allowance.lease(&driver.cancellation);
        let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
        assert!(slate.reserve_output_slot());
        assert!(matches!(step(&slate, &mut root), Step::Deferred));
        slate.leave(0, Some(root));
    }
    driver.proposals.frontier.termination =
        Termination::resuming(driver.proposals.frontier.termination.unresolved());
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Candidate(candidate) = step(&slate, &mut root) else {
        panic!("the returned leaf is a leaf");
    };
    slate.emit(candidate);
    assert_eq!(emitted(&slate), 1);
    assert!(slate.frontier.termination.is_resolved());
}

/// In a slate of more than one candidate a step can be under way when the
/// slate fills. If it fails, its stop is recorded beside the full output,
/// which the batch keeps and checks before it reports the stop.
#[test]
fn a_step_failing_after_the_filling_emission_keeps_the_full_output() {
    let driver = driver(tautologies(2), 2);
    let slate = driver.slate(2);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let mut first = slate
        .copy_and_hand_over(0, &mut Some(root), atom, reserve_regions)
        .unwrap();
    // The peer steals the published child and is under way when the slate
    // fills from the other child's two leaves.
    let under_way = slate.admit(1, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut first) else {
        panic!("a region with an open atom splits");
    };
    let mut newer = slate
        .copy_and_hand_over(0, &mut Some(first), atom, reserve_regions)
        .unwrap();
    let mut older = slate.frontier.take_local(0).unwrap();
    for leaf in [&mut newer, &mut older] {
        let Step::Candidate(candidate) = step(&slate, leaf) else {
            panic!("a decided tautology is a leaf");
        };
        slate.emit(candidate);
    }
    assert!(slate.frontier.termination.is_closed());
    slate.fail(1, under_way, Incomplete::WorkLimit);
    assert_eq!(stopped(&slate), Some(Incomplete::WorkLimit));
    assert_eq!(emitted(&slate), 2);
}

#[test]
fn a_requested_stop_after_the_filling_emission_is_not_recorded() {
    let driver = driver(tautologies(0), 2);
    let slate = filled(&driver);
    // An idle producer polls control after the emission that filled the
    // slate. The request stays set, so the next slate's first poll sees it.
    slate.stop_unless_full(Incomplete::Cancelled);
    assert!(stopped(&slate).is_none());
}

#[test]
fn a_requested_stop_before_the_slate_fills_ends_it_with_that_stop() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    slate.stop_unless_full(Incomplete::Cancelled);
    assert_eq!(stopped(&slate), Some(Incomplete::Cancelled));
    assert!(slate.frontier.termination.is_closed());
}

#[test]
fn a_split_keeps_the_newer_child_in_hand_and_publishes_the_older() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let hand = slate
        .copy_and_hand_over(0, &mut Some(root), atom, reserve_regions)
        .unwrap();
    assert!(hand.region.is_held(0));
    assert!(slate.frontier.take_local(0).unwrap().region.is_cut(0));
    assert_eq!(slate.frontier.termination.unresolved(), 2);
    assert_eq!(hands(&slate), 1, "the newer child kept the parent's hand");
}

/// After a refutation or an emission, the region's resolution, the producer
/// keeps its hand while it finds work.
#[test]
fn a_resolution_keeps_its_hand_only_while_work_can_be_found() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(2);
    let lease = driver.allowance.lease(&driver.cancellation);
    let _resolved = slate.admit(0, Attempt::Outside, &lease).unwrap();
    // Local work: the producer takes its own newest region.
    slate.frontier.requeue(0, open(1), reserve_regions).unwrap();
    assert!(slate.after_resolution(0, &lease).is_some());
    assert_eq!(hands(&slate), 1);
    slate.frontier.requeue(1, open(1), reserve_regions).unwrap();
    // An empty own deque: the producer steals its peer's region.
    assert!(slate.after_resolution(0, &lease).is_some());
    assert_eq!(hands(&slate), 1);
    // Nothing anywhere: the hand is released.
    assert!(slate.after_resolution(0, &lease).is_none());
    assert_eq!(hands(&slate), 0);
}

#[test]
fn a_producer_whose_deque_is_busy_under_the_gate_does_not_steal() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    slate.frontier.requeue(1, open(1), reserve_regions).unwrap();
    let lease = driver.allowance.lease(&driver.cancellation);
    // A thief holds producer 0's deque, which still holds the root.
    let thief = slate.frontier.queue(0);
    assert!(slate.admit(0, Attempt::UnderGate, &lease).is_none());
    drop(thief);
    assert_eq!(
        slate.frontier.queue(1).len(),
        1,
        "the peer's region was stolen"
    );
}

#[test]
fn a_busy_attempt_under_the_gate_releases_its_hand() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let thief = slate.frontier.queue(0);
    assert!(slate.admit(0, Attempt::UnderGate, &lease).is_none());
    drop(thief);
    assert_eq!(hands(&slate), 0);
}

/// Once the output is spoken for, a resolution takes no further region, even
/// with one in the producer's own deque, and releases its hand.
#[test]
fn a_resolution_takes_nothing_once_the_output_is_spoken_for() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let _resolved = slate.admit(0, Attempt::Outside, &lease).unwrap();
    slate.frontier.requeue(0, open(1), reserve_regions).unwrap();
    assert!(slate.reserve_output_slot());
    assert!(slate.after_resolution(0, &lease).is_none());
    assert_eq!(slate.frontier.queue(0).len(), 1);
    assert_eq!(hands(&slate), 0);
}

#[test]
fn admission_takes_nothing_once_the_output_is_spoken_for() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    assert!(slate.reserve_output_slot());
    let lease = driver.allowance.lease(&driver.cancellation);
    assert!(slate.admit(0, Attempt::Outside, &lease).is_none());
    assert_eq!(slate.frontier.queue(0).len(), 1);
    assert_eq!(hands(&slate), 0);
}

/// A refused return commits the typed terminal stop before abandoning its
/// entry. The unresolved count is not a proof of exhausted coverage.
#[test]
fn a_return_refused_entry_capacity_records_an_allocation_stop() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let root = slate.frontier.take_local(0).unwrap();
    slate.give_back(0, root, |queue, _| reserve_regions(queue, usize::MAX));
    assert_eq!(stopped(&slate), Some(Incomplete::Allocation));
    assert!(slate.frontier.termination.is_closed());
    assert!(!slate.frontier.termination.is_resolved());
    assert_eq!(slate.frontier.termination.unresolved(), 1);
    assert!(slate.frontier.queue(0).is_empty());
}

#[test]
fn a_refused_publication_returns_the_unsplit_parent() {
    let driver = driver(tautologies(2), 2);
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    root.prefer(0);
    let interval = root.region.clone();
    let knowledge_before = format!("{:?}", root.state);
    let receipts = slate.frontier.statistics();
    // Guaranteed capacity overflow exercises the actual typed reservation.
    let mut pending = Some(root);
    let error = slate
        .copy_and_hand_over(0, &mut pending, atom, |queue, _| {
            reserve_regions(queue, usize::MAX)
        })
        .unwrap_err();
    let parent = pending.take().expect("refusal retains the whole parent");
    assert_eq!(error, Incomplete::Allocation);
    assert_eq!(parent.region, interval);
    assert_eq!(parent.split_atom(), Some(0));
    assert_eq!(format!("{:?}", parent.state), knowledge_before);
    assert_eq!(slate.frontier.statistics(), receipts);
    assert_eq!(slate.frontier.termination.unresolved(), 1);
    assert!(slate.frontier.queue(0).is_empty());
    assert_eq!(hands(&slate), 1);
    assert!(stopped(&slate).is_none());
    assert!(!slate.frontier.termination.is_closed());
    // The unchanged all-open parent still covers all four interpretations.
    assert_eq!(parent.region.open().count(), 2);
    // Production commits the stop while owning this parent, then abandons it.
    slate.stop(error);
    drop(parent);
    assert_eq!(stopped(&slate), Some(Incomplete::Allocation));
    assert!(slate.frontier.termination.is_closed());
    assert!(!slate.frontier.termination.is_resolved());
}

#[test]
fn a_region_taken_from_a_closed_slate_is_returned_unstepped() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let root = slate.frontier.take_local(0).unwrap();
    slate.stop(Incomplete::WorkLimit);
    assert!(slate.ready(root).is_break());
}

/// A protocol harness for a failed step: a peer spinning on steals must find
/// the region the failing producer returns only after the stop has closed the
/// slate, so the check after its take refuses it. Returning the region before
/// the stop lets the peer take it from an open slate and step it again.
#[test]
fn a_failed_region_is_taken_only_from_a_closed_slate() {
    const ROUNDS: usize = 1000;
    let mut driver = driver(tautologies(1), 2);
    let mut early = 0;
    for _ in 0..ROUNDS {
        // Each round starts open, with the root as the one region.
        driver.proposals.frontier.termination = Termination::resuming(1);
        let slate = driver.slate(1);
        let root = slate.frontier.take_local(0).unwrap();
        let spinning = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let peer = scope.spawn(|| {
                spinning.store(true, Ordering::Release);
                loop {
                    if let Some(entry) = slate.frontier.steal(1) {
                        return (entry, slate.frontier.termination.is_closed());
                    }
                    std::hint::spin_loop();
                }
            });
            // The peer is spinning before the region is returned.
            while !spinning.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            slate.fail(0, root, Incomplete::WorkLimit);
            let (entry, closed) = peer.join().unwrap();
            if !closed {
                early += 1;
            }
            slate.frontier.requeue(0, entry, reserve_regions).unwrap();
        });
    }
    assert_eq!(
        early, 0,
        "rounds whose peer took the region from an open slate"
    );
}

#[test]
fn a_stop_keeps_emitted_candidates() {
    let driver = driver(tautologies(0), 2);
    let slate = driver.slate(2);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Candidate(candidate) = step(&slate, &mut root) else {
        panic!("an atomless region is a leaf");
    };
    slate.emit(candidate);
    slate.stop(Incomplete::WorkLimit);
    assert_eq!(emitted(&slate), 1);
}

#[test]
fn a_producer_leaving_a_stopped_slate_returns_its_hand() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(3);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let _first = slate
        .copy_and_hand_over(0, &mut Some(root), atom, reserve_regions)
        .unwrap();
    // The peer steals the published child, and a stop ends the slate.
    let second = slate.admit(1, Attempt::Outside, &lease).unwrap();
    slate.stop(Incomplete::WorkLimit);
    slate.leave(1, Some(second));
    assert_eq!(slate.frontier.queue(1).len(), 1);
    assert_eq!(slate.frontier.termination.unresolved(), 2);
}

#[test]
fn a_shared_order_publishes_every_split_on_the_shared_deque() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate_in(2, FrontierOrder::Shared);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(1, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let _hand = slate
        .copy_and_hand_over(1, &mut Some(root), atom, reserve_regions)
        .unwrap();
    assert_eq!(slate.frontier.queue(0).len(), 1);
    assert!(slate.frontier.queue(1).is_empty());
}

#[test]
fn a_shared_order_resumes_with_the_region_published_last() {
    let driver = driver(tautologies(2), 2);
    let slate = driver.slate_in(3, FrontierOrder::Shared);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let mut first = slate
        .copy_and_hand_over(0, &mut Some(root), atom, reserve_regions)
        .unwrap();
    // The peer takes the published child and publishes a newer region.
    let mut second = slate.admit(1, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut second) else {
        panic!("a region with an open atom splits");
    };
    let _peer = slate
        .copy_and_hand_over(1, &mut Some(second), atom, reserve_regions)
        .unwrap();
    let Step::Split(atom) = step(&slate, &mut first) else {
        panic!("a region with an open atom splits");
    };
    let _own = slate
        .copy_and_hand_over(0, &mut Some(first), atom, reserve_regions)
        .unwrap();
    let published_last = slate.frontier.queue(0).back().unwrap().region.clone();
    // Producer 1 published its region before producer 0 published this one,
    // so after a refutation producer 1 takes producer 0's.
    let next = slate.after_resolution(1, &lease).unwrap();
    assert_eq!(next.region, published_last);
}

#[test]
fn a_per_producer_order_publishes_each_split_on_its_producers_deque() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(2);
    let lease = driver.allowance.lease(&driver.cancellation);
    // Producer 1's deque is empty, so it steals the root from deque 0.
    let mut root = slate.admit(1, Attempt::Outside, &lease).unwrap();
    let Step::Split(atom) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let _hand = slate
        .copy_and_hand_over(1, &mut Some(root), atom, reserve_regions)
        .unwrap();
    assert_eq!(slate.frontier.queue(1).len(), 1);
    assert!(slate.frontier.queue(0).is_empty());
}

fn two_producers(theory: &Theory) -> StableModels {
    StableModels::with_region_producers(
        theory,
        nonzero(2),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap()
}

#[test]
fn producers_keep_their_own_deques_unless_bounds_are_expected() {
    let search = two_producers(&tautologies(2));
    assert_eq!(
        search.statistics().regions.unwrap().frontier_order,
        Some(FrontierOrder::PerProducer)
    );
    assert_eq!(Driver::of_stream(search).first_publication(1, 2), 1);
}

#[test]
fn expecting_candidate_bounds_shares_the_producers_frontier() {
    let mut search = two_producers(&tautologies(2));
    search.expect_candidate_bounds().unwrap();
    assert_eq!(
        search.statistics().regions.unwrap().frontier_order,
        Some(FrontierOrder::Shared)
    );
    assert_eq!(Driver::of_stream(search).first_publication(1, 2), 0);
}

#[test]
fn a_started_stream_refuses_candidate_bounds_and_keeps_its_order() {
    let mut search = two_producers(&tautologies(4));
    let limits = crate::BatchLimits {
        max_candidates: nonzero(1),
        max_pending_bytes: 4096,
    };
    let models = search
        .next_batch(limits, |_, candidates| {
            Ok::<_, std::convert::Infallible>(vec![crate::BatchVerdict::Residual; candidates.len()])
        })
        .unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(
        search.expect_candidate_bounds(),
        Err(Incomplete::LateCandidateBounds)
    );
    assert_eq!(
        search.statistics().regions.unwrap().frontier_order,
        Some(FrontierOrder::PerProducer)
    );
    assert_eq!(Driver::of_stream(search).first_publication(1, 2), 1);
}

#[test]
fn a_closed_stream_refuses_candidate_bounds() {
    let mut search = two_producers(&tautologies(2));
    search.stop().unwrap();
    assert_eq!(
        search.expect_candidate_bounds(),
        Err(Incomplete::ClosedEnumerator)
    );
}

#[test]
fn an_idle_producer_holds_no_work_permits() {
    let driver = driver(tautologies(1), 1);
    let slate = driver.slate(1);
    // The only region is held elsewhere, so no attempt can find one.
    let _held = slate.frontier.take_local(0).unwrap();
    let lease = driver.allowance.lease(&driver.cancellation);
    lease.tick().unwrap();
    assert!(slate.admit(0, Attempt::Outside, &lease).is_none());
    // Every work permit but the one spent is available again.
    driver
        .allowance
        .charge(SearchLimits::default().max_work - 1)
        .unwrap();
}

#[test]
fn the_resolution_to_zero_ends_the_slate_with_exhaustion() {
    let theory = falsum();
    let mut proposals = proposals(&theory, 2);
    let (produced, output) = fill(&mut proposals, &theory, 4);
    assert!(produced.stopped.is_none());
    assert!(produced.exhausted);
    assert!(output.is_empty());
}

#[test]
fn regions_left_at_a_full_slates_end_supply_the_next() {
    let theory = tautologies(3);
    let mut proposals = proposals(&theory, 4);
    let mut found = std::collections::BTreeSet::new();
    let mut slates = 0;
    loop {
        let (produced, output) = fill(&mut proposals, &theory, 1);
        assert!(produced.stopped.is_none());
        slates += 1;
        for candidate in output {
            assert!(found.insert(candidate.atoms().collect::<Vec<_>>()));
        }
        if produced.exhausted {
            break;
        }
    }
    assert_eq!(found.len(), 8);
    assert!(slates >= 8);
}

/// Panics on any region with a decided atom: the root's step completes, and
/// the next step of a single producer panics.
#[derive(Debug)]
struct PanicOnDecided;

struct PanicOnDecidedWorker;

impl InjectedSource for PanicOnDecided {
    fn worker(
        &self,
        _: &Theory,
        _: &Cancellation,
    ) -> Result<Box<dyn InjectedWorker + '_>, Incomplete> {
        Ok(Box::new(PanicOnDecidedWorker))
    }
}

impl InjectedWorker for PanicOnDecidedWorker {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        _: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        assert!(
            (0..theory.atom_count()).all(|atom| region.decision(atom).is_none()),
            "injected producer failure"
        );
        Ok(RegionFeasibility::NotRefuted)
    }
}

#[test]
fn a_panicking_producers_counts_reach_the_merged_receipts() {
    let theory = tautologies(2);
    let mut proposals = proposals(&theory, 1);
    proposals.set_filter(crate::region_filter::Filter::new(
        crate::region_filter::Source::Fixture(Arc::new(PanicOnDecided)),
    ));
    let before = proposals.statistics().counts.regions;
    let (produced, output) = fill(&mut proposals, &theory, 4);
    assert_eq!(produced.stopped, Some(Incomplete::WorkerPanicked));
    assert!(output.is_empty());
    assert_eq!(proposals.statistics().counts.regions, before + 1);
}

#[derive(Debug)]
struct PanickingFilter {
    dropped: AtomicUsize,
    fail_check: bool,
}

struct PanickingWorker<'a>(&'a PanickingFilter);

impl Drop for PanickingWorker<'_> {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
        panic!("injected checker teardown failure");
    }
}

impl InjectedSource for PanickingFilter {
    fn worker(
        &self,
        _: &Theory,
        _: &Cancellation,
    ) -> Result<Box<dyn InjectedWorker + '_>, Incomplete> {
        Ok(Box::new(PanickingWorker(self)))
    }
}

impl InjectedWorker for PanickingWorker<'_> {
    fn check(
        &mut self,
        _: &Theory,
        _: &Region,
        _: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        if self.0.fail_check {
            Err(Incomplete::RegionFilter)
        } else {
            Ok(RegionFeasibility::NotRefuted)
        }
    }
}

fn panicking_search(fail_check: bool) -> (StableModels, Arc<PanickingFilter>) {
    let theory = Theory::new(
        0,
        zetesis_ferraris::FormulaParts::default(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut search = StableModels::with_region_producers(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let filter = Arc::new(PanickingFilter {
        dropped: AtomicUsize::new(0),
        fail_check,
    });
    search.set_region_filter(filter.clone()).unwrap();
    (search, filter)
}

/// Prepare both workspaces directly to make simultaneous teardown obligations
/// independent of how many Rayon tasks a small candidate slate happens to use.
fn retain_panicking_checkers(search: &mut StableModels) {
    let crate::ferraris::Proposer::Proposals(proposals) = &mut search.proposer else {
        panic!("expected joined production");
    };
    proposals.workers.with_dependent_mut(|owner, workspaces| {
        let filter = owner.as_ref().unwrap();
        for _ in 0..2 {
            let mut workspace = Workspace::default();
            filter
                .check(
                    &mut workspace.filter,
                    &search.theory,
                    &Region::all_open(search.theory.atom_count()),
                    &search.cancellation,
                    &mut None,
                )
                .unwrap();
            workspaces.push(workspace);
        }
    });
}

#[test]
fn teardown_contains_each_checker_panic() {
    let (mut search, filter) = panicking_search(false);
    retain_panicking_checkers(&mut search);
    let receipt = search.statistics();
    assert_eq!(search.stop(), Err(Incomplete::WorkerPanicked));
    assert_eq!(filter.dropped.load(Ordering::SeqCst), 2);
    assert_eq!(search.statistics(), receipt);
    assert!(!search.exhausted());
    assert!(search.next().is_none());
    assert_eq!(search.stop(), Ok(()));
}

#[test]
fn iterator_drop_contains_checker_panics() {
    let (mut search, filter) = panicking_search(false);
    retain_panicking_checkers(&mut search);
    drop(search);
    assert_eq!(filter.dropped.load(Ordering::SeqCst), 2);
}

#[test]
fn source_failure_survives_checker_drop_panic() {
    let (mut search, filter) = panicking_search(true);
    assert!(matches!(search.next(), Some(Err(Incomplete::RegionFilter))));
    assert_eq!(filter.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(search.statistics().region_filter.unwrap().failed, 1);
    assert!(!search.exhausted());
    assert!(search.next().is_none());
}

#[test]
fn proposals_split_on_the_original_choice_head_first() {
    // One producer walks depth first and publishes each split's cut sibling on
    // its deque. Splitting the root on the choice head decides it in every
    // pending region; splitting on a loop atom first would leave the root's
    // sibling with the choice open.
    let theory = crate::ferraris::fixtures::choice_beside_a_loop();
    let mut proposals = proposals(&theory, 1);
    let (_, output) = fill(&mut proposals, &theory, 1);
    assert_eq!(output.len(), 1);
    let pending = proposals.frontier.queue(0);
    assert!(!pending.is_empty());
    assert!(pending.iter().all(|entry| !entry.region.is_open(0)));
}

/// `a0 <- a2`, `a1 <- a0`, `a2 <- a1` beside two atomic choices: the root's
/// pass captures the loop's absence, and every later region inherits the cut,
/// so no producer captures it again. The loop has three atoms because the
/// support cut alone decides a loop of two.
fn loop_beside_two_choices() -> Theory {
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::falsum(),
        Node::implies(2, 0),
        Node::implies(0, 1),
        Node::implies(1, 2),
    ];
    let mut roots = vec![4, 5, 6];
    for choice in 0..2 {
        let index = nodes.len();
        nodes.extend([
            Node::atom(3 + choice),
            Node::implies(index, 3),
            Node::or_pair([index, index + 1]),
        ]);
        roots.push(index + 2);
    }
    Theory::new(
        5,
        zetesis_ferraris::FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn every_producer_draws_a_joined_peers_rows_at_the_next_slate() {
    let theory = loop_beside_two_choices();
    let mut proposals = proposals(&theory, 2);
    proposals.set_learned_store(Arc::new(crate::LearnedStore::new()));
    for _ in 0..2 {
        let (produced, candidates) = fill(&mut proposals, &theory, 1);
        assert_eq!(produced.stopped, None);
        assert_eq!(candidates.len(), 1);
    }
    // One producer captured the row in the first slate and delivered it at
    // the join; the second slate's start seeded both tiers.
    let held = proposals.workers.with_dependent_mut(|_, workspaces| {
        workspaces
            .iter()
            .map(|workspace| workspace.scratch.learned_rows())
            .collect::<Vec<_>>()
    });
    assert_eq!(held, [1, 1]);
}

mod publication;

mod arithmetic_receipts;
