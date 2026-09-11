//! Physical formula execution preserves unrestricted family and capture evidence.
#![cfg(feature = "gpu")]

use std::{collections::BTreeSet, io, num::NonZeroUsize};

use zetesis_cpu::Control;
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, Grounder, Oracle, PreparedInput, Session, SolveConfig, SolveError, Subject,
    WorldViewError, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, Admitted, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_extended,
    admit_formula,
};
use zetesis_wgpu::{
    AdapterBackend, GpuBackendPreference, GpuContext, GpuErrorKind, GpuOptions, GpuSelection,
};

fn resources() -> ExecutionResources {
    let context = GpuContext::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend: GpuBackendPreference::Metal,
            vendor_id: None,
        },
    )
    .unwrap();
    assert_eq!(context.info().backend_kind(), AdapterBackend::Metal);
    assert!(context.info().is_hardware_gpu());
    eprintln!(
        "collection resources adapter={:?}",
        context.info().metadata()
    );
    ExecutionResources::with_gpu(&context)
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

fn choices() -> Admitted {
    admit_extended(
        "{a}. {b}. {c}. {d}. {e}. {f}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap()
}

fn lazy_config(backend: Backend) -> SolveConfig {
    SolveConfig {
        backend,
        oracle: Oracle::Closure,
        grounder: Grounder::Lazy,
        batch_size: NonZeroUsize::new(32).unwrap(),
        workers: NonZeroUsize::MIN,
        ..config()
    }
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Metal,
        oracle: Oracle::Countermodel,
        grounder: Grounder::Eager,
        models: 0,
        batch_size: NonZeroUsize::new(4).unwrap(),
        max_optimal_models: 0,
        max_optimal_atoms: 0,
        max_optimal_bytes: 0,
        ..Default::default()
    }
}

fn record(answer: &AnswerSet) -> (Vec<String>, Vec<(i32, i64)>) {
    // These fixtures use positive nullary atoms, so names retain full identity.
    let atoms = answer
        .interpretation()
        .atoms()
        .iter()
        .map(|atom| atom.predicate().name().to_owned())
        .collect();
    let score = answer.score().expect("the original objective is active");
    assert!(score.is_present());
    (atoms, score.costs().to_vec())
}

#[test]
#[ignore = "requires actual Metal; unrestricted collection never substitutes CPU"]
fn metal_world_view_preserves_nonoptimal_answers() {
    let owner = formula("1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}. #show.");
    let world_view = Session::builder(PreparedInput::formula(&owner), config(), Control::default())
        .resources(&resources())
        .selection(AnswerSelection::Optimal)
        .collect(WorldViewLimits::default())
        .unwrap();
    let answers: BTreeSet<_> = world_view.answer_sets().iter().map(record).collect();
    assert_eq!(
        answers,
        BTreeSet::from([
            (vec!["a".into()], vec![(2, 1)]),
            (vec!["b".into()], vec![(2, 2)]),
        ])
    );
    assert_eq!(world_view.len(), 2);
    let outcome = world_view.outcome();
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(outcome.verified_models(), 2);
    assert_eq!(outcome.scored_models(), 2);
    assert_eq!(outcome.retained_models(), 0);
    assert!(!outcome.optimum_proved());
    assert_eq!(
        outcome
            .countermodel_statistics()
            .unwrap()
            .candidate_restrictions,
        0
    );
    let execution = outcome
        .formula_execution()
        .expect("actual formula device execution");
    assert!(execution.adapter.contains("Metal"), "{}", execution.adapter);
    assert!(execution.gpu_batches > 0);
    assert!(execution.gpu_work > 0);
    assert_eq!(execution.gpu_candidates, 2);
    assert_eq!(execution.gpu_decided + execution.cpu_residuals, 2);
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 0)
    );
}

