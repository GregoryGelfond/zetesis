//! Final witness assembly charges one unit per variable and nothing per clause:
//! clause satisfaction is the watch scheme's invariant, stated against truth
//! tables in `tests/integration/cnf.rs` and asserted here in debug builds.

use super::{Budget, State};
use crate::{
    AdmissionLimits, Assignment, Cancellation, Cnf, Incomplete, Literal, SearchLimits,
    SearchStatistics,
};

fn cnf(variables: usize, clauses: Vec<Vec<Literal>>) -> Cnf {
    Cnf::new(variables, clauses, AdmissionLimits::default()).unwrap()
}

fn budget(cancellation: &Cancellation, max_work: u64) -> Budget<'_> {
    Budget {
        quota: crate::search::LocalQuota,
        limits: SearchLimits {
            max_work,
            ..SearchLimits::default()
        },
        cancellation,
        statistics: SearchStatistics::default(),
    }
}

fn state(cnf: &Cnf, values: &[Option<bool>]) -> State {
    assert_eq!(cnf.variables(), values.len());
    let cancellation = Cancellation::default();
    let mut state = State::new(cnf, &mut budget(&cancellation, u64::MAX)).unwrap();
    state.values.copy_from_slice(values);
    state
}

fn assemble(
    cnf: &Cnf,
    values: &[Option<bool>],
    max_work: u64,
) -> (Result<Assignment, Incomplete>, u64) {
    let state = state(cnf, values);
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, max_work);
    let result = state.finish(cnf, &mut budget);
    (result, budget.statistics.work)
}

#[test]
fn an_unassigned_variable_prevents_a_complete_witness() {
    let formula = cnf(3, vec![vec![Literal::new(0, true), Literal::new(1, true)]]);
    // The unassigned variable does not occur in any clause and the clause is
    // already true. A witness is still a total assignment.
    let values = [Some(true), Some(false), None];
    let (result, work) = assemble(&formula, &values, 3);
    assert!(matches!(result, Err(Incomplete::InvalidWitness)));
    assert_eq!(work, 3);
    let (result, work) = assemble(&formula, &values, 2);
    assert!(matches!(result, Err(Incomplete::WorkLimit)));
    assert_eq!(work, 2);
}

#[test]
fn assembly_charges_one_unit_per_variable_whatever_the_clauses() {
    let wide = cnf(
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
        let (result, work) = assemble(&wide, &values, 4);
        assert_eq!(result.unwrap().0, values.map(Option::unwrap));
        assert_eq!(work, 4, "no clause is rescanned");
        let (result, work) = assemble(&wide, &values, 3);
        assert!(matches!(result, Err(Incomplete::WorkLimit)));
        assert_eq!(work, 3);
    }
    let empty_formula = cnf(0, Vec::new());
    let (result, work) = assemble(&empty_formula, &[], 0);
    assert!(result.unwrap().0.is_empty());
    assert_eq!(work, 0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "falsifies a base clause")]
fn a_debug_build_asserts_the_watch_invariant_on_every_witness() {
    let units = cnf(
        2,
        vec![vec![Literal::new(0, true)], vec![Literal::new(1, true)]],
    );
    // Propagation can never leave this state; assembling it is an internal
    // error that the debug build reports rather than a verdict.
    let _ = assemble(&units, &[Some(true), Some(false)], 4);
}

#[test]
fn cancellation_is_polled_before_accepting_a_nonempty_witness() {
    let formula = cnf(2, vec![vec![Literal::new(0, true), Literal::new(1, true)]]);
    let state = state(&formula, &[Some(true), Some(false)]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut budget = budget(&cancellation, 3);
    assert!(matches!(
        state.finish(&formula, &mut budget),
        Err(Incomplete::Cancelled)
    ));
    assert_eq!(budget.statistics.work, 0);
}
