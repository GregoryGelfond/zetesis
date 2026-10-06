//! Base membership becomes original membership only after full reconstruction.

use std::{collections::BTreeSet, convert::Infallible, num::NonZeroUsize};

use zetesis_core::{Atom, Model, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_solve::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder, GroundingMode,
    Interruption, Oracle, PreparedInput, PreparedProfile, Session, SolveConfig, SolveError,
    SolveMeasurements, SolvePhase, SolveStage, Subject, WorldView, WorldViewError, WorldViewLimits,
};
use zetesis_test_support::programs::unary as atom;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaMaterialization,
    FormulaResource, TerminalFormula, admit_formula, prepare_formula,
};

const OPTIONAL: &str = "{seed(1);seed(2)}. receipt(X):-seed(X).";

fn terminal(source: &str) -> TerminalFormula {
    terminal_with_limits(source, &FormulaLimits::default())
}

fn terminal_with_limits(source: &str, limits: &FormulaLimits) -> TerminalFormula {
    let materialized = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
    )
    .unwrap()
    .ground_adaptive()
    .unwrap();
    let FormulaMaterialization::Terminal(owner) = materialized else {
        panic!("fixture requires a certified terminal partition");
    };
    owner
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Auto,
        oracle: Oracle::Auto,
        models: 0,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        ..SolveConfig::default()
    }
}

fn expected() -> BTreeSet<Model> {
    (0..4)
        .map(|mask| {
            let atoms = (1..=2)
                .filter(|number| mask & (1 << (number - 1)) != 0)
                .flat_map(|number| [atom("seed", number), atom("receipt", number)]);
            Model::new(atoms).unwrap()
        })
        .collect()
}

fn collect(input: PreparedInput<'_>) -> WorldView {
    let view = Session::builder(input, config(), Cancellation::default())
        .collect(WorldViewLimits::default())
        .unwrap();
    assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
    view
}

fn family(view: &WorldView) -> BTreeSet<Model> {
    view.answer_sets()
        .iter()
        .map(|answer| answer.interpretation().clone())
        .collect()
}

fn hidden_compound() -> Atom {
    let value = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 2,
            },
            ValueNode::Function {
                name: "g".into(),
                sign: Sign::Positive,
                arity: 1,
            },
            ValueNode::Number(1),
            ValueNode::String("1".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    Atom::new(Predicate::new("hidden", 1).unwrap(), vec![value]).unwrap()
}

#[test]
fn ground_only_definitions_preserve_complete_families() {
    let hidden = hidden_compound();
    for (source, expected) in [
        (
            "hidden(f(g(1),\"1\")). receipt(1). #show receipt/1.",
            BTreeSet::from([Model::new([hidden.clone(), atom("receipt", 1)]).unwrap()]),
        ),
        (
            "hidden(f(g(1),\"1\")). {seed(1)}. receipt(1):-seed(1). #show receipt/1.",
            BTreeSet::from([
                Model::new([hidden.clone()]).unwrap(),
                Model::new([hidden.clone(), atom("seed", 1), atom("receipt", 1)]).unwrap(),
            ]),
        ),
    ] {
        let FormulaMaterialization::Complete(owner) = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_adaptive()
        .unwrap() else {
            panic!("ground-only definitions require complete materialization");
        };
        let original = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        for input in [
            PreparedInput::formula(&owner),
            PreparedInput::formula(&original),
        ] {
            let view = collect(input);
            assert_eq!(family(&view), expected);
            assert!(view.outcome().terminal_execution().is_none());
        }
    }
}

#[test]
fn retained_ground_facts_preserve_terminal_families() {
    let source = "closed(7). {seed(1)}. receipt(X):-seed(X). #show receipt/1.";
    let owner = terminal(source);
    assert_eq!(owner.deferred_templates(), 1);
    let possible_base = Model::from_positions(
        owner.base_atom_catalog(),
        0..owner.base_atom_catalog().atoms().len(),
    )
    .unwrap();
    assert_eq!(
        possible_base,
        Model::new([atom("closed", 7), atom("seed", 1)]).unwrap()
    );
    let original = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let expected = BTreeSet::from([
        Model::new([atom("closed", 7)]).unwrap(),
        Model::new([atom("closed", 7), atom("seed", 1), atom("receipt", 1)]).unwrap(),
    ]);
    let eager = collect(PreparedInput::formula(&original));
    assert_eq!(family(&eager), expected);
    let adaptive = collect(PreparedInput::terminal(&owner));
    assert_eq!(family(&adaptive), expected);
    let receipt = adaptive.outcome().terminal_execution().unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (2, 2, 0)
    );
}

