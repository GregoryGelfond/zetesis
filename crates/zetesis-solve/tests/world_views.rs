//! Complete original families require unrestricted coverage and retained members.

use std::{collections::BTreeSet, convert::Infallible, io, num::NonZeroUsize, sync::Arc};

use zetesis_core::{GroundProgram, StaticLimits};
use zetesis_cpu::Control;
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, Grounder, Interruption, Oracle, PreparedInput, Session, SolveConfig,
    SolveError, Subject, WorldView, WorldViewError, WorldViewFailure, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, Admitted, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_extended,
    admit_formula,
};

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        workers: NonZeroUsize::MIN,
        ..Default::default()
    }
}

fn normal(source: &str) -> Admitted {
    admit_extended(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap()
}

fn formula(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn names(answer: &AnswerSet) -> Vec<String> {
    // These fixtures use positive nullary atoms, so names retain full identity.
    answer
        .interpretation()
        .atoms()
        .iter()
        .map(|atom| atom.predicate().name().to_owned())
        .collect()
}

fn family(world_view: &WorldView) -> BTreeSet<Vec<String>> {
    world_view.answer_sets().iter().map(names).collect()
}

fn collect(input: PreparedInput<'_>) -> WorldView {
    WorldView::collect(
        input,
        config(),
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap()
}

#[test]
fn builder_collection_overrides_optimal_selection() {
    let owner = formula("1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}.");
    let resources = ExecutionResources::default();
    let world_view = Session::builder(PreparedInput::formula(&owner), config(), Control::default())
        .selection(AnswerSelection::Optimal)
        .resources(&resources)
        .collect(WorldViewLimits::default())
        .unwrap();
    let answers: BTreeSet<_> = world_view
        .answer_sets()
        .iter()
        .map(|answer| (names(answer), answer.score().unwrap().costs().to_vec()))
        .collect();
    assert_eq!(
        answers,
        BTreeSet::from([
            (vec!["a".into()], vec![(2, 1)]),
            (vec!["b".into()], vec![(2, 2)]),
        ])
    );
    assert_eq!(world_view.outcome().selection(), Some(AnswerSelection::All));
    assert_eq!(
        world_view.outcome().completion(),
        Some(Completion::Exhausted)
    );
    assert_eq!(world_view.outcome().retained_models(), 0);
    assert!(world_view.outcome().incumbent().is_none());
}

#[test]
fn unrestricted_enumeration_keeps_nonoptimal_answers() {
    let owner = formula("1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}.");
    let mut session =
        Session::enumerate(PreparedInput::formula(&owner), config(), Control::default()).unwrap();
    let answers: BTreeSet<_> = session
        .by_ref()
        .map(|result| {
            let answer = result.unwrap();
            (names(&answer), answer.score().unwrap().costs().to_vec())
        })
        .collect();
    assert_eq!(
        answers,
        BTreeSet::from([
            (vec!["a".into()], vec![(2, 1)]),
            (vec!["b".into()], vec![(2, 2)]),
        ])
    );
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(
        outcome
            .countermodel_statistics()
            .unwrap()
            .candidate_restrictions,
        0
    );
    assert!(outcome.incumbent().is_none());
    assert!(!outcome.optimum_proved());
}

#[test]
fn unrestricted_enumeration_does_not_retain_incumbents() {
    let owner = formula("{a;b}. #minimize {0,k:a}.");
    let configured = SolveConfig {
        max_optimal_models: 0,
        max_optimal_atoms: 0,
        max_optimal_bytes: 0,
        ..config()
    };
    let world_view = WorldView::collect(
        PreparedInput::formula(&owner),
        configured,
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap();
    assert_eq!(
        family(&world_view),
        BTreeSet::from([
            vec![],
            vec!["a".into()],
            vec!["b".into()],
            vec!["a".into(), "b".into()],
        ])
    );
    assert_eq!(world_view.outcome().retained_models(), 0);
    assert_eq!(world_view.outcome().scored_models(), 4);
}

#[test]
fn selected_optimum_is_distinct_from_original_family() {
    let owner = formula("1 {a;b} 1. #minimize {1,a:a; 2,b:b}.");
    let mut selected =
        Session::new(PreparedInput::formula(&owner), config(), Control::default()).unwrap();
    let answers: Vec<_> = selected
        .by_ref()
        .map(|result| names(&result.unwrap()))
        .collect();
    assert_eq!(answers, [vec!["a".to_owned()]]);
    let outcome = selected.outcome().unwrap();
    assert_eq!(outcome.selection(), Some(AnswerSelection::Optimal));
    assert!(outcome.optimum_proved());
    assert_eq!(collect(PreparedInput::formula(&owner)).len(), 2);
}

#[test]
fn capped_optimal_ties_do_not_describe_every_answer() {
    let owner = formula("a. {b}. #minimize {0,k:a}.");
    let mut selected = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            models: 1,
            ..config()
        },
        Control::default(),
    )
    .unwrap();
    assert_eq!(selected.by_ref().count(), 1);
    let outcome = selected.outcome().unwrap();
    assert!(outcome.optimum_proved());
    assert_eq!(outcome.incumbent().unwrap().tied_models, 2);
    assert_eq!(outcome.retained_models(), 1);
    assert_eq!(collect(PreparedInput::formula(&owner)).len(), 2);
}

#[test]
fn objective_absence_differs_from_an_active_zero_score() {
    let absent = formula("a.");
    let active = formula("a. #minimize {0@3,k:a}.");
    let absent = collect(PreparedInput::formula(&absent));
    let active = collect(PreparedInput::formula(&active));
    assert_eq!(family(&absent), family(&active));
    assert!(absent.answer_sets()[0].score().is_none());
    let score = active.answer_sets()[0].score().unwrap();
    assert!(score.is_present());
    assert_eq!(score.costs(), [(3, 0)]);
}

#[test]
fn full_identity_survives_identical_empty_displays() {
    let owner = normal("{a}. {b}. #show.");
    let world_view = collect(PreparedInput::admitted(&owner));
    assert_eq!(
        family(&world_view),
        BTreeSet::from([
            vec![],
            vec!["a".into()],
            vec!["b".into()],
            vec!["a".into(), "b".into()],
        ])
    );
    assert_eq!(world_view.len(), 4);
    assert_eq!(world_view.outcome().verified_models(), 4);
    for answer in world_view.answer_sets() {
        let display = owner
            .metadata()
            .observations()
            .render(
                answer.interpretation(),
                owner.metadata().output(),
                zetesis_themelios::observation::Limits::default(),
                &Control::default(),
            )
            .unwrap();
        assert!(display.text().is_empty());
    }
}

#[test]
fn empty_program_has_one_empty_answer() {
    let relational = normal("");
    let formulas = formula("");
    for input in [
        PreparedInput::admitted(&relational),
        PreparedInput::formula(&formulas),
    ] {
        let world_view = collect(input);
        assert_eq!(family(&world_view), BTreeSet::from([vec![]]));
        assert_eq!(world_view.len(), 1);
        assert!(!world_view.is_empty());
        assert!(!world_view.outcome().unsatisfiable());
    }
}

#[test]
fn inconsistent_program_has_an_empty_complete_family() {
    let relational = normal(":-.");
    let formulas = formula(":-.");
    for input in [
        PreparedInput::admitted(&relational),
        PreparedInput::formula(&formulas),
    ] {
        let world_view = WorldView::collect(
            input,
            config(),
            WorldViewLimits {
                max_answer_sets: 0,
                max_atoms: 0,
                max_bytes: 0,
            },
            Control::default(),
        )
        .unwrap();
        assert!(world_view.is_empty());
        assert!(world_view.outcome().unsatisfiable());
        assert_eq!(
            world_view.outcome().completion(),
            Some(Completion::Exhausted)
        );
    }
}

#[test]
fn detached_answer_retains_original_instance() {
    let owner = normal("a.");
    let other = normal("a.");
    let answer = collect(PreparedInput::admitted(&owner))
        .into_answer_sets()
        .pop()
        .unwrap();
    assert!(
        answer
            .subject()
            .same_instance(&Subject::Program(owner.program().clone()))
    );
    assert!(
        !answer
            .subject()
            .same_instance(&Subject::Program(other.program().clone()))
    );
    assert_eq!(names(&answer), ["a"]);
}

#[test]
fn world_view_keeps_its_subject_after_owner_drop() {
    let world_view = {
        let owner = formula("a | b.");
        collect(PreparedInput::formula(&owner))
    };
    let other = formula("a | b.");
    assert!(
        !world_view
            .subject()
            .same_instance(&Subject::Theory(other.theory().clone()))
    );
    for answer in world_view.answer_sets() {
        assert!(answer.subject().same_instance(world_view.subject()));
    }
    assert!(
        world_view
            .outcome()
            .subject()
            .unwrap()
            .same_instance(world_view.subject())
    );
    assert_eq!(
        family(&world_view),
        BTreeSet::from([vec!["a".into()], vec!["b".into()]])
    );
}

#[test]
fn distinct_semantic_profiles_do_not_share_instance() {
    let relational = normal("a.");
    let formulas = formula("a.");
    assert!(
        !Subject::Program(relational.program().clone())
            .same_instance(&Subject::Theory(formulas.theory().clone()))
    );
}

#[test]
fn exact_prepared_routes_enumerate_the_same_family() {
    let source = "a :- not b. b :- not a. {hidden}.";
    let relational = normal(source);
    let formulas = formula(source);
    let graph =
        Arc::new(GroundProgram::compile(relational.program(), StaticLimits::default()).unwrap());
    let expected = BTreeSet::from([
        vec!["a".into()],
        vec!["a".into(), "hidden".into()],
        vec!["b".into()],
        vec!["b".into(), "hidden".into()],
    ]);
    for (input, oracle, grounder) in [
        (
            PreparedInput::admitted(&relational),
            Oracle::Closure,
            Grounder::Lazy,
        ),
        (
            PreparedInput::ground(&graph),
            Oracle::Closure,
            Grounder::Eager,
        ),
        (
            PreparedInput::formula(&formulas),
            Oracle::Countermodel,
            Grounder::Eager,
        ),
    ] {
        let configured = SolveConfig {
            oracle,
            grounder,
            ..config()
        };
        let mut routes = Routes::default();
        let world_view = Session::builder(input, configured, Control::default())
            .resources(&ExecutionResources::default())
            .collect_observed(WorldViewLimits::default(), &mut routes)
            .unwrap();
        assert_eq!(family(&world_view), expected);
        if oracle == Oracle::Countermodel {
            assert_eq!(routes.formulas, 1);
            assert!(routes.closures.is_empty());
        } else {
            assert_eq!(routes.closures, [grounder]);
            assert_eq!(routes.formulas, 0);
        }
        assert_eq!(
            world_view.outcome().countermodel_statistics().is_some(),
            oracle == Oracle::Countermodel
        );
        assert!(world_view.outcome().formula_execution().is_none());
        assert!(world_view.outcome().lazy_execution().is_none());
    }
}

#[test]
fn ground_collection_reuses_the_supplied_graph() {
    let owner = normal("a. b :- a.");
    let graph = Arc::new(GroundProgram::compile(owner.program(), StaticLimits::default()).unwrap());
    let configured = SolveConfig {
        max_ground_rules: 0,
        max_substitutions: 0,
        ..config()
    };
    let world_view = WorldView::collect(
        PreparedInput::ground(&graph),
        configured,
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap();
    assert_eq!(
        family(&world_view),
        BTreeSet::from([vec!["a".into(), "b".into()]])
    );
    assert!(
        world_view
            .subject()
            .same_instance(&Subject::Program(graph.program().clone()))
    );
}

#[test]
fn formula_completion_batches_preserve_the_original_family() {
    let owner = formula("{a;b}. #minimize {1,a:a; -2,b:b}.");
    let configured = SolveConfig {
        completion_workers: NonZeroUsize::new(2).unwrap(),
        ..config()
    };
    let world_view = WorldView::collect(
        PreparedInput::formula(&owner),
        configured,
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap();
    assert_eq!(world_view.len(), 4);
    assert_eq!(
        world_view
            .outcome()
            .countermodel_statistics()
            .unwrap()
            .candidate_restrictions,
        0
    );
    assert!(world_view.outcome().formula_execution().is_some());
    assert_eq!(
        family(&world_view),
        family(&collect(PreparedInput::formula(&owner)))
    );
}

#[test]
fn requested_model_limit_refuses_complete_collection() {
    let owner = formula("{a;b}. #minimize {1,k:a}.");
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            models: 1,
            ..config()
        },
        Control::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::NotExhausted));
    assert_eq!(failure.answer_sets().len(), 1);
    assert_eq!(
        failure.outcome().unwrap().completion(),
        Some(Completion::RequestedModels)
    );
    assert!(
        failure.answer_sets()[0]
            .subject()
            .same_instance(failure.subject())
    );
}

#[test]
fn consumer_stop_does_not_establish_coverage() {
    let owner = formula("{a;b}. #minimize {1,k:a}.");
    let mut session =
        Session::enumerate(PreparedInput::formula(&owner), config(), Control::default()).unwrap();
    let answer = session.next().unwrap().unwrap();
    let outcome = session.stop();
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert!(outcome.subject().unwrap().same_instance(answer.subject()));
    assert!(!outcome.unsatisfiable());
}

#[test]
fn cancellation_cannot_produce_an_empty_world_view() {
    let relational = normal("a.");
    let formulas = formula("a.");
    for input in [
        PreparedInput::admitted(&relational),
        PreparedInput::formula(&formulas),
    ] {
        let control = Control::default();
        let request = Session::builder(input, config(), control.clone());
        control.cancel();
        let failure = request.collect(WorldViewLimits::default()).unwrap_err();
        assert!(matches!(failure.cause(), WorldViewError::NotExhausted));
        assert!(failure.answer_sets().is_empty());
        let outcome = failure.outcome().unwrap();
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert!(!outcome.unsatisfiable());
    }
}

#[test]
fn search_budget_retains_only_a_checked_prefix() {
    let owner = formula("{a;b}.");
    let failure = WorldView::collect(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_candidates: 1,
            ..config()
        },
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::NotExhausted));
    assert_eq!(failure.answer_sets().len(), 1);
    assert_eq!(
        failure.outcome().unwrap().completion(),
        Some(Completion::Interrupted)
    );
    assert_eq!(failure.outcome().unwrap().verified_models(), 1);
    assert!(
        failure.answer_sets()[0]
            .subject()
            .same_instance(failure.subject())
    );
    assert_eq!(failure.into_answer_sets().len(), 1);
}

