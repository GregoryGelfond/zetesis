//! Refused publication receipts are checked against the same live allocation.

use std::convert::Infallible;

use super::{AtomCatalog, ModelError, ModelFailure, Selected, prepare_ordered_selection};
use crate::{Atom, Predicate, Value};

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

#[test]
fn caller_refusals_report_the_same_buffer_capacity() {
    let catalog = catalog(0..130);
    let mut complete = Vec::new();
    let mut permits = 0;
    prepare_ordered_selection(&catalog, &mut complete, usize::MAX, || {
        permits += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    assert!(permits > 1);

    // The complete run supplies only the callback population. Every receipt is
    // compared with its own still-live buffer, never with another reservation.
    for stop_at in 0..permits {
        let mut positions = Vec::new();
        let mut calls = 0;
        let failure = prepare_ordered_selection(&catalog, &mut positions, usize::MAX, || {
            let current = calls;
            calls += 1;
            if current == stop_at {
                Err(current)
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        let buffer_bytes = positions.capacity() as u128 * size_of::<usize>() as u128;
        assert_eq!(
            failure.into_parts(),
            (ModelFailure::Stopped(stop_at), buffer_bytes),
            "caller refusal at {stop_at}"
        );
        if stop_at == 0 {
            assert_eq!(buffer_bytes, 0);
        } else {
            assert!(positions.capacity() >= catalog.atoms().len());
        }
    }
}

#[test]
fn order_refusals_report_the_same_buffer_capacity() {
    for numbers in [[1, 1], [2, 1]] {
        let mut positions = Vec::new();
        let failure =
            prepare_ordered_selection(&catalog(numbers), &mut positions, usize::MAX, || {
                Ok::<_, Infallible>(())
            })
            .unwrap_err();
        assert_eq!(
            failure.into_parts(),
            (
                ModelFailure::Model(ModelError::Order { position: 1 }),
                positions.capacity() as u128 * size_of::<usize>() as u128
            )
        );
    }
}

#[test]
fn capacity_admission_uses_the_same_buffer_allocation() {
    let catalog = catalog(0..130);
    let mut positions = Vec::new();
    let header = size_of::<Selected>() as u128;
    let limit = size_of::<Selected>() + catalog.atoms().len() * size_of::<usize>();
    let result =
        prepare_ordered_selection(&catalog, &mut positions, limit, || Ok::<_, Infallible>(()));
    let buffer_bytes = positions.capacity() as u128 * size_of::<usize>() as u128;
    match result {
        Ok(()) => {
            assert_eq!(positions, (0..catalog.atoms().len()).collect::<Vec<_>>());
            assert!(header + buffer_bytes <= limit as u128);
        }
        Err(failure) => {
            // An allocator may expose capacity beyond the requested count.
            // Its actual refusal must report that same allocation, without a
            // final header which this preparation stage never allocated.
            assert_eq!(failure.peak_bytes(), buffer_bytes);
            assert!(matches!(
                failure.failure(),
                ModelFailure::Model(ModelError::Bytes { required, limit: reported })
                    if *reported == limit && *required == header + buffer_bytes
                        && *required > limit as u128
            ));
        }
    }
}
