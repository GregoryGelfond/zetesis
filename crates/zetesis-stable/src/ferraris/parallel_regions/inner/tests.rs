//! A published query's regions, served by a worker that did not publish them,
//! decide the candidate as the inline query does; the counts and stops that
//! license each verdict hold at the points the design names.
//!
//! Most tests drive one schedule by hand: worker 0 steps the candidate tree
//! until a leaf's query is published, and worker 1, the only other party,
//! then runs its loop on the calling thread, so every published region it
//! serves descends from one it stole.

use std::num::NonZeroUsize;
use std::sync::mpsc::sync_channel;

use zetesis_ferraris::{AdmissionLimits, FormulaParts, Narrower, Node, Region, Theory};

use crate::ferraris::conditions::CountermodelKnowledge;
use crate::ferraris::regions::InnerPreparation;

use super::super::{
    Engagement, ParallelRegions, Stepped, WalkRecord, WorkerReport, Workspaces, advance, worker,
};
use super::*;
use crate::budget::LocalQuota;
use crate::ferraris::Queried;
use crate::{Cancellation, Limits, RegionCounts, SearchStatistics, Statistics};

const WAIT: Duration = Duration::from_secs(2);

/// One clause literal: a universal variable and whether it occurs positively.
type Literal = (usize, bool);

fn push(nodes: &mut Vec<Node>, node: Node) -> usize {
    nodes.push(node);
    nodes.len() - 1
}

