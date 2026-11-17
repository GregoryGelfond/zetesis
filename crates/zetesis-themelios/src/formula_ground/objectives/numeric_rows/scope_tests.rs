//! The actual worker scan owns one absence certificate per logical block.

use std::num::NonZeroUsize;

use super::*;
use crate::FormulaLimits;
use crate::formula_ground::arithmetic::reference_scopes;
use crate::formula_support::{Ir, SourceExecution, testing, with_source_workers};

#[derive(Debug, PartialEq, Eq)]
struct Receipt {
    numeric: bool,
    zero: bool,
    work: u64,
    substitutions: u64,
    tasks: usize,
}

fn parallel(source: &str, workers: usize, reference: bool, literals: usize) -> Receipt {
    let mut receipt = None;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        assert_eq!(objective.condition.literals().len(), literals);
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut bindings = Join::objective(
            Ir::Source(objective),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        bindings.evidence();
        let before = (counters.accounting.work, counters.accounting.substitutions);
        let (result, tasks) = with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
            let execution = SourceExecution::new();
            crate::formula_support::row_blocks::parallel_numeric_rows(
                execution.executor().expect("test source pool"),
                objective,
                &mut bindings,
                &mut budget,
                Context::new(&mut *computation, &limits, counters, objective.location),
                |rows, support, budget, context| {
                    // The control must run inside each actual task. A caller's
                    // thread-local flag does not configure a Rayon worker.
                    reference_scopes(reference, || {
                        scan(
                            objective,
                            rows,
                            support,
                            budget,
                            &mut Warnings::default(),
                            context,
                        )
                    })
                },
            )
        });
        let (numeric, evidence) = result.unwrap().expect("eligible nonempty first probe");
        receipt = Some(Receipt {
            numeric,
            zero: evidence.finish(),
            work: counters.accounting.work - before.0,
            substitutions: counters.accounting.substitutions - before.1,
            tasks,
        });
    });
    receipt.unwrap()
}

#[test]
fn actual_blocks_charge_only_their_first_passing_absence_scan() {
    // K source literals, R passing completions, Q blocks with a completion.
    // All cases have the same ordinary first generator under the planner.
    for (source, literals, rows, nonempty, tasks) in [
        (
            "d(1..32).e(1..40).#minimize{1@0,X,Y:d(X),e(Y)}.",
            2,
            1280,
            14,
            14,
        ),
        ("d(1..3).e(1..5).#minimize{1@0,X,Y:d(X),e(Y)}.", 2, 15, 3, 3),
        (
            "d(1..16).q(2;4;6;8;10;12;14;16;100..116).e(1..32).#minimize{1@0,X,Y:d(X),q(X),e(Y)}.",
            3,
            256,
            8,
            14,
        ),
        (
            "d(1..4).q(5..12).e(1..16).#minimize{1@0,X,Y:d(X),q(X),e(Y)}.",
            3,
            0,
            0,
            4,
        ),
    ] {
        let mut single = None;
        for workers in [1, 14] {
            let reference = parallel(source, workers, true, literals);
            let cached = parallel(source, workers, false, literals);
            assert_eq!(reference.tasks, tasks, "reference: {source}");
            assert_eq!(cached.tasks, tasks, "cached: {source}");
            assert_eq!(
                reference.substitutions, rows,
                "complete row visits: {source}"
            );
            assert_eq!(
                cached.substitutions, rows,
                "complete cached visits: {source}"
            );
            assert_eq!((cached.numeric, cached.zero), (rows != 0, false));
            assert_eq!(
                (cached.numeric, cached.zero),
                (reference.numeric, reference.zero)
            );
            let removed = u64::try_from(literals).unwrap() * (rows - nonempty);
            assert_eq!(
                reference.work - cached.work,
                removed,
                "K × (R − Q): {source}"
            );
            if let Some(first) = &single {
                assert_eq!(&cached, first, "fixed logical geometry: {source}");
            } else {
                single = Some(cached);
            }
        }
    }
}

#[test]
fn combined_workers_preserve_lifted_and_general_objective_programs() {
    use crate::formula_ground::ground;
    use crate::test_support::compiled_atoms;

    for source in [
        "d(1..3).e(1..4).#minimize{X@0,X,Y:d(X),e(Y)}.",
        "d(1..3).e(1..4).#minimize{X+1@0,X,Y:d(X),e(Y);1@1,X:d(X)}.",
        "{a;b}.#minimize{2@1,k:a;2@1,k:b}.",
        "d(1;2).{p(1);p(2)}.:~d(K),N=#count{X:p(X),X=K}.[N@K,K]",
    ] {
        // No executor is enabled on this original complete-scan reference.
        let reference =
            reference_scopes(true, || ground(testing::prepare(source), None, None)).unwrap();
        for workers in [1, 14] {
            let (actual, _) = with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
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
        }
    }
}
