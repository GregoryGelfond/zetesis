//! The decline lemmas: each attempt the join path makes on a rule declines on
//! a fact rule, for a structural reason, so a fact block makes none of them.
//! They run on the reference path's fact rules, with every attempt's test
//! switch at its default.

use super::*;
use crate::formula_ir::testing::with_facts_as_rules;
use crate::formula_ir::{HeadIr, Prepared};
use crate::formula_support::testing;

/// Fact shapes, and facts beside every kind of producer the attempts serve.
const POPULATION: &[&str] = &[
    "p(1). p(2). p(3). q(X) :- p(X).",
    "p(0..9). r(X) :- p(X), X > 7.",
    "a(1,2). a(2,3). b(2,9). c(X,Z) :- a(X,Y), b(Y,Z).",
    "e(1, c1, \"s1\", f(1, g(1))). e(2, c2, \"s2\", f(2, g(2))). k(X) :- e(X,_,_,_).",
    "p(1;2;3). { q(X) } :- p(X). #minimize { X : q(X) }.",
    "p(1..4). s(X+1) :- p(X). t(X) :- p(X), s(X), X = 2 * Y, p(Y).",
    "d(1). d(2). n(N) :- N = #count { X : d(X) }.",
    "-p(1). p(2). a. r(X) :- -p(X), a.",
];

fn fact_rules(prepared: &Prepared) -> impl Iterator<Item = (usize, &RuleIr)> {
    prepared
        .rules_without_facts()
        .iter()
        .enumerate()
        .filter(|(_, rule)| {
            rule.body.is_empty()
                && rule.variables == 0
                && rule.bindings.is_none()
                && matches!(rule.head, HeadIr::Normal(Some(_)))
        })
}

#[test]
fn every_join_path_attempt_declines_on_a_fact_rule() {
    assert!(crate::formula_factor::testing::enabled());
    assert!(producer::testing::enabled());
    assert!(crate::formula_support::range_selection_enabled());
    for source in POPULATION {
        with_facts_as_rules(|| {
            testing::with_completed_source(source, |prepared, support, computation, counters| {
                let limits = FormulaLimits::default();
                let mut budget = Budget::new(crate::ExpansionLimits::default(), usize::MAX);
                let location = crate::test_support::location();
                let mut builder = Builder::empty(
                    computation,
                    &limits,
                    &mut budget,
                    counters,
                    Purpose::Theory,
                    None,
                    location,
                )
                .unwrap();
                builder.initialize(location).unwrap();
                let mut checked = 0;
                for (_, rule) in fact_rules(prepared) {
                    // Factoring: an empty literal list leaves one group.
                    assert!(!crate::formula_factor::rule(&mut builder, rule, support).unwrap());
                    let mut outer = Join::domain_rule(
                        rule,
                        support,
                        None,
                        builder.computation,
                        builder.limits,
                        builder.budget,
                        &mut builder.counters,
                    )
                    .unwrap();
                    outer
                        .select_total_body(
                            rule,
                            builder.computation,
                            builder.limits,
                            &mut builder.counters,
                        )
                        .unwrap();
                    // Continuations: no generated join, no binding plan.
                    assert!(
                        !crate::formula_factor::continuations(
                            &mut builder,
                            rule,
                            &mut outer,
                            support
                        )
                        .unwrap()
                    );
                    // Producer rules and support witnesses: fewer than two
                    // body literals.
                    assert!(!producer::rule(&mut builder, rule, &mut outer).unwrap());
                    assert!(
                        crate::formula_support::witnesses::eligible(
                            rule,
                            &limits,
                            &mut builder.counters
                        )
                        .unwrap()
                        .is_none()
                    );
                    // Range multiples: no generated join.
                    assert!(
                        !outer
                            .select_range_multiples(
                                rule,
                                builder.computation,
                                builder.limits,
                                &mut builder.counters,
                            )
                            .unwrap()
                    );
                    checked += 1;
                }
                assert!(checked > 0, "{source}");
            });
        });
    }
}

#[test]
fn domain_guards_decline_on_a_fact_rule() {
    for source in [POPULATION[0], POPULATION[1], POPULATION[2]] {
        with_facts_as_rules(|| {
            let crate::formula::Preparation {
                program,
                catalog,
                mut budget,
                limits,
                accounting,
                location,
                ..
            } = testing::prepare(source);
            let mut counters =
                Counters::resume(accounting, crate::grounding_observer::Work::default());
            let (mut catalog, domains) = complete_support(
                &program,
                catalog,
                Some(crate::DomainLimits::default()),
                &mut budget,
                &Profile::new(None),
                GroundingWork::new(&limits, &mut counters, location),
            )
            .unwrap();
            let domains = domains.expect("positive flat source");
            let (completed, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
            let queries = completed
                .queries(crate::JoinStrategy::Indexed, &limits, &counters, location)
                .unwrap();
            let computation = Computation::new(&mut append, queries.support());
            let mut checked = 0;
            for (index, rule) in fact_rules(&program) {
                // A rule without variables has no guard to prepare.
                let candidates = domains.for_rule(index, rule).unwrap();
                assert!(
                    queries
                        .support()
                        .domain_guards(rule, candidates, &computation, &limits, &mut counters)
                        .unwrap()
                        .is_none()
                );
                checked += 1;
            }
            assert!(checked > 0, "{source}");
        });
    }
}
