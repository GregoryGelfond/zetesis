//! Pruning controls must not discard reusable objective eligibility.

use super::super::{Bounds, Preparation};
use crate::SolveConfig;
use crate::countermodel::Input;
use crate::execution_observation::Ignore;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_reference_support::formula as admitted;
use zetesis_themelios::AdmittedFormula;

fn input(owner: &AdmittedFormula) -> Input<'_> {
    Input {
        theory: owner.theory(),
        atoms: owner.atom_catalog(),
        gate_atoms: 0,
        keyed_constraints: 0,
        key_analysis: zetesis_themelios::KeyAnalysis::Complete,
        objectives: owner.objectives(),
        required_choices: owner.required_choices(),
        certificate_order: zetesis_sat::CertificateOrder::TightFirst,
    }
}

fn check_reuse(bound_work: u64) {
    let owner = admitted("{a;b}. #minimize {1,a:a;2,b:b}.");
    let options = SolveConfig {
        max_objective_bound_work: bound_work,
        models: 0,
        ..Default::default()
    };
    let cancellation = Cancellation::default();
    let preparation = Preparation::new(input(&owner), &options, &cancellation);
    let plan = preparation.plan().unwrap();
    let mut bounds = Bounds::new(&options);
    let mut incumbents = crate::optimization::Incumbents::default();
    incumbents.prepare(preparation.work()).unwrap();
    let mut search = zetesis_sat::StableModels::new(
        owner.theory(),
        zetesis_sat::Limits::default(),
        cancellation.clone(),
    )
    .unwrap();
    let mut expected_work = preparation.work();
    for selected in [vec![0, 1], vec![0]] {
        let candidate = Interpretation::new(owner.theory(), selected).unwrap();
        let model = Model::from_positions(owner.atom_catalog(), candidate.atoms()).unwrap();
        let prepared = plan
            .score(
                &candidate,
                zetesis_objective::Limits::default(),
                &cancellation,
            )
            .unwrap()
            .unwrap();
        expected_work += prepared.work();
        let detailed = zetesis_objective::evaluate(
            owner.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        assert_eq!(prepared.into_score(), *detailed.score());
        assert!(
            incumbents
                .consider(
                    owner.objectives(),
                    model,
                    &options,
                    &cancellation,
                    Some((plan, &candidate))
                )
                .unwrap()
        );
        assert_eq!(incumbents.metadata().unwrap().work, expected_work);
        bounds
            .improve(
                Some(plan),
                incumbents.score().unwrap(),
                &mut search,
                &options,
                &mut Ignore,
                &cancellation,
            )
            .unwrap();
        assert!(!bounds.enabled);
        assert!(preparation.plan().is_some());
    }
    assert_eq!(incumbents.scored(), 2);
    assert_eq!(search.statistics().candidate_restrictions, 0);
    assert_eq!(bounds.work, bound_work);
}

#[test]
fn disabled_pruning_retains_prepared_scoring() {
    check_reuse(0);
}

#[test]
fn refused_pruning_retains_prepared_scoring() {
    check_reuse(1);
}

#[test]
fn preparation_refusals_keep_the_actual_prefix() {
    let owner = admitted("{a;b}. #minimize {1,a:a;2,b:b}.");
    let cancellation = Cancellation::default();
    let complete = Preparation::new(input(&owner), &SolveConfig::default(), &cancellation);
    assert!(complete.plan().is_some());
    for cutoff in 1..complete.work() {
        let options = SolveConfig {
            max_objective_work: cutoff,
            ..Default::default()
        };
        let attempted = Preparation::new(input(&owner), &options, &cancellation);
        assert!(attempted.plan().is_none());
        let error = attempted.refusal.unwrap();
        assert_eq!(attempted.work(), error.statistics().work);
        assert!(attempted.work() <= cutoff);
        assert!(matches!(
            error.kind(),
            zetesis_themelios::objective_bound::ObjectiveBoundErrorKind::Limit(
                zetesis_themelios::objective_bound::ObjectiveBoundResource::Work
            )
        ));
    }
    let exact = Preparation::new(
        input(&owner),
        &SolveConfig {
            max_objective_work: complete.work(),
            ..Default::default()
        },
        &cancellation,
    );
    assert!(exact.plan().is_some());
    assert_eq!(exact.work(), complete.work());
}

#[test]
fn zero_scoring_work_skips_optional_preparation() {
    let owner = admitted("{a}. #minimize {1:a}.");
    let preparation = Preparation::new(
        input(&owner),
        &SolveConfig {
            max_objective_work: 0,
            ..Default::default()
        },
        &Cancellation::default(),
    );
    assert_eq!(preparation.work(), 0);
    assert!(preparation.plan().is_none());
    assert!(preparation.refusal.is_none());
}

#[test]
fn choice_refusal_retains_exact_scoring() {
    let owner = admitted(include_str!(
        "../../../tests/fixtures/required-choice-objective.lp"
    ));
    let cancellation = Cancellation::default();
    let mut preparation = Preparation::new(input(&owner), &SolveConfig::default(), &cancellation);
    let exact_work = preparation.work();
    let options = SolveConfig {
        max_objective_work: exact_work + 1,
        ..Default::default()
    };
    assert!(owner.required_choices().is_some());
    preparation.prepare_choices(owner.required_choices(), &options, &cancellation);
    assert!(preparation.refusal.is_none());
    let refusal = preparation.choice_refusal.unwrap();
    assert_eq!(preparation.work(), exact_work + refusal.statistics().work);
    assert!(preparation.work() <= options.max_objective_work);
    let plan = preparation.plan().unwrap();
    assert_eq!(plan.choice_bound_groups(), 0);

    let position = owner
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == "a")
        .unwrap();
    let candidate = Interpretation::new(owner.theory(), [position]).unwrap();
    let model = Model::from_positions(owner.atom_catalog(), candidate.atoms()).unwrap();
    let expected = zetesis_objective::evaluate(
        owner.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &cancellation,
    )
    .unwrap();
    let score = plan
        .score(
            &candidate,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap()
        .unwrap();
    assert_eq!(score.into_score(), *expected.score());
    let bound = plan
        .bound(
            expected.score(),
            zetesis_themelios::objective_bound::ObjectiveBoundLimits::default(),
            &cancellation,
        )
        .unwrap();
    assert!(bound.original().same_instance(owner.theory()));
}

#[test]
fn disabled_bounds_skip_choice_preparation() {
    let owner = admitted(include_str!(
        "../../../tests/fixtures/required-choice-objective.lp"
    ));
    let options = SolveConfig {
        max_objective_bound_work: 0,
        ..Default::default()
    };
    let cancellation = Cancellation::default();
    let mut preparation = Preparation::new(input(&owner), &options, &cancellation);
    let exact_work = preparation.work();
    assert!(owner.required_choices().is_some());
    preparation.prepare_choices(owner.required_choices(), &options, &cancellation);
    assert_eq!(preparation.work(), exact_work);
    assert_eq!(preparation.plan().unwrap().choice_bound_groups(), 0);
    assert!(preparation.choice_refusal.is_none());
}
