//! Each construction has an allowance; cumulative receipts retain incomplete work.

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_cpu::{Cancellation, Stop};
use zetesis_reference_support::formula;
use zetesis_solve::{
    Backend, Completion, Grounder, Interruption, ModelConstructionStop, PreparedInput, Session,
    SolveConfig, SolvePhase,
};
use zetesis_test_support::programs::model;

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        models: 0,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        max_objective_bound_work: 0,
        stats: true,
        ..SolveConfig::default()
    }
}

#[test]
fn one_order_serves_each_complete_formula_family() {
    let owner = formula("{a;b}.");
    let expected = BTreeSet::from([model(&[]), model(&["a"]), model(&["b"]), model(&["a", "b"])]);
    let mut receipts = Vec::new();
    for _ in 0..2 {
        let mut session = Session::new(
            PreparedInput::formula(&owner),
            config(),
            Cancellation::default(),
        )
        .unwrap();
        let prepared = *session.progress().model_construction().unwrap();
        assert!(prepared.prepared_bytes > 0);
        assert!(prepared.work > 0);
        assert_eq!(prepared.constructed, 0);
        let mut answers = BTreeSet::new();
        for answer in session.by_ref() {
            let answer = answer.unwrap();
            assert!(
                answer
                    .interpretation()
                    .catalog()
                    .shares_snapshot(owner.atom_catalog())
            );
            answers.insert(answer.interpretation().clone());
        }
        let outcome = session.outcome().unwrap();
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        let receipt = *outcome.model_construction().unwrap();
        assert_eq!(receipt.constructed, 4);
        assert_eq!(receipt.prepared_bytes, prepared.prepared_bytes);
        assert!(receipt.work > prepared.work);
        assert!(receipt.peak_bytes >= prepared.peak_bytes);
        assert_eq!(
            session
                .phase_timings()
                .unwrap()
                .get(SolvePhase::ModelConstruction)
                .unwrap()
                .calls,
            5
        );
        drop(session);
        assert_eq!(answers, expected);
        receipts.push(receipt);
    }
    assert_eq!(receipts[0], receipts[1]);
}

#[test]
fn construction_limits_refuse_before_candidate_setup() {
    let owner = formula("a | b.");
    for limits in [
        SolveConfig {
            max_model_work: 0,
            ..config()
        },
        SolveConfig {
            max_model_bytes: 0,
            ..config()
        },
    ] {
        let mut session = Session::new(
            PreparedInput::formula(&owner),
            limits,
            Cancellation::default(),
        )
        .unwrap();
        assert!(session.next().is_none());
        let outcome = session.outcome().unwrap();
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(outcome.verified_models(), 0);
        assert!(!outcome.unsatisfiable());
        assert!(outcome.countermodel_statistics().is_none());
        let receipt = outcome.model_construction().unwrap();
        assert_eq!(receipt.prepared_bytes, 0);
        assert_eq!(receipt.constructed, 0);
        match outcome.interruption().unwrap() {
            Interruption::ModelConstruction(ModelConstructionStop::Work { observed, limit }) => {
                assert_eq!((observed, limit, receipt.work), (1, 0, 0));
            }
            Interruption::ModelConstruction(ModelConstructionStop::Bytes { required, limit }) => {
                assert_eq!(limit, 0);
                assert!(required > 0);
            }
            other => panic!("expected construction refusal, got {other:?}"),
        }
        let phases = session.phase_timings().unwrap();
        assert_eq!(phases.get(SolvePhase::ModelConstruction).unwrap().calls, 1);
        assert!(phases.get(SolvePhase::CandidateSetup).is_none());
        assert!(phases.get(SolvePhase::ExecutionSetup).is_none());
    }
}

#[test]
fn each_model_receives_its_own_work_allowance() {
    let owner = formula("{a;b;c;d}.");
    let mut complete = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let mut prior_work = complete.progress().model_construction().unwrap().work;
    let mut allowance = prior_work;
    let mut expected = BTreeSet::new();
    while let Some(answer) = complete.next() {
        expected.insert(answer.unwrap().interpretation().clone());
        let work = complete.progress().model_construction().unwrap().work;
        allowance = allowance.max(work - prior_work);
        prior_work = work;
    }
    assert_eq!(expected.len(), 16);
    let receipt = *complete.outcome().unwrap().model_construction().unwrap();
    assert!(receipt.work > allowance);
    let mut limited = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_model_work: allowance,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    let actual = limited
        .by_ref()
        .map(|answer| answer.unwrap().interpretation().clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    let outcome = limited.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.model_construction(), Some(&receipt));
    assert_eq!(receipt.constructed, 16);
}

#[test]
fn preparation_work_allowance_is_inclusive() {
    let owner = formula("{a}.");
    let complete = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let prepared = *complete.progress().model_construction().unwrap();
    assert!(prepared.work > 0);
    let exact = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_model_work: prepared.work,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert_eq!(exact.progress().model_construction(), Some(&prepared));
    let limit = prepared.work - 1;
    let mut below = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_model_work: limit,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert!(below.next().is_none());
    let outcome = below.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(outcome.model_construction().unwrap().work, limit);
    assert_eq!(outcome.model_construction().unwrap().prepared_bytes, 0);
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::ModelConstruction(
            ModelConstructionStop::Work {
                observed: u128::from(prepared.work),
                limit,
            }
        ))
    );
    assert!(
        below
            .phase_timings()
            .unwrap()
            .get(SolvePhase::CandidateSetup)
            .is_none()
    );
}

