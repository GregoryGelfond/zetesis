//! Shared interpretation ownership preserves logical identity across catalogs.

use std::{cmp::Ordering, collections::BTreeSet};

use zetesis_core::{
    Atom, AtomCatalog, Model, ModelError, Predicate, Sign, Value, ValueLimits, ValueNode,
    catalog::{Catalog, Limits},
};

fn atom(sign: Sign, value: Value) -> Atom {
    Atom::new(Predicate::with_sign("p", 1, sign).unwrap(), vec![value]).unwrap()
}

fn fixture() -> Vec<Atom> {
    vec![
        atom(Sign::Positive, Value::Number(1)),
        atom(Sign::Negative, Value::Number(1)),
        atom(Sign::Positive, Value::String("1".into())),
        atom(Sign::Positive, Value::Symbol("1".into())),
        atom(
            Sign::Positive,
            Value::from_nodes(
                vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
                ValueLimits::default(),
            )
            .unwrap(),
        ),
    ]
}

#[test]
fn catalog_import_preserves_original_occurrences() {
    let mut atoms = fixture();
    atoms.push(atoms[0].clone());
    atoms.reverse();
    let expected = atoms.clone();
    let catalog = AtomCatalog::new(atoms).unwrap();
    assert_eq!(catalog.atoms().len(), expected.len());
    for (actual, expected) in catalog.atoms().iter().zip(&expected) {
        assert_eq!(actual.compare(expected), Ordering::Equal);
    }
}

#[test]
fn selection_matches_the_independent_logical_set() {
    let mut atoms = fixture();
    atoms.push(atoms[0].clone());
    atoms.reverse();
    let catalog = AtomCatalog::new(atoms.clone()).unwrap();
    for subset in 0..(1_usize << catalog.atoms().len()) {
        let positions: Vec<_> = (0..catalog.atoms().len())
            .filter(|index| subset & (1 << index) != 0)
            .rev()
            .collect();
        let expected: BTreeSet<_> = positions.iter().map(|&position| &atoms[position]).collect();
        let model = Model::from_positions(&catalog, positions).unwrap();
        for (actual, expected) in model.atoms().iter().zip(&expected) {
            assert_eq!(actual.compare(expected), Ordering::Equal);
        }
        assert_eq!(model.atoms().len(), expected.len());
        for atom in &atoms {
            assert_eq!(model.contains(atom), expected.contains(atom));
        }
    }
}

#[test]
fn model_equality_ignores_catalog_order() {
    let first = AtomCatalog::new(fixture()).unwrap();
    let mut reversed = fixture();
    reversed.reverse();
    let second = AtomCatalog::new(reversed).unwrap();
    for subset in 0..32_usize {
        let positions = (0..5).filter(|index| subset & (1 << index) != 0);
        let left = Model::from_positions(&first, positions.clone()).unwrap();
        let right = Model::from_positions(&second, positions.map(|p| 4 - p)).unwrap();
        assert!(!first.same_owner(&second));
        assert_eq!(left, right);
        assert_eq!(left.cmp(&right), Ordering::Equal);
    }
}

#[test]
fn model_order_matches_canonical_atom_sequences() {
    let atoms = fixture();
    let catalog = AtomCatalog::new(atoms.clone()).unwrap();
    let selections: Vec<Vec<usize>> = (0..32_usize)
        .map(|subset| (0..5).filter(|index| subset & (1 << index) != 0).collect())
        .collect();
    let models: Vec<_> = selections
        .iter()
        .map(|positions| Model::from_positions(&catalog, positions.iter().copied()).unwrap())
        .collect();
    let expected: Vec<Vec<&Atom>> = selections
        .iter()
        .map(|positions| {
            positions
                .iter()
                .map(|&position| &atoms[position])
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        })
        .collect();
    for (left_position, left) in models.iter().enumerate() {
        for (right_position, right) in models.iter().enumerate() {
            assert_eq!(
                left.cmp(right),
                expected[left_position].cmp(&expected[right_position])
            );
        }
    }
}

#[test]
fn duplicate_positions_denote_one_true_atom() {
    let catalog = AtomCatalog::new(fixture()).unwrap();
    let model = Model::from_positions(&catalog, [2, 2, 2]).unwrap();
    assert_eq!(model.atoms().len(), 1);
    assert_eq!(model.positions(), &[2]);
    assert_eq!(model.atoms().first(), catalog.atoms().at(2));
}

#[test]
fn invalid_positions_return_no_model() {
    let catalog = AtomCatalog::new(fixture()).unwrap();
    for invalid in [catalog.atoms().len(), usize::MAX] {
        assert_eq!(
            Model::from_positions(&catalog, [0, invalid]).unwrap_err(),
            ModelError::Position {
                position: invalid,
                atoms: catalog.atoms().len()
            }
        );
    }
    for (actual, expected) in catalog.atoms().iter().zip(fixture()) {
        assert_eq!(actual.compare(&expected), Ordering::Equal);
    }
}