#[test]
#[ignore = "requires actual Metal; collection refusal preserves completed and queued work"]
fn metal_collection_limit_retains_checked_accounting() {
    let owner = formula("{a;b}. #minimize {1,a:a; 2,b:b}.");
    let failure = Session::builder(PreparedInput::formula(&owner), config(), Control::default())
        .resources(&resources())
        .collect(WorldViewLimits {
            max_answer_sets: 1,
            ..Default::default()
        })
        .unwrap_err();
    assert!(
        matches!(failure.cause(), WorldViewError::AnswerSets),
        "{failure:?}"
    );
    assert_eq!(failure.answer_sets().len(), 1);
    let expected = BTreeSet::from([
        (vec![], vec![(0, 0)]),
        (vec!["a".into()], vec![(0, 1)]),
        (vec!["b".into()], vec![(0, 2)]),
        (vec!["a".into(), "b".into()], vec![(0, 3)]),
    ]);
    assert!(expected.contains(&record(&failure.answer_sets()[0])));
    assert!(
        failure.answer_sets()[0]
            .subject()
            .same_instance(failure.subject())
    );
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.verified_models(), 4);
    assert_eq!(outcome.scored_models(), 2);
    assert_eq!(outcome.retained_models(), 0);
    assert!(!outcome.unsatisfiable());
    assert!(!outcome.optimum_proved());
    let execution = outcome
        .formula_execution()
        .expect("actual formula device execution");
    assert!(execution.adapter.contains("Metal"), "{}", execution.adapter);
    assert_eq!(execution.gpu_batches, 1);
    assert!(execution.gpu_work > 0);
    assert_eq!(execution.gpu_candidates, 4);
    assert_eq!(execution.gpu_decided + execution.cpu_residuals, 4);
    assert_eq!(
        (execution.pending_candidates, execution.queued_models),
        (0, 2)
    );
}

#[test]
#[ignore = "requires actual Metal; collection cannot replace a supplied context"]
fn metal_collection_refuses_a_foreign_context() {
    let owner = formula("a.");
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            backend: Backend::Vulkan,
            ..config()
        },
        Control::default(),
    )
    .resources(&resources())
    .collect(WorldViewLimits::default())
    .unwrap_err();
    assert!(matches!(failure.cause(), WorldViewError::Solve(error)
        if matches!(error.cause.as_ref(), SolveError::Gpu(cause)
            if cause.kind() == GpuErrorKind::AdapterRefused)));
    assert!(failure.answer_sets().is_empty());
    assert!(failure.outcome().is_none());
    assert!(
        failure
            .subject()
            .same_instance(&Subject::Theory(owner.theory().clone()))
    );
}

#[derive(Default)]
struct RefuseDeferredDevice {
    cpu_closures: usize,
    deferred: usize,
    refusals: usize,
}

impl ExecutionObserver for RefuseDeferredDevice {
    type Error = io::Error;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        assert_eq!(self.refusals, 0, "a refusal must stop subsequent callbacks");
        match observation {
            ExecutionObservation::CpuClosure { .. } => self.cpu_closures += 1,
            ExecutionObservation::DeferredDevice { .. } => self.deferred += 1,
            ExecutionObservation::DeviceClosure { adapter, .. } => {
                assert_eq!(adapter.backend, AdapterBackend::Metal);
                self.refusals += 1;
                return Err(io::Error::other("deferred collection observer refused"));
            }
            _ => {}
        }
        Ok(())
    }
}

#[test]
fn deferred_collection_fixture_starts_with_empty_answer() {
    let owner = choices();
    let world_view = Session::builder(
        PreparedInput::admitted(&owner),
        lazy_config(Backend::Cpu),
        Control::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap();
    assert!(
        world_view.answer_sets()[0]
            .interpretation()
            .atoms()
            .is_empty()
    );
}

#[test]
#[ignore = "requires actual Metal; a deferred observation fails after retaining the first answer"]
fn metal_collection_observer_failure_retains_a_prefix() {
    let owner = choices();
    let mut observer = RefuseDeferredDevice::default();
    let failure = Session::builder(
        PreparedInput::admitted(&owner),
        lazy_config(Backend::Auto),
        Control::default(),
    )
    .resources(&resources())
    .collect_observed(WorldViewLimits::default(), &mut observer)
    .unwrap_err();
    assert_eq!(observer.cpu_closures, 1);
    assert_eq!(observer.deferred, 1);
    assert_eq!(observer.refusals, 1);
    let WorldViewError::Solve(solve) = failure.cause() else {
        panic!("the deferred observer must stop collection: {failure:?}");
    };
    let SolveError::ExecutionObservation(cause) = solve.cause.as_ref() else {
        panic!("the external failure must not request CPU fallback: {failure:?}");
    };
    assert!(cause.downcast_ref::<io::Error>().is_some());
    assert_eq!(cause.to_string(), "deferred collection observer refused");
    assert_eq!(failure.answer_sets().len(), 1);
    let answer = &failure.answer_sets()[0];
    assert!(answer.interpretation().atoms().is_empty());
    assert!(answer.subject().same_instance(failure.subject()));
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.selection(), Some(AnswerSelection::All));
    assert!(!outcome.unsatisfiable());
}
