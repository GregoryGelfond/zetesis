//! Only eligibility owns a pool in the direct controls below. Reported task
//! counts therefore prove the post-lowering producer path, not another phase.

use std::num::NonZeroUsize;

use super::*;
use crate::formula_support::{
    Computation, Counters, SourceExecution, testing, with_source_workers,
};
use crate::grounding_observer::Work;
use crate::{FormulaLimits, FormulaResource, JoinStrategy};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Atom, ValueLimits};

const CHAIN: &str = "d(1..32).q(a;b;c;d).{blocked}.p(X,Y):-d(X),q(Y).s(X,Y):-p(X,Y),not blocked.#minimize{X@1,Y:s(X,Y)}.";
const CYCLE: &str = "d(1;2).{seed(X)}:-d(X).p(X):-d(X),q(X).q(X):-p(X).q(X):-seed(X).#minimize{X@1:p(X),not not p(X)}.";

#[derive(Debug)]
struct Report {
    tasks: usize,
    work: u64,
    substitutions: u64,
    atoms: Vec<(Atom, u8)>,
}

fn classifications(
    owner: &SourceEligibility,
    context: &mut crate::formula_source_activity::Context<'_, '_, '_>,
) -> Vec<(Atom, u8)> {
    assert!(
        owner.round.is_none(),
        "only a fully published certificate escapes"
    );
    (0..owner.atoms.len())
        .map(|position| {
            let atom = owner
                .atoms
                .atom(
                    position,
                    context.computation,
                    context.limits,
                    context.counters,
                    context.location,
                )
                .unwrap()
                .to_atom(ValueLimits::default())
                .unwrap();
            let activity = match owner.activity[position] {
                super::super::Activity::Absent => 0,
                super::super::Activity::Optional => 1,
                super::super::Activity::Required => 2,
            };
            (atom, activity)
        })
        .collect()
}

fn run(
    source: &str,
    strategy: JoinStrategy,
    workers: Option<usize>,
    allowance: Option<u64>,
    cancelled: bool,
) -> Result<Report, FormulaFailure> {
    super::super::retained::controlled(false, || {
        run_inner(source, strategy, workers, allowance, cancelled)
    })
    .0
}

fn ground_without_retention(source: &str, workers: Option<usize>) -> crate::formula::Compiled {
    super::super::retained::controlled(false, || {
        let prepare = || crate::formula_ground::ground(testing::prepare(source), None, None);
        if let Some(workers) = workers {
            with_source_workers(NonZeroUsize::new(workers).unwrap(), prepare)
                .0
                .unwrap()
        } else {
            prepare().unwrap()
        }
    })
    .0
}

fn run_inner(
    source: &str,
    strategy: JoinStrategy,
    workers: Option<usize>,
    allowance: Option<u64>,
    cancelled: bool,
) -> Result<Report, FormulaFailure> {
    let preparation = testing::prepare(source);
    let program = preparation.program;
    assert!(
        program
            .objectives
            .iter()
            .any(|objective| objective.needs_eligibility_query)
    );
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
    let mut computation = Computation::new(&mut append, queries.support());
    let presence = crate::formula_objective_dependencies::check_presence(
        &program,
        queries.support(),
        &mut computation,
        &preparation.limits,
        &mut budget,
        &mut counters,
    )?;
    let before = (counters.accounting.work, counters.accounting.substitutions);
    let limits = FormulaLimits {
        max_work: allowance.map_or(preparation.limits.max_work, |amount| before.0 + amount),
        ..preparation.limits
    };
    let cancellation = Cancellation::default();
    if cancelled {
        cancellation.cancel();
        counters = counters.with_cancellation(Some(&cancellation));
    }
    let mut context = crate::formula_source_activity::Context {
        computation: &mut computation,
        limits: &limits,
        budget: &mut budget,
        counters: &mut counters,
        location: preparation.location,
    };
    let mut build = |parallel: bool| {
        let execution = parallel.then(SourceExecution::new);
        let executor = execution.as_ref().map(|execution| {
            execution
                .executor()
                .expect("eligibility-only pool initialized")
        });
        SourceEligibility::build_from_conditions_with_executor(
            &program,
            &queries,
            presence.retained_entries(),
            program
                .objectives
                .iter()
                .filter(|objective| objective.needs_eligibility_query)
                .map(|objective| objective.condition.literals()),
            &mut context,
            executor,
        )
    };
    let (owner, tasks) = if let Some(workers) = workers {
        let (owner, tasks) =
            with_source_workers(NonZeroUsize::new(workers).unwrap(), || build(true));
        (owner?, tasks)
    } else {
        (build(false)?, 0)
    };
    let work = context.counters.accounting.work - before.0;
    let substitutions = context.counters.accounting.substitutions - before.1;
    Ok(Report {
        tasks,
        work,
        substitutions,
        atoms: classifications(&owner, &mut context),
    })
}