#[test]
fn empty_catalog_admits_only_the_empty_selection() {
    let catalog = AtomCatalog::new(Vec::new()).unwrap();
    let model = Model::from_positions(&catalog, []).unwrap();
    assert!(model.atoms().is_empty());
    assert_eq!(model, Model::default());
    assert_eq!(
        Model::from_positions(&catalog, [0]).unwrap_err(),
        ModelError::Position {
            position: 0,
            atoms: 0
        }
    );
}

#[test]
fn model_clone_shares_selected_storage() {
    let model = Model::from_positions(&AtomCatalog::new(fixture()).unwrap(), [4, 2, 0]).unwrap();
    let clone = model.clone();
    assert!(clone.catalog().same_owner(model.catalog()));
    assert_eq!(clone.positions().as_ptr(), model.positions().as_ptr());
    for (left, right) in model.atoms().iter().zip(clone.atoms()) {
        assert_eq!(left, right);
    }
}

#[test]
fn selected_payload_survives_catalog_drop() {
    let model = {
        let catalog = AtomCatalog::new(vec![atom(
            Sign::Positive,
            Value::String("retained string".into()),
        )])
        .unwrap();
        Model::from_positions(&catalog, [0]).unwrap()
    };
    let expected = atom(Sign::Positive, Value::String("retained string".into()));
    assert!(model.contains(&expected));
    assert_eq!(
        model.atoms().first().unwrap().compare(&expected),
        Ordering::Equal
    );
}

#[test]
fn ordered_cursor_has_an_exact_remaining_length() {
    let model = Model::new(fixture()).unwrap();
    let all: Vec<_> = model.atoms().iter().collect();
    let mut cursor = model.atoms().iter();
    assert_eq!(cursor.len(), 5);
    assert_eq!(cursor.next(), Some(all[0]));
    assert_eq!(cursor.next_back(), Some(all[4]));
    assert_eq!(cursor.len(), 3);
    assert_eq!(cursor.by_ref().collect::<Vec<_>>(), all[1..4]);
    assert_eq!(cursor.next(), None);
    assert_eq!(cursor.next_back(), None);
    assert_eq!(cursor.size_hint(), (0, Some(0)));
}

#[test]
fn selected_model_retains_unselected_catalog_atoms() {
    let catalog = AtomCatalog::new(fixture()).unwrap();
    let model = Model::from_positions(&catalog, [0]).unwrap();
    assert_eq!(model.atoms().len(), 1);
    assert_eq!(model.catalog().atoms().len(), 5);
}

#[test]
fn retained_bytes_include_the_whole_catalog_record() {
    let atoms = vec![
        Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap(),
        Atom::new(
            Predicate::new("hidden", 1).unwrap(),
            vec![Value::String("secret".into())],
        )
        .unwrap(),
    ];
    let model = Model::from_positions(&AtomCatalog::new(atoms).unwrap(), [0]).unwrap();
    // Catalog length:8. Nullary a:8+1+8+1. Hidden string atom:
    // predicate length8+name6+arity8+sign1+tag1+text length8+text6.
    // Selected-position record: length8+one position8.
    assert_eq!(model.retained_payload_bytes(), Some(8 + 18 + 38 + 16));
}

#[test]
fn ordered_import_preserves_the_canonical_model() {
    let mut atoms = fixture();
    atoms.sort();
    atoms.dedup();
    let imported = Model::from_ordered(atoms.clone()).unwrap();
    assert_eq!(imported, Model::new(atoms.clone()).unwrap());
    assert_eq!(imported.atoms().len(), atoms.len());
    for (actual, expected) in imported.atoms().iter().zip(&atoms) {
        assert_eq!(actual.compare(expected), Ordering::Equal);
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "strictly increasing")]
fn importing_unordered_atoms_is_a_precondition_violation() {
    let mut atoms = fixture();
    atoms.sort();
    atoms.reverse();
    let _ = Model::from_ordered(atoms);
}

#[test]
fn a_predicate_sub_view_is_the_contiguous_range_of_its_atoms() {
    let model = Model::new(fixture()).unwrap();
    for sign in [Sign::Positive, Sign::Negative] {
        let predicate = Predicate::with_sign("p", 1, sign).unwrap();
        let range = model.atoms().of_predicate(&predicate);
        let expected: Vec<_> = model
            .atoms()
            .iter()
            .filter(|atom| atom.predicate().compare(&predicate).is_eq())
            .collect();
        assert_eq!(range.iter().collect::<Vec<_>>(), expected);
        assert_eq!(range.len(), expected.len());
    }
    let absent = Predicate::with_sign("q", 1, Sign::Positive).unwrap();
    assert!(model.atoms().of_predicate(&absent).is_empty());
}

