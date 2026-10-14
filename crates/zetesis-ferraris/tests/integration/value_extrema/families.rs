//! A shared guard family has the complete failing-subset meaning of each guard.

use super::{COMPARISONS, EXTREMA, Element, Extremum, ordered, reference};
use crate::support::aggregate_theories::{prefix, push};
use crate::support::worlds::eval;
use zetesis_core::{Atom, AtomCatalog, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AggregateFamilyLimits, NodeView, ValueExtremumGuard, append_value_extremum_family_refs,
};

fn verify(elements: &[Element], extremum: Extremum, bounds: &[Value]) {
    let predicate = Predicate::new("value", 1).unwrap();
    let catalog = AtomCatalog::new(
        elements
            .iter()
            .map(|element| &element.value)
            .chain(bounds)
            .map(|value| Atom::new(predicate.clone(), vec![value.clone()]).unwrap())
            .collect(),
    )
    .unwrap();
    let value = |slot| catalog.atoms().at(slot).unwrap().values().at(0).unwrap();
    let borrowed: Vec<_> = elements
        .iter()
        .enumerate()
        .map(|(index, element)| Element {
            value: value(index),
            condition: element.condition,
        })
        .collect();
    // Reverse bound order and repeat boundaries across comparisons. Neither the
    // guard position nor the catalog's insertion order denotes ASP term order.
    let guards: Vec<_> = bounds
        .iter()
        .enumerate()
        .rev()
        .flat_map(|(index, _)| {
            COMPARISONS.map(|comparison| ValueExtremumGuard {
                comparison,
                bound: value(elements.len() + index),
            })
        })
        .collect();
    let mut nodes = prefix();
    let built = append_value_extremum_family_refs(
        &mut nodes,
        &borrowed,
        extremum,
        &guards,
        AggregateFamilyLimits::default(),
        &Cancellation::default(),
    )
    .result
    .unwrap();
    assert_eq!(built.roots().len(), guards.len());
    assert_eq!(
        nodes.parts().rows().len(),
        1,
        "the ordinary native family shares one row"
    );
    assert_eq!(nodes.parts().conditions().len(), elements.len());
    assert_eq!(nodes.parts().extremum_cuts().len(), 2 * guards.len());
    assert!(
        (0..nodes.view().len())
            .any(|index| matches!(nodes.view().node(index).unwrap(), NodeView::ExtremumCut(_)))
    );
    for ((bound, comparison), &root) in bounds
        .iter()
        .rev()
        .flat_map(|bound| COMPARISONS.map(|comparison| (bound, comparison)))
        .zip(built.roots())
    {
        let (mut specified, mut expected) = reference(elements, extremum, comparison, bound);
        let mut actual = root;
        for _ in 0..3 {
            for outer in 0..4 {
                assert_eq!(
                    eval(nodes.view(), actual, outer, None),
                    eval(specified.view(), expected, outer, None),
                    "original: {extremum:?} {comparison:?} {bound:?} M={outer}"
                );
                for inner in 0..4 {
                    assert_eq!(
                        eval(nodes.view(), actual, inner, Some(outer)),
                        eval(specified.view(), expected, inner, Some(outer)),
                        "frozen: {extremum:?} {comparison:?} {bound:?} M={outer} J={inner}"
                    );
                }
            }
            actual = push(&mut nodes, NodeView::Implies(actual, 0));
            expected = push(&mut specified, NodeView::Implies(expected, 0));
        }
    }
}

#[test]
fn shared_guards_preserve_original_and_frozen_meaning() {
    let values = ordered();
    for (index, value) in values.iter().enumerate() {
        for condition in [2, 4, 5, 9, 10] {
            let elements = [
                Element {
                    value: value.clone(),
                    condition: 2,
                },
                Element {
                    value: values[(index + 4) % values.len()].clone(),
                    condition,
                },
                // Equal values with distinct conditions still contribute their
                // own eligibility. Equal conditions at unequal values also occur.
                Element {
                    value: value.clone(),
                    condition: 3,
                },
            ];
            for extremum in EXTREMA {
                verify(&elements, extremum, &values);
            }
        }
    }
}

#[test]
fn empty_families_preserve_ordered_endpoints() {
    for extremum in EXTREMA {
        verify(&[], extremum, &ordered());
    }
}

#[test]
fn absent_bounds_preserve_their_ordered_insertion_cuts() {
    let values = ordered();
    let elements: Vec<_> = values
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 4)
        .map(|(index, value)| Element {
            value: value.clone(),
            condition: 2 + index % 2,
        })
        .collect();
    for extremum in EXTREMA {
        verify(&elements, extremum, &values[4..5]);
    }
}
