//! External consumers distinguish semantic membership from delivery and reuse admission.

use std::{
    collections::BTreeSet,
    io::{self, Write},
    num::NonZeroUsize,
    sync::Arc,
};

use clap::Parser;
use zetesis_cli::{
    Backend, Completion, Grounder, Interruption, Options, Oracle, PreparedInput, RunError, Session,
    SessionModel, SolveConfig, Subject, run_finalized_with_diagnostics,
};
use zetesis_core::{GroundProgram, StaticLimits};
use zetesis_cpu::Control;
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
fn atoms(model: &SessionModel) -> Vec<String> {
    model
        .interpretation()
        .atoms()
        .iter()
        .map(|atom| atom.predicate().name().to_string())
        .collect()
}
fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(extra.iter().copied()),
    )
    .unwrap()
}

#[test]
fn configuration_defaults_preserve_legacy_limits() {
    // A separate plain configuration must not silently change CLI defaults.
    // The command alone follows the host: its worker count is the host's
    // parallelism and each closure's allowance is that count's share of the
    // collective ceiling. Every other default is the library's.
    let command = SolveConfig::from(&Options::try_parse_from(["zetesis"]).unwrap());
    let library = SolveConfig::default();
    assert_eq!(
        command.max_closure_bytes,
        library.max_closure_batch_bytes / command.workers.get()
    );
    let aligned = SolveConfig {
        workers: library.workers,
        max_closure_bytes: library.max_closure_bytes,
        ..command
    };
    assert_eq!(format!("{aligned:?}"), format!("{library:?}"));
}

#[test]
fn prepared_closure_preserves_hidden_interpretations() {
    let admitted = normal("{a}. {b}. #show.");
    let mut session = Session::new(
        PreparedInput::admitted(&admitted),
        config(),
        Control::default(),
    )
    .unwrap();
    let actual: BTreeSet<_> = session
        .by_ref()
        .map(|result| atoms(&result.unwrap()))
        .collect();
    assert_eq!(
        actual,
        [
            vec![],
            vec!["a".into()],
            vec!["b".into()],
            vec!["a".into(), "b".into()]
        ]
        .into_iter()
        .collect()
    );
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 4);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(!outcome.unsatisfiable());
}

#[test]
fn detached_model_retains_program_identity() {
    let admitted = normal("a.");
    let other = normal("a.");
    let model = {
        let mut session = Session::new(
            PreparedInput::admitted(&admitted),
            config(),
            Control::default(),
        )
        .unwrap();
        session.next().unwrap().unwrap()
    };
    let Subject::Program(program) = model.subject() else {
        panic!("closure subject")
    };
    assert!(program.same_instance(admitted.program()));
    assert!(!program.same_instance(other.program()));
    assert_eq!(atoms(&model), ["a"]);
}

