use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Sign, ValueLimits, ValueNode};

use super::*;

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn insert(catalog: &mut SupportCatalog, atom: Atom) {
    catalog
        .insert(
            atom,
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
}

fn atom(values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new("row", values.len()).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

#[test]
fn membership_preserves_append_order() {
    let mut catalog = SupportCatalog::default();
    for value in [3, 1, 2] {
        insert(&mut catalog, atom(&[value]));
    }
    {
        let support = catalog
            .snapshot(
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            )
            .unwrap();
        for value in [1, 2, 3] {
            assert!(support.contains(&atom(&[value])));
        }
        assert!(!support.contains(&atom(&[0])));
        let rows: Vec<_> = support
            .rows(atom(&[0]).predicate())
            .map(|row| (row.source_index(), row.value(0).unwrap().clone()))
            .collect();
        assert_eq!(
            rows,
            vec![
                (0, Value::Number(3)),
                (1, Value::Number(1)),
                (2, Value::Number(2))
            ]
        );
    }
    insert(&mut catalog, atom(&[0]));
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    assert_eq!(
        support.row(atom(&[0]).predicate(), 3).unwrap().value(0),
        Some(&Value::Number(0))
    );
    assert_eq!(
        support.row(atom(&[3]).predicate(), 0).unwrap().value(0),
        Some(&Value::Number(3))
    );
}

#[test]
fn nullary_membership_retains_predicate_sign() {
    let positive = Atom::new(Predicate::new("p", 0).unwrap(), Vec::new()).unwrap();
    let negative = Atom::new(
        Predicate::with_sign("p", 0, Sign::Negative).unwrap(),
        Vec::new(),
    )
    .unwrap();
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, negative.clone());
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    assert!(support.contains(&negative));
    assert!(!support.contains(&positive));
    let row = support.row(negative.predicate(), 0).unwrap();
    assert_eq!(row.predicate(), negative.predicate());
    assert_eq!(row.value(0), None);
    assert_eq!(support.row_count(positive.predicate()), 0);
}

#[test]
fn column_lookup_preserves_the_shortest_posting() {
    let original: Vec<_> = [[2, 1], [1, 2], [2, 2], [3, 1]]
        .iter()
        .map(|row| atom(row))
        .collect();
    let mut catalog = SupportCatalog::default();
    for row in &original {
        insert(&mut catalog, row.clone());
    }
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let pattern = AtomPattern::new(
        original[0].predicate().clone(),
        vec![Term::Variable(0), Term::Variable(1)],
    )
    .unwrap();
    for left in [None, Some(-1), Some(1), Some(2), Some(3)] {
        for right in [None, Some(-1), Some(1), Some(2), Some(3)] {
            let values = [left.map(Value::Number), right.map(Value::Number)];
            let postings: Vec<Vec<usize>> = values
                .iter()
                .enumerate()
                .filter_map(|(column, value)| {
                    value.as_ref().map(|value| {
                        original
                            .iter()
                            .enumerate()
                            .filter_map(|(row, atom)| {
                                (atom.values()[column] == *value).then_some(row)
                            })
                            .collect()
                    })
                })
                .collect();
            let expected = postings
                .iter()
                .min_by_key(|rows| rows.len())
                .map(Vec::as_slice);
            let selected = support
                .probe(
                    &pattern,
                    &values,
                    &FormulaLimits::default(),
                    &mut Counters::default(),
                    location(),
                )
                .unwrap();
            assert_eq!(selected, expected);
        }
    }
}

#[test]
fn column_lookup_retains_whole_typed_keys() {
    let tuple = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
        ValueLimits::default(),
    )
    .unwrap();
    let values = [
        Value::Infimum,
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
        tuple,
        Value::Supremum,
    ];
    let predicate = Predicate::new("row", 1).unwrap();
    let mut catalog = SupportCatalog::default();
    for value in &values {
        insert(
            &mut catalog,
            Atom::new(predicate.clone(), vec![value.clone()]).unwrap(),
        );
    }
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    for (position, value) in values.iter().enumerate() {
        let pattern =
            AtomPattern::new(predicate.clone(), vec![Term::Constant(value.clone())]).unwrap();
        assert_eq!(
            support
                .probe(
                    &pattern,
                    &[],
                    &FormulaLimits::default(),
                    &mut Counters::default(),
                    location()
                )
                .unwrap(),
            Some([position].as_slice())
        );
    }
}

