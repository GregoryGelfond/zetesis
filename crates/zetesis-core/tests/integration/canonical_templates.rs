//! Canonical template components preserve borrowed identity and occurrence order.

use super::support::canonical::template_catalog;
use zetesis_core::catalog::{Error as CatalogError, TermRef};
use zetesis_core::{
    Atom, AtomPattern, Filter, PatternRef, Predicate, Template, TemplateCatalogBuilder,
    TemplateTerm, Term, Value, ValueLimits, ValueNode,
};

fn fact(value: Value) -> Template {
    let pattern =
        AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Constant(value)]).unwrap();
    Template::new(Some(pattern), vec![], vec![], vec![], vec![])
}

#[test]
fn template_rows_keep_explicit_constant_roots() {
    let value = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: zetesis_core::Sign::Positive,
                arity: 1,
            },
            ValueNode::Symbol("child".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let catalog = template_catalog(&[fact(value.clone()), fact(value.clone())]);
    let terms = catalog.at(0).unwrap().patterns().at(0).unwrap().terms();
    assert_eq!(terms.len(), 1);
    let TemplateTerm::Constant(root) = terms.at(0).unwrap() else {
        panic!("closed root");
    };
    assert_eq!(root, value);
    assert_ne!(root, Value::Symbol("child".into()));
    let TemplateTerm::Constant(other) = catalog
        .at(1)
        .unwrap()
        .patterns()
        .at(0)
        .unwrap()
        .terms()
        .at(0)
        .unwrap()
    else {
        panic!("repeated closed root");
    };
    let zetesis_core::ValueNodeRef::Function { name, .. } = root.descriptor() else {
        panic!("structured root");
    };
    let zetesis_core::ValueNodeRef::Function {
        name: other_name, ..
    } = other.descriptor()
    else {
        panic!("structured occurrence");
    };
    assert!(std::ptr::eq(name, other_name));
    assert_eq!(terms.slice(0..1).unwrap().len(), 1);
    assert!(terms.slice(std::ops::Range { start: 1, end: 0 }).is_none());
    assert_eq!(terms.variables().count(), 0);
}

#[test]
fn admitted_views_share_the_constant_payload() {
    let template = fact(Value::Symbol("a-longer-owned-symbol".into()));
    let catalog = template_catalog(&[template.clone(), template]);
    let mut signatures = Vec::new();
    let mut constants = Vec::new();
    for row in 0..catalog.len() {
        let pattern = catalog.at(row).unwrap().patterns().at(0).unwrap();
        signatures.push(pattern.predicate().name());
        let TemplateTerm::Constant(value) = pattern.terms().at(0).unwrap() else {
            panic!("closed fact");
        };
        let zetesis_core::ValueNodeRef::Symbol(text) = value.descriptor() else {
            panic!("symbol constant");
        };
        constants.push(text);
    }
    assert!(std::ptr::eq(constants[0], constants[1]));
    assert!(std::ptr::eq(signatures[0], signatures[1]));
    assert_eq!(catalog.len(), 2);
}

#[test]
fn catalog_import_preserves_explicitly_admitted_depth() {
    let depth = 256;
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; depth - 1];
    nodes.push(ValueNode::Number(7));
    let value = Value::from_nodes(
        nodes,
        ValueLimits {
            max_depth: depth,
            ..ValueLimits::default()
        },
    )
    .unwrap();
    let catalog = template_catalog(&[fact(value)]);
    let TemplateTerm::Constant(root) = catalog
        .at(0)
        .unwrap()
        .patterns()
        .at(0)
        .unwrap()
        .terms()
        .at(0)
        .unwrap()
    else {
        panic!("closed root");
    };
    assert_eq!(root.depth(), depth);
}

