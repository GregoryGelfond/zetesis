//! The coordinator phase, engagement and the walk's record.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicUsize;

use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node};

use super::*;

/// A delay no test reaches by the clock: a walk under it engages only when
/// its deadline test says so.
const LATE: Engagement = Engagement::After(Duration::from_hours(1));
const NEVER: Engagement = Engagement::After(Duration::MAX);
const WAIT: Duration = Duration::from_secs(2);

/// The positive loop `a0 <- a1. a1 <- a0.` beside `choices` atomic choices:
/// with the loop true, every leaf is a classical model the reduct refutes,
/// and the walk interleaves those leaves with the stable models.
fn loop_beside_choices(choices: usize) -> Theory {
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::falsum(),
        Node::implies(1, 0),
        Node::implies(0, 1),
    ];
    let mut roots = vec![3, 4];
    for choice in 0..choices {
        let index = nodes.len();
        nodes.extend([
            Node::atom(2 + choice),
            Node::implies(index, 2),
            Node::or_pair([index, index + 1]),
        ]);
        roots.push(index + 2);
    }
    Theory::new(
        2 + choices,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// `loops` independent positive loops `a <- b. b <- a.`: a positive program
/// whose one stable model is empty, and whose every other leaf is a
/// classical model with a loop held true.
fn positive_loops(loops: usize) -> Theory {
    let mut nodes = Vec::new();
    let mut roots = Vec::new();
    for pair in 0..loops {
        let index = nodes.len();
        nodes.extend([
            Node::atom(2 * pair),
            Node::atom(2 * pair + 1),
            Node::implies(index + 1, index),
            Node::implies(index, index + 1),
        ]);
        roots.extend([index + 2, index + 3]);
    }
    Theory::new(
        2 * loops,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// `n` independent tautologies `a | not a`: every leaf is a stable model.
fn tautologies(n: usize) -> Theory {
    let mut nodes = vec![Node::falsum()];
    let mut roots = Vec::new();
    for atom in 0..n {
        let index = nodes.len();
        nodes.extend([
            Node::atom(atom),
            Node::implies(index, 0),
            Node::or_pair([index, index + 1]),
        ]);
        roots.push(index + 2);
    }
    Theory::new(
        n,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The positive-program certificate: a leaf is stable exactly when it is the
/// program's least model.
fn positive(theory: &Theory) -> Arc<Certification> {
    let plan = zetesis_ferraris::PositivePlan::compile(
        theory,
        zetesis_ferraris::PositivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    Arc::new(Certification::Positive {
        plan,
        max_bytes: usize::MAX,
    })
}

fn tight(theory: &Theory) -> Arc<Certification> {
    let plan = zetesis_ferraris::TightPlan::compile(
        theory,
        zetesis_ferraris::TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    Arc::new(Certification::Tight {
        plan: Arc::new(plan),
        max_bytes: u64::MAX,
    })
}

/// A walk over `theory` with `workers`, started as `engagement` says, under
/// `limits`, and the original index the enumeration would hand it.
fn walk_with(
    theory: &Theory,
    workers: usize,
    engagement: Engagement,
    limits: Limits,
) -> (ParallelRegions, Arc<IndexedTheory>) {
    let cancellation = Cancellation::with_deadline(Instant::now() + 30 * WAIT).unwrap();
    let mut budget = Budget {
        quota: crate::search::LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let search = ParallelRegions::new(
        theory,
        NonZeroUsize::new(workers).unwrap(),
        engagement,
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap();
    (search, super::original(theory))
}

fn walk(theory: &Theory, engagement: Engagement) -> (ParallelRegions, Arc<IndexedTheory>) {
    walk_with(theory, 2, engagement, Limits::default())
}

/// The caller's budget, as the enumeration holds it.
fn caller<'c>(search: &ParallelRegions, cancellation: &'c Cancellation) -> Budget<'c> {
    Budget {
        quota: crate::search::LocalQuota,
        limits: search.shared.limits.search,
        cancellation,
        statistics: search.search_statistics(),
    }
}

fn atoms(model: &Interpretation) -> Vec<usize> {
    model.atoms().collect()
}

/// Pull every remaining model with the clock's deadline test.
fn drain(
    search: &mut ParallelRegions,
    index: &Arc<IndexedTheory>,
    certificate: Option<&Arc<Certification>>,
    budget: &mut Budget<'_>,
) -> Vec<Vec<usize>> {
    let mut models = Vec::new();
    while let Some(model) = search.propose(index, certificate, false, budget).unwrap() {
        models.push(atoms(&model));
    }
    models
}

fn outstanding(search: &ParallelRegions) -> usize {
    search
        .shared
        .termination
        .outstanding
        .load(Ordering::Acquire)
}

fn walking(search: &ParallelRegions) -> Duration {
    match search.record {
        WalkRecord::Unengaged { walking, .. } | WalkRecord::Engaged { walking, .. } => walking,
        WalkRecord::NotBegun => Duration::ZERO,
    }
}

fn frontier_released(search: &ParallelRegions) -> bool {
    search.shared.queues.iter().all(|queue| {
        let queue = queue.lock().unwrap();
        queue.is_empty() && queue.capacity() == 0
    })
}

/// What a walk returned and the receipts that do not depend on the schedule.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    family: BTreeSet<Vec<usize>>,
    regions: usize,
    refuted: usize,
    leaves: usize,
    work: u64,
    candidates: u64,
    countermodel_queries: u64,
    countermodels: u64,
    reduct: crate::ReductStatistics,
    certificate_checks: Option<(u64, u64, u64)>,
}

fn outcome(search: &ParallelRegions, models: &[Vec<usize>]) -> Outcome {
    let family: BTreeSet<Vec<usize>> = models.iter().cloned().collect();
    assert_eq!(family.len(), models.len(), "each model once");
    let counts = search.statistics().counts;
    let merged = search.merged();
    Outcome {
        family,
        regions: counts.regions,
        refuted: counts.refuted,
        leaves: counts.leaves,
        work: counts.work,
        candidates: merged.candidates,
        countermodel_queries: merged.countermodel_queries,
        countermodels: merged.countermodels,
        reduct: merged.reduct,
        certificate_checks: merged
            .certified
            .map(|checked| (checked.checks, checked.stable, checked.refuted)),
    }
}

/// The reference: the walk that never engages, which steps the scalar walk's
/// order on the calling thread.
fn unengaged(theory: &Theory, certificate: Option<&Arc<Certification>>) -> Outcome {
    let (mut search, index) = walk(theory, NEVER);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    let models = drain(&mut search, &index, certificate, &mut budget);
    assert!(search.exhausted);
    assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
    outcome(&search, &models)
}

/// A filter that passes every region, counting its live checkers.
#[derive(Debug)]
struct Pass {
    theory: Theory,
    live: Arc<AtomicUsize>,
}

struct PassWorker(Arc<AtomicUsize>);

impl crate::RegionFilter for Pass {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn crate::RegionFilterWorker + '_>, Incomplete> {
        cancellation.poll()?;
        if !theory.same_instance(&self.theory) {
            return Err(Incomplete::WrongTheory);
        }
        self.live.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(PassWorker(Arc::clone(&self.live))))
    }
}

impl crate::RegionFilterWorker for PassWorker {
    fn check(
        &mut self,
        _: &Theory,
        _: &Region,
        cancellation: &Cancellation,
    ) -> Result<crate::RegionFeasibility, Incomplete> {
        cancellation.poll()?;
        Ok(crate::RegionFeasibility::NotRefuted)
    }
}

impl Drop for PassWorker {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A filter whose checker panics on drop, or whose check panics once it has
/// checked `panic_at` regions (never while `panic_at` is zero).
#[derive(Debug)]
struct Faulty {
    theory: Theory,
    panic_on_drop: bool,
    panic_at: Arc<AtomicUsize>,
}

struct FaultyWorker {
    panic_on_drop: bool,
    panic_at: Arc<AtomicUsize>,
    checked: usize,
}

impl crate::RegionFilter for Faulty {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn crate::RegionFilterWorker + '_>, Incomplete> {
        cancellation.poll()?;
        if !theory.same_instance(&self.theory) {
            return Err(Incomplete::WrongTheory);
        }
        Ok(Box::new(FaultyWorker {
            panic_on_drop: self.panic_on_drop,
            panic_at: Arc::clone(&self.panic_at),
            checked: 0,
        }))
    }
}

impl crate::RegionFilterWorker for FaultyWorker {
    fn check(
        &mut self,
        _: &Theory,
        _: &Region,
        cancellation: &Cancellation,
    ) -> Result<crate::RegionFeasibility, Incomplete> {
        cancellation.poll()?;
        self.checked += 1;
        assert_ne!(
            self.panic_at.load(Ordering::SeqCst),
            self.checked,
            "injected panic in a region check"
        );
        Ok(crate::RegionFeasibility::NotRefuted)
    }
}

impl Drop for FaultyWorker {
    fn drop(&mut self) {
        assert!(
            !self.panic_on_drop,
            "injected panic in a checker's teardown"
        );
    }
}

/// Begin a walk, pull `skip` models with a deadline that never passes, then
/// pull with a deadline test that answers once the pull has stepped (and,
/// when `refuted` asks for it, decided a leaf without a model, which the
/// reduct refuted) and deque 0 holds more than one region, until the walk
/// engages; then pull to exhaustion. The test is asked on entry to each
/// coordinator iteration and after each step that returned no model, so the
/// engaging pull holds that step's lease and unmerged receipts when it
/// engages.
fn handoff(
    theory: &Theory,
    certificate: Option<&Arc<Certification>>,
    filter: Option<&Arc<dyn crate::RegionFilter>>,
    refuted: bool,
    skip: usize,
) -> (ParallelRegions, Vec<Vec<usize>>) {
    let (mut search, index) = walk(theory, LATE);
    if let Some(filter) = filter {
        search
            .set_filter(crate::region_filter::Filter::new(Arc::clone(filter)))
            .unwrap();
    }
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    let mut models = Vec::new();
    search.begin(certificate, false, &mut budget).unwrap();
    for _ in 0..skip {
        let model = search
            .propose_with(&index, certificate, false, &mut budget, &mut |_| false)
            .unwrap()
            .expect("a model before the engaging pull");
        models.push(atoms(&model));
    }
    let shared = Arc::clone(&search.shared);
    let frontier = Cell::new(None);
    let walked = loop {
        let walked = walking(&search);
        let mut entry = None;
        let model = search
            .propose_with(&index, certificate, false, &mut budget, &mut |_| {
                let now = (
                    Live::read(&shared.live.regions),
                    Live::read(&shared.live.leaves),
                );
                let (regions, leaves) = *entry.get_or_insert(now);
                let passed =
                    now.0 > regions && (!refuted || now.1 > leaves) && shared.queue(0).len() > 1;
                if passed {
                    frontier.set(Some(shared.queue(0).len()));
                }
                passed
            })
            .unwrap();
        if matches!(search.record, WalkRecord::Engaged { .. }) {
            models.extend(model.as_ref().map(atoms));
            break walked;
        }
        models.push(atoms(&model.expect("the walk engages before it ends")));
    };
    drop(shared);
    assert!(
        matches!(search.record, WalkRecord::Engaged { launched: 2, .. }),
        "{:?}",
        search.record
    );
    assert!(
        frontier.get().expect("the deadline passed during the pull") > 1,
        "deque 0 held more than one region at engagement"
    );
    assert!(
        walking(&search) > walked,
        "the engaging pull's steps entered the record"
    );
    models.extend(drain(&mut search, &index, certificate, &mut budget));
    assert!(search.exhausted);
    assert_eq!(outstanding(&search), 0);
    // The workers have joined: the caller's counters are the shared
    // allowance's spent totals, the engaging pull's coordinator part included.
    let mut spent = SearchStatistics::default();
    search.shared.budget.record(&mut spent);
    assert_eq!(
        (budget.statistics.work, budget.statistics.decisions),
        (spent.work, spent.decisions)
    );
    (search, models)
}

#[test]
fn the_deadline_can_pass_between_two_steps_of_a_pull() {
    // The engaging pull steps through a leaf the reduct refutes before the
    // deadline passes, so it holds unmerged membership receipts and a live
    // lease when it engages.
    let theory = loop_beside_choices(3);
    let expected = unengaged(&theory, None);
    assert!(expected.countermodels > 0, "the reduct refutes leaves");
    for skip in 0..4 {
        let (search, models) = handoff(&theory, None, None, true, skip);
        assert_eq!(outcome(&search, &models), expected, "after {skip} models");
    }
}

#[test]
fn a_handoff_keeps_the_certificate_checks_of_the_engaging_pull() {
    // The same with a certificate deciding the leaves: the engaging pull
    // holds the unmerged checks of leaves the certificate refuted.
    let theory = positive_loops(3);
    let certificate = positive(&theory);
    let expected = unengaged(&theory, Some(&certificate));
    assert_eq!(expected.certificate_checks, Some((8, 1, 7)));
    assert_eq!(expected.countermodel_queries, 0, "the certificate decides");
    let (search, models) = handoff(&theory, Some(&certificate), None, true, 0);
    assert_eq!(outcome(&search, &models), expected);
}

#[test]
fn a_handoff_counts_the_coordinators_tight_checks_once() {
    // Narrowing's support cut decides every leaf of a tight theory before
    // its certificate does, so each tight check is a model its pull returns:
    // the coordinator's pulls merge theirs before the handoff, and the
    // workers theirs after it.
    let theory = tautologies(4);
    let certificate = tight(&theory);
    let expected = unengaged(&theory, Some(&certificate));
    assert_eq!(expected.certificate_checks, Some((16, 16, 0)));
    for skip in 0..4 {
        let (search, models) = handoff(&theory, Some(&certificate), None, false, skip);
        assert_eq!(outcome(&search, &models), expected, "after {skip} models");
    }
}

#[test]
fn a_filtered_handoff_prepares_at_most_one_checker_more_than_its_workers() {
    let theory = loop_beside_choices(3);
    let live = Arc::new(AtomicUsize::new(0));
    let filter: Arc<dyn crate::RegionFilter> = Arc::new(Pass {
        theory: theory.clone(),
        live: Arc::clone(&live),
    });
    let expected = unengaged(&theory, None);
    let (search, models) = handoff(&theory, None, Some(&filter), true, 1);
    let preparations = search.filter().unwrap().statistics().preparations;
    assert!((1..=3).contains(&preparations), "{preparations}");
    assert_eq!(live.load(Ordering::SeqCst), 0);
    assert_eq!(
        models.iter().cloned().collect::<BTreeSet<_>>(),
        expected.family
    );
}

#[test]
fn a_walk_that_ended_on_the_calling_thread_never_engages() {
    // The last region in the walk's order is a model, so the pull returning it
    // leaves no region outstanding.
    let theory = tautologies(3);
    let (mut search, index) = walk(&theory, LATE);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    for _ in 0..8 {
        assert!(
            search
                .propose_with(&index, None, false, &mut budget, &mut |_| false)
                .unwrap()
                .is_some()
        );
    }
    assert_eq!(outstanding(&search), 0);
    let record = search.statistics().workers.unwrap().engagement;
    assert!(
        search
            .propose_with(&index, None, false, &mut budget, &mut |_| true)
            .unwrap()
            .is_none()
    );
    assert!(search.exhausted);
    assert_eq!(search.statistics().workers.unwrap().engagement, record);
    assert!(matches!(record, RegionWalkEngagement::Unengaged { .. }));
    assert!(search.handles.is_empty());
    assert!(search.coordinator.is_none());
}

/// A walk under `engagement` whose shared work ceiling stops it a few
/// regions after its construction.
fn ceiling(engagement: Engagement) -> (ParallelRegions, Arc<IndexedTheory>) {
    let theory = tautologies(6);
    let construction = walk(&theory, engagement).0.search_statistics().work;
    let limits = Limits {
        search: crate::SearchLimits {
            max_work: construction + 60,
            ..Default::default()
        },
        ..Limits::default()
    };
    walk_with(&theory, 2, engagement, limits)
}

#[test]
fn a_recorded_stop_is_returned_on_every_later_pull() {
    let (mut search, index) = ceiling(LATE);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    let stop = loop {
        match search.propose_with(&index, None, false, &mut budget, &mut |_| false) {
            Ok(Some(_)) => {}
            Ok(None) => panic!("the ceiling must stop the walk"),
            Err(error) => break error,
        }
    };
    assert_eq!(stop, Incomplete::WorkLimit);
    let counts = search.statistics().counts;
    for _ in 0..2 {
        assert_eq!(
            search
                .propose_with(&index, None, false, &mut budget, &mut |_| true)
                .err(),
            Some(stop)
        );
    }
    assert_eq!(search.statistics().counts, counts, "no step");
    assert!(search.handles.is_empty(), "no thread");
    assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
    assert!(!search.exhausted);
}

#[test]
fn a_closed_walk_returns_its_recorded_stop_in_both_states() {
    for engagement in [LATE, Engagement::Immediate] {
        let (mut search, index) = ceiling(engagement);
        let cancellation = search.shared.cancellation.clone();
        let mut budget = caller(&search, &cancellation);
        let stop = loop {
            match search.propose(&index, None, false, &mut budget) {
                Ok(Some(_)) => {}
                Ok(None) => panic!("the ceiling must stop the walk"),
                Err(error) => break error,
            }
        };
        assert_eq!(search.stop(), Err(stop));
        assert_eq!(
            search.propose(&index, None, false, &mut budget).err(),
            Some(stop),
            "{engagement:?}"
        );
        assert_state(&search, engagement);
    }
}

/// The state a walk under `engagement` is in once it has begun: the
/// coordinator's under a delay no test reaches, else engaged.
fn assert_state(search: &ParallelRegions, engagement: Engagement) {
    if engagement == Engagement::Immediate {
        assert!(
            matches!(search.record, WalkRecord::Engaged { launched: 2, .. }),
            "{:?}",
            search.record
        );
    } else {
        assert!(
            matches!(search.record, WalkRecord::Unengaged { .. }),
            "{:?}",
            search.record
        );
    }
}

#[test]
fn a_walk_closed_without_a_stop_answers_the_closed_enumerator_in_both_states() {
    for engagement in [LATE, Engagement::Immediate] {
        let (mut search, index) = walk(&tautologies(3), engagement);
        let cancellation = search.shared.cancellation.clone();
        let mut budget = caller(&search, &cancellation);
        search
            .propose_with(&index, None, false, &mut budget, &mut |_| false)
            .unwrap()
            .unwrap();
        search.stop().unwrap();
        let record = search.statistics().workers.unwrap();
        let counts = search.statistics().counts;
        assert_eq!(
            search
                .propose_with(&index, None, false, &mut budget, &mut |_| true)
                .err(),
            Some(Incomplete::ClosedEnumerator),
            "{engagement:?}"
        );
        assert_eq!(search.statistics().workers.unwrap(), record);
        assert_eq!(search.statistics().counts, counts, "no step");
        assert!(search.handles.is_empty());
        assert_state(&search, engagement);
    }
}

/// A spawn that starts a real worker for every slot below `refused`, and
/// refuses that slot.
fn refusing_at(
    refused: usize,
    index: &Arc<IndexedTheory>,
) -> impl FnMut(
    Arc<Shared>,
    usize,
    SyncSender<Interpretation>,
) -> std::io::Result<JoinHandle<Option<WorkerReport>>>
+ use<> {
    let index = Arc::clone(index);
    move |shared, slot, sender| {
        if slot == refused {
            return Err(std::io::Error::other("injected thread creation refusal"));
        }
        let original = Arc::clone(&index);
        std::thread::Builder::new()
            .spawn(move || contain_worker(&shared, || worker(&shared, &original, slot, &sender)))
    }
}

#[test]
fn a_refused_spawn_after_the_coordinator_phase_joins_the_launched_subset() {
    let (mut search, index) = walk(&tautologies(4), LATE);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    for _ in 0..2 {
        search
            .propose_with(&index, None, false, &mut budget, &mut |_| false)
            .unwrap()
            .unwrap();
    }
    let returned = search.merged().candidates;
    assert_eq!(
        search.engage_with(&mut budget, refusing_at(1, &index)),
        Err(Incomplete::Allocation)
    );
    assert!(search.handles.is_empty());
    assert!(frontier_released(&search));
    assert!(!search.exhausted);
    assert!(
        search.merged().candidates >= returned,
        "the returned models counted"
    );
    assert!(matches!(
        search.record,
        WalkRecord::Engaged { launched: 1, .. }
    ));
    assert_eq!(
        search.propose(&index, None, false, &mut budget).err(),
        Some(Incomplete::Allocation)
    );
}

#[test]
fn a_refused_first_spawn_records_no_launch() {
    let (mut search, index) = walk(&tautologies(4), LATE);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    search
        .propose_with(&index, None, false, &mut budget, &mut |_| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        search.engage_with(&mut budget, refusing_at(0, &index)),
        Err(Incomplete::Allocation)
    );
    assert!(search.handles.is_empty());
    assert!(matches!(
        search.record,
        WalkRecord::Engaged { launched: 0, .. }
    ));
}

#[test]
fn a_failed_release_at_engagement_starts_no_thread() {
    let theory = tautologies(3);
    let (mut search, index) = walk(&theory, LATE);
    search
        .set_filter(crate::region_filter::Filter::new(Arc::new(Faulty {
            theory: theory.clone(),
            panic_on_drop: true,
            panic_at: Arc::new(AtomicUsize::new(0)),
        })))
        .unwrap();
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    search
        .propose_with(&index, None, false, &mut budget, &mut |_| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        search
            .propose_with(&index, None, false, &mut budget, &mut |_| true)
            .err(),
        Some(Incomplete::WorkerPanicked)
    );
    assert!(search.handles.is_empty());
    assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
    assert!(!search.exhausted);
}

#[test]
fn a_panic_in_a_coordinator_step_merges_nothing_of_its_pull() {
    let theory = loop_beside_choices(3);
    let (mut search, index) = walk(&theory, LATE);
    let panic_at = Arc::new(AtomicUsize::new(0));
    search
        .set_filter(crate::region_filter::Filter::new(Arc::new(Faulty {
            theory: theory.clone(),
            panic_on_drop: false,
            panic_at: Arc::clone(&panic_at),
        })))
        .unwrap();
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    search
        .propose_with(&index, None, false, &mut budget, &mut |_| false)
        .unwrap()
        .unwrap();
    // The second pull decides a leaf the reduct refutes, whose receipts it
    // holds unmerged, and then panics in its next region check.
    let checks = search.filter().unwrap().statistics().checks;
    panic_at.store(usize::try_from(checks).unwrap() + 2, Ordering::SeqCst);
    let merged = search.merged();
    let leaves = Live::read(&search.shared.live.leaves);
    let walked = walking(&search);
    assert_eq!(
        search
            .propose_with(&index, None, false, &mut budget, &mut |_| false)
            .err(),
        Some(Incomplete::WorkerPanicked)
    );
    // The leaf reached the live counters as it was decided; the pull's
    // report, which holds its reduct receipts, merged nothing.
    assert!(Live::read(&search.shared.live.leaves) > leaves);
    let after = search.merged();
    assert!(after.countermodel_queries > merged.countermodel_queries);
    assert_eq!(after.reduct, merged.reduct);
    assert_eq!(after.certified, merged.certified);
    let mut spent = SearchStatistics::default();
    search.shared.budget.record(&mut spent);
    assert_eq!(budget.statistics.work, spent.work, "the pull was accounted");
    assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
    assert!(walking(&search) > walked);
    assert!(search.shared.termination.is_closed());
    assert!(!search.exhausted);
    assert!(frontier_released(&search));
    assert!(search.coordinator.is_none());
}

#[test]
fn a_failed_merge_at_a_pull_holding_a_model_stops_on_the_next_pull() {
    let theory = tautologies(2);
    let certificate = tight(&theory);
    let (mut search, index) = walk(&theory, NEVER);
    // At the counter's width: the next leaf's certificate check overflows it.
    search.merged.certified = Some(super::super::super::certified::CertifiedStatistics {
        checks: u64::MAX,
        ..Default::default()
    });
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    assert!(
        search
            .propose(&index, Some(&certificate), false, &mut budget)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        search
            .propose(&index, Some(&certificate), false, &mut budget)
            .err(),
        Some(Incomplete::CounterOverflow)
    );
    assert!(!search.exhausted);
    assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
}

#[test]
fn cancellation_on_the_calling_thread_returns_after_the_models() {
    let theory = tautologies(3);
    let (mut search, index) = walk(&theory, LATE);
    let cancellation = Cancellation::default();
    let mut budget = caller(&search, &cancellation);
    assert!(
        search
            .propose_with(&index, None, false, &mut budget, &mut |_| false)
            .unwrap()
            .is_some()
    );
    cancellation.cancel();
    assert_eq!(
        search
            .propose_with(&index, None, false, &mut budget, &mut |_| true)
            .err(),
        Some(Incomplete::Cancelled)
    );
    assert!(search.handles.is_empty());
    assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
}

#[test]
fn configuration_is_refused_once_the_walk_has_begun() {
    let theory = tautologies(3);
    let (mut search, index) = walk(&theory, LATE);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    search
        .propose_with(&index, None, false, &mut budget, &mut |_| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        search.set_filter(crate::region_filter::Filter::new(Arc::new(Pass {
            theory: theory.clone(),
            live: Arc::new(AtomicUsize::new(0)),
        }))),
        Err(Incomplete::LateRegionFilter)
    );
    let candidate = Interpretation::new(&theory, []).unwrap();
    assert_eq!(
        search.permits_determined(&candidate, &mut budget, &mut None),
        Err(Incomplete::LateCertificate)
    );
    // A restriction charges the shared allowance the walk draws on, without
    // resynchronizing it.
    let before = search.search_statistics().work;
    let restriction = Theory::new(
        3,
        FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    search.restrict(&restriction, &mut budget).unwrap();
    assert!(search.search_statistics().work > before);
    assert!(matches!(search.record, WalkRecord::Unengaged { .. }));
}

#[test]
fn an_unengaged_walk_keeps_its_record_once_it_ends() {
    let (mut search, index) = walk(&tautologies(3), NEVER);
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    drain(&mut search, &index, None, &mut budget);
    let statistics = search.statistics().workers.unwrap();
    let RegionWalkEngagement::Unengaged { elapsed, walking } = statistics.engagement else {
        panic!("{statistics:?}");
    };
    assert!(walking <= elapsed);
    assert!(
        search
            .propose(&index, None, false, &mut budget)
            .unwrap()
            .is_none()
    );
    assert_eq!(search.statistics().workers.unwrap(), statistics);
    search.stop().unwrap();
    assert_eq!(search.statistics().workers.unwrap(), statistics);
}

#[test]
fn an_engaged_walk_launches_every_configured_worker() {
    let (mut search, index) =
        walk_with(&tautologies(3), 3, Engagement::Immediate, Limits::default());
    let cancellation = search.shared.cancellation.clone();
    let mut budget = caller(&search, &cancellation);
    assert_eq!(drain(&mut search, &index, None, &mut budget).len(), 8);
    let statistics = search.statistics().workers.unwrap();
    assert_eq!(statistics.configured, 3);
    assert!(matches!(
        statistics.engagement,
        RegionWalkEngagement::Engaged { launched: 3, .. }
    ));
    assert_eq!(statistics.engagement_delay, Some(Duration::ZERO));
}

#[test]
fn a_delay_whose_deadline_overflows_reports_no_delay() {
    let (search, _) = walk(&tautologies(1), NEVER);
    let statistics = search.statistics().workers.unwrap();
    assert_eq!(statistics.engagement_delay, None);
    assert_eq!(statistics.engagement, RegionWalkEngagement::NotBegun);
}

#[test]
fn the_default_delay_is_the_engagement_cost_per_configured_worker() {
    let (search, _) = walk_with(&tautologies(1), 4, Engagement::default(), Limits::default());
    let statistics = search.statistics().workers.unwrap();
    assert_eq!(
        statistics.engagement_delay,
        Some(ENGAGEMENT_COST_PER_WORKER * 4)
    );
    assert_eq!(statistics.engagement, RegionWalkEngagement::NotBegun);
}

#[test]
fn stable_models_with_a_coordinator_state_stay_send() {
    fn send<T: Send>() {}
    send::<crate::StableModels>();
    send::<ParallelRegions>();
}
