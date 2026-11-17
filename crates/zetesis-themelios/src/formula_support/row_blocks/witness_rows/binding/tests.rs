use std::num::NonZeroUsize;

use super::*;
use crate::formula_support::row_blocks::witness_rows::Replay;
use crate::formula_support::row_blocks::{coordinator, cursor_rows::CursorRows};
use crate::formula_support::{Ir, Join, SourceExecution, testing, with_source_workers};
use crate::{FormulaLimits, FormulaResource};
use zetesis_core::{Value, ValueLimits};

fn values(
    binding: &Binding<'_>,
    computation: &Computation<'_, '_>,
    location: crate::ProgramSite,
) -> Vec<Value> {
    (0..binding.len())
        .map(|slot| {
            computation
                .read()
                .term(&binding.key(slot, location).unwrap())
                .unwrap()
                .to_value(ValueLimits::default())
                .unwrap()
        })
        .collect()
}

fn compare(source: &str, workers: usize, inspect_first: bool) -> usize {
    let mut submitted = 0;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let location = rule.location;
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::rule(
            Ir::Source(rule),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        let (actual, tasks) = with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
            let execution = SourceExecution::new();
            let rows =
                CursorRows::prepare_producer(rule, &mut cursor, computation, &limits, counters)
                    .unwrap()
                    .unwrap();
            let plan = WitnessPlan::for_emission(
                rows.descriptor(),
                support,
                computation,
                &limits,
                counters,
            )
            .unwrap();
            let mut completed = coordinator::stage_witnesses(
                execution.executor().unwrap(),
                &plan,
                &mut budget,
                Context::new(&*computation, &limits, counters, location),
            )
            .unwrap();
            completed
                .attach_reservation(Context::new(&*computation, &limits, counters, location))
                .unwrap();
            let mut replay = Replay::new(
                &plan,
                support,
                Context::new(&*computation, &limits, counters, location),
            )
            .unwrap();
            let mut binding = None;
            let mut actual = Vec::new();
            for batch in completed.results() {
                let mut selected = replay.batch(batch, &limits, counters).unwrap();
                while let Some(row) = selected
                    .next(Context::new(&*computation, &limits, counters, location))
                    .unwrap()
                {
                    if binding.is_none() {
                        binding = Some(initial_binding(
                            &plan,
                            &row,
                            rows.descriptor(),
                            support,
                            Context::new(&*computation, &limits, counters, location),
                            inspect_first,
                        ));
                    }
                    let binding = binding
                        .as_mut()
                        .unwrap()
                        .read(
                            &row,
                            Context::new(&*computation, &limits, counters, location),
                        )
                        .unwrap();
                    actual.push(values(binding, computation, location));
                }
            }
            actual
        });
        let mut expected = Vec::new();
        while let Some(binding) = cursor
            .next(computation, &limits, &mut budget, counters, location)
            .unwrap()
        {
            expected.push(values(&binding, computation, location));
        }
        assert!(
            !cursor.take_family().finish(),
            "the complete flat-positive family is defined"
        );
        assert_eq!(
            actual, expected,
            "all bindings retain exact serial order and typed values"
        );
        submitted = tasks;
    });
    submitted
}

fn initial_binding<'plan, 'source: 'plan>(
    plan: &'plan WitnessPlan<'plan, 'source>,
    row: &WitnessRow<'_, 'source>,
    descriptor: super::super::super::cursor_rows::Descriptor<'plan, 'source>,
    support: &crate::formula_support::Support<'source>,
    context: Context<'_, &Computation<'_, '_>>,
    inspect_first: bool,
) -> RowBinding<'plan, 'source> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    if inspect_first {
        inspect(
            plan,
            row,
            descriptor,
            support,
            computation,
            limits,
            counters,
        );
    }
    RowBinding::new(
        plan,
        row,
        Context::new(computation, limits, counters, location),
    )
    .unwrap()
}

fn inspect<'plan, 'source: 'plan>(
    plan: &'plan WitnessPlan<'plan, 'source>,
    row: &WitnessRow<'_, 'source>,
    descriptor: super::super::super::cursor_rows::Descriptor<'plan, 'source>,
    support: &crate::formula_support::Support<'source>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut super::super::Counters,
) {
    let location = plan.location();
    let other =
        WitnessPlan::for_emission(descriptor, support, computation, limits, counters).unwrap();
    assert!(matches!(
        RowBinding::new(
            &other,
            row,
            Context::new(computation, limits, counters, location)
        ),
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
    let stopped = FormulaLimits {
        max_work: counters.accounting.work,
        ..*limits
    };
    assert!(matches!(
        RowBinding::new(
            plan,
            row,
            Context::new(computation, &stopped, counters, location)
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    let limited = FormulaLimits {
        max_support_bytes: 0,
        ..*limits
    };
    assert!(matches!(
        RowBinding::new(
            plan,
            row,
            Context::new(computation, &limited, counters, location)
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            limit: 0,
            ..
        })
    ));
}

#[test]
fn producer_witness_replay_preserves_every_typed_binding_and_order() {
    for workers in [1, 14] {
        let tasks = compare("d(1..32).e(1..32).out(X,Y):-d(X),e(Y),X<Y.", workers, true);
        assert_eq!(
            tasks, 14,
            "the actual selected source probe has fourteen blocks"
        );
        assert!(
            compare(
                "p(1,1).p(2,2).p(2,3).q(a;f(3)).out(X,Y):-p(X,X),q(Y).",
                workers,
                false
            ) > 0
        );
    }
}

#[test]
fn producer_witness_replay_preserves_empty_partition_outputs() {
    for workers in [1, 14] {
        assert!(compare("d(1..4).q(9..12).out(X,Y):-d(X),q(Y),X>Y.", workers, false) > 0);
    }
}
