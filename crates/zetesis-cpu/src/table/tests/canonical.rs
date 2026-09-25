//! Canonical terms preserve table denotation and interruption boundaries.

use super::*;
use zetesis_core::{AtomCatalog, ValueLimits, ValueNode, ValueNodeRef};

#[test]
fn foreign_finite_domains_preserve_source_occurrences() {
    let predicate = Predicate::new("pair", 2).unwrap();
    let source = atoms(
        &predicate,
        &[
            vec![Value::Symbol("same".into()), Value::Number(1)],
            vec![Value::String("same".into()), Value::Number(1)],
            vec![Value::Symbol("same".into()), Value::Number(2)],
            vec![Value::Symbol("same".into()), Value::Number(1)],
        ],
    );
    let foreign = AtomCatalog::new(vec![
        source[1].clone(),
        source[2].clone(),
        source[0].clone(),
    ])
    .unwrap();
    let catalog = AtomCatalog::new(source).unwrap();
    let indices = [2, 0, 3, 1, 0];
    let relation = Relation::from_catalog_refs(
        (&predicate).into(),
        catalog.atoms(),
        &indices,
        RelationLimits::default(),
    )
    .unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let foreign_row = foreign.atoms().at(2).unwrap();
    let symbol = foreign_row.values().at(0).unwrap();
    let first = [symbol, symbol];
    let domains = [
        Domain::Finite(first.as_slice().into()),
        Domain::Singleton(foreign_row.values().at(1).unwrap()),
    ];
    let selected = table
        .select(&domains, Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(selected.rows().collect::<Vec<_>>(), vec![1, 2, 4]);
    let projected = table
        .project_domains(&domains, Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(projected.words(), selected.words());
    let actual: Vec<_> = projected.domain(0).unwrap().collect();
    assert_eq!(actual, vec![symbol]);
}

#[test]
fn projected_values_borrow_the_relation_dictionary() {
    let predicate = Predicate::new("table", 1).unwrap();
    let row = Atom::new(
        predicate.clone(),
        vec![Value::String("shared text".repeat(128))],
    )
    .unwrap();
    let catalog = AtomCatalog::new(vec![row.clone()]).unwrap();
    let foreign = AtomCatalog::new(vec![row]).unwrap();
    let relation = Relation::from_refs(
        (&predicate).into(),
        catalog.atoms(),
        RelationLimits::default(),
    )
    .unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domain = [foreign.atoms().at(0).unwrap().values().at(0).unwrap()];
    let projection = table
        .project_domains(
            &[Domain::Finite(domain.as_slice().into())],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let actual = projection.domain(0).unwrap().next().unwrap();
    let original = relation.row(0).unwrap().value(0).unwrap();
    let (ValueNodeRef::String(actual), ValueNodeRef::String(original)) =
        (actual.descriptor(), original.descriptor())
    else {
        panic!("fixture projects a string");
    };
    assert!(std::ptr::eq(actual, original));
}

#[test]
fn canonical_selection_refuses_each_comparison_prefix() {
    let predicate = Predicate::new("table", 1).unwrap();
    let value = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 1 },
            ValueNode::String("common prefix".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let row = Atom::new(predicate.clone(), vec![value]).unwrap();
    let catalog = AtomCatalog::new(vec![row.clone()]).unwrap();
    let foreign = AtomCatalog::new(vec![row]).unwrap();
    let relation = Relation::from_refs(
        (&predicate).into(),
        catalog.atoms(),
        RelationLimits::default(),
    )
    .unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domains = [Domain::Singleton(
        foreign.atoms().at(0).unwrap().values().at(0).unwrap(),
    )];
    let complete = table
        .select(&domains, Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(complete.words(), &[1]);
    let total = complete.statistics().work;
    assert!(total > 3);
    for limit in 0..total {
        let failure = table
            .select(
                &domains,
                Limits {
                    max_work: limit,
                    ..Limits::default()
                },
                &Cancellation::default(),
            )
            .err()
            .expect("the next charged step must refuse");
        assert_eq!(failure.work, limit);
        assert!(
            matches!(failure.cause, Cause::Limit { resource: Resource::Work, observed, .. } if observed == u128::from(limit) + 1)
        );
    }
    let exact = table
        .select(
            &domains,
            Limits {
                max_work: total,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(exact.words(), complete.words());
}
