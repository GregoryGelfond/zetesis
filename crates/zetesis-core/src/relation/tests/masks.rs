use super::{Atom, Failure, Limits, Relation, Resource, Value, atoms, limit, predicate};

#[test]
fn bit_positions_reconstruct_original_row_order() {
    let signature = predicate(0);
    for count in [0_usize, 1, 31, 32, 33, 63, 64, 65, 129] {
        let source = vec![Atom::new(signature.clone(), Vec::new()).unwrap(); count];
        let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
        let mut words = vec![0_u32; count.div_ceil(32)];
        let expected: Vec<_> = (0..count).filter(|row| row % 3 == 0).collect();
        for &row in &expected {
            words[row / 32] |= 1 << (row % 32);
        }
        let selected = relation
            .selection_from_mask(&words, Limits::default())
            .unwrap();
        assert_eq!(selected.positions(), expected);
        assert!(relation.same_owner(selected.relation()));
    }
}

#[test]
fn wrong_mask_word_counts_are_refused() {
    let signature = predicate(0);
    let source = vec![Atom::new(signature.clone(), Vec::new()).unwrap(); 65];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    for words in [&[0_u32, 0][..], &[0_u32, 0, 0, 0][..]] {
        assert!(matches!(
            relation.selection_from_mask(words, Limits::default()),
            Err(Failure::Mask)
        ));
    }
}

#[test]
fn nonzero_unused_tail_bits_are_refused() {
    let signature = predicate(0);
    let source = vec![Atom::new(signature.clone(), Vec::new()).unwrap(); 33];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    assert!(matches!(
        relation.selection_from_mask(&[0, 2], Limits::default()),
        Err(Failure::Mask)
    ));
    assert_eq!(
        relation
            .selection_from_mask(&[0, 1], Limits::default())
            .unwrap()
            .positions(),
        &[32]
    );
}

#[test]
fn an_empty_relation_requires_an_empty_mask() {
    let signature = predicate(0);
    let relation = Relation::from_atoms(&signature, &[], Limits::default()).unwrap();
    assert!(
        relation
            .selection_from_mask(&[], Limits::default())
            .unwrap()
            .positions()
            .is_empty()
    );
    assert!(matches!(
        relation.selection_from_mask(&[0], Limits::default()),
        Err(Failure::Mask)
    ));
}

#[test]
fn mask_bytes_are_included_once_in_reconstruction() {
    let signature = predicate(0);
    let source = vec![Atom::new(signature.clone(), Vec::new()).unwrap(); 32];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let words = [1_u32];
    let selected = relation
        .selection_from_mask(&words, Limits::default())
        .unwrap();
    let exact = relation.storage().retained_bytes
        + std::mem::size_of_val(&words)
        + selected.retained_bytes();
    drop(selected);
    assert_eq!(
        relation
            .selection_from_mask(
                &words,
                Limits {
                    max_bytes: exact,
                    ..Limits::default()
                }
            )
            .unwrap()
            .positions(),
        &[0]
    );
    limit(
        &relation.selection_from_mask(
            &words,
            Limits {
                max_bytes: exact - 1,
                ..Limits::default()
            },
        ),
        Resource::Bytes,
    );
}

#[test]
fn zero_masks_still_charge_both_word_scans() {
    let signature = predicate(0);
    let source = vec![Atom::new(signature.clone(), Vec::new()).unwrap(); 65];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let words = [0; 3];
    limit(
        &relation.selection_from_mask(
            &words,
            Limits {
                max_work: 5,
                ..Limits::default()
            },
        ),
        Resource::Work,
    );
    let selected = relation
        .selection_from_mask(
            &words,
            Limits {
                max_work: 6,
                ..Limits::default()
            },
        )
        .unwrap();
    assert!(selected.positions().is_empty());
    assert_eq!(selected.work(), 6);
}

#[test]
fn interrupted_decoding_does_not_publish_a_prefix() {
    let signature = predicate(0);
    let source = vec![Atom::new(signature.clone(), Vec::new()).unwrap(); 2];
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    let words = [3];
    limit(
        &relation.selection_from_mask(
            &words,
            Limits {
                max_work: 5,
                ..Limits::default()
            },
        ),
        Resource::Work,
    );
    assert_eq!(words, [3]);
    let selected = relation
        .selection_from_mask(
            &words,
            Limits {
                max_work: 6,
                ..Limits::default()
            },
        )
        .unwrap();
    assert_eq!(selected.positions(), &[0, 1]);
    assert_eq!(selected.work(), 6);
}

#[test]
fn mask_reconstruction_preserves_catalog_occurrences() {
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
    let selected = relation
        .selection_from_mask(&[5], Limits::default())
        .unwrap();
    assert_eq!(selected.positions(), &[0, 2]);
    assert_eq!(selected.row(0).unwrap().source_index(), 2);
    assert_eq!(selected.row(1).unwrap().source_index(), 2);
}
