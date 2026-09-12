use super::Cursor;
use crate::search::{Budget, query};
use crate::{
    AdmissionLimits, Cnf, Control, Incomplete, Literal, SearchLimits, SearchStatistics, Solve,
};
use std::collections::BTreeSet;

fn refined_cursor(width: usize) -> Cursor {
    let mut cursor = Cursor::projected(width);
    cursor.restart();
    cursor
}

pub(super) fn exclude(
    cursor: &mut Cursor,
    cnf: &mut Cnf,
    values: &[bool],
    budget: &mut Budget<'_>,
) -> Result<(), Incomplete> {
    let theory = zetesis_ferraris::Theory::new(
        values.len(),
        vec![],
        vec![],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    let candidate = zetesis_ferraris::Interpretation::new(
        &theory,
        (0..values.len()).filter(|index| values[*index]),
    )
    .unwrap();
    cursor.exclude(cnf, &candidate, budget)
}

pub(super) fn budget(control: &Control) -> Budget<'_> {
    Budget {
        quota: crate::search::LocalQuota,
        limits: SearchLimits::default(),
        control,
        statistics: SearchStatistics::default(),
    }
}

fn models(cnf: &Cnf) -> BTreeSet<Vec<bool>> {
    (0..1_usize << cnf.variables())
        .map(|bits| {
            (0..cnf.variables())
                .map(|variable| bits & (1 << variable) != 0)
                .collect::<Vec<_>>()
        })
        .filter(|values| {
            cnf.clauses().all(|clause| {
                clause
                    .iter()
                    .any(|literal| values[literal.variable()] == literal.positive())
            })
        })
        .collect()
}

fn enumerate(cnf: &Cnf, budget: &mut Budget<'_>) -> (BTreeSet<Vec<bool>>, Solve) {
    enumerate_with_cursor(cnf, budget, Cursor::projected(cnf.variables()))
}

fn enumerate_with_cursor(
    cnf: &Cnf,
    budget: &mut Budget<'_>,
    mut cursor: Cursor,
) -> (BTreeSet<Vec<bool>>, Solve) {
    let mut result = BTreeSet::new();
    loop {
        match cursor.query(cnf, budget) {
            Solve::Sat(assignment) => assert!(result.insert(assignment.0)),
            end => return (result, end),
        }
    }
}

