use super::*;
use crate::ExpansionFailure;

fn reuse_rows(source: &str, enabled: bool) -> (Run, usize) {
    let mut output = None;
    testing::with_completed_source(source, |program, support, computation, counters| {
        // Canonical rule order need not end with the authored producer. Scope
        // both hit counting and work measurement to this exact structured join;
        // support-construction hits cannot satisfy the query's route assertion.
        let mut producers = program.rules.iter().filter(|rule| {
            rule.body
                .iter()
                .any(|literal| matches!(literal, crate::formula_ir::LiteralIr::PatternAtom(_)))
        });
        let rule = producers.next().expect("structured producer fixture");
        assert!(producers.next().is_none());
        output = Some(reuse_scoped(enabled, || {
            query_rule(rule, support, computation, counters)
        }));
    });
    output.expect("completed source callback")
}

const REPEATED: &str = "k(1..2).t(1..10).p(other(1..30)).p(f(g(1),1,7)).p(f(g(2),2,7)). \
                       r(X,Y):-k(X),t(Y),p(f(g(X),X,7)).";

#[test]
fn unchanged_inputs_reduce_complete_query_work() {
    let (actual, reused) = reuse_rows(REPEATED, true);
    let (reference, skipped) = reuse_rows(REPEATED, false);
    assert_eq!(reused, 18);
    assert_eq!(skipped, 0);
    assert_eq!(actual.rows, reference.rows);
    let mut expected = (1..=2)
        .flat_map(|x| (1..=10).map(move |y| vec![x.to_string(), y.to_string()]))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual.rows, expected);
    assert_eq!(actual.receipt.join_rows, reference.receipt.join_rows);
    // Includes one-time source-coordinate preparation, retained storage and
    // all changed-input validation; no test thread inherits this control.
    assert!(
        actual.work < reference.work,
        "{} >= {}",
        actual.work,
        reference.work
    );
}

fn theory(source: &str, enabled: bool) -> (crate::formula::Compiled, usize) {
    reuse_scoped(enabled, || {
        crate::formula_ground::ground(testing::prepare(source), None, None).unwrap()
    })
}

