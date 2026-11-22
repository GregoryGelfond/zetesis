use super::*;
use crate::FormulaResource;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Atom, Predicate, Value};

fn fixture() -> Fixture {
    let predicate = Predicate::new("p", 1).unwrap();
    Fixture::from_atoms(
        // Three values admitted in order need two adjacent conversion reads.
        // This retains a genuine partial-prefix refusal at a one-read limit.
        [1, 2, 3].map(|value| Atom::new(predicate.clone(), vec![Value::Number(value)]).unwrap()),
        location(),
    )
}

fn work_refusal(error: &FormulaFailure, before: u64) {
    assert!(matches!(error, FormulaFailure::Limit {
        resource: FormulaResource::Work,
        observed,
        limit,
        location: site,
    } if *observed == u128::from(before) + 2
        && *limit == u128::from(before) + 1
        && *site == location()));
}

#[test]
fn term_table_forwards_partial_work_refusal_without_declining() {
    let _intern = InternCacheGuard::new();
    fixture().with(location(), |support, _, counters| {
        let predicate = Predicate::new("p", 1).unwrap();
        let mut terms = TermMap::for_support(support);
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: before + 1,
            ..FormulaLimits::default()
        };
        let result = term_table(
            support,
            (&predicate).into(),
            0,
            None,
            1,
            &mut terms,
            &mut |amount| charge(counters, &limits, location(), amount),
        );
        work_refusal(&result.unwrap_err(), before);
        assert_eq!(counters.accounting.work, before + 1);
    });
}

#[test]
fn term_table_charges_its_conversion_and_preserves_complete_values() {
    let _intern = InternCacheGuard::new();
    fixture().with(location(), |support, _, counters| {
        let predicate = Predicate::new("p", 1).unwrap();
        let mut terms = TermMap::for_support(support);
        let before = counters.accounting.work;
        let table = term_table(
            support,
            (&predicate).into(),
            0,
            None,
            1,
            &mut terms,
            &mut |amount| charge(counters, &FormulaLimits::default(), location(), amount),
        )
        .unwrap()
        .unwrap();
        assert_eq!(counters.accounting.work, before + 2);
        let mut values: Vec<_> = table
            .column(0)
            .iter()
            .map(|&key| match terms.get(key).unwrap().descriptor() {
                zetesis_core::ValueNodeRef::Number(value) => value,
                _ => panic!("numeric fixture"),
            })
            .collect();
        values.sort_unstable();
        assert_eq!(values, vec![1, 2, 3]);
    });
}

#[test]
fn intermediate_conversion_retains_only_completed_work_on_refusal() {
    let bindings = Bindings::from_columns(vec![vec![2, 1]]).unwrap();
    let mut counters = Counters::default();
    let limits = FormulaLimits {
        max_work: 1,
        ..FormulaLimits::default()
    };
    let result = bindings_to_atoms(&bindings, &mut |amount| {
        charge(&mut counters, &limits, location(), amount)
    });
    work_refusal(&result.unwrap_err(), 0);
    assert_eq!(counters.accounting.work, 1);

    let mut counters = Counters::default();
    let table = bindings_to_atoms(&bindings, &mut |amount| {
        charge(&mut counters, &FormulaLimits::default(), location(), amount)
    })
    .unwrap();
    assert_eq!(table.to_rows(), vec![vec![1], vec![2]]);
    assert_eq!(counters.accounting.work, 3);
}

#[test]
fn both_conversions_preserve_the_typed_cancellation() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut counters = Counters::default();
    counters.attach_test_control(&cancellation);
    let bindings = Bindings::from_columns(vec![vec![2, 1]]).unwrap();
    let error = bindings_to_atoms(&bindings, &mut |amount| {
        charge(&mut counters, &FormulaLimits::default(), location(), amount)
    })
    .unwrap_err();
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
    assert_eq!(counters.accounting.work, 0);

    let _intern = InternCacheGuard::new();
    fixture().with(location(), |support, _, counters| {
        counters.attach_test_control(&cancellation);
        let before = counters.accounting.work;
        let predicate = Predicate::new("p", 1).unwrap();
        let mut terms = TermMap::for_support(support);
        let error = term_table(
            support,
            (&predicate).into(),
            0,
            None,
            1,
            &mut terms,
            &mut |amount| charge(counters, &FormulaLimits::default(), location(), amount),
        )
        .unwrap_err();
        assert_eq!(error.interruption(), Some(Stop::Cancelled));
        assert_eq!(counters.accounting.work, before);
    });
}

#[test]
fn positive_join_propagates_a_second_source_table_refusal() {
    let _intern = InternCacheGuard::new();
    fixture().with(location(), |support, _, counters| {
        let predicate = Predicate::new("p", 1).unwrap();
        let atom = PlannedAtom {
            predicate: (&predicate).into(),
            arity: 1,
            slots: vec![0],
        };
        let mut terms = TermMap::for_support(support);
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: before + 3,
            ..FormulaLimits::default()
        };
        let error = join_atoms(
            &[atom.clone(), atom],
            &[(0, None), (0, None)],
            support,
            &mut terms,
            &mut |amount| charge(counters, &limits, location(), amount),
        )
        .unwrap_err();
        work_refusal(&error, before + 2);
        assert_eq!(counters.accounting.work, before + 3);
    });
}

#[test]
fn negative_filter_propagates_its_source_table_refusal() {
    let _intern = InternCacheGuard::new();
    fixture().with(location(), |support, _, counters| {
        let predicate = Predicate::new("p", 1).unwrap();
        let plan = Plan {
            width: 1,
            atoms: Vec::new(),
            compares: Vec::new(),
            negated: vec![Negated {
                occurrence: 0,
                predicate: (&predicate).into(),
                slots: vec![0],
            }],
        };
        let bindings = Bindings::empty(1);
        let mut terms = TermMap::for_support(support);
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: before + 1,
            ..FormulaLimits::default()
        };
        let error = apply_residuals(&bindings, &plan, support, &mut terms, &[], &mut |amount| {
            charge(counters, &limits, location(), amount)
        })
        .unwrap_err();
        work_refusal(&error, before);
        assert_eq!(counters.accounting.work, before + 1);
    });
}
