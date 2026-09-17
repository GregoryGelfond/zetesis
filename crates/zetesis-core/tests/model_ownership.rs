//! Shared interpretation ownership preserves logical identity across catalogs.

use std::{cmp::Ordering, collections::BTreeSet, ptr};

use zetesis_core::{
    Atom, AtomCatalog, Model, ModelError, Predicate, Sign, Value, ValueLimits, ValueNode,
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
fn catalog_transfer_preserves_atom_addresses() {
    let atoms = fixture();
    let first = atoms.as_ptr();
    let capacity = atoms.capacity();
    let catalog = AtomCatalog::new(atoms);
    assert_eq!(catalog.atoms().as_ptr(), first);
    assert_eq!(catalog.capacity(), capacity);
}

#[test]
fn selection_matches_the_independent_logical_set() {
    let mut atoms = fixture();
    atoms.push(atoms[0].clone());
    atoms.reverse();
    let catalog = AtomCatalog::new(atoms);
    for subset in 0..(1_usize << catalog.atoms().len()) {
        let positions: Vec<_> = (0..catalog.atoms().len())
            .filter(|index| subset & (1 << index) != 0)
            .rev()
            .collect();
        let expected: BTreeSet<_> = positions.iter().map(|&p| &catalog.atoms()[p]).collect();
        let model = Model::from_positions(&catalog, positions).unwrap();
        assert_eq!(
            model.atoms().iter().collect::<Vec<_>>(),
            expected.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(model.atoms().len(), expected.len());
        for atom in catalog.atoms() {
            assert_eq!(model.contains(atom), expected.contains(atom));
        }
    }
}

#[test]
fn model_equality_ignores_catalog_order() {
    let first = AtomCatalog::new(fixture());
    let mut reversed = fixture();
    reversed.reverse();
    let second = AtomCatalog::new(reversed);
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
    let catalog = AtomCatalog::new(fixture());
    let models: Vec<_> = (0..32_usize)
        .map(|subset| {
            Model::from_positions(&catalog, (0..5).filter(|i| subset & (1 << i) != 0)).unwrap()
        })
        .collect();
    for left in &models {
        for right in &models {
            let expected = left
                .atoms()
                .iter()
                .collect::<Vec<_>>()
                .cmp(&right.atoms().iter().collect());
            assert_eq!(left.cmp(right), expected);
        }
    }
}

#[test]
fn duplicate_positions_denote_one_true_atom() {
    let catalog = AtomCatalog::new(fixture());
    let model = Model::from_positions(&catalog, [2, 2, 2]).unwrap();
    assert_eq!(model.atoms().len(), 1);
    assert!(ptr::eq(
        model.atoms().first().unwrap(),
        &raw const catalog.atoms()[2]
    ));
}

#[test]
fn invalid_positions_return_no_model() {
    let catalog = AtomCatalog::new(fixture());
    for invalid in [catalog.atoms().len(), usize::MAX] {
        assert_eq!(
            Model::from_positions(&catalog, [0, invalid]).unwrap_err(),
            ModelError::Position {
                position: invalid,
                atoms: catalog.atoms().len()
            }
        );
    }
    assert_eq!(catalog.atoms(), fixture());
}

#[test]
fn empty_catalog_admits_only_the_empty_selection() {
    let catalog = AtomCatalog::new(Vec::new());
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
    let model = Model::from_positions(&AtomCatalog::new(fixture()), [4, 2, 0]).unwrap();
    let clone = model.clone();
    assert!(clone.catalog().same_owner(model.catalog()));
    assert_eq!(clone.positions().as_ptr(), model.positions().as_ptr());
    for (left, right) in model.atoms().iter().zip(clone.atoms()) {
        assert!(ptr::eq(left, right));
    }
}

#[test]
fn selected_payload_survives_catalog_drop() {
    let (model, original) = {
        let catalog = AtomCatalog::new(vec![atom(
            Sign::Positive,
            Value::String("retained string".into()),
        )]);
        let Value::String(text) = &catalog.atoms()[0].values()[0] else {
            unreachable!()
        };
        let original = text.as_ptr();
        (Model::from_positions(&catalog, [0]).unwrap(), original)
    };
    let Value::String(text) = &model.atoms().first().unwrap().values()[0] else {
        unreachable!()
    };
    assert_eq!(text, "retained string");
    assert_eq!(text.as_ptr(), original);
}

#[test]
fn ordered_cursor_has_an_exact_remaining_length() {
    let model = Model::new(fixture());
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
    let catalog = AtomCatalog::new(fixture());
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
    let model = Model::from_positions(&AtomCatalog::new(atoms), [0]).unwrap();
    // Catalog length:8. Nullary a:8+1+8+1. Hidden string atom:
    // predicate length8+name6+arity8+sign1+tag1+text length8+text6.
    // Selected-position record: length8+one position8.
    assert_eq!(model.retained_payload_bytes(), Some(8 + 18 + 38 + 16));
}

#[test]
fn ordered_atoms_are_adopted_as_the_same_model_without_sorting() {
    let mut atoms = fixture();
    atoms.sort();
    atoms.dedup();
    let adopted = Model::from_ordered(atoms.clone());
    assert_eq!(adopted, Model::new(atoms.clone()));
    assert_eq!(adopted.atoms().iter().cloned().collect::<Vec<_>>(), atoms);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "strictly increasing")]
fn adopting_unordered_atoms_is_a_precondition_violation() {
    let mut atoms = fixture();
    atoms.sort();
    atoms.reverse();
    let _ = Model::from_ordered(atoms);
}

#[test]
fn a_predicate_sub_view_is_the_contiguous_range_of_its_atoms() {
    let model = Model::new(fixture());
    for sign in [Sign::Positive, Sign::Negative] {
        let predicate = Predicate::with_sign("p", 1, sign).unwrap();
        let range = model.atoms().of_predicate(&predicate);
        let expected: Vec<_> = model
            .atoms()
            .iter()
            .filter(|atom| atom.predicate() == &predicate)
            .collect();
        assert_eq!(range.iter().collect::<Vec<_>>(), expected);
        assert_eq!(range.len(), expected.len());
    }
    let absent = Predicate::with_sign("q", 1, Sign::Positive).unwrap();
    assert!(model.atoms().of_predicate(&absent).is_empty());
}
