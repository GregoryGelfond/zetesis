use proptest::prelude::*;

use super::{Catalog, CatalogFailure};
use crate::relation::{Failure, Limits, Relation, Resource};
use crate::{Atom, Predicate, Sign, Value, ValueLimits, ValueNode};

fn atom(left: i32, right: i32) -> Atom {
    Atom::new(
        Predicate::new("pair", 2).unwrap(),
        vec![Value::Number(left), Value::Number(right)],
    )
    .unwrap()
}

fn owner() -> Catalog {
    Catalog::new(Predicate::new("pair", 2).unwrap(), Limits::default()).unwrap()
}

/// Row IDs of a prepared catalog in canonical order, merged from its runs.
fn ids(catalog: &Catalog) -> Vec<usize> {
    catalog.canonical(Limits::default()).expect("prepared").ids
}

#[test]
fn insertion_preserves_existing_equality_ids() {
    let mut catalog = owner();
    catalog.insert(atom(9, 3), Limits::default()).unwrap();
    let before: Vec<Vec<u32>> = catalog.view().columns().map(<[u32]>::to_vec).collect();
    for tuple in [atom(1, 8), atom(8, 1), atom(3, 3), atom(0, 9)] {
        catalog.insert(tuple, Limits::default()).unwrap();
    }
    let view = catalog.view();
    for (column, previous) in before.iter().enumerate() {
        assert_eq!(&view.column(column).unwrap()[..previous.len()], previous);
    }
    for (row, original) in catalog.atoms().iter().enumerate() {
        for column in 0..2 {
            assert_eq!(
                view.row(row).unwrap().value(column).unwrap(),
                &original.values()[column]
            );
        }
    }
}

#[test]
fn duplicate_insertion_reuses_the_original_row() {
    let mut catalog = owner();
    assert_eq!(
        catalog.insert(atom(4, 7), Limits::default()).unwrap().row,
        0
    );
    let duplicate = catalog.insert(atom(4, 7), Limits::default()).unwrap();
    assert!(!duplicate.inserted);
    assert_eq!(duplicate.row, 0);
    assert_eq!(catalog.into_atoms(), [atom(4, 7)]);
}

#[test]
fn failed_work_admission_preserves_all_rows() {
    let mut reference = owner();
    reference.insert(atom(4, 7), Limits::default()).unwrap();
    let required = reference
        .insert(atom(1, 9), Limits::default())
        .unwrap()
        .storage
        .construction_work;
    for limit in 0..required {
        let mut catalog = owner();
        catalog.insert(atom(4, 7), Limits::default()).unwrap();
        assert!(matches!(
            catalog.insert(
                atom(1, 9),
                Limits {
                    max_work: u64::try_from(limit).unwrap(),
                    ..Default::default()
                }
            ),
            Err(CatalogFailure {
                error: Failure::Limit {
                    resource: Resource::Work,
                    ..
                },
                ..
            })
        ));
        assert_eq!(catalog.atoms(), &[atom(4, 7)]);
        let view = catalog.view();
        assert_eq!(view.row_count(), 1);
        assert!(view.columns().all(|column| column.len() == 1));
        assert_eq!(view.row(0).unwrap().value(0), Some(&Value::Number(4)));
        assert!(
            catalog
                .insert(atom(1, 9), Limits::default())
                .unwrap()
                .inserted
        );
    }
}

#[test]
fn failed_value_admission_publishes_no_dictionary_entries() {
    let mut catalog = owner();
    catalog.insert(atom(4, 4), Limits::default()).unwrap();
    assert!(matches!(
        catalog.insert(
            atom(1, 9),
            Limits {
                max_values: 2,
                ..Default::default()
            }
        ),
        Err(CatalogFailure {
            error: Failure::Limit {
                resource: Resource::Values,
                ..
            },
            ..
        })
    ));
    assert_eq!(catalog.atoms(), &[atom(4, 4)]);
    let result = catalog.insert(atom(1, 9), Limits::default()).unwrap();
    assert_eq!(result.row, 1);
    assert!(result.inserted);
}