#[test]
fn retained_traversal_matches_all_tiny_truth_tables_without_added_blocks() {
    let control = Control::default();
    for variables in 0..=2 {
        let clauses = (0..3_usize.pow(u32::try_from(variables).unwrap()))
            .map(|mut code| {
                (0..variables)
                    .filter_map(|variable| {
                        let digit = code % 3;
                        code /= 3;
                        (digit != 0).then(|| Literal::new(variable, digit == 2))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for selected in 0..1_usize << clauses.len() {
            let cnf = Cnf::new(
                variables,
                clauses
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| selected & (1 << index) != 0)
                    .map(|(_, clause)| clause.clone())
                    .collect(),
                AdmissionLimits::default(),
            )
            .unwrap();
            let (actual, end) = enumerate(&cnf, &mut budget(&control));
            assert!(matches!(end, Solve::Unsat));
            assert_eq!(actual, models(&cnf));
            let (refined, end) =
                enumerate_with_cursor(&cnf, &mut budget(&control), refined_cursor(variables));
            assert!(matches!(end, Solve::Unsat));
            assert_eq!(refined, actual);
        }
    }
    let p = Literal::new(0, true);
    let q = Literal::new(1, true);
    let cnf = Cnf::new(
        2,
        vec![vec![p, p, q], vec![p, p.negated()], vec![p, p, q]],
        AdmissionLimits::default(),
    )
    .unwrap();
    let (actual, end) = enumerate(&cnf, &mut budget(&control));
    assert!(matches!(end, Solve::Unsat));
    assert_eq!(actual, models(&cnf));
}

#[test]
fn exact_projection_blocks_skip_multiple_free_auxiliary_extensions() {
    let mut cnf = Cnf::new(4, vec![], AdmissionLimits::default()).unwrap();
    let control = Control::default();
    let mut budget = budget(&control);
    let mut cursor = Cursor::projected(2);
    let mut projections = BTreeSet::new();
    loop {
        match cursor.query(&cnf, &mut budget) {
            Solve::Sat(assignment) => {
                assert!(projections.insert(assignment.0[..2].to_vec()));
                exclude(&mut cursor, &mut cnf, &assignment.0[..2], &mut budget).unwrap();
            }
            Solve::Unsat => break,
            Solve::Inconclusive(error) => panic!("unexpected stop: {error}"),
        }
    }
    assert_eq!(
        projections,
        BTreeSet::from([
            vec![false, false],
            vec![false, true],
            vec![true, false],
            vec![true, true]
        ])
    );
    assert!(
        budget.statistics.conflicts > 0,
        "duplicate auxiliary leaves must be filtered"
    );
    assert!(matches!(cursor.query(&cnf, &mut budget), Solve::Unsat));
}

#[test]
fn appended_constraints_filter_leaves_without_changing_the_base_watch_registry() {
    let control = Control::default();
    let p = Literal::new(0, true);
    let q = Literal::new(1, true);
    let mut cnf = Cnf::new(3, vec![vec![p, q]], AdmissionLimits::default()).unwrap();
    let mut budget = budget(&control);
    let mut cursor = Cursor::default();
    let Solve::Sat(first) = cursor.query(&cnf, &mut budget) else {
        panic!("first model");
    };
    cnf.append(vec![Literal::new(2, !first.0[2])]).unwrap();
    cnf.append(vec![p.negated(), q.negated()]).unwrap();
    let mut remaining = BTreeSet::new();
    loop {
        match cursor.query(&cnf, &mut budget) {
            Solve::Sat(assignment) => assert!(remaining.insert(assignment.0)),
            Solve::Unsat => break,
            Solve::Inconclusive(error) => panic!("unexpected stop: {error}"),
        }
    }
    let mut expected = models(&cnf);
    expected.remove(&first.0);
    assert_eq!(remaining, expected);

    let mut cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::default();
    assert!(matches!(cursor.query(&cnf, &mut budget), Solve::Sat(_)));
    cnf.append(vec![]).unwrap();
    assert!(matches!(cursor.query(&cnf, &mut budget), Solve::Unsat));
}

#[test]
fn shape_changes_and_resumption_cancellation_are_terminal_inconclusive() {
    let control = Control::default();
    let mut budget = budget(&control);
    let mut cnf = Cnf::new(0, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::default();
    assert!(matches!(cursor.query(&cnf, &mut budget), Solve::Sat(_)));
    cnf.fresh().unwrap();
    for _ in 0..2 {
        assert!(matches!(
            cursor.query(&cnf, &mut budget),
            Solve::Inconclusive(Incomplete::InvalidWitness)
        ));
    }
    let cnf = Cnf::new(0, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::default();
    assert!(matches!(cursor.query(&cnf, &mut budget), Solve::Sat(_)));
    control.cancel();
    for _ in 0..2 {
        assert!(matches!(
            cursor.query(&cnf, &mut budget),
            Solve::Inconclusive(Incomplete::Cancelled)
        ));
    }
}

#[test]
fn omitted_root_assignments_survive_resumption_and_refined_probe_backtracking() {
    let p = |variable| Literal::new(variable, true);
    let cnf = Cnf::new(
        6,
        vec![
            vec![p(0)],
            vec![p(0).negated(), p(1)],
            vec![p(2).negated(), p(3)],
            vec![p(2).negated(), p(3).negated()],
            vec![p(3), p(5)],
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let expected = models(&cnf);
    assert_eq!(expected.len(), 6);
    let control = Control::default();
    for refined in [false, true] {
        let mut cursor = if refined {
            refined_cursor(6)
        } else {
            Cursor::projected(6)
        };
        let mut budget = budget(&control);
        let mut actual = BTreeSet::new();
        loop {
            match cursor.query(&cnf, &mut budget) {
                Solve::Sat(assignment) => {
                    assert_eq!(&assignment.0[..3], &[true, true, false]);
                    let state = cursor.state.as_ref().unwrap();
                    assert!(!state.order.contains(&0));
                    assert!(!state.order.contains(&1));
                    if refined {
                        assert!(!state.order.contains(&2));
                    }
                    assert!(actual.insert(assignment.0));
                }
                Solve::Unsat => break,
                Solve::Inconclusive(error) => panic!("complete tiny traversal: {error}"),
            }
        }
        assert_eq!(actual, expected);
        assert!(matches!(cursor.query(&cnf, &mut budget), Solve::Unsat));
    }
}

#[test]
fn exact_cumulative_work_and_decision_ceilings_include_final_exhaustion() {
    let control = Control::default();
    for variables in [0, 3] {
        let cnf = Cnf::new(variables, vec![], AdmissionLimits::default()).unwrap();
        let mut measured = budget(&control);
        let (expected, end) = enumerate(&cnf, &mut measured);
        assert!(matches!(end, Solve::Unsat));
        let mut exact = budget(&control);
        exact.limits.max_work = measured.statistics.work;
        exact.limits.max_decisions = measured.statistics.decisions;
        let (actual, end) = enumerate(&cnf, &mut exact);
        assert!(matches!(end, Solve::Unsat));
        assert_eq!(actual, expected);
        assert_eq!(exact.statistics, measured.statistics);
        let mut short = budget(&control);
        short.limits.max_work = measured.statistics.work - 1;
        assert!(matches!(
            enumerate(&cnf, &mut short).1,
            Solve::Inconclusive(Incomplete::WorkLimit)
        ));
        assert_eq!(short.statistics.work, short.limits.max_work);
        if measured.statistics.decisions != 0 {
            let mut short = budget(&control);
            short.limits.max_decisions = measured.statistics.decisions - 1;
            assert!(matches!(
                enumerate(&cnf, &mut short).1,
                Solve::Inconclusive(Incomplete::DecisionLimit)
            ));
        }
    }
}

fn blocked_run(retain: bool) -> (BTreeSet<Vec<bool>>, SearchStatistics) {
    let mut cnf = Cnf::new(6, vec![], AdmissionLimits::default()).unwrap();
    let control = Control::default();
    let mut budget = budget(&control);
    let mut cursor = Cursor::default();
    let mut result = BTreeSet::new();
    loop {
        let outcome = if retain {
            cursor.query(&cnf, &mut budget)
        } else {
            query(&cnf, &mut budget)
        };
        match outcome {
            Solve::Sat(assignment) => {
                cnf.append(
                    assignment
                        .0
                        .iter()
                        .enumerate()
                        .map(|(variable, value)| Literal::new(variable, !value))
                        .collect(),
                )
                .unwrap();
                assert!(result.insert(assignment.0));
            }
            Solve::Unsat => return (result, budget.statistics),
            Solve::Inconclusive(error) => panic!("unexpected stop: {error}"),
        }
    }
}

#[test]
fn retained_state_reduces_charged_work_for_complete_independent_choices() {
    let (retained, retained_statistics) = blocked_run(true);
    let (restarted, restarted_statistics) = blocked_run(false);
    assert_eq!(retained.len(), 64);
    assert_eq!(retained, restarted);
    assert!(retained_statistics.work * 2 < restarted_statistics.work);
    assert!(retained_statistics.decisions < restarted_statistics.decisions);
}

#[test]
fn projected_mode_refuses_nonexact_blocks_instead_of_silently_filtering_them() {
    let control = Control::default();
    for clause in [
        vec![Literal::new(0, true)],
        vec![Literal::new(0, true), Literal::new(2, false)],
    ] {
        let mut cnf = Cnf::new(3, vec![], AdmissionLimits::default()).unwrap();
        let mut cursor = Cursor::projected(2);
        let mut charged = budget(&control);
        assert!(matches!(cursor.query(&cnf, &mut charged), Solve::Sat(_)));
        cnf.append(clause).unwrap();
        assert!(matches!(
            cursor.query(&cnf, &mut charged),
            Solve::Inconclusive(Incomplete::InvalidWitness)
        ));
    }
}

#[test]
fn exact_projection_index_including_empty_key_agrees_with_linear_clause_filter() {
    let control = Control::default();
    for width in 0..=4 {
        let mut cnf = Cnf::new(width + 2, vec![], AdmissionLimits::default()).unwrap();
        let mut indexed_cnf = Cnf::new(width + 2, vec![], AdmissionLimits::default()).unwrap();
        let mut linear = Cursor::default();
        let mut indexed = Cursor::projected(width);
        let mut linear_work = budget(&control);
        let mut indexed_work = budget(&control);
        let mut seen = BTreeSet::new();
        loop {
            match (
                linear.query(&cnf, &mut linear_work),
                indexed.query(&indexed_cnf, &mut indexed_work),
            ) {
                (Solve::Sat(left), Solve::Sat(right)) => {
                    assert_eq!(left, right);
                    assert!(seen.insert(left.0[..width].to_vec()));
                    exclude(
                        &mut indexed,
                        &mut indexed_cnf,
                        &left.0[..width],
                        &mut indexed_work,
                    )
                    .unwrap();
                    cnf.append(
                        (0..width)
                            .map(|variable| Literal::new(variable, !left.0[variable]))
                            .collect(),
                    )
                    .unwrap();
                }
                (Solve::Unsat, Solve::Unsat) => break,
                pair => panic!("projection algorithms disagree: {pair:?}"),
            }
        }
        assert_eq!(seen.len(), 1 << width);
    }
}

#[test]
fn interrupted_probe_or_index_never_claims_exhaustion() {
    let control = Control::default();
    let cnf = Cnf::new(
        2,
        vec![
            vec![Literal::new(0, true), Literal::new(1, true)],
            vec![Literal::new(0, true), Literal::new(1, false)],
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut measured = budget(&control);
    let (_, end) = enumerate_with_cursor(&cnf, &mut measured, refined_cursor(2));
    assert!(matches!(end, Solve::Unsat));
    for ceiling in 0..measured.statistics.work {
        let mut bounded = budget(&control);
        bounded.limits.max_work = ceiling;
        let (_, end) = enumerate_with_cursor(&cnf, &mut bounded, refined_cursor(2));
        assert!(matches!(end, Solve::Inconclusive(Incomplete::WorkLimit)));
        assert_eq!(bounded.statistics.work, ceiling);
    }
    let mut cnf = Cnf::new(2, vec![], AdmissionLimits::default()).unwrap();
    let mut cursor = Cursor::projected(2);
    let mut charged = budget(&control);
    let Solve::Sat(model) = cursor.query(&cnf, &mut charged) else {
        panic!("initial model")
    };
    exclude(&mut cursor, &mut cnf, &model.0, &mut charged).unwrap();
    charged.limits.max_work = charged.statistics.work + 3;
    assert!(matches!(
        cursor.query(&cnf, &mut charged),
        Solve::Inconclusive(Incomplete::WorkLimit)
    ));
    charged.limits = SearchLimits::default();
    assert!(matches!(
        cursor.query(&cnf, &mut charged),
        Solve::Inconclusive(Incomplete::WorkLimit)
    ));
}

#[test]
fn refined_regions_probe_while_initial_regions_keep_indexed_complete_search() {
    let control = Control::default();
    let p = Literal::new(0, true);
    let q = Literal::new(1, true);
    let cnf = Cnf::new(
        2,
        vec![vec![p, q], vec![p, q.negated()]],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut initial_budget = budget(&control);
    let mut refined_budget = budget(&control);
    let (initial, initial_end) =
        enumerate_with_cursor(&cnf, &mut initial_budget, Cursor::projected(2));
    let (refined, refined_end) =
        enumerate_with_cursor(&cnf, &mut refined_budget, refined_cursor(2));
    assert!(matches!(initial_end, Solve::Unsat));
    assert!(matches!(refined_end, Solve::Unsat));
    assert_eq!(initial, models(&cnf));
    assert_eq!(initial, refined);
    // Probing proves p at root; initial search establishes it by visiting the
    // failed p=false decision branch instead. Neither path loses a model.
    assert!(refined_budget.statistics.decisions < initial_budget.statistics.decisions);
}