#[test]
fn detached_model_retains_theory_identity() {
    let admitted = formula("a | b.");
    let other = formula("a | b.");
    let model = Session::new(
        PreparedInput::formula(&admitted),
        config(),
        Control::default(),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap();
    let Subject::Theory(theory) = model.subject() else {
        panic!("formula subject")
    };
    assert!(theory.same_instance(admitted.theory()));
    assert!(!theory.same_instance(other.theory()));
    assert_eq!(model.interpretation().atoms().len(), 1);
}

#[test]
fn ground_session_reuses_the_supplied_graph() {
    let admitted = normal("a. b :- a.");
    let graph =
        Arc::new(GroundProgram::compile(admitted.program(), StaticLimits::default()).unwrap());
    // Zero substitution/rule ceilings refuse recompilation; max_atoms also bounds
    // membership and therefore retains its ordinary allowance.
    let configured = SolveConfig {
        max_ground_rules: 0,
        max_substitutions: 0,
        stats: true,
        ..config()
    };
    let mut session = Session::new(
        PreparedInput::ground(&graph),
        configured,
        Control::default(),
    )
    .unwrap();
    assert_eq!(Arc::strong_count(&graph), 2);
    let result = session.next().unwrap().unwrap();
    assert_eq!(atoms(&result), ["a", "b"]);
    assert!(session.next().is_none());
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
    assert_eq!(
        session.phase_timings().unwrap().stages.grounding_mode,
        zetesis_cli::GroundingMode::Unentered
    );
    drop(session);
    assert_eq!(Arc::strong_count(&graph), 1);
}

#[test]
fn prepared_strategies_refuse_incompatible_requests() {
    let admitted = normal("a.");
    let formulas = formula("a | b.");
    let ground =
        Arc::new(GroundProgram::compile(admitted.program(), StaticLimits::default()).unwrap());
    for (input, oracle, grounder) in [
        (
            PreparedInput::admitted(&admitted),
            Oracle::Countermodel,
            Grounder::Auto,
        ),
        (
            PreparedInput::formula(&formulas),
            Oracle::Closure,
            Grounder::Auto,
        ),
        (
            PreparedInput::formula(&formulas),
            Oracle::Auto,
            Grounder::Lazy,
        ),
        (PreparedInput::ground(&ground), Oracle::Auto, Grounder::Lazy),
    ] {
        let configured = SolveConfig {
            oracle,
            grounder,
            ..config()
        };
        let failure = Session::new(input, configured, Control::default())
            .err()
            .unwrap();
        assert!(matches!(
            *failure.cause,
            zetesis_cli::SolveError::PreparedInput { .. }
        ));
        assert!(failure.semantic().is_none());
    }
}

#[test]
fn early_consumer_stop_preserves_partial_coverage() {
    let admitted = normal("{a}. {b}.");
    let mut session = Session::new(
        PreparedInput::admitted(&admitted),
        config(),
        Control::default(),
    )
    .unwrap();
    session.next().unwrap().unwrap();
    assert!(session.outcome().is_none());
    let outcome = session.stop();
    assert_eq!(outcome.completion(), None);
    assert!(!outcome.unsatisfiable());
    assert!(!outcome.optimum_proved());
    assert_eq!(outcome.verified_models(), 1);
}

#[test]
fn requested_count_preserves_batch_verification() {
    let admitted = normal("{a}. {b}.");
    let mut session = Session::new(
        PreparedInput::admitted(&admitted),
        SolveConfig {
            models: 2,
            ..config()
        },
        Control::default(),
    )
    .unwrap();
    assert_eq!(session.by_ref().count(), 2);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::RequestedModels));
    assert_eq!(outcome.verified_models(), 4);
    assert_eq!(outcome.candidate_progress(), 2);
    assert!(session.next().is_none());
}

#[test]
fn repeated_sessions_own_independent_search_budgets() {
    let admitted = formula("a | b.");
    for _ in 0..2 {
        let mut session = Session::new(
            PreparedInput::formula(&admitted),
            config(),
            Control::default(),
        )
        .unwrap();
        assert_eq!(session.by_ref().filter_map(Result::ok).count(), 2);
        let outcome = session.outcome().unwrap();
        assert_eq!(outcome.verified_models(), 2);
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    }
}