#[test]
fn score_refusal_preserves_verified_membership() {
    let owner = formula("a. #minimize {1,k:a}.");
    let failure = WorldView::collect(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_objective_work: 0,
            ..config()
        },
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::NotExhausted));
    assert!(failure.answer_sets().is_empty());
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.scored_models(), 0);
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Objective(_))
    ));
    assert!(!outcome.unsatisfiable());
}

#[test]
fn unrestricted_scoring_spends_one_cumulative_budget() {
    let owner = formula("a. {b}. #minimize {1,k:a}.");
    let first = Session::enumerate(PreparedInput::formula(&owner), config(), Control::default())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let evaluation = zetesis_objective::evaluate(
        owner.objectives(),
        first.interpretation(),
        zetesis_objective::Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let failure = WorldView::collect(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_objective_work: evaluation.statistics().work,
            ..config()
        },
        WorldViewLimits::default(),
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::NotExhausted));
    assert_eq!(failure.answer_sets().len(), 1);
    assert_eq!(names(&failure.answer_sets()[0]), names(&first));
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 2);
    assert_eq!(outcome.scored_models(), 1);
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Objective(_))
    ));
}

#[test]
fn answer_storage_limit_preserves_the_retained_prefix() {
    let owner = formula("a. {b}.");
    let failure = Session::builder(PreparedInput::formula(&owner), config(), Control::default())
        .resources(&ExecutionResources::default())
        .collect(WorldViewLimits {
            max_answer_sets: 1,
            ..Default::default()
        })
        .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::AnswerSets));
    assert_eq!(failure.answer_sets().len(), 1);
    assert!(
        failure.answer_sets()[0]
            .interpretation()
            .atoms()
            .iter()
            .any(|atom| atom.predicate().name() == "a")
    );
    assert_eq!(failure.outcome().unwrap().verified_models(), 2);
    assert_eq!(failure.outcome().unwrap().completion(), None);
}

