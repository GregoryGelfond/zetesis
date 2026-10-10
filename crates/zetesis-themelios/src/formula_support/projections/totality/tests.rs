mod numeric;

use super::*;
use crate::FormulaLimits;
use crate::formula_ir::Prepared;
use crate::formula_support::{Coverage, Join, family::Evidence, testing};
use themelios_program::term::EvalError;

fn constraints(program: &Prepared) -> impl Iterator<Item = &RuleIr> {
    program
        .rules
        .iter()
        .filter(|rule| matches!(rule.head, HeadIr::Normal(None)))
}

#[test]
fn unreachable_bad_domain_value_declines_without_source_failure() {
    // The complete X column includes a symbol, but the constant second argument
    // prevents that source row from extending the original body.
    testing::with_completed_source(
        "pair(symbol,skip).pair(2,keep). :- pair(X,keep),X/2=0.",
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
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
                !join
                    .select_total_constraint(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(join.coverage == Coverage::Complete);
            let mut rows = 0;
            while let Some(row) = join
                .next_row(
                    computation,
                    &limits,
                    &mut testing::budget(),
                    counters,
                    rule.location,
                )
                .unwrap()
            {
                rows += 1;
                assert!(!row.passes);
            }
            assert_eq!(rows, 1);
        },
    );
}

#[test]
fn reached_independent_overflow_remains_fatal() {
    testing::with_completed_source(
        "d(1). :- d(X),not X/2=0,X+2147483647=0.",
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
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
                !join
                    .select_total_constraint(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(matches!(
                join.next_row(
                    computation,
                    &limits,
                    &mut testing::budget(),
                    counters,
                    rule.location
                ),
                Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvalError::Overflow,
                    ..
                }))
            ));
        },
    );
}

#[test]
fn each_comparison_form_requires_total_expressions() {
    for body in [
        ":- d(X),(1/X,X)=(1,1).",
        ":- d(X),not 1/X=1.",
        "1/X=1 :- d(X).",
    ] {
        for (input, expected) in [(0, false), (1, true)] {
            let source = format!("d({input}). {body}");
            testing::with_completed_source(&source, |program, support, computation, counters| {
                let rule = constraints(program).next().unwrap();
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
                assert_eq!(
                    join.select_total_constraint(rule, computation, &limits, counters)
                        .unwrap(),
                    expected,
                    "{source}"
                );
            });
        }
    }
}

#[test]
fn later_column_certificate_does_not_diagnose_an_incomplete_prefix() {
    // a is smaller and joins first, but b's X column is the smaller input
    // domain. X=0 can reach a speculative comparison and has no b extension.
    testing::with_completed_source(
        "a(0;1).b(1,1..10). :- a(X),b(X,Y),1/X=1.",
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
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
                join.select_total_constraint(rule, computation, &limits, counters)
                    .unwrap()
            );
            let mut rows = 0;
            while let Some(row) = join
                .next_row(
                    computation,
                    &limits,
                    &mut testing::budget(),
                    counters,
                    rule.location,
                )
                .unwrap()
            {
                assert!(row.passes);
                rows += 1;
            }
            assert_eq!(rows, 10);
        },
    );
}

#[test]
fn evidence_keeps_false_defined_rows_of_same_location_siblings() {
    testing::with_completed_source(
        "d(0). :- d(X),X/(1;X)=1.",
        |program, support, computation, counters| {
            let rules: Vec<_> = constraints(program).collect();
            assert_eq!(rules.len(), 2);
            assert_eq!(rules[0].location, rules[1].location);
            let limits = FormulaLimits::default();
            let mut evidence = Evidence::default();
            let mut certificates = 0;
            for rule in rules {
                let mut join = Join::rule(
                    rule,
                    support,
                    computation,
                    &limits,
                    &mut testing::budget(),
                    counters,
                )
                .unwrap();
                certificates += usize::from(
                    join.select_total_constraint(rule, computation, &limits, counters)
                        .unwrap(),
                );
                join.evidence();
                assert!(join.coverage == Coverage::Complete);
                assert!(!join.certified_total);
                while join
                    .next_row(
                        computation,
                        &limits,
                        &mut testing::budget(),
                        counters,
                        rule.location,
                    )
                    .unwrap()
                    .is_some()
                {}
                evidence.merge(join.take_family());
            }
            assert_eq!(certificates, 1);
            // The total sibling's false row converts the zero-only sibling
            // into a warning. Pruning that witness would cause an error.
            assert!(evidence.finish().unwrap());
        },
    );
}

#[test]
fn constructor_expressions_decline_numeric_totality() {
    testing::with_completed_source(
        "d(1). :- d(X),not f(X)=f(1),X/2=0.",
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
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
                !join
                    .select_total_constraint(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(join.coverage == Coverage::Complete);
        },
    );
}

#[test]
fn another_rule_cannot_certify_this_join() {
    testing::with_completed_source(
        "d(1). :- d(X),X/2=0. :- d(X),X/2=1.",
        |program, support, computation, counters| {
            let rules: Vec<_> = constraints(program).collect();
            assert_eq!(rules.len(), 2);
            let limits = FormulaLimits::default();
            let mut join = Join::rule(
                rules[0],
                support,
                computation,
                &limits,
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            assert!(matches!(
                join.select_total_constraint(rules[1], computation, &limits, counters),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
            assert!(!join.certified_total);
            assert!(join.coverage == Coverage::Complete);
        },
    );
}

#[test]
fn a_column_only_the_comparison_reads_remains_a_domain() {
    // Y occurs in e and in the comparison alone. Support demand counts the
    // comparison's occurrence, so e's second column keeps its postings, and
    // its values (no zero) certify 1/Y total.
    testing::with_completed_source(
        "d(1..3). e(1..3,1..2). :- d(X), e(X,Y), 1/Y = 1.",
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
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
                join.select_total_constraint(rule, computation, &limits, counters)
                    .unwrap()
            );
        },
    );
}
