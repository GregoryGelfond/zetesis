//! Projected enumeration preserves full membership and original search evidence.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, Grounder, Oracle, PreparedInput, ProjectionError,
    ProjectionLimits, ProjectionResource, Session, SolveConfig, SolveError, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        oracle: Oracle::Countermodel,
        models: 0,
        workers: NonZeroUsize::new(1).unwrap(),
        completion_workers: NonZeroUsize::new(1).unwrap(),
        batch_size: NonZeroUsize::new(1).unwrap(),
        ..Default::default()
    }
}

#[test]
fn distinct_keys_retain_full_membership() {
    let owner = input("{p;q}. #project p/0.");
    let full = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap();
    assert_eq!(full.len(), 4);
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .selection(AnswerSelection::All)
    .projected(ProjectionLimits::default())
    .start()
    .unwrap();
    let mut keys = BTreeSet::new();
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        assert!(
            full.answer_sets()
                .iter()
                .any(|original| original.interpretation() == answer.interpretation())
        );
        assert!(answer.subject().same_instance(full.subject()));
        let key: Vec<_> = owner
            .projection()
            .atoms()
            .iter()
            .filter(|atom| answer.interpretation().contains(*atom))
            .collect();
        assert!(keys.insert(key));
    }
    assert_eq!(keys.len(), 2);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 4);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    let projection = outcome.projection().unwrap();
    assert_eq!((projection.representatives, projection.duplicates), (2, 2));
    assert!(projection.complete);
}

#[test]
fn world_view_collection_forces_full_identity() {
    let owner = input("{p;q}. #project p/0.");
    let family = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .projected(ProjectionLimits {
        max_keys: 0,
        ..Default::default()
    })
    .collect(WorldViewLimits::default())
    .unwrap();
    assert_eq!(family.len(), 4);
    assert!(family.outcome().projection().is_none());
}

#[test]
fn empty_projection_has_one_complete_class() {
    let owner = input("{p;q}. #project absent/0.");
    assert!(owner.projection().atoms().is_empty());
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .projected(ProjectionLimits {
        max_keys: 1,
        ..Default::default()
    })
    .start()
    .unwrap();
    assert_eq!(session.by_ref().map(Result::unwrap).count(), 1);
    let outcome = session.outcome().unwrap();
    let projection = outcome.projection().unwrap();
    assert_eq!(projection.duplicates, 3);
    assert!(projection.complete);
}

#[test]
fn objective_selection_precedes_projection() {
    let owner = input("{p;q}. #project absent/0. #minimize{1@1:not q}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .projected(ProjectionLimits::default())
    .start()
    .unwrap();
    let answers: Vec<_> = session.by_ref().map(Result::unwrap).collect();
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].score().unwrap().costs(), &[(1, 0)]);
    assert!(
        answers[0]
            .interpretation()
            .atoms()
            .iter()
            .any(|atom| atom.predicate().name() == "q")
    );
    let outcome = session.outcome().unwrap();
    assert!(outcome.optimum_proved());
    assert!(outcome.projection().unwrap().complete);
    assert_eq!(outcome.projection().unwrap().duplicates, 1);
}

#[test]
fn requested_count_counts_representatives() {
    let owner = input("{p;q}. #project p/0.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            models: 1,
            ..config()
        },
        Cancellation::default(),
    )
    .projected(ProjectionLimits::default())
    .start()
    .unwrap();
    assert_eq!(session.by_ref().map(Result::unwrap).count(), 1);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::RequestedModels));
    assert!(!outcome.projection().unwrap().complete);
}

#[test]
fn projected_stops_preserve_established_optimum() {
    let owner = input("{p;q}. #project p/0. #minimize{1@1:not q}.");
    for (models, keys, refused) in [(1, 2, false), (0, 0, true)] {
        let mut session = Session::builder(
            PreparedInput::formula(&owner),
            SolveConfig { models, ..config() },
            Cancellation::default(),
        )
        .projected(ProjectionLimits {
            max_keys: keys,
            ..Default::default()
        })
        .start()
        .unwrap();
        let answers: Result<Vec<_>, _> = session.by_ref().collect();
        match answers {
            Ok(answers) => {
                assert!(!refused);
                assert_eq!(answers.len(), 1);
                assert_eq!(answers[0].score().unwrap().costs(), &[(1, 0)]);
            }
            Err(failure) => {
                assert!(refused);
                assert!(matches!(
                    *failure.cause,
                    SolveError::Projection(ProjectionError::Limit {
                        resource: ProjectionResource::Keys,
                        ..
                    })
                ));
                let evidence = failure.semantic().unwrap();
                assert!(evidence.optimum_proved());
                assert_eq!(evidence.completion(), Some(Completion::Exhausted));
                assert!(!evidence.projection().unwrap().complete);
            }
        }
        let outcome = session.outcome().unwrap();
        assert!(outcome.optimum_proved());
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(
            outcome.projection().unwrap().representatives,
            usize::from(!refused)
        );
        assert!(!outcome.projection().unwrap().complete);
        assert!(session.next().is_none());
    }
}