#[test]
fn completed_producer_rows_keep_activity_and_identity_order() {
    for source in [CHAIN, CYCLE] {
        let expected = run(source, JoinStrategy::Indexed, None, None, false).unwrap();
        let one = run(source, JoinStrategy::Indexed, Some(1), None, false).unwrap();
        let fourteen = run(source, JoinStrategy::Indexed, Some(14), None, false).unwrap();
        if source == CHAIN {
            assert!(
                one.tasks > 1,
                "the expanding producer must submit several tasks"
            );
        } else {
            assert_eq!(
                one.tasks, 0,
                "the original one-generator cycle remains serial"
            );
        }
        assert_eq!(one.tasks, fourteen.tasks);
        assert_eq!(one.work, fourteen.work);
        assert_eq!(one.substitutions, fourteen.substitutions);
        assert_eq!(one.atoms, expected.atoms);
        assert_eq!(fourteen.atoms, expected.atoms);
    }
}

#[test]
fn table_and_bound_choice_elements_remain_serial() {
    for (source, strategy) in [
        (CHAIN, JoinStrategy::Table),
        (
            "d(1..4).{p(X):d(X)}.#minimize{X@1:p(X),not not p(X)}.",
            JoinStrategy::Indexed,
        ),
        (
            "d(1..4).{p(X,Y):d(Y)}:-d(X).#minimize{X@1,Y:p(X,Y),not not p(X,Y)}.",
            JoinStrategy::Indexed,
        ),
    ] {
        let expected = run(source, strategy, None, None, false).unwrap();
        let actual = run(source, strategy, Some(14), None, false).unwrap();
        assert_eq!(
            actual.tasks, 0,
            "unsupported cursor must not be reinterpreted"
        );
        assert_eq!(actual.atoms, expected.atoms);
    }
}

#[test]
fn zero_output_producers_preserve_completed_absence() {
    let source = "d(1..4).q(9..12).p(X):-d(X),q(X).#minimize{X@1:d(X),not p(X)}.";
    let expected = run(source, JoinStrategy::Indexed, None, None, false).unwrap();
    for workers in [1, 14] {
        let actual = run(source, JoinStrategy::Indexed, Some(workers), None, false).unwrap();
        assert_eq!(actual.atoms, expected.atoms);
        assert!(
            actual
                .atoms
                .iter()
                .all(|(atom, _)| atom.predicate().name() != "p")
        );
    }
}

#[test]
fn producer_refusal_and_cancellation_never_return_a_certificate() {
    for workers in [1, 14] {
        let full = run(CHAIN, JoinStrategy::Indexed, Some(workers), None, false).unwrap();
        assert!(full.tasks > 1);
        for allowance in [0, full.work - 1] {
            let error = run(
                CHAIN,
                JoinStrategy::Indexed,
                Some(workers),
                Some(allowance),
                false,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                }
            ));
        }
        let error = run(CHAIN, JoinStrategy::Indexed, Some(workers), Some(0), true).unwrap_err();
        assert!(matches!(
            error,
            FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            }
        ));
        let retried = run(CHAIN, JoinStrategy::Indexed, Some(workers), None, false).unwrap();
        assert_eq!(retried.atoms, full.atoms);
    }
}