/// The saturation encoding of "no assignment of `variables` universal
/// variables satisfies `clauses`": `y(v) | ny(v)` for each variable, a falsified
/// clause derives `w`, `w` saturates every `y(v)` and `ny(v)`, and `:- not w`.
/// Atoms `0..variables` are `y`, the next `variables` are `ny`, and the last is
/// `w`. Its one classical model holds every atom, and is stable exactly when
/// the clauses are unsatisfiable: a satisfying assignment is a proper subset
/// that models its reduct.
fn saturation(variables: usize, clauses: &[Vec<Literal>]) -> Theory {
    let w = 2 * variables;
    let atoms = w + 1;
    let mut nodes: Vec<Node> = (0..atoms).map(Node::atom).collect();
    let falsum = push(&mut nodes, Node::falsum());
    let mut roots = Vec::new();
    for variable in 0..variables {
        let negated = variables + variable;
        roots.push(push(&mut nodes, Node::or_pair([variable, negated])));
        roots.push(push(&mut nodes, Node::implies(w, variable)));
        roots.push(push(&mut nodes, Node::implies(w, negated)));
    }
    for clause in clauses {
        // A positive literal is false when `ny` holds, a negative one when `y`
        // does.
        let mut body = clause.iter().map(|&(variable, positive)| {
            if positive {
                variables + variable
            } else {
                variable
            }
        });
        let first = body.next().expect("a clause has a literal");
        let falsified = body.fold(first, |left, right| {
            push(&mut nodes, Node::and_pair([left, right]))
        });
        roots.push(push(&mut nodes, Node::implies(falsified, w)));
    }
    let not_w = push(&mut nodes, Node::implies(w, falsum));
    roots.push(push(&mut nodes, Node::implies(not_w, falsum)));
    Theory::new(
        atoms,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// A gate's four clauses over `(a, i, o)`, forbidding the odd patterns, in
/// this module's literal convention: a positive literal is falsified when
/// its variable is cut.
fn gate_clauses(a: usize, i: usize, o: usize) -> Vec<Vec<Literal>> {
    [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, true),
    ]
    .into_iter()
    .map(|(sa, si, so)| vec![(a, !sa), (i, !si), (o, !so)])
    .collect()
}

/// Two gates over shared inputs, `y(2) = y(0) ⊕ y(1)` and `y(3) = y(0) ⊕
/// y(1)`: a satisfiable core whose rows combine to `y(2) ⊕ y(3) = 0`, read
/// only through the candidate's preparation.
fn shared_input_gates() -> Vec<Vec<Literal>> {
    let mut clauses = gate_clauses(0, 1, 2);
    clauses.extend(gate_clauses(0, 1, 3));
    clauses
}

/// Every clause over `variables`, each forbidding one assignment: no
/// assignment satisfies them all.
fn every_clause(variables: usize) -> Vec<Vec<Literal>> {
    (0..1_usize << variables)
        .map(|signs| {
            (0..variables)
                .map(|variable| (variable, signs & (1 << variable) != 0))
                .collect()
        })
        .collect()
}

/// [`every_clause`] less one: the assignment it forbade satisfies the rest.
fn all_but_one_clause(variables: usize) -> Vec<Vec<Literal>> {
    let mut clauses = every_clause(variables);
    clauses.remove(clauses.len() / 3);
    clauses
}

/// The original index the enumeration would hand its workers.
fn index(theory: &Theory) -> Arc<IndexedTheory> {
    let mut owner = crate::ferraris::OriginalIndex::new(theory, u64::MAX);
    Arc::clone(owner.ensure(&Cancellation::default(), |_| Ok(())).unwrap())
}

/// A two-worker walk over `theory` under `limits`, its root on deque 0,
/// dividing its queries as `division`.
fn walk(
    theory: &Theory,
    division: QueryDivision,
    limits: Limits,
) -> (ParallelRegions, Arc<IndexedTheory>) {
    let cancellation = Cancellation::with_deadline(Instant::now() + 3 * WAIT).unwrap();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut search = ParallelRegions::new(
        crate::ferraris::regions::open(
            theory,
            limits.max_reduct_bytes,
            limits.learning,
            &mut budget,
        )
        .unwrap(),
        NonZeroUsize::new(2).unwrap(),
        Engagement::Immediate,
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap();
    search.set_query_division(division);
    (search, index(theory))
}

fn lease(shared: &Shared) -> Budget<'_, WorkLease<'_>> {
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

/// Advance worker 0's candidate regions until a leaf is decided or its query
/// is published, as worker 0 would; returns the last step and worker 0's
/// report.
fn step_worker_0(search: &ParallelRegions, original: &IndexedTheory) -> (Stepped, WorkerReport) {
    let mut workspaces = Workspaces::new();
    let mut report = report();
    loop {
        let entry = search
            .shared
            .frontier
            .take_local(0)
            .expect("a candidate region remains");
        let mut budget = lease(&search.shared);
        let stepped = advance(
            &search.shared,
            original,
            (entry, 0),
            &mut workspaces,
            &mut budget,
            (&mut report, &mut SearchPhaseTimings::default()),
            Timing::Worker,
        )
        .unwrap();
        if !matches!(stepped, Stepped::Split) {
            return (stepped, report);
        }
    }
}

/// Run worker 1's loop on the calling thread until the walk ends; returns
/// its report and the models it sent.
fn run_worker_1(
    search: &ParallelRegions,
    original: &IndexedTheory,
) -> (WorkerReport, Vec<Interpretation>) {
    let (sender, receiver) = sync_channel(16);
    let report = worker(&search.shared, original, 1, &sender);
    drop(sender);
    (report, receiver.into_iter().collect())
}

/// The candidate's query regions, as the inline query walks them.
fn inline_regions(theory: &Theory) -> RegionCounts {
    let (search, original) = walk(theory, QueryDivision::Never, Limits::default());
    let (stepped, report) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Resolved(_)));
    report.statistics.reduct.regions
}

#[test]
fn a_published_leaf_stays_outstanding_until_its_query_is_decided() {
    let theory = saturation(9, &every_clause(9));
    let (search, original) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Published));
    assert_eq!(search.shared.frontier.termination.unresolved(), 1);
    assert_eq!(search.query_division().published, 1);
    run_worker_1(&search, &original);
    assert_eq!(search.shared.frontier.termination.unresolved(), 0);
}

