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
    *catalog = std::mem::take(catalog)
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

fn contains(support: &Relations<'_>, atom: &Atom) -> bool {
    let pattern = AtomPattern::new(
        atom.predicate().clone(),
        atom.values().iter().cloned().map(Term::Constant).collect(),
    )
    .unwrap();
    support
        .contains(
            &pattern.key(&[] as &[Value]).unwrap(),
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
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
            assert!(contains(&support, &atom(&[value])));
        }
        assert!(!contains(&support, &atom(&[0])));
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
fn snapshots_reuse_columns_without_row_work() {
    let mut catalog = SupportCatalog::default();
    for value in 0..64 {
        insert(&mut catalog, atom(&[value]));
    }
    let mut counters = Counters::default();
    let first = catalog
        .snapshot(&FormulaLimits::default(), &mut counters, location())
        .unwrap();
    let first_work = counters.work;
    let second = catalog
        .snapshot(&FormulaLimits::default(), &mut counters, location())
        .unwrap();
    assert_eq!(first_work, 1);
    assert_eq!(counters.work, 2);
    let predicate = atom(&[0]);
    let left = &first.rows[predicate.predicate()];
    let right = &second.rows[predicate.predicate()];
    assert!(std::ptr::eq(left.columns, right.columns));
    assert!(std::ptr::eq(
        left.relation.column(0).unwrap(),
        right.relation.column(0).unwrap()
    ));
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
    assert!(contains(&support, &negative));
    assert!(!contains(&support, &positive));
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
fn snapshot_bytes_include_the_borrowed_owner() {
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
    let exact = support.bytes;
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
    assert!(matches!(
        catalog.snapshot(&below, &mut Counters::default(), location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            limit,
            observed,
            location: actual_location,
        }) if limit == (exact - 1) as u128
            && observed == exact as u128
            && actual_location == location()
    ));
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

#[test]
fn membership_preserves_cumulative_work_limits() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, atom(&[3]));
    let limits = FormulaLimits::default();
    let support = catalog
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let pattern =
        AtomPattern::new(atom(&[3]).predicate().clone(), vec![Term::Variable(0)]).unwrap();
    let values = [Some(Value::Number(3))];
    let key = pattern.key(values.as_slice()).unwrap();
    let initial = 7;
    let fresh = || Counters {
        work: initial,
        ..Counters::default()
    };
    let mut counters = fresh();
    assert!(
        support
            .contains(&key, &limits, &mut counters, location())
            .unwrap()
    );
    assert!(counters.work > initial);
    let exact = FormulaLimits {
        max_work: counters.work,
        ..limits
    };
    assert!(
        support
            .contains(&key, &exact, &mut fresh(), location())
            .unwrap()
    );
    let short = FormulaLimits {
        max_work: counters.work - 1,
        ..exact
    };
    assert!(
        matches!(support.contains(&key, &short, &mut fresh(), location()), Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, location: found }) if observed == u128::from(counters.work) && limit == u128::from(counters.work - 1) && found == location())
    );
}
