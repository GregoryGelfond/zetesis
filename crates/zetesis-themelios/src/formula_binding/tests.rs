use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::Value;

use super::{Binding, complete};
use crate::FormulaLimits;
use crate::expansion::Budget;
use crate::formula_support::Counters;
use crate::{ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure};

fn location() -> Location {
    Location {
        source: SourceId::new(37),
        span: Span::empty(ByteOffset::new(12)),
    }
}

fn budget() -> Budget {
    Budget::new(ExpansionLimits::default(), usize::MAX)
}

#[test]
fn absence_is_distinct_from_numeric_zero() {
    let binding = copy_slots(&[None, Some(Value::Number(0))], &mut budget(), location()).unwrap();
    assert!(
        matches!(binding.read(0, location()), Err(FormulaFailure::UnsafeVariable { variable: 0, location: found }) if found == location())
    );
    assert_eq!(binding.read(1, location()).unwrap(), &Value::Number(0));
}

#[test]
fn a_prefix_cannot_read_the_parent_suffix() {
    let parent = complete([Value::Number(4), Value::Number(9)]);
    let prefix = parent.prefix(1);
    assert!(std::ptr::eq(
        prefix.read(0, location()).unwrap(),
        parent.read(0, location()).unwrap()
    ));
    assert!(matches!(
        prefix.read(1, location()),
        Err(FormulaFailure::UnsafeVariable { variable: 1, .. })
    ));
}

#[test]
fn copying_preserves_absent_slots() {
    let original = copy_slots(
        &[Some(Value::Symbol("name".into())), None],
        &mut budget(),
        location(),
    )
    .unwrap();
    let copied = original
        .copied(
            &FormulaLimits::default(),
            &mut Counters::default(),
            &mut budget(),
            location(),
        )
        .unwrap();
    assert_eq!(original, copied);
    assert!(copied.read(1, location()).is_err());
}

#[test]
fn scope_growth_publishes_only_absence() {
    let mut binding = complete([Value::Number(3)]);
    binding.extend_scope(3, &mut budget(), location()).unwrap();
    assert_eq!(binding.slots(), &[Some(Value::Number(3)), None, None]);
    binding.set(2, Value::Number(0), location()).unwrap();
    binding.clear(2);
    assert!(binding.read(2, location()).is_err());
}

#[test]
fn frame_admission_charges_optional_cells() {
    let required = std::mem::size_of::<Option<Value>>() as u128;
    let mut limited = Budget::new(
        ExpansionLimits {
            max_scalar_bytes: usize::try_from(required - 1).unwrap(),
            ..Default::default()
        },
        usize::MAX,
    );
    assert!(
        matches!(copy_slots(&[None], &mut limited, location()), Err(FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::ScalarBytes, observed, .. })) if observed == required)
    );
}

#[test]
fn local_joins_cannot_rebind_absent_outer_inputs() {
    use themelios_program::program::DefaultNegation;
    use zetesis_core::{AtomPattern, Predicate, Sign, Term};

    let prefix = copy_slots(&[None], &mut budget(), location()).unwrap();
    let literals = [crate::formula_ir::LiteralIr::Atom(
        DefaultNegation::None,
        AtomPattern::new(
            Predicate::with_sign("p", 1, Sign::Positive).unwrap(),
            vec![Term::Variable(0)],
        )
        .unwrap(),
    )];
    let support = crate::formula_support::Support::default();
    assert!(matches!(
        crate::formula_support::Join::new(
            &literals,
            &prefix,
            1,
            &support,
            &mut budget(),
            location()
        ),
        Err(FormulaFailure::UnsafeVariable { variable: 0, .. })
    ));
}

fn copy_slots(
    source: &[Option<Value>],
    budget: &mut Budget,
    location: Location,
) -> Result<Binding<'static>, FormulaFailure> {
    Binding::copy_slots(
        source,
        &FormulaLimits::default(),
        &mut Counters::default(),
        budget,
        location,
    )
}

#[test]
fn absent_frame_inspections_obey_the_work_limit() {
    for limit in [2, 3] {
        let mut counters = Counters::default();
        let result = Binding::copy_slots(
            &[None, None, None],
            &FormulaLimits {
                max_work: limit,
                ..Default::default()
            },
            &mut counters,
            &mut budget(),
            location(),
        );
        if limit == 3 {
            assert_eq!(result.unwrap().slots(), &[None, None, None]);
            assert_eq!(counters.work, 3);
        } else {
            assert!(
                matches!(result, Err(FormulaFailure::Limit { resource: crate::FormulaResource::Work, observed: 3, limit: 2, location: found }) if found == location())
            );
        }
    }
}
