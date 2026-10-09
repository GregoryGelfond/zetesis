//! Device core membership composes with host acceptance and reconstruction.

use std::{collections::BTreeMap, convert::Infallible, num::NonZeroUsize};
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, GpuApi, Grounder, Oracle, PreparedInput, SearchMethod, SemanticOutcome,
    Session, SolveConfig, SolveError,
};
use zetesis_themelios::{
    AdmissionOptions, ConstraintCheckLimits, ExpansionLimits, FormulaLimits,
    FormulaMaterialization, HybridFormula, prepare_formula,
};
use zetesis_wgpu::{AdapterBackend, GpuContext, GpuOptions, GpuSelection};

type Family = BTreeMap<Model, Option<Score>>;

fn hybrid(source: &str) -> HybridFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn config(api: GpuApi, oracle: Oracle) -> SolveConfig {
    SolveConfig {
        backend: Backend::Gpu(Some(api)),
        grounder: Grounder::Lazy,
        oracle,
        models: 0,
        batch_size: NonZeroUsize::new(4).unwrap(),
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        ..SolveConfig::default()
    }
}

fn resources(api: GpuApi) -> ExecutionResources {
    let context = GpuContext::new_selected(GpuOptions::default(), GpuSelection { api }).unwrap();
    assert_eq!(
        context.info().backend_kind(),
        match api {
            GpuApi::Metal => AdapterBackend::Metal,
            GpuApi::Vulkan => AdapterBackend::Vulkan,
        }
    );
    assert!(context.info().is_hardware_gpu());
    eprintln!("hybrid adapter={:?}", context.info().metadata());
    ExecutionResources::with_gpu(&context)
}

#[derive(Default)]
struct Routes {
    tight: usize,
    general: usize,
    cpu: usize,
}
impl ExecutionObserver for Routes {
    type Error = Infallible;
    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::DeviceTight { .. } => self.tight += 1,
            ExecutionObservation::DeviceFormula { .. } => self.general += 1,
            ExecutionObservation::CpuFormula { .. } | ExecutionObservation::CpuClosure { .. } => {
                self.cpu += 1;
            }
            _ => {}
        }
        Ok(())
    }
}

fn capture(
    input: PreparedInput<'_>,
    config: SolveConfig,
    resources: &ExecutionResources,
    selection: AnswerSelection,
) -> (Family, SemanticOutcome, Routes) {
    let mut routes = Routes::default();
    let mut session = Session::builder(input, config, Cancellation::default())
        .resources(resources)
        .selection(selection)
        .start_observed(&mut routes)
        .unwrap();
    let mut family = Family::new();
    while let Some(answer) = session.next_observed(&mut routes) {
        let answer = answer.unwrap();
        assert!(
            family
                .insert(answer.interpretation().clone(), answer.score().cloned())
                .is_none()
        );
    }
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    (family, outcome, routes)
}

fn complete_scored_families(api: GpuApi) {
    let source = include_str!("../fixtures/hybrid-objectives/priorities.lp");
    let owner = hybrid(source);
    let original = zetesis_reference_support::formula(source);
    let resources = resources(api);
    for selection in [AnswerSelection::All, AnswerSelection::Optimal] {
        let (expected, _, _) = capture(
            PreparedInput::formula(&original),
            SolveConfig {
                backend: Backend::Cpu,
                grounder: Grounder::Eager,
                ..config(api, Oracle::Auto)
            },
            &resources,
            selection,
        );
        assert_eq!(
            expected.len(),
            if selection == AnswerSelection::All {
                6
            } else {
                2
            }
        );
        for oracle in [Oracle::Auto, Oracle::Countermodel] {
            for (search, workers) in [(SearchMethod::Clauses, 1), (SearchMethod::Regions, 2)] {
                let (actual, outcome, routes) = capture(
                    PreparedInput::hybrid(&owner),
                    SolveConfig {
                        search,
                        workers: NonZeroUsize::new(workers).unwrap(),
                        ..config(api, oracle)
                    },
                    &resources,
                    selection,
                );
                assert_eq!(actual, expected);
                assert_eq!(routes.cpu, 0);
                assert_eq!(
                    (routes.tight, routes.general),
                    if oracle == Oracle::Auto {
                        (1, 0)
                    } else {
                        (0, 1)
                    }
                );
                let receipt = outcome.hybrid_execution().unwrap();
                assert_eq!(receipt.core_answers, receipt.accepted + receipt.rejected);
                assert_eq!(receipt.accepted, outcome.verified_models());
                assert_eq!(receipt.pending, 0);
                assert!(outcome.formula_execution().unwrap().gpu_candidates > 0);
                if selection == AnswerSelection::Optimal {
                    assert!(outcome.optimum_proved());
                    assert_eq!(outcome.incumbent().unwrap().tied_models, 2);
                } else if search == SearchMethod::Clauses {
                    assert_eq!(
                        (receipt.core_answers, receipt.accepted, receipt.rejected),
                        (8, 6, 2)
                    );
                }
            }
        }
    }
    rejected_cheapest_answer(api, &resources);
}