#[test]
fn catalog_bounds_template_metadata() {
    let pattern = AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let result = (|| {
        let mut builder = TemplateCatalogBuilder::new(1024)?;
        for _ in 0..100 {
            builder.append([], [PatternRef::from(&pattern)], [])?;
        }
        builder.finish()
    })();
    let CatalogError::Storage { required, limit } = result.unwrap_err() else {
        panic!("named storage refusal");
    };
    assert_eq!(limit, 1024);
    assert!(required > limit as u128);
}

#[test]
fn failed_component_import_cannot_publish_an_accepted_prefix() {
    let first = fact(Value::Number(1));
    let oversized = fact(Value::String("x".repeat(1 << 20)));
    let mut builder = TemplateCatalogBuilder::new(1 << 19).unwrap();
    builder
        .append([], first.head().map(PatternRef::from), [])
        .unwrap();
    let error = builder
        .append([], oversized.head().map(PatternRef::from), [])
        .unwrap_err();
    assert!(matches!(error, CatalogError::Storage { required, limit } if required > limit as u128));
    assert_eq!(builder.finish().unwrap_err(), error);
}

#[test]
fn admitted_keys_preserve_substitution_identity() {
    let predicate = Predicate::new("p", 3).unwrap();
    let pattern = AtomPattern::new(
        predicate.clone(),
        vec![
            Term::Variable(0),
            Term::Constant(Value::String("s".into())),
            Term::Variable(0),
        ],
    )
    .unwrap();
    let template = Template::new(
        Some(pattern.clone()),
        vec![pattern.clone()],
        vec![],
        vec![],
        vec![],
    );
    let catalog = template_catalog(&[template]);
    let assignment = [Value::Number(9)];
    let expected = Atom::new(
        predicate,
        vec![
            Value::Number(9),
            Value::String("s".into()),
            Value::Number(9),
        ],
    )
    .unwrap();
    let admitted = catalog
        .at(0)
        .unwrap()
        .patterns()
        .at(0)
        .unwrap()
        .key(assignment.as_slice())
        .unwrap();
    let ingress = PatternRef::from(&pattern)
        .key(assignment.as_slice())
        .unwrap();
    assert_eq!(admitted, ingress);
    assert!(admitted.compare(&expected).is_eq());
    assert_eq!(admitted.value(0), admitted.value(2));
    let missing: &[Option<TermRef<'_>>] = &[None];
    assert_eq!(
        catalog
            .at(0)
            .unwrap()
            .patterns()
            .at(0)
            .unwrap()
            .key(missing)
            .unwrap_err()
            .variable,
        0
    );
}

#[test]
fn admitted_key_comparison_stops_at_each_work_boundary() {
    let value = Value::String("compared text".into());
    let template = fact(value.clone());
    let catalog = template_catalog(&[template]);
    let empty: &[Value] = &[];
    let key = catalog
        .at(0)
        .unwrap()
        .patterns()
        .at(0)
        .unwrap()
        .key(empty)
        .unwrap();
    let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap();
    let mut total = 0;
    assert!(
        key.compare_identity_with(&atom, || {
            total += 1;
            Ok::<_, usize>(())
        })
        .unwrap()
        .is_eq()
    );
    assert!(total > 0);
    for stop in 0..total {
        let mut calls = 0;
        assert_eq!(
            key.compare_identity_with(&atom, || {
                let at = calls;
                calls += 1;
                if at == stop { Err(stop) } else { Ok(()) }
            }),
            Err(stop)
        );
        assert_eq!(calls, stop + 1);
    }
}

#[test]
fn filter_views_share_the_ingress_evaluator() {
    let filter = Filter::Neq(
        Term::Constant(Value::Number(7)),
        Term::Constant(Value::String("7".into())),
    );
    let catalog = template_catalog(&[Template::new(
        None,
        vec![],
        vec![],
        vec![],
        vec![filter.clone()],
    )]);
    let empty: &[Value] = &[];
    assert_eq!(
        catalog
            .at(0)
            .unwrap()
            .filters()
            .at(0)
            .unwrap()
            .evaluate(empty),
        filter.evaluate(empty)
    );
    assert!(filter.evaluate(empty).unwrap());
}
