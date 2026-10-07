//! The proper-subset query on regions: the coverage tree over the subsets
//! of a classical model, narrowed by the frozen reduct's knowledge. Its
//! verdicts agree with the exhaustive reference and with the clause query
//! on every candidate of small theories, its witnesses are validated proper
//! subsets, and its resources stop it without a verdict.

use crate::support::batching::{batch, residual};
use crate::support::choice_theories::choices;
use crate::support::formula_theories as theories;
use crate::support::interpretations::interpretation;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_ferraris::{Node, Theory, Verdict};
use zetesis_sat::{
    BatchError, Cancellation, Check, CompletionExecutor, Incomplete, Limits, ReductWorkspace,
    SearchLimits, SearchMethod, StableModels, check_with,
};

use theories::mixed;
use zetesis_theory_support::theories::theory;

/// a <- b.  b <- a.  a | b | c.  A positive cycle the support law misses.
fn cycle() -> Theory {
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::implies(1, 0),
        Node::implies(0, 1),
        Node::or_pair([0, 1]),
        Node::or_pair([5, 2]),
    ];
    theory(3, nodes, vec![3, 4, 6])
}

/// {a}. {b}. c <- a, b.  c <- c.  A self-supporting rule.
fn self_support() -> Theory {
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::falsum(),
        Node::implies(0, 3),
        Node::or_pair([0, 4]),
        Node::implies(1, 3),
        Node::or_pair([1, 6]),
        Node::and_pair([0, 1]),
        Node::implies(8, 2),
        Node::implies(2, 2),
    ];
    theory(3, nodes, vec![5, 7, 9, 10])
}

/// a <- b.  b <- a.  :- not a.  A positive loop under a negative constraint:
/// {a, b} is a classical model whose reduct the empty set models, since
/// the constraint's negative literal is false under it and freezes to falsum.
fn loop_under_constraint() -> Theory {
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::falsum(),
        Node::implies(1, 0),
        Node::implies(0, 1),
        Node::implies(0, 2),
        Node::implies(5, 2),
    ];
    theory(2, nodes, vec![3, 4, 6])
}

/// m.  x.  c <- x, not not m.  c <- c.  The doubly negated literal is true
/// under a candidate holding m and falsum inside it is masked: the
/// disjunction above the masked node must still learn from it.
fn masked_inside_a_consequent() -> Theory {
    let nodes = vec![
        Node::atom(0),         // m
        Node::atom(1),         // x
        Node::atom(2),         // c
        Node::falsum(),        // 3
        Node::implies(0, 3),   // not m
        Node::or_pair([4, 2]), // not m | c
        Node::implies(1, 5),   // x -> (not m | c)
        Node::implies(2, 2),   // c <- c
    ];
    theory(3, nodes, vec![0, 1, 6, 7])
}

#[test]
fn a_negative_literal_false_under_the_candidate_teaches_nothing_in_the_reduct() {
    let theory = loop_under_constraint();
    let candidate = interpretation(&theory, 0b11);
    let verdict = check_with(
        &theory,
        &candidate,
        SearchMethod::Regions,
        Limits::default(),
        &Cancellation::default(),
    );
    assert!(matches!(verdict, Check::NonMinimal(ref subset) if subset.atoms().count() == 0));
}

