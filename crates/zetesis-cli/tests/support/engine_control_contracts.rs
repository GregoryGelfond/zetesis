//! A stopped batch produces a typed stop before CPU oracle execution.

use std::time::Instant;

use clap::Parser;
use zetesis_core::Seed;
use zetesis_cpu::{Control, Stop};
use zetesis_themelios::{AdmissionOptions, admit};

use super::Engine;
use crate::presentation::Diagnostics;
use crate::{ColorMode, Options};

#[test]
fn automatic_lazy_policy_permits_device_discovery() {
    let admitted = admit("a.".into(), AdmissionOptions::default()).unwrap();
    for grounder in ["auto", "lazy", "eager"] {
        let options =
            Options::try_parse_from(["zetesis", "--grounder", grounder, "--workers", "1"]).unwrap();
        let engine = Engine::new(
            &(&options).into(),
            admitted.program(),
            &mut Diagnostics::new(Vec::new(), ColorMode::Never),
            &crate::phase_timing::Recorder::new(false),
        )
        .unwrap();
        assert_eq!(engine.automatic, cfg!(feature = "gpu"));
        assert!(!engine.attempted_gpu);
        assert_eq!(engine.executor.ground().is_some(), grounder == "eager");
    }
}

#[test]
fn retired_device_work_excludes_cpu_queue_entries() {
    let admitted = admit("a.".into(), AdmissionOptions::default()).unwrap();
    let options = Options::try_parse_from(["zetesis", "--backend", "cpu"]).unwrap();
    let mut engine = Engine::new(
        &(&options).into(),
        admitted.program(),
        &mut Diagnostics::new(Vec::new(), ColorMode::Never),
        &crate::phase_timing::Recorder::new(false),
    )
    .unwrap();
    // A reporting fixture represents a failed attempt, not physical execution.
    let prior = crate::lazy_execution::tests::fixture();
    engine.retired_lazy_statistics = Some(prior.clone());
    let observed = engine.lazy_statistics(32).unwrap();
    assert_eq!(observed.queued_results, 0);
    assert_eq!(observed.completed_candidates, prior.completed_candidates);
    assert_eq!(observed.stopped_candidates, prior.stopped_candidates);
    assert_eq!(observed.dispatches, prior.dispatches);
}

#[test]
fn collected_seeds_observe_cancellation_and_deadline_before_any_oracle() {
    let admitted = admit("p.".into(), AdmissionOptions::default()).unwrap();
    let program = admitted.program();
    let seed = Seed::new(program, []).unwrap();
    for grounder in ["lazy", "eager"] {
        let options = Options::try_parse_from([
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--grounder",
            grounder,
        ])
        .unwrap();
        let mut diagnostics = Vec::new();
        let mut engine = Engine::new(
            &(&options).into(),
            program,
            &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
            &crate::phase_timing::Recorder::new(false),
        )
        .unwrap();
        let initial = diagnostics.clone();
        let cancelled = Control::default();
        cancelled.cancel();
        for (control, reason) in [
            (cancelled, Stop::Cancelled),
            (Control::with_deadline(Instant::now()), Stop::Deadline),
        ] {
            let batch = engine
                .check(
                    &(&options).into(),
                    program,
                    &[seed.clone(), seed.clone()],
                    &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
                    &control,
                    &crate::phase_timing::Recorder::new(false),
                )
                .unwrap();
            assert_eq!(batch, vec![Err(reason)]);
            assert_eq!(
                diagnostics, initial,
                "no scheduling or fallback after a stop"
            );
        }
        let batch = engine
            .check(
                &(&options).into(),
                program,
                &[seed.clone(), seed.clone()],
                &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
                &Control::default(),
                &crate::phase_timing::Recorder::new(false),
            )
            .unwrap();
        assert_eq!(batch.len(), 2);
        for result in batch {
            let model = result
                .unwrap()
                .expect("the unchanged fact remains derivable");
            assert_eq!(model.atoms().len(), 1);
            assert_eq!(model.atoms().iter().next().unwrap().predicate().name(), "p");
        }
    }
}

#[test]
fn eager_static_cache_reuse_does_not_record_a_second_materialization() {
    let admitted = admit("p.".into(), AdmissionOptions::default()).unwrap();
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--workers",
        "1",
        "--grounder",
        "eager",
    ])
    .unwrap();
    let phases = crate::phase_timing::Recorder::new(true);
    let first = super::Executor::cpu(
        &(&options).into(),
        admitted.program(),
        None,
        &mut Diagnostics::new(Vec::new(), ColorMode::Never),
        &phases,
    )
    .unwrap();
    let ground = first.ground().unwrap();
    let second = super::Executor::cpu(
        &(&options).into(),
        admitted.program(),
        Some(ground.clone()),
        &mut Diagnostics::new(Vec::new(), ColorMode::Never),
        &phases,
    )
    .unwrap();
    assert!(std::sync::Arc::ptr_eq(&ground, &second.ground().unwrap()));
    assert_eq!(
        phases
            .snapshot()
            .unwrap()
            .stages
            .get(crate::SolveStage::Grounding)
            .unwrap()
            .calls,
        1
    );
}
