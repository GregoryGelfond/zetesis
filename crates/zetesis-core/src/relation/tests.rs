use super::*;

mod masks;
mod mask_selection;

fn predicate(arity: usize) -> Predicate {
    Predicate::new("relation", arity).unwrap()
}

fn atoms(predicate: &Predicate, values: Vec<Vec<Value>>) -> Vec<Atom> {
    values
        .into_iter()
        .map(|values| Atom::new(predicate.clone(), values).unwrap())
        .collect()
}

fn limit<T>(result: &Result<T, Failure>, resource: Resource) {
    assert!(matches!(result, Err(Failure::Limit { resource: actual, .. }) if *actual == resource));
}

#[test]
fn row_values_borrow_the_source_beyond_the_view() {
    let signature = predicate(1);
    let source = atoms(&signature, vec![vec![Value::Number(7)]]);
    let value = {
        let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
        relation.row(0).unwrap().value(0).unwrap()
    };
    assert_eq!(value, &Value::Number(7));
}

#[test]
fn columns_reconstruct_every_typed_argument() {
    let signature = predicate(2);
    let values = [
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
        Value::Infimum,
        Value::Supremum,
        Value::from_nodes(
            vec![
                crate::ValueNode::Tuple { arity: 1 },
                crate::ValueNode::Number(1),
            ],
            crate::ValueLimits::default(),
        )
        .unwrap(),
    ];
    let source = atoms(
        &signature,
        values
            .iter()
            .flat_map(|left| {
                values
                    .iter()
                    .map(move |right| vec![left.clone(), right.clone()])
            })
            .collect(),
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    for (position, expected) in source.iter().enumerate() {
        let row = relation.row(position).unwrap();
        assert_eq!(row.predicate(), expected.predicate());
        assert_eq!(row.source_index(), position);
        for (column, value) in expected.values().iter().enumerate() {
            assert_eq!(row.value(column), Some(value));
        }
    }
}

#[test]
fn source_occurrences_are_not_deduplicated() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(4)]],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    assert_eq!(relation.row_count(), 2);
    assert_eq!(
        relation.all(Limits::default()).unwrap().positions(),
        &[0, 1]
    );
}

#[test]
fn catalog_views_preserve_original_indices() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![
            vec![Value::Number(4)],
            vec![Value::Number(5)],
            vec![Value::Number(6)],
        ],
    );
    let indices = [2, 0, 2];
    let relation =
        Relation::from_catalog(&signature, &source, &indices, Limits::default()).unwrap();
    let selected = relation.all(Limits::default()).unwrap();
    let actual: Vec<_> = (0..indices.len())
        .map(|index| {
            let row = selected.row(index).unwrap();
            (
                row.position(),
                row.source_index(),
                row.value(0).unwrap().clone(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (0, 2, Value::Number(6)),
            (1, 0, Value::Number(4)),
            (2, 2, Value::Number(6))
        ]
    );
}

#[test]
fn foreign_signed_predicate_is_refused() {
    let signature = predicate(0);
    let negative = Predicate::with_sign("relation", 0, crate::Sign::Negative).unwrap();
    let source = atoms(&negative, vec![vec![]]);
    assert!(matches!(
        Relation::from_atoms(&signature, &source, Limits::default()),
        Err(Failure::Predicate)
    ));
}

#[test]
fn catalog_indices_are_checked() {
    let signature = predicate(0);
    let source = atoms(&signature, vec![vec![]]);
    assert!(matches!(
        Relation::from_catalog(&signature, &source, &[1], Limits::default()),
        Err(Failure::CatalogIndex)
    ));
}

#[test]
fn nullary_row_count_remains_explicit() {
    let signature = predicate(0);
    let source = atoms(&signature, vec![vec![]]);
    let singleton = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let empty = Relation::from_atoms(&signature, &[], Limits::default()).unwrap();
    assert!(singleton.columns().is_empty());
    assert!(empty.columns().is_empty());
    assert_eq!(singleton.row_count(), 1);
    assert_eq!(empty.row_count(), 0);
}

#[test]
fn absent_positions_return_absence() {
    let signature = predicate(1);
    let source = atoms(&signature, vec![vec![Value::Number(4)]]);
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    assert!(relation.row(1).is_none());
    assert!(relation.column(1).is_none());
    assert!(relation.row(0).unwrap().value(1).is_none());
    assert!(relation.all(Limits::default()).unwrap().row(1).is_none());
}

#[test]
fn selection_matches_independent_row_equality() {
    let signature = predicate(2);
    let source = atoms(
        &signature,
        (0..3)
            .flat_map(|left| {
                (0..3).map(move |right| vec![Value::Number(left), Value::Number(right)])
            })
            .collect(),
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation
        .selection(&[0, 2, 3, 5, 8], Limits::default())
        .unwrap();
    for left in -1..4 {
        for right in -1..4 {
            let values = [Value::Number(left), Value::Number(right)];
            for bound in [
                vec![],
                vec![(0, &values[0])],
                vec![(1, &values[1])],
                vec![(0, &values[0]), (1, &values[1])],
            ] {
                let query = relation.query(&bound, Limits::default()).unwrap();
                let selected = relation.select(&query, &input, Limits::default()).unwrap();
                let expected: Vec<_> = input
                    .positions()
                    .iter()
                    .copied()
                    .filter(|row| {
                        bound
                            .iter()
                            .all(|(column, value)| &source[*row].values()[*column] == *value)
                    })
                    .collect();
                assert_eq!(selected.positions(), expected);
            }
        }
    }
}

#[test]
fn empty_equalities_preserve_the_input() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(5)]],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation.selection(&[1], Limits::default()).unwrap();
    let query = relation.query(&[], Limits::default()).unwrap();
    assert!(query.is_possible());
    assert!(query.equalities().is_empty());
    assert_eq!(
        relation
            .select(&query, &input, Limits::default())
            .unwrap()
            .positions(),
        &[1]
    );
}

