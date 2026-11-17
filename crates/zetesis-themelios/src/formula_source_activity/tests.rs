//! Selected producer rows reuse only their completed ordinary comparisons.
//! General activity and original guard evaluation retain their own semantics.

use super::*;
use crate::formula_guard::{Guard, GuardComparison};
use crate::formula_ir::{Expression, Operation};
use crate::formula_support::testing::{self, Fixture, binding};
use crate::grounding_observer::{GroundingWork, Work};
use crate::test_support::location;
use crate::{ExpansionLimits, FormulaResource, JoinStrategy};
use themelios_program::program::Relation;
use themelios_program::term::{BinaryOp, EvalError};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Value, ValueLimits};

mod producers;

fn rule(fixture: &mut Fixture, relation: Relation) -> RuleIr {
    let one = fixture.scalar(&Value::Number(1), location());
    let three = fixture.scalar(&Value::Number(3), location());
    let comparison = fixture.comparison(
        vec![
            Operation::Constant(one),
            Operation::Constant(one),
            Operation::Binary(BinaryOp::Add, 0, 1),
        ],
        relation,
        vec![Operation::Constant(three)],
        location(),
    );
    RuleIr {
        head: HeadIr::Normal(None),
        body: vec![LiteralIr::Compare(comparison)],
        body_variables: 0,
        variables: 0,
        bindings: None,
        origins: vec![location()],
        location: location(),
    }
}

struct Outcome {
    result: Result<Activity, FormulaFailure>,
    work: u64,
    observed: GroundingWork,
}

/// Obtain a real selected row before either reading its activity through the
/// proposed producer boundary or running the unchanged general reference fold.
fn selected(
    fixture: &mut Fixture,
    rule: &RuleIr,
    reuse: bool,
    allowance: Option<u64>,
    cancelled: bool,
) -> Outcome {
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut join =
            Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
        let row = join
            .next(computation, &limits, &mut budget, counters, location())
            .unwrap()
            .expect("the actual join selected this comparison");
        let owner = SourceEligibility::new(&Context {
            computation,
            limits: &limits,
            budget: &mut budget,
            counters,
            location: location(),
        })
        .unwrap();
        let observed = Work::worker(true);
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: allowance.map_or(limits.max_work, |value| before + value),
            ..limits
        };
        counters
            .accounting
            .controlled(None, observed.clone(), location(), |counters| {
                let cancellation = Cancellation::default();
                if cancelled {
                    cancellation.cancel();
                    counters.attach_test_control(&cancellation);
                }
                let result = {
                    let context = Context {
                        computation,
                        limits: &limits,
                        budget: &mut budget,
                        counters,
                        location: location(),
                    };
                    let mut bodies = crate::formula_ground::SourceBodies::new(context, support);
                    if reuse {
                        owner.selected_producer_activity(
                            &rule.body,
                            &rule.body_binding(&row),
                            &mut bodies,
                            location(),
                        )
                    } else {
                        owner.activity(&rule.body, &row, &mut bodies.context(location()))
                    }
                };
                counters.detach_test_control();
                Ok::<_, FormulaFailure>(Outcome {
                    result,
                    work: counters.accounting.work - before,
                    observed: observed.snapshot(),
                })
            })
            .unwrap()
    })
}

#[test]
fn selected_comparison_keeps_one_literal_charge_without_expression_replay() {
    let mut fixture = Fixture::default();
    let rule = rule(&mut fixture, Relation::Lt);
    let reused = selected(&mut fixture, &rule, true, None, false);
    assert!(matches!(reused.result, Ok(Activity::Required)));
    assert_eq!(reused.work, 1);
    assert_eq!(reused.observed.expression_evaluations, Some(0));
    assert_eq!(reused.observed.expression_nodes, Some(0));
    let reference = selected(&mut fixture, &rule, false, None, false);
    assert!(matches!(reference.result, Ok(Activity::Required)));
    assert!(reference.work > reused.work);
    assert_eq!(reference.observed.expression_evaluations, Some(2));
    assert_eq!(reference.observed.expression_nodes, Some(4));
}

