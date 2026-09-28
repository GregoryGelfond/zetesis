//! Ordering changes traversal only. Independent truth tables and exhaustive
//! reduct enumeration compare complete model identities under variable renaming.

use std::collections::BTreeSet;
use std::time::Instant;

use zetesis_ferraris::{Interpretation, Node, Theory};
use zetesis_sat::{
    AdmissionError, AdmissionLimits, Cancellation, Cnf, Incomplete, Limits, Literal, Resource,
    SearchLimits, Solve, StableModels, solve, solve_with_statistics,
};

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

fn permutations() -> Vec<[usize; 4]> {
    let mut result = Vec::new();
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if order.into_iter().collect::<BTreeSet<_>>().len() == 4 {
                        result.push(order);
                    }
                }
            }
        }
    }
    assert_eq!(result.len(), 24);
    result
}

fn truth_models(clauses: &[Vec<Literal>]) -> BTreeSet<usize> {
    (0..16)
        .filter(|mask| {
            clauses.iter().all(|clause| {
                clause
                    .iter()
                    .any(|literal| (mask & (1 << literal.variable()) != 0) == literal.positive())
            })
        })
        .collect()
}

fn enumerate_renamed(clauses: &[Vec<Literal>], order: [usize; 4], flips: usize) -> BTreeSet<usize> {
    let mut pending: Vec<Vec<_>> = clauses
        .iter()
        .rev()
        .map(|clause| {
            clause
                .iter()
                .map(|literal| {
                    Literal::new(
                        order[literal.variable()],
                        literal.positive() ^ (flips & (1 << literal.variable()) != 0),
                    )
                })
                .collect()
        })
        .collect();
    let mut models = BTreeSet::new();
    loop {
        let cnf = Cnf::new(4, pending.clone(), AdmissionLimits::default())
            .expect("bounded independent clauses and exact model blocks");
        match solve(&cnf, SearchLimits::default(), &Cancellation::default()) {
            Solve::Sat(assignment) => {
                let mut original = 0;
                for (variable, &renamed) in order.iter().enumerate() {
                    let value = assignment.value(renamed).expect("total assignment")
                        ^ (flips & (1 << variable) != 0);
                    if value {
                        original |= 1 << variable;
                    }
                }
                assert!(models.insert(original), "no repeated complete assignment");
                pending.push(
                    (0..4)
                        .map(|variable| {
                            Literal::new(variable, !assignment.value(variable).unwrap())
                        })
                        .collect(),
                );
            }
            Solve::Unsat => return models,
            Solve::Inconclusive(error) => panic!("tiny complete search: {error}"),
        }
    }
}

fn next(state: &mut u64, bound: u64) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    usize::try_from(*state % bound).expect("small generated index")
}

#[test]
fn complete_truth_tables_survive_variable_permutations_and_polarity_renaming() {
    let mut state = 0x43f1_a098_41b5_28dd;
    let permutations = permutations();
    for case in 0..64 {
        let mut clauses = Vec::new();
        for _ in 0..(case % 13) {
            let length = 2 + next(&mut state, 3);
            let clause = (0..length)
                .map(|_| Literal::new(next(&mut state, 4), next(&mut state, 2) != 0))
                .collect();
            clauses.push(clause);
        }
        if case % 8 == 0 {
            clauses.push(vec![Literal::new(case % 4, case % 16 == 0)]);
        }
        let expected = truth_models(&clauses);
        for &order in &permutations {
            assert_eq!(
                enumerate_renamed(&clauses, order, case % 16),
                expected,
                "case {case}, renaming {order:?}: {clauses:?}"
            );
        }
    }
}

fn stable_models(theory: &Theory) -> BTreeSet<Vec<usize>> {
    let mut search = StableModels::new(theory, Limits::default(), Cancellation::default())
        .expect("bounded formula encoding");
    let mut result = BTreeSet::new();
    for model in search.by_ref() {
        assert!(result.insert(model.expect("complete search").atoms().collect()));
    }
    assert!(search.exhausted());
    result
}

