//! Physical formula execution preserves unrestricted family and capture evidence.

use std::{collections::BTreeSet, io, num::NonZeroUsize};

use zetesis_cpu::Cancellation;
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, GpuApi, Grounder, Oracle, PreparedInput, Session, SolveConfig, SolveError,
    Subject, WorldViewError, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, Admitted, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_extended,
    admit_formula,
};
use zetesis_wgpu::{AdapterBackend, GpuContext, GpuErrorKind, GpuOptions, GpuSelection};

/// A physical device on the requested API, and its adapter's own kind, so a
/// test names the device it ran on.
#[derive(Clone, Copy)]
struct Device {
    api: GpuApi,
    kind: AdapterBackend,
}

const METAL: Device = Device {
    api: GpuApi::Metal,
    kind: AdapterBackend::Metal,
};

const VULKAN: Device = Device {
    api: GpuApi::Vulkan,
    kind: AdapterBackend::Vulkan,
};

impl Device {
    /// The backend that names this device's API.
    const fn backend(self) -> Backend {
        Backend::Gpu(Some(self.api))
    }

    /// The other backend: a context a session on this one cannot take.
    const fn foreign(self) -> Backend {
        match self.api {
            GpuApi::Metal => Backend::Gpu(Some(GpuApi::Vulkan)),
            GpuApi::Vulkan => Backend::Gpu(Some(GpuApi::Metal)),
        }
    }
}

fn resources(device: Device) -> ExecutionResources {
    let context =
        GpuContext::new_selected(GpuOptions::default(), GpuSelection { api: device.api }).unwrap();
    assert_eq!(context.info().backend_kind(), device.kind);
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

fn lazy_config(backend: Backend, device: Device) -> SolveConfig {
    SolveConfig {
        backend,
        oracle: Oracle::Closure,
        grounder: Grounder::Lazy,
        batch_size: NonZeroUsize::new(32).unwrap(),
        workers: NonZeroUsize::MIN,
        ..config(device)
    }
}

fn config(device: Device) -> SolveConfig {
    SolveConfig {
        backend: device.backend(),
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
    world_view_preserves_nonoptimal_answers(METAL);
}

#[test]
#[ignore = "requires actual Vulkan; unrestricted collection never substitutes CPU"]
fn vulkan_world_view_preserves_nonoptimal_answers() {
    world_view_preserves_nonoptimal_answers(VULKAN);
}

fn world_view_preserves_nonoptimal_answers(device: Device) {
    let owner = formula("1 {a;b} 1. #minimize {1@2,a:a; 2@2,b:b}. #show.");
    let world_view = Session::builder(
        PreparedInput::formula(&owner),
        config(device),
        Cancellation::default(),
    )
    .resources(&resources(device))
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
    assert!(
        execution.adapter.contains(device.api.name()),
        "{}",
        execution.adapter
    );
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
    collection_limit_retains_checked_accounting(METAL);
}

#[test]
#[ignore = "requires actual Vulkan; collection refusal preserves completed and queued work"]
fn vulkan_collection_limit_retains_checked_accounting() {
    collection_limit_retains_checked_accounting(VULKAN);
}

fn collection_limit_retains_checked_accounting(device: Device) {
    let owner = formula("{a;b}. #minimize {1,a:a; 2,b:b}.");
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        config(device),
        Cancellation::default(),
    )
    .resources(&resources(device))
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
    assert!(
        execution.adapter.contains(device.api.name()),
        "{}",
        execution.adapter
    );
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
    collection_refuses_a_foreign_context(METAL);
}

#[test]
#[ignore = "requires actual Vulkan; collection cannot replace a supplied context"]
fn vulkan_collection_refuses_a_foreign_context() {
    collection_refuses_a_foreign_context(VULKAN);
}

fn collection_refuses_a_foreign_context(device: Device) {
    let owner = formula("a.");
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            backend: device.foreign(),
            ..config(device)
        },
        Cancellation::default(),
    )
    .resources(&resources(device))
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
struct CpuExecution {
    cpu_closures: usize,
}

impl ExecutionObserver for CpuExecution {
    type Error = io::Error;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match observation {
            ExecutionObservation::CpuClosure { .. } => self.cpu_closures += 1,
            ExecutionObservation::DeviceClosure { .. } => {
                panic!("a CPU session must keep its CPU route");
            }
            _ => {}
        }
        Ok(())
    }
}

#[test]
#[ignore = "requires actual Metal resources to verify a CPU session keeps its CPU route"]
fn metal_resources_leave_a_cpu_collection_on_the_cpu() {
    resources_leave_a_cpu_collection_on_the_cpu(METAL);
}

#[test]
#[ignore = "requires actual Vulkan resources to verify a CPU session keeps its CPU route"]
fn vulkan_resources_leave_a_cpu_collection_on_the_cpu() {
    resources_leave_a_cpu_collection_on_the_cpu(VULKAN);
}

fn resources_leave_a_cpu_collection_on_the_cpu(device: Device) {
    let owner = choices();
    let mut observer = CpuExecution::default();
    let world_view = Session::builder(
        PreparedInput::admitted(&owner),
        lazy_config(Backend::Cpu, device),
        Cancellation::default(),
    )
    .resources(&resources(device))
    .collect_observed(WorldViewLimits::default(), &mut observer)
    .unwrap();
    assert_eq!(observer.cpu_closures, 1);
    let actual: BTreeSet<Vec<String>> = world_view
        .answer_sets()
        .iter()
        .map(|answer| {
            answer
                .interpretation()
                .atoms()
                .iter()
                .map(|atom| atom.predicate().name().to_owned())
                .collect()
        })
        .collect();
    let expected = (0_u8..64)
        .map(|mask| {
            ["a", "b", "c", "d", "e", "f"]
                .into_iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, atom)| atom.to_owned())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(world_view.answer_sets().len(), 64);
}