#[test]
fn stolen_regions_prove_an_unrefuted_candidate_stable() {
    let theory = saturation(9, &every_clause(9));
    let (search, original) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let (stepped, publisher) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Published));
    let (server, models) = run_worker_1(&search, &original);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].atoms().count(), theory.atom_count());
    assert_eq!(search.shared.stopped(), None);
    assert_eq!(search.shared.frontier.termination.unresolved(), 0);
    // Every region of the query was narrowed once, by the server alone: the
    // publisher published before its first narrowing.
    let receipts = search.query_division();
    assert_eq!(publisher.statistics.reduct.regions.regions, 0);
    assert_eq!(
        server.statistics.reduct.regions.regions,
        inline_regions(&theory).regions
    );
    assert_eq!(
        receipts.served,
        server.statistics.reduct.regions.regions as u64
    );
    assert!(receipts.stolen >= 1);
    assert_eq!((receipts.published, receipts.servers), (1, 1));
    assert_eq!(receipts.peak_served, receipts.served);
}

#[test]
fn a_countermodel_in_a_stolen_region_refutes_its_candidate() {
    let theory = saturation(9, &all_but_one_clause(9));
    let (search, original) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Published));
    let (server, models) = run_worker_1(&search, &original);
    assert!(models.is_empty(), "a refuted candidate was delivered");
    assert_eq!(search.shared.stopped(), None);
    assert_eq!(search.shared.frontier.termination.unresolved(), 0);
    assert_eq!(server.statistics.countermodels, 1);
    let receipts = search.query_division();
    assert!(receipts.stolen >= 1);
    assert_eq!(receipts.published, 1);
}

#[test]
fn a_validation_its_ceiling_stops_never_yields_stable() {
    // Measure the candidate's original evaluation, then allow exactly that
    // much verification work: the evaluation passes, and the validation of a
    // real countermodel, which freezes the reduct before testing it, stops.
    let theory = saturation(9, &all_but_one_clause(9));
    let (search, original) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let (_, publisher) = step_worker_0(&search, &original);
    let evaluation = publisher.statistics.reduct.original_work;
    let limits = Limits {
        max_verification_work: evaluation,
        ..Limits::default()
    };
    let (search, original) = walk(&theory, QueryDivision::Immediate, limits);
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Published));
    let (server, models) = run_worker_1(&search, &original);
    assert!(models.is_empty(), "an undecided candidate was delivered");
    assert!(matches!(
        search.shared.stopped(),
        Some(Incomplete::Verification(zetesis_algebra::Stop::WorkLimit))
    ));
    // The undecided leaf stays outstanding: the walk can claim nothing.
    assert_eq!(search.shared.frontier.termination.unresolved(), 1);
    assert_eq!(server.statistics.countermodels, 0);
    assert!(search.query_division().served >= 1);
}

#[test]
fn a_server_stopped_by_the_decision_ceiling_never_yields_stable() {
    let theory = saturation(9, &every_clause(9));
    let limits = Limits {
        search: crate::SearchLimits {
            max_decisions: 4,
            ..crate::SearchLimits::default()
        },
        ..Limits::default()
    };
    let (search, original) = walk(&theory, QueryDivision::Immediate, limits);
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Published));
    let (_, models) = run_worker_1(&search, &original);
    assert!(models.is_empty(), "an undecided candidate was delivered");
    assert_eq!(search.shared.stopped(), Some(Incomplete::DecisionLimit));
    assert_eq!(search.shared.frontier.termination.unresolved(), 1);
    assert!(search.query_division().served >= 1);
}

#[test]
fn a_cancelled_server_never_yields_stable() {
    let theory = saturation(9, &every_clause(9));
    let (search, original) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Published));
    search.shared.cancellation.cancel();
    let (_, models) = run_worker_1(&search, &original);
    assert!(models.is_empty(), "an undecided candidate was delivered");
    assert_eq!(search.shared.stopped(), Some(Incomplete::Cancelled));
    assert_eq!(search.shared.frontier.termination.unresolved(), 1);
}

