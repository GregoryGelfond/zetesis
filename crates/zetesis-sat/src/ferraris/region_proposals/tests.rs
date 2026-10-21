use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    Attempt, CandidateKnowledge, Entry, Mutex, Outcome, RegionProposals, SlateFill, Step,
    Workspace, reserve_regions,
};
use crate::search::{Budget, LocalQuota, SharedBudget};
use crate::{
    Cancellation, Incomplete, Limits, RegionFeasibility, RegionFilter, RegionFilterWorker,
    SearchLimits, SearchStatistics, StableModels,
};
use zetesis_ferraris::{AdmissionLimits, Interpretation, Narrower, Node, Region, Theory};

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
    RegionProposals::new(theory, nonzero(producers), u64::MAX, u64::MAX, &mut budget).unwrap()
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
    fn slate(&self, maximum: usize) -> SlateFill<'_, '_> {
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
            output_slots: AtomicUsize::new(0),
            outcome: Mutex::new(Outcome {
                candidates: Vec::with_capacity(maximum),
                stopped: None,
            }),
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

fn open(atoms: usize) -> Entry {
    (Region::all_open(atoms), CandidateKnowledge::default())
}

fn slots(slate: &SlateFill<'_, '_>) -> usize {
    slate.output_slots.load(Ordering::Acquire)
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
fn admission_never_exceeds_the_slates_output_slots() {
    let driver = driver(tautologies(3), 3);
    let slate = driver.slate(2);
    for index in 1..3 {
        slate.frontier.requeue(index, open(3));
    }
    let lease = driver.allowance.lease(&driver.cancellation);
    let admitted = (0..3)
        .filter_map(|index| slate.admit(index, Attempt::Outside, &lease))
        .count();
    assert_eq!(admitted, 2);
    assert_eq!(slots(&slate), 2);
}

#[test]
fn the_emission_that_fills_the_slate_closes_it_without_a_stop() {
    let driver = driver(tautologies(0), 2);
    let slate = filled(&driver);
    assert!(slate.frontier.termination.is_closed());
    assert!(stopped(&slate).is_none());
    // Every output slot is a candidate's, so no producer holds a region.
    assert_eq!(slots(&slate), emitted(&slate));
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
    let Step::Split(cut, held) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let hand = slate.hand_over(0, (cut, held), reserve_regions).unwrap();
    assert!(hand.0.is_held(0));
    assert!(slate.frontier.take_local(0).unwrap().0.is_cut(0));
    assert_eq!(slate.frontier.termination.unresolved(), 2);
    assert_eq!(slots(&slate), 1, "the hand kept the parent's output slot");
}

#[test]
fn a_refutation_keeps_its_slot_only_while_work_can_be_found() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(2);
    let lease = driver.allowance.lease(&driver.cancellation);
    let _refuted = slate.admit(0, Attempt::Outside, &lease).unwrap();
    slate.frontier.requeue(1, open(1));
    // An empty own deque: the producer steals its peer's region.
    assert!(slate.after_refutation(0, &lease).is_some());
    assert_eq!(slots(&slate), 1);
    // Nothing anywhere: the slot is released.
    assert!(slate.after_refutation(0, &lease).is_none());
    assert_eq!(slots(&slate), 0);
}

#[test]
fn a_producer_whose_deque_is_busy_under_the_gate_does_not_steal() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    slate.frontier.requeue(1, open(1));
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
fn a_busy_attempt_under_the_gate_releases_its_output_slot() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let thief = slate.frontier.queue(0);
    assert!(slate.admit(0, Attempt::UnderGate, &lease).is_none());
    drop(thief);
    assert_eq!(slots(&slate), 0);
}

#[test]
fn a_refused_publication_requeues_the_held_child_and_leaves_the_count() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Split(cut, held) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    // Exercise the actual reservation and error conversion, using guaranteed
    // capacity overflow rather than attempting host OOM.
    let refused = slate.hand_over(0, (cut, held), |queue, _| {
        reserve_regions(queue, usize::MAX)
    });
    assert!(refused.is_none());
    assert_eq!(stopped(&slate), Some(Incomplete::Allocation));
    assert!(slate.frontier.termination.is_closed());
    assert_eq!(slate.frontier.termination.unresolved(), 1);
    assert!(slate.frontier.take_local(0).unwrap().0.is_held(0));
}

#[test]
fn a_stop_keeps_emitted_candidates_and_every_returned_hand() {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(3);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut root = slate.admit(0, Attempt::Outside, &lease).unwrap();
    let Step::Split(cut, held) = step(&slate, &mut root) else {
        panic!("a tautology's root splits");
    };
    let mut first = slate.hand_over(0, (cut, held), reserve_regions).unwrap();
    // The peer steals the published child; the owner's hand becomes a leaf.
    let second = slate.admit(1, Attempt::Outside, &lease).unwrap();
    let Step::Candidate(candidate) = step(&slate, &mut first) else {
        panic!("a fully decided region is a leaf");
    };
    slate.emit(candidate);
    slate.stop(Incomplete::WorkLimit);
    // Leaving a closed slate, the peer returns its hand.
    slate.frontier.requeue(1, second);
    assert_eq!(stopped(&slate), Some(Incomplete::WorkLimit));
    assert_eq!(emitted(&slate), 1);
    assert_eq!(slate.frontier.termination.unresolved(), 1);
    assert_eq!(slate.frontier.queue(1).len(), 1);
}

#[test]
fn an_idle_producer_holds_no_permits() {
    let driver = driver(tautologies(1), 1);
    let slate = driver.slate(1);
    // The only region is held elsewhere, so no attempt can find one.
    let _held = slate.frontier.take_local(0).unwrap();
    let lease = driver.allowance.lease(&driver.cancellation);
    lease.tick().unwrap();
    assert!(slate.admit(0, Attempt::Outside, &lease).is_none());
    // Every permit but the one spent is available again.
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

impl RegionFilter for PanicOnDecided {
    fn worker(
        &self,
        _: &Theory,
        _: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
        Ok(Box::new(PanicOnDecidedWorker))
    }
}

impl RegionFilterWorker for PanicOnDecidedWorker {
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
    proposals.set_filter(crate::region_filter::Filter::new(Arc::new(PanicOnDecided)));
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

impl RegionFilter for PanickingFilter {
    fn worker(
        &self,
        _: &Theory,
        _: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
        Ok(Box::new(PanickingWorker(self)))
    }
}

impl RegionFilterWorker for PanickingWorker<'_> {
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
    assert!(pending.iter().all(|(region, _)| !region.is_open(0)));
}
