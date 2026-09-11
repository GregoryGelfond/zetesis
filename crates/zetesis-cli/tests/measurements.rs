//! Shared host scopes preserve attempts independently of application outcomes.

use std::time::Duration;
use zetesis_cli::{
    GroundingOutcome, GroundingPhase, GroundingWork, SolveMeasurements, SolvePhase, SolveStage,
};
use zetesis_themelios::GroundingObserver as _;

#[test]
fn clones_retain_the_original_measurements() {
    let owner = SolveMeasurements::new(true);
    owner.measure(SolvePhase::AdmissionMaterialization, || ());
    let shared = owner.clone();
    drop(owner);
    shared.measure(SolvePhase::ExecutionSetup, || ());
    let snapshot = shared.snapshot().unwrap();
    for phase in [
        SolvePhase::AdmissionMaterialization,
        SolvePhase::ExecutionSetup,
    ] {
        assert_eq!(snapshot.get(phase).unwrap().calls, 1);
    }
    let independent = SolveMeasurements::new(true);
    assert!(
        independent
            .snapshot()
            .unwrap()
            .get(SolvePhase::AdmissionMaterialization)
            .is_none()
    );
}

#[test]
fn disabled_measurements_remain_absent() {
    let owner = SolveMeasurements::new(false);
    assert!(!owner.is_enabled());
    assert_eq!(
        owner.measure(SolvePhase::ExecutionSetup, || Err::<(), _>(17)),
        Err(17)
    );
    let shared = owner.clone();
    let stage = shared.stage(SolveStage::Solving);
    let phase = owner.enter(SolvePhase::CandidateSetup);
    assert!(owner.grounding_observer().is_none());
    assert!(shared.snapshot().is_none());
    drop(phase);
    drop(stage);
    assert!(owner.snapshot().is_none());
}

#[test]
fn returned_errors_close_the_measured_attempt() {
    let owner = SolveMeasurements::new(true);
    assert!(owner.is_enabled());
    assert_eq!(
        owner.measure(SolvePhase::ExecutionSetup, || Err::<(), _>(17)),
        Err(17)
    );
    let snapshot = owner.snapshot().unwrap();
    assert_eq!(snapshot.get(SolvePhase::ExecutionSetup).unwrap().calls, 1);
    assert!(snapshot.get(SolvePhase::CandidateSetup).is_none());
}

#[test]
fn unwinding_closes_the_measured_attempt() {
    let owner = SolveMeasurements::new(true);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        owner.measure(SolvePhase::CandidateSetup, || {
            panic!("injected operation failure")
        });
    }));
    assert!(result.is_err());
    assert_eq!(
        owner
            .snapshot()
            .unwrap()
            .get(SolvePhase::CandidateSetup)
            .unwrap()
            .calls,
        1
    );
}

#[test]
fn cloned_stage_guards_share_exclusive_accounting() {
    let owner = SolveMeasurements::new(true);
    let shared = owner.clone();
    let parent = owner.stage(SolveStage::SourcePreparation);
    let child = shared.stage(SolveStage::Grounding);
    assert!(owner.snapshot().unwrap().stages.is_complete());
    drop(child);
    drop(parent);
    let stages = shared.snapshot().unwrap().stages;
    assert!(stages.is_complete());
    for stage in [SolveStage::SourcePreparation, SolveStage::Grounding] {
        assert_eq!(stages.get(stage).unwrap().calls, 1);
    }
}

#[test]
fn grounding_observations_reach_the_shared_owner() {
    let owner = SolveMeasurements::new(true);
    let shared = owner.clone();
    let observer = shared.grounding_observer().unwrap();
    observer.enter();
    observer.phase_enter(GroundingPhase::RuleInstantiation, None);
    observer.phase_exit(
        GroundingPhase::RuleInstantiation,
        None,
        GroundingOutcome::Failed,
        GroundingWork::default(),
    );
    observer.exit();
    let snapshot = owner.snapshot().unwrap();
    assert_eq!(
        snapshot
            .grounding
            .get(GroundingPhase::RuleInstantiation)
            .unwrap()
            .count(GroundingOutcome::Failed),
        Some(1)
    );
    assert_eq!(snapshot.stages.get(SolveStage::Grounding).unwrap().calls, 1);
}

#[test]
fn editing_a_snapshot_does_not_mutate_the_recorder() {
    let owner = SolveMeasurements::new(true);
    owner.measure(SolvePhase::ExecutionSetup, || ());
    let mut detached = owner.snapshot().unwrap();
    detached.driver_elapsed = Duration::MAX;
    detached.stages.driver_elapsed = Duration::MAX;
    assert!(!detached.stages.is_complete());
    let current = owner.snapshot().unwrap();
    assert_ne!(current.driver_elapsed, Duration::MAX);
    assert!(current.stages.is_complete());
    assert_eq!(current.get(SolvePhase::ExecutionSetup).unwrap().calls, 1);
}