#[test]
fn renamed_ferraris_models_match_independent_subset_enumeration() {
    let original = [
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::Atom(3),
        Node::False,
        Node::Implies(0, 4),
        Node::Implies(1, 4),
        Node::Or(0, 5),
        Node::Or(1, 6),
        Node::Or(0, 1),
        Node::Implies(9, 2),
        Node::Implies(2, 4),
    ];
    for roots_mask in 0..16 {
        let roots: Vec<_> = [7, 8, 10, 11]
            .into_iter()
            .enumerate()
            .filter(|&(index, _)| roots_mask & (1 << index) != 0)
            .map(|(_, root)| root)
            .collect();
        for order in permutations() {
            let nodes = original
                .iter()
                .map(|&node| match node {
                    Node::Atom(atom) => Node::Atom(order[atom]),
                    other => other,
                })
                .collect();
            let theory = Theory::new(
                4,
                nodes,
                roots.clone(),
                zetesis_ferraris::AdmissionLimits::default(),
            )
            .expect("renamed finite formula");
            let mut expected = BTreeSet::new();
            for mask in 0..16 {
                let candidate =
                    Interpretation::new(&theory, (0..4).filter(|&atom| mask & (1 << atom) != 0))
                        .expect("same-theory interpretation");
                if zetesis_ferraris::check(
                    &theory,
                    &candidate,
                    zetesis_ferraris::Limits::default(),
                    &Cancellation::default(),
                )
                .expect("complete independent reduct enumeration")
                .accepted()
                {
                    expected.insert(candidate.atoms().collect());
                }
            }
            assert_eq!(stable_models(&theory), expected);
        }
    }
}

#[test]
fn work_decision_and_control_stops_remain_inconclusive_with_exact_accounting() {
    let p = |variable| Literal::new(variable, true);
    let clauses = [
        vec![p(5), p(1)],
        vec![p(5), p(1).negated()],
        vec![p(5).negated(), p(4)],
        vec![p(5).negated(), p(4).negated()],
    ];
    for length in [3, 4] {
        let cnf = Cnf::new(8, clauses[..length].to_vec(), AdmissionLimits::default()).unwrap();
        let (outcome, statistics) =
            solve_with_statistics(&cnf, SearchLimits::default(), &Cancellation::default());
        assert_eq!(matches!(outcome, Solve::Sat(_)), length == 3);
        assert_eq!(matches!(outcome, Solve::Unsat), length == 4);
        assert!(statistics.decisions > 0);
        let exact = SearchLimits {
            max_work: statistics.work,
            max_decisions: statistics.decisions,
        };
        let (complete, exact_statistics) =
            solve_with_statistics(&cnf, exact, &Cancellation::default());
        assert!(!matches!(complete, Solve::Inconclusive(_)));
        assert_eq!(exact_statistics, statistics);
        for max_work in [0, 1, statistics.work / 2, statistics.work - 1] {
            let (limited, accounting) = solve_with_statistics(
                &cnf,
                SearchLimits { max_work, ..exact },
                &Cancellation::default(),
            );
            assert!(matches!(
                limited,
                Solve::Inconclusive(Incomplete::WorkLimit)
            ));
            assert_eq!(accounting.work, max_work);
        }
        let (limited, accounting) = solve_with_statistics(
            &cnf,
            SearchLimits {
                max_decisions: statistics.decisions - 1,
                ..exact
            },
            &Cancellation::default(),
        );
        assert!(matches!(
            limited,
            Solve::Inconclusive(Incomplete::DecisionLimit)
        ));
        assert_eq!(accounting.decisions, statistics.decisions - 1);
        let cancelled = Cancellation::default();
        cancelled.cancel();
        for (cancellation, expected) in [
            (cancelled, Incomplete::Cancelled),
            (
                Cancellation::with_deadline(Instant::now()).unwrap(),
                Incomplete::Deadline,
            ),
        ] {
            let (limited, accounting) = solve_with_statistics(&cnf, exact, &cancellation);
            assert!(matches!(limited, Solve::Inconclusive(reason) if reason == expected));
            assert_eq!(accounting.work, 0);
        }
    }
}

#[test]
fn compacted_aliases_need_no_auxiliary_but_remaining_gate_needs_fresh_capacity() {
    let aliases = Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::False,
            Node::Implies(0, 2),
            Node::Implies(1, 2),
            Node::Or(0, 3),
            Node::Or(1, 4),
        ],
        vec![5, 6],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let limits = Limits {
        admission: AdmissionLimits {
            max_variables: 2,
            ..AdmissionLimits::default()
        },
        ..Limits::default()
    };
    let mut search = by_clauses(&aliases, limits, Cancellation::default()).unwrap();
    let models: BTreeSet<Vec<_>> = search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect();
    assert!(search.exhausted());
    assert_eq!(
        models,
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
    let gate = Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::And(0, 1)],
        vec![2],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        by_clauses(&gate, limits, Cancellation::default()),
        Err(Incomplete::Admission(AdmissionError::Limit {
            resource: Resource::Variables,
            observed: 3,
            limit: 2
        }))
    ));
    let mut search = by_clauses(
        &gate,
        Limits {
            admission: AdmissionLimits {
                max_variables: 3,
                ..AdmissionLimits::default()
            },
            ..Limits::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        search.next().unwrap().unwrap().atoms().collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!(search.next().is_none());
    assert!(search.exhausted());
}
