//! Candidate-only constraints preserve the original reduct and transactional state.

use std::collections::BTreeSet;

use proptest::prelude::*;
use zetesis_ferraris::{Interpretation, Node, Theory};
use zetesis_sat::{Cancellation, Incomplete, Limits, StableModels};
use zetesis_theory_support::theories::theory;

/// Enumerate by the clause forms, the subject of the tests below.
fn by_clauses(
    theory: &zetesis_ferraris::Theory,
    limits: zetesis_sat::Limits,
    cancellation: zetesis_sat::Cancellation,
) -> Result<zetesis_sat::StableModels, zetesis_sat::Incomplete> {
    zetesis_sat::StableModels::with_method(
        theory,
        zetesis_sat::SearchMethod::Clauses,
        limits,
        cancellation,
    )
}

fn choices(atoms: usize) -> Theory {
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
    theory(atoms, nodes, roots)
}
fn key(model: &Interpretation) -> Vec<usize> {
    model.atoms().collect()
}
fn remaining(search: &mut StableModels) -> BTreeSet<Vec<usize>> {
    let mut found = BTreeSet::new();
    for model in search.by_ref() {
        assert!(found.insert(key(&model.unwrap())));
    }
    assert!(search.exhausted());
    found
}

#[test]
fn restrictions_keep_the_original_reduct_and_semantic_instance() {
    let original = theory(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(0, 1),
            Node::Implies(1, 0),
        ],
        vec![2, 3],
    );
    let guard = theory(2, vec![Node::Atom(0)], vec![0]);
    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    assert!(search.theory().same_instance(&original));
    search.restrict_candidates(&guard).unwrap();
    // Treating this guard as an original fact would incorrectly support {a,b}.
    assert!(remaining(&mut search).is_empty());
    assert_eq!(search.statistics().countermodels, 1);
    assert_eq!(search.statistics().candidate_restrictions, 1);
}

#[test]
fn refinements_accumulate_and_restarts_preserve_exact_previous_blocks() {
    let original = choices(2);
    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    let first = search.next().unwrap().unwrap();
    assert!(key(&first).is_empty());
    let tautology = theory(2, vec![Node::False, Node::Implies(0, 0)], vec![1]);
    search.restrict_candidates(&tautology).unwrap();
    let positive = theory(2, vec![Node::Atom(0)], vec![0]);
    search.restrict_candidates(&positive).unwrap();
    assert_eq!(
        remaining(&mut search),
        BTreeSet::from([vec![0], vec![0, 1]])
    );
    assert_eq!(search.statistics().candidate_restrictions, 2);
    assert_eq!(
        search.restrict_candidates(&positive),
        Err(Incomplete::ClosedEnumerator)
    );

    let mut search =
        StableModels::new(&original, Limits::default(), Cancellation::default()).unwrap();
    search.restrict_candidates(&positive).unwrap();
    let negative = theory(
        2,
        vec![Node::False, Node::Atom(0), Node::Implies(1, 0)],
        vec![2],
    );
    search.restrict_candidates(&negative).unwrap();
    assert!(remaining(&mut search).is_empty());
}

#[test]
fn a_late_capacity_failure_restores_auxiliary_ids_clauses_and_live_cursor() {
    let original = choices(2);
    let mut limits = Limits::default();
    // The actual disjunction restriction needs four clauses; history is separate.
    limits.admission.max_clauses = 3;
    let mut search = by_clauses(&original, limits, Cancellation::default()).unwrap();
    assert!(key(&search.next().unwrap().unwrap()).is_empty());
    let before = search.statistics();
    let guard = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
    );
    assert!(matches!(
        search.restrict_candidates(&guard),
        Err(Incomplete::Admission(_))
    ));
    assert_eq!(search.statistics().candidate_restrictions, 0);
    assert!(search.statistics().search.work > before.search.work);
    assert_eq!(
        remaining(&mut search),
        BTreeSet::from([vec![0], vec![1], vec![0, 1]])
    );
}