#[test]
fn collection_refusal_preserves_queued_cpu_membership() {
    let owner = formula("{a;b}. #minimize {1,k:a}.");
    let configured = SolveConfig {
        oracle: Oracle::Countermodel,
        completion_workers: NonZeroUsize::new(2).unwrap(),
        batch_size: NonZeroUsize::new(4).unwrap(),
        ..config()
    };
    let failure = WorldView::collect(
        PreparedInput::formula(&owner),
        configured,
        WorldViewLimits {
            max_answer_sets: 1,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::AnswerSets));
    assert_eq!(failure.answer_sets().len(), 1);
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 4);
    assert_eq!(outcome.scored_models(), 2);
    assert_eq!(outcome.completion(), None);
    let execution = outcome.formula_execution().unwrap();
    assert_eq!(execution.completion.entered, 4);
    assert_eq!(execution.gpu_batches, 0);
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 2)
    );
    assert!(
        failure.answer_sets()[0]
            .subject()
            .same_instance(failure.subject())
    );
}

#[test]
fn atom_storage_limit_counts_hidden_atoms() {
    let owner = normal("a. b. #show.");
    let failure = WorldView::collect(
        PreparedInput::admitted(&owner),
        config(),
        WorldViewLimits {
            max_atoms: 1,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::Atoms));
    assert!(failure.answer_sets().is_empty());
    assert_eq!(failure.outcome().unwrap().verified_models(), 1);
}