#[test]
fn a_query_that_never_divides_is_decided_where_it_ran() {
    let theory = saturation(9, &every_clause(9));
    let (search, original) = walk(&theory, QueryDivision::Never, Limits::default());
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Resolved(Some(_))));
    assert_eq!(search.query_division().published, 0);
    assert!(search.shared.inner.take_local(0).is_none());
}

#[test]
fn a_query_waits_for_an_idle_worker_before_it_publishes() {
    // No worker is idle at the gate, so however long the query runs it is
    // decided inline.
    let theory = saturation(9, &every_clause(9));
    let (search, original) = walk(&theory, QueryDivision::WhenIdle, Limits::default());
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Resolved(Some(_))));
    assert_eq!(search.query_division().published, 0);
}

#[test]
fn a_query_publishes_to_a_worker_idle_at_the_gate_after_its_delay() {
    let theory = saturation(9, &every_clause(9));
    let (search, original) = walk(&theory, QueryDivision::WhenIdle, Limits::default());
    // Stand in for a worker waiting at the gate, as it registers there.
    search
        .shared
        .frontier
        .termination
        .idle_waiters
        .fetch_add(1, Ordering::Relaxed);
    let mut publication = WalkPublication::new(&search.shared, 0, Timing::Worker);
    assert!(!publication.due(), "the query has not run for its delay");
    // The query started at that first reading, before this instant.
    let asked = Instant::now();
    while asked.elapsed() < INNER_DELAY {
        std::hint::spin_loop();
    }
    assert!(publication.due());
    search
        .shared
        .frontier
        .termination
        .idle_waiters
        .fetch_sub(1, Ordering::Relaxed);
    assert!(!publication.due(), "no worker is idle");
    // The whole walk under this division still decides the candidate.
    let (stepped, _) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Resolved(Some(_))));
}

#[test]
fn a_split_counts_its_gain_before_its_children_are_queued() {
    let theory = saturation(3, &every_clause(3));
    let (search, _) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let shared = &search.shared;
    let candidate = Interpretation::new(&theory, 0..theory.atom_count()).unwrap();
    let truth = vec![true; theory.nodes().len()];
    let cell = Arc::new(QueryCell::new(&truth, &candidate, 1).unwrap());
    let knowledge = CountermodelKnowledge::fresh(&Narrower::new(&theory));
    let cut_knowledge = knowledge.clone();
    let mut pending = Some(InnerEntry::new(
        Entry::new(Region::all_open(theory.atom_count()), knowledge),
        &cell,
    ));
    shared
        .inner
        .publish_split(
            1,
            &mut pending,
            0,
            cut_knowledge,
            &shared.frontier.termination,
            |queue, additional| {
                // The parent, in hand, and the split's net gain: both counted
                // while neither child is queued.
                assert_eq!(cell.outstanding.load(Ordering::Acquire), 2);
                assert!(queue.is_empty());
                reserve_entries(queue, additional)
            },
        )
        .unwrap_or_else(|error| panic!("split refused: {error:?}"));
    assert!(pending.is_none(), "an admitted split consumes the parent");
    assert_eq!(cell.outstanding.load(Ordering::Acquire), 2);
    let newest = shared.inner.take_local(1).unwrap();
    assert!(
        newest.entry.region.is_cut(0),
        "the cut child is served first"
    );
    assert!(shared.inner.take_local(1).unwrap().entry.region.is_held(0));
}