#[test]
fn the_region_query_agrees_with_the_reference_on_every_candidate() {
    for theory in [
        mixed(),
        cycle(),
        self_support(),
        loop_under_constraint(),
        masked_inside_a_consequent(),
    ] {
        for mask in 0..(1usize << theory.atom_count()) {
            let candidate = interpretation(&theory, mask);
            let reference = zetesis_ferraris::check(
                &theory,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            let regions = check_with(
                &theory,
                &candidate,
                SearchMethod::Regions,
                Limits::default(),
                &Cancellation::default(),
            );
            let clauses = check_with(
                &theory,
                &candidate,
                SearchMethod::Clauses,
                Limits::default(),
                &Cancellation::default(),
            );
            assert_eq!(
                regions.accepted(),
                reference.accepted(),
                "{mask}: {regions:?}"
            );
            assert_eq!(regions.accepted(), clauses.accepted(), "{mask}");
            match regions {
                Check::Stable => {}
                Check::NotModel => {
                    assert!(matches!(reference.verdict(), Verdict::NotModel { .. }));
                }
                Check::NonMinimal(subset) => {
                    assert!(matches!(reference.verdict(), Verdict::NonMinimal { .. }));
                    let atoms: BTreeSet<usize> = subset.atoms().collect();
                    let candidate_atoms: BTreeSet<usize> = candidate.atoms().collect();
                    assert!(atoms.is_subset(&candidate_atoms) && atoms != candidate_atoms);
                    assert!(
                        zetesis_ferraris::models_reduct(
                            &theory,
                            &candidate,
                            &subset,
                            zetesis_ferraris::Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap()
                    );
                }
                zetesis_sat::Check::Inconclusive(error) => panic!("{mask}: {error}"),
            }
        }
    }
}

#[test]
fn a_work_limit_stops_the_region_query_without_a_verdict() {
    let theory = mixed();
    let candidate = interpretation(&theory, 0b00011);
    let limits = Limits {
        search: SearchLimits {
            max_work: 5,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let verdict = check_with(
        &theory,
        &candidate,
        SearchMethod::Regions,
        limits,
        &Cancellation::default(),
    );
    assert!(matches!(
        verdict,
        Check::Inconclusive(Incomplete::WorkLimit)
    ));
}

/// The same original theory and traversal, with only its retained query ceiling changed.
fn regions(theory: &Theory, max_reduct_bytes: u64) -> StableModels {
    StableModels::with_method(
        theory,
        SearchMethod::Regions,
        Limits {
            max_reduct_bytes,
            ..Limits::default()
        },
        Cancellation::default(),
    )
    .unwrap()
}

/// Measure actual capacity so the boundary includes the host's headers and
/// allocator capacity without assuming their sizes. The first query is for
/// the empty answer set and must grow narrowing scratch beyond truth storage.
fn first_query_workspace_bytes(theory: &Theory) -> u64 {
    let mut search = regions(theory, u64::MAX);
    assert!(search.next().unwrap().unwrap().atoms().next().is_none());
    let statistics = search.statistics();
    assert_eq!(statistics.countermodel_queries, 1);
    let retained = statistics.reduct.peak_workspace_bytes;
    assert!(
        retained > ReductWorkspace::default().retained_bytes() + theory.nodes().len() as u128,
        "the query must retain narrowing scratch"
    );
    u64::try_from(retained).unwrap()
}

#[test]
fn the_first_region_query_refuses_retained_scratch_over_its_limit() {
    let theory = choices(16);
    let required = first_query_workspace_bytes(&theory);
    let limit = required - 1;
    let mut search = regions(&theory, limit);
    assert!(matches!(
        search.next(),
        Some(Err(Incomplete::ReductStorage { required: actual, limit: ceiling }))
            if actual == u128::from(required) && ceiling == u128::from(limit)
    ));
    assert_eq!(search.statistics().stable_models, 0);
    assert_eq!(
        search.statistics().reduct.peak_workspace_bytes,
        u128::from(required)
    );
    assert!(!search.exhausted());
}

#[test]
fn the_first_region_query_accepts_its_exact_retained_limit() {
    let theory = choices(16);
    let required = first_query_workspace_bytes(&theory);
    let mut search = regions(&theory, required);
    assert!(search.next().unwrap().unwrap().atoms().next().is_none());
    assert_eq!(
        search.statistics().reduct.peak_workspace_bytes,
        u128::from(required)
    );
}

#[test]
fn residual_batch_workspace_limits_match_retained_capacity() {
    let theory = choices(16);
    let required = first_query_workspace_bytes(&theory);
    // One residual takes both execution paths: scalar without a pool, and
    // joined completion with a pool, even though only one workspace is needed.
    for workers in [1, 2] {
        for limit in [required - 1, required] {
            let mut search = regions(&theory, limit);
            let mut executor =
                CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
            let result = search.next_batch_with_completion(batch(1), &mut executor, residual);
            if limit < required {
                assert!(matches!(
                    result,
                    Err(BatchError::Search(Incomplete::ReductStorage {
                        required: actual,
                        limit: ceiling,
                    })) if actual == u128::from(required) && ceiling == u128::from(limit)
                ));
                assert_eq!(search.batch_statistics().pending, 1);
                assert_eq!(search.batch_statistics().committed, 0);
                assert_eq!(search.statistics().stable_models, 0);
                assert!(!search.exhausted());
                let completion = executor.last_statistics().unwrap();
                assert_eq!(completion.residual_failed, 1);
                assert_eq!(completion.residual_completed, 0);
            } else {
                let models = result.unwrap();
                assert_eq!(models.len(), 1);
                assert!(models[0].atoms().next().is_none());
                assert_eq!(search.batch_statistics().pending, 0);
                assert_eq!(search.batch_statistics().committed, 1);
            }
            assert_eq!(
                search.statistics().reduct.peak_workspace_bytes,
                u128::from(required)
            );
        }
    }
}
