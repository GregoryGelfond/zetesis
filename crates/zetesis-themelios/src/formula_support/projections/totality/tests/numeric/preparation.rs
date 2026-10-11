use super::*;
use crate::formula_support::projections::totality::numeric::Preparation;
use crate::{FormulaResource, ProgramSite};

fn with_expressions(
    run: impl FnOnce(&RuleIr, [&Expression; 2], &Support<'_>, &mut Computation<'_, '_>, &mut Counters),
) {
    testing::with_completed_source(
        "p(1..32). q(1..32). :- p(X),q(Y),X+Y=0,X-Y=0.",
        |program, support, computation, counters| {
            let rule = constraints(program).next().unwrap();
            let expressions: Vec<_> = rule
                .body
                .iter()
                .filter_map(|literal| match literal {
                    LiteralIr::Compare(left, _, _) if left.inputs().count() > 1 => Some(left),
                    _ => None,
                })
                .collect();
            assert_eq!(expressions.len(), 2);
            run(
                rule,
                [expressions[0], expressions[1]],
                support,
                computation,
                counters,
            );
        },
    );
}

fn total(
    preparation: &mut Preparation<'_, '_>,
    expression: &Expression,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    limits: &FormulaLimits,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    preparation.total(
        expression,
        &mut Context::new(computation, limits, counters, location),
    )
}

#[test]
fn comparison_expressions_reuse_completed_numeric_hulls() {
    with_expressions(|rule, [first, second], support, computation, counters| {
        let limits = FormulaLimits::default();
        let live = counters.workspace_bytes();
        let mut preparation = Preparation::new(rule, support);
        assert_eq!(counters.workspace_bytes(), live);
        let before = counters.accounting.work;
        assert!(
            total(
                &mut preparation,
                first,
                computation,
                counters,
                &limits,
                rule.location
            )
            .unwrap()
        );
        let cold = counters.accounting.work - before;
        let prepared_bytes = counters.workspace_bytes();
        assert!(prepared_bytes > live);
        let before = counters.accounting.work;
        assert!(
            total(
                &mut preparation,
                second,
                computation,
                counters,
                &limits,
                rule.location
            )
            .unwrap()
        );
        let reused = counters.accounting.work - before;
        // Both expressions read the same two complete 32-value columns. The
        // second checks its own arithmetic without scanning either column again.
        assert!(cold >= reused + 64, "cold={cold}, reused={reused}");
        assert_eq!(counters.workspace_bytes(), prepared_bytes);
        drop(preparation);
        assert_eq!(counters.workspace_bytes(), live);
    });
}

#[test]
fn a_new_attempt_recomputes_its_numeric_evidence() {
    with_expressions(|rule, [first, _], support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut costs = Vec::new();
        for _ in 0..2 {
            let mut preparation = Preparation::new(rule, support);
            let before = counters.accounting.work;
            assert!(
                total(
                    &mut preparation,
                    first,
                    computation,
                    counters,
                    &limits,
                    rule.location
                )
                .unwrap()
            );
            costs.push(counters.accounting.work - before);
        }
        assert_eq!(costs[0], costs[1]);
        assert!(costs[0] > 64);
    });
}

#[test]
fn retained_numeric_hulls_obey_current_storage_limits() {
    with_expressions(|rule, [first, second], support, computation, counters| {
        let mut preparation = Preparation::new(rule, support);
        assert!(
            total(
                &mut preparation,
                first,
                computation,
                counters,
                &FormulaLimits::default(),
                rule.location
            )
            .unwrap()
        );
        let limits = FormulaLimits {
            max_support_bytes: 0,
            ..FormulaLimits::default()
        };
        assert!(matches!(
            total(&mut preparation, second, computation, counters, &limits, rule.location),
            Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, location, .. })
                if location == rule.location
        ));
    });
}

#[test]
fn cancelled_hull_reuse_cannot_certify_an_expression() {
    with_expressions(|rule, [first, second], support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut preparation = Preparation::new(rule, support);
        assert!(
            total(
                &mut preparation,
                first,
                computation,
                counters,
                &limits,
                rule.location
            )
            .unwrap()
        );
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        counters.cancellation = Some(cancellation);
        assert!(matches!(
            total(&mut preparation, second, computation, counters, &limits, rule.location),
            Err(FormulaFailure::Interrupted { reason: zetesis_cpu::Stop::Cancelled, location })
                if location == rule.location
        ));
    });
}

#[test]
fn hull_reuse_work_cutoffs_are_inclusive() {
    with_expressions(|rule, [first, second], support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut preparation = Preparation::new(rule, support);
        assert!(
            total(
                &mut preparation,
                first,
                computation,
                counters,
                &limits,
                rule.location
            )
            .unwrap()
        );
        let before = counters.accounting.work;
        assert!(
            total(
                &mut preparation,
                second,
                computation,
                counters,
                &limits,
                rule.location
            )
            .unwrap()
        );
        let required = counters.accounting.work - before;
        assert!(required > 0);
        for cutoff in 0..=required {
            let bounded = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..limits
            };
            let result = total(
                &mut preparation,
                second,
                computation,
                counters,
                &bounded,
                rule.location,
            );
            if cutoff == required {
                assert!(result.unwrap());
            } else {
                assert!(
                    matches!(result, Err(FormulaFailure::Limit { resource: FormulaResource::Work, location, .. }) if location == rule.location)
                );
            }
            assert!(counters.accounting.work <= bounded.max_work);
        }
    });
}

#[test]
fn successful_hulls_do_not_hide_later_intermediate_overflow() {
    testing::with_completed_source(
        "p(1).q(1). :-p(X),q(Y),X+Y=2,(X+2147483647)-Y=0.",
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
