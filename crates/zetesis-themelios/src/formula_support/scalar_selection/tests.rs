use super::*;
use crate::FormulaLimits;
use crate::formula_support::{Join, testing};
use zetesis_core::Value;

#[test]
fn signed_division_masks_preserve_every_join_witness() {
    testing::with_completed_source(
        "wide(-7..7). allowed(-1..1). tight(-1..0). \
         p(X,Y) :- wide(X), allowed(Y), tight(Y), A=X-1, K=1+1, Q=A/K, Y=Q.",
        |program, support, computation, counters| {
            let rule = program
                .rules_without_facts()
                .iter()
                .find(|rule| {
                    rule.body
                        .iter()
                        .any(|literal| matches!(literal, LiteralIr::Bind { .. }))
                })
                .unwrap();
            let limits = FormulaLimits::default();
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &limits,
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            assert!(
                join.select_total_body(rule, computation, &limits, counters)
                    .unwrap()
            );
            let view = View::prepare(
                &rule.body,
                rule.body_variables,
                Context::new(&*computation, &limits, counters, rule.location),
            )
            .unwrap()
            .unwrap();
            // Put the wider Y column first. Either full column covers a complete
            // binding; masks must remain sound without assuming their equality.
            let mut patterns = join.plan.patterns.clone();
            patterns.sort_by_key(|pattern| match pattern.atom().predicate().name() {
                "allowed" => 0,
                "tight" => 1,
                _ => 2,
            });
            let input = slot(&patterns, "wide");
            let output = slot(&patterns, "allowed");
            assert_eq!(output, slot(&patterns, "tight"));
            let mut domains = Domains::prepare(
                &patterns,
                rule.body_variables,
                support,
                Context::new(&*computation, &limits, counters, rule.location),
            )
            .unwrap();
            let mut selected = Vec::new();
            let mut witnesses = Vec::new();
            for x in -7..=7 {
                let mut values = vec![None; rule.variables];
                values[input] = Some(Value::Number(x));
                let prefix = testing::binding(&values, computation, counters, rule.location);
                let permits = domains
                    .permits(
                        &rule.body,
                        &view,
                        &prefix,
                        Context::new(&*computation, &limits, counters, rule.location),
                    )
                    .unwrap();
                if permits {
                    selected.push(x);
                }
                for y in -1..=0 {
                    values[output] = Some(Value::Number(y));
                    let binding = testing::binding(&values, computation, counters, rule.location);
                    let actual = (x - 1) / 2 == y;
                    assert!(!actual || permits, "discarded witness ({x},{y})");
                    assert_eq!(
                        domains
                            .permits(
                                &rule.body,
                                &view,
                                &binding,
                                Context::new(&*computation, &limits, counters, rule.location),
                            )
                            .unwrap(),
                        actual
                    );
                    if actual {
                        witnesses.push((x, y));
                    }
                }
            }
            // X=0 yields -1/2=0. Floor division would lose that witness.
            assert_eq!(witnesses, [(-2, -1), (-1, -1), (0, 0), (1, 0), (2, 0)]);
            // The wider column permits two prefixes which tight(Y) later
            // excludes. An interval mask supplies a necessary condition only.
            assert_eq!(selected, [-2, -1, 0, 1, 2, 3, 4]);
        },
    );
}

fn slot(patterns: &[PatternOccurrence<'_>], name: &str) -> usize {
    let pattern = patterns
        .iter()
        .find(|pattern| pattern.atom().predicate().name() == name)
        .unwrap();
    let Some(TemplateTerm::Variable(slot)) = pattern.atom().terms().at(0) else {
        panic!("a scalar fixture column is a variable");
    };
    slot
}
