//! Prepared occurrence order preserves logical sets without another atom owner.

use std::{cmp::Ordering, collections::BTreeSet, convert::Infallible};

use proptest::prelude::*;
use zetesis_core::{
    Atom, AtomCatalog, Model, ModelError, ModelFailure, ModelOrder, Predicate, Sign, Value,
    ValueLimits, ValueNode,
};

fn atom(kind: u8, number: i16, negative: bool) -> Atom {
    let value = match kind % 5 {
        0 => Value::Number(i32::from(number)),
        1 => Value::String(number.to_string()),
        2 => Value::Symbol(number.to_string()),
        3 => Value::from_nodes(
            vec![
                ValueNode::Tuple { arity: 1 },
                ValueNode::Number(i32::from(number)),
            ],
            ValueLimits::default(),
        )
        .unwrap(),
        _ => Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    sign: Sign::Negative,
                    arity: 1,
                },
                ValueNode::Tuple { arity: 1 },
                ValueNode::Number(i32::from(number)),
            ],
            ValueLimits::default(),
        )
        .unwrap(),
    };
    let sign = if negative {
        Sign::Negative
    } else {
        Sign::Positive
    };
    Atom::new(Predicate::with_sign("p", 1, sign).unwrap(), vec![value]).unwrap()
}

fn catalog() -> AtomCatalog {
    AtomCatalog::new(vec![
        atom(4, 7, true),
        atom(0, 9, false),
        atom(1, -3, false),
        atom(4, 7, true),
        atom(2, -3, false),
        atom(0, 2, false),
    ])
    .unwrap()
}

fn prepare(catalog: &AtomCatalog) -> ModelOrder<'_> {
    ModelOrder::prepare_with(catalog, usize::MAX, || Ok::<_, Infallible>(())).unwrap()
}

fn select(order: &ModelOrder<'_>, positions: impl IntoIterator<Item = usize>) -> Model {
    order
        .select_with(positions, usize::MAX, || Ok::<_, Infallible>(()))
        .unwrap()
        .into_model()
}

proptest! {
    #[test]
    fn prepared_selection_matches_an_independent_logical_set(
        descriptions in prop::collection::vec((0_u8..5, -8_i16..8, any::<bool>()), 1..32),
        supplied in prop::collection::vec(any::<usize>(), 0..64),
    ) {
        let atoms: Vec<_> = descriptions.into_iter()
            .map(|(kind, number, negative)| atom(kind, number, negative)).collect();
        let positions: Vec<_> = supplied.into_iter().map(|p| p % atoms.len()).collect();
        let expected: BTreeSet<_> = positions.iter().map(|&p| &atoms[p]).collect();
        let catalog = AtomCatalog::new(atoms.clone()).unwrap();
        let order = prepare(&catalog);
        let model = select(&order, positions.iter().copied());
        prop_assert!(model.catalog().same_owner(&catalog));
        prop_assert_eq!(model.atoms().len(), expected.len());
        for (actual, expected) in model.atoms().iter().zip(&expected) {
            prop_assert_eq!(actual.compare(expected), Ordering::Equal);
        }
        for &position in model.positions() {
            prop_assert!(positions.contains(&position));
            let least = positions.iter().copied()
                .filter(|&p| atoms[p] == atoms[position]).min().unwrap();
            prop_assert_eq!(position, least);
        }
        let decoded: Vec<_> = model.atoms().iter().collect();
        prop_assert!(decoded.windows(2).all(|pair| pair[0] < pair[1]));
        for atom in &atoms {
            prop_assert_eq!(model.contains(atom), expected.contains(atom));
        }
    }
}

#[test]
fn equivalent_occurrences_keep_a_selected_representative() {
    let catalog = catalog();
    let order = prepare(&catalog);
    // Occurrences 0 and 3 are equal, but 0 was not supplied.
    let model = select(&order, [3, 3]);
    assert_eq!(model.positions(), [3]);
    assert_eq!(model.atoms().iter().next(), catalog.atoms().at(0));
    assert_eq!(select(&order, [3, 0, 3]).positions(), [0]);
}

#[test]
fn ingress_equivalent_values_share_one_selected_rank() {
    let symbol = Value::Symbol("f".into());
    let function = Value::from_nodes(
        vec![ValueNode::Function {
            name: "f".into(),
            sign: Sign::Positive,
            arity: 0,
        }],
        ValueLimits::default(),
    )
    .unwrap();
    let predicate = Predicate::new("p", 1).unwrap();
    let catalog = AtomCatalog::new(vec![
        Atom::new(predicate.clone(), vec![symbol]).unwrap(),
        Atom::new(predicate, vec![function]).unwrap(),
    ])
    .unwrap();
    let order = prepare(&catalog);
    assert_eq!(select(&order, [1, 0]).positions(), [0]);
    assert_eq!(select(&order, [1]).positions(), [1]);
}

