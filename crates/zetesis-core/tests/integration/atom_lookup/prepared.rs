use super::*;
use zetesis_core::{AtomKey, AtomLookup, PredicateLookup};

fn key(atom: &Atom) -> AtomPattern {
    AtomPattern::new(
        atom.predicate().clone(),
        (0..atom.values().len()).map(Term::Variable).collect(),
    )
    .unwrap()
}

#[test]
fn predicate_windows_preserve_typed_membership() {
    let source = catalog();
    let atoms = AtomCatalog::new(source.clone()).unwrap();
    // A selection deliberately has gaps in the authoritative catalog.
    let model = Model::from_positions(&atoms, (0..source.len()).filter(|i| i % 3 != 1)).unwrap();
    let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    for lookup in [model.lookup(), index.lookup()] {
        for query in &source {
            let window = lookup
                .prepare_predicate_with(query.predicate(), || Ok::<_, Infallible>(()))
                .unwrap();
            for candidate in &source {
                let pattern = key(candidate);
                let bound = pattern.key(candidate.values()).unwrap();
                let actual = window
                    .get_key_with(&bound, || Ok::<_, Infallible>(()))
                    .unwrap();
                let expected = if candidate.predicate() == query.predicate() {
                    lookup
                        .get_key_with(&bound, || Ok::<_, Infallible>(()))
                        .unwrap()
                } else {
                    None
                };
                assert_eq!(
                    actual.map(zetesis_core::AtomRow::position),
                    expected.map(zetesis_core::AtomRow::position)
                );
                if let Some(row) = actual {
                    assert_eq!(row.atom(), atoms.atoms().at(row.position()).unwrap());
                }
            }
        }
    }
}

#[test]
fn predicate_windows_preserve_original_row_order() {
    let source = catalog();
    let atoms = AtomCatalog::new(source.clone()).unwrap();
    // A selection deliberately has gaps in the authoritative catalog.
    let model = Model::from_positions(&atoms, (0..source.len()).filter(|i| i % 3 != 1)).unwrap();
    let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    for lookup in [model.lookup(), index.lookup()] {
        for query in &source {
            let window = lookup
                .prepare_predicate_with(query.predicate(), || Ok::<_, Infallible>(()))
                .unwrap();
            assert_eq!(
                window
                    .rows()
                    .map(zetesis_core::AtomRow::position)
                    .collect::<Vec<_>>(),
                lookup
                    .predicate_with(query.predicate(), || Ok::<_, Infallible>(()))
                    .unwrap()
                    .map(zetesis_core::AtomRow::position)
                    .collect::<Vec<_>>(),
            );
        }
    }
}

#[test]
fn independent_predicate_owners_keep_equal_content() {
    let source = catalog();
    let atoms = AtomCatalog::new(source.clone()).unwrap();
    let foreign = AtomCatalog::new(source.clone()).unwrap();
    let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    for (position, query) in source.iter().enumerate() {
        let predicate = foreign.atoms().at(position).unwrap().predicate();
        let view = index
            .lookup()
            .prepare_predicate_with(predicate, || Ok::<_, Infallible>(()))
            .unwrap();
        let pattern = key(query);
        let bound = pattern.key(query.values()).unwrap();
        assert_eq!(
            view.get_key_with(&bound, || Ok::<_, Infallible>(()))
                .unwrap()
                .unwrap()
                .position(),
            position
        );
    }
}

#[test]
fn empty_predicate_window_has_no_tuple_or_row() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    let query = atom("absent", Sign::Negative, vec![Value::Number(9)]);
    let view = index
        .lookup()
        .prepare_predicate_with(query.predicate(), || Ok::<_, Infallible>(()))
        .unwrap();
    assert_eq!(view.rows().len(), 0);
    let pattern = key(&query);
    let bound = pattern.key(query.values()).unwrap();
    assert!(
        view.get_key_with(&bound, || Err::<(), _>("must not probe"))
            .unwrap()
            .is_none()
    );
}

