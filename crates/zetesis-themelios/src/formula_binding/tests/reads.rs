use super::location;
use crate::FormulaFailure;
use crate::formula_binding::Binding;
use crate::formula_support::testing::{Fixture, binding};
use zetesis_core::atom_interner::{AtomInterner, Limits as InternerLimits};
use zetesis_core::catalog::{
    AssignmentError, AtomCatalog, Limits as TermLimits, ReadError, TermAssignment,
};
use zetesis_core::{Value, ValueNodeRef};

#[test]
fn bound_read_rejects_equal_foreign_terms() {
    Fixture::default().with(location(), |_, computation, counters| {
        let frame = binding(&[Some(Value::Number(4))], computation, counters, location());
        Fixture::default().with(location(), |_, foreign, foreign_counters| {
            let other = binding(
                &[Some(Value::Number(4))],
                foreign,
                foreign_counters,
                location(),
            );
            assert_eq!(
                frame.read(0, computation.read(), location()).unwrap(),
                other.read(0, foreign.read(), location()).unwrap()
            );
            let borrowed = frame.prefix(frame.len());
            for selected in [&frame, &borrowed] {
                assert!(matches!(
                    selected.read(0, foreign.read(), location()),
                    Err(FormulaFailure::TermAssignment {
                        error: AssignmentError::Read(ReadError::ForeignCatalog),
                        location: actual,
                    }) if actual == location()
                ));
            }
        });
    });
}

#[test]
fn absent_read_precedes_foreign_scope() {
    Fixture::default().with(location(), |_, computation, counters| {
        let frame = binding(&[None], computation, counters, location());
        Fixture::default().with(location(), |_, foreign, _| {
            let borrowed = frame.prefix(frame.len());
            for selected in [&frame, &borrowed] {
                assert!(matches!(
                    selected.read(0, foreign.read(), location()),
                    Err(FormulaFailure::UnsafeVariable {
                        variable: 0,
                        location: actual,
                    }) if actual == location()
                ));
            }
        });
    });
}

#[test]
fn invalid_read_precedes_foreign_scope() {
    Fixture::default().with(location(), |_, computation, counters| {
        let frame = binding(&[], computation, counters, location());
        Fixture::default().with(location(), |_, foreign, _| {
            let borrowed = frame.prefix(frame.len());
            for selected in [&frame, &borrowed] {
                for variable in [0, usize::MAX] {
                    assert!(matches!(
                        selected.read(variable, foreign.read(), location()),
                        Err(FormulaFailure::UnsafeVariable {
                            variable: actual_variable,
                            location: actual,
                        }) if actual_variable == variable && actual == location()
                    ));
                }
            }
        });
    });
}

fn published_prefixes() -> (AtomCatalog, AtomCatalog, TermAssignment) {
    let mut owner = AtomInterner::new();
    let limits = InternerLimits {
        max_atoms: 0,
        max_bytes: 1 << 20,
    };
    let permit = || Ok::<(), ()>(());
    let first = owner
        .split()
        .1
        .import_term_with(
            (&Value::Number(4)).into(),
            TermLimits::default(),
            limits,
            permit,
        )
        .unwrap();
    let prefix = owner.publish_selection_with(&[], limits, permit).unwrap();
    let second = owner
        .split()
        .1
        .import_term_with(
            (&Value::Number(9)).into(),
            TermLimits::default(),
            limits,
            permit,
        )
        .unwrap();
    let complete = owner.publish_selection_with(&[], limits, permit).unwrap();
    let mut values = complete.read().assignment();
    values.resize_with(3, 1 << 20, permit).unwrap();
    values.set_with(0, &first, permit).unwrap();
    values.set_with(1, &second, permit).unwrap();
    (prefix, complete, values)
}

#[test]
fn newer_term_read_refuses_older_prefix() {
    let (prefix, complete, values) = published_prefixes();
    let frame = Binding::borrowed(values.as_slice());
    assert_eq!(
        frame
            .read(1, complete.read(), location())
            .unwrap()
            .descriptor(),
        ValueNodeRef::Number(9)
    );
    assert!(matches!(
        frame.read(1, prefix.read(), location()),
        Err(FormulaFailure::TermAssignment {
            error: AssignmentError::Read(ReadError::OutsidePrefix),
            location: actual,
        }) if actual == location()
    ));
}

#[test]
fn read_ignores_unrelated_newer_slots() {
    let (prefix, _, values) = published_prefixes();
    let frame = Binding::borrowed(values.as_slice());
    assert_eq!(
        frame
            .read(0, prefix.read(), location())
            .unwrap()
            .descriptor(),
        ValueNodeRef::Number(4)
    );
}

#[test]
fn absent_read_ignores_unrelated_newer_slots() {
    let (prefix, _, values) = published_prefixes();
    let frame = Binding::borrowed(values.as_slice());
    assert!(matches!(
        frame.read(2, prefix.read(), location()),
        Err(FormulaFailure::UnsafeVariable {
            variable: 2,
            location: actual,
        }) if actual == location()
    ));
}

#[test]
fn charged_reads_preserve_owned_failure_order() {
    let (prefix, complete, values) = published_prefixes();
    let frame = Binding::borrowed(values.as_slice());
    for read in [prefix.read(), complete.read()] {
        for variable in [0, 1, 2, 3, usize::MAX] {
            for stop in [false, true] {
                let refuse = || FormulaFailure::UnsafeVariable {
                    variable: 17,
                    location: location(),
                };
                let mut expected_calls = 0;
                let expected = frame.key(variable, location()).and_then(|key| {
                    expected_calls += 1;
                    if stop {
                        return Err(refuse());
                    }
                    read.term(&key).map_err(|error| {
                        crate::formula_binding::assignment(error.into(), location())
                    })
                });
                let mut actual_calls = 0;
                let actual = frame.read_with(variable, read, location(), || {
                    actual_calls += 1;
                    if stop { Err(refuse()) } else { Ok(()) }
                });
                let expected = expected
                    .map(zetesis_core::catalog::TermRef::descriptor)
                    .map_err(|error| format!("{error:?}"));
                let actual = actual
                    .map(zetesis_core::catalog::TermRef::descriptor)
                    .map_err(|error| format!("{error:?}"));
                assert_eq!(actual, expected);
                assert_eq!(actual_calls, expected_calls);
            }
        }
    }
}