#[test]
fn rejected_comparison_has_no_selected_producer_row() {
    let mut fixture = Fixture::default();
    let rule = rule(&mut fixture, Relation::Gt);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut join =
            Join::rule(&rule, support, computation, &limits, &mut budget, counters).unwrap();
        assert!(
            join.next(computation, &limits, &mut budget, counters, location())
                .unwrap()
                .is_none()
        );
        let source = binding(&[], computation, counters, location());
        let mut context = Context {
            computation,
            limits: &limits,
            budget: &mut budget,
            counters,
            location: location(),
        };
        let owner = SourceEligibility::new(&context).unwrap();
        assert!(matches!(
            owner.activity(&rule.body, &source, &mut context),
            Ok(Activity::Absent)
        ));
    });
}

#[test]
fn discarded_operand_is_no_longer_admitted_to_the_canonical_catalog() {
    for reuse in [false, true] {
        let mut fixture = Fixture::default();
        let rule = rule(&mut fixture, Relation::Lt);
        assert!(matches!(
            selected(&mut fixture, &rule, reuse, None, false).result,
            Ok(Activity::Required)
        ));
        let (completed, mut counters) = fixture.finish(location());
        let limits = FormulaLimits::default();
        let support = completed
            .snapshot(&limits, &mut counters, location())
            .unwrap();
        let queries = support
            .queries(JoinStrategy::Indexed, &limits, &counters, location())
            .unwrap();
        let mut computation = queries.computation(location()).unwrap();
        let result = computation.number(2, &limits, &mut counters, location());
        if reuse {
            assert!(
                matches!(result, Err(FormulaFailure::UnadmittedTerm { location: site }) if site == location())
            );
        } else {
            assert!(
                result.is_ok(),
                "the old discarded full evaluation admitted 2"
            );
        }
    }
}

#[test]
fn selected_literal_still_polls_and_refuses_at_its_exact_work_boundary() {
    let mut fixture = Fixture::default();
    let rule = rule(&mut fixture, Relation::Lt);
    let refused = selected(&mut fixture, &rule, true, Some(0), false);
    assert_eq!(refused.work, 0);
    assert!(
        matches!(refused.result, Err(FormulaFailure::Limit { resource: FormulaResource::Work, location: site, limit, observed }) if site == location() && observed == limit + 1)
    );
    let admitted = selected(&mut fixture, &rule, true, Some(1), false);
    assert_eq!(admitted.work, 1);
    assert!(matches!(admitted.result, Ok(Activity::Required)));
    let interrupted = selected(&mut fixture, &rule, true, Some(0), true);
    assert_eq!(interrupted.work, 0);
    assert!(
        matches!(interrupted.result, Err(FormulaFailure::Interrupted { reason: Stop::Cancelled, location: site }) if site == location())
    );
}

fn ordinary_activity(
    fixture: &mut Fixture,
    body: &[LiteralIr],
) -> Result<Activity, FormulaFailure> {
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let source = binding(&[], computation, counters, location());
        let context = Context {
            computation,
            limits: &limits,
            budget: &mut budget,
            counters,
            location: location(),
        };
        let owner = SourceEligibility::new(&context).unwrap();
        let mut bodies = crate::formula_ground::SourceBodies::new(context, support);
        // There are no Compare literals here: their selected premise is vacuous.
        owner.selected_producer_activity(body, &source, &mut bodies, location())
    })
}

#[test]
fn selected_comparisons_do_not_replace_original_guard_truth() {
    for head in [false, true] {
        let literal = if head {
            LiteralIr::HeadGuard(Guard::Boolean(false))
        } else {
            LiteralIr::Guard(Guard::Boolean(false))
        };
        assert!(matches!(
            ordinary_activity(&mut Fixture::default(), &[literal]),
            Ok(Activity::Absent)
        ));
    }
}

#[test]
fn a_false_guard_does_not_conceal_a_later_fatal_head_guard() {
    let mut fixture = Fixture::default();
    let maximum = fixture.scalar(&Value::Number(i32::MAX), location());
    let one = fixture.scalar(&Value::Number(1), location());
    let guard = Guard::Comparisons {
        negation: themelios_program::program::DefaultNegation::None,
        comparisons: vec![GuardComparison::Scalar(
            Expression::new(vec![
                Operation::Constant(maximum),
                Operation::Constant(one),
                Operation::Binary(BinaryOp::Add, 0, 1),
            ]),
            Relation::Eq,
            Expression::new(vec![Operation::Constant(one)]),
        )],
        intervals: vec![],
    };
    let body = [
        LiteralIr::Guard(Guard::Boolean(false)),
        LiteralIr::HeadGuard(guard),
    ];
    assert!(
        matches!(ordinary_activity(&mut fixture, &body), Err(FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation { error: EvalError::Overflow, location: site })) if site == location())
    );
}
