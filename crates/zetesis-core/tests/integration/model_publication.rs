//! Model publication receipts describe allocated selection metadata, not proposals.

use std::convert::Infallible;

use zetesis_core::{Atom, AtomCatalog, Model, ModelError, ModelFailure, Predicate, Value};

fn catalog(numbers: impl IntoIterator<Item = i32>) -> AtomCatalog {
    AtomCatalog::new(
        numbers
            .into_iter()
            .map(|number| {
                Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn publish(catalog: AtomCatalog) -> Model {
    Model::publish_ordered_catalog_with(catalog, usize::MAX, || Ok::<_, Infallible>(())).unwrap()
}

#[test]
fn rejected_selection_proposals_have_no_observed_peak() {
    let failure =
        Model::publish_ordered_catalog_with(catalog(0..130), 0, || -> Result<(), Infallible> {
            panic!("selection preflight precedes the caller permit")
        })
        .unwrap_err();
    assert!(matches!(
        failure.failure(),
        ModelFailure::Model(ModelError::Bytes { required, limit: 0 }) if *required > 0
    ));
    assert_eq!(failure.peak_bytes(), 0);
}

#[test]
fn initial_caller_refusal_has_no_observed_peak() {
    let failure =
        Model::publish_ordered_catalog_with(catalog(0..130), usize::MAX, || Err("initial"))
            .unwrap_err();
    assert_eq!(failure.into_parts(), (ModelFailure::Stopped("initial"), 0));
}

#[test]
fn empty_refusals_exclude_the_unallocated_header() {
    let empty = catalog([]);
    let mut permits = 0;
    let model = Model::publish_ordered_catalog_with(empty.clone(), usize::MAX, || {
        permits += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    assert_eq!(model.selection_capacity(), 0);
    assert!(permits > 0);
    // The empty route has reservation and publication permits, but its empty
    // position buffer never allocates. Even the final refusal has a zero peak.
    for stop_at in 0..permits {
        let mut calls = 0;
        let failure = Model::publish_ordered_catalog_with(empty.clone(), usize::MAX, || {
            let current = calls;
            calls += 1;
            if current == stop_at {
                Err(current)
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(failure.into_parts(), (ModelFailure::Stopped(stop_at), 0));
    }
}

#[test]
fn publication_obeys_the_actual_selection_allowance() {
    let catalog = catalog(0..130);
    let expected = publish(catalog.clone());
    let header = Model::default().selection_bytes();
    let limit = usize::try_from(header + 130 * size_of::<usize>() as u128).unwrap();
    match Model::publish_ordered_catalog_with(catalog, limit, || Ok::<_, Infallible>(())) {
        Ok(actual) => {
            assert_eq!(actual, expected);
            assert!(actual.selection_bytes() <= limit as u128);
        }
        Err(failure) => {
            let (cause, peak) = failure.into_parts();
            assert!(matches!(
                cause,
                ModelFailure::Model(ModelError::Bytes { required, limit: reported })
                    if reported == limit && required == header + peak && required > limit as u128
            ));
        }
    }
}

#[test]
fn empty_publication_fits_its_exact_header_allowance() {
    let header = usize::try_from(publish(catalog([])).selection_bytes()).unwrap();
    let actual =
        Model::publish_ordered_catalog_with(catalog([]), header, || Ok::<_, Infallible>(()))
            .unwrap();
    assert!(actual.atoms().is_empty());
    assert_eq!(actual.selection_bytes(), header as u128);
}
