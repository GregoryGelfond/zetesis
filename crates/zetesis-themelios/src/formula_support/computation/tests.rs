use crate::test_support::location;
use zetesis_core::Value;

use super::*;
use crate::formula_support::testing::Fixture;

#[test]
fn frozen_computation_resolves_admitted_identity() {
    let limits = FormulaLimits::default();
    let mut fixture = Fixture::default();
    let key = fixture.with(location(), |_, computation, counters| {
        computation
            .import(
                (&Value::String("admitted text".into())).into(),
                &limits,
                counters,
                location(),
            )
            .unwrap()
    });
    let (completed, _) = fixture.finish(location());
    // A checker has its own accounting workspace. Its identity authority is
    // borrowed from completed admission and cannot grow during checking.
    let mut counters = Counters::default();
    let support = completed
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let queries = support
        .queries(crate::JoinStrategy::Indexed, &limits, &counters, location())
        .unwrap();
    let mut computation = queries.computation(location()).unwrap();
    let found = computation
        .import(
            (&Value::String("admitted text".into())).into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    assert_eq!(
        computation.read().term(&found).unwrap(),
        computation.read().term(&key).unwrap()
    );
}

#[test]
fn frozen_computation_refuses_unadmitted_identity() {
    let (completed, _) = Fixture::default().finish(location());
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let support = completed
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let queries = support
        .queries(crate::JoinStrategy::Indexed, &limits, &counters, location())
        .unwrap();
    let mut computation = queries.computation(location()).unwrap();
    let result = computation.number(731, &limits, &mut counters, location());
    assert!(matches!(result, Err(FormulaFailure::UnadmittedTerm { .. })));
}

#[test]
fn lease_allowance_keeps_query_metadata_charged() {
    Fixture::default().with(location(), |support, computation, _counters| {
        let lease = computation.lease();
        let spare = 32;
        let limits = FormulaLimits {
            max_support_bytes: support.live_bytes() + spare,
            ..FormulaLimits::default()
        };
        assert_eq!(
            computation.allowance(&lease, &limits, location()).unwrap(),
            spare
        );
    });
}