#[test]
fn prepared_models_use_storage_order_across_foreign_owners() {
    let values = vec![atom(2, 1, false), atom(1, 1, false), atom(0, 1, false)];
    let first = AtomCatalog::new(values.clone()).unwrap();
    let second = AtomCatalog::new(values.into_iter().rev().collect()).unwrap();
    let left = select(&prepare(&first), [0, 1, 2]);
    let right = select(&prepare(&second), [0, 1, 2]);
    assert!(!first.same_owner(&second));
    assert_eq!(left.positions(), [2, 1, 0]);
    assert_eq!(right.positions(), [0, 1, 2]);
    assert_eq!(left, right);
    assert_eq!(left.cmp(&right), Ordering::Equal);
}

#[test]
fn a_model_outlives_its_order_and_input_borrow() {
    let (model, owner) = {
        let catalog = catalog();
        let order = prepare(&catalog);
        (select(&order, [5, 3, 2]), catalog.clone())
    };
    assert!(model.catalog().same_owner(&owner));
    assert_eq!(model, Model::from_positions(&owner, [5, 3, 2]).unwrap());
}

#[test]
fn empty_catalogs_publish_only_the_empty_interpretation() {
    let catalog = AtomCatalog::default();
    let order = prepare(&catalog);
    assert!(select(&order, []).atoms().is_empty());
    let error = order
        .select_with([0], usize::MAX, || Ok::<_, Infallible>(()))
        .unwrap_err();
    assert_eq!(
        error.failure(),
        &ModelFailure::Model(ModelError::Position {
            position: 0,
            atoms: 0
        })
    );
}

#[test]
fn an_invalid_position_preserves_prior_models_and_order() {
    let catalog = catalog();
    let order = prepare(&catalog);
    let prior = select(&order, [1, 4]);
    let expected = prior.clone();
    let error = order
        .select_with([5, usize::MAX, 1], usize::MAX, || Ok::<_, Infallible>(()))
        .unwrap_err();
    assert_eq!(
        error.failure(),
        &ModelFailure::Model(ModelError::Position {
            position: usize::MAX,
            atoms: catalog.atoms().len(),
        })
    );
    assert_eq!(prior, expected);
    assert_eq!(select(&order, [1, 4]), expected);
}

