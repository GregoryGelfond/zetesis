//! Independent truth-table and resource-boundary checks for native CNF search.
//!
//! These references evaluate literal truth directly and enumerate interpretations;
//! they do not reuse the solver's propagation, branching, or backtracking logic.

use std::time::Instant;

use zetesis_cpu::Control;
use zetesis_sat::{AdmissionLimits, Cnf, Incomplete, Literal, SearchLimits, Solve, solve};

fn literal(variable: usize, positive: bool) -> Literal {
    Literal::new(variable, positive)
}

fn generous_search() -> SearchLimits {
    SearchLimits {
        max_work: 100_000_000,
        max_decisions: 1_000_000,
    }
}

fn formula(variables: usize, clauses: Vec<Vec<Literal>>) -> Cnf {
    Cnf::new(variables, clauses, AdmissionLimits::default())
        .expect("small, well-formed reference formula")
}

fn satisfies(clauses: &[Vec<Literal>], interpretation: usize) -> bool {
    clauses.iter().all(|clause| {
        clause
            .iter()
            .any(|entry| (interpretation & (1usize << entry.variable()) != 0) == entry.positive())
    })
}

fn reference_has_model(variables: usize, clauses: &[Vec<Literal>]) -> bool {
    assert!(variables < usize::BITS as usize);
    (0..(1usize << variables)).any(|interpretation| satisfies(clauses, interpretation))
}

fn assert_truth_table_agreement(variables: usize, clauses: &[Vec<Literal>]) {
    let expected = reference_has_model(variables, clauses);
    let cnf = formula(variables, clauses.to_vec());
    assert_eq!(cnf.variables(), variables);
    for interpretation in 0..(1usize << variables) {
        assert_eq!(
            satisfies(clauses, interpretation),
            cnf.clauses().all(|clause| clause
                .iter()
                .any(|entry| (interpretation & (1usize << entry.variable()) != 0)
                    == entry.positive())),
            "admission changed truth at interpretation {interpretation}: {clauses:?}"
        );
    }
    match solve(&cnf, generous_search(), &Control::default()) {
        Solve::Sat(assignment) => {
            assert!(
                expected,
                "solver accepted an unsatisfiable formula: {clauses:?}"
            );
            for variable in 0..variables {
                assert!(assignment.value(variable).is_some());
            }
            assert_eq!(assignment.value(variables), None);
            assert!(clauses.iter().all(|clause| {
                clause
                    .iter()
                    .any(|entry| assignment.value(entry.variable()) == Some(entry.positive()))
            }));
        }
        Solve::Unsat => assert!(
            !expected,
            "solver rejected a satisfiable formula: {clauses:?}"
        ),
        Solve::Inconclusive(reason) => panic!("tiny formula did not finish: {reason:?}"),
    }
}

/// Every non-tautological clause has one of three states for each variable:
/// absent, negative, or positive. State zero includes the empty clause.
fn canonical_clauses(variables: usize) -> Vec<Vec<Literal>> {
    let count = 3usize.pow(u32::try_from(variables).expect("tiny variable count"));
    (0..count)
        .map(|mut code| {
            let mut clause = Vec::new();
            for variable in 0..variables {
                match code % 3 {
                    1 => clause.push(literal(variable, false)),
                    2 => clause.push(literal(variable, true)),
                    _ => {}
                }
                code /= 3;
            }
            clause
        })
        .collect()
}

#[test]
fn every_canonical_cnf_through_two_variables_matches_truth_tables() {
    let mut checked = 0;
    for variables in 0..=2 {
        let possible = canonical_clauses(variables);
        for selection in 0..(1usize << possible.len()) {
            let clauses: Vec<_> = possible
                .iter()
                .enumerate()
                .filter(|(index, _)| selection & (1usize << index) != 0)
                .map(|(_, clause)| clause.clone())
                .collect();
            assert_truth_table_agreement(variables, &clauses);
            checked += 1;
        }
    }
    assert_eq!(checked, 522);
}

