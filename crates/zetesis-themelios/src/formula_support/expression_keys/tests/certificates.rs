use super::*;
use crate::{FormulaLimits, FormulaResource};

fn with_local(
    check: impl FnOnce(
        &mut Join<'_, '_>,
        &mut Computation<'_, '_>,
        &mut crate::formula_support::Counters,
        &FormulaLimits,
        ProgramSite,
        usize,
    ),
) {
    testing::with_completed_source(
        "p(3;1;2). b(2). out(B,N) :- b(B), N=#count{X:p(X),X<B}.",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| {
                    rule.body
                        .iter()
                        .any(|literal| matches!(literal, LiteralIr::Aggregate(_)))
                })
                .unwrap();
            let aggregate = rule
                .body
                .iter()
                .find_map(|literal| match literal {
                    LiteralIr::Aggregate(aggregate) => Some(aggregate),
                    _ => None,
                })
                .unwrap();
            assert_eq!(aggregate.family_inputs.len(), 1);
            let input = aggregate.family_inputs[0];
            let element = &aggregate.elements[0];
            let mut inputs = vec![None; rule.variables];
            inputs[input] = Some(zetesis_core::Value::Number(2));
            let prefix = testing::binding(&inputs, computation, counters, rule.location);
            let limits = FormulaLimits::default();
            let mut budget = testing::budget();
            let mut join = Join::local(
                element.condition.as_slice(),
                &prefix,
                element.variables,
                support,
                &mut budget,
                Context::new(&*computation, &limits, counters, rule.location),
            )
            .unwrap();
            check(
                &mut join,
                computation,
                counters,
                &limits,
                rule.location,
                input,
            );
        },
    );
}

#[test]
fn missing_inherited_input_declines_local_selection() {
    with_local(|join, computation, counters, limits, location, input| {
        join.values
            .clear(input, limits, counters, location)
            .unwrap();
        join.select_numeric_local(Context::new(&*computation, limits, counters, location))
            .unwrap();
        assert!(join.expression_keys.is_none());
    });
}

#[test]
fn refused_local_certificate_publishes_no_selector() {
    with_local(|join, computation, counters, limits, location, _| {
        let before = counters.accounting.work;
        let limited = FormulaLimits {
            max_work: before,
            ..*limits
        };
        let patterns = join
            .plan
            .patterns
            .iter()
            .map(|pattern| pattern.source)
            .collect::<Vec<_>>();
        let refused =
            join.select_numeric_local(Context::new(&*computation, &limited, counters, location));
        assert!(matches!(
            refused,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(counters.accounting.work, before);
        assert!(join.expression_keys.is_none());
        assert_eq!(
            join.plan
                .patterns
                .iter()
                .map(|pattern| pattern.source)
                .collect::<Vec<_>>(),
            patterns
        );
        join.select_numeric_local(Context::new(&*computation, limits, counters, location))
            .unwrap();
        assert!(join.expression_keys.is_some());
    });
}

mod boundaries;