#[test]
fn zero_payload_budget_refuses_even_an_empty_answer() {
    let owner = normal("");
    let failure = WorldView::collect(
        PreparedInput::admitted(&owner),
        config(),
        WorldViewLimits {
            max_bytes: 0,
            ..Default::default()
        },
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::Bytes));
    assert_eq!(failure.outcome().unwrap().verified_models(), 1);
    assert!(!failure.outcome().unwrap().unsatisfiable());
}

#[test]
fn score_priorities_consume_collection_payload() {
    let absent = formula("a.");
    let active = formula("a. #minimize {0@3,k:a}.");
    // Catalog length + nullary atom record + selected-position length/index;
    // an absent score adds only its one-byte option tag.
    let model_bytes = 8 + 18 + 8 + 8;
    let limits = WorldViewLimits {
        max_bytes: model_bytes + 1,
        ..Default::default()
    };
    let world_view = WorldView::collect(
        PreparedInput::formula(&absent),
        config(),
        limits,
        Control::default(),
    )
    .unwrap();
    assert_eq!(world_view.len(), 1);
    let failure = WorldView::collect(
        PreparedInput::formula(&active),
        config(),
        limits,
        Control::default(),
    )
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::Bytes));
    assert_eq!(failure.outcome().unwrap().scored_models(), 1);
}