#[test]
fn source_and_prepared_objective_costs_agree() {
    let source = "a. {b}. #show. #minimize{0@2,k:a;0@-1,j:b}.";
    let admitted = formula(source);
    for workers in [1, 2] {
        let mut session = Session::new(
            PreparedInput::formula(&admitted),
            SolveConfig {
                completion_workers: NonZeroUsize::new(workers).unwrap(),
                ..config()
            },
            Control::default(),
        )
        .unwrap();
        let models: Vec<_> = session.by_ref().map(Result::unwrap).collect();
        let semantic = session.outcome().unwrap();
        let mut output = Vec::new();
        let rendered = run_finalized_with_diagnostics(
            source.into(),
            &options(&["--json", "--completion-workers", &workers.to_string()]),
            &mut output,
            &mut io::sink(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(
            semantic.verified_models(),
            rendered.semantic().verified_models()
        );
        assert_eq!(
            semantic.scored_models(),
            rendered.semantic().scored_models()
        );
        assert_eq!(semantic.retained_models(), 2);
        assert!(semantic.optimum_proved());
        let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(json["models"].as_array().unwrap().len(), models.len());
        let mut raw = BTreeSet::new();
        for (record, model) in json["models"].as_array().unwrap().iter().zip(&models) {
            let names: Vec<_> = record["model"]["full_model"]
                .as_array()
                .unwrap()
                .iter()
                .map(|atom| atom["predicate"].as_str().unwrap().to_string())
                .collect();
            assert_eq!(names, atoms(model));
            assert_eq!(
                model.score().unwrap().costs(),
                semantic.incumbent().unwrap().score.costs()
            );
            raw.insert(atoms(model));
        }
        assert_eq!(
            raw,
            [vec!["a".into()], vec!["a".into(), "b".into()]]
                .into_iter()
                .collect()
        );
        assert_eq!(models.len(), rendered.publication().models());
    }
}

#[test]
fn objective_retention_precedes_tie_delivery() {
    let admitted = formula("a. {b}. #minimize{0,k:a}.");
    let mut session = Session::new(
        PreparedInput::formula(&admitted),
        config(),
        Control::default(),
    )
    .unwrap();
    session.next().unwrap().unwrap();
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.retained_models(), 2);
    assert_eq!(outcome.incumbent().unwrap().tied_models, 2);
    assert!(outcome.optimum_proved());
    assert_eq!(session.count(), 1);
}

#[test]
fn interruption_retains_an_unproved_incumbent() {
    let admitted = formula("1 {a;b} 1. #minimize{1,a:a;2,b:b}.");
    let mut session = Session::new(
        PreparedInput::formula(&admitted),
        SolveConfig {
            max_candidates: 1,
            max_objective_bound_work: 0,
            ..config()
        },
        Control::default(),
    )
    .unwrap();
    let model = session.next().unwrap().unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        (
            outcome.verified_models(),
            outcome.scored_models(),
            outcome.retained_models()
        ),
        (1, 1, 1)
    );
    let expected = if atoms(&model) == ["a"] {
        1
    } else {
        assert_eq!(atoms(&model), ["b"]);
        2
    };
    assert_eq!(model.score().unwrap().costs(), &[(0, expected)]);
    assert_eq!(
        outcome.incumbent().unwrap().score.costs(),
        model.score().unwrap().costs()
    );
    assert!(!outcome.optimum_proved());
}

#[test]
fn scoring_refusal_preserves_verified_membership() {
    let admitted = formula("a. #minimize{1,k:a}.");
    let mut session = Session::new(
        PreparedInput::formula(&admitted),
        SolveConfig {
            max_objective_work: 0,
            ..config()
        },
        Control::default(),
    )
    .unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(
        (
            outcome.verified_models(),
            outcome.scored_models(),
            outcome.retained_models()
        ),
        (1, 0, 0)
    );
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Objective(_))
    ));
    assert!(!outcome.unsatisfiable());
}

#[test]
fn cancelled_formula_does_not_enter_membership() {
    let admitted = formula("a | b.");
    let control = Control::default();
    control.cancel();
    let mut session = Session::new(PreparedInput::formula(&admitted), config(), control).unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Preparation(zetesis_cpu::Stop::Cancelled))
    ));
    assert_eq!(outcome.candidate_progress(), 0);
    assert!(outcome.countermodel_statistics().is_none());
    assert!(outcome.formula_execution().is_none());
    assert!(!outcome.unsatisfiable());
}

#[test]
fn empty_exhausted_search_establishes_unsatisfiability() {
    let admitted = formula(":-.");
    let mut session = Session::new(
        PreparedInput::formula(&admitted),
        config(),
        Control::default(),
    )
    .unwrap();
    assert!(session.next().is_none());
    assert!(session.outcome().unwrap().unsatisfiable());
}

struct Closed;
impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn failed_publication_preserves_established_optimum() {
    let failure = run_finalized_with_diagnostics(
        "a. #minimize{1,k:a}.".into(),
        &options(&[]),
        &mut Closed,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    let outcome = failure.semantic().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.retained_models(), 1);
    assert!(outcome.optimum_proved());
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(!failure.publication().unwrap().summary());
    assert!(matches!(*failure.cause, RunError::Output(_)));
}