#[test]
fn equal_local_positions_do_not_alias_foreign_atoms() {
    let left = AtomCatalog::new(vec![atom(Sign::Positive, Value::Number(1))]).unwrap();
    let right = AtomCatalog::new(vec![atom(Sign::Positive, Value::Number(2))]).unwrap();
    let first = Model::from_positions(&left, [0]).unwrap();
    let second = Model::from_positions(&right, [0]).unwrap();
    assert_eq!(first.positions(), second.positions());
    assert_ne!(first, second);
    assert_eq!(first.cmp(&second), Ordering::Less);
}

#[test]
fn later_catalog_publication_preserves_a_retained_model() {
    let old_atom = atom(Sign::Positive, Value::Number(1));
    let new_atom = atom(Sign::Positive, Value::Number(2));
    let mut owner = Catalog::new(usize::MAX);
    let original = owner
        .edit(Limits::default(), |builder| {
            let old = builder.intern_atom(&old_atom)?;
            builder.publish(&[old])
        })
        .unwrap();
    let model = Model::from_positions(&original, [0]).unwrap();
    let later = owner
        .edit(Limits::default(), |builder| {
            let old = builder.intern_atom(&old_atom)?;
            let new = builder.intern_atom(&new_atom)?;
            builder.publish(&[new, old, old])
        })
        .unwrap();
    assert!(original.shares_terms(&later));
    assert!(!original.same_owner(&later));
    drop(original);
    drop(owner);
    assert_eq!(model.catalog().atoms().len(), 1);
    assert!(model.contains(&old_atom));
    assert!(!model.contains(&new_atom));
    let equivalent = Model::from_positions(&later, [1, 2]).unwrap();
    assert_eq!(model, equivalent);
    assert_eq!(equivalent.atoms().len(), 1);
}

#[test]
fn canonical_lookup_preserves_original_dense_positions() {
    let atoms = fixture();
    let catalog = AtomCatalog::new(atoms.clone()).unwrap();
    let index = zetesis_core::AtomIndex::from_catalog_with(catalog.atoms(), || {
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    for (position, query) in atoms.iter().enumerate() {
        let found = index
            .lookup()
            .get_with(query, || Ok::<_, std::convert::Infallible>(()))
            .unwrap()
            .unwrap();
        assert_eq!(found.position(), position);
        assert_eq!(found.atom().compare(query), Ordering::Equal);
        let canonical_query = catalog.atoms().at(position).unwrap();
        let repeated = index
            .lookup()
            .get_with(canonical_query, || Ok::<_, std::convert::Infallible>(()))
            .unwrap()
            .unwrap();
        assert_eq!(repeated.position(), position);
        let pattern = zetesis_core::AtomPattern::new(
            query.predicate().clone(),
            query
                .values()
                .iter()
                .cloned()
                .map(zetesis_core::Term::Constant)
                .collect(),
        )
        .unwrap();
        let key = pattern.key(&[] as &[Value]).unwrap();
        let keyed = index
            .lookup()
            .get_key_with(&key, || Ok::<_, std::convert::Infallible>(()))
            .unwrap()
            .unwrap();
        assert_eq!(keyed.position(), position);
    }
}

#[test]
fn canonical_lookup_refusal_is_never_absence() {
    let atoms = fixture();
    let catalog = AtomCatalog::new(atoms.clone()).unwrap();
    let model = Model::from_positions(&catalog, 0..atoms.len()).unwrap();
    let query = &atoms[4];
    let mut required = 0;
    assert!(
        model
            .lookup()
            .get_with(query, || {
                required += 1;
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap()
            .is_some()
    );
    for limit in 0..required {
        let mut spent = 0;
        let mut calls = 0;
        let result = model.lookup().get_with(query, || {
            calls += 1;
            if spent == limit {
                return Err(spent);
            }
            spent += 1;
            Ok(())
        });
        assert!(matches!(result, Err(stopped) if stopped == limit));
        assert_eq!(spent, limit);
        assert_eq!(calls, limit + 1);
    }
    assert_eq!(model.atoms().len(), atoms.len());
    assert!(model.contains(query));
}

#[test]
fn canonical_index_rejects_duplicate_occurrences() {
    let mut atoms = fixture();
    atoms.push(atoms[0].clone());
    let catalog = AtomCatalog::new(atoms).unwrap();
    let refusal = zetesis_core::AtomIndex::from_catalog_with(catalog.atoms(), || {
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap_err();
    assert_eq!(
        refusal,
        zetesis_core::AtomIndexError::Duplicate {
            first: 0,
            second: 5
        }
    );
    assert_eq!(catalog.atoms().len(), 6);
}