fn rejected_cheapest_answer(api: GpuApi, resources: &ExecutionResources) {
    let owner = hybrid(include_str!(
        "../fixtures/hybrid-objectives/cheap-invalid.lp"
    ));
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        let (answers, outcome, _) = capture(
            PreparedInput::hybrid(&owner),
            SolveConfig {
                search: SearchMethod::Clauses,
                ..config(api, oracle)
            },
            resources,
            AnswerSelection::Optimal,
        );
        assert_eq!(answers.len(), 1);
        assert_eq!(
            answers[&zetesis_test_support::programs::model(&["p"])]
                .as_ref()
                .unwrap()
                .costs(),
            [(0, 1)]
        );
        assert!(outcome.optimum_proved());
        let receipt = outcome.hybrid_execution().unwrap();
        assert_eq!(
            (receipt.accepted, receipt.rejected, receipt.pending),
            (1, 1, 0)
        );
    }
}

fn source_failure(api: GpuApi) {
    let owner = hybrid(include_str!("../fixtures/hybrid-gpu/source-failure.lp"));
    let prepared = owner
        .checker(ConstraintCheckLimits::default())
        .unwrap()
        .statistics();
    let resources = resources(api);
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        let mut session = Session::builder(
            PreparedInput::hybrid(&owner),
            SolveConfig {
                search: SearchMethod::Clauses,
                constraints: ConstraintCheckLimits {
                    max_work: prepared.work,
                    ..Default::default()
                },
                ..config(api, oracle)
            },
            Cancellation::default(),
        )
        .resources(&resources)
        .selection(AnswerSelection::Optimal)
        .start()
        .unwrap();
        let failure = session.next().unwrap().unwrap_err();
        assert!(matches!(failure.cause.as_ref(), SolveError::Constraint(_)));
        let outcome = session.outcome().unwrap();
        let receipt = outcome.hybrid_execution().unwrap();
        assert_eq!(
            (
                receipt.core_answers,
                receipt.accepted,
                receipt.rejected,
                receipt.pending
            ),
            (1, 0, 0, 1)
        );
        assert!(outcome.formula_execution().unwrap().gpu_candidates > 0);
        assert_eq!((outcome.verified_models(), outcome.scored_models()), (0, 0));
        assert!(!outcome.optimum_proved());
        assert!(!outcome.unsatisfiable());
        assert_ne!(outcome.completion(), Some(Completion::Exhausted));
        assert!(session.next().is_none());
    }
}

fn cancelled_queue(api: GpuApi) {
    let owner = hybrid(include_str!(
        "../fixtures/hybrid-gpu/queued-cancellation.lp"
    ));
    let resources = resources(api);
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        let cancellation = Cancellation::default();
        let mut session = Session::builder(
            PreparedInput::hybrid(&owner),
            SolveConfig {
                search: SearchMethod::Clauses,
                ..config(api, oracle)
            },
            cancellation.clone(),
        )
        .resources(&resources)
        .selection(AnswerSelection::All)
        .start()
        .unwrap();
        assert!(session.next().unwrap().is_ok());
        assert!(
            session
                .progress()
                .formula_execution()
                .unwrap()
                .queued_models
                > 0
        );
        cancellation.cancel();
        assert!(session.next().is_none());
        let outcome = session.outcome().unwrap();
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(outcome.verified_models(), 1);
        let receipt = outcome.hybrid_execution().unwrap();
        assert_eq!((receipt.accepted, receipt.pending), (1, 0));
        assert!(!outcome.unsatisfiable());
        assert!(session.next().is_none());
    }
}