fn check_refusals(view: PredicateLookup<'_, '_, '_>, bound: &AtomKey<'_>) {
    let mut work = 0;
    let expected = view
        .get_key_with(bound, || {
            work += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap()
        .map(zetesis_core::AtomRow::position);
    assert!(work > 0);
    for ceiling in 0..work {
        let mut spent = 0;
        let result = view.get_key_with(bound, || {
            if spent == ceiling {
                return Err(ceiling);
            }
            spent += 1;
            Ok(())
        });
        assert_eq!(result.unwrap_err(), ceiling);
        assert_eq!(spent, ceiling);
        assert_eq!(
            view.get_key_with(bound, || Ok::<_, Infallible>(()))
                .unwrap()
                .map(zetesis_core::AtomRow::position),
            expected
        );
    }
}

#[test]
fn refused_predicate_preparation_publishes_no_window() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    let predicate = Predicate::new("p", 1).unwrap();
    let mut work = 0;
    index
        .lookup()
        .prepare_predicate_with(&predicate, || {
            work += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    for ceiling in 0..work {
        let mut spent = 0;
        let result = index.lookup().prepare_predicate_with(&predicate, || {
            if spent == ceiling {
                return Err(ceiling);
            }
            spent += 1;
            Ok(())
        });
        assert_eq!(result.unwrap_err(), ceiling);
        assert_eq!(spent, ceiling);
    }
    assert!(
        index
            .lookup()
            .prepare_predicate_with(&predicate, || Ok::<_, Infallible>(()))
            .unwrap()
            .rows()
            .len()
            > 0
    );
}

#[test]
fn refused_prepared_key_never_becomes_absence() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    let predicate = Predicate::new("p", 1).unwrap();
    let view = index
        .lookup()
        .prepare_predicate_with(&predicate, || Ok::<_, Infallible>(()))
        .unwrap();
    // Includes present typed values, absent tuples and wrong predicate/arity.
    for query in catalog().into_iter().chain([
        atom("p", Sign::Positive, vec![Value::Number(42)]),
        atom("foreign", Sign::Positive, vec![Value::Number(0)]),
    ]) {
        let pattern = key(&query);
        check_refusals(view, &pattern.key(query.values()).unwrap());
    }
}

fn lookup_work(lookup: AtomLookup<'_, '_>, source: &[Atom], prepared: bool) -> usize {
    let mut work = 0;
    for query in source {
        let view = lookup
            .prepare_predicate_with(query.predicate(), || Ok::<_, Infallible>(()))
            .unwrap();
        let pattern = key(query);
        let bound = pattern.key(query.values()).unwrap();
        let mut before = || {
            work += 1;
            Ok::<_, Infallible>(())
        };
        let found = if prepared {
            view.get_key_with(&bound, &mut before)
        } else {
            lookup.get_key_with(&bound, &mut before)
        }
        .unwrap();
        assert!(found.is_some());
    }
    work
}

#[test]
fn prepared_keys_avoid_repeated_predicate_comparisons() {
    let source = catalog();
    let atoms = AtomCatalog::new(source.clone()).unwrap();
    let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    assert!(
        lookup_work(index.lookup(), &source, true) < lookup_work(index.lookup(), &source, false)
    );
}

proptest::proptest! {
    #[test]
    fn predicate_windows_match_full_lookup(values in proptest::collection::vec(-32i32..32, 0..32)) {
        let mut source = Vec::new();
        for value in values {
            let atom = atom("p", Sign::Positive, vec![Value::Number(value)]);
            if !source.contains(&atom) { source.push(atom); }
        }
        source.push(atom("p", Sign::Negative, vec![Value::Number(0)]));
        source.push(atom("p", Sign::Positive, vec![]));
        let atoms = AtomCatalog::new(source).unwrap();
        let index = CatalogIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
        let predicate = Predicate::new("p", 1).unwrap();
        let view = index.lookup().prepare_predicate_with(&predicate, || Ok::<_, Infallible>(())).unwrap();
        for value in -33..33 {
            let query = atom("p", Sign::Positive, vec![Value::Number(value)]);
            let pattern = key(&query);
            let bound = pattern.key(query.values()).unwrap();
            proptest::prop_assert_eq!(
                view.get_key_with(&bound, || Ok::<_, Infallible>(())).unwrap().map(zetesis_core::AtomRow::position),
                index.lookup().get_key_with(&bound, || Ok::<_, Infallible>(())).unwrap().map(zetesis_core::AtomRow::position),
            );
        }
    }
}
