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
    for (position, original) in [1, 2, 0].into_iter().enumerate() {
        assert!(std::ptr::eq(
            catalog.ordered_row(position).unwrap(),
            &raw const catalog.atoms()[original]
        ));
    }
    assert!(catalog.ordered_row(3).is_none());
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
