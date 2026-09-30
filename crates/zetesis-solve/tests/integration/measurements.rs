//! Shared host scopes preserve attempts independently of application outcomes.

use crate::support::sessions::config as cpu_config;
use std::{sync::mpsc, thread, time::Duration};
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    Completion, ExecutionObservation, ExecutionObserver, GroundingOutcome, GroundingPhase,
    GroundingWork, PreparedInput, Session, SolveConfig, SolveMeasurements, SolvePhase, SolveStage,
};
use zetesis_themelios::GroundingObserver as _;
use zetesis_themelios::{AdmissionOptions, admit};

const WAIT: Duration = Duration::from_secs(5);

#[test]
fn sessions_and_shared_measurements_retain_their_thread_bounds() {
    fn requires_send<T: Send>() {}
    fn requires_send_sync<T: Send + Sync>() {}
    requires_send::<Session<'static>>();
    requires_send_sync::<SolveMeasurements>();
}

#[test]
fn a_default_cpu_session_can_move_to_another_thread() {
    let input = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let mut session = Session::new(
        PreparedInput::admitted(&input),
        cpu_config(),
        Cancellation::default(),
    )
    .unwrap();
    thread::scope(|scope| {
        scope
            .spawn(move || {
                let answers = session.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
                assert_eq!(answers.len(), 1);
                assert_eq!(answers[0].interpretation().atoms().len(), 1);
                let outcome = session.outcome().unwrap();
                assert_eq!(outcome.completion(), Some(Completion::Exhausted));
                assert_eq!(outcome.verified_models(), 1);
                assert!(session.phase_timings().is_none());
            })
            .join()
            .unwrap();
    });
}

#[test]
fn injected_measurements_move_with_the_session_and_choose_its_policy() {
    let input = admit("a.".into(), AdmissionOptions::default()).unwrap();
    for enabled in [false, true] {
        let owner = SolveMeasurements::new(enabled);
        let mut session = Session::builder(
            PreparedInput::admitted(&input),
            SolveConfig {
                stats: !enabled,
                ..cpu_config()
            },
            Cancellation::default(),
        )
        .measurements(&owner)
        .start()
        .unwrap();
        thread::scope(|scope| {
            scope
                .spawn(move || {
                    let answers = session.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
                    assert_eq!(answers.len(), 1);
                    assert_eq!(
                        session.outcome().unwrap().completion(),
                        Some(Completion::Exhausted)
                    );
                    assert_eq!(session.phase_timings().is_some(), enabled);
                })
                .join()
                .unwrap();
        });
        assert_eq!(owner.snapshot().is_some(), enabled);
        if let Some(snapshot) = owner.snapshot() {
            assert_eq!(snapshot.get(SolvePhase::ExecutionSetup).unwrap().calls, 1);
            assert!(snapshot.stages.is_complete());
        }
    }
}

