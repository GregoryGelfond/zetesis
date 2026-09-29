use super::*;

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());

fn prepared() -> (AtomInterner, AtomCatalog) {
    let mut source = owner(&[3]);
    for value in [1, 2] {
        source
            .store
            .import_value(&Value::Number(value), TermLimits::default())
            .unwrap();
    }
    let prior = source
        .publish_selection_with(&[0], limits(), PERMIT)
        .unwrap();
    (source, prior)
}

#[test]
fn closed_base_starts_without_discovery() {
    let (source, prior) = prepared();
    let base = source.into_closed_with(limits(), PERMIT).unwrap();
    let descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    assert!(descendant.is_empty());
    assert_eq!(
        descendant
            .find_atom_with(
                prior.atoms().at(0).unwrap(),
                Limits {
                    max_atoms: 0,
                    ..limits()
                },
                PERMIT
            )
            .unwrap(),
        None
    );
    assert!(descendant.committed().is_empty());
}

#[test]
fn discovery_can_select_an_existing_base_row() {
    let (mut source, prior) = prepared();
    source
        .store
        .import_atom(&atom(1), TermLimits::default())
        .unwrap();
    let base = source.into_closed_with(limits(), PERMIT).unwrap();
    let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let count = descendant.store.snapshot(0).unwrap().atom_count();
    let first = insert(&mut descendant, &atom(1));
    assert_eq!(first, 0);
    assert_eq!(descendant.store.snapshot(0).unwrap().atom_count(), count);
    let second = descendant
        .entry_atom_with(prior.atoms().at(0).unwrap(), limits(), PERMIT)
        .unwrap()
        .insert_with(limits(), PERMIT)
        .unwrap();
    assert_eq!(second, 1);
    assert_eq!(descendant.get(0), Some(AtomRef::from(&atom(1))));
}

#[test]
fn closed_descendant_publishes_typed_model_order() {
    let (source, prior) = prepared();
    let base = source.into_closed_with(limits(), PERMIT).unwrap();
    let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    for value in [2, 3, 1] {
        insert(&mut descendant, &atom(value));
    }
    let catalog = descendant
        .into_ordered_catalog_with(limits(), PERMIT)
        .unwrap();
    let model = crate::Model::from_ordered_catalog_with(catalog, usize::MAX, PERMIT).unwrap();
    let expected: Vec<_> = [1, 2, 3].into_iter().map(atom).collect();
    assert!(model.atoms().iter().eq(expected.iter().map(AtomRef::from)));
    assert_eq!(prior.atoms().at(0), Some(AtomRef::from(&atom(3))));
}

#[test]
fn close_receipt_authenticates_prior_publications() {
    let (source, prior) = prepared();
    let expected = prior.publication_bytes() + prior.0.snapshot.metadata_bytes();
    let base = source.into_closed_with(limits(), PERMIT).unwrap();
    assert_eq!(base.prior_publication_metadata_bytes(&prior), Ok(expected));
    assert_eq!(
        base.prior_publication_metadata_bytes(&prior.clone()),
        Ok(expected)
    );
    let foreign = AtomCatalog::new(vec![atom(3)]).unwrap();
    assert_eq!(
        base.prior_publication_metadata_bytes(&foreign),
        Err(crate::catalog::ReadError::ForeignCatalog)
    );
}

#[test]
fn new_writer_charges_shared_base_once() {
    let (source, _) = prepared();
    let base = source.into_closed_with(limits(), PERMIT).unwrap();
    let descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    assert_eq!(
        descendant.shared_closed_catalog_bytes(),
        base.storage_bytes() - size_of::<ClosedCatalog>() as u128
    );
    let required = descendant.storage_bytes();
    assert!(AtomInterner::for_closed_catalog(&base, usize::try_from(required).unwrap()).is_ok());
    assert!(matches!(
        AtomInterner::for_closed_catalog(&base, usize::try_from(required - 1).unwrap()),
        Err(crate::catalog::Error::Storage { .. })
    ));
}

#[test]
fn immediate_close_stop_preserves_cause() {
    let (source, _) = prepared();
    let expected = source.storage_bytes();
    let failure = source
        .into_closed_with(limits(), || Err::<(), _>("stop"))
        .unwrap_err();
    assert_eq!(failure.peak_bytes(), expected);
    assert!(matches!(failure.into_parts(), (Failure::Stopped("stop"), peak) if peak == expected));
}

#[test]
fn close_population_still_counts_discovery() {
    let (mut source, _) = prepared();
    source
        .store
        .import_atom(&atom(1), TermLimits::default())
        .unwrap();
    let base = source
        .into_closed_with(
            Limits {
                max_atoms: 1,
                ..limits()
            },
            PERMIT,
        )
        .unwrap();
    let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    descendant
        .entry_atom_with(&atom(1), limits(), PERMIT)
        .unwrap()
        .insert_with(
            Limits {
                max_atoms: 1,
                ..limits()
            },
            PERMIT,
        )
        .unwrap();
    assert!(matches!(
        descendant
            .entry_atom_with(&atom(3), limits(), PERMIT)
            .unwrap()
            .insert_with(
                Limits {
                    max_atoms: 1,
                    ..limits()
                },
                PERMIT
            ),
        Err(Failure::Atoms {
            required: 2,
            limit: 1
        })
    ));
}

#[test]
fn refused_descendant_admission_can_retry() {
    let (source, prior) = prepared();
    let base = source.into_closed_with(limits(), PERMIT).unwrap();
    let mut complete = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let mut count = 0;
    let input = atom(1);
    let mut permit = || {
        count += 1;
        Ok::<_, usize>(())
    };
    complete
        .entry_atom_with(&input, limits(), &mut permit)
        .unwrap()
        .insert_with(limits(), &mut permit)
        .unwrap();
    for cut in 0..count {
        let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
        let mut calls = 0;
        let mut permit = || {
            let current = calls;
            calls += 1;
            if current == cut { Err(cut) } else { Ok(()) }
        };
        let result = descendant
            .entry_atom_with(&input, limits(), &mut permit)
            .and_then(|entry| entry.insert_with(limits(), &mut permit));
        assert!(matches!(result, Err(Failure::Stopped(value)) if value == cut));
        assert!(descendant.is_empty());
        assert_eq!(
            descendant.find_atom_with(&input, limits(), PERMIT).unwrap(),
            None
        );
        assert_eq!(insert(&mut descendant, &input), 0);
        assert_eq!(prior.atoms().at(0), Some(AtomRef::from(&atom(3))));
    }
}

#[test]
fn close_panic_preserves_old_snapshot() {
    let (complete, _) = prepared();
    let mut count = 0;
    complete
        .into_closed_with(limits(), || {
            count += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    for cut in 0..count {
        let (source, prior) = prepared();
        let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source.into_closed_with(limits(), || {
                assert_ne!(calls, cut, "caller unwind");
                calls += 1;
                Ok::<_, Infallible>(())
            })
        }));
        assert!(result.is_err());
        assert_eq!(prior.atoms().at(0), Some(AtomRef::from(&atom(3))));
    }
}
