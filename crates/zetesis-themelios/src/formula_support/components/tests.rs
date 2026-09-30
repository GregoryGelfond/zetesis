use super::*;
use crate::formula_support::{Computation, Support};
use crate::test_support::location;
use zetesis_core::{Sign, Value};

#[test]
fn committed_components_borrow_across_generated_admission_without_truth() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let text = Value::String("one static canonical payload".into());
    let mut admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let scalar = admission
        .scalar((&text).into(), &limits, &mut counters, location())
        .unwrap();
    let constructor = admission
        .constructor(
            ValueNodeRef::Function {
                name: "only_a_shape",
                sign: Sign::Positive,
                arity: 1,
            },
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    admission
        .finish(&limits, &mut counters, location())
        .unwrap();
    assert_eq!(catalog.owner.len(), 0);
    assert_eq!(counters.accounting.workspace.bytes(), 0);
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let borrowed = computation
        .static_scalar(scalar, &limits, &mut counters, location())
        .unwrap();
    let shape = computation
        .static_constructor(constructor, &limits, &mut counters, location())
        .unwrap();
    // These references borrow the committed source, not the append capability.
    computation
        .number(17, &limits, &mut counters, location())
        .unwrap();
    let key = computation
        .static_key(scalar, &limits, &mut counters, location())
        .unwrap();
    let retained = computation.read().term(&key).unwrap();
    assert_eq!(borrowed, retained);
    let ValueNodeRef::String(before) = borrowed.descriptor() else {
        panic!("string fixture")
    };
    let ValueNodeRef::String(after) = retained.descriptor() else {
        panic!("string fixture")
    };
    assert!(std::ptr::eq(before, after));
    assert!(matches!(
        shape,
        ValueNodeRef::Function {
            name: "only_a_shape",
            sign: Sign::Positive,
            arity: 1
        }
    ));
    assert_eq!(relations.entries, 0);
}

#[test]
fn repeated_admission_keeps_occurrences_and_rewrites_constructor_metadata() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let mut first = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let scalar = first
        .scalar(
            (&Value::Number(4)).into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    let shape = first
        .constructor(
            ValueNodeRef::Function {
                name: "f",
                sign: Sign::Positive,
                arity: 2,
            },
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    first.finish(&limits, &mut counters, location()).unwrap();
    let mut second = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let negative = second
        .negate_constructor(shape, &limits, &mut counters, location())
        .unwrap()
        .unwrap();
    let positive = second
        .negate_constructor(negative, &limits, &mut counters, location())
        .unwrap()
        .unwrap();
    let tuple = second
        .constructor(
            ValueNodeRef::Tuple { arity: 2 },
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    assert!(
        second
            .negate_constructor(tuple, &limits, &mut counters, location())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        second
            .scalar_ref(scalar, &limits, &mut counters, location())
            .unwrap()
            .descriptor(),
        ValueNodeRef::Number(4)
    );
    let view = second.bound(&limits, &mut counters, location()).unwrap();
    let original = view.constructor(shape.0).unwrap();
    assert_eq!(original, view.constructor(positive.0).unwrap());
    assert!(matches!(
        view.constructor(negative.0),
        Some(ValueNodeRef::Function {
            name: "f",
            sign: Sign::Negative,
            arity: 2
        })
    ));
    second.finish(&limits, &mut counters, location()).unwrap();
    assert_eq!(catalog.owner.len(), 0);
}

#[test]
fn metadata_refusal_keeps_actual_capacity_and_cannot_finish() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let mut admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let key = admission
        .import(
            (&Value::Number(1)).into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    let before = admission.total(location()).unwrap();
    let narrow = FormulaLimits {
        max_support_bytes: before,
        ..limits
    };
    let result = admission.scalar_key(&key, &narrow, &mut counters, location());
    assert!(
        matches!(result, Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. }) if observed > limit)
    );
    assert_eq!(admission.total(location()).unwrap(), before);
    assert!(matches!(
        admission.finish(&limits, &mut counters, location()),
        Err(FormulaFailure::TemplateCatalog {
            error: TemplateCatalogFailure::Incomplete,
            ..
        })
    ));
    assert_eq!(counters.accounting.workspace.bytes(), 0);
    assert!(catalog.owner.read().term(&key).is_ok());
}

#[test]
fn admission_rejects_another_accounting_workspace() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let unrelated = Counters::default().accounting.workspace.lease();
    assert!(matches!(
        admission.allowance(&unrelated, &limits, location()),
        Err(FormulaFailure::TemplateCatalog {
            error: TemplateCatalogFailure::Read(ReadError::ForeignCatalog),
            ..
        })
    ));
}

