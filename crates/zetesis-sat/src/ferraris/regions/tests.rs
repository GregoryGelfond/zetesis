//! Exact subject ownership and construction accounting for shared indexes.

use std::num::NonZeroUsize;

use super::*;
use crate::ferraris::Proposer;
use crate::prepared_reduct::State;
use crate::search::LocalQuota;
use crate::{Cancellation, Limits, SearchLimits, SearchStatistics, StableModels, Statistics};
use zetesis_ferraris::{AdmissionLimits, Node};

fn cycle() -> Theory {
    Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(0, 1),
            Node::Implies(1, 0),
        ],
        vec![2, 3],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn budget(cancellation: &Cancellation, work: u64) -> Budget<'_> {
    Budget {
        quota: LocalQuota,
        limits: SearchLimits {
            max_work: work,
            ..SearchLimits::default()
        },
        cancellation,
        statistics: SearchStatistics::default(),
    }
}

#[test]
fn enumeration_queries_share_the_candidate_index() {
    let theory = cycle();
    for (producers, workers) in [(false, 1), (false, 2), (false, 4), (true, 2), (true, 4)] {
        let workers = NonZeroUsize::new(workers).unwrap();
        let search = if producers {
            StableModels::with_region_producers(
                &theory,
                workers,
                Limits::default(),
                Cancellation::default(),
            )
        } else {
            StableModels::with_region_workers(
                &theory,
                workers,
                Limits::default(),
                Cancellation::default(),
            )
        }
        .unwrap();
        let index = match &search.proposer {
            Proposer::Regions(proposer) => proposer.index(),
            Proposer::Parallel(proposer) => proposer.index(),
            Proposer::Proposals(proposer) => proposer.index(),
            Proposer::Clauses(_) => panic!("region constructor selected clauses"),
        };
        let query = search.reduct.query().unwrap();
        assert!(Arc::ptr_eq(index, &query.index));
        assert!(query.theory().same_instance(&theory));
        let extraction =
            zetesis_ferraris::producers(&theory, RegionLimits::default(), &Cancellation::default())
                .unwrap();
        let construction = extraction.work + u64::try_from(theory.nodes().len()).unwrap();
        assert_eq!(search.statistics().search.work, construction);
        assert_eq!(
            search.statistics().regions.unwrap().counts.work,
            construction
        );
        assert_eq!(search.statistics().reduct.regions.work, 0);
    }
}

#[test]
fn a_shared_index_rejects_an_independent_equal_theory() {
    let theory = cycle();
    let equal = cycle();
    let candidate = Interpretation::new(&equal, []).unwrap();
    let mut state = State::with_index(Arc::new(IndexedTheory::new(&theory).unwrap()));
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, 100);
    let mut statistics = Statistics::default();
    state
        .ensure(
            &theory.clone(),
            Limits::default(),
            &mut budget,
            &mut statistics,
        )
        .unwrap();
    assert_eq!(
        state.ensure(&equal, Limits::default(), &mut budget, &mut statistics),
        Err(Incomplete::WrongTheory)
    );
    // An empty truth buffer would be invalid for either DAG. Identity refusal
    // must precede every indexed access, independently of equal dimensions.
    assert!(matches!(
        state.query().unwrap().check(
            &equal,
            &candidate,
            &[],
            Limits::default(),
            &mut budget,
            &mut statistics
        ),
        Err(Incomplete::WrongTheory)
    ));
    assert!(matches!(
        state.query().unwrap().check(
            &theory,
            &candidate,
            &[],
            Limits::default(),
            &mut budget,
            &mut statistics
        ),
        Err(Incomplete::WrongTheory)
    ));
    assert_eq!(budget.statistics, SearchStatistics::default());
    assert_eq!(statistics, Statistics::default());
}

#[test]
fn reused_index_needs_no_second_construction_grant() {
    let theory = cycle();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let opened = open(&theory, &mut budget).unwrap();
    let paid = budget.statistics;
    budget.limits.max_work = paid.work;
    let mut state = State::with_index(opened.index);
    let mut statistics = Statistics::default();
    state
        .ensure(&theory, Limits::default(), &mut budget, &mut statistics)
        .unwrap();
    state
        .ensure(&theory, Limits::default(), &mut budget, &mut statistics)
        .unwrap();
    assert_eq!(budget.statistics, paid);
    assert_eq!(statistics.reduct.regions.work, 0);
    let candidate = Interpretation::new(&theory, []).unwrap();
    assert!(matches!(
        state.check(
            &theory,
            &candidate,
            Limits::default(),
            &mut budget,
            &mut statistics
        ),
        Err(Incomplete::WorkLimit)
    ));
    assert_eq!(budget.statistics, paid);
    assert_eq!(statistics.countermodel_queries, 1);
    assert_eq!(statistics.reduct.regions.work, 0);
}

#[test]
fn standalone_membership_charges_its_one_index() {
    let theory = cycle();
    let cancellation = Cancellation::default();
    let required = u64::try_from(theory.nodes().len()).unwrap();
    let mut budget = budget(&cancellation, required - 1);
    let mut state = State::new(SearchMethod::Regions);
    let mut statistics = Statistics::default();
    assert_eq!(
        state.ensure(&theory, Limits::default(), &mut budget, &mut statistics),
        Err(Incomplete::WorkLimit)
    );
    assert!(state.query().is_none());
    assert_eq!(budget.statistics.work, 0);
    assert_eq!(statistics.reduct.regions.work, 0);
    budget.limits.max_work = required;
    state
        .ensure(&theory, Limits::default(), &mut budget, &mut statistics)
        .unwrap();
    let owner = Arc::clone(&state.query().unwrap().index);
    state
        .ensure(&theory, Limits::default(), &mut budget, &mut statistics)
        .unwrap();
    assert!(Arc::ptr_eq(&owner, &state.query().unwrap().index));
    assert_eq!(budget.statistics.work, required);
    assert_eq!(statistics.reduct.regions.work, required);
}

#[test]
fn cancelled_shared_membership_performs_no_query_work() {
    let theory = cycle();
    let candidate = Interpretation::new(&theory, []).unwrap();
    let mut state = State::with_index(Arc::new(IndexedTheory::new(&theory).unwrap()));
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, 100);
    let mut statistics = Statistics::default();
    cancellation.cancel();
    assert!(matches!(
        state.check(
            &theory,
            &candidate,
            Limits::default(),
            &mut budget,
            &mut statistics
        ),
        Err(Incomplete::Cancelled)
    ));
    assert_eq!(budget.statistics, SearchStatistics::default());
    assert_eq!(statistics, Statistics::default());
}
