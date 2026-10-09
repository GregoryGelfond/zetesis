use super::*;
use crate::formula_support::GroundingWork;
use zetesis_core::catalog::{AssignmentError, ReadError};

#[test]
fn absent_copy_precedes_exhausted_work() {
    Fixture::default().with(location(), |_, computation, counters| {
        let source = binding(&[None], computation, counters, location());
        let mut target = binding(&[], computation, counters, location());
        let limits = FormulaLimits {
            max_work: counters.accounting.work,
            ..Default::default()
        };
        for slot in [0, 1, usize::MAX] {
            assert!(matches!(target.copy_slot(usize::MAX, &source, slot,
                &mut GroundingWork::new(&limits, counters, location())),
                Err(FormulaFailure::UnsafeVariable { variable, .. }) if variable == slot));
        }
    });
}

#[test]
fn borrowed_copy_charges_only_the_destination_steps() {
    Fixture::default().with(location(), |_, computation, counters| {
        let source = binding(&[Some(Value::Number(4))], computation, counters, location());
        let mut target = binding(&[None], computation, counters, location());
        let before = counters.accounting.work;
        target
            .copy_slot(
                0,
                &source,
                0,
                &mut GroundingWork::new(&FormulaLimits::default(), counters, location()),
            )
            .unwrap();
        assert_eq!(counters.accounting.work - before, 2);
        assert_eq!(
            target.read(0, computation.read(), location()).unwrap(),
            Value::Number(4)
        );
    });
}

#[test]
fn foreign_borrowed_copy_preserves_destination() {
    Fixture::default().with(location(), |_, computation, counters| {
        let mut target = binding(&[Some(Value::Number(4))], computation, counters, location());
        Fixture::default().with(location(), |_, foreign, other_counters| {
            let source = binding(
                &[Some(Value::Number(4))],
                foreign,
                other_counters,
                location(),
            );
            assert!(matches!(
                target.copy_slot(
                    0,
                    &source,
                    0,
                    &mut GroundingWork::new(&FormulaLimits::default(), counters, location())
                ),
                Err(FormulaFailure::TermAssignment {
                    error: AssignmentError::Read(ReadError::ForeignCatalog),
                    ..
                })
            ));
            assert_eq!(
                target.read(0, computation.read(), location()).unwrap(),
                Value::Number(4)
            );
        });
    });
}