#[test]
fn every_three_variable_cnf_with_at_most_three_distinct_clauses_matches() {
    let possible = canonical_clauses(3);
    let mut checked = 1;
    assert_truth_table_agreement(3, &[]);
    for (first_index, first) in possible.iter().enumerate() {
        assert_truth_table_agreement(3, std::slice::from_ref(first));
        checked += 1;
        for (second_index, second) in possible.iter().enumerate().skip(first_index + 1) {
            assert_truth_table_agreement(3, &[first.clone(), second.clone()]);
            checked += 1;
            for third in possible.iter().skip(second_index + 1) {
                assert_truth_table_agreement(3, &[first.clone(), second.clone(), third.clone()]);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3_304);
}

fn sample(state: &mut u64, bound: usize) -> usize {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    usize::try_from((*state >> 32) % u64::try_from(bound).expect("small sample bound"))
        .expect("bounded sample fits usize")
}

#[test]
fn deterministic_noncanonical_formulas_match_truth_tables() {
    let mut state = 0x794c_a22b_50f8_e7d3;
    for case in 0..1_024 {
        let variables = 1 + sample(&mut state, 6);
        let clause_count = sample(&mut state, 20);
        let mut clauses = Vec::new();
        for _ in 0..clause_count {
            // Most samples exclude empty clauses so search and backtracking
            // receive substantial coverage rather than immediate conflicts.
            let length = if case % 8 == 0 {
                sample(&mut state, 9)
            } else {
                1 + sample(&mut state, 8)
            };
            let clause = (0..length)
                .map(|_| literal(sample(&mut state, variables), sample(&mut state, 2) != 0))
                .collect();
            clauses.push(clause);
        }
        assert_truth_table_agreement(variables, &clauses);
    }
}

#[test]
fn deterministic_distinct_three_sat_formulas_match_truth_tables() {
    let mut state = 0x9386_23c4_a72e_f951;
    for _ in 0..1_024 {
        let variables = 3 + sample(&mut state, 5);
        let clause_count = sample(&mut state, 40);
        let mut clauses = Vec::new();
        for _ in 0..clause_count {
            let mut available: Vec<_> = (0..variables).collect();
            let clause = (0..3)
                .map(|_| {
                    let index = sample(&mut state, available.len());
                    let variable = available.swap_remove(index);
                    literal(variable, sample(&mut state, 2) != 0)
                })
                .collect();
            clauses.push(clause);
        }
        assert_truth_table_agreement(variables, &clauses);
    }
}

#[test]
fn duplicate_literals_tautologies_and_repeated_clauses_preserve_truth() {
    let a = literal(0, true);
    let b = literal(1, true);
    for clauses in [
        vec![vec![a, a], vec![a.negated()]],
        vec![vec![a, a.negated()], vec![b, b.negated()]],
        vec![vec![a, b], vec![a, b], vec![a.negated()], vec![b.negated()]],
        vec![vec![a, a, b, b], vec![a.negated(), a.negated()]],
        vec![vec![], vec![a, a.negated()]],
    ] {
        assert_truth_table_agreement(2, &clauses);
    }
}

#[test]
fn literal_accessors_preserve_variable_and_polarity() {
    for variable in [0, 1, 63, 64, usize::MAX] {
        for positive in [false, true] {
            let entry = literal(variable, positive);
            assert_eq!(entry.variable(), variable);
            assert_eq!(entry.positive(), positive);
            assert_eq!(entry.negated().variable(), variable);
            assert_eq!(entry.negated().positive(), !positive);
            assert_eq!(entry.negated().negated().positive(), positive);
        }
    }
}

#[test]
fn borrowed_clauses_preserve_empty_positions_and_canonical_order() {
    let cnf = formula(
        3,
        vec![
            vec![],
            vec![literal(2, true), literal(0, false), literal(2, true)],
            vec![literal(1, false), literal(1, true)],
            vec![],
            vec![literal(1, true)],
        ],
    );
    let expected = [
        vec![],
        vec![literal(0, false), literal(2, true)],
        vec![],
        vec![literal(1, true)],
    ];
    let mut clauses = cnf.clauses();
    assert_eq!(clauses.len(), expected.len());
    for (index, expected) in expected.iter().enumerate() {
        assert_eq!(clauses.size_hint(), (4 - index, Some(4 - index)));
        let clause = clauses.next().unwrap();
        assert_eq!(Some(clause), cnf.clause(index));
        assert_eq!(clause.len(), expected.len());
        assert_eq!(clause.is_empty(), expected.is_empty());
        assert!(clause.iter().eq(expected.iter().copied()));
        assert_eq!(clause.get(expected.len()), None);
        assert_eq!(clause.get(usize::MAX), None);
        for (position, &literal) in expected.iter().enumerate() {
            assert_eq!(clause.get(position), Some(literal));
        }
    }
    assert!(clauses.is_empty());
    assert_eq!(clauses.next(), None);
    assert_eq!(clauses.next(), None);
    assert_eq!(cnf.clause(4), None);
    assert_eq!(cnf.clause(usize::MAX), None);
}

#[test]
fn packing_preserves_the_largest_admitted_variable() {
    let variables = usize::MAX / 2;
    let limits = AdmissionLimits {
        max_variables: variables,
        max_clauses: 2,
        max_literals: 2,
    };
    let expected = [literal(variables - 1, false), literal(variables - 1, true)];
    let cnf = Cnf::new(variables, expected.map(|entry| vec![entry]).into(), limits).unwrap();
    assert!(
        cnf.clauses()
            .map(|clause| clause.get(0).unwrap())
            .eq(expected)
    );
    for invalid in [variables, usize::MAX] {
        assert_eq!(
            Cnf::new(variables, vec![vec![literal(invalid, true)]], limits).unwrap_err(),
            zetesis_sat::AdmissionError::Variable {
                variable: invalid,
                variables
            }
        );
    }
}

#[test]
fn admission_accepts_exact_ceilings_and_rejects_each_excess() {
    let clauses = vec![
        vec![literal(0, true), literal(1, false)],
        vec![literal(0, false)],
    ];
    let exact = AdmissionLimits {
        max_variables: 2,
        max_clauses: 2,
        max_literals: 3,
    };
    assert!(Cnf::new(2, clauses.clone(), exact).is_ok());
    for limits in [
        AdmissionLimits {
            max_variables: 1,
            ..exact
        },
        AdmissionLimits {
            max_clauses: 1,
            ..exact
        },
        AdmissionLimits {
            max_literals: 2,
            ..exact
        },
    ] {
        assert!(Cnf::new(2, clauses.clone(), limits).is_err());
    }
    assert!(Cnf::new(2, vec![vec![literal(2, true)]], exact).is_err());
    assert!(Cnf::new(0, vec![vec![literal(0, false)]], exact).is_err());
    assert!(Cnf::new(2, vec![vec![literal(usize::MAX, false)]], exact).is_err());
    let zero = AdmissionLimits {
        max_variables: 0,
        max_clauses: 0,
        max_literals: 0,
    };
    assert!(Cnf::new(0, vec![], zero).is_ok());
    assert!(Cnf::new(0, vec![vec![]], zero).is_err());
    assert!(
        Cnf::new(
            usize::MAX,
            vec![],
            AdmissionLimits {
                max_variables: usize::MAX,
                ..zero
            },
        )
        .is_err()
    );
    // Canonicalization must not erase the cost of the submitted input.
    let one = AdmissionLimits {
        max_variables: 1,
        max_clauses: 1,
        max_literals: 1,
    };
    assert!(Cnf::new(1, vec![vec![literal(0, true), literal(0, true)]], one).is_err());
    assert!(Cnf::new(1, vec![vec![literal(0, true), literal(0, false)]], one).is_err());
}

#[test]
fn cancellation_and_expired_deadline_are_inconclusive_even_for_empty_cnf() {
    let cnf = formula(0, vec![]);
    let cancelled = Control::default();
    cancelled.cancel();
    assert!(matches!(
        solve(&cnf, generous_search(), &cancelled),
        Solve::Inconclusive(Incomplete::Cancelled)
    ));
    let expired = Control::with_deadline(Instant::now());
    assert!(matches!(
        solve(&cnf, generous_search(), &expired),
        Solve::Inconclusive(Incomplete::Deadline)
    ));
    expired.cancel();
    assert!(matches!(
        solve(&cnf, generous_search(), &expired),
        Solve::Inconclusive(Incomplete::Cancelled)
    ));
}

#[test]
fn zero_decisions_allows_unit_proofs_but_not_a_required_choice() {
    let a = literal(0, true);
    let b = literal(1, true);
    let without_decisions = SearchLimits {
        max_decisions: 0,
        ..generous_search()
    };
    assert!(matches!(
        solve(
            &formula(1, vec![vec![a]]),
            without_decisions,
            &Control::default()
        ),
        Solve::Sat(_)
    ));
    assert!(matches!(
        solve(
            &formula(1, vec![vec![a], vec![a.negated()]]),
            without_decisions,
            &Control::default()
        ),
        Solve::Unsat
    ));
    let exclusive = formula(2, vec![vec![a, b], vec![a.negated(), b.negated()]]);
    assert!(matches!(
        solve(&exclusive, without_decisions, &Control::default()),
        Solve::Inconclusive(Incomplete::DecisionLimit)
    ));
    assert!(matches!(
        solve(
            &exclusive,
            SearchLimits {
                max_decisions: 1,
                ..generous_search()
            },
            &Control::default()
        ),
        Solve::Sat(_)
    ));
}

#[test]
fn work_limits_are_exact_for_sat_and_unsat_proofs() {
    let a = literal(0, true);
    let b = literal(1, true);
    for clauses in [
        vec![vec![a, b], vec![a.negated(), b.negated()]],
        vec![
            vec![a, b],
            vec![a.negated(), b],
            vec![a, b.negated()],
            vec![a.negated(), b.negated()],
        ],
    ] {
        let expected_sat = reference_has_model(2, &clauses);
        let cnf = formula(2, clauses);
        let run = |max_work| {
            solve(
                &cnf,
                SearchLimits {
                    max_work,
                    ..generous_search()
                },
                &Control::default(),
            )
        };
        let completed = |result: &Solve| {
            matches!(
                (result, expected_sat),
                (Solve::Sat(_), true) | (Solve::Unsat, false)
            )
        };
        assert!(matches!(run(0), Solve::Inconclusive(Incomplete::WorkLimit)));
        let mut insufficient = 0;
        let mut sufficient = 10_000;
        assert!(completed(&run(sufficient)));
        while sufficient - insufficient > 1 {
            let middle = insufficient + (sufficient - insufficient) / 2;
            match run(middle) {
                Solve::Inconclusive(Incomplete::WorkLimit) => insufficient = middle,
                result if completed(&result) => sufficient = middle,
                result => panic!("unexpected result at work ceiling {middle}: {result:?}"),
            }
        }
        assert!(completed(&run(sufficient)));
        assert!(matches!(
            run(sufficient - 1),
            Solve::Inconclusive(Incomplete::WorkLimit)
        ));
    }
}

#[test]
fn deep_search_uses_a_heap_trail_on_a_small_thread_stack() {
    const PAIRS: usize = 1_024;
    let mut clauses = Vec::with_capacity(PAIRS * 2);
    for pair in 0..PAIRS {
        let left = literal(pair * 2, true);
        let right = literal(pair * 2 + 1, true);
        clauses.push(vec![left, right]);
        clauses.push(vec![left.negated(), right.negated()]);
    }
    let cnf = formula(PAIRS * 2, clauses);
    let worker = std::thread::Builder::new()
        .name("bounded-iterative-sat".to_owned())
        .stack_size(64 * 1_024)
        .spawn(move || {
            let result = solve(&cnf, generous_search(), &Control::default());
            let Solve::Sat(assignment) = result else {
                panic!("deep independent choices must be satisfiable: {result:?}");
            };
            for pair in 0..PAIRS {
                assert_ne!(assignment.value(pair * 2), assignment.value(pair * 2 + 1));
            }
        })
        .expect("spawn bounded-stack reference test");
    worker
        .join()
        .expect("iterative search must not overflow the thread stack");
}