#[test]
fn snapshot_bytes_include_construction_scratch() {
    let mut catalog = SupportCatalog::default();
    for values in [[2, 1], [1, 2], [2, 2]] {
        insert(&mut catalog, atom(&values));
    }
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let relation = &support.rows.values().next().unwrap().relation;
    let construction =
        catalog.index_bytes + size_of::<Support<'_>>() + size_of::<RelationRows<'_>>()
            - size_of::<Relation<'_>>()
            + size_of::<&Predicate>()
            + relation.storage().peak_construction_bytes;
    let postings = support.bytes + size_of::<BTreeMap<u32, Vec<usize>>>();
    let exact = postings.max(construction);
    drop(support);
    let limits = FormulaLimits {
        max_support_bytes: exact,
        ..FormulaLimits::default()
    };
    assert!(
        catalog
            .snapshot(&limits, &mut Counters::default(), location())
            .is_ok()
    );
    let below = FormulaLimits {
        max_support_bytes: exact - 1,
        ..limits
    };
    assert!(
        catalog
            .snapshot(&below, &mut Counters::default(), location())
            .is_err()
    );
}

#[test]
fn probe_bytes_include_keys_beside_the_query() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, atom(&[1, 2]));
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let pattern = AtomPattern::new(
        atom(&[1, 2]).predicate().clone(),
        vec![Term::Constant(Value::Number(1)), Term::Variable(0)],
    )
    .unwrap();
    let binding = [Some(Value::Number(2))];
    let keys = [(0, &Value::Number(1)), (1, &Value::Number(2))];
    let query = support
        .rows
        .values()
        .next()
        .unwrap()
        .relation
        .query(&keys, Limits::default())
        .unwrap();
    let bytes = support.bytes
        + size_of::<Vec<(usize, &Value)>>()
        + 2 * size_of::<(usize, &Value)>()
        + query.retained_bytes();
    drop(query);
    let exact = FormulaLimits {
        max_support_bytes: bytes,
        ..FormulaLimits::default()
    };
    assert_eq!(
        support
            .probe(
                &pattern,
                &binding,
                &exact,
                &mut Counters::default(),
                location()
            )
            .unwrap(),
        Some([0].as_slice())
    );
    let below = FormulaLimits {
        max_support_bytes: bytes - 1,
        ..exact
    };
    assert!(matches!(
        support.probe(&pattern, &binding, &below, &mut Counters::default(), location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, observed, limit, location: found,
        }) if observed == bytes as u128 && limit == (bytes - 1) as u128 && found == location()
    ));
}

#[test]
fn probe_work_includes_typed_query_resolution() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, atom(&[1, 2]));
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let pattern = AtomPattern::new(
        atom(&[1, 2]).predicate().clone(),
        vec![Term::Constant(Value::Number(1)), Term::Variable(0)],
    )
    .unwrap();
    let binding = [Some(Value::Number(2))];
    let initial_work = 7;
    let counters_at_entry = || Counters {
        work: initial_work,
        ..Counters::default()
    };
    let mut counters = counters_at_entry();
    support
        .probe(
            &pattern,
            &binding,
            &FormulaLimits::default(),
            &mut counters,
            location(),
        )
        .unwrap();
    assert!(counters.work > initial_work + pattern.terms().len() as u64);
    let exact = FormulaLimits {
        max_work: counters.work,
        ..FormulaLimits::default()
    };
    assert_eq!(
        support
            .probe(
                &pattern,
                &binding,
                &exact,
                &mut counters_at_entry(),
                location()
            )
            .unwrap(),
        Some([0].as_slice())
    );
    let below = FormulaLimits {
        max_work: counters.work - 1,
        ..exact
    };
    assert!(matches!(
        support.probe(&pattern, &binding, &below, &mut counters_at_entry(), location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work, observed, limit, location: found,
        }) if observed == u128::from(counters.work)
            && limit == u128::from(counters.work - 1) && found == location()
    ));
}

#[test]
fn relation_refusal_retains_the_source_location() {
    let error = failure(Failure::Allocation, location());
    let FormulaFailure::SupportRelation {
        error: Failure::Allocation,
        location: found,
    } = error
    else {
        panic!("typed relation refusal");
    };
    assert_eq!(found, location());
}