#[test]
fn views_do_not_rebuild_the_layout() {
    let mut catalog = owner();
    for value in 0..65 {
        catalog
            .insert(atom(value, value + 1), Limits::default())
            .unwrap();
    }
    let first = catalog.view();
    let second = catalog.view();
    assert_eq!(first.storage().construction_work, 0);
    assert_eq!(second.storage().construction_work, 0);
    assert!(std::ptr::eq(
        first.column(0).unwrap().as_ptr(),
        second.column(0).unwrap().as_ptr()
    ));
    let query = first.query(&[], Limits::default()).unwrap();
    let rows = second.all(Limits::default()).unwrap();
    assert!(matches!(
        second.select(&query, &rows, Limits::default()),
        Err(Failure::Owner)
    ));
}

#[test]
fn construction_reports_its_completed_column_work() {
    let catalog = owner();
    assert_eq!(catalog.construction().construction_work, 2);
    assert_eq!(
        catalog.construction().retained_bytes,
        catalog.retained_bytes()
    );
    let Err(failure) = Catalog::new(
        Predicate::new("pair", 2).unwrap(),
        Limits {
            max_work: 1,
            ..Default::default()
        },
    ) else {
        panic!("the second column must exceed the work allowance");
    };
    assert_eq!(failure.work, 1);
    assert_eq!(failure.retained_bytes, 0);
    assert_eq!(
        failure.error,
        Failure::Limit {
            resource: Resource::Work,
            observed: 2,
            limit: 1
        }
    );
}