#[test]
fn prepared_arguments_preserve_formula_semantics() {
    use zetesis_cpu::Cancellation;
    use zetesis_ferraris::{Limits, models, models_reduct};
    let source = "{k(1);t(1);t(2);p(f(1));p(other(1));p(other(2))}.r:-k(X),t(Y),p(f(X)).";
    let (actual, hits) = theory(source, true);
    let (reference, skipped) = theory(source, false);
    assert!(hits > 0);
    assert_eq!(skipped, 0);
    assert_eq!(actual.origins, reference.origins);
    let atoms = |compiled: &crate::formula::Compiled| {
        compiled
            .atoms
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(atoms(&actual), atoms(&reference));
    let worlds = worlds(&actual.theory, &reference.theory);
    for (left, right) in &worlds {
        assert_eq!(
            models(
                &actual.theory,
                left,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            models(
                &reference.theory,
                right,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
        );
        for (left_tested, right_tested) in &worlds {
            assert_eq!(
                models_reduct(
                    &actual.theory,
                    left,
                    left_tested,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                models_reduct(
                    &reference.theory,
                    right,
                    right_tested,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap()
            );
        }
    }
}

#[test]
fn later_support_rounds_keep_complete_producers() {
    let source = "p(f(1)).t(1..2).edge(1,2).edge(2,3).p(other(1..4)). \
                  p(f(Y)):-edge(X,Y),t(T),p(f(X)).";
    let (actual, hits) = theory(source, true);
    let (reference, _) = theory(source, false);
    assert!(hits > 0);
    assert_eq!(actual.origins, reference.origins);
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
    assert_eq!(actual.theory.roots(), reference.theory.roots());
    assert_eq!(actual.atoms.atoms().len(), reference.atoms.atoms().len());
}

#[test]
fn prepared_arguments_preserve_arithmetic_diagnostics() {
    let source = "k(0).t(1..2).p(f(0)).p(other(1..3)).r(X,Y,Z):-k(X),t(Y),p(f(X)),Z=1/X.";
    let failure = |enabled| {
        reuse_scoped(enabled, || {
            crate::formula_ground::ground(testing::prepare(source), None, None).unwrap_err()
        })
        .0
    };
    let actual = failure(true);
    let reference = failure(false);
    assert!(matches!(
        actual,
        FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
    ));
    assert_eq!(format!("{actual:?}"), format!("{reference:?}"));
}

fn structural<'a>(join: &Join<'a, '_>) -> Pattern<'a> {
    join.plan
        .patterns
        .iter()
        .find_map(|occurrence| match occurrence.pattern {
            PositivePattern::Structural(pattern) => Some(pattern),
            PositivePattern::Flat(_) => None,
        })
        .unwrap()
}

fn input(pattern: Pattern<'_>) -> usize {
    pattern
        .arguments()
        .iter()
        .flat_map(|argument| &argument.nodes)
        .find_map(|node| match node {
            PatternNode::Slot(slot) => Some(*slot),
            _ => None,
        })
        .unwrap()
}

const DIRECT: &str = "k(1..2).p(f(g(1),1,7)).p(f(g(2),2,7)).r(X):-k(X),p(f(g(X),X,7)).";

#[test]
fn binding_transitions_preserve_resolution() {
    testing::with_completed_source(DIRECT, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
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
        let pattern = structural(&join);
        let slot = input(pattern);
        let mut scratch = Scratch::new(
            &join.plan.patterns,
            &mut Context::new(&*computation, &limits, counters, location),
        )
        .unwrap();
        let ((rows, empty), hits) = reuse_scoped(true, || {
            let mut rows = Vec::new();
            for value in [1, 1, 2, 2, 1, 1] {
                let key = computation
                    .number(value, &limits, counters, location)
                    .unwrap();
                join.values
                    .set(slot, &key, &limits, counters, location)
                    .unwrap();
                assert!(
                    scratch
                        .prepare(
                            pattern,
                            &join.values,
                            &mut Context::new(computation, &limits, counters, location)
                        )
                        .unwrap()
                );
                rows.push(
                    scratch
                        .arguments
                        .read(0, computation.read(), location)
                        .unwrap()
                        .to_string(),
                );
                scratch.clear();
            }
            join.values
                .clear(slot, &limits, counters, location)
                .unwrap();
            assert!(
                scratch
                    .prepare(
                        pattern,
                        &join.values,
                        &mut Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
            );
            let empty = !scratch.arguments.is_bound(0, location).unwrap();
            (rows, empty)
        });
        assert_eq!(hits, 3);
        assert!(empty);
        assert_eq!(
            rows,
            [
                "f(g(1),1,7)",
                "f(g(1),1,7)",
                "f(g(2),2,7)",
                "f(g(2),2,7)",
                "f(g(1),1,7)",
                "f(g(1),1,7)"
            ]
        );
    });
}

#[test]
fn absent_results_are_reconsidered_after_growth() {
    let source = "k(1).p(other(0)).r(X):-k(X),p(f(X)).";
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
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
        let pattern = structural(&join);
        let slot = input(pattern);
        let key = computation.number(1, &limits, counters, location).unwrap();
        join.values
            .set(slot, &key, &limits, counters, location)
            .unwrap();
        let mut scratch = Scratch::new(
            &join.plan.patterns,
            &mut Context::new(&*computation, &limits, counters, location),
        )
        .unwrap();
        let ((), hits) = reuse_scoped(true, || {
            assert!(
                !scratch
                    .prepare(
                        pattern,
                        &join.values,
                        &mut Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
            );
            scratch.clear();
            computation
                .construct(
                    ValueNodeRef::Function {
                        name: "f",
                        sign: zetesis_core::Sign::Positive,
                        arity: 1,
                    },
                    join.values.slots(),
                    &[slot],
                    zetesis_core::catalog::Limits::default(),
                    GroundingWork::new(&limits, counters, location),
                )
                .unwrap();
            assert!(
                scratch
                    .prepare(
                        pattern,
                        &join.values,
                        &mut Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
            );
            assert_eq!(
                scratch
                    .arguments
                    .read(0, computation.read(), location)
                    .unwrap()
                    .to_string(),
                "f(1)"
            );
            scratch.clear();
            computation.number(99, &limits, counters, location).unwrap();
            assert!(
                scratch
                    .prepare(
                        pattern,
                        &join.values,
                        &mut Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
            );
            assert_eq!(
                scratch
                    .arguments
                    .read(0, computation.read(), location)
                    .unwrap()
                    .to_string(),
                "f(1)"
            );
        });
        assert_eq!(hits, 1);
    });
}

#[test]
fn ambiguous_capture_coordinates_decline_reuse() {
    testing::with_completed_source(DIRECT, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
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
        let occurrence = *join
            .plan
            .patterns
            .iter()
            .find(|occurrence| matches!(occurrence.pattern, PositivePattern::Structural(_)))
            .unwrap();
        let PositivePattern::Structural(pattern) = occurrence.pattern else {
            unreachable!()
        };
        let key = computation.number(1, &limits, counters, location).unwrap();
        join.values
            .set(input(pattern), &key, &limits, counters, location)
            .unwrap();
        // A manually assembled IR can reuse a whole capture coordinate. The
        // actual plan sees both occurrences and declines instead of conflating.
        let mut scratch = Scratch::new(
            &[occurrence, occurrence],
            &mut Context::new(&*computation, &limits, counters, location),
        )
        .unwrap();
        let ((), hits) = reuse_scoped(true, || {
            for _ in 0..2 {
                assert!(
                    scratch
                        .prepare(
                            pattern,
                            &join.values,
                            &mut Context::new(computation, &limits, counters, location)
                        )
                        .unwrap()
                );
                assert_eq!(
                    scratch
                        .arguments
                        .read(0, computation.read(), location)
                        .unwrap()
                        .to_string(),
                    "f(g(1),1,7)"
                );
                scratch.clear();
            }
        });
        assert_eq!(hits, 0);
    });
}

#[test]
fn interrupted_replacement_cannot_publish_partial_inputs() {
    testing::with_completed_source(DIRECT, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
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
        let pattern = structural(&join);
        let slot = input(pattern);
        let first = computation.number(1, &limits, counters, location).unwrap();
        let second = computation.number(2, &limits, counters, location).unwrap();
        let baseline = support.workspace_bytes();
        let mut attempt = |allowance: Option<u64>| {
            let mut scratch = Scratch::new(
                &join.plan.patterns,
                &mut Context::new(&*computation, &limits, counters, location),
            )
            .unwrap();
            join.values
                .set(slot, &first, &limits, counters, location)
                .unwrap();
            assert!(
                scratch
                    .prepare(
                        pattern,
                        &join.values,
                        &mut Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
            );
            scratch.clear();
            join.values
                .set(slot, &second, &limits, counters, location)
                .unwrap();
            let before = counters.accounting.work;
            let cut = FormulaLimits {
                max_work: allowance.map_or(u64::MAX, |work| before + work),
                ..limits
            };
            let result = scratch.prepare(
                pattern,
                &join.values,
                &mut Context::new(computation, &cut, counters, location),
            );
            let work = counters.accounting.work - before;
            scratch.clear();
            if allowance.is_some() {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ));
            } else {
                assert!(result.unwrap());
            }
            // The same retained owner retries after every refusal, including
            // cuts in the repeated-X dependency copy and final result write.
            assert!(
                scratch
                    .prepare(
                        pattern,
                        &join.values,
                        &mut Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
            );
            assert_eq!(
                scratch
                    .arguments
                    .read(0, computation.read(), location)
                    .unwrap()
                    .to_string(),
                "f(g(2),2,7)"
            );
            drop(scratch);
            assert_eq!(support.workspace_bytes(), baseline);
            work
        };
        let exact = attempt(None);
        assert!(exact > 0);
        for cut in 0..exact {
            attempt(Some(cut));
        }
    });
}

#[derive(Clone, Copy)]
enum Refusal {
    Cancellation,
    Storage,
}

fn retained_hit_refusal(refusal: Refusal) {
    testing::with_completed_source(DIRECT, |program, support, computation, counters| {
        let rule = program.rules.last().unwrap();
        let location = rule.location;
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
        let pattern = structural(&join);
        let key = computation.number(1, &limits, counters, location).unwrap();
        join.values
            .set(input(pattern), &key, &limits, counters, location)
            .unwrap();
        let mut scratch = Scratch::new(
            &join.plan.patterns,
            &mut Context::new(&*computation, &limits, counters, location),
        )
        .unwrap();
        assert!(
            scratch
                .prepare(
                    pattern,
                    &join.values,
                    &mut Context::new(computation, &limits, counters, location)
                )
                .unwrap()
        );
        scratch.clear();
        let restricted = match refusal {
            Refusal::Cancellation => {
                let cancelled = zetesis_cpu::Cancellation::default();
                cancelled.cancel();
                counters.cancellation = Some(cancelled);
                limits
            }
            Refusal::Storage => FormulaLimits {
                max_support_bytes: 0,
                ..limits
            },
        };
        let result = scratch.prepare(
            pattern,
            &join.values,
            &mut Context::new(computation, &restricted, counters, location),
        );
        match refusal {
            Refusal::Cancellation => assert!(matches!(
                result,
                Err(FormulaFailure::Interrupted {
                    reason: zetesis_cpu::Stop::Cancelled,
                    ..
                })
            )),
            Refusal::Storage => assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    ..
                })
            )),
        }
        counters.cancellation = None;
        scratch.clear();
        let ((), hits) = reuse_scoped(true, || {
            assert!(
                scratch
                    .prepare(
                        pattern,
                        &join.values,
                        &mut Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
            );
        });
        assert_eq!(hits, 1);
    });
}

#[test]
fn retained_hits_recheck_cancellation() {
    retained_hit_refusal(Refusal::Cancellation);
}

#[test]
fn retained_hits_recheck_live_storage() {
    retained_hit_refusal(Refusal::Storage);
}

fn query_attempt(
    program: &crate::formula_ir::Prepared,
    support: &super::super::super::Support<'_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut super::super::super::Counters,
    work: Option<u64>,
    bytes: Option<usize>,
    cancelled: bool,
) -> (Result<bool, FormulaFailure>, u64, Receipt) {
    let rule = program.rules.last().unwrap();
    let location = rule.location;
    let mut budget = testing::budget();
    let mut join = Join::rule(
        rule,
        support,
        computation,
        &FormulaLimits::default(),
        &mut budget,
        counters,
    )
    .unwrap();
    let before = counters.accounting.work;
    let limits = FormulaLimits {
        max_work: work.map_or(u64::MAX, |work| before + work),
        max_support_bytes: bytes.unwrap_or(usize::MAX),
        ..FormulaLimits::default()
    };
    if cancelled {
        let token = zetesis_cpu::Cancellation::default();
        token.cancel();
        counters.cancellation = Some(token);
    }
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let previous = std::mem::replace(&mut counters.observed, profile.work());
    let result = profile.phase(GroundingPhase::RuleInstantiation, Some(location), || {
        join.next_row(computation, &limits, &mut budget, counters, location)
            .map(|row| row.is_some_and(|row| row.passes))
    });
    counters.observed = previous;
    counters.cancellation = None;
    assert_eq!(join.lease.bytes(), join.storage_bytes());
    assert!(join.pattern_captures.is_empty());
    cleared(&join);
    (result, counters.accounting.work - before, observer.0.get())
}

#[test]
fn prepared_queries_keep_exact_work_refusals() {
    testing::with_completed_source(DIRECT, |program, support, computation, counters| {
        let (result, exact, _) =
            query_attempt(program, support, computation, counters, None, None, false);
        assert!(result.unwrap());
        for cut in 0..exact {
            assert!(
                matches!(
                    query_attempt(
                        program,
                        support,
                        computation,
                        counters,
                        Some(cut),
                        None,
                        false
                    )
                    .0,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ),
                "cut={cut}"
            );
        }
        assert!(
            query_attempt(
                program,
                support,
                computation,
                counters,
                Some(exact),
                None,
                false
            )
            .0
            .unwrap()
        );
        assert!(matches!(
            query_attempt(program, support, computation, counters, None, None, true).0,
            Err(FormulaFailure::Interrupted {
                reason: zetesis_cpu::Stop::Cancelled,
                ..
            })
        ));
    });
}

#[test]
fn prepared_storage_has_an_inclusive_boundary() {
    let attempt = |bytes| {
        let mut output = None;
        testing::with_completed_source(DIRECT, |program, support, computation, counters| {
            output = Some(query_attempt(
                program,
                support,
                computation,
                counters,
                None,
                bytes,
                false,
            ));
        });
        output.unwrap()
    };
    let (result, _, receipt) = attempt(None);
    assert!(result.unwrap());
    let exact = usize::try_from(receipt.support_peak_bytes.unwrap()).unwrap();
    assert!(attempt(Some(exact)).0.unwrap());
    assert!(matches!(
        attempt(Some(exact - 1)).0,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
}