#[test]
fn construction_refusal_preserves_the_checked_prefix() {
    let owner = formula("{a}.");
    let mut complete = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let prepared = complete.progress().model_construction().unwrap().work;
    let expected_first = complete.next().unwrap().unwrap();
    let first_work = complete.progress().model_construction().unwrap().work - prepared;
    let expected_second = complete.next().unwrap().unwrap();
    assert!(complete.next().is_none());
    assert_eq!(expected_first.interpretation(), &model(&[]));
    assert_eq!(expected_second.interpretation(), &model(&["a"]));
    let second_work = complete
        .outcome()
        .unwrap()
        .model_construction()
        .unwrap()
        .work
        - prepared
        - first_work;
    let work_limit = second_work - 1;
    // Order preparation and the empty answer fit; selecting a has more work.
    assert!(prepared <= work_limit && first_work <= work_limit);
    let mut limited = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_model_work: work_limit,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    let first = limited.next().unwrap().unwrap();
    assert_eq!(first.interpretation(), expected_first.interpretation());
    assert!(limited.next().is_none());
    let outcome = limited.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 2);
    assert!(
        matches!(outcome.interruption(), Some(Interruption::ModelConstruction(
        ModelConstructionStop::Work { observed, limit }
    )) if observed == u128::from(work_limit) + 1 && limit == work_limit)
    );
    let receipt = outcome.model_construction().unwrap();
    assert_eq!(receipt.work, prepared + first_work + work_limit);
    assert_eq!(receipt.constructed, 1);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(!outcome.unsatisfiable());
    assert!(limited.next().is_none());
    assert_eq!(first.interpretation(), expected_first.interpretation());
}

#[test]
fn construction_refusal_drains_prior_incumbents() {
    let owner = formula("{a}. #minimize{1,a:a}.");
    let mut complete = Session::enumerate(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let prepared = complete.progress().model_construction().unwrap().work;
    let expected = complete.next().unwrap().unwrap();
    let first_work = complete.progress().model_construction().unwrap().work - prepared;
    let second = complete.next().unwrap().unwrap();
    assert!(complete.next().is_none());
    assert_eq!(expected.interpretation(), &model(&[]));
    assert_eq!(second.interpretation(), &model(&["a"]));
    let second_work = complete
        .outcome()
        .unwrap()
        .model_construction()
        .unwrap()
        .work
        - prepared
        - first_work;
    let work_limit = second_work - 1;
    assert!(prepared <= work_limit && first_work <= work_limit);
    let mut limited = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_model_work: work_limit,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    let retained = limited.next().unwrap().unwrap();
    assert_eq!(retained.interpretation(), expected.interpretation());
    assert_eq!(retained.score(), expected.score());
    assert!(limited.next().is_none());
    let outcome = limited.outcome().unwrap();
    assert_eq!(
        (
            outcome.verified_models(),
            outcome.scored_models(),
            outcome.retained_models()
        ),
        (2, 1, 1)
    );
    let receipt = outcome.model_construction().unwrap();
    assert_eq!(receipt.work, prepared + first_work + work_limit);
    assert_eq!(receipt.constructed, 1);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::ModelConstruction(
            ModelConstructionStop::Work { .. }
        ))
    ));
    assert!(!outcome.optimum_proved());
}

#[test]
fn construction_storage_refusal_keeps_verified_membership() {
    let owner = formula("p(1..16).");
    let mut complete = Session::new(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .unwrap();
    let prepared_peak = complete.progress().model_construction().unwrap().peak_bytes;
    assert!(complete.next().unwrap().is_ok());
    assert!(complete.next().is_none());
    let peak = complete
        .outcome()
        .unwrap()
        .model_construction()
        .unwrap()
        .peak_bytes;
    assert!(
        peak > prepared_peak,
        "the full selection must require more than order preparation"
    );
    let limit = usize::try_from(peak - 1).unwrap();
    let mut limited = Session::new(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_model_bytes: limit,
            ..config()
        },
        Cancellation::default(),
    )
    .unwrap();
    assert!(
        limited
            .progress()
            .model_construction()
            .unwrap()
            .prepared_bytes
            > 0
    );
    assert!(limited.next().is_none());
    let outcome = limited.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.model_construction().unwrap().constructed, 0);
    assert!(
        matches!(outcome.interruption(), Some(Interruption::ModelConstruction(
        ModelConstructionStop::Bytes { required, limit: actual }
    )) if actual == limit && required > limit as u128)
    );
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(!outcome.unsatisfiable());
}

#[test]
fn pre_cancelled_sessions_prepare_no_model_order() {
    let owner = formula("a | b.");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut session = Session::new(PreparedInput::formula(&owner), config(), cancellation).unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Preparation(Stop::Cancelled))
    );
    assert!(outcome.model_construction().is_none());
    assert!(
        session
            .phase_timings()
            .unwrap()
            .get(SolvePhase::ModelConstruction)
            .is_none()
    );
}
