//! One objective account covers preparation, score reads and refused prefixes.

use crate::optimization::Incumbents;
use crate::{Interruption, SolveConfig};
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_reference_support::formula as admitted;
use zetesis_themelios::objective_bound::{ObjectivePlan, ObjectivePlanLimits};

#[test]
fn exhausted_preparation_prevents_fallback_success() {
    let owner = admitted("{a;b}. #minimize {1,a:a;2,b:b}.");
    let cancellation = Cancellation::default();
    let limits = ObjectivePlanLimits {
        max_work: 1,
        ..Default::default()
    };
    let refused = ObjectivePlan::new(
        owner.theory(),
        owner.atom_catalog().atoms(),
        owner.objectives(),
        limits,
        &cancellation,
    )
    .unwrap_err();
    assert_eq!(refused.statistics().work, 1);
    let model = Model::from_positions(owner.atom_catalog(), []).unwrap();
    let mut incumbents = Incumbents::default();
    incumbents.prepare(refused.statistics().work).unwrap();
    let options = SolveConfig {
        max_objective_work: 1,
        ..Default::default()
    };
    let error = incumbents
        .evaluate(owner.objectives(), &model, &options, &cancellation, None)
        .unwrap_err();
    assert!(
        matches!(error, Interruption::Objective(error) if error.kind() == zetesis_objective::ErrorKind::Stopped(zetesis_objective::Stop::WorkLimit) && error.statistics().work == 0)
    );
    assert_eq!(incumbents.work, 1);
    assert_eq!(incumbents.scored(), 0);
    assert!(incumbents.metadata().is_none());
}

#[test]
fn score_cutoffs_include_preparation_once() {
    let owner = admitted("{a;b}. #minimize {1,a:a;2,b:b}.");
    let cancellation = Cancellation::default();
    let plan = ObjectivePlan::new(
        owner.theory(),
        owner.atom_catalog().atoms(),
        owner.objectives(),
        ObjectivePlanLimits::default(),
        &cancellation,
    )
    .unwrap();
    let candidate = Interpretation::new(owner.theory(), [0]).unwrap();
    let model = Model::from_positions(owner.atom_catalog(), candidate.atoms()).unwrap();
    let complete = plan
        .score(
            &candidate,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap()
        .unwrap();
    let preparation = plan.statistics().work;
    for remaining in 0..complete.work() {
        let mut incumbents = Incumbents::default();
        incumbents.prepare(preparation).unwrap();
        let options = SolveConfig {
            max_objective_work: preparation + remaining,
            ..Default::default()
        };
        let error = incumbents
            .evaluate(
                owner.objectives(),
                &model,
                &options,
                &cancellation,
                Some((&plan, &candidate)),
            )
            .unwrap_err();
        assert!(
            matches!(error, Interruption::PreparedObjective(error) if error.work() == remaining)
        );
        assert_eq!(incumbents.work, preparation + remaining);
        assert_eq!(incumbents.scored(), 0);
    }
    let mut exact = Incumbents::default();
    exact.prepare(preparation).unwrap();
    let options = SolveConfig {
        max_objective_work: preparation + complete.work(),
        ..Default::default()
    };
    let score = exact
        .evaluate(
            owner.objectives(),
            &model,
            &options,
            &cancellation,
            Some((&plan, &candidate)),
        )
        .unwrap();
    assert_eq!(score, complete.into_score());
    assert_eq!(exact.work, options.max_objective_work);
    assert_eq!(exact.scored(), 1);
}

#[test]
fn population_decline_uses_the_detailed_model_ceiling() {
    let owner = admitted("{a;b}. #minimize {1,a:a;2,b:b}.");
    let cancellation = Cancellation::default();
    let plan = ObjectivePlan::new(
        owner.theory(),
        owner.atom_catalog().atoms(),
        owner.objectives(),
        ObjectivePlanLimits::default(),
        &cancellation,
    )
    .unwrap();
    let candidate = Interpretation::new(owner.theory(), []).unwrap();
    let model = Model::from_positions(owner.atom_catalog(), []).unwrap();
    let limits = zetesis_objective::Limits {
        max_keys: 0,
        ..Default::default()
    };
    assert!(
        plan.score(&candidate, limits, &cancellation)
            .unwrap()
            .is_none()
    );
    let detailed =
        zetesis_objective::evaluate(owner.objectives(), &model, limits, &cancellation).unwrap();
    let mut incumbents = Incumbents::default();
    let options = SolveConfig {
        max_objective_keys: 0,
        ..Default::default()
    };
    let score = incumbents
        .evaluate(
            owner.objectives(),
            &model,
            &options,
            &cancellation,
            Some((&plan, &candidate)),
        )
        .unwrap();
    assert_eq!(score, *detailed.score());
    assert_eq!(incumbents.work, detailed.statistics().work);
}