#[test]
fn reconstruction_uses_answer_truth_not_possible_support() {
    let owner = terminal(OPTIONAL);
    let original = admit_formula(
        OPTIONAL.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let reference = Session::builder(
        PreparedInput::formula(&original),
        config(),
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap();
    let family: BTreeSet<_> = reference
        .answer_sets()
        .iter()
        .map(|answer| answer.interpretation().clone())
        .collect();
    assert_eq!(family, expected());
    for workers in [1, 4] {
        let view = Session::builder(
            PreparedInput::terminal(&owner),
            SolveConfig {
                workers: NonZeroUsize::new(workers).unwrap(),
                ..config()
            },
            Cancellation::default(),
        )
        .collect(WorldViewLimits::default())
        .unwrap();
        let actual: BTreeSet<_> = view
            .answer_sets()
            .iter()
            .map(|answer| answer.interpretation().clone())
            .collect();
        assert_eq!(actual, family);
        assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
        assert_eq!(view.outcome().verified_models(), 4);
        let receipt = view.outcome().terminal_execution().unwrap();
        assert_eq!(
            (receipt.base_answers, receipt.reconstructed, receipt.pending),
            (4, 4, 0)
        );
        assert_eq!(receipt.reconstruction.attempts, 4);
        assert_eq!(receipt.reconstruction.completed, 4);
        assert!(
            view.answer_sets()
                .iter()
                .all(|answer| answer.subject().same_instance(view.subject()))
        );
        assert!(
            view.subject()
                .same_instance(&Subject::TerminalDefinitions(owner.clone()))
        );
        assert!(
            !view
                .subject()
                .same_instance(&Subject::Theory(owner.base_theory().clone()))
        );
    }
}

#[test]
fn facts_and_multiple_producers_preserve_the_full_family() {
    for source in [
        "seed(1). receipt(X):-seed(X). receipt(2).",
        "{left(1);right(1)}. receipt(X):-left(X). receipt(X):-right(X).",
        "seed(1). :-seed(1). receipt(X):-seed(X).",
    ] {
        let owner = terminal(source);
        let original = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let mut families = Vec::new();
        for input in [
            PreparedInput::formula(&original),
            PreparedInput::terminal(&owner),
        ] {
            let view = Session::builder(input, config(), Cancellation::default())
                .collect(WorldViewLimits::default())
                .unwrap();
            families.push(
                view.answer_sets()
                    .iter()
                    .map(|answer| answer.interpretation().clone())
                    .collect::<BTreeSet<_>>(),
            );
            assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
        }
        assert_eq!(families[0], families[1]);
    }
}

#[test]
fn requested_models_count_only_reconstructed_original_answers() {
    let owner = terminal(OPTIONAL);
    let failure = Session::builder(
        PreparedInput::terminal(&owner),
        SolveConfig {
            models: 1,
            ..config()
        },
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::NotExhausted));
    assert_eq!(failure.answer_sets().len(), 1);
    assert!(expected().contains(failure.answer_sets()[0].interpretation()));
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::RequestedModels));
    assert_eq!(outcome.verified_models(), 1);
    let receipt = outcome.terminal_execution().unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (1, 1, 0)
    );
}

