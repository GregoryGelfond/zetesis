use super::Binding;
use crate::ProgramSite;
use crate::formula_support::Context;
use crate::formula_support::testing::{Fixture, binding};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use zetesis_core::{Value, ValueNodeRef};

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(37),
        span: Span::empty(ByteOffset::new(12)),
    })
}

#[test]
fn absence_is_distinct_from_numeric_zero() {
    Fixture::default().with(location(), |_, computation, counters| {
        let frame = binding(
            &[None, Some(Value::Number(0))],
            computation,
            counters,
            location(),
        );
        assert!(matches!(
            frame.read(0, computation.read(), location()),
            Err(FormulaFailure::UnsafeVariable { variable: 0, .. })
        ));
        assert_eq!(
            frame
                .read(1, computation.read(), location())
                .unwrap()
                .descriptor(),
            ValueNodeRef::Number(0)
        );
    });
}

#[test]
fn a_prefix_cannot_read_the_parent_suffix() {
    Fixture::default().with(location(), |_, computation, counters| {
        let parent = binding(
            &[Some(Value::Number(4)), Some(Value::Number(9))],
            computation,
            counters,
            location(),
        );
        let prefix = parent.prefix(1);
        assert_eq!(
            prefix.read(0, computation.read(), location()).unwrap(),
            parent.read(0, computation.read(), location()).unwrap()
        );
        assert!(matches!(
            prefix.read(1, computation.read(), location()),
            Err(FormulaFailure::UnsafeVariable { variable: 1, .. })
        ));
    });
}

#[test]
fn copying_preserves_absent_slots() {
    Fixture::default().with(location(), |_, computation, counters| {
        let original = binding(
            &[Some(Value::Symbol("name".into())), None],
            computation,
            counters,
            location(),
        );
        let copied = original
            .copied(computation, &FormulaLimits::default(), counters, location())
            .unwrap();
        assert_eq!(copied.len(), original.len());
        assert_eq!(
            copied.read(0, computation.read(), location()).unwrap(),
            original.read(0, computation.read(), location()).unwrap()
        );
        assert!(!copied.is_bound(1, location()).unwrap());
    });
}

#[test]
fn scope_growth_publishes_only_absence() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut frame = binding(&[Some(Value::Number(3))], computation, counters, location());
        frame
            .extend_scope(3, computation, &limits, counters, location())
            .unwrap();
        assert_eq!(
            frame
                .read(0, computation.read(), location())
                .unwrap()
                .descriptor(),
            ValueNodeRef::Number(3)
        );
        for slot in 1..3 {
            assert!(!frame.is_bound(slot, location()).unwrap());
        }
    });
}

#[test]
fn clearing_a_slot_preserves_its_scope() {
    Fixture::default().with(location(), |_, computation, counters| {
        let mut frame = binding(&[Some(Value::Number(0))], computation, counters, location());
        frame
            .clear(0, &FormulaLimits::default(), counters, location())
            .unwrap();
        assert!(!frame.is_bound(0, location()).unwrap());
        assert_eq!(frame.len(), 1);
        assert!(
            frame
                .view(
                    computation.read(),
                    &FormulaLimits::default(),
                    counters,
                    location()
                )
                .is_ok()
        );
    });
}

#[test]
fn frame_capacity_obeys_shared_storage_admission() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let frame = binding(&[None, None, None], computation, counters, location());
        let empty = computation.lease();
        let current =
            limits.max_support_bytes - computation.allowance(&empty, &limits, location()).unwrap();
        let bounded = FormulaLimits {
            max_support_bytes: current,
            ..limits
        };
        assert!(matches!(
            frame.copied(computation, &bounded, counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
    });
}

#[test]
fn local_joins_cannot_rebind_absent_outer_inputs() {
    use themelios_program::program::DefaultNegation;
    use zetesis_core::{AtomPattern, Predicate, Term};
    let mut fixture = Fixture::default();
    let pattern = fixture.pattern(
        &AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap(),
        location(),
    );
    fixture.with(location(), |support, computation, counters| {
        let prefix = binding(&[None], computation, counters, location());
        let literals = [crate::formula_ir::LiteralIr::Atom(
            DefaultNegation::None,
            pattern,
        )];
        let mut budget =
            crate::expansion::Budget::new(crate::ExpansionLimits::default(), usize::MAX);
        assert!(matches!(
            crate::formula_support::Join::new(
                &literals,
                &prefix,
                1,
                support,
                &mut budget,
                Context::new(computation, &FormulaLimits::default(), counters, location())
            ),
            Err(FormulaFailure::UnsafeVariable { variable: 0, .. })
        ));
    });
}

#[test]
fn stopped_copy_preserves_the_source_frame() {
    Fixture::default().with(location(), |_, computation, counters| {
        let source = binding(&[None, None, None], computation, counters, location());
        let bounded = FormulaLimits {
            max_work: counters.accounting.work,
            ..Default::default()
        };
        assert!(matches!(
            Binding::copy_slots(source.slots(), computation, &bounded, counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(source.len(), 3);
        for slot in 0..3 {
            assert!(!source.is_bound(slot, location()).unwrap());
        }
    });
}

mod reads;

mod copies;
