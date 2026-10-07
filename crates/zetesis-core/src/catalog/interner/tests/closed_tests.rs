use super::*;
use crate::test_support::PERMIT;

fn selected_base() -> (ClosedCatalog, crate::Model) {
    // Canonical IDs deliberately disagree with semantic argument and predicate
    // order. The last occurrence stays false, and another row arrives after
    // the selected catalog's immutable prefix was published.
    let mut source = owner(&[9, 1, 8, 2, 7, 3, 6, 4, 5, 0]);
    let nested = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 2 },
            ValueNode::String("x".into()),
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            ValueNode::Number(2),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    for sign in [Sign::Positive, Sign::Negative] {
        for (name, arguments) in [
            ("z", vec![]),
            ("p", vec![]),
            ("p", vec![nested.clone()]),
            ("a", vec![Value::Symbol("x".into())]),
            ("a", vec![Value::String("x".into())]),
        ] {
            insert(
                &mut source,
                &Atom::new(
                    Predicate::with_sign(name, arguments.len(), sign).unwrap(),
                    arguments,
                )
                .unwrap(),
            );
        }
    }
    insert(&mut source, &atom(99));
    let count = source.len();
    let catalog = source
        .publish_selection_with(&(0..count).collect::<Vec<_>>(), limits(), PERMIT)
        .unwrap();
    let model = crate::Model::from_positions(&catalog, (0..count - 1).rev().chain([3, 3])).unwrap();
    insert(&mut source, &atom(77));
    (source.into_closed_with(limits(), PERMIT).unwrap(), model)
}

#[test]
fn selected_discovery_preserves_both_indexes() {
    let (base, model) = selected_base();
    let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let count = descendant.store.snapshot(0).unwrap().atom_count();
    descendant
        .discover_model_with(&base, &model, limits(), PERMIT)
        .unwrap();
    assert_eq!(descendant.len(), model.atoms().len());
    assert!(descendant.committed().is_empty());
    assert!(
        (0..descendant.len())
            .map(|at| descendant.get(at).unwrap())
            .eq(model.atoms().iter())
    );
    assert_eq!(descendant.store.snapshot(0).unwrap().atom_count(), count);
    assert_eq!(
        descendant
            .find_atom_with(&atom(99), limits(), PERMIT)
            .unwrap(),
        None
    );
    validate(&descendant);
    // A new ordinary entry must still find its place in both seeded indexes.
    assert_eq!(insert(&mut descendant, &atom(99)), model.atoms().len());
    validate(&descendant);
    let catalog = descendant
        .into_ordered_catalog_with(limits(), PERMIT)
        .unwrap();
    let actual = crate::Model::from_ordered_catalog_with(catalog, usize::MAX, PERMIT).unwrap();
    let mut expected = model.atoms().iter().collect::<Vec<_>>();
    let added = atom(99);
    expected.push(AtomRef::from(&added));
    expected.sort_unstable();
    assert!(actual.atoms().iter().eq(expected));
}