#[test]
fn failed_first_answer_preserves_unknown_coverage() {
    let failure = run_finalized_with_diagnostics(
        "a.".into(),
        &options(&[]),
        &mut Closed,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    let outcome = failure.semantic().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.completion(), None);
    assert_eq!(failure.publication().unwrap().models(), 0);
    assert!(!outcome.unsatisfiable());
}

#[test]
fn reporting_failures_remain_separately_observable() {
    struct HeaderOnly(bool);
    impl Write for HeaderOnly {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if std::mem::replace(&mut self.0, false) {
                Ok(bytes.len())
            } else {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "summary closed"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    // Admission is the first cause; statistics and the JSON footer each fail later.
    let failure = run_finalized_with_diagnostics(
        "a(".into(),
        &options(&["--json", "--stats"]),
        &mut HeaderOnly(true),
        &mut Closed,
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, RunError::Expansion(_)));
    assert_eq!(failure.diagnostics_failure().unwrap().to_string(), "closed");
    assert_eq!(
        failure.summary_failure().unwrap().to_string(),
        "summary closed"
    );
    assert_eq!(
        failure.secondary_output.as_ref().unwrap().to_string(),
        "summary closed"
    );
    assert!(failure.semantic().is_none());
}

#[test]
fn detached_outcome_retains_original_subject() {
    let admitted = formula("a. {b}. #minimize{0,k:a}.");
    let outcome = {
        let mut session = Session::new(
            PreparedInput::formula(&admitted),
            config(),
            Control::default(),
        )
        .unwrap();
        assert_eq!(session.by_ref().map(Result::unwrap).count(), 2);
        session.outcome().unwrap()
    };
    let Subject::Theory(theory) = outcome.subject().unwrap() else {
        panic!("original theory")
    };
    assert!(theory.same_instance(admitted.theory()));
    assert!(outcome.optimum_proved());
}

#[test]
fn compatibility_mutation_cannot_rewrite_publication() {
    let mut failure = run_finalized_with_diagnostics(
        "a. #minimize{1,k:a}.".into(),
        &options(&[]),
        &mut Closed,
        &mut io::sink(),
        &Control::default(),
    )
    .unwrap_err();
    let before = failure.publication().unwrap();
    let partial = failure.partial_report.as_mut().unwrap();
    partial.published_models = 100;
    partial.summary_published = true;
    partial.verified_models = 0;
    partial.completion = None;
    assert_eq!(failure.publication().unwrap(), before);
    assert_eq!(failure.semantic().unwrap().verified_models(), 1);
    assert!(failure.semantic().unwrap().optimum_proved());
}

#[test]
fn cancelled_closure_retains_typed_incomplete_coverage() {
    let admitted = normal("a.");
    for grounder in [Grounder::Lazy, Grounder::Eager] {
        let control = Control::default();
        control.cancel();
        let mut session = Session::new(
            PreparedInput::admitted(&admitted),
            SolveConfig {
                grounder,
                ..config()
            },
            control,
        )
        .unwrap();
        assert!(session.next().is_none());
        let outcome = session.outcome().unwrap();
        assert_eq!(outcome.verified_models(), 0);
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(
            outcome.interruption(),
            Some(Interruption::Preparation(zetesis_cpu::Stop::Cancelled))
        );
        assert!(!outcome.unsatisfiable());
    }
}

#[test]
fn stopped_prepared_inputs_skip_execution_setup() {
    let admitted = normal("a. b :- a.");
    let graph =
        Arc::new(GroundProgram::compile(admitted.program(), StaticLimits::default()).unwrap());
    for input in [
        PreparedInput::admitted(&admitted),
        PreparedInput::ground(&graph),
    ] {
        let control = Control::default();
        control.cancel();
        let mut session = Session::new(
            input,
            SolveConfig {
                grounder: Grounder::Eager,
                max_ground_rules: 0,
                max_substitutions: 0,
                stats: true,
                ..config()
            },
            control,
        )
        .unwrap();
        assert!(session.next().is_none());
        let outcome = session.outcome().unwrap();
        assert_eq!(
            outcome.interruption(),
            Some(Interruption::Preparation(zetesis_cpu::Stop::Cancelled))
        );
        let Subject::Program(program) = outcome.subject().unwrap() else {
            panic!("original program")
        };
        assert!(program.same_instance(admitted.program()));
        assert!(
            session
                .phase_timings()
                .unwrap()
                .get(zetesis_cli::SolvePhase::ExecutionSetup)
                .is_none()
        );
        assert_eq!(Arc::strong_count(&graph), 1);
    }
}

#[test]
fn native_program_preparation_reuses_the_original_subject() {
    let admitted = normal("{a}. b :- a.");
    let prepared = PreparedInput::program(admitted.program());
    assert!(prepared.metadata().is_none());
    let mut session = Session::new(prepared, config(), Control::default()).unwrap();
    let actual: BTreeSet<_> = session
        .by_ref()
        .map(|answer| {
            let answer = answer.unwrap();
            assert!(
                answer
                    .subject()
                    .same_instance(&Subject::Program(admitted.program().clone()))
            );
            atoms(&answer)
        })
        .collect();
    assert_eq!(
        actual,
        BTreeSet::from([vec![], vec!["a".into(), "b".into()]])
    );
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
}

#[test]
fn prepared_strategy_failure_retains_only_known_subject() {
    let admitted = normal("a.");
    let result = Session::new(
        PreparedInput::program(admitted.program()),
        SolveConfig {
            oracle: Oracle::Countermodel,
            ..config()
        },
        Control::default(),
    );
    let Err(failure) = result else {
        panic!("native source is not an eager formula")
    };
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::Program(admitted.program().clone()))
    );
    assert!(failure.semantic().is_none());
}

