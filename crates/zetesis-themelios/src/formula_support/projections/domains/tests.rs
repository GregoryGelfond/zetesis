use super::*;
use crate::ProgramSite;
use crate::formula_ir::HeadIr;
use crate::formula_support::testing::{Fixture, binding, budget, numbers, with_completed_source};
use crate::formula_support::{Counters, Coverage};
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use crate::{FormulaLimits, FormulaResource, GroundingPhase, GroundingWork};
use zetesis_core::catalog::{AssignmentError, ReadError};
use zetesis_core::{Value, ValueLimits};

fn with_join(
    source: &str,
    run: impl FnOnce(&mut Join<'_, '_>, &mut Computation<'_, '_>, &mut Counters, ProgramSite),
) {
    with_completed_source(source, |program, support, computation, counters| {
        let rule = program
            .rules
            .iter()
            .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
            .unwrap();
        let limits = FormulaLimits::default();
        let mut join =
            Join::rule(rule, support, computation, &limits, &mut budget(), counters).unwrap();
        assert!(
            join.select_total_constraint(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(join.certified_total);
        run(&mut join, computation, counters, rule.location);
    });
}

fn collect(
    join: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: ProgramSite,
) -> (Vec<Vec<Value>>, GroundingWork) {
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    counters.observed = profile.work();
    let rows = profile
        .phase(GroundingPhase::RuleInstantiation, Some(location), || {
            let mut rows = Vec::new();
            while let Some(row) = join.next_row(
                computation,
                &FormulaLimits::default(),
                &mut budget(),
                counters,
                location,
            )? {
                if row.passes {
                    rows.push(
                        (0..row.values.len())
                            .map(|slot| {
                                row.values
                                    .read(slot, computation.read(), location)
                                    .unwrap()
                                    .to_value(ValueLimits::default())
                                    .unwrap()
                            })
                            .collect(),
                    );
                }
            }
            Ok::<_, FormulaFailure>(rows)
        })
        .unwrap();
    (rows, observer.0.get())
}

#[test]
fn computed_groups_keep_each_matching_input() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_preparations, Some(1));
            assert_eq!(work.table_probes, Some(1));
            assert_eq!(work.join_rows, Some(3));
        },
    );
}

#[test]
fn additional_equalities_remain_residual_checks() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y,X\\2=1.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[5, 2])]);
            assert!(work.table_probes.unwrap() > 0);
        },
    );
}

#[test]
fn covering_columns_preserve_complete_bindings() {
    with_join(
        "a(2,1..10).a(4,1..10).b(2;4;5).q(2). :-q(K),b(X),a(X,Y),X/2=K.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            // Normalized body order introduces a(X,Y) before q(K): the
            // retained assignment columns are X, Y, K, not authored K, X, Y.
            assert!(matches!(
                target(join, "a").terms().at(0),
                Some(TemplateTerm::Variable(0))
            ));
            assert!(matches!(
                target(join, "a").terms().at(1),
                Some(TemplateTerm::Variable(1))
            ));
            assert!(matches!(
                target(join, "q").terms().at(0),
                Some(TemplateTerm::Variable(2))
            ));
            let expected: Vec<_> = (1..=10).map(|y| numbers(&[4, y, 2])).collect();
            assert_eq!(rows, expected);
            assert!(work.table_probes.unwrap() > 0);
        },
    );
}

#[test]
fn computed_groups_preserve_repeated_columns() {
    with_join(
        "p(4,4).p(4,5).p(5,5).p(6,6).q(2). :-p(X,X),q(Y),X/2=Y,X<5.",
        |join, computation, counters, location| {
            // Exercise computed selection with Y known and X still free,
            // independently of the planner's comparison-readiness preference.
            assert_eq!(join.depth, 0);
            assert!(join.probes.iter().all(Option::is_none));
            join.owned_plan()
                .patterns
                .sort_by_key(|occurrence| occurrence.atom().predicate().name() != "q");
            join.decide(
                computation,
                &FormulaLimits::default(),
                &mut budget(),
                counters,
                location,
            )
            .unwrap();
            assert_eq!(
                join.plan
                    .patterns
                    .iter()
                    .map(|occurrence| occurrence.atom().predicate().name())
                    .collect::<Vec<_>>(),
                ["q", "p"],
            );
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2])]);
            assert_eq!(work.table_probes, Some(1));
        },
    );
}

#[test]
fn typed_known_values_are_not_coerced() {
    with_join(
        "p(1..6).q(2;\"2\";two). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_probes, Some(1));
        },
    );
}

