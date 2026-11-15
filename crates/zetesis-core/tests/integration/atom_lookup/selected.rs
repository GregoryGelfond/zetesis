//! Sparse indexing retains original coordinates and the shared checked order.

use super::*;
use zetesis_core::SelectedAtomIndexErrorKind;

fn header_bytes() -> u128 {
    (size_of::<AtomIndex<'_>>() + size_of::<Vec<usize>>()) as u128
}

#[test]
fn selected_rows_retain_source_order_and_typed_identity() {
    let source = catalog();
    let atoms = AtomCatalog::new(source.clone()).unwrap();
    let selected: Vec<_> = (0..source.len())
        .filter(|position| position % 3 != 1)
        .collect();
    let index = AtomIndex::from_catalog_selection_with(atoms.atoms(), &selected, u128::MAX, || {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    for (position, query) in source.iter().enumerate() {
        let found = index
            .lookup()
            .get_with(query, || Ok::<_, Infallible>(()))
            .unwrap();
        assert_eq!(
            found.map(zetesis_core::AtomRow::position),
            selected.contains(&position).then_some(position)
        );
        if let Some(row) = found {
            assert_eq!(row.atom(), atoms.atoms().at(position).unwrap());
        }
        let expected: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&row| source[row].predicate() == query.predicate())
            .collect();
        let actual: Vec<_> = index
            .lookup()
            .predicate_with(query.predicate(), || Ok::<_, Infallible>(()))
            .unwrap()
            .map(zetesis_core::AtomRow::position)
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn selected_coordinates_can_exceed_the_selected_extent() {
    let source: Vec<_> = (0..=65_537)
        .map(|value| atom("p", Sign::Positive, vec![Value::Number(value)]))
        .collect();
    let atoms = AtomCatalog::new(source.clone()).unwrap();
    let selected = [1, 65_536, 65_537];
    let index = AtomIndex::from_catalog_selection_with(atoms.atoms(), &selected, u128::MAX, || {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    for position in selected {
        assert_eq!(
            index
                .lookup()
                .get_with(&source[position], || Ok::<_, Infallible>(()))
                .unwrap()
                .unwrap()
                .position(),
            position
        );
    }
    assert!(
        index
            .lookup()
            .get_with(&source[65_535], || Ok::<_, Infallible>(()))
            .unwrap()
            .is_none()
    );
    assert!(index.retained_bytes() < 1024);
}

#[test]
fn unselected_logical_duplicates_do_not_enter_the_index() {
    let duplicate = atom("p", Sign::Positive, vec![Value::Number(0)]);
    let selected = atom("q", Sign::Positive, vec![]);
    let atoms = AtomCatalog::new(vec![duplicate.clone(), selected.clone(), duplicate]).unwrap();
    let index = AtomIndex::from_catalog_selection_with(atoms.atoms(), &[1], u128::MAX, || {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert_eq!(
        index
            .lookup()
            .get_with(&selected, || Ok::<_, Infallible>(()))
            .unwrap()
            .unwrap()
            .position(),
        1
    );
    assert!(matches!(
        AtomIndex::from_catalog_with(atoms.atoms(), || Ok::<_, Infallible>(())),
        Err(AtomIndexError::Duplicate {
            first: 0,
            second: 2
        })
    ));
}

#[test]
fn selected_logical_duplicates_keep_original_positions() {
    let duplicate = atom("p", Sign::Positive, vec![]);
    let atoms = AtomCatalog::new(vec![
        atom("q", Sign::Positive, vec![]),
        duplicate.clone(),
        atom("r", Sign::Positive, vec![]),
        duplicate,
    ])
    .unwrap();
    let error = AtomIndex::from_catalog_selection_with(atoms.atoms(), &[1, 3], u128::MAX, || {
        Ok::<_, Infallible>(())
    })
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &SelectedAtomIndexErrorKind::Index(AtomIndexError::Duplicate {
            first: 1,
            second: 3
        })
    );
    assert!(error.preparation_peak_bytes() > header_bytes());
}

#[test]
fn repeated_and_reversed_positions_are_selection_errors() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    for positions in [[1, 1], [2, 1]] {
        let error =
            AtomIndex::from_catalog_selection_with(atoms.atoms(), &positions, u128::MAX, || {
                Ok::<_, Infallible>(())
            })
            .unwrap_err();
        assert_eq!(error.preparation_peak_bytes(), header_bytes());
        assert_eq!(
            error.into_kind(),
            SelectedAtomIndexErrorKind::Order {
                offset: 1,
                previous: positions[0],
                position: positions[1],
            }
        );
    }
}

#[test]
fn out_of_range_selection_refuses_before_catalog_access() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let length = atoms.atoms().len();
    for position in [length, usize::MAX] {
        let error = AtomIndex::from_catalog_selection_with(
            atoms.atoms(),
            &[0, position],
            u128::MAX,
            || Ok::<_, Infallible>(()),
        )
        .unwrap_err();
        assert_eq!(error.preparation_peak_bytes(), header_bytes());
        assert_eq!(
            error.into_kind(),
            SelectedAtomIndexErrorKind::Position {
                offset: 1,
                position,
                atoms: length,
            }
        );
    }
}

#[test]
fn cancellation_precedes_selection_and_byte_diagnostics() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    for positions in [&[][..], &[usize::MAX][..], &[1, 1][..]] {
        let mut calls = 0;
        let error = AtomIndex::from_catalog_selection_with(atoms.atoms(), positions, 0, || {
            calls += 1;
            Err("cancelled")
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(
            error.kind(),
            &SelectedAtomIndexErrorKind::Index(AtomIndexError::Stopped("cancelled"))
        );
        assert_eq!(error.preparation_peak_bytes(), header_bytes());
    }
}

#[test]
fn empty_selection_admits_only_its_index_headers() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let index = AtomIndex::from_catalog_selection_with(atoms.atoms(), &[], header_bytes(), || {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert_eq!(index.retained_bytes(), size_of::<AtomIndex<'_>>() as u128);
    assert_eq!(index.preparation_peak_bytes(), header_bytes());
    assert!(
        index
            .lookup()
            .get_with(&catalog()[0], || Err::<(), _>("must not read"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn byte_refusal_excludes_unallocated_requested_capacity() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    for (positions, required) in [
        (&[][..], header_bytes()),
        (
            &[0, 1, 2][..],
            header_bytes() + 3 * size_of::<usize>() as u128,
        ),
    ] {
        let limit = required - 1;
        let error = AtomIndex::from_catalog_selection_with(atoms.atoms(), positions, limit, || {
            Ok::<_, Infallible>(())
        })
        .unwrap_err();
        assert_eq!(error.preparation_peak_bytes(), header_bytes());
        assert_eq!(
            error.into_kind(),
            SelectedAtomIndexErrorKind::Bytes { required, limit }
        );
    }
}

#[test]
fn later_byte_refusal_retains_observed_allocations() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let selected = [0, 2, 5, 7, 9];
    let limit = header_bytes() + (selected.len() * size_of::<usize>()) as u128;
    let error = AtomIndex::from_catalog_selection_with(atoms.atoms(), &selected, limit, || {
        Ok::<_, Infallible>(())
    })
    .unwrap_err();
    let SelectedAtomIndexErrorKind::Bytes {
        required,
        limit: actual_limit,
    } = *error.kind()
    else {
        panic!("expected a byte refusal after the first allocation");
    };
    assert_eq!(actual_limit, limit);
    assert!(required > limit);
    assert!(error.preparation_peak_bytes() > header_bytes());
    assert!(error.preparation_peak_bytes() <= required);
}

#[test]
fn each_selected_preparation_step_can_refuse_exactly() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let selected = [0, 2, 5, 7, 9];
    let mut total = 0;
    let reference =
        AtomIndex::from_catalog_selection_with(atoms.atoms(), &selected, u128::MAX, || {
            total += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    let mut previous_peak = 0;
    for limit in 0..total {
        let mut spent = 0;
        let error =
            AtomIndex::from_catalog_selection_with(atoms.atoms(), &selected, u128::MAX, || {
                if spent == limit {
                    return Err(limit);
                }
                spent += 1;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(spent, limit);
        assert_eq!(
            error.kind(),
            &SelectedAtomIndexErrorKind::Index(AtomIndexError::Stopped(limit))
        );
        let peak = error.preparation_peak_bytes();
        assert!(peak >= previous_peak);
        assert!(peak <= reference.preparation_peak_bytes());
        previous_peak = peak;
    }
    assert_eq!(previous_peak, reference.preparation_peak_bytes());
    let exact = AtomIndex::from_catalog_selection_with(
        atoms.atoms(),
        &selected,
        reference.preparation_peak_bytes(),
        || Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert_eq!(exact.retained_bytes(), reference.retained_bytes());
}

#[test]
fn selected_stops_preserve_the_original_error_cause() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let error = AtomIndex::from_catalog_selection_with(atoms.atoms(), &[], u128::MAX, || {
        Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "selected stop",
        ))
    })
    .unwrap_err();
    let index_error = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<AtomIndexError<std::io::Error>>()
        .unwrap();
    let cause = std::error::Error::source(index_error)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(error.to_string(), "selected stop");
}

#[test]
fn selection_refusals_render_distinct_diagnostics() {
    for (kind, expected) in [
        (
            SelectedAtomIndexErrorKind::<Infallible>::Position {
                offset: 2,
                position: 10,
                atoms: 9,
            },
            "selected atom at offset 2 has position 10 outside 9 occurrences",
        ),
        (
            SelectedAtomIndexErrorKind::Order {
                offset: 2,
                previous: 7,
                position: 7,
            },
            "selected atom at offset 2 has position 7 after 7",
        ),
        (
            SelectedAtomIndexErrorKind::Bytes {
                required: 100,
                limit: 99,
            },
            "selected atom index requires 100 bytes, exceeding 99",
        ),
    ] {
        assert_eq!(kind.to_string(), expected);
        assert!(std::error::Error::source(&kind).is_none());
    }
}

#[test]
fn dense_empty_index_keeps_three_reservation_callbacks() {
    let atoms = AtomCatalog::new(vec![]).unwrap();
    let mut calls = 0;
    AtomIndex::from_catalog_with(atoms.atoms(), || {
        calls += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert_eq!(calls, 3);
}

proptest::proptest! {
    #[test]
    fn selected_lookup_agrees_with_a_source_filter(
        membership in proptest::collection::vec(proptest::bool::ANY, 0..48)
    ) {
        let source: Vec<_> = (0i32..).zip(&membership)
            .map(|(value, _)| atom("p", Sign::Positive, vec![Value::Number(value)]))
            .collect();
        let atoms = AtomCatalog::new(source.clone()).unwrap();
        let selected: Vec<_> = membership.iter().enumerate()
            .filter_map(|(position, &included)| included.then_some(position))
            .collect();
        let index = AtomIndex::from_catalog_selection_with(
            atoms.atoms(), &selected, u128::MAX, || Ok::<_, Infallible>(())
        ).unwrap();
        for (position, query) in source.iter().enumerate() {
            let found = index.lookup().get_with(query, || Ok::<_, Infallible>(())).unwrap();
            proptest::prop_assert_eq!(
                found.map(zetesis_core::AtomRow::position),
                membership[position].then_some(position)
            );
        }
    }
}
