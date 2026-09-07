//! Independent final-witness validation and exact inspected-prefix accounting.

use super::{Budget, State};
use crate::{
    AdmissionLimits, Assignment, Cnf, Control, Incomplete, Literal, SearchLimits, SearchStatistics,
};

fn cnf(variables: usize, clauses: Vec<Vec<Literal>>) -> Cnf {
    Cnf::new(variables, clauses, AdmissionLimits::default()).unwrap()
}

fn budget(control: &Control, max_work: u64) -> Budget<'_> {
    Budget {
        quota: crate::search::LocalQuota,
        limits: SearchLimits {
            max_work,
            ..SearchLimits::default()
        },
        control,
        statistics: SearchStatistics::default(),
    }
}

fn state(cnf: &Cnf, values: &[Option<bool>]) -> State {
    assert_eq!(cnf.variables(), values.len());
    let control = Control::default();
    let mut state = State::new(cnf, &mut budget(&control, u64::MAX)).unwrap();
    state.values.copy_from_slice(values);
    state
}

fn validate(
    cnf: &Cnf,
    values: &[Option<bool>],
    max_work: u64,
) -> (Result<Assignment, Incomplete>, u64) {
    let state = state(cnf, values);
    let control = Control::default();
    let mut budget = budget(&control, max_work);
    let result = state.finish(cnf, &mut budget);
    (result, budget.statistics.work)
}

#[test]
fn satisfying_prefix_never_bypasses_total_assignment_validation() {
    let cnf = cnf(3, vec![vec![Literal::new(0, true), Literal::new(1, true)]]);
    // The unassigned variable does not occur in any clause, and the first
    // literal is already true. It still prevents a complete witness.
    let values = [Some(true), Some(false), None];
    let (result, work) = validate(&cnf, &values, 3);
    assert!(matches!(result, Err(Incomplete::InvalidWitness)));
    assert_eq!(work, 3);
    let (result, work) = validate(&cnf, &values, 2);
    assert!(matches!(result, Err(Incomplete::WorkLimit)));
    assert_eq!(work, 2);
}

#[test]
fn exact_first_true_prefix_fits_its_work_ceiling() {
    let cnf = cnf(
        4,
        vec![
            (0..4)
                .map(|variable| Literal::new(variable, true))
                .collect(),
        ],
    );
    for first_true in 0..4 {
        let mut values = [Some(false); 4];
        values[first_true] = Some(true);
        let required = 4 + u64::try_from(first_true).unwrap() + 1;
        let (result, work) = validate(&cnf, &values, required);
        assert_eq!(result.unwrap().0, values.map(Option::unwrap));
        assert_eq!(work, required);
        let (result, work) = validate(&cnf, &values, required - 1);
        assert!(matches!(result, Err(Incomplete::WorkLimit)));
        assert_eq!(work, required - 1);
    }
    let (result, work) = validate(&cnf, &[Some(false); 4], 8);
    assert!(matches!(result, Err(Incomplete::InvalidWitness)));
    assert_eq!(work, 8, "a false clause needs every literal checked");
}

#[test]
fn every_clause_is_required_and_empty_clauses_are_false() {
    let empty_formula = cnf(0, Vec::new());
    let (result, work) = validate(&empty_formula, &[], 0);
    assert!(result.unwrap().0.is_empty());
    assert_eq!(work, 0);
    let empty_clause = cnf(0, vec![Vec::new()]);
    let (result, work) = validate(&empty_clause, &[], 0);
    assert!(matches!(result, Err(Incomplete::InvalidWitness)));
    assert_eq!(work, 0);
    let cnf = cnf(
        2,
        vec![vec![Literal::new(0, true)], vec![Literal::new(1, true)]],
    );
    let (result, work) = validate(&cnf, &[Some(true), Some(false)], 4);
    assert!(matches!(result, Err(Incomplete::InvalidWitness)));
    assert_eq!(
        work, 4,
        "a true first clause cannot skip a false later clause"
    );
}

#[test]
fn cancellation_is_polled_before_accepting_a_nonempty_witness() {
    let cnf = cnf(2, vec![vec![Literal::new(0, true), Literal::new(1, true)]]);
    let state = state(&cnf, &[Some(true), Some(false)]);
    let control = Control::default();
    control.cancel();
    let mut budget = budget(&control, 3);
    assert!(matches!(
        state.finish(&cnf, &mut budget),
        Err(Incomplete::Cancelled)
    ));
    assert_eq!(budget.statistics.work, 0);
}

fn canonical_clauses(variables: usize) -> Vec<Vec<Literal>> {
    (0..3_usize.pow(u32::try_from(variables).unwrap()))
        .map(|mut code| {
            (0..variables)
                .filter_map(|variable| {
                    let digit = code % 3;
                    code /= 3;
                    (digit != 0).then(|| Literal::new(variable, digit == 2))
                })
                .collect()
        })
        .collect()
}

#[test]
fn every_complete_small_witness_matches_independent_clause_truth() {
    let mut checked = 0;
    for variables in 0..=3 {
        let clauses = canonical_clauses(variables);
        for first in &clauses {
            for second in &clauses {
                let cnf = cnf(variables, vec![first.clone(), second.clone()]);
                for bits in 0..1_usize << variables {
                    let values: Vec<_> = (0..variables)
                        .map(|variable| bits & (1 << variable) != 0)
                        .collect();
                    let expected = [&first, &second].iter().all(|clause| {
                        clause
                            .iter()
                            .any(|literal| values[literal.variable()] == literal.positive())
                    });
                    let proposed: Vec<_> = values.iter().copied().map(Some).collect();
                    let (result, _) = validate(&cnf, &proposed, u64::MAX);
                    if expected {
                        assert_eq!(result.unwrap().0, values);
                    } else {
                        assert!(matches!(result, Err(Incomplete::InvalidWitness)));
                    }
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 6_175);
}
