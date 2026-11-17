//! Result admission is tested after actual tasks and receipt settlement.

use std::num::NonZeroUsize;

use super::*;
use crate::FormulaResource;
use crate::formula_support::{SourceExecution, testing, with_source_workers};
use zetesis_algebra::{Cancellation, Stop};

#[test]
fn each_completed_result_is_charged_after_all_worker_receipts_settle() {
    const RESULTS: usize = 3;
    for workers in [1, 14] {
        for allowance in 0..=RESULTS {
            for cancelled in [false, true] {
                testing::with_completed_source(
                    "a.#minimize{1:a}.",
                    |program, _, computation, counters| {
                        let location = program.objectives[0].location;
                        let ordinary = FormulaLimits::default();
                        let mut budget = testing::budget();
                        let before = counters.accounting.work;
                        let (mut completed, tasks) =
                            with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
                                let execution = SourceExecution::new();
                                super::super::coordinator::run(
                                    execution.executor().expect("test source pool"),
                                    RESULTS,
                                    &mut budget,
                                    Context::new(&*computation, &ordinary, counters, location),
                                    |block, limits, _, counters| {
                                        counters.charge_work(
                                            block.len() as u128,
                                            limits,
                                            location,
                                        )?;
                                        Ok((block.start == 1, Evidence::default()))
                                    },
                                )
                                .unwrap()
                            });
                        assert_eq!(tasks, RESULTS, "the actual executor ran every fixed block");
                        assert_eq!(completed.results().count(), RESULTS);
                        assert_eq!(completed.results().filter(|(value, _)| *value).count(), 1);
                        let settled = counters.accounting.work;
                        assert!(settled >= before + u64::try_from(RESULTS).unwrap());
                        let permitted = u64::try_from(allowance).unwrap();
                        let limited = FormulaLimits {
                            max_work: settled + permitted,
                            ..ordinary
                        };
                        let cancellation = Cancellation::default();
                        if cancelled {
                            cancellation.cancel();
                            counters.attach_test_control(&cancellation);
                        }
                        let result = combine_results(&mut completed, &limited, counters, location);
                        if cancelled {
                            assert!(matches!(
                                result,
                                Err(FormulaFailure::Interrupted {
                                    reason: Stop::Cancelled,
                                    ..
                                })
                            ));
                            assert_eq!(counters.accounting.work, settled);
                        } else if allowance < RESULTS {
                            assert!(matches!(result, Err(FormulaFailure::Limit {
                            resource: FormulaResource::Work, location: failed, ..
                        }) if failed == location));
                            assert_eq!(counters.accounting.work, settled + permitted);
                        } else {
                            let (numeric, evidence) = result.unwrap();
                            assert!(numeric);
                            assert!(!evidence.finish());
                            assert_eq!(counters.accounting.work, settled + permitted);
                        }
                        counters.detach_test_control();
                        // Even the zero-allowance and cancelled reduction retains
                        // all already settled worker work; no template is returned.
                        assert!(counters.accounting.work >= settled);
                    },
                );
            }
        }
    }
}

fn each_descriptor(source: &str, expected_depth: usize, expected_rows: usize) {
    testing::with_completed_source(source, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::objective(
            Ir::Source(objective),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        cursor.evidence();
        let descriptor = ObjectiveRows::prepare(
            objective,
            &mut cursor,
            &mut budget,
            computation,
            &limits,
            counters,
        )
        .unwrap()
        .unwrap();
        assert_eq!(descriptor.partition_depth, expected_depth);
        assert_eq!(descriptor.rows.len(), expected_rows);
        drop(descriptor);
        assert_eq!(cursor.depth, 0);
        assert!(cursor.positions.iter().all(|position| *position == 0));
        for slot in 0..cursor.values.len() {
            assert!(!cursor.values.is_bound(slot, objective.location).unwrap());
        }
        let mut count = 0;
        while cursor
            .next_row(
                computation,
                &limits,
                &mut budget,
                counters,
                objective.location,
            )
            .unwrap()
            .is_some()
        {
            count += 1;
        }
        assert_eq!(count, if expected_rows == 0 { 0 } else { 32 });
    });
}

#[test]
fn bound_singletons_move_the_partition_without_consuming_the_original_cursor() {
    each_descriptor("d(1..32).gate(k).#minimize{X:d(X),gate(k)}.", 1, 32);
    each_descriptor(
        "d(1..32).gate(f(1)).ready.#minimize{X:d(X),gate(f(1)),ready}.",
        2,
        32,
    );
    each_descriptor("d(1..32).#minimize{X:d(X),gate(k)}.", 0, 0);
}

#[test]
fn an_unbound_singleton_generator_remains_the_original_partition() {
    // The singleton binds Y. Reusing an empty binding at the later d(X)
    // probe would need a separate captured-prefix correspondence.
    each_descriptor("seed(7).d(1..32).#minimize{X,Y:seed(Y),d(X)}.", 0, 1);
}

