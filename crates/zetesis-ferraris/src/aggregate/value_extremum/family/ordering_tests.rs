//! The checked ordered route retains the same complete carrier as sorting.

use super::*;
use crate::{AggregateLimits, FormulaParts, Node};
use zetesis_core::{Atom, AtomCatalog, Predicate};

fn prefix() -> FormulaNodes {
    FormulaNodes::new(FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap())
}

fn append(
    nodes: &mut FormulaNodes,
    values: &[Value],
    bound: &Value,
    max_work: u64,
) -> ValueExtremumFamilyAttempt {
    let elements: Vec<_> = values
        .iter()
        .map(|value| ValueExtremumElement {
            value: value.into(),
            condition: 0,
        })
        .collect();
    nodes.append_value_extremum_family_refs(
        &elements,
        AggregateExtremum::Min,
        &[ValueExtremumGuard {
            comparison: AggregateComparison::Eq,
            bound: bound.into(),
        }],
        AggregateFamilyLimits {
            aggregate: AggregateLimits {
                max_work,
                ..AggregateLimits::default()
            },
            ..AggregateFamilyLimits::default()
        },
        &Cancellation::default(),
    )
}

#[test]
fn ordered_ties_reuse_the_complete_carrier() {
    let values = [
        Value::Infimum,
        Value::Infimum,
        Value::Number(-2),
        Value::Number(-2),
        Value::Number(0),
        Value::Symbol("a".into()),
        Value::String("a".into()),
        Value::Supremum,
        Value::Supremum,
    ];
    for (bound, expected) in [
        (Value::Infimum, [1, 0]),
        (Value::Number(-1), [2, 2]),
        (Value::Number(0), [3, 2]),
        (Value::Symbol("a".into()), [4, 3]),
        (Value::Supremum, [6, 5]),
    ] {
        let mut nodes = prefix();
        append(&mut nodes, &values, &bound, u64::MAX)
            .result
            .unwrap();
        assert_eq!(nodes.parts().numeric_values(), [0, 0, 1, 1, 2, 3, 4, 5, 5]);
        assert_eq!(nodes.parts().conditions(), [0; 9]);
        let NodeView::ExtremumCut(cut) = nodes.view().node(1).unwrap() else {
            panic!("native row");
        };
        assert_eq!(cut.row.rank_count, 6);
        let rules: Vec<_> = nodes
            .parts()
            .extremum_cuts()
            .iter()
            .map(|cut| cut.rule)
            .collect();
        assert_eq!(rules, expected.map(ExtremumCutRule::Prefix));
    }
}

#[test]
fn inversions_discard_incomplete_ordered_ranks() {
    for values in [
        [5, 1, 2, 3, 4],
        [1, 2, 3, 5, 4],
        [5, 4, 3, 2, 1],
        [1, 3, 3, 2, 5],
    ] {
        let values = values.map(Value::Number);
        let mut nodes = prefix();
        append(&mut nodes, &values, &Value::Number(3), u64::MAX)
            .result
            .unwrap();
        let expected: Vec<_> = values
            .iter()
            .map(|value| {
                1 + u64::try_from(
                    values
                        .iter()
                        .filter(|other| *other < value)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(nodes.parts().numeric_values(), expected);
    }
}

#[test]
fn ordered_input_avoids_repeated_sorting_work() {
    let values: Vec<_> = (0..64).map(Value::Number).collect();
    let bound = Value::Number(32);
    let mut ordered = prefix();
    let ordered_attempt = append(&mut ordered, &values, &bound, u64::MAX);
    let ordered_work = ordered_attempt.result.unwrap().statistics().work;
    let mut reversed = values;
    reversed.reverse();
    let mut sorted = prefix();
    let sorted_attempt = append(&mut sorted, &reversed, &bound, u64::MAX);
    let sorted_work = sorted_attempt.result.unwrap().statistics().work;
    assert!(ordered_work < sorted_work);
    assert_eq!(
        ordered_attempt.auxiliary_peak_bytes,
        sorted_attempt.auxiliary_peak_bytes
    );
    let mut reversed_ranks = sorted.parts().numeric_values().to_vec();
    reversed_ranks.reverse();
    assert_eq!(ordered.parts().numeric_values(), reversed_ranks);
    assert_eq!(
        ordered.parts().extremum_cuts(),
        sorted.parts().extremum_cuts()
    );
}

#[test]
fn ordered_refusal_rolls_back_each_work_prefix() {
    let values = [Value::Number(1), Value::Number(1), Value::Number(3)];
    let bound = Value::Number(2);
    let complete = append(&mut prefix(), &values, &bound, u64::MAX)
        .result
        .unwrap();
    for max_work in 0..complete.statistics().work {
        let mut nodes = prefix();
        let error = append(&mut nodes, &values, &bound, max_work)
            .result
            .unwrap_err();
        assert_eq!(error.kind(), AggregateErrorKind::WorkLimit);
        assert_eq!(error.statistics().work, max_work);
        assert_eq!(nodes.view().len(), 1);
        assert!(nodes.parts().rows().is_empty());
        assert!(nodes.parts().conditions().is_empty());
        assert!(nodes.parts().numeric_values().is_empty());
        assert!(nodes.parts().extremum_cuts().is_empty());
    }
}

#[test]
fn bounds_from_another_authority_use_typed_order() {
    let catalog = |values: Vec<Value>| {
        let predicate = Predicate::new("value", 1).unwrap();
        AtomCatalog::new(
            values
                .into_iter()
                .map(|value| Atom::new(predicate.clone(), vec![value]).unwrap())
                .collect(),
        )
        .unwrap()
    };
    let values = catalog(vec![Value::Symbol("a".into()), Value::String("a".into())]);
    let bounds = catalog(vec![Value::String("a".into()), Value::Symbol("a".into())]);
    let elements: Vec<_> = (0..2)
        .map(|index| ValueExtremumElement {
            value: values.atoms().at(index).unwrap().values().at(0).unwrap(),
            condition: 0,
        })
        .collect();
    let guards: Vec<_> = (0..2)
        .map(|index| ValueExtremumGuard {
            comparison: AggregateComparison::Eq,
            bound: bounds.atoms().at(index).unwrap().values().at(0).unwrap(),
        })
        .collect();
    let mut nodes = prefix();
    nodes
        .append_value_extremum_family_refs(
            &elements,
            AggregateExtremum::Min,
            &guards,
            AggregateFamilyLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    let rules: Vec<_> = nodes
        .parts()
        .extremum_cuts()
        .iter()
        .map(|cut| cut.rule)
        .collect();
    assert_eq!(rules, [3, 2, 2, 1].map(ExtremumCutRule::Prefix));
    assert_eq!(nodes.parts().numeric_values(), [1, 2]);
}
