use super::{Atom, Failure, Limits, Relation, Resource, Value, atoms, limit, predicate};
use crate::catalog::TermRef;

#[test]
fn masks_preserve_typed_conjunctions_over_the_input() {
    let signature = predicate(2);
    let values = [
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
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
    let input = relation
        .selection(&[0, 1, 3, 5, 7, 9, 10, 15], Limits::default())
        .unwrap();
    for left in &values {
        for right in &values {
            let query = relation
                .query(
                    &[(0, TermRef::from(left)), (1, TermRef::from(right))],
                    Limits::default(),
                )
                .unwrap();
            let mask = relation
                .select_mask(&query, &input, Limits::default())
                .unwrap();
            let expected: Vec<_> = input
                .positions()
                .iter()
                .copied()
                .filter(|&position| source[position].values() == [left.clone(), right.clone()])
                .collect();
            let decoded = relation
                .selection_from_mask(mask.words(), Limits::default())
                .unwrap();
            let selected = relation.select(&query, &input, Limits::default()).unwrap();
            assert_eq!(decoded.positions(), expected);
            assert_eq!(decoded.positions(), selected.positions());
            assert!(relation.same_owner(mask.relation()));
            assert_eq!(mask.work(), selected.work() + 1);
        }
    }
}

#[test]
fn vacuous_masks_preserve_row_boundaries() {
    let signature = predicate(0);
    for count in [0_usize, 1, 31, 32, 33, 63, 64, 65, 129] {
        let source = vec![Atom::new(signature.clone(), vec![]).unwrap(); count];
        let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
        let input = relation.all(Limits::default()).unwrap();
        let query = relation.query(&[], Limits::default()).unwrap();
        let mask = relation
            .select_mask(&query, &input, Limits::default())
            .unwrap();
        assert_eq!(mask.words().len(), count.div_ceil(32));
        for (position, &word) in mask.words().iter().enumerate() {
            let remaining = (count - position * 32).min(32);
            let expected = u32::MAX >> (32 - remaining);
            assert_eq!(word, expected);
        }
        assert_eq!(mask.work(), (2 * count + count.div_ceil(32)) as u128);
        let selected = relation
            .selection_from_mask(mask.words(), Limits::default())
            .unwrap();
        assert_eq!(selected.positions(), input.positions());
    }
}

#[test]
fn contradictory_equalities_produce_zero_masks() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![vec![Value::Number(1)], vec![Value::Number(2)]],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let query = relation
        .query(
            &[
                (0, TermRef::from(&Value::Number(1))),
                (0, TermRef::from(&Value::Number(2))),
            ],
            Limits::default(),
        )
        .unwrap();
    let mask = relation
        .select_mask(&query, &input, Limits::default())
        .unwrap();
    assert_eq!(mask.words(), &[0]);
    // Two row visits and three short-circuited comparisons, plus one zero write.
    assert_eq!(mask.work(), 6);
}

