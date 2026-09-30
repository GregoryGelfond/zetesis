use super::*;
use crate::FormulaLimits;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use zetesis_core::{Atom, Predicate, Value, ValueNodeRef};

#[test]
fn closing_keeps_the_original_component_vocabulary() {
    let mut fixture = Fixture::default();
    fixture.scalar(&Value::String("closed text".into()), location());
    let (completed, mut counters) = fixture.finish(location());
    let closed = completed
        .into_closed(
            0,
            GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
        )
        .unwrap();
    let components = closed
        .components
        .as_ref()
        .unwrap()
        .bind_with(closed.storage.vocabulary_read(), || {
            counters.work(&FormulaLimits::default(), location())
        })
        .unwrap();
    let zetesis_core::TemplateTerm::Constant(value) = components.term(0).unwrap() else {
        panic!("the admitted scalar occurrence is a constant");
    };
    assert_eq!(value.descriptor(), ValueNodeRef::String("closed text"));
}

#[test]
fn closing_preserves_canonical_rows_without_discovering_them() {
    let atom = Atom::new(
        Predicate::new("archived", 1).unwrap(),
        vec![Value::Number(13)],
    )
    .unwrap();
    let (completed, mut counters) =
        Fixture::from_atoms([atom.clone()], location()).finish(location());
    let closed = completed
        .into_closed(
            0,
            GroundingWork::new(&FormulaLimits::default(), &mut counters, location()),
        )
        .unwrap();
    let mut descendant =
        zetesis_core::atom_interner::AtomInterner::for_closed_catalog(&closed.storage, usize::MAX)
            .unwrap();
    assert_eq!(descendant.len(), 0);
    let entry = descendant
        .entry_atom_with(
            &atom,
            zetesis_core::atom_interner::Limits {
                max_atoms: 1,
                max_bytes: usize::MAX as u128,
            },
            || Ok::<_, std::convert::Infallible>(()),
        )
        .unwrap();
    assert_eq!(
        entry
            .insert_with(
                zetesis_core::atom_interner::Limits {
                    max_atoms: 1,
                    max_bytes: usize::MAX as u128
                },
                || Ok::<_, std::convert::Infallible>(()),
            )
            .unwrap(),
        0
    );
}

#[test]
fn close_refusal_reports_actual_source_storage() {
    let (completed, mut counters) = Fixture::default().finish(location());
    let actual = completed.catalog.bytes(location()).unwrap() as u128;
    let external = 53;
    let limits = FormulaLimits {
        max_support_bytes: usize::try_from(actual + external - 1).unwrap(),
        ..FormulaLimits::default()
    };
    let error = completed
        .into_closed(
            external,
            GroundingWork::new(&limits, &mut counters, location()),
        )
        .err()
        .unwrap();
    let (failure, peak) = error.into_parts();
    assert!(matches!(
        failure,
        FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        }
    ));
    assert_eq!(
        peak, actual,
        "rejected capacity proposals are not actual peaks"
    );
}

#[test]
fn close_keeps_the_callers_stop() {
    let (completed, mut counters) = Fixture::default().finish(location());
    let accepted = counters.accounting.work;
    let limits = FormulaLimits {
        max_work: accepted,
        ..FormulaLimits::default()
    };
    let (failure, _) = completed
        .into_closed(0, GroundingWork::new(&limits, &mut counters, location()))
        .err()
        .unwrap()
        .into_parts();
    assert!(
        matches!(failure, FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. }
        if observed == u128::from(accepted) + 1 && limit == u128::from(accepted))
    );
    assert_eq!(counters.accounting.work, accepted);
}
