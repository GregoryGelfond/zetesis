use crate::ProgramSite;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};

use super::*;
use crate::formula_support::testing::Fixture;
use zetesis_core::{Value, ValueNodeRef};

fn with_context(values: &[Value], run: impl FnOnce(&[Term], &mut Context<'_, '_, '_>)) {
    let limits = FormulaLimits::default();
    let location = ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    });
    let mut fixture = Fixture::default();
    let terms: Vec<_> = fixture.admit(location, |source, counters| {
        values
            .iter()
            .map(|value| {
                Term::Constant(
                    source
                        .scalar(value.into(), &limits, counters, location)
                        .unwrap(),
                )
            })
            .collect()
    });
    fixture.with(location, |_, computation, counters| {
        run(
            &terms,
            &mut Context {
                computation,
                limits: &limits,
                counters,
                location,
                entries: 0,
            },
        );
    });
}

/// The objective carrier of `required` and `optional` tuples, whose possible
/// tuples are both.
fn carrier_values<'ir>(
    function: AggregateFunction,
    required: &[&'ir [Term]],
    optional: &[&'ir [Term]],
    context: &mut Context<'_, '_, '_>,
) -> TermSelection {
    let mut required_tuples = Tuples::new(context).unwrap();
    let mut possible = Tuples::new(context).unwrap();
    for &tuple in required {
        required_tuples.insert(tuple, context).unwrap();
        possible.insert(tuple, context).unwrap();
    }
    for &tuple in optional {
        possible.insert(tuple, context).unwrap();
    }
    complete(function, &required_tuples, &possible, context).unwrap()
}

#[test]
fn numeric_carriers_include_every_optional_subset_sum() {
    let values = [Value::Number(2), Value::Number(-1), Value::Number(3)];
    with_context(&values, |terms, context| {
        let selected = carrier_values(
            AggregateFunction::Sum,
            &[&[terms[2]]],
            &[&[terms[0]], &[terms[1]]],
            context,
        );
        let ordered = selected
            .ordered(
                false,
                context.computation,
                context.limits,
                context.counters,
                context.location,
            )
            .unwrap();
        let values: Vec<_> = (0..ordered.len())
            .map(|slot| {
                ordered
                    .read(slot, context.computation.read(), context.location)
                    .unwrap()
                    .descriptor()
            })
            .collect();
        assert_eq!(
            values,
            vec![
                ValueNodeRef::Number(2),
                ValueNodeRef::Number(3),
                ValueNodeRef::Number(4),
                ValueNodeRef::Number(5)
            ]
        );
    });
}

fn assert_values(
    selected: &TermSelection,
    included: &[Value],
    excluded: &[Value],
    context: &mut Context<'_, '_, '_>,
) {
    assert_eq!(selected.len(), included.len());
    for (values, expected) in [(included, true), (excluded, false)] {
        for value in values {
            let key = context
                .computation
                .import(
                    value.into(),
                    context.limits,
                    context.counters,
                    context.location,
                )
                .unwrap();
            assert_eq!(
                selected
                    .contains(&key, context.limits, context.counters, context.location)
                    .unwrap(),
                expected,
                "carrier membership for {value:?}"
            );
        }
    }
}

#[test]
fn minimum_carrier_uses_asp_order_between_symbols_and_strings() {
    with_context(
        &[
            Value::Symbol("z".into()),
            Value::Number(5),
            Value::String("a".into()),
            Value::Symbol("zz".into()),
        ],
        |terms, context| {
            let selected = carrier_values(
                AggregateFunction::Min,
                &[&[terms[0]]],
                &[&[terms[1]], &[terms[2]], &[terms[3]]],
                context,
            );
            // ASP order places every symbol below every string, independently of
            // spelling. Storage order deliberately has the opposite class order.
            assert_values(
                &selected,
                &[Value::Number(5), Value::Symbol("z".into())],
                &[Value::String("a".into()), Value::Symbol("zz".into())],
                context,
            );
        },
    );
}

#[test]
fn maximum_carrier_keeps_same_spelling_symbol_and_string_distinct() {
    with_context(
        &[
            Value::Symbol("z".into()),
            Value::Number(5),
            Value::String("z".into()),
            Value::Symbol("zz".into()),
            Value::Symbol("a".into()),
        ],
        |terms, context| {
            let selected = carrier_values(
                AggregateFunction::Max,
                &[&[terms[0]]],
                &[&[terms[1]], &[terms[2]], &[terms[3]], &[terms[4]]],
                context,
            );
            assert_values(
                &selected,
                &[
                    Value::Symbol("z".into()),
                    Value::Symbol("zz".into()),
                    Value::String("z".into()),
                ],
                &[Value::Number(5), Value::Symbol("a".into())],
                context,
            );
        },
    );
}

#[test]
fn empty_extrema_carriers_keep_bounds_and_all_optional_types() {
    // Without a required tuple the carrier keeps every optional value, of every
    // type, and the empty value: #sup for #min, #inf for #max. Each case's
    // optional values hold the other bound but not its empty value, which only
    // the carrier can supply.
    for (function, empty, other) in [
        (AggregateFunction::Min, Value::Supremum, Value::Infimum),
        (AggregateFunction::Max, Value::Infimum, Value::Supremum),
    ] {
        let values = [
            other,
            Value::Number(5),
            Value::Symbol("5".into()),
            Value::String("5".into()),
        ];
        with_context(&values, |terms, context| {
            let optional: Vec<_> = terms.iter().copied().map(|term| [term]).collect();
            let optional: Vec<&[Term]> = optional.iter().map(<[Term; 1]>::as_slice).collect();
            let selected = carrier_values(function, &[], &optional, context);
            let mut included = values.to_vec();
            included.push(empty);
            assert_values(&selected, &included, &[], context);
        });
    }
}

#[test]
fn separately_admitted_equal_constants_coalesce_as_full_tuple_keys() {
    with_context(
        &[
            Value::Symbol("z".into()),
            Value::Symbol("z".into()),
            Value::String("z".into()),
            Value::Number(1),
        ],
        |terms, context| {
            assert_ne!(
                terms[0], terms[1],
                "the fixture must use distinct compiled occurrences"
            );
            let tuples = [
                [terms[0], terms[3]],
                [terms[1], terms[3]],
                [terms[2], terms[3]],
            ];
            let mut keys = Tuples::new(context).unwrap();
            for tuple in &tuples {
                keys.insert(tuple, context).unwrap();
            }
            assert_eq!(keys.values.len(), 2);
            // Storage identity order differs from ASP order: String precedes Symbol.
            assert!(
                tuple_compare(keys.values.slice()[0], &tuples[2], context)
                    .unwrap()
                    .is_eq()
            );
            let keys_required: Vec<&[Term]> = keys.values.iter().copied().collect();
            let count = carrier_values(AggregateFunction::Count, &keys_required, &[], context);
            assert_values(&count, &[Value::Number(2)], &[], context);
            let mut required = Tuples::new(context).unwrap();
            required.insert(&tuples[0], context).unwrap();
            assert!(required.contains(&tuples[1], context).unwrap());
            assert!(!required.contains(&tuples[2], context).unwrap());
        },
    );
}

#[test]
fn stopped_tuple_insertion_preserves_the_ordered_population() {
    let values = [Value::Number(2), Value::Number(1)];
    let mut complete_work = 0;
    with_context(&values, |terms, context| {
        let first = [terms[0]];
        let second = [terms[1]];
        let mut keys = Tuples::new(context).unwrap();
        keys.insert(&first, context).unwrap();
        let start = context.counters.accounting.work;
        keys.insert(&second, context).unwrap();
        complete_work = context.counters.accounting.work - start;
    });
    assert!(complete_work > 0);
    for allowance in 0..complete_work {
        with_context(&values, |terms, context| {
            let first = [terms[0]];
            let second = [terms[1]];
            let mut keys = Tuples::new(context).unwrap();
            keys.insert(&first, context).unwrap();
            let limits = FormulaLimits {
                max_work: context.counters.accounting.work + allowance,
                ..FormulaLimits::default()
            };
            let mut limited = Context {
                computation: context.computation,
                limits: &limits,
                counters: context.counters,
                location: context.location,
                entries: context.entries,
            };
            assert!(matches!(
                keys.insert(&second, &mut limited),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert_eq!(keys.values.slice(), &[first.as_slice()]);
        });
    }
}