#[test]
fn mask_selection_requires_both_exact_owners() {
    let signature = predicate(0);
    let source = [Atom::new(signature.clone(), vec![]).unwrap()];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let foreign = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let query = relation.query(&[], Limits::default()).unwrap();
    let foreign_query = foreign.query(&[], Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let foreign_input = foreign.all(Limits::default()).unwrap();
    let no_allocation = Limits {
        max_bytes: 0,
        ..Limits::default()
    };
    for result in [
        relation.select_mask(&foreign_query, &input, no_allocation),
        relation.select_mask(&query, &foreign_input, no_allocation),
    ] {
        assert!(matches!(result, Err(Failure::Owner)));
    }
}

#[test]
fn missing_values_still_charge_zero_mask_storage() {
    let signature = predicate(1);
    let source = atoms(&signature, vec![vec![Value::Number(1)]; 65]);
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let query = relation
        .query(&[(0, TermRef::from(&Value::Number(2)))], Limits::default())
        .unwrap();
    let mask = relation
        .select_mask(&query, &input, Limits::default())
        .unwrap();
    assert_eq!(mask.words(), &[0, 0, 0]);
    assert_eq!(mask.work(), 3);
    let exact = relation.storage().retained_bytes
        + query.retained_bytes()
        + input.retained_bytes()
        + mask.retained_bytes();
    drop(mask);
    let tight = Limits {
        max_bytes: exact,
        max_work: 3,
        ..Limits::default()
    };
    assert_eq!(
        relation.select_mask(&query, &input, tight).unwrap().words(),
        &[0, 0, 0]
    );
    limit(
        &relation.select_mask(
            &query,
            &input,
            Limits {
                max_bytes: exact - 1,
                ..tight
            },
        ),
        Resource::Bytes,
    );
    limit(
        &relation.select_mask(
            &query,
            &input,
            Limits {
                max_work: 2,
                ..tight
            },
        ),
        Resource::Work,
    );
}

#[test]
fn mask_limit_failure_does_not_publish_selected_prefixes() {
    let signature = predicate(0);
    let source = vec![Atom::new(signature.clone(), vec![]).unwrap(); 33];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let query = relation.query(&[], Limits::default()).unwrap();
    let mask = relation
        .select_mask(&query, &input, Limits::default())
        .unwrap();
    assert_eq!(mask.work(), 68);
    let exact = relation.storage().retained_bytes
        + query.retained_bytes()
        + input.retained_bytes()
        + mask.retained_bytes();
    drop(mask);
    let tight = Limits {
        max_bytes: exact,
        max_work: 68,
        ..Limits::default()
    };
    limit(
        &relation.select_mask(
            &query,
            &input,
            Limits {
                max_work: 67,
                ..tight
            },
        ),
        Resource::Work,
    );
    limit(
        &relation.select_mask(
            &query,
            &input,
            Limits {
                max_bytes: exact - 1,
                ..tight
            },
        ),
        Resource::Bytes,
    );
    assert_eq!(
        relation.select_mask(&query, &input, tight).unwrap().words(),
        &[u32::MAX, 1]
    );
    assert_eq!(input.positions().len(), 33);
}

#[test]
fn empty_input_still_has_original_row_mask_shape() {
    let signature = predicate(0);
    let source = vec![Atom::new(signature.clone(), vec![]).unwrap(); 33];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let input = relation.selection(&[], Limits::default()).unwrap();
    let query = relation.query(&[], Limits::default()).unwrap();
    let mask = relation
        .select_mask(&query, &input, Limits::default())
        .unwrap();
    assert_eq!(mask.words(), &[0, 0]);
    assert_eq!(mask.work(), 2);
}

#[test]
fn masks_preserve_repeated_catalog_occurrences() {
    let signature = crate::Predicate::with_sign("relation", 1, crate::Sign::Negative).unwrap();
    let source = atoms(
        &signature,
        vec![vec![Value::Number(4)], vec![Value::Number(5)]],
    );
    let indices = [1, 0, 1, 0];
    let relation =
        Relation::from_catalog(&signature, &source, &indices, Limits::default()).unwrap();
    let input = relation.selection(&[0, 1, 2], Limits::default()).unwrap();
    let query = relation
        .query(&[(0, TermRef::from(&Value::Number(5)))], Limits::default())
        .unwrap();
    let mask = relation
        .select_mask(&query, &input, Limits::default())
        .unwrap();
    assert_eq!(mask.words(), &[5]);
    let selected = relation
        .selection_from_mask(mask.words(), Limits::default())
        .unwrap();
    assert_eq!(selected.positions(), &[0, 2]);
    for row in 0..2 {
        assert_eq!(selected.row(row).unwrap().source_index(), 1);
        assert_eq!(
            selected.row(row).unwrap().predicate(),
            crate::catalog::PredicateRef::from(&signature)
        );
    }
}
