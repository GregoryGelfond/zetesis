//! Authenticated preparation shares exact scoring and one attempted-work account.

use super::super::{Preparation, plan_limits};
use crate::countermodel::Input;
use crate::execution_observation::{Ignore, Observer};
use crate::{ExecutionObservation, ExecutionObserver, SolveConfig, SolveError};
use std::io;
use zetesis_algebra::Cancellation;
use zetesis_core::Model;
use zetesis_ferraris::Interpretation;
use zetesis_reference_support::formula;
use zetesis_themelios::AdmittedFormula;
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundResource, ObjectivePlan, ObjectiveProjectionError,
    ObjectiveProjectionErrorKind,
};

fn input(owner: &AdmittedFormula) -> Input<'_> {
    Input {
        theory: owner.theory(),
        atoms: owner.atom_catalog(),
        keyed_constraints: 0,
        key_analysis: zetesis_themelios::KeyAnalysis::Complete,
        objectives: owner.objectives(),
        objective_source: Some(owner.objective_source()),
        required_choices: owner.required_choices(),
        certificate_order: zetesis_stable::CertificateOrder::TightFirst,
    }
}

#[test]
fn projected_preparation_charges_the_direct_constructor_once() {
    let owner = formula("unused(1..12). {a}. #minimize {2,a:a}.");
    let options = SolveConfig::default();
    let cancellation = Cancellation::default();
    let expected = ObjectivePlan::new_projected(
        owner.objective_source(),
        plan_limits(&options, 0),
        options.max_candidate_bytes as u128,
        &cancellation,
    )
    .into_parts()
    .0
    .unwrap();
    let prepared = Preparation::new(input(&owner), &options, &cancellation);
    assert_eq!(prepared.work(), expected.statistics().work);
    assert!(prepared.refusal.is_none());
    assert!(prepared.projection_refusal.is_none());
    let plan = prepared.plan().unwrap();
    for include_a in [false, true] {
        let positions = owner
            .atoms()
            .iter()
            .enumerate()
            .filter_map(|(position, atom)| {
                (atom.predicate().name() != "a" || include_a).then_some(position)
            });
        let candidate = Interpretation::new(owner.theory(), positions).unwrap();
        let model = Model::from_positions(owner.atom_catalog(), candidate.atoms()).unwrap();
        let actual = plan
            .score(
                &candidate,
                zetesis_objective::Limits::default(),
                &cancellation,
            )
            .unwrap()
            .unwrap();
        let detailed = zetesis_objective::evaluate(
            owner.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        assert_eq!(actual.into_score(), *detailed.score());
    }
}

#[test]
fn projected_byte_refusal_keeps_its_distinct_reason() {
    let owner = formula("{a}. #minimize {1:a}.");
    let options = SolveConfig {
        max_candidate_bytes: 0,
        ..Default::default()
    };
    let prepared = Preparation::new(input(&owner), &options, &Cancellation::default());
    assert!(prepared.plan().is_none());
    assert!(prepared.refusal.is_none());
    let error = prepared.projection_refusal.unwrap();
    assert!(
        matches!(error.kind(), ObjectiveProjectionErrorKind::Bytes { required, limit: 0 } if *required > 0)
    );
    assert_eq!(prepared.work(), error.statistics().work);
}

#[test]
fn projected_refusal_keeps_detailed_scores_and_performed_work() {
    let owner = formula("{a;b}. :- a,b. #minimize {1,a:a;2,b:b}.");
    let options = SolveConfig {
        max_objective_keys: 1,
        ..Default::default()
    };
    let cancellation = Cancellation::default();
    let prepared = Preparation::new(input(&owner), &options, &cancellation);
    assert!(prepared.plan().is_none());
    assert!(prepared.refusal.is_none());
    let error = prepared.projection_refusal.unwrap();
    assert_eq!(
        error.kind(),
        &ObjectiveProjectionErrorKind::Existing(ObjectiveBoundErrorKind::Limit(
            ObjectiveBoundResource::Keys
        ))
    );
    assert!(error.statistics().work > 0);
    let mut incumbents = crate::optimization::Incumbents::default();
    incumbents.prepare(prepared.work()).unwrap();
    let mut expected_work = prepared.work();
    for names in [&[][..], &["a"][..], &["b"][..]] {
        let positions = owner
            .atoms()
            .iter()
            .enumerate()
            .filter_map(|(position, atom)| {
                names.contains(&atom.predicate().name()).then_some(position)
            });
        let model = Model::from_positions(owner.atom_catalog(), positions).unwrap();
        let expected = zetesis_objective::evaluate(
            owner.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        let actual = incumbents
            .evaluate(owner.objectives(), &model, &options, &cancellation, None)
            .unwrap();
        expected_work += expected.statistics().work;
        assert_eq!(actual, *expected.score());
        assert_eq!(incumbents.work(), expected_work);
    }
    assert_eq!(incumbents.scored(), 3);
}

#[test]
fn projected_choice_refusal_retains_the_exact_plan() {
    let owner = formula(include_str!(
        "../../../tests/fixtures/required-choice-objective.lp"
    ));
    let cancellation = Cancellation::default();
    let mut prepared = Preparation::new(input(&owner), &SolveConfig::default(), &cancellation);
    let exact_work = prepared.work();
    let identity = prepared.plan().unwrap().identity();
    let options = SolveConfig {
        max_objective_work: exact_work + 1,
        ..Default::default()
    };
    assert!(owner.required_choices().is_some());
    prepared.prepare_choices(owner.required_choices(), &options, &cancellation);
    let refusal = prepared.choice_refusal.unwrap();
    assert_eq!(prepared.work(), exact_work + refusal.statistics().work);
    assert!(prepared.projection_refusal.is_none());
    assert_eq!(prepared.plan().unwrap().identity(), identity);
    assert_eq!(prepared.plan().unwrap().choice_bound_groups(), 0);
}

#[derive(Default)]
struct RefuseObservation(Option<ObjectiveProjectionError>);

impl ExecutionObserver for RefuseObservation {
    type Error = io::Error;
    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::ObjectiveProjectionUnavailable(error) = observation {
            self.0 = Some(error);
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "projection diagnostic closed",
            ));
        }
        Ok(())
    }
}