#[test]
fn refused_key_preserves_verified_evidence() {
    let owner = input("{p;q}. #project p/0.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .projected(ProjectionLimits {
        max_keys: 0,
        ..Default::default()
    })
    .start()
    .unwrap();
    let error = session.next().unwrap().unwrap_err();
    assert!(matches!(
        *error.cause,
        SolveError::Projection(ProjectionError::Limit {
            resource: ProjectionResource::Keys,
            limit: 0,
            ..
        })
    ));
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert!(outcome.verified_models() > 0);
    assert_eq!(outcome.projection().unwrap().representatives, 0);
    assert!(!outcome.projection().unwrap().complete);
}

#[test]
fn projection_requires_an_explicit_domain() {
    let owner = input("{p}.");
    let error = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .projected(ProjectionLimits::default())
    .start()
    .err()
    .unwrap();
    assert!(matches!(
        *error.cause,
        SolveError::Projection(ProjectionError::MissingDeclaration)
    ));
}

#[test]
fn refused_history_setup_retains_attempted_timing() {
    let owner = input("{p}. #project p/0.");
    let error = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            stats: true,
            ..config()
        },
        Cancellation::default(),
    )
    .projected(ProjectionLimits {
        max_bytes: 0,
        ..Default::default()
    })
    .start()
    .err()
    .unwrap();
    assert!(matches!(
        *error.cause,
        SolveError::Projection(ProjectionError::Limit {
            resource: ProjectionResource::Bytes,
            limit: 0,
            ..
        })
    ));
    let timing = error.phase_timings.unwrap();
    assert!(timing.stages.is_complete());
    assert_eq!(
        timing
            .stages
            .get(zetesis_solve::SolveStage::Solving)
            .unwrap()
            .calls,
        1
    );
}

#[test]
fn cancellation_keeps_the_delivered_projected_prefix() {
    let owner = input("{p;q}. #project p/0.");
    let cancellation = Cancellation::default();
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        cancellation.clone(),
    )
    .projected(ProjectionLimits::default())
    .start()
    .unwrap();
    let first = session.next().unwrap().unwrap();
    cancellation.cancel();
    // The engine may observe the stop before the history does. Either route
    // must finish without delivering another representative or claiming coverage.
    assert!(!matches!(session.next(), Some(Ok(_))));
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert!(!outcome.projection().unwrap().complete);
    assert_eq!(outcome.projection().unwrap().representatives, 1);
    assert!(first.subject().same_instance(outcome.subject().unwrap()));
    assert!(!outcome.unsatisfiable());
}

#[test]
fn history_ceilings_are_inclusive() {
    let owner = input("{p;q}. #project p/0.");
    let run = |limits| {
        let mut session = Session::builder(
            PreparedInput::formula(&owner),
            config(),
            Cancellation::default(),
        )
        .projected(limits)
        .start()
        .unwrap();
        let answers: Result<Vec<_>, _> = session.by_ref().collect();
        (answers, session.outcome().unwrap())
    };
    let (_, baseline) = run(ProjectionLimits::default());
    let stats = baseline.projection().unwrap();
    for limits in [
        ProjectionLimits {
            max_keys: stats.representatives,
            ..Default::default()
        },
        ProjectionLimits {
            max_work: stats.work,
            ..Default::default()
        },
        ProjectionLimits {
            max_bytes: usize::try_from(stats.peak_bytes).unwrap(),
            ..Default::default()
        },
    ] {
        let (answers, outcome) = run(limits);
        assert_eq!(answers.unwrap().len(), 2);
        assert!(outcome.projection().unwrap().complete);
    }
    for limits in [
        ProjectionLimits {
            max_keys: stats.representatives - 1,
            ..Default::default()
        },
        ProjectionLimits {
            max_work: stats.work - 1,
            ..Default::default()
        },
        ProjectionLimits {
            max_bytes: usize::try_from(stats.peak_bytes).unwrap() - 1,
            ..Default::default()
        },
    ] {
        let (answers, outcome) = run(limits);
        assert!(answers.is_err());
        assert!(!outcome.projection().unwrap().complete);
    }
}