#[test]
fn selected_discovery_avoids_semantic_reinsertion() {
    let (base, model) = selected_base();
    let mut seeded = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let mut selected_work = 0;
    seeded
        .discover_model_with(&base, &model, limits(), || {
            selected_work += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    let mut ordinary = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let mut ordinary_work = 0;
    let mut before = || {
        ordinary_work += 1;
        Ok::<_, Infallible>(())
    };
    for atom in model.atoms() {
        ordinary
            .entry_atom_with(atom, limits(), &mut before)
            .unwrap()
            .insert_with(limits(), &mut before)
            .unwrap();
    }
    assert!(selected_work < ordinary_work);
    assert_eq!(seeded.pending, ordinary.pending);
}

#[test]
fn selected_discovery_refuses_unrelated_owners() {
    let (base, model) = selected_base();
    let (other_base, other_model) = selected_base();
    let mut sibling = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    sibling
        .discover_model_with(&base, &model, limits(), PERMIT)
        .unwrap();
    let sibling = crate::Model::from_ordered_catalog_with(
        sibling.into_ordered_catalog_with(limits(), PERMIT).unwrap(),
        usize::MAX,
        PERMIT,
    )
    .unwrap();
    // Equal catalogs, a mismatched supplied base and an actual sibling all fail.
    for (supplied_base, supplied_model) in [
        (&base, &other_model),
        (&other_base, &model),
        (&base, &sibling),
    ] {
        let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
        assert!(matches!(
            descendant.discover_model_with(supplied_base, supplied_model, limits(), PERMIT),
            Err(Failure::Catalog(crate::catalog::Error::Shape))
        ));
        assert!(descendant.is_empty());
        descendant
            .discover_model_with(&base, &model, limits(), PERMIT)
            .unwrap();
        validate(&descendant);
    }
}

#[test]
fn selected_discovery_refuses_nonempty_discovery() {
    let (base, model) = selected_base();
    let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    insert(&mut descendant, &atom(77));
    assert!(matches!(
        descendant.discover_model_with(&base, &model, limits(), PERMIT),
        Err(Failure::Catalog(crate::catalog::Error::Shape))
    ));
    assert_eq!(descendant.len(), 1);
    assert_eq!(descendant.get(0), Some(AtomRef::from(&atom(77))));
    validate(&descendant);
}

#[test]
fn selected_discovery_stops_at_complete_prefixes() {
    let (base, model) = selected_base();
    let mut complete = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let mut total = 0;
    complete
        .discover_model_with(&base, &model, limits(), || {
            total += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    for cutoff in 0..total {
        let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
        let mut accepted = 0;
        let result = descendant.discover_model_with(&base, &model, limits(), || {
            if accepted == cutoff {
                Err(cutoff)
            } else {
                accepted += 1;
                Ok(())
            }
        });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cutoff));
        assert_eq!(accepted, cutoff);
        assert!(
            (0..descendant.len())
                .map(|at| descendant.get(at).unwrap())
                .eq(model.atoms().iter().take(descendant.len()))
        );
        validate(&descendant);
        for atom in model.atoms() {
            descendant
                .entry_atom_with(atom, limits(), PERMIT)
                .unwrap()
                .insert_with(limits(), PERMIT)
                .unwrap();
        }
        assert_eq!(descendant.len(), model.atoms().len());
        validate(&descendant);
    }
}

#[test]
fn selected_discovery_obeys_population_limits() {
    let (base, model) = selected_base();
    for maximum in 0..model.atoms().len() {
        let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
        assert!(matches!(
            descendant.discover_model_with(&base, &model, Limits {
                max_atoms: maximum,
                ..limits()
            }, PERMIT),
            Err(Failure::Atoms { required, limit })
                if required == maximum + 1 && limit == maximum
        ));
        assert_eq!(descendant.len(), maximum);
        validate(&descendant);
    }
}

#[test]
fn selected_discovery_obeys_storage_limits() {
    let (base, model) = selected_base();
    let mut complete = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
    let initial = complete.storage_bytes();
    complete
        .discover_model_with(&base, &model, limits(), PERMIT)
        .unwrap();
    let peak = complete.storage_peak_bytes();
    assert!(peak > initial);
    for maximum in [initial - 1, initial, peak - 1, peak] {
        let mut descendant = AtomInterner::for_closed_catalog(&base, usize::MAX).unwrap();
        let result = descendant.discover_model_with(
            &base,
            &model,
            Limits {
                max_bytes: maximum,
                ..limits()
            },
            PERMIT,
        );
        if maximum < peak {
            assert!(matches!(result, Err(Failure::Bytes { .. })));
        } else {
            result.unwrap();
        }
        assert!(descendant.storage_peak_bytes() >= descendant.storage_bytes());
        assert!(descendant.storage_peak_bytes() <= maximum.max(initial));
        validate(&descendant);
    }
}

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