#[test]
fn lookup_reports_inclusive_work_admission() {
    let mut catalog = owner();
    catalog.insert(atom(4, 7), Limits::default()).unwrap();
    let receipt = catalog.lookup(&atom(4, 7), Limits::default()).unwrap();
    assert_eq!(receipt.row, Some(0));
    let required = u64::try_from(receipt.storage.construction_work).unwrap();
    assert!(required > 0);
    let exact = catalog
        .lookup(
            &atom(4, 7),
            Limits {
                max_work: required,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(exact, receipt);
    let short = catalog
        .lookup(
            &atom(4, 7),
            Limits {
                max_work: required - 1,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(short.work < u128::from(required));
    assert_eq!(short.retained_bytes, catalog.retained_bytes());
    assert_eq!(
        short.error,
        Failure::Limit {
            resource: Resource::Work,
            observed: u128::from(required),
            limit: u128::from(required - 1)
        }
    );
}

#[test]
fn borrowed_views_do_not_duplicate_owner_capacity() {
    let mut catalog = owner();
    catalog.insert(atom(4, 7), Limits::default()).unwrap();
    let first = catalog.view();
    let second = catalog.view();
    assert_eq!(
        first.storage().retained_bytes,
        std::mem::size_of_val(&first)
    );
    assert_eq!(
        second.storage().retained_bytes,
        std::mem::size_of_val(&second)
    );
    assert!(catalog.retained_bytes() > first.storage().retained_bytes);
}

#[test]
fn sorted_positions_borrow_the_original_atoms() {
    let mut catalog = owner();
    for value in [9, 2, 5] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    assert_eq!(ids(&catalog), [1, 2, 0]);
    assert_eq!(catalog.ordered().unwrap().len(), 3);
}

#[test]
fn append_dictionary_preserves_complete_value_kinds() {
    let mut catalog = owner();
    let compound = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
        ValueLimits::default(),
    )
    .unwrap();
    let values = [
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
        compound,
        Value::Infimum,
        Value::Supremum,
    ];
    for value in &values {
        catalog
            .insert(
                Atom::new(
                    catalog.predicate().clone(),
                    vec![value.clone(), value.clone()],
                )
                .unwrap(),
                Limits::default(),
            )
            .unwrap();
    }
    for (row, value) in values.iter().enumerate() {
        let view = catalog.view();
        assert_eq!(view.row(row).unwrap().value(0), Some(value));
        assert_eq!(view.row(row).unwrap().value(1), Some(value));
        assert_eq!(
            catalog
                .lookup(&catalog.atoms()[row], Limits::default())
                .unwrap()
                .row,
            Some(row)
        );
    }
    assert_eq!(catalog.layout.dictionary.len(), values.len());
}

#[test]
fn opposite_predicate_sign_refuses_without_publication() {
    let mut catalog = owner();
    let negative = Atom::new(
        Predicate::with_sign("pair", 2, Sign::Negative).unwrap(),
        vec![Value::Number(1), Value::Number(2)],
    )
    .unwrap();
    assert_eq!(
        catalog
            .insert(negative, Limits::default())
            .unwrap_err()
            .error,
        Failure::Predicate
    );
    assert!(catalog.atoms().is_empty());
}

#[test]
fn nullary_append_retains_one_empty_tuple() {
    let predicate = Predicate::new("p", 0).unwrap();
    let mut catalog = Catalog::new(predicate.clone(), Limits::default()).unwrap();
    let atom = Atom::new(predicate, vec![]).unwrap();
    assert!(
        catalog
            .insert(atom.clone(), Limits::default())
            .unwrap()
            .inserted
    );
    assert!(!catalog.insert(atom, Limits::default()).unwrap().inserted);
    assert_eq!(catalog.view().row_count(), 1);
    assert_eq!(catalog.view().columns().len(), 0);
}

#[test]
fn insertion_admits_exact_peak_capacity() {
    let mut reference = owner();
    reference.insert(atom(4, 7), Limits::default()).unwrap();
    let peak = reference
        .insert(atom(1, 9), Limits::default())
        .unwrap()
        .storage
        .peak_construction_bytes;
    let mut exact = owner();
    exact.insert(atom(4, 7), Limits::default()).unwrap();
    assert!(
        exact
            .insert(
                atom(1, 9),
                Limits {
                    max_bytes: peak,
                    ..Default::default()
                }
            )
            .unwrap()
            .inserted
    );
    let mut short = owner();
    short.insert(atom(4, 7), Limits::default()).unwrap();
    let failure = short
        .insert(
            atom(1, 9),
            Limits {
                max_bytes: peak - 1,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        failure.error,
        Failure::Limit {
            resource: Resource::Bytes,
            ..
        }
    ));
    assert_eq!(short.atoms(), &[atom(4, 7)]);
    assert_eq!(failure.retained_bytes, short.retained_bytes());
    assert!(
        short
            .insert(atom(1, 9), Limits::default())
            .unwrap()
            .inserted
    );
}

proptest! {
    #[test]
    fn append_views_match_bulk_typed_selection(tuples in prop::collection::vec((-8_i32..8,-8_i32..8),0..48), left in -9_i32..9, right in -9_i32..9) {
        let mut catalog=owner();
        for (x,y) in tuples { catalog.insert(atom(x,y),Limits::default()).unwrap(); }
        let expected=catalog.atoms().iter().enumerate().filter_map(|(row,atom)|(atom.values()==[Value::Number(left),Value::Number(right)]).then_some(row)).collect::<Vec<_>>();
        let view=catalog.view();
        let bulk=Relation::from_atoms(catalog.predicate(),catalog.atoms(),Limits::default()).unwrap();
        for relation in [&view,&bulk] {
            let l=Value::Number(left);let r=Value::Number(right);
            let query=relation.query(&[(0,&l),(1,&r)],Limits::default()).unwrap();
            let rows=relation.all(Limits::default()).unwrap();
            let selected = relation.select(&query,&rows,Limits::default()).unwrap();
            prop_assert_eq!(selected.positions(),expected.as_slice());
        }
    }
}

#[test]
fn prepared_order_reuses_the_published_extent() {
    let mut catalog = owner();
    for value in [9, 2, 5] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    assert!(catalog.ordered().is_none());
    let preparation = catalog.prepare_ordered(Limits::default()).unwrap().storage;
    assert!(preparation.construction_work > 0);
    let reused = catalog
        .prepare_ordered(Limits {
            max_work: 0,
            ..Limits::default()
        })
        .unwrap();
    assert_eq!(reused.storage.construction_work, 0);
    assert_eq!(reused.storage.retained_bytes, preparation.retained_bytes);
    assert_eq!(reused.runs.len(), 3);
    for (position, value) in [2, 5, 9].into_iter().enumerate() {
        assert_eq!(catalog.atoms()[ids(&catalog)[position]], atom(value, 0));
    }
}

#[test]
fn duplicate_insert_preserves_prepared_order() {
    let mut catalog = owner();
    catalog.insert(atom(2, 9), Limits::default()).unwrap();
    catalog.prepare_ordered(Limits::default()).unwrap();
    assert!(
        !catalog
            .insert(atom(2, 9), Limits::default())
            .unwrap()
            .inserted
    );
    assert_eq!(ids(&catalog), [0]);
    assert_eq!(
        catalog
            .prepare_ordered(Limits {
                max_work: 0,
                ..Limits::default()
            })
            .unwrap()
            .storage
            .construction_work,
        0
    );
}

fn rotation_owner() -> Catalog {
    let predicate = Predicate::new("quad", 4).unwrap();
    let mut catalog = Catalog::new(predicate.clone(), Limits::default()).unwrap();
    catalog
        .insert(
            Atom::new(predicate, vec![Value::Number(40); 4]).unwrap(),
            Limits::default(),
        )
        .unwrap();
    catalog.prepare_ordered(Limits::default()).unwrap();
    catalog
}

fn rotating_tuple() -> Atom {
    Atom::new(
        Predicate::new("quad", 4).unwrap(),
        [30, 20, 10, 50].map(Value::Number).to_vec(),
    )
    .unwrap()
}

#[test]
fn refused_rotation_preserves_the_published_extent() {
    let mut reference = rotation_owner();
    let required = reference
        .insert(rotating_tuple(), Limits::default())
        .unwrap()
        .storage
        .construction_work;
    assert_eq!(reference.layout.dictionary.len(), 5);
    for limit in 0..required {
        let mut catalog = rotation_owner();
        let original = catalog.atoms().to_vec();
        let original_ids: Vec<Vec<u32>> = catalog.view().columns().map(<[u32]>::to_vec).collect();
        let failure = catalog
            .insert(
                rotating_tuple(),
                Limits {
                    max_work: u64::try_from(limit).unwrap(),
                    ..Limits::default()
                },
            )
            .unwrap_err();
        assert!(matches!(failure.error, Failure::Limit {
            resource: Resource::Work, observed, limit: bound
        } if bound == limit && observed > bound));
        assert!(failure.work <= limit);
        assert_eq!(catalog.atoms(), original);
        assert_eq!(catalog.layout.dictionary.len(), 1);
        assert_eq!(
            catalog
                .view()
                .columns()
                .map(<[u32]>::to_vec)
                .collect::<Vec<_>>(),
            original_ids
        );
        let rows = catalog
            .ordered()
            .expect("refusal preserves prior preparation");
        assert_eq!(rows.len(), 1);
        assert_eq!(ids(&catalog), [0]);
        assert_eq!(
            catalog
                .lookup(&rotating_tuple(), Limits::default())
                .unwrap()
                .row,
            None
        );
        assert_eq!(failure.retained_bytes, catalog.retained_bytes());
    }
    let mut exact = rotation_owner();
    let insertion = exact
        .insert(
            rotating_tuple(),
            Limits {
                max_work: u64::try_from(required).unwrap(),
                ..Limits::default()
            },
        )
        .unwrap();
    assert_eq!(insertion.row, 1);
    assert!(exact.ordered().is_none());
    assert_eq!(
        exact
            .lookup(&rotating_tuple(), Limits::default())
            .unwrap()
            .row,
        Some(1)
    );
}

#[test]
fn refused_preparation_publishes_no_partial_order() {
    let mut reference = owner();
    for value in [9, 2, 5] {
        reference.insert(atom(value, 0), Limits::default()).unwrap();
    }
    let required = reference
        .prepare_ordered(Limits::default())
        .unwrap()
        .storage
        .construction_work;
    for limit in 0..required {
        let mut catalog = owner();
        for value in [9, 2, 5] {
            catalog.insert(atom(value, 0), Limits::default()).unwrap();
        }
        let Err(failure) = catalog.prepare_ordered(Limits {
            max_work: u64::try_from(limit).unwrap(),
            ..Limits::default()
        }) else {
            panic!("every shorter prefix must refuse preparation");
        };
        assert!(matches!(
            failure.error,
            Failure::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert!(catalog.ordered().is_none());
        assert_eq!(catalog.atoms(), [atom(9, 0), atom(2, 0), atom(5, 0)]);
        assert_eq!(
            catalog
                .prepare_ordered(Limits::default())
                .unwrap()
                .runs
                .len(),
            3
        );
        for (position, value) in [2, 5, 9].into_iter().enumerate() {
            assert_eq!(catalog.atoms()[ids(&catalog)[position]], atom(value, 0));
        }
    }
}

#[test]
fn nested_capacity_includes_owned_spare_storage() {
    // The predicate's name is shared and counted by its bytes; the values'
    // spare storage is owned and counted by capacity.
    let mut name = String::with_capacity(64);
    name.push('p');
    let mut text = String::with_capacity(128);
    text.push('x');
    let mut values = Vec::with_capacity(8);
    let expected = name.len() as u128
        + values.capacity() as u128 * std::mem::size_of::<Value>() as u128
        + text.capacity() as u128;
    values.push(Value::String(text));
    let atom = Atom::new(Predicate::new(name, 1).unwrap(), values).unwrap();
    assert_eq!(atom.checked_payload_capacity_bytes(), Some(expected));
    assert!(
        atom.checked_payload_capacity_bytes().unwrap() > atom.values()[0].payload_bytes() as u128
    );
}

#[test]
fn extraction_starts_a_new_catalog_extent() {
    let mut catalog = owner();
    catalog.insert(atom(9, 3), Limits::default()).unwrap();
    catalog.insert(atom(1, 8), Limits::default()).unwrap();
    catalog.prepare_ordered(Limits::default()).unwrap();
    let address = catalog.atoms().as_ptr();
    let extracted = catalog.take_atoms(Limits::default()).unwrap();
    assert_eq!(extracted.atoms.as_ptr(), address);
    assert_eq!(extracted.atoms, [atom(9, 3), atom(1, 8)]);
    assert!(catalog.atoms().is_empty());
    assert!(catalog.ordered().unwrap().is_empty());
    assert!(catalog.view().columns().all(<[u32]>::is_empty));
    assert_eq!(
        catalog.lookup(&atom(9, 3), Limits::default()).unwrap().row,
        None
    );
    let empty_capacity = catalog.retained_bytes();
    assert_eq!(extracted.storage.retained_bytes, empty_capacity);
    assert!(empty_capacity > owner().retained_bytes());
    let inserted = catalog.insert(atom(7, 2), Limits::default()).unwrap();
    assert_eq!(inserted.row, 0);
    assert_eq!(catalog.view().column(0), Some([0].as_slice()));
    assert_eq!(catalog.view().column(1), Some([1].as_slice()));
    assert_eq!(extracted.atoms, [atom(9, 3), atom(1, 8)]);
}

#[test]
fn refused_extraction_preserves_the_prepared_extent() {
    let mut catalog = owner();
    catalog.insert(atom(9, 3), Limits::default()).unwrap();
    catalog.prepare_ordered(Limits::default()).unwrap();
    let required = 13;
    let Err(failure) = catalog.take_atoms(Limits {
        max_work: required - 1,
        ..Limits::default()
    }) else {
        panic!("reset must be admitted before moving atoms");
    };
    assert_eq!(
        failure.error,
        Failure::Limit {
            resource: Resource::Work,
            observed: required.into(),
            limit: (required - 1).into()
        }
    );
    assert_eq!(catalog.atoms(), [atom(9, 3)]);
    assert_eq!(ids(&catalog), [0]);
    assert_eq!(catalog.view().column(0), Some([0].as_slice()));
    assert_eq!(catalog.view().column(1), Some([1].as_slice()));
    assert_eq!(
        catalog
            .take_atoms(Limits {
                max_work: required,
                ..Limits::default()
            })
            .unwrap()
            .atoms,
        [atom(9, 3)]
    );
}

#[test]
fn ordered_ids_preserve_rows_when_ranks_move() {
    let mut catalog = owner();
    assert!(catalog.ordered().unwrap().is_empty());
    for (id, value) in [9, 2, 5].into_iter().enumerate() {
        assert_eq!(
            catalog
                .insert(atom(value, 0), Limits::default())
                .unwrap()
                .row,
            id
        );
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    assert_eq!(ids(&catalog), [1, 2, 0]);
    assert_eq!(
        catalog.insert(atom(0, 0), Limits::default()).unwrap().row,
        3
    );
    catalog.prepare_ordered(Limits::default()).unwrap();
    let canonical = ids(&catalog);
    for (rank, (id, value)) in [(3, 0), (1, 2), (2, 5), (0, 9)].into_iter().enumerate() {
        assert_eq!(canonical[rank], id);
        assert_eq!(catalog.atoms()[id], atom(value, 0));
    }
    // The appended smaller tuple is its own run; the older rows kept theirs.
    let runs = catalog.ordered().unwrap();
    assert_eq!(runs.levels(), [vec![1, 2, 0]]);
    assert_eq!(runs.tail(), [3]);
}

#[test]
fn ordered_ids_preserve_complete_typed_identity() {
    let nested = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                arity: 1,
                sign: Sign::Negative,
            },
            ValueNode::Number(1),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    for sign in [Sign::Positive, Sign::Negative] {
        let predicate = Predicate::with_sign("typed", 1, sign).unwrap();
        let mut catalog = Catalog::new(predicate.clone(), Limits::default()).unwrap();
        let values = [
            Value::String("1".into()),
            Value::Number(1),
            nested.clone(),
            Value::Symbol("1".into()),
            Value::Infimum,
            Value::Supremum,
        ];
        for value in &values {
            catalog
                .insert(
                    Atom::new(predicate.clone(), vec![value.clone()]).unwrap(),
                    Limits::default(),
                )
                .unwrap();
        }
        catalog.prepare_ordered(Limits::default()).unwrap();
        // Storage order: infimum, integer, string, symbol, constructor, supremum.
        // It is independent of printed spelling and differs from ASP term order.
        assert_eq!(ids(&catalog), [4, 1, 0, 3, 2, 5]);
        for id in [4, 1, 0, 3, 2, 5] {
            let atom = &catalog.atoms()[id];
            assert_eq!(atom.predicate(), &predicate);
            assert_eq!(atom.values(), &[values[id].clone()]);
        }
    }
}

/// Two rows prepared, then two more appended and prepared: the first view
/// `[1, 0]` is the one level and the appended rows are the run `[3, 2]`.
fn prepared_twice() -> Catalog {
    let mut catalog = owner();
    for value in [4, 2] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    for value in [3, 1] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    catalog
}

#[test]
fn a_preparation_merges_the_appended_rows_into_the_view() {
    let mut catalog = owner();
    for value in [4, 2] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    assert_eq!(ids(&catalog), [1, 0]);
    for value in [3, 1] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    assert!(catalog.ordered().is_none());
    catalog.prepare_ordered(Limits::default()).unwrap();
    assert_eq!(ids(&catalog), [3, 1, 2, 0]);
    let runs = catalog.ordered().unwrap();
    assert_eq!(runs.levels(), [vec![1, 0]]);
    assert_eq!(runs.tail(), [3, 2]);
}

#[test]
fn a_preparation_with_nothing_appended_keeps_the_view_and_the_runs() {
    let mut catalog = prepared_twice();
    catalog
        .prepare_ordered(Limits {
            max_work: 0,
            ..Limits::default()
        })
        .unwrap();
    let runs = catalog.ordered().unwrap();
    assert_eq!(
        (runs.levels(), runs.tail()),
        (&[vec![1, 0]][..], &[3, 2][..])
    );
}

#[test]
fn the_first_preparation_is_one_run_over_nothing() {
    let mut catalog = owner();
    for value in [9, 2, 5] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    let runs = catalog.ordered().unwrap();
    assert!(runs.levels().is_empty());
    assert_eq!(runs.tail(), [1, 2, 0]);
}

#[test]
fn one_append_to_a_large_extent_costs_its_own_run_not_a_copy_of_the_extent() {
    let mut catalog = owner();
    for value in 0..2048 {
        catalog
            .insert(atom(value * 2, 0), Limits::default())
            .unwrap();
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    catalog.insert(atom(1, 0), Limits::default()).unwrap();
    let work = catalog
        .prepare_ordered(Limits::default())
        .unwrap()
        .storage
        .construction_work;
    // The previous run becomes a level unchanged and the new row is a run of
    // one; nothing of the 2,048 older rows is compared or copied.
    assert!(work < 64, "{work}");
    assert_eq!(ids(&catalog)[..3], [0, 2048, 1]);
    let runs = catalog.ordered().unwrap();
    assert_eq!(runs.levels().len(), 1);
    assert_eq!(runs.tail(), [2048]);
}

/// Append `count` rows one at a time, preparing after each and showing
/// `each` the levels after every preparation; the charged construction
/// work of all the preparations.
fn append_one_at_a_time(count: i32, mut each: impl FnMut(&[Vec<usize>])) -> u128 {
    let mut catalog = owner();
    let mut work = 0;
    for value in 0..count {
        catalog
            .insert(atom(value * 7 % count, value), Limits::default())
            .unwrap();
        work += catalog
            .prepare_ordered(Limits::default())
            .unwrap()
            .storage
            .construction_work;
        each(catalog.ordered().unwrap().levels());
    }
    assert_eq!(ids(&catalog).len(), usize::try_from(count).unwrap());
    work
}

#[test]
fn levels_shrink_by_more_than_half_at_every_step() {
    append_one_at_a_time(4096, |levels| {
        for pair in levels.windows(2) {
            assert!(
                pair[1].len() * 2 < pair[0].len(),
                "{:?}",
                levels.iter().map(Vec::len).collect::<Vec<_>>()
            );
        }
        assert!(levels.len() <= 13);
    });
}

#[test]
fn merging_rows_one_at_a_time_costs_n_log_n() {
    // n log n doubles to 2.18 times; n squared to 4 times.
    let (half, whole) = (
        append_one_at_a_time(2048, |_| {}),
        append_one_at_a_time(4096, |_| {}),
    );
    assert!(whole < half * 5 / 2, "{half} then {whole}");
}

proptest! {
    #[test]
    fn incremental_preparation_matches_a_rebuild(
        rows in proptest::collection::vec((-8i32..8, -8i32..8), 1..40),
        prepare in proptest::collection::vec(any::<bool>(), 40),
    ) {
        let mut incremental = owner();
        // The view at the last preparation that merged, and its runs; a
        // preparation with nothing appended keeps both.
        let mut merged_from: Vec<usize> = Vec::new();
        let mut runs: Option<(Vec<Vec<usize>>, Vec<usize>)> = None;
        for (index, (left, right)) in rows.iter().enumerate() {
            incremental.insert(atom(*left, *right), Limits::default()).unwrap();
            if prepare[index] {
                let grew = incremental.ordered().is_none();
                incremental.prepare_ordered(Limits::default()).unwrap();
                let mut rebuilt = owner();
                for (left, right) in &rows[..=index] {
                    rebuilt.insert(atom(*left, *right), Limits::default()).unwrap();
                }
                rebuilt.prepare_ordered(Limits::default()).unwrap();
                prop_assert_eq!(ids(&incremental), ids(&rebuilt));
                let canonical = ids(&incremental);
                let mut rank = vec![usize::MAX; canonical.len()];
                for (position, &id) in canonical.iter().enumerate() {
                    rank[id] = position;
                }
                let view = incremental.ordered().unwrap();
                let levels: Vec<Vec<usize>> = view.levels().to_vec();
                let tail = view.tail().to_vec();
                // Every run is in canonical order and the runs partition the rows.
                let mut union = Vec::new();
                for run in view.runs() {
                    prop_assert!(run.windows(2).all(|pair| rank[pair[0]] < rank[pair[1]]));
                    union.extend_from_slice(run);
                }
                union.sort_unstable();
                let mut all = canonical.clone();
                all.sort_unstable();
                prop_assert_eq!(union, all);
                prop_assert!(levels.windows(2).all(|pair| pair[1].len() * 2 < pair[0].len()));
                if grew {
                    // The rows before this preparation are exactly the levels.
                    let mut before: Vec<usize> = levels.iter().flatten().copied().collect();
                    before.sort_unstable();
                    let mut expected = merged_from.clone();
                    expected.sort_unstable();
                    prop_assert_eq!(before, expected);
                    merged_from = canonical.clone();
                    runs = Some((levels, tail));
                } else if let Some((kept_levels, kept_tail)) = &runs {
                    prop_assert_eq!(&levels, kept_levels);
                    prop_assert_eq!(&tail, kept_tail);
                }
            }
        }
    }
}

#[test]
fn the_merge_charges_its_run_slices_and_cursors_to_the_byte_ceiling() {
    // Beside the merged ids the merge holds one slice and one cursor per
    // run; all three are reserved through the work, so the peak reports
    // them and the byte ceiling bounds them.
    let mut catalog = owner();
    for value in [9, 2, 5] {
        catalog.insert(atom(value, 0), Limits::default()).unwrap();
    }
    catalog.prepare_ordered(Limits::default()).unwrap();
    catalog.insert(atom(7, 0), Limits::default()).unwrap();
    catalog.prepare_ordered(Limits::default()).unwrap();
    let runs = catalog.ordered().unwrap().runs().count();
    assert_eq!(runs, 2);
    let canonical = catalog.canonical(Limits::default()).unwrap();
    let scratch = canonical.storage.peak_construction_bytes - canonical.storage.retained_bytes;
    let ids = canonical.ids.len() * size_of::<usize>();
    let per_run = size_of::<&[usize]>() + size_of::<usize>();
    assert!(
        scratch >= ids + runs * per_run,
        "{scratch} < {ids} + {runs} * {per_run}"
    );
}