#[test]
fn concurrent_clones_retain_each_phase_attempt_without_locking_the_action() {
    let owner = SolveMeasurements::new(true);
    let (ready, arrivals) = mpsc::channel();
    thread::scope(|scope| {
        let mut releases = Vec::new();
        let mut workers = Vec::new();
        for _ in 0..2 {
            let shared = owner.clone();
            let ready = ready.clone();
            let (release, resume) = mpsc::channel();
            releases.push(release);
            workers.push(scope.spawn(move || {
                shared.measure(SolvePhase::ExecutionSetup, || {
                    ready.send(()).unwrap();
                    resume.recv_timeout(WAIT).unwrap();
                });
            }));
        }
        arrivals.recv_timeout(WAIT).unwrap();
        arrivals.recv_timeout(WAIT).unwrap();
        assert!(
            owner
                .snapshot()
                .unwrap()
                .get(SolvePhase::ExecutionSetup)
                .is_none()
        );
        for release in releases {
            release.send(()).unwrap();
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
    let measurement = owner
        .snapshot()
        .unwrap()
        .get(SolvePhase::ExecutionSetup)
        .unwrap();
    assert_eq!(measurement.calls, 2);
    assert!(!measurement.overflowed);
}

#[test]
fn nested_measurement_and_snapshot_can_reenter_the_same_owner() {
    let (completed, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        let owner = SolveMeasurements::new(true);
        owner.measure(SolvePhase::ExecutionSetup, || {
            assert!(
                owner
                    .snapshot()
                    .unwrap()
                    .get(SolvePhase::ExecutionSetup)
                    .is_none()
            );
            owner.measure(SolvePhase::CandidateSetup, || {
                assert!(owner.snapshot().is_some());
            });
        });
        completed.send(owner.snapshot().unwrap()).unwrap();
    });
    let snapshot = result.recv_timeout(WAIT).unwrap();
    worker.join().unwrap();
    for phase in [SolvePhase::ExecutionSetup, SolvePhase::CandidateSetup] {
        assert_eq!(snapshot.get(phase).unwrap().calls, 1);
    }
}

#[test]
fn session_observations_can_snapshot_and_measure_their_shared_owner() {
    struct Reentrant {
        measurements: SolveMeasurements,
        events: u64,
    }
    impl ExecutionObserver for Reentrant {
        type Error = std::convert::Infallible;

        fn observe(&mut self, _: ExecutionObservation<'_>) -> Result<(), Self::Error> {
            assert!(self.measurements.snapshot().unwrap().stages.is_complete());
            self.measurements
                .measure(SolvePhase::ObservationOutput, || {
                    assert!(self.measurements.snapshot().unwrap().stages.is_complete());
                });
            self.events += 1;
            Ok(())
        }
    }

    let (completed, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        let input = admit("a.".into(), AdmissionOptions::default()).unwrap();
        let owner = SolveMeasurements::new(true);
        let mut observer = Reentrant {
            measurements: owner.clone(),
            events: 0,
        };
        let mut session = Session::builder(
            PreparedInput::admitted(&input),
            cpu_config(),
            Cancellation::default(),
        )
        .measurements(&owner)
        .start_observed(&mut observer)
        .unwrap();
        while let Some(answer) = session.next_observed(&mut observer) {
            answer.unwrap();
        }
        assert_eq!(
            session.outcome().unwrap().completion(),
            Some(Completion::Exhausted)
        );
        assert_eq!(session.outcome().unwrap().verified_models(), 1);
        completed
            .send((observer.events, owner.snapshot().unwrap()))
            .unwrap();
    });
    let (events, snapshot) = result.recv_timeout(WAIT).unwrap();
    worker.join().unwrap();
    assert!(events > 0);
    assert_eq!(
        snapshot.get(SolvePhase::ObservationOutput).unwrap().calls,
        events
    );
    assert!(snapshot.stages.is_complete());
}

#[test]
fn cross_thread_stage_overlap_leaves_attribution_unavailable() {
    let owner = SolveMeasurements::new(true);
    let (ready, arrivals) = mpsc::channel();
    thread::scope(|scope| {
        let mut releases = Vec::new();
        let mut workers = Vec::new();
        for stage in [SolveStage::SourcePreparation, SolveStage::Solving] {
            let shared = owner.clone();
            let ready = ready.clone();
            let (release, resume) = mpsc::channel();
            releases.push(release);
            workers.push(scope.spawn(move || {
                let _stage = shared.stage(stage);
                ready.send(()).unwrap();
                resume.recv_timeout(WAIT).unwrap();
            }));
        }
        arrivals.recv_timeout(WAIT).unwrap();
        arrivals.recv_timeout(WAIT).unwrap();
        let snapshot = owner.snapshot().unwrap();
        assert!(snapshot.stages.unattributed.is_none());
        assert!(!snapshot.stages.is_complete());
        for release in releases {
            release.send(()).unwrap();
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
    let snapshot = owner.snapshot().unwrap();
    assert!(snapshot.stages.unattributed.is_none());
    assert!(!snapshot.stages.is_complete());
    for stage in [SolveStage::SourcePreparation, SolveStage::Solving] {
        assert_eq!(snapshot.stages.get(stage).unwrap().calls, 1);
    }
}

#[test]
fn concurrent_grounding_observers_retain_independent_phase_tokens() {
    let owner = SolveMeasurements::new(true);
    let (ready, arrivals) = mpsc::channel();
    let attempts = [
        (
            GroundingPhase::SupportCompletion,
            GroundingOutcome::Completed,
            3,
        ),
        (
            GroundingPhase::RuleInstantiation,
            GroundingOutcome::Failed,
            7,
        ),
    ];
    thread::scope(|scope| {
        let mut releases = Vec::new();
        let mut workers = Vec::new();
        for (phase, outcome, rows) in attempts {
            let shared = owner.clone();
            let ready = ready.clone();
            let (release, resume) = mpsc::channel();
            releases.push(release);
            workers.push(scope.spawn(move || {
                let observer = shared.grounding_observer().unwrap();
                observer.enter();
                observer.phase_enter(phase, None);
                ready.send(()).unwrap();
                resume.recv_timeout(WAIT).unwrap();
                let mut work = GroundingWork::default();
                work.join_rows = Some(rows);
                observer.phase_exit(phase, None, outcome, work);
                observer.exit();
            }));
        }
        arrivals.recv_timeout(WAIT).unwrap();
        arrivals.recv_timeout(WAIT).unwrap();
        for release in releases {
            release.send(()).unwrap();
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
    let snapshot = owner.snapshot().unwrap();
    for (phase, outcome, rows) in attempts {
        let measurement = snapshot.grounding.get(phase).unwrap();
        assert_eq!(measurement.count(outcome), Some(1));
        assert_eq!(measurement.work.join_rows, Some(rows));
        assert!(measurement.elapsed.is_some());
    }
    assert!(snapshot.stages.unattributed.is_none());
}

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