#[test]
fn carrier_control_and_encoding_work_failures_never_claim_completion() {
    let original = choices(2);
    let guard = theory(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
    );
    let cancellation = Cancellation::default();
    let mut search = by_clauses(&original, Limits::default(), cancellation.clone()).unwrap();
    assert_eq!(
        search.restrict_candidates(&choices(3)),
        Err(Incomplete::RestrictionUniverse {
            expected: 2,
            actual: 3
        })
    );
    assert_eq!(search.statistics().candidate_restrictions, 0);
    cancellation.cancel();
    assert_eq!(
        search.restrict_candidates(&guard),
        Err(Incomplete::Cancelled)
    );
    assert!(!search.exhausted());
    assert!(matches!(search.next(), Some(Err(Incomplete::Cancelled))));
    assert!(!search.exhausted());

    let mut measured = by_clauses(&original, Limits::default(), Cancellation::default()).unwrap();
    measured.restrict_candidates(&guard).unwrap();
    let exact = measured.statistics().search.work;
    for ceiling in [exact - 1, exact] {
        let mut limits = Limits::default();
        limits.search.max_work = ceiling;
        let mut search = by_clauses(&original, limits, Cancellation::default()).unwrap();
        let result = search.restrict_candidates(&guard);
        assert_eq!(result.is_ok(), ceiling == exact);
        assert_eq!(
            search.statistics().candidate_restrictions,
            u64::from(ceiling == exact)
        );
        assert_eq!(search.statistics().search.work, ceiling);
        assert!(!search.exhausted());
        assert!(matches!(search.next(), Some(Err(Incomplete::WorkLimit))));
    }
    let zero = choices(0);
    let mut search = by_clauses(&zero, Limits::default(), Cancellation::default()).unwrap();
    search
        .restrict_candidates(&theory(0, vec![Node::False], vec![0]))
        .unwrap();
    assert!(remaining(&mut search).is_empty());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn restricted_results_equal_independent_original_stability_and_guard_truth(
        instructions in prop::collection::vec((0_u8..3, 0_usize..12, 0_usize..12), 0..8),
        assert_roots in prop::collection::vec(any::<bool>(), 1..12),
        positive_loop in any::<bool>(),
    ) {
        let original = if positive_loop {
            theory(3, vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1), Node::Implies(0, 1), Node::Implies(1, 0)], vec![2, 3, 4])
        } else { choices(3) };
        let mut nodes = vec![Node::False, Node::Atom(0), Node::Atom(1), Node::Atom(2)];
        for (operation, a, b) in instructions {
            let a=a % nodes.len();let b=b % nodes.len();
            nodes.push(match operation {0=>Node::And(a,b),1=>Node::Or(a,b),_=>Node::Implies(a,b)});
        }
        let roots=assert_roots.into_iter().enumerate().filter_map(|(index, selected)| selected.then_some(index % nodes.len())).collect();
        let guard=theory(3,nodes,roots);
        let cancellation=Cancellation::default();
        let mut expected=BTreeSet::new();
        for mask in 0..8 {
            let selected:Vec<_>=(0..3).filter(|atom| mask & (1<<atom)!=0).collect();
            let candidate=Interpretation::new(&original,selected.clone()).unwrap();
            let tested=Interpretation::new(&guard,selected).unwrap();
            if zetesis_ferraris::check(&original,&candidate,zetesis_ferraris::Limits::default(),&cancellation).unwrap().accepted()
                && zetesis_ferraris::models(&guard,&tested,zetesis_ferraris::Limits::default(),&cancellation).unwrap() {
                expected.insert(key(&candidate));
            }
        }
        let mut search=StableModels::new(&original,Limits::default(),cancellation).unwrap();
        search.restrict_candidates(&guard).unwrap();
        prop_assert_eq!(remaining(&mut search),expected);
    }
}