fn collect_rows<'source>(
    observed: &std::cell::RefCell<Vec<Vec<i32>>>,
    cursor: &mut Join<'_, 'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<(bool, Evidence), FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    while let Some(row) = cursor.next_row(computation, limits, budget, counters, location)? {
        assert!(row.passes);
        let mut values = Vec::new();
        for slot in 0..row.values.len() {
            let key = row.values.key(slot, location)?;
            let value = computation.read().term(&key).unwrap();
            let zetesis_core::ValueNodeRef::Number(number) = value.descriptor() else {
                panic!("the fixture binds only integers")
            };
            values.push(number);
        }
        observed.borrow_mut().push(values);
    }
    Ok((!observed.borrow().is_empty(), cursor.take_family()))
}

#[test]
fn ordered_later_probe_blocks_preserve_every_binding_after_constant_prefixes() {
    let source = "d(1..32).gate(k).ready.#minimize{X:d(X),gate(k),ready}.";
    testing::with_completed_source(source, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::objective(
            Ir::Source(objective),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        cursor.evidence();
        let descriptor = ObjectiveRows::prepare(
            objective,
            &mut cursor,
            &mut budget,
            computation,
            &limits,
            counters,
        )
        .unwrap()
        .unwrap();
        assert_eq!(descriptor.partition_depth, 2);
        assert_eq!(
            descriptor.plan.patterns[2].source, 0,
            "the original varying occurrence"
        );
        let observed = std::cell::RefCell::new(Vec::new());
        let append = computation.pending_reader().unwrap();
        for block in [0..3, 3..19, 19..32] {
            let (_, family) = descriptor
                .execute(
                    block,
                    append,
                    &limits,
                    &mut budget,
                    counters,
                    &|cursor, _, budget, context| collect_rows(&observed, cursor, budget, context),
                )
                .unwrap();
            assert!(!family.finish());
        }
        assert_eq!(
            *observed.borrow(),
            (1..=32).map(|value| vec![value]).collect::<Vec<_>>()
        );
        drop(descriptor);
        let reference = std::cell::RefCell::new(Vec::new());
        let (_, family) = collect_rows(
            &reference,
            &mut cursor,
            &mut budget,
            Context::new(computation, &limits, counters, objective.location),
        )
        .unwrap();
        assert!(!family.finish());
        assert_eq!(observed.into_inner(), reference.into_inner());
    });
}

#[test]
fn every_partition_preparation_work_refusal_restores_the_unconsumed_depth() {
    let source = "d(1..32).gate(k).ready.#minimize{X:d(X),gate(k),ready}.";
    let mut cost = 0;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::objective(
            Ir::Source(objective),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        cursor.evidence();
        let before = counters.accounting.work;
        assert_eq!(
            partition_probe(
                &mut cursor,
                &mut budget,
                computation,
                &limits,
                counters,
                objective.location
            )
            .unwrap(),
            2
        );
        cost = counters.accounting.work - before;
    });
    assert!(cost > 0);
    for allowed in 0..cost {
        testing::with_completed_source(source, |program, support, computation, counters| {
            let objective = &program.objectives[0];
            let ordinary = FormulaLimits::default();
            let mut budget = testing::budget();
            let mut cursor = Join::objective(
                Ir::Source(objective),
                support,
                computation,
                &ordinary,
                &mut budget,
                counters,
            )
            .unwrap();
            cursor.evidence();
            let before = counters.accounting.work;
            let limited = FormulaLimits {
                max_work: before + allowed,
                ..ordinary
            };
            let refused = partition_probe(
                &mut cursor,
                &mut budget,
                computation,
                &limited,
                counters,
                objective.location,
            );
            let Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                limit,
                observed,
                location,
            }) = refused
            else {
                panic!("the exact smaller allowance must refuse")
            };
            assert_eq!(limit, u128::from(before + allowed));
            assert_eq!(location, objective.location);
            assert!(observed > limit);
            // An indivisible existing index charge need not spend the residual
            // allowance. Keep its real performed prefix, never fill the ceiling.
            assert!(counters.accounting.work <= before + allowed);
            assert_eq!(cursor.depth, 0);
            assert!(cursor.positions.iter().all(|position| *position == 0));
            for slot in 0..cursor.values.len() {
                assert!(!cursor.values.is_bound(slot, objective.location).unwrap());
            }
            let mut rows = 0;
            while cursor
                .next_row(
                    computation,
                    &ordinary,
                    &mut budget,
                    counters,
                    objective.location,
                )
                .unwrap()
                .is_some()
            {
                rows += 1;
            }
            assert_eq!(rows, 32, "an explicit retry can use the original cursor");
        });
    }
}