#[test]
fn projected_refusal_survives_observer_failure_in_the_session_receipt() {
    use crate::formula_execution::Execution;
    use crate::formula_session::FormulaSession;
    use crate::phase_timing::Recorder;
    let owner = formula("{a;b}. :- a,b. #minimize {1,a:a;2,b:b}.");
    let options = SolveConfig {
        max_objective_keys: 1,
        max_objective_bound_work: 0,
        workers: std::num::NonZeroUsize::MIN,
        ..Default::default()
    };
    let cancellation = Cancellation::default();
    let phases = Recorder::new(false);
    let mut diagnostic = RefuseObservation::default();
    let mut session = FormulaSession::with_selection(
        input(&owner),
        Execution::Cpu,
        &options,
        &mut Observer(&mut diagnostic),
        &cancellation,
        &phases,
        crate::AnswerSelection::All,
    );
    let expected = diagnostic.0.unwrap().statistics().work;
    assert!(expected > 0);
    assert_eq!(session.outcome(&phases).objective_work(), expected);
    assert_eq!(session.outcome(&phases).scored_models(), 0);
    assert!(matches!(
        session.next(&options, &mut Ignore, &cancellation, &phases),
        Some(Err(SolveError::ExecutionObservation(_)))
    ));
    assert_eq!(session.outcome(&phases).objective_work(), expected);
}

#[test]
fn cancelled_projected_preparation_keeps_a_typed_stop() {
    let owner = formula("{a}. #minimize {1:a}.");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let prepared = Preparation::new(input(&owner), &SolveConfig::default(), &cancellation);
    let error = prepared.projection_refusal.unwrap();
    assert_eq!(
        error.kind(),
        &ObjectiveProjectionErrorKind::Existing(ObjectiveBoundErrorKind::Control(
            zetesis_algebra::Stop::Cancelled
        ))
    );
    assert!(prepared.plan().is_none());
    assert!(prepared.refusal.is_none());
    assert_eq!(prepared.work(), error.statistics().work);
}