#[test]
fn original_observations_read_reconstructed_atoms() {
    let owner = terminal("{seed(1);seed(2)}. receipt(X):-seed(X). #show X:receipt(X).");
    let mut session = Session::new(
        PreparedInput::terminal(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        let shown = owner
            .metadata()
            .observations()
            .evaluate(
                answer.interpretation(),
                zetesis_themelios::observation::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let expected: Vec<_> = (1..=2)
            .filter(|number| answer.interpretation().contains(&atom("receipt", *number)))
            .map(zetesis_themelios::observation::Symbol::Number)
            .collect();
        assert_eq!(shown.symbols(), expected);
    }
    assert_eq!(session.outcome().unwrap().verified_models(), 4);
}

#[derive(Default)]
struct Observed {
    terminal: usize,
    base: usize,
}

impl ExecutionObserver for Observed {
    type Error = Infallible;
    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::TerminalDefinitions {
                requested,
                deferred_templates,
            } => {
                assert_eq!(requested, Grounder::Auto);
                assert_eq!(deferred_templates, 1);
                self.terminal += 1;
            }
            ExecutionObservation::Formula { .. } => {
                assert_eq!(
                    self.terminal, 1,
                    "base events need their original-source context"
                );
                self.base += 1;
            }
            _ => {}
        }
        Ok(())
    }
}

#[test]
fn repeated_sessions_keep_reconstruction_and_measurements_independent() {
    let owner = terminal(OPTIONAL);
    let mut receipts = Vec::new();
    for stats in [false, true] {
        let mut observed = Observed::default();
        let mut session = Session::builder(
            PreparedInput::terminal(&owner),
            SolveConfig { stats, ..config() },
            Cancellation::default(),
        )
        .start_observed(&mut observed)
        .unwrap();
        let mut answers = BTreeSet::new();
        while let Some(answer) = session.next_observed(&mut observed) {
            assert!(answers.insert(answer.unwrap().into_interpretation()));
        }
        assert_eq!(answers, expected());
        assert_eq!((observed.terminal, observed.base), (1, 1));
        receipts.push(*session.outcome().unwrap().terminal_execution().unwrap());
        assert_eq!(session.phase_timings().is_some(), stats);
        if let Some(phases) = session.phase_timings() {
            assert_eq!(
                phases.get(SolvePhase::AnswerReconstruction).unwrap().calls,
                4
            );
        }
    }
    assert_eq!(receipts[0], receipts[1]);
}

#[test]
fn explicit_grounding_policies_do_not_acquire_terminal_meaning() {
    let owner = terminal(OPTIONAL);
    for grounder in [Grounder::Eager, Grounder::Lazy] {
        let failure = Session::new(
            PreparedInput::terminal(&owner),
            SolveConfig {
                grounder,
                ..config()
            },
            Cancellation::default(),
        )
        .err()
        .expect("explicit policy must be preserved");
        assert!(matches!(
            *failure.cause,
            SolveError::PreparedInput {
                profile: PreparedProfile::TerminalDefinitions,
                ..
            }
        ));
        assert!(
            failure
                .subject()
                .unwrap()
                .same_instance(&Subject::TerminalDefinitions(owner.clone()))
        );
    }
}

#[test]
fn pre_cancelled_sessions_retain_the_original_subject_without_reconstruction() {
    let owner = terminal(OPTIONAL);
    let token = Cancellation::default();
    token.cancel();
    let mut observed = Observed::default();
    let mut session = Session::builder(
        PreparedInput::terminal(&owner),
        SolveConfig {
            stats: true,
            ..config()
        },
        token,
    )
    .start_observed(&mut observed)
    .unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Preparation(Stop::Cancelled))
    );
    assert!(outcome.terminal_execution().is_none());
    assert_eq!((observed.terminal, observed.base), (0, 0));
    let timings = session.phase_timings().unwrap();
    assert_eq!(
        timings.stages.grounding_mode,
        GroundingMode::EagerBaseTerminalDefinitions
    );
    assert!(timings.stages.get(SolveStage::Grounding).is_none());
    assert!(timings.get(SolvePhase::AnswerReconstruction).is_none());
    assert!(
        outcome
            .subject()
            .unwrap()
            .same_instance(&Subject::TerminalDefinitions(owner))
    );
}

