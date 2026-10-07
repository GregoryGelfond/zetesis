//! Successive candidate bounds retain independent conditions and original membership.

use std::collections::BTreeSet;
use zetesis_ferraris::{Node, Theory};
use zetesis_sat::{
    BatchError, BatchVerdict, Cancellation, CertificateLimits, CertificateOrder, Incomplete,
    Limits, SearchMethod, StableModels,
};
use zetesis_theory_support::theories::theory;

use crate::support::batching::{batch, residual};
use crate::support::region_filters::{ROUTES, Route, choices};

fn disjunction(atoms: usize) -> Theory {
    theory(
        atoms,
        vec![Node::atom(0), Node::atom(1), Node::or_pair([0, 1])],
        vec![2],
    )
}

fn conjunction(atoms: usize) -> Theory {
    theory(
        atoms,
        vec![Node::atom(0), Node::atom(1), Node::and_pair([0, 1])],
        vec![2],
    )
}

#[test]
fn tightening_keeps_permanent_restrictions_added_after_a_bound() {
    let original = choices(3);
    let permanent = theory(
        3,
        vec![Node::atom(2), Node::falsum(), Node::implies(0, 1)],
        vec![2],
    );
    for route in ROUTES {
        let mut search = route.search(&original, Cancellation::default());
        search.tighten_candidate_bound(&disjunction(3)).unwrap();
        search.restrict_candidates(&permanent).unwrap();
        search.tighten_candidate_bound(&conjunction(3)).unwrap();
        assert_eq!(route.collect(&mut search), BTreeSet::from([vec![0, 1]]));
        assert_eq!(search.statistics().candidate_restrictions, 3);
    }
}

#[test]
fn a_bound_does_not_support_original_atoms() {
    let original = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![2, 3],
    );
    for route in ROUTES {
        let mut search = route.search(&original, Cancellation::default());
        search.tighten_candidate_bound(&disjunction(2)).unwrap();
        search.tighten_candidate_bound(&conjunction(2)).unwrap();
        assert!(route.collect(&mut search).is_empty());
        assert!(search.theory().same_instance(&original));
        assert_eq!(search.statistics().countermodels, 1);
    }
}

#[test]
fn universe_refusal_preserves_the_active_bound() {
    let original = choices(2);
    for route in ROUTES {
        let mut search = route.search(&original, Cancellation::default());
        search.tighten_candidate_bound(&conjunction(2)).unwrap();
        assert_eq!(
            search.tighten_candidate_bound(&choices(3)),
            Err(Incomplete::RestrictionUniverse {
                expected: 2,
                actual: 3
            })
        );
        assert_eq!(search.statistics().candidate_restrictions, 1);
        assert_eq!(route.collect(&mut search), BTreeSet::from([vec![0, 1]]));
    }
}

#[test]
fn tightening_preserves_pending_candidates_for_original_membership() {
    let original = choices(3);
    for route in [Route::Scalar, Route::Producers] {
        let mut search = route.search(&original, Cancellation::default());
        search.tighten_candidate_bound(&disjunction(3)).unwrap();
        let mut pending = Vec::new();
        let failed = search.next_batch(batch(3), |subject, candidates| {
            assert!(subject.same_instance(&original));
            pending = candidates
                .iter()
                .map(|candidate| candidate.atoms().collect::<Vec<_>>())
                .collect();
            Err::<Vec<BatchVerdict>, _>("retry")
        });
        assert!(matches!(failed, Err(BatchError::Checker("retry"))));
        let before = search.statistics().candidates;
        search
            .tighten_candidate_bound(&theory(3, vec![Node::falsum()], vec![0]))
            .unwrap();
        let completed = search
            .next_batch(batch(3), |subject, candidates| {
                assert!(subject.same_instance(&original));
                assert_eq!(
                    candidates
                        .iter()
                        .map(|candidate| candidate.atoms().collect::<Vec<_>>())
                        .collect::<Vec<_>>(),
                    pending
                );
                residual(subject, candidates)
            })
            .unwrap();
        assert_eq!(completed.len(), pending.len());
        assert_eq!(search.statistics().candidates, before);
        assert!(route.collect(&mut search).is_empty());
    }
}

#[test]
fn active_workers_preserve_every_model_under_the_latest_bound() {
    let original = choices(8);
    let mut search = Route::Native.search(&original, Cancellation::default());
    search.tighten_candidate_bound(&disjunction(8)).unwrap();
    let first = search.next().unwrap().unwrap();
    search.tighten_candidate_bound(&conjunction(8)).unwrap();
    let mut all = Route::Native.collect(&mut search);
    assert!(all.insert(first.atoms().collect()));
    let expected: BTreeSet<Vec<usize>> = (0..256)
        .filter(|mask| mask & 3 == 3)
        .map(|mask| (0..8).filter(|atom| mask & (1 << atom) != 0).collect())
        .collect();
    // Already sent or active old-generation models are permitted; none is
    // duplicated, none violates the earlier condition, and none newly eligible
    // under the stronger condition is lost.
    assert!(expected.is_subset(&all));
    assert!(
        all.iter()
            .all(|atoms| atoms.contains(&0) || atoms.contains(&1))
    );
}

#[test]
fn positive_cursor_checks_the_current_bound() {
    let original = theory(1, vec![Node::atom(0)], vec![0]);
    for route in ROUTES {
        let mut search = route.search(&original, Cancellation::default());
        assert!(
            search
                .enable_class_checking(
                    CertificateLimits::default(),
                    CertificateOrder::PositiveFirst
                )
                .unwrap()
        );
        assert!(matches!(
            search.statistics().certified.unwrap().plan,
            Some(zetesis_sat::CertificatePlanStatistics::Positive(_))
        ));
        search.tighten_candidate_bound(&original).unwrap();
        search
            .tighten_candidate_bound(&theory(1, vec![Node::falsum()], vec![0]))
            .unwrap();
        assert!(route.collect(&mut search).is_empty());
        assert_eq!(search.statistics().candidates, 0);
    }
}

#[test]
fn empty_universe_bounds_can_exclude_the_empty_interpretation() {
    let original = theory(0, vec![], vec![]);
    for route in ROUTES {
        let mut search = route.search(&original, Cancellation::default());
        search.tighten_candidate_bound(&original).unwrap();
        search
            .tighten_candidate_bound(&theory(0, vec![Node::falsum()], vec![0]))
            .unwrap();
        assert!(route.collect(&mut search).is_empty());
    }
}

#[test]
fn clauses_retain_equivalent_bounds_and_independent_restrictions() {
    let original = choices(3);
    let mut search = StableModels::with_method(
        &original,
        SearchMethod::Clauses,
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    search.tighten_candidate_bound(&disjunction(3)).unwrap();
    search
        .restrict_candidates(&theory(
            3,
            vec![Node::atom(2), Node::falsum(), Node::implies(0, 1)],
            vec![2],
        ))
        .unwrap();
    search.tighten_candidate_bound(&conjunction(3)).unwrap();
    assert_eq!(
        Route::Scalar.collect(&mut search),
        BTreeSet::from([vec![0, 1]])
    );
}