fn pattern_inputs(
    admission: &mut Admission<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> (Predicate, [Term; 4]) {
    let scalar = admission
        .scalar(
            (&Value::String("shared column".into())).into(),
            limits,
            counters,
            location(),
        )
        .unwrap();
    let predicate = admission
        .predicate(
            (&zetesis_core::Predicate::with_sign("p", 4, Sign::Negative).unwrap()).into(),
            limits,
            counters,
            location(),
        )
        .unwrap();
    (
        predicate,
        [
            Term::Variable(7),
            Term::Constant(scalar),
            Term::Variable(7),
            Term::Constant(scalar),
        ],
    )
}

#[test]
fn pattern_and_filter_admission_reuse_scalar_payload_and_preserve_columns() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let mut admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let (predicate, arguments) = pattern_inputs(&mut admission, &limits, &mut counters);
    let owner_bytes = admission.catalog.owner.storage_bytes();
    let pattern = admission
        .pattern(predicate, &arguments, &limits, &mut counters, location())
        .unwrap();
    let filter = admission
        .filter(
            false,
            arguments[0],
            arguments[1],
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    assert_eq!(admission.catalog.owner.storage_bytes(), owner_bytes);
    assert_eq!(
        predicate
            .get(
                admission.bound(&limits, &mut counters, location()).unwrap(),
                &limits,
                &mut counters,
                location()
            )
            .unwrap()
            .sign(),
        Sign::Negative
    );
    admission
        .finish(&limits, &mut counters, location())
        .unwrap();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let pattern = computation
        .static_pattern(pattern, &limits, &mut counters, location())
        .unwrap();
    let filter = computation
        .static_filter(filter, &limits, &mut counters, location())
        .unwrap();
    let predicate = computation
        .static_predicate(predicate, &limits, &mut counters, location())
        .unwrap();
    let constant = computation
        .static_term(arguments[1], &limits, &mut counters, location())
        .unwrap();
    computation
        .number(42, &limits, &mut counters, location())
        .unwrap();
    assert_eq!(pattern.predicate(), predicate);
    assert_eq!(pattern.terms().at(0), Some(TemplateTerm::Variable(7)));
    assert_eq!(pattern.terms().at(0), pattern.terms().at(2));
    assert_eq!(pattern.terms().at(1), pattern.terms().at(3));
    assert_eq!(pattern.terms().at(1), Some(constant));
    assert_eq!(filter.terms(), (TemplateTerm::Variable(7), constant));
    assert!(!filter.is_equality());
    let TemplateTerm::Constant(expected) = constant else {
        panic!("constant fixture")
    };
    let Some(TemplateTerm::Constant(actual)) = pattern.terms().at(3) else {
        panic!("constant column")
    };
    let (ValueNodeRef::String(expected), ValueNodeRef::String(actual)) =
        (expected.descriptor(), actual.descriptor())
    else {
        panic!("string fixture")
    };
    assert!(std::ptr::eq(expected, actual));
    assert_eq!(relations.entries, 0);
}

#[test]
fn every_pattern_stop_releases_argument_scratch_and_exposes_no_partial_pattern() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let mut admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let (predicate, arguments) = pattern_inputs(&mut admission, &limits, &mut counters);
    let start = counters.accounting.work;
    admission
        .pattern(predicate, &arguments, &limits, &mut counters, location())
        .unwrap();
    let work = counters.accounting.work - start;
    drop(admission);
    for cut in 0..work {
        let mut counters = Counters::default();
        let mut catalog = SupportCatalog::default();
        let mut admission = catalog
            .component_admission(&limits, &mut counters, location())
            .unwrap();
        let (predicate, arguments) = pattern_inputs(&mut admission, &limits, &mut counters);
        let workspace = counters.accounting.workspace.bytes();
        let narrow = FormulaLimits {
            max_work: counters.accounting.work + cut,
            ..limits
        };
        assert!(matches!(
            admission.pattern(predicate, &arguments, &narrow, &mut counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(counters.accounting.workspace.bytes(), workspace);
        match admission.components(&limits, &mut counters, location()) {
            Ok(view) => assert!(view.patterns().is_empty()),
            Err(FormulaFailure::TemplateCatalog {
                error: TemplateCatalogFailure::Incomplete,
                ..
            }) => {}
            Err(error) => panic!("unexpected post-stop diagnosis: {error}"),
        }
    }
}

#[test]
fn refused_argument_scratch_does_not_mutate_components() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let mut admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let (predicate, arguments) = pattern_inputs(&mut admission, &limits, &mut counters);
    let before = admission.total(location()).unwrap();
    let narrow = FormulaLimits {
        max_support_bytes: before,
        ..limits
    };
    assert!(matches!(
        admission.pattern(predicate, &arguments, &narrow, &mut counters, location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert_eq!(admission.total(location()).unwrap(), before);
    assert!(
        admission
            .components(&limits, &mut counters, location())
            .unwrap()
            .patterns()
            .is_empty()
    );
    admission
        .pattern(predicate, &arguments, &limits, &mut counters, location())
        .unwrap();
}