#[test]
fn reconstruction_work_refusal_preserves_the_checked_prefix() {
    let owner = terminal(OPTIONAL);
    // Each answer gets the headroom admission left. The answer holding both
    // seeds costs the most to reconstruct; a ceiling one short of admission
    // plus its cost refuses that answer alone.
    let mut cursor = owner.reconstruction().unwrap();
    let admission = cursor.statistics().work;
    let both = Model::from_positions(
        owner.base_atom_catalog(),
        0..owner.base_atom_catalog().atoms().len(),
    )
    .unwrap();
    cursor.reconstruct(&both, &Cancellation::default()).unwrap();
    let costliest = cursor.statistics().latest_work;
    let completed_work = Session::builder(
        PreparedInput::terminal(&owner),
        config(),
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap()
    .outcome()
    .terminal_execution()
    .unwrap()
    .reconstruction
    .work;
    let limited = terminal_with_limits(
        OPTIONAL,
        &FormulaLimits {
            max_work: admission + costliest - 1,
            ..FormulaLimits::default()
        },
    );
    let mut session = Session::new(
        PreparedInput::terminal(&limited),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let mut accepted = 0;
    let failure = loop {
        match session
            .next()
            .expect("one work-unit-short allowance cannot finish the family")
        {
            Ok(answer) => {
                assert!(expected().contains(answer.interpretation()));
                accepted += 1;
            }
            Err(failure) => break failure,
        }
    };
    assert!(matches!(*failure.cause, SolveError::Reconstruction(_)));
    let outcome = failure.semantic().unwrap();
    assert_eq!(outcome.verified_models(), accepted);
    assert!(accepted < 4);
    assert_eq!(outcome.completion(), None);
    let receipt = outcome.terminal_execution().unwrap();
    assert_eq!(receipt.base_answers, accepted + 1);
    assert_eq!((receipt.reconstructed, receipt.pending), (accepted, 1));
    assert_eq!(receipt.reconstruction.completed, accepted);
    assert!(receipt.reconstruction.work < completed_work);
    assert!(session.next().is_none());
    assert_eq!(
        session.outcome().unwrap().terminal_execution(),
        Some(receipt)
    );
}

#[test]
fn base_observer_failure_preserves_original_identity_without_reconstruction() {
    struct RefuseBase;
    impl ExecutionObserver for RefuseBase {
        type Error = std::io::Error;
        fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
            if matches!(event, ExecutionObservation::Formula { .. }) {
                Err(std::io::Error::other("base observation refused"))
            } else {
                Ok(())
            }
        }
    }
    let owner = terminal(OPTIONAL);
    let mut session = Session::builder(
        PreparedInput::terminal(&owner),
        config(),
        Cancellation::default(),
    )
    .start_observed(&mut RefuseBase)
    .unwrap();
    let failure = session.next().unwrap().unwrap_err();
    assert!(matches!(
        *failure.cause,
        SolveError::ExecutionObservation(_)
    ));
    assert!(
        failure
            .subject()
            .unwrap()
            .same_instance(&Subject::TerminalDefinitions(owner.clone()))
    );
    let outcome = failure.semantic().unwrap();
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.verified_models(), 0);
    let receipt = outcome.terminal_execution().unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (0, 0, 0)
    );
    assert_eq!(receipt.reconstruction.attempts, 0);
    assert!(session.next().is_none());
}

#[test]
fn refused_terminal_base_grounding_keeps_its_composite_route() {
    for enabled in [false, true] {
        let measurements = SolveMeasurements::new(enabled);
        let observer = measurements.grounding_observer();
        let prepared = prepare_formula(
            OPTIONAL.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_support_rounds: 0,
                ..FormulaLimits::default()
            },
        )
        .unwrap();
        let failure = prepared
            .ground_adaptive_with_observer(
                observer
                    .as_ref()
                    .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver),
            )
            .unwrap_err();
        assert!(matches!(
            failure,
            FormulaFailure::Limit {
                resource: FormulaResource::SupportRounds,
                limit: 0,
                ..
            }
        ));
        if let Some(timings) = measurements.snapshot() {
            assert_eq!(
                timings.stages.grounding_mode,
                GroundingMode::EagerBaseTerminalDefinitions
            );
            assert!(timings.stages.get(SolveStage::Grounding).is_some());
            assert!(timings.stages.get(SolveStage::Solving).is_none());
            assert!(timings.get(SolvePhase::AnswerReconstruction).is_none());
        } else {
            assert!(!enabled);
        }
    }
}