#[test]
fn all_allowed_groups_keep_indexed_probes() {
    with_join(
        "p(4;5).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let (rows, work) = collect(join, computation, counters, location);
            assert_eq!(rows, vec![numbers(&[4, 2]), numbers(&[5, 2])]);
            assert_eq!(work.table_preparations, Some(0));
            assert_eq!(work.table_probes, Some(0));
        },
    );
}

#[test]
fn source_evidence_retains_complete_row_coverage() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            join.evidence();
            assert!(join.coverage == Coverage::Complete);
            let (_, work) = collect(join, computation, counters, location);
            assert_eq!(work.join_rows, Some(7));
            assert_eq!(work.table_preparations, Some(0));
            assert_eq!(work.table_probes, Some(0));
        },
    );
}

fn target<'a>(join: &Join<'a, '_>, name: &str) -> PatternRef<'a> {
    join.plan
        .patterns
        .iter()
        .find(|pattern| pattern.atom().predicate().name() == name)
        .unwrap()
        .atom()
}

#[test]
fn uncovered_known_inputs_decline_selection() {
    with_join(
        "a(0;1).b(1,1..10).c(1..4). :-a(K),b(K,Y),c(X),1/K=X/2.",
        |join, computation, counters, location| {
            let values = binding(
                &[Some(Value::Number(0)), None, None],
                computation,
                counters,
                location,
            );
            let domains = join
                .projections
                .domains(
                    join.literals,
                    target(join, "c"),
                    &values,
                    Context::new(&*computation, &FormulaLimits::default(), counters, location),
                )
                .unwrap();
            assert!(domains.is_none());
            assert!(join.failure.is_none());
        },
    );
}

#[test]
fn foreign_known_values_cannot_select_local_rows() {
    let mut foreign = Fixture::default();
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let values = foreign.with(location, |_, foreign_computation, foreign_counters| {
                binding(
                    &[None, Some(Value::Number(2))],
                    foreign_computation,
                    foreign_counters,
                    location,
                )
            });
            assert!(matches!(
                join.projections.domains(
                    join.literals,
                    target(join, "p"),
                    &values,
                    Context::new(&*computation, &FormulaLimits::default(), counters, location)
                ),
                Err(FormulaFailure::TermAssignment {
                    error: AssignmentError::Read(ReadError::ForeignCatalog),
                    ..
                })
            ));
        },
    );
}

#[test]
fn domain_work_cutoffs_are_inclusive() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let values = binding(
                &[None, Some(Value::Number(2))],
                computation,
                counters,
                location,
            );
            let pattern = target(join, "p");
            let limits = FormulaLimits::default();
            let before = counters.accounting.work;
            let domains = join
                .projections
                .domains(
                    join.literals,
                    pattern,
                    &values,
                    Context::new(&*computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert_eq!(domains.borrowed().values.len(), 2);
            drop(domains);
            let required = counters.accounting.work - before;
            assert!(required > 0);
            let live = counters.workspace_bytes();
            for cutoff in 0..=required {
                let bounded = FormulaLimits {
                    max_work: counters.accounting.work + cutoff,
                    ..limits
                };
                let result = join.projections.domains(
                    join.literals,
                    pattern,
                    &values,
                    Context::new(&*computation, &bounded, counters, location),
                );
                if cutoff == required {
                    assert_eq!(result.unwrap().unwrap().borrowed().values.len(), 2);
                } else {
                    assert!(matches!(
                        result,
                        Err(FormulaFailure::Limit {
                            resource: FormulaResource::Work,
                            ..
                        })
                    ));
                }
                assert!(counters.accounting.work <= bounded.max_work);
                assert_eq!(counters.workspace_bytes(), live);
            }
        },
    );
}

#[test]
fn cancelled_domains_cannot_publish_a_restriction() {
    with_join(
        "p(1..6).q(2). :-p(X),q(Y),X/2=Y.",
        |join, computation, counters, location| {
            let values = binding(
                &[None, Some(Value::Number(2))],
                computation,
                counters,
                location,
            );
            let cancellation = zetesis_cpu::Cancellation::default();
            cancellation.cancel();
            counters.cancellation = Some(cancellation);
            assert!(matches!(
                join.projections.domains(
                    join.literals,
                    target(join, "p"),
                    &values,
                    Context::new(&*computation, &FormulaLimits::default(), counters, location)
                ),
                Err(FormulaFailure::Interrupted {
                    reason: zetesis_cpu::Stop::Cancelled,
                    ..
                })
            ));
        },
    );
}