#[test]
fn missing_dictionary_values_select_nothing() {
    let signature = predicate(1);
    let source = atoms(&signature, vec![vec![Value::Number(4)]]);
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let query = relation
        .query(&[(0, &Value::Number(8))], Limits::default())
        .unwrap();
    assert!(!query.is_possible());
    assert!(
        relation
            .select(&query, &input, Limits::default())
            .unwrap()
            .positions()
            .is_empty()
    );
}

#[test]
fn repeated_columns_remain_conjunctive() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(5)]],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let query = relation
        .query(
            &[(0, &Value::Number(4)), (0, &Value::Number(5))],
            Limits::default(),
        )
        .unwrap();
    assert!(query.is_possible());
    assert_eq!(query.equalities().len(), 2);
    assert_eq!(query.equalities()[0].column(), 0);
    assert_ne!(
        query.equalities()[0].value_id(),
        query.equalities()[1].value_id()
    );
    assert!(
        relation
            .select(&query, &input, Limits::default())
            .unwrap()
            .positions()
            .is_empty()
    );
}

#[test]
fn missing_values_do_not_hide_invalid_columns() {
    let signature = predicate(1);
    let source = atoms(&signature, vec![vec![Value::Number(4)]]);
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    assert!(matches!(
        relation.query(
            &[(0, &Value::Number(8)), (1, &Value::Number(4))],
            Limits::default()
        ),
        Err(Failure::Column)
    ));
}

#[test]
fn malformed_row_selections_are_refused() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(5)]],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    for positions in [&[1, 0][..], &[0, 0], &[2]] {
        assert!(matches!(
            relation.selection(positions, Limits::default()),
            Err(Failure::Selection)
        ));
    }
}

#[test]
fn equal_contents_do_not_establish_owner_identity() {
    let signature = predicate(1);
    let source = atoms(&signature, vec![vec![Value::Number(4)]]);
    let first = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let second = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let query = first.query(&[], Limits::default()).unwrap();
    let input = first.all(Limits::default()).unwrap();
    assert!(first.same_owner(query.relation()));
    assert!(first.same_owner(input.relation()));
    assert!(!first.same_owner(&second));
    assert!(matches!(
        second.select(&query, &input, Limits::default()),
        Err(Failure::Owner)
    ));
}

#[test]
fn foreign_input_is_refused_with_a_local_query() {
    let signature = predicate(0);
    let first = Relation::from_atoms(&signature, &[], Limits::default()).unwrap();
    let second = Relation::from_atoms(&signature, &[], Limits::default()).unwrap();
    let query = first.query(&[], Limits::default()).unwrap();
    let input = second.all(Limits::default()).unwrap();
    assert!(matches!(
        first.select(&query, &input, Limits::default()),
        Err(Failure::Owner)
    ));
}

#[test]
fn construction_respects_each_resource_ceiling() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(5)]],
    );
    for (limits, resource) in [
        (
            Limits {
                max_rows: 1,
                ..Limits::default()
            },
            Resource::Rows,
        ),
        (
            Limits {
                max_columns: 0,
                ..Limits::default()
            },
            Resource::Columns,
        ),
        (
            Limits {
                max_values: 1,
                ..Limits::default()
            },
            Resource::Values,
        ),
        (
            Limits {
                max_bytes: 0,
                ..Limits::default()
            },
            Resource::Bytes,
        ),
        (
            Limits {
                max_work: 0,
                ..Limits::default()
            },
            Resource::Work,
        ),
    ] {
        limit(&Relation::from_atoms(&signature, &source, limits), resource);
    }
}

#[test]
fn filtering_charges_all_supplied_live_objects() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(5)]],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let query = relation.query(&[], Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let result = relation.select(&query, &input, Limits::default()).unwrap();
    let bytes = relation.storage().retained_bytes
        + query.retained_bytes()
        + input.retained_bytes()
        + result.retained_bytes();
    drop(result);
    let exact = Limits {
        max_bytes: bytes,
        ..Limits::default()
    };
    assert_eq!(
        relation.select(&query, &input, exact).unwrap().positions(),
        input.positions()
    );
    limit(
        &relation.select(
            &query,
            &input,
            Limits {
                max_bytes: bytes - 1,
                ..Limits::default()
            },
        ),
        Resource::Bytes,
    );
}

#[test]
fn a_filter_stop_preserves_its_inputs() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(5)]],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let query = relation
        .query(&[(0, &Value::Number(4))], Limits::default())
        .unwrap();
    let input = relation.all(Limits::default()).unwrap();
    limit(
        &relation.select(
            &query,
            &input,
            Limits {
                max_work: 1,
                ..Limits::default()
            },
        ),
        Resource::Work,
    );
    assert_eq!(input.positions(), &[0, 1]);
    assert_eq!(query.equalities().len(), 1);
    assert_eq!(
        relation
            .select(&query, &input, Limits::default())
            .unwrap()
            .positions(),
        &[0]
    );
}

#[test]
fn construction_records_transient_sort_capacity() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        (0..16)
            .rev()
            .map(|number| vec![Value::Number(number)])
            .collect(),
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let storage = relation.storage();
    assert!(storage.peak_construction_bytes > storage.retained_bytes);
    assert_eq!(storage.referenced_payload_bytes, 0);
    assert_eq!(storage.borrowed_mapping_bytes, 0);
    assert!(storage.construction_work > 0);
}
