//! Several workers walking the region tree return exactly the stable
//! models one worker returns, once each, in an order that is the
//! schedule's and not a property of the result; a restriction narrows what
//! the workers have not yet visited; a shared ceiling stops them all.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_ferraris::{AdmissionLimits, Node, Theory, TightPlanLimits};
use zetesis_sat::{Control, Incomplete, Limits, SearchLimits, SearchMethod, StableModels};

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

/// p | q <- d.  d.  r <- not p.  :- q, r.  s | not s.
fn mixed() -> Theory {
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::Atom(3),
        Node::Or(1, 2),
        Node::Implies(0, 4),
        Node::False,
        Node::Implies(1, 6),
        Node::Implies(7, 3),
        Node::And(2, 3),
        Node::Implies(9, 6),
        Node::Atom(4),
        Node::Implies(11, 6),
        Node::Or(11, 12),
    ];
    theory(5, nodes, vec![0, 5, 8, 10, 13])
}

/// Independent choices: a | not a, for each atom.
fn choices(atoms: usize) -> Theory {
    let mut nodes = vec![Node::False];
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let a = nodes.len();
        nodes.push(Node::Atom(atom));
        nodes.push(Node::Implies(a, 0));
        nodes.push(Node::Or(a, a + 1));
        roots.push(a + 2);
    }
    theory(atoms, nodes, roots)
}

fn workers(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

fn family(search: &mut StableModels) -> Vec<Vec<usize>> {
    search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect()
}

#[test]
fn four_workers_return_the_scalar_family_once_each() {
    for theory in [mixed(), choices(6)] {
        let mut scalar = StableModels::with_method(
            &theory,
            SearchMethod::Regions,
            Limits::default(),
            Control::default(),
        )
        .unwrap();
        let expected: BTreeSet<Vec<usize>> = family(&mut scalar).into_iter().collect();
        let mut parallel = StableModels::with_region_workers(
            &theory,
            workers(4),
            Limits::default(),
            Control::default(),
        )
        .unwrap();
        let found = family(&mut parallel);
        assert!(parallel.exhausted());
        let distinct: BTreeSet<Vec<usize>> = found.iter().cloned().collect();
        assert_eq!(distinct.len(), found.len(), "each model once");
        assert_eq!(distinct, expected);
        let statistics = parallel.statistics();
        assert_eq!(
            statistics.stable_models,
            u64::try_from(found.len()).unwrap()
        );
        assert!(statistics.regions.expect("region receipts").regions > 0);
        assert_eq!(statistics.candidate_queries, 0);
    }
}

#[test]
fn a_restriction_reaches_the_workers_for_what_they_have_not_visited() {
    let theory = choices(8);
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Control::default(),
    )
    .unwrap();
    let first: Vec<usize> = parallel.next().unwrap().unwrap().atoms().collect();
    let restriction = theory_over(&theory, vec![Node::Atom(7)], vec![0]);
    parallel.restrict_candidates(&restriction).unwrap();
    let rest = family(&mut parallel);
    assert!(parallel.exhausted());
    // Models already sent before the restriction may still arrive; every
    // model the workers narrow afterwards holds atom 7, and no model
    // repeats.
    let mut all = vec![first];
    all.extend(rest);
    let distinct: BTreeSet<&Vec<usize>> = all.iter().collect();
    assert_eq!(distinct.len(), all.len());
    let without: usize = all.iter().filter(|model| !model.contains(&7)).count();
    assert!(
        without <= 4 * 16 + 1,
        "at most the channel's slack lacked the restriction"
    );
    assert!(all.iter().filter(|model| model.contains(&7)).count() >= 1);
}

fn theory_over(original: &Theory, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    theory(original.atom_count(), nodes, roots)
}

#[test]
fn a_shared_work_ceiling_stops_every_worker_without_exhaustion() {
    let theory = choices(10);
    let construction = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Control::default(),
    )
    .unwrap()
    .statistics()
    .search
    .work;
    let limits = Limits {
        search: SearchLimits {
            max_work: construction + 200,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let mut parallel =
        StableModels::with_region_workers(&theory, workers(4), limits, Control::default()).unwrap();
    let outcomes: Vec<_> = parallel.by_ref().collect();
    assert!(matches!(outcomes.last(), Some(Err(Incomplete::WorkLimit))));
    assert!(!parallel.exhausted());
}

#[test]
fn a_candidate_ceiling_delivers_the_admitted_leaves_before_it_stops() {
    // Two independent choices leave four leaves for sixteen workers. One
    // leaf is admitted under the ceiling; the workers refused by the
    // ceiling raise the stop while that leaf is still being decided, and
    // its model must arrive before the stop does. Repeated, because the
    // interleaving is the schedule's.
    let theory = choices(2);
    let limits = Limits {
        max_candidates: 1,
        ..Limits::default()
    };
    for _ in 0..20 {
        let mut parallel =
            StableModels::with_region_workers(&theory, workers(16), limits, Control::default())
                .unwrap();
        let outcomes: Vec<_> = parallel.by_ref().collect();
        assert_eq!(outcomes.len(), 2, "{outcomes:?}");
        assert!(outcomes[0].is_ok(), "{outcomes:?}");
        assert!(matches!(outcomes[1], Err(Incomplete::CandidateLimit)));
        assert!(parallel.next().is_none());
        assert!(!parallel.exhausted());
        assert_eq!(parallel.statistics().candidates, 1);
        assert_eq!(parallel.statistics().stable_models, 1);
    }
}

#[test]
fn a_tight_certificate_decides_the_workers_leaves() {
    let theory = choices(5);
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(3),
        Limits::default(),
        Control::default(),
    )
    .unwrap();
    assert!(
        parallel
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let outcomes: Vec<_> = parallel.by_ref().collect();
    let found: Vec<Vec<usize>> = outcomes
        .into_iter()
        .map(|model| match model {
            Ok(model) => model.atoms().collect::<Vec<_>>(),
            Err(error) => panic!("{error:?}"),
        })
        .collect();
    assert_eq!(found.len(), 32);
    assert!(parallel.exhausted());
    assert_eq!(parallel.statistics().countermodel_queries, 0);
}

#[test]
fn phase_timings_sum_the_workers_narrowing_and_leaf_decisions() {
    let theory = choices(6);
    let mut parallel = StableModels::with_region_workers(
        &theory,
        workers(4),
        Limits::default(),
        Control::default(),
    )
    .unwrap();
    parallel.enable_phase_timing();
    assert_eq!(family(&mut parallel).len(), 64);
    let timings = parallel.statistics().phase_timings.unwrap();
    // Every leaf is decided by the reduct query once, in some worker; every
    // region is narrowed once; no certificate was enabled.
    assert_eq!(timings.reduct.calls, 64);
    assert!(timings.candidates.calls >= 64);
    assert_eq!(timings.certified.calls, 0);
    assert!(timings.reduct.elapsed > std::time::Duration::ZERO);
}

#[test]
fn one_worker_is_the_scalar_walk() {
    let theory = mixed();
    let mut one = StableModels::with_region_workers(
        &theory,
        workers(1),
        Limits::default(),
        Control::default(),
    )
    .unwrap();
    let mut scalar = StableModels::with_method(
        &theory,
        SearchMethod::Regions,
        Limits::default(),
        Control::default(),
    )
    .unwrap();
    assert_eq!(family(&mut one), family(&mut scalar));
}
