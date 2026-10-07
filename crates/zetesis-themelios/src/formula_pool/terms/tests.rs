use super::*;
use crate::{ExpansionLimits, StatementId};
use themelios_program::symbol::Name;

fn source(arity: usize) -> Term {
    Term::Function {
        name: Name::new("f").unwrap(),
        arguments: (0..arity)
            .map(|_| Term::pool(vec![1.into(), 2.into()]).unwrap())
            .collect(),
    }
}

fn budget(bytes: usize) -> Budget {
    Budget::new(
        ExpansionLimits {
            max_values: usize::MAX,
            max_term_work: usize::MAX,
            max_scalar_bytes: usize::MAX,
            max_family_bytes: bytes,
            ..ExpansionLimits::default()
        },
        usize::MAX,
    )
}

fn site() -> ProgramSite {
    ProgramSite::statement(StatementId::new(0), None)
}

#[test]
fn materialized_pool_refuses_before_output_construction() {
    let error = distribute(source(12), &mut budget(1024), site()).unwrap_err();
    assert!(matches!(error, ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes, limit: 1024, observed, ..
    } if observed > 1_000_000));
}

#[test]
fn family_storage_boundary_is_inclusive() {
    let error = distribute(source(2), &mut budget(0), site()).unwrap_err();
    let ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes,
        observed,
        ..
    } = error
    else {
        panic!("wrong pre-materialization refusal: {error}");
    };
    let exact = usize::try_from(observed).unwrap();
    assert!(
        matches!(distribute(source(2), &mut budget(exact - 1), site()), Err(ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes, observed: amount, ..
    }) if amount == observed)
    );
    let Term::Pool(ref values) = distribute(source(2), &mut budget(exact), site()).unwrap() else {
        panic!("constructor alternatives must remain an ordered pool");
    };
    assert_eq!(values.len(), 4);
}

#[test]
fn released_families_do_not_accumulate_storage() {
    let error = distribute(source(2), &mut budget(0), site()).unwrap_err();
    let ExpansionFailure::Limit {
        resource: ExpansionResource::FamilyBytes,
        observed,
        ..
    } = error
    else {
        panic!("wrong pre-materialization refusal: {error}");
    };
    let exact = usize::try_from(observed).unwrap();
    let mut budget = budget(exact);
    for _ in 0..32 {
        let Term::Pool(ref values) = distribute(source(2), &mut budget, site()).unwrap() else {
            panic!("constructor alternatives must remain a pool");
        };
        assert_eq!(values.len(), 4);
    }
    assert!(budget.usage().scalar_bytes > exact);
}