#[test]
fn every_preparation_permit_can_refuse_without_mutation() {
    let catalog = catalog();
    let expected = Model::from_positions(&catalog, 0..catalog.atoms().len()).unwrap();
    let mut permits = 0;
    let prepared = ModelOrder::prepare_with(&catalog, usize::MAX, || {
        permits += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    assert!(permits > 10 && permits < 2000);
    let mut observed = BTreeSet::new();
    for stop_at in 0..permits {
        let mut calls = 0;
        let error = ModelOrder::prepare_with(&catalog, usize::MAX, || {
            let current = calls;
            calls += 1;
            if current == stop_at {
                Err(current)
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(calls, stop_at + 1);
        assert_eq!(error.failure(), &ModelFailure::Stopped(stop_at));
        observed.insert(error.peak_bytes());
        assert_eq!(
            Model::from_positions(&catalog, 0..catalog.atoms().len()).unwrap(),
            expected
        );
    }
    assert!(observed.len() >= 3, "receipts include both reservations");
    assert_eq!(
        observed.last().copied(),
        Some(prepared.preparation_peak_bytes())
    );
    assert_eq!(
        select(&prepare(&catalog), 0..catalog.atoms().len()),
        expected
    );
}

#[test]
fn every_selection_permit_can_refuse_and_then_retry() {
    let catalog = catalog();
    let order = prepare(&catalog);
    let positions = [5, 0, 3, 2, 4, 1, 5, 3, 1];
    let mut permits = 0;
    let publication = order
        .select_with(positions, usize::MAX, || {
            permits += 1;
            Ok::<_, usize>(())
        })
        .unwrap();
    assert!(permits > 10 && permits < 2000);
    let expected = publication.model().clone();
    let mut observed = BTreeSet::new();
    for stop_at in 0..permits {
        let mut calls = 0;
        let error = order
            .select_with(positions, usize::MAX, || {
                let current = calls;
                calls += 1;
                if current == stop_at {
                    Err(current)
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(calls, stop_at + 1);
        assert_eq!(error.failure(), &ModelFailure::Stopped(stop_at));
        observed.insert(error.peak_bytes());
        assert_eq!(select(&order, positions), expected);
    }
    assert!(observed.len() >= 3, "receipts include growth and scratch");
    let (model, peak) = publication.into_parts();
    assert_eq!(observed.last().copied(), Some(peak));
    assert!(peak >= order.retained_bytes() + model.selection_bytes());
}

struct CannotConvert;

impl IntoIterator for CannotConvert {
    type Item = usize;
    type IntoIter = std::iter::Empty<usize>;

    fn into_iter(self) -> Self::IntoIter {
        panic!("initial refusal must precede iterator conversion")
    }
}

#[test]
fn initial_cancellation_precedes_iterator_conversion() {
    let catalog = catalog();
    let order = prepare(&catalog);
    let failure = order
        .select_with(CannotConvert, usize::MAX, || Err("cancelled"))
        .unwrap_err();
    assert_eq!(failure.failure(), &ModelFailure::Stopped("cancelled"));
    assert_eq!(
        failure.peak_bytes(),
        order.retained_bytes() + 2 * size_of::<Vec<usize>>() as u128
    );
}

#[test]
fn initial_preparation_refusal_has_only_named_headers() {
    let catalog = catalog();
    let failure = ModelOrder::prepare_with(&catalog, usize::MAX, || Err("cancelled")).unwrap_err();
    assert_eq!(failure.failure(), &ModelFailure::Stopped("cancelled"));
    assert_eq!(
        failure.peak_bytes(),
        (size_of::<ModelOrder<'_>>() + size_of::<Vec<usize>>()) as u128
    );
}

#[test]
fn a_lower_allowance_refuses_before_iterator_conversion() {
    let catalog = catalog();
    let order = prepare(&catalog);
    let error = order
        .select_with(CannotConvert, 0, || Ok::<_, Infallible>(()))
        .unwrap_err();
    assert!(
        matches!(error.failure(), ModelFailure::Model(ModelError::Bytes { required, limit: 0 })
        if *required >= order.retained_bytes())
    );
}

#[test]
fn a_finite_work_policy_stops_an_unbounded_iterator() {
    let catalog = catalog();
    let order = prepare(&catalog);
    let mut work = 0;
    let failure = order
        .select_with(std::iter::repeat(0), usize::MAX, || {
            if work == 100 {
                return Err(work);
            }
            work += 1;
            Ok(())
        })
        .unwrap_err();
    assert_eq!(failure.failure(), &ModelFailure::Stopped(100));
    assert_eq!(work, 100);
}

#[test]
fn rejected_preparation_proposals_are_not_observed_capacity() {
    let catalog = catalog();
    let error = ModelOrder::prepare_with(&catalog, 0, || Ok::<_, Infallible>(())).unwrap_err();
    let base = error.peak_bytes();
    let limit = usize::try_from(base).unwrap();
    let error = ModelOrder::prepare_with(&catalog, limit, || Ok::<_, Infallible>(())).unwrap_err();
    assert!(
        matches!(error.failure(), ModelFailure::Model(ModelError::Bytes { required, limit: actual })
        if *required > base && *actual == limit)
    );
    assert_eq!(error.peak_bytes(), base);
}

#[test]
fn rejected_selection_proposals_are_not_observed_capacity() {
    let catalog = catalog();
    let order = prepare(&catalog);
    let error = order
        .select_with([0], 0, || Ok::<_, Infallible>(()))
        .unwrap_err();
    let base = error.peak_bytes();
    let limit = usize::try_from(base).unwrap();
    let error = order
        .select_with([0], limit, || Ok::<_, Infallible>(()))
        .unwrap_err();
    assert!(
        matches!(error.failure(), ModelFailure::Model(ModelError::Bytes { required, limit: actual })
        if *required > base && *actual == limit)
    );
    assert_eq!(error.peak_bytes(), base);
}

#[test]
fn every_insufficient_preparation_allowance_reports_bytes() {
    let catalog = catalog();
    let prepared = prepare(&catalog);
    let peak = usize::try_from(prepared.preparation_peak_bytes()).unwrap();
    assert!(peak < 4096);
    for limit in 0..peak {
        let error =
            ModelOrder::prepare_with(&catalog, limit, || Ok::<_, Infallible>(())).unwrap_err();
        assert!(
            matches!(error.failure(), ModelFailure::Model(ModelError::Bytes { required, limit: actual })
            if *required > limit as u128 && *actual == limit)
        );
    }
    let exact = ModelOrder::prepare_with(&catalog, peak, || Ok::<_, Infallible>(())).unwrap();
    assert!(exact.preparation_peak_bytes() <= peak as u128);
    assert!(exact.retained_bytes() < exact.preparation_peak_bytes());
}

#[test]
fn every_insufficient_selection_allowance_reports_bytes() {
    let catalog = catalog();
    let order = prepare(&catalog);
    let publication = order
        .select_with([5, 3, 2, 4, 1], usize::MAX, || Ok::<_, Infallible>(()))
        .unwrap();
    let peak = usize::try_from(publication.peak_bytes()).unwrap();
    assert!(peak < 4096);
    for limit in 0..peak {
        let error = order
            .select_with([5, 3, 2, 4, 1], limit, || Ok::<_, Infallible>(()))
            .unwrap_err();
        assert!(
            matches!(error.failure(), ModelFailure::Model(ModelError::Bytes { required, limit: actual })
            if *required > limit as u128 && *actual == limit)
        );
    }
    let exact = order
        .select_with([5, 3, 2, 4, 1], peak, || Ok::<_, Infallible>(()))
        .unwrap();
    assert!(exact.peak_bytes() <= peak as u128);
    assert_eq!(exact.model(), publication.model());
}