#[test]
fn a_refused_split_publishes_nothing_and_withdraws_its_gain() {
    let theory = saturation(3, &every_clause(3));
    let (search, _) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let shared = &search.shared;
    let candidate = Interpretation::new(&theory, 0..theory.atom_count()).unwrap();
    let truth = vec![true; theory.nodes().len()];
    let cell = Arc::new(QueryCell::new(&truth, &candidate, 1).unwrap());
    let knowledge = CountermodelKnowledge::fresh(&Narrower::new(&theory));
    let mut entry = Entry::new(Region::all_open(theory.atom_count()), knowledge);
    entry.prefer(1);
    let region = entry.region.clone();
    let knowledge_before = format!("{:?}", entry.state);
    let cut_knowledge = entry.state.clone();
    let mut pending = Some(InnerEntry::new(entry, &cell));
    let result = shared.inner.publish_split(
        1,
        &mut pending,
        1,
        cut_knowledge,
        &shared.frontier.termination,
        |queue, _| reserve_entries(queue, usize::MAX),
    );
    let Err(error) = result else {
        panic!("capacity overflow must refuse publication");
    };
    let parent = pending.take().expect("refusal retains the whole parent");
    assert_eq!(error, Incomplete::Allocation);
    assert_eq!(parent.entry.region, region);
    assert_eq!(parent.entry.split_atom(), Some(1));
    assert_eq!(format!("{:?}", parent.entry.state), knowledge_before);
    assert!(Arc::ptr_eq(&parent.cell, &cell));
    assert_eq!(cell.outstanding.load(Ordering::Acquire), 1);
    assert!(shared.inner.take_local(1).is_none());
    assert!(cell.stop.get().is_none());
    assert_eq!(shared.stopped(), None);
    // The whole parent is still pending. Teardown is a separate, typed stop.
    abandon(shared, parent, error);
    assert_eq!(cell.stop.get(), Some(&Incomplete::Allocation));
    assert_eq!(shared.stopped(), Some(Incomplete::Allocation));
    assert_eq!(cell.outstanding.load(Ordering::Acquire), 0);
    assert!(!cell.is_resolved());
    assert!(!shared.frontier.termination.is_resolved());
}

#[test]
fn a_decided_querys_queued_region_is_dropped_unnarrowed() {
    let theory = saturation(3, &every_clause(3));
    let (search, original) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let shared = &search.shared;
    let candidate = Interpretation::new(&theory, 0..theory.atom_count()).unwrap();
    let truth = vec![true; theory.nodes().len()];
    let cell = Arc::new(QueryCell::new(&truth, &candidate, 2).unwrap());
    assert!(cell.absorb(candidate.try_clone().unwrap()));
    let entry = InnerEntry::new(
        Entry::new(
            Region::all_open(theory.atom_count()),
            CountermodelKnowledge::fresh(&Narrower::new(&theory)),
        ),
        &cell,
    );
    let mut report = report();
    let model = serve(
        shared,
        &original,
        (entry, Taken::Stolen, 1),
        &mut Workspaces::new(),
        &mut lease(shared),
        (&mut report, &mut SearchPhaseTimings::default()),
    )
    .unwrap();
    assert!(model.is_none());
    assert_eq!(report.statistics.reduct.regions.regions, 0);
    assert_eq!(search.query_division().served, 0);
    assert_eq!(search.query_division().stolen, 0);
    assert_eq!(cell.outstanding.load(Ordering::Acquire), 1);
}