#[test]
fn setup_refusal_preserves_the_original_subject() {
    let owner = formula("a | b.");
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            oracle: Oracle::Closure,
            ..config()
        },
        Control::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap_err();
    assert!(
        matches!(failure.cause(), WorldViewError::Solve(error) if matches!(error.cause.as_ref(), zetesis_solve::SolveError::PreparedInput { .. }))
    );
    assert!(failure.answer_sets().is_empty());
    assert!(failure.outcome().is_none());
    assert!(
        failure
            .subject()
            .same_instance(&Subject::Theory(owner.theory().clone()))
    );
}

#[derive(Default)]
struct Routes {
    closures: Vec<Grounder>,
    formulas: usize,
}

impl ExecutionObserver for Routes {
    type Error = Infallible;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match observation {
            ExecutionObservation::CpuClosure { grounder, .. } => self.closures.push(grounder),
            ExecutionObservation::CpuFormula { .. } => self.formulas += 1,
            _ => {}
        }
        Ok(())
    }
}

enum Refusal {
    Execution,
    Formula,
}

struct RefuseObservation {
    at: Refusal,
    calls: usize,
}

impl ExecutionObserver for RefuseObservation {
    type Error = io::Error;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        self.calls += 1;
        let refuse = matches!(
            (&self.at, observation),
            (Refusal::Execution, ExecutionObservation::CpuFormula { .. })
                | (Refusal::Formula, ExecutionObservation::Formula { .. })
        );
        if refuse {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "collection observer refused",
            ));
        }
        Ok(())
    }
}

fn observer_failure(at: Refusal) -> (WorldViewFailure, usize) {
    let owner = formula("1 {a;b} 1. #minimize {1,a:a; 2,b:b}.");
    let mut observer = RefuseObservation { at, calls: 0 };
    let failure = Session::builder(PreparedInput::formula(&owner), config(), Control::default())
        .resources(&ExecutionResources::default())
        .collect_observed(WorldViewLimits::default(), &mut observer)
        .unwrap_err();
    assert!(
        failure
            .subject()
            .same_instance(&Subject::Theory(owner.theory().clone()))
    );
    assert!(failure.answer_sets().is_empty());
    let WorldViewError::Solve(solve) = failure.cause() else {
        panic!("observer refusal must retain a solve failure: {failure:?}");
    };
    let SolveError::ExecutionObservation(cause) = solve.cause.as_ref() else {
        panic!("observer refusal must remain external: {failure:?}");
    };
    assert_eq!(
        cause.downcast_ref::<io::Error>().unwrap().kind(),
        io::ErrorKind::ConnectionAborted
    );
    assert_eq!(cause.to_string(), "collection observer refused");
    (failure, observer.calls)
}

#[test]
fn collection_preserves_preparation_observer_failure() {
    let (failure, calls) = observer_failure(Refusal::Execution);
    assert_eq!(calls, 1);
    assert!(failure.outcome().is_none());
}

#[test]
fn collection_preserves_deferred_formula_failure() {
    let (failure, calls) = observer_failure(Refusal::Formula);
    assert_eq!(calls, 2);
    let outcome = failure.outcome().unwrap();
    assert!(outcome.subject().unwrap().same_instance(failure.subject()));
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.completion(), None);
    assert!(!outcome.unsatisfiable());
}
