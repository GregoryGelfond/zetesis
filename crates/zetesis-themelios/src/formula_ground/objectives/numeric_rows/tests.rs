//! The proposed worker route and its original serial cursor agree, including
//! unsupported shapes which must remain untouched for ordinary traversal.

use std::num::NonZeroUsize;

use super::*;
use crate::formula_support::{Counters, Ir, SourceExecution, testing, with_source_workers};
use crate::grounding_observer::Work;
use crate::{FormulaLimits, FormulaResource, JoinStrategy};
use zetesis_algebra::{Cancellation, Stop};

#[derive(Debug, PartialEq, Eq)]
struct Report {
    offered: bool,
    numeric: bool,
    zero: bool,
    warnings: usize,
    tasks: usize,
}

fn run(
    source: &str,
    strategy: JoinStrategy,
    workers: Option<usize>,
    remaining_work: Option<u64>,
    cancelled: bool,
) -> Result<Report, FormulaFailure> {
    let preparation = testing::prepare(source);
    let program = preparation.program;
    let mut budget = preparation.budget;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = crate::formula_support::build(
        preparation.catalog,
        &program,
        None,
        &preparation.limits,
        &mut budget,
        &mut counters,
        preparation.location,
    )?;
    let (completed, mut append) =
        catalog.split(&preparation.limits, &mut counters, preparation.location)?;
    let queries = completed.queries(
        strategy,
        &preparation.limits,
        &counters,
        preparation.location,
    )?;
    let support = queries.support();
    let mut computation = Computation::new(&mut append, support);
    let objective = &program.objectives[0];
    let mut bindings = Join::objective(
        Ir::Source(objective),
        support,
        &computation,
        &preparation.limits,
        &mut budget,
        &mut counters,
    )?;
    bindings.evidence();
    let limits = FormulaLimits {
        max_work: remaining_work.map_or(preparation.limits.max_work, |remaining| {
            counters.accounting.work + remaining
        }),
        ..preparation.limits
    };
    let cancellation = Cancellation::default();
    if cancelled {
        cancellation.cancel();
        counters = counters.with_cancellation(Some(&cancellation));
    }
    let mut warnings = Warnings::default();
    let (offered, tasks) = if let Some(workers) = workers {
        let (result, tasks) = with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
            let execution = SourceExecution::new();
            try_parallel(
                execution.executor().expect("test pool initialized"),
                objective,
                &mut bindings,
                &mut budget,
                Context::new(&mut computation, &limits, &mut counters, objective.location),
            )
        });
        (result?, tasks)
    } else {
        (None, 0)
    };
    let parallel = offered.is_some();
    let (numeric, evidence) = match offered {
        Some(result) => result,
        None => scan(
            objective,
            &mut bindings,
            support,
            &mut budget,
            &mut warnings,
            Context::new(&mut computation, &limits, &mut counters, objective.location),
        )?,
    };
    Ok(Report {
        offered: parallel,
        numeric,
        zero: evidence.finish(),
        warnings: warnings.into_values().len(),
        tasks,
    })
}

fn same_numeric(source: &str, expected_parallel: bool) {
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let reference = run(source, strategy, None, None, false).unwrap();
        for workers in [1, 14] {
            let actual = run(source, strategy, Some(workers), None, false).unwrap();
            assert_eq!(
                actual.offered,
                expected_parallel && strategy == JoinStrategy::Indexed,
                "{source}"
            );
            assert_eq!(
                actual.tasks > 0,
                actual.offered,
                "objective-only task admission: {source}"
            );
            assert_eq!(
                (actual.numeric, actual.zero, actual.warnings),
                (reference.numeric, reference.zero, reference.warnings),
                "{source}"
            );
        }
    }
}

#[test]
fn constant_and_as_written_variable_rows_use_the_same_complete_scan() {
    for source in [
        "d(1..32).e(1..4).#minimize{2@0,X,Y:d(X),e(Y)}.",
        "d(1..32).e(1..4).#minimize{X@0,X,Y:d(X),e(Y)}.",
        "d(1..32).e(1..4).#maximize{2@0,X,Y:d(X),e(Y)}.",
    ] {
        same_numeric(source, true);
    }
}

#[test]
fn simple_nonnumeric_variable_values_are_not_numeric_or_arithmetic_failures() {
    for source in [
        "d(foo;f(1);\"text\").#minimize{X,X:d(X)}.",
        "d(foo;f(1);\"text\";7).#minimize{X,X:d(X)}.",
    ] {
        same_numeric(source, true);
    }
}

#[test]
fn as_written_minimum_weight_is_total() {
    same_numeric("d((-2147483647-1)).#minimize{X:d(X)}.", true);
}

#[test]
fn repeated_slots_source_occurrences_and_nullary_completions_survive_partitioning() {
    for source in [
        "d(1..16).edge(1,1;1,2;2,2).#minimize{1@0,X,Y:d(X),edge(Y,Y),d(X)}.",
        "a.b.#minimize{1:a,b}.",
        "p(f(1)).#minimize{1:p(f(1))}.",
    ] {
        same_numeric(source, true);
    }
}

#[test]
fn empty_join_and_unsupported_shapes_leave_the_original_cursor_available() {
    for source in [
        "#minimize{1,X:d(X)}.",
        "d(1;2).#minimize{1,X:d(X),X<2}.",
        "d(1;2).{q(1);q(2)}.#minimize{1,X:d(X),not q(X)}.",
        "d(1;2).#minimize{X+1,X:d(X)}.",
        "d(1;2).#maximize{X,X:d(X)}.",
        "{a;b}.:~#count{1:a;2:b}>0.[1]",
    ] {
        same_numeric(source, false);
    }
}