#[derive(Clone, Copy)]
enum Rejection {
    Execution,
    Formula,
    Bound,
}
struct RejectObservation {
    rejection: Rejection,
    calls: usize,
    failures: usize,
    incumbent_costs: Option<Vec<(i32, i64)>>,
}
impl RejectObservation {
    const fn new(rejection: Rejection) -> Self {
        Self {
            rejection,
            calls: 0,
            failures: 0,
            incumbent_costs: None,
        }
    }
}
impl zetesis_cli::ExecutionObserver for RejectObservation {
    // Deliberately backend-shaped: a device error must not turn an observer
    // failure into another execution route.
    type Error = RunError;
    fn observe(
        &mut self,
        observation: zetesis_cli::ExecutionObservation<'_>,
    ) -> Result<(), Self::Error> {
        use zetesis_cli::ExecutionObservation as Event;
        self.calls += 1;
        if let Event::ObjectiveBound { costs, .. } = &observation {
            self.incumbent_costs = Some(costs.to_vec());
        }
        if matches!(
            (self.rejection, observation),
            (Rejection::Execution, Event::CpuFormula { .. })
                | (Rejection::Formula, Event::Formula { .. })
                | (Rejection::Bound, Event::ObjectiveBound { .. })
        ) {
            self.failures += 1;
            Err(RunError::BackendUnavailable)
        } else {
            Ok(())
        }
    }
}
fn observer_cause(failure: &zetesis_cli::SolveFailure) {
    let zetesis_cli::SolveError::ExecutionObservation(cause) = failure.cause.as_ref() else {
        panic!("the callback failure must remain external: {failure}")
    };
    assert!(matches!(
        cause.downcast_ref::<RunError>(),
        Some(RunError::BackendUnavailable)
    ));
}

#[test]
fn execution_observer_failure_precedes_formula_search() {
    let admitted = formula("a | b.");
    let mut observer = RejectObservation::new(Rejection::Execution);
    let result = Session::new_observed(
        PreparedInput::formula(&admitted),
        config(),
        Control::default(),
        &mut observer,
    );
    let Err(failure) = result else {
        panic!("execution observation failed")
    };
    observer_cause(&failure);
    assert_eq!(observer.calls, 1);
    assert_eq!(observer.failures, 1);
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::Theory(admitted.theory().clone()))
    );
    assert!(failure.semantic().is_none());
}

