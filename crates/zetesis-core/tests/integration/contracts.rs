//! Typed value, pattern construction, and borrowed template component laws.

use super::support::canonical::template_catalog;
use zetesis_core::{
    Atom, AtomPattern, ConstructionError, Filter, FilterRef, Model, Predicate, Template,
    TemplateTerm, Term, Value,
};
use zetesis_test_support::programs::{atom, number, pattern};

fn predicate(name: &str, arity: usize) -> Predicate {
    Predicate::new(name, arity).expect("nonempty test predicate")
}

#[test]
fn values_have_typed_canonical_identity() {
    let values = [
        Value::Number(-1),
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
    ];
    for pair in values.windows(2) {
        assert!(pair[0] < pair[1]);
    }
    assert!(
        !Filter::Eq(number(1), Term::Constant(Value::String("1".into())))
            .evaluate(&[])
            .expect("closed terms")
    );
    assert!(
        Filter::Neq(number(1), Term::Constant(Value::String("1".into())))
            .evaluate(&[])
            .expect("closed terms")
    );
}

#[test]
fn extrema_surround_finite_terms_without_coercing_printed_spellings() {
    let values = [
        Value::Infimum,
        Value::Number(i32::MIN),
        Value::Number(i32::MAX),
        Value::Symbol("a".into()),
        Value::String("#inf".into()),
        Value::String("#sup".into()),
        Value::Supremum,
    ];
    for (left, a) in values.iter().enumerate() {
        for (right, b) in values.iter().enumerate() {
            assert_eq!(a.compare_terms(b), left.cmp(&right));
            assert_eq!(a == b, left == right);
        }
    }
    let model = Model::new(values.iter().cloned().map(|value| atom("v", vec![value]))).unwrap();
    assert_eq!(model.atoms().len(), values.len());
    // Canonical storage preserves the same complete typed population.
    for value in &values {
        assert!(model.contains(&atom("v", vec![value.clone()])));
    }
}

#[test]
fn signatures_and_patterns_refuse_arity_mismatches() {
    assert_eq!(
        Predicate::new("", 0),
        Err(ConstructionError::EmptyPredicateName)
    );
    assert!(matches!(
        Atom::new(predicate("a", 2), vec![Value::Number(0)]),
        Err(ConstructionError::ArityMismatch {
            expected: 2,
            actual: 1
        })
    ));
    assert!(matches!(
        AtomPattern::new(predicate("a", 1), vec![]),
        Err(ConstructionError::ArityMismatch {
            expected: 1,
            actual: 0
        })
    ));
}

#[test]
fn missing_assignments_are_reported_by_variable_id() {
    let p = pattern("a", vec![Term::Variable(2)]);
    assert_eq!(
        p.instantiate(&[Value::Number(0)])
            .expect_err("missing variable")
            .variable,
        2
    );
    assert_eq!(
        Filter::Neq(number(0), Term::Variable(3))
            .evaluate(&[])
            .expect_err("missing variable")
            .variable,
        3
    );
}

#[test]
fn template_rows_preserve_filter_occurrences() {
    let template = Template::new(
        None,
        vec![],
        vec![],
        vec![],
        vec![Filter::Eq(number(7), number(9))],
    );
    let catalog = template_catalog(&[template.clone(), template.clone()]);
    assert_eq!(catalog.len(), 2);
    for row in 0..catalog.len() {
        let row = catalog.at(row).unwrap();
        assert!(row.patterns().is_empty());
        assert_eq!(row.filters().len(), 1);
        let filter = row.filters().at(0).unwrap();
        assert_eq!(filter, FilterRef::from(&template.filters()[0]));
        let (left, right) = filter.terms();
        assert_eq!(left, TemplateTerm::Constant((&Value::Number(7)).into()));
        assert_eq!(right, TemplateTerm::Constant((&Value::Number(9)).into()));
    }
}
