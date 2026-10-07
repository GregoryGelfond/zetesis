use super::*;

mod test_harness;
mod certificate_order_tests;
mod certificate_limits_tests;
mod batch_orchestration_tests;
mod partial_batch_tests;

#[test]
fn cnf_populations_follow_their_named_owner_allowances() {
    let small = search_limits(&SolveConfig {
        max_candidate_bytes: 1024,
        max_reduct_bytes: 2048,
        ..SolveConfig::default()
    });
    let large = search_limits(&SolveConfig {
        max_candidate_bytes: 2048,
        max_reduct_bytes: 4096,
        ..SolveConfig::default()
    });
    assert_eq!(
        large.admission.max_variables,
        2 * small.admission.max_variables
    );
    assert_eq!(large.admission.max_clauses, 2 * small.admission.max_clauses);
    assert_eq!(
        large.admission.max_literals,
        2 * small.admission.max_literals
    );
    assert_eq!(
        large.reduct_admission.max_variables,
        2 * small.reduct_admission.max_variables
    );
    assert_eq!(
        large.reduct_admission.max_clauses,
        2 * small.reduct_admission.max_clauses
    );
    assert_eq!(
        large.reduct_admission.max_literals,
        2 * small.reduct_admission.max_literals
    );
}

#[test]
fn maximal_cnf_capacity_keeps_double_indices_representable() {
    let limits = cnf_admission(u128::MAX);
    assert!(limits.max_variables.checked_mul(2).is_some());
    assert!(limits.max_literals.checked_mul(2).is_some());
}

#[test]
fn zero_cnf_capacity_admits_no_retained_population() {
    let limits = cnf_admission(0);
    assert_eq!(limits.max_variables, 0);
    assert_eq!(limits.max_clauses, 0);
    assert_eq!(limits.max_literals, 0);
}