#[test]
fn a_declined_publication_leaves_the_query_to_decide_inline() {
    // A publication due at every narrowing and always declined is offered
    // once, and the query decides the candidate where it ran.
    struct Declining {
        offered: usize,
    }
    impl Publication for Declining {
        type Receipt = ();
        fn due(&mut self) -> bool {
            true
        }
        fn publish(
            &mut self,
            _: &[bool],
            _: &Interpretation,
            _: &mut InnerPreparation,
            traversal: &mut Walk<CountermodelKnowledge>,
        ) -> Option<()> {
            self.offered += 1;
            assert!(traversal.pending() >= 1);
            None
        }
    }
    for (clauses, stable) in [(every_clause(9), true), (all_but_one_clause(9), false)] {
        let theory = saturation(9, &clauses);
        let original = index(&theory);
        let candidate = Interpretation::new(&theory, 0..theory.atom_count()).unwrap();
        let cancellation = Cancellation::default();
        let mut budget = Budget {
            quota: LocalQuota,
            limits: Limits::default().search,
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        let mut declining = Declining { offered: 0 };
        let queried = crate::reduct::State::default()
            .check_publishing(
                (&theory, Some(&original)),
                &candidate,
                Limits::default(),
                &mut budget,
                &mut Statistics::default(),
                &mut declining,
            )
            .unwrap();
        assert_eq!(declining.offered, 1);
        match queried {
            Queried::Decided(check) => assert_eq!(check.accepted(), stable),
            Queried::Published(()) => panic!("a declined publication published"),
        }
    }
}

#[test]
fn a_coordinator_publishes_its_query_at_engagement() {
    let theory = saturation(9, &every_clause(9));
    let cancellation = Cancellation::with_deadline(Instant::now() + 3 * WAIT).unwrap();
    let limits = Limits::default();
    let mut budget = Budget {
        quota: LocalQuota,
        limits: limits.search,
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut search = ParallelRegions::new(
        crate::ferraris::regions::open(
            &theory,
            limits.max_reduct_bytes,
            limits.learning,
            &mut budget,
        )
        .unwrap(),
        NonZeroUsize::new(2).unwrap(),
        Engagement::After(Duration::from_hours(1)),
        limits,
        cancellation.clone(),
        &mut budget,
    )
    .unwrap();
    let original = index(&theory);
    // The deadline has not passed at the pull's first check, and has at every
    // later reading: the root's leaf query asks before its first narrowing.
    let mut readings = 0;
    let mut deadline_passed = |_| {
        readings += 1;
        readings > 1
    };
    let model = search
        .propose_with(&original, None, false, &mut budget, &mut deadline_passed)
        .unwrap()
        .expect("the stable candidate");
    assert_eq!(model.atoms().count(), theory.atom_count());
    assert!(matches!(search.record, WalkRecord::Engaged { .. }));
    let receipts = search.query_division();
    assert_eq!(receipts.published, 1);
    assert!(receipts.served >= 1);
    // The coordinator narrowed no region of the query before publishing it.
    assert_eq!(search.merged().reduct.regions.regions, 0);
    assert!(
        search
            .propose_with(&original, None, false, &mut budget, &mut deadline_passed)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        search.merged().reduct.regions.regions,
        inline_regions(&theory).regions
    );
}

mod publication;

#[test]
fn a_published_query_carries_its_preparation_to_its_servers() {
    let theory = saturation(4, &shared_input_gates());
    let (search, original) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let (stepped, publisher) = step_worker_0(&search, &original);
    assert!(matches!(stepped, Stepped::Published));
    // Published before its first narrowing: the publisher prepared nothing,
    // and the root's server prepares for every later server.
    assert_eq!(publisher.statistics.reduct.inner_preparation.attempts, 0);
    let pending_region = search
        .shared
        .inner
        .take_local(0)
        .expect("the query published its pending regions");
    let cell = Arc::clone(&pending_region.cell);
    assert!(cell.preparation.get().is_none());
    let mut workspaces = Workspaces::new();
    let mut budget = lease(&search.shared);
    let mut served_report = report();
    serve(
        &search.shared,
        &original,
        (pending_region, Taken::Own, 0),
        &mut workspaces,
        &mut budget,
        (&mut served_report, &mut SearchPhaseTimings::default()),
    )
    .unwrap();
    assert_eq!(served_report.statistics.reduct.regions.regions, 1);
    let preparation = served_report.statistics.reduct.inner_preparation;
    assert_eq!((preparation.attempts, preparation.prepared), (1, 1));
    assert_eq!(preparation.rows, 2);
    assert!(cell.preparation.get().is_some_and(Option::is_some));
    assert_ne!(
        served_report.statistics.reduct.regions.arithmetic,
        crate::ferraris::regions::arithmetic_counts::ArithmeticCounts::default()
    );
    run_worker_1(&search, &original);
    assert_eq!(search.shared.frontier.termination.unresolved(), 0);
}