#[test]
fn formula_setup_observer_failure_is_retained_for_first_pull() {
    let admitted = formula("a | b.");
    let mut observer = RejectObservation::new(Rejection::Formula);
    let mut session = Session::new_observed(
        PreparedInput::formula(&admitted),
        config(),
        Control::default(),
        &mut observer,
    )
    .unwrap();
    assert_eq!(observer.calls, 2);
    assert_eq!(observer.failures, 1);
    let failure = session.next_observed(&mut observer).unwrap().unwrap_err();
    observer_cause(&failure);
    let outcome = failure.semantic().unwrap();
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.completion(), None);
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::Theory(admitted.theory().clone()))
    );
    assert!(session.next_observed(&mut observer).is_none());
    assert!(session.next().is_none());
    assert_eq!(
        observer.calls, 2,
        "failed initialization never reaches certificate setup"
    );
}

#[test]
fn bound_observer_failure_retains_the_verified_incumbent() {
    let admitted = formula("{a;b}. #minimize {1,a:a;1,b:b}.");
    let mut observer = RejectObservation::new(Rejection::Bound);
    let mut session = Session::new_observed(
        PreparedInput::formula(&admitted),
        config(),
        Control::default(),
        &mut observer,
    )
    .unwrap();
    let failure = session.next_observed(&mut observer).unwrap().unwrap_err();
    observer_cause(&failure);
    assert_eq!(observer.failures, 1);
    let outcome = failure.semantic().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.scored_models(), 1);
    assert_eq!(outcome.retained_models(), 1);
    assert_eq!(
        outcome.incumbent().unwrap().score.costs(),
        observer.incumbent_costs.as_deref().unwrap()
    );
    assert!(
        outcome
            .subject()
            .unwrap()
            .same_instance(&Subject::Theory(admitted.theory().clone()))
    );
    assert_eq!(outcome.completion(), None);
    assert!(!outcome.optimum_proved());
    let calls = observer.calls;
    assert!(session.next_observed(&mut observer).is_none());
    assert!(session.next().is_none());
    assert_eq!(observer.calls, calls);
    assert_eq!(session.outcome().unwrap().verified_models(), 1);
}

#[test]
fn observed_enumeration_preserves_all_objective_scores() {
    use std::convert::Infallible;
    use zetesis_cli::{ExecutionObservation, ExecutionObserver};

    #[derive(Default)]
    struct Facts {
        formula: Option<(usize, usize, usize)>,
        bounds: usize,
    }
    impl ExecutionObserver for Facts {
        type Error = Infallible;
        fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
            match observation {
                ExecutionObservation::Formula {
                    atoms,
                    nodes,
                    roots,
                } => {
                    assert!(self.formula.replace((atoms, nodes, roots)).is_none());
                }
                ExecutionObservation::ObjectiveBound { .. } => self.bounds += 1,
                _ => {}
            }
            Ok(())
        }
    }
    let admitted = formula("{a;b}. #minimize {1,a:a;1,b:b}.");
    let input = PreparedInput::formula(&admitted);
    let mut observer = Facts::default();
    let mut session =
        Session::enumerate_observed(input, config(), Control::default(), &mut observer).unwrap();
    assert_eq!(
        observer.formula,
        Some((
            admitted.theory().atom_count(),
            admitted.theory().nodes().len(),
            admitted.theory().roots().len()
        ))
    );
    let mut actual = Vec::new();
    while let Some(answer) = session.next_observed(&mut observer) {
        let answer = answer.unwrap();
        actual.push((atoms(&answer), answer.score().unwrap().costs().to_vec()));
    }
    let mut expected: Vec<_> = Session::enumerate(input, config(), Control::default())
        .unwrap()
        .map(|answer| {
            let answer = answer.unwrap();
            (atoms(&answer), answer.score().unwrap().costs().to_vec())
        })
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 4);
    assert_eq!(
        observer.bounds, 0,
        "unrestricted enumeration installs no dominance bounds"
    );
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
}