#[test]
fn normalization_failure_keeps_empty_family_and_original_error_distinct() {
    let empty = "#maximize{(-2147483647-1),X:d(X)}.";
    same_numeric(empty, false);
    for source in [
        "d((-2147483647-1)).#maximize{X:d(X)}.",
        "d(1).#maximize{(-2147483647-1),X:d(X)}.",
    ] {
        let reference = run(source, JoinStrategy::Indexed, None, None, false).unwrap_err();
        for workers in [1, 14] {
            let actual =
                run(source, JoinStrategy::Indexed, Some(workers), None, false).unwrap_err();
            assert_eq!(format!("{reference:?}"), format!("{actual:?}"));
        }
    }
}

#[test]
fn entry_refusal_and_cancellation_remain_typed_not_serial_retries() {
    let source = "d(1..32).#minimize{X:d(X)}.";
    for workers in [1, 14] {
        let stopped =
            run(source, JoinStrategy::Indexed, Some(workers), Some(0), false).unwrap_err();
        assert!(matches!(
            stopped,
            FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            }
        ));
        let cancelled =
            run(source, JoinStrategy::Indexed, Some(workers), Some(0), true).unwrap_err();
        assert!(matches!(
            cancelled,
            FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            }
        ));
    }
}

#[test]
fn full_objective_preparation_keeps_one_template_origins_and_theory() {
    use crate::formula_ground::ground;
    use crate::test_support::compiled_atoms;
    for source in [
        "d(1..8).e(1..3).#minimize{1@0,X,Y:d(X),e(Y)}.",
        "d(1..32).selected_intent(minimize_use).#minimize{Total:d(Total),selected_intent(minimize_use)}.",
        "d(foo;2).#minimize{X@0,X:d(X)}.",
        "d(foo;f(1);\"text\").#minimize{X:d(X)}.",
        "#minimize{X:d(X)}.",
    ] {
        let reference = ground(testing::prepare(source), None, None).unwrap();
        for workers in [1, 14] {
            let (actual, tasks) = with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
                ground(testing::prepare(source), None, None)
            });
            let actual = actual.unwrap();
            assert_eq!(compiled_atoms(&reference), compiled_atoms(&actual));
            assert_eq!(reference.theory.nodes(), actual.theory.nodes());
            assert_eq!(reference.theory.operands(), actual.theory.operands());
            assert_eq!(reference.theory.roots(), actual.theory.roots());
            assert_eq!(reference.origins, actual.origins);
            assert_eq!(reference.warnings, actual.warnings);
            assert!(
                reference
                    .objectives
                    .templates()
                    .iter()
                    .eq(actual.objectives.templates().iter())
            );
            assert_eq!(
                reference.objectives.priorities(),
                actual.objectives.priorities()
            );
            assert_eq!(reference.objective_origins, actual.objective_origins);
            assert_eq!(
                reference.objective_declarations,
                actual.objective_declarations
            );
            assert_eq!(
                actual.objectives.templates().len(),
                usize::from(source.starts_with("d(1") || source.starts_with("d(foo;2)"))
            );
            if source.starts_with("d(1") || source.starts_with("d(foo;2)") {
                assert!(
                    tasks > 0,
                    "facts-only source: observed tasks belong to objectives"
                );
            }
        }
    }
}

#[test]
fn matched_physical_worker_counts_keep_exact_objective_receipts() {
    let source = "d(1..32).e(1..4).#minimize{X@0,X,Y:d(X),e(Y)}.";
    let capture = |workers| {
        with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
            crate::formula_ground::ground_retained(testing::prepare(source), None, false).unwrap()
        })
    };
    let (single, single_tasks) = capture(1);
    let (parallel, parallel_tasks) = capture(14);
    assert!(single_tasks > 0);
    assert_eq!(single_tasks, parallel_tasks);
    assert_eq!(single.accounting.work, parallel.accounting.work);
    assert_eq!(
        single.accounting.substitutions,
        parallel.accounting.substitutions
    );
    assert_eq!(single.compiled.expansion, parallel.compiled.expansion);
    assert!(
        single.compiled.objectives.templates().iter().eq(parallel
            .compiled
            .objectives
            .templates()
            .iter())
    );
}

#[test]
fn a_constant_prefix_before_a_large_relation_uses_multiple_actual_objective_tasks() {
    let source = "d(1..32).selected_intent(minimize_use).#minimize{Total: d(Total),selected_intent(minimize_use)}.";
    let expected = run(source, JoinStrategy::Indexed, None, None, false).unwrap();
    for workers in [1, 14] {
        let actual = run(source, JoinStrategy::Indexed, Some(workers), None, false).unwrap();
        assert!(actual.offered);
        assert_eq!(
            actual.tasks, 14,
            "fixed logical blocks span the later 32-row relation"
        );
        assert_eq!(
            (actual.numeric, actual.zero, actual.warnings),
            (expected.numeric, expected.zero, expected.warnings)
        );
        let table = run(source, JoinStrategy::Table, Some(workers), None, false).unwrap();
        assert!(!table.offered);
        assert_eq!(table.tasks, 0);
        assert_eq!(
            (table.numeric, table.zero, table.warnings),
            (expected.numeric, expected.zero, expected.warnings)
        );
    }
}