fn assert_program(reference: &crate::formula::Compiled, actual: &crate::formula::Compiled) {
    assert_eq!(
        crate::test_support::compiled_atoms(reference),
        crate::test_support::compiled_atoms(actual)
    );
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

#[test]
fn replay_preserves_original_program_provenance_and_scores() {
    let source =
        "d(1;2).{blocked}.p(X):-d(X).s(X):-p(X),not blocked.#minimize{X@1,k:s(X);1@1,k:p(X)}.";
    let reference = ground_without_retention(source, None);
    let actual = ground_without_retention(source, Some(14));
    assert_program(&reference, &actual);
    let count = reference.theory.atom_count();
    assert!(count <= 8);
    for mask in 0..(1_usize << count) {
        let mut records = Vec::new();
        for program in [&reference, &actual] {
            let positions = || (0..count).filter(|&atom| mask & (1 << atom) != 0);
            let interpretation =
                zetesis_ferraris::Interpretation::new(&program.theory, positions()).unwrap();
            let verdict = zetesis_ferraris::check(
                &program.theory,
                &interpretation,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            let model = zetesis_core::Model::from_positions(&program.atoms, positions()).unwrap();
            let score = zetesis_objective::evaluate(
                &program.objectives,
                &model,
                zetesis_objective::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            records.push((verdict.accepted(), score.score().costs().to_vec()));
        }
        assert_eq!(records[0], records[1], "interpretation {mask}");
    }
}

// The pool below is owned only by SourceEligibility. Support/presence have
// completed before it is created, and no objective-template executor is called.
const LEAF: &str = "d(1..8).q(1..8).{seed(X)}:-d(X).p(X,Y):-seed(X),q(Y),X<Y.#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.";

#[test]
fn leaf_producers_reach_the_real_eligibility_executor() {
    for source in [
        LEAF,
        "d(a;b;f(1)).q(a;b;f(1)).{seed(X)}:-d(X).p(X,Y):-seed(X),q(Y),X!=Y.#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.",
        "d(a;b;f(1)).q(a;b;f(1)).{seed(X)}:-d(X).p(X,Y):-seed(X),q(Y),X<Y.#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.",
        "d(1..4).e(1..3).{seed(X)}:-d(X).p(X,Y):-seed(X),e(Y),X!=Y.p(X,Y):-q(X,Y),d(X),e(Y),X!=Y.q(X,Y):-p(X,Y).#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.",
    ] {
        let reference = run(source, JoinStrategy::Indexed, None, None, false).unwrap();
        let one = run(source, JoinStrategy::Indexed, Some(1), None, false).unwrap();
        let many = run(source, JoinStrategy::Indexed, Some(14), None, false).unwrap();
        assert!(
            one.tasks > 1,
            "only the relevant leaf producer can dispatch: {source}"
        );
        assert_eq!(one.tasks, many.tasks);
        assert_eq!(one.work, many.work);
        assert_eq!(one.substitutions, many.substitutions);
        assert_eq!(one.atoms, reference.atoms);
        assert_eq!(many.atoms, reference.atoms);
    }
}

#[test]
fn leaf_false_rows_do_not_become_activity_contributors() {
    let source = "d(1..4).q(1..4).{seed(X)}:-d(X).p(X,Y):-seed(X),q(Y),X<Y,X>=Y.#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.";
    let reference = run(source, JoinStrategy::Indexed, None, None, false).unwrap();
    for workers in [1, 14] {
        let actual = run(source, JoinStrategy::Indexed, Some(workers), None, false).unwrap();
        assert!(actual.tasks > 1);
        assert_eq!(actual.atoms, reference.atoms);
        assert!(
            actual
                .atoms
                .iter()
                .all(|(atom, _)| atom.predicate().name() != "p")
        );
    }
}

#[test]
fn unsupported_producer_shapes_and_table_owners_keep_the_serial_route() {
    for (source, strategy) in [
        (LEAF, JoinStrategy::Table),
        (
            "d(1..4).{seed(X)}:-d(X).p(X):-seed(X),X<3.#minimize{1@0,X:p(X),not not p(X)}.",
            JoinStrategy::Indexed,
        ),
        (
            "d(1..4).q(1..4).{seed(X)}:-d(X).p(X,Y):-seed(X),q(Y),X<Y+1.#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.",
            JoinStrategy::Indexed,
        ),
        (
            "d(1..4).q(1..4).{seed(X)}:-d(X).p(X,Y):-seed(X),q(Y),not absent(X,Y).#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.",
            JoinStrategy::Indexed,
        ),
    ] {
        let reference = run(source, strategy, None, None, false).unwrap();
        let actual = run(source, strategy, Some(14), None, false).unwrap();
        assert_eq!(
            actual.tasks, 0,
            "no unsupported producer is reinterpreted: {source}"
        );
        assert_eq!(actual.atoms, reference.atoms);
    }
}

#[test]
fn leaf_producer_work_refusal_and_cancellation_publish_no_certificate() {
    for workers in [1, 14] {
        let full = run(LEAF, JoinStrategy::Indexed, Some(workers), None, false).unwrap();
        assert!(full.tasks > 1);
        for allowance in [0, full.work - 1] {
            assert!(matches!(
                run(
                    LEAF,
                    JoinStrategy::Indexed,
                    Some(workers),
                    Some(allowance),
                    false
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
        }
        assert!(matches!(
            run(LEAF, JoinStrategy::Indexed, Some(workers), Some(0), true),
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(
            run(LEAF, JoinStrategy::Indexed, Some(workers), None, false)
                .unwrap()
                .atoms,
            full.atoms
        );
    }
}

#[test]
fn leaf_replay_preserves_complete_theory_provenance_and_all_scored_subsets() {
    let source = "d(1;2).q(1;2).{seed(X)}:-d(X).p(X,Y):-seed(X),q(Y),X<Y.#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.";
    assert!(
        run(source, JoinStrategy::Indexed, Some(14), None, false)
            .unwrap()
            .tasks
            > 1
    );
    let reference = ground_without_retention(source, None);
    for workers in [1, 14] {
        let actual = ground_without_retention(source, Some(workers));
        assert_program(&reference, &actual);
        let count = reference.theory.atom_count();
        assert!(count <= 12);
        for mask in 0..(1_usize << count) {
            let mut records = Vec::new();
            for program in [&reference, &actual] {
                let positions = || (0..count).filter(|&atom| mask & (1 << atom) != 0);
                let interpretation =
                    zetesis_ferraris::Interpretation::new(&program.theory, positions()).unwrap();
                let verdict = zetesis_ferraris::check(
                    &program.theory,
                    &interpretation,
                    zetesis_ferraris::Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                let model =
                    zetesis_core::Model::from_positions(&program.atoms, positions()).unwrap();
                let score = zetesis_objective::evaluate(
                    &program.objectives,
                    &model,
                    zetesis_objective::Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
                records.push((verdict.accepted(), score.score().costs().to_vec()));
            }
            assert_eq!(records[0], records[1], "interpretation {mask}");
        }
    }
}

#[test]
fn combined_retention_precedes_workers_without_reusing_activity() {
    // Both generators can dispatch when retention is disabled. The required
    // first seed forces genuine later refinement and warm retained visits.
    let cyclic = "d(1;2).e(a;b).seed(1).{seed(2)}.p(X,Y):-d(X),e(Y),q(X),X<=2.q(X):-p(X,Y).q(X):-seed(X).#minimize{1@0,X,Y:p(X,Y),not not p(X,Y)}.";
    let ordinary = run(cyclic, JoinStrategy::Indexed, None, None, false).unwrap();
    assert!(
        run(cyclic, JoinStrategy::Indexed, Some(14), None, false)
            .unwrap()
            .tasks
            > 1
    );
    for workers in [1, 14] {
        let (actual, hits, captures) = super::super::retained::controlled(true, || {
            run_inner(cyclic, JoinStrategy::Indexed, Some(workers), None, false)
        });
        let actual = actual.unwrap();
        assert!(
            captures > 0 && hits > 0,
            "completed rows are really captured and revisited"
        );
        assert_eq!(
            actual.tasks, 0,
            "serial retention capture precedes leaf worker dispatch"
        );
        assert_eq!(actual.atoms, ordinary.atoms);
        assert!(
            actual
                .atoms
                .iter()
                .any(|(atom, state)| atom.predicate().name() == "p" && *state == 2)
        );
        assert!(
            actual
                .atoms
                .iter()
                .any(|(atom, state)| atom.predicate().name() == "p" && *state == 1)
        );

        let expected = run(CHAIN, JoinStrategy::Indexed, None, None, false).unwrap();
        let (actual, hits, captures) = super::super::retained::controlled(true, || {
            run_inner(CHAIN, JoinStrategy::Indexed, Some(workers), None, false)
        });
        let actual = actual.unwrap();
        assert_eq!(
            (captures, hits),
            (0, 0),
            "the acyclic producer has no retention owner"
        );
        assert!(
            actual.tasks > 1,
            "the eligible acyclic producer still reaches the worker path"
        );
        assert_eq!(actual.atoms, expected.atoms);
    }
}