fn terminal_families(api: GpuApi) {
    let source = include_str!("../fixtures/hybrid-gpu/terminal.lp");
    let FormulaMaterialization::Terminal(owner) = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_lazy()
    .unwrap() else {
        panic!("expected terminal partition")
    };
    let original = zetesis_reference_support::formula(source);
    let resources = resources(api);
    let (expected, _, _) = capture(
        PreparedInput::formula(&original),
        SolveConfig {
            backend: Backend::Cpu,
            grounder: Grounder::Eager,
            ..config(api, Oracle::Auto)
        },
        &resources,
        AnswerSelection::All,
    );
    assert_eq!(expected.len(), 2);
    assert!(
        expected.contains_key(
            &Model::new([
                zetesis_test_support::programs::nullary("a"),
                zetesis_test_support::programs::unary("d", 1),
                zetesis_test_support::programs::unary("d", 2),
                zetesis_test_support::programs::unary("t", 1),
                zetesis_test_support::programs::unary("t", 2),
            ])
            .unwrap()
        )
    );
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        let (actual, outcome, routes) = capture(
            PreparedInput::terminal(&owner),
            SolveConfig {
                search: SearchMethod::Clauses,
                ..config(api, oracle)
            },
            &resources,
            AnswerSelection::All,
        );
        assert_eq!(actual, expected);
        assert_eq!(routes.cpu, 0);
        assert_eq!(routes.tight + routes.general, 1);
        let core = outcome.hybrid_execution().unwrap();
        let terminal = outcome.terminal_execution().unwrap();
        assert_eq!((core.accepted, core.rejected, core.pending), (2, 2, 0));
        assert_eq!(
            (
                terminal.base_answers,
                terminal.reconstructed,
                terminal.pending
            ),
            (2, 2, 0)
        );
        assert_eq!(outcome.verified_models(), 2);
        assert!(outcome.formula_execution().unwrap().gpu_candidates > 0);
    }
}

#[test]
#[ignore = "requires Metal: hybrid device checking preserves full answers and optimal ties"]
fn metal_hybrid_preserves_complete_scored_families() {
    complete_scored_families(GpuApi::Metal);
}
#[test]
#[ignore = "requires Vulkan: hybrid device checking preserves full answers and optimal ties"]
fn vulkan_hybrid_preserves_complete_scored_families() {
    complete_scored_families(GpuApi::Vulkan);
}
#[test]
#[ignore = "requires Metal: an unfinished source check cannot publish a core answer"]
fn metal_hybrid_source_failure_preserves_pending_acceptance() {
    source_failure(GpuApi::Metal);
}
#[test]
#[ignore = "requires Vulkan: an unfinished source check cannot publish a core answer"]
fn vulkan_hybrid_source_failure_preserves_pending_acceptance() {
    source_failure(GpuApi::Vulkan);
}
#[test]
#[ignore = "requires Metal: cancellation stops queued core answers before publication"]
fn metal_hybrid_cancellation_stops_queued_core_models() {
    cancelled_queue(GpuApi::Metal);
}
#[test]
#[ignore = "requires Vulkan: cancellation stops queued core answers before publication"]
fn vulkan_hybrid_cancellation_stops_queued_core_models() {
    cancelled_queue(GpuApi::Vulkan);
}
#[test]
#[ignore = "requires Metal: terminal reconstruction follows completed source acceptance"]
fn metal_hybrid_reconstructs_only_accepted_answers() {
    terminal_families(GpuApi::Metal);
}
#[test]
#[ignore = "requires Vulkan: terminal reconstruction follows completed source acceptance"]
fn vulkan_hybrid_reconstructs_only_accepted_answers() {
    terminal_families(GpuApi::Vulkan);
}
