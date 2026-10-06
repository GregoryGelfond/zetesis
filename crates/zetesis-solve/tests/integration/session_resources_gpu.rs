//! Ordinary sessions share device ownership, never search or answer-set evidence.

use std::{convert::Infallible, num::NonZeroUsize};

use crate::support::sessions::normal;
use zetesis_core::{
    Atom, Model, Predicate, Sign, Value,
    relation::{Limits, Relation},
};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_reference_support::formula;
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, GpuApi, Grounder, Interruption, Oracle, PreparedInput, SemanticOutcome,
    Session, SolveConfig, SolveError, Subject,
};
use zetesis_test_support::programs::signed as atom;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits};
use zetesis_wgpu::{
    AdapterBackend, AdapterCategory, GateProjection, GpuContext, GpuError, GpuErrorKind,
    GpuFormulaProfile, GpuOptions, GpuRelationExecutor, GpuSelection, RelationGpuLimits,
};

/// The physical device a test runs on, reached through one API.
trait Physical: Copy {
    fn backend(self) -> Backend;
    fn observed(self) -> AdapterBackend;
    fn selection(self) -> GpuSelection;
    fn other(self) -> Self;
    fn context(self) -> GpuContext;
}

impl Physical for GpuApi {
    fn backend(self) -> Backend {
        Backend::Gpu(Some(self))
    }

    fn observed(self) -> AdapterBackend {
        match self {
            Self::Metal => AdapterBackend::Metal,
            Self::Vulkan => AdapterBackend::Vulkan,
        }
    }

    fn selection(self) -> GpuSelection {
        GpuSelection { api: self }
    }

    fn other(self) -> Self {
        match self {
            Self::Metal => Self::Vulkan,
            Self::Vulkan => Self::Metal,
        }
    }

    fn context(self) -> GpuContext {
        let context = GpuContext::new_selected(GpuOptions::default(), self.selection()).unwrap();
        assert_eq!(context.info().backend_kind(), self.observed());
        assert!(context.info().is_hardware_gpu());
        eprintln!("session resources adapter={:?}", context.info().metadata());
        context
    }
}

#[derive(Clone, Copy)]
enum Profile {
    Eager,
    Lazy,
    Formula,
}

fn config(backend: Backend, profile: Profile) -> SolveConfig {
    SolveConfig {
        backend,
        grounder: match profile {
            Profile::Lazy => Grounder::Lazy,
            Profile::Eager | Profile::Formula => Grounder::Eager,
        },
        oracle: match profile {
            Profile::Formula => Oracle::Countermodel,
            Profile::Eager | Profile::Lazy => Oracle::Closure,
        },
        models: 0,
        batch_size: NonZeroUsize::new(4).unwrap(),
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        max_candidates: 128,
        max_search_work: 1_000_000,
        max_search_decisions: 1_000,
        max_work: 100_000,
        max_atoms: 256,
        max_optimal_models: 8,
        ..Default::default()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Record {
    atoms: Model,
    costs: Option<Vec<(i32, i64)>>,
}

impl Record {
    fn expected(choice: &str, value: Value, cost: Option<i64>) -> Self {
        let mut atoms = vec![
            atom(choice, Sign::Positive, vec![]),
            atom("value", Sign::Positive, vec![value]),
            atom("tag", Sign::Negative, vec![Value::String("7".into())]),
        ];
        atoms.sort();
        Self {
            atoms: Model::new(atoms).unwrap(),
            costs: cost.map(|cost| vec![(2, cost)]),
        }
    }

    fn from_answer(answer: &AnswerSet) -> Self {
        let atoms = answer.interpretation().clone();
        Self {
            atoms,
            costs: answer.score().map(|score| {
                assert!(score.is_present());
                score.costs().to_vec()
            }),
        }
    }
}

#[derive(Default)]
struct Routes {
    device_closure: usize,
    device_formula: usize,
    device_tight: usize,
    proposal_workers: Option<NonZeroUsize>,
    cpu_closure: usize,
    cpu_formula: usize,
    observed_backend: Option<AdapterBackend>,
    observed_projection: Option<GateProjection>,
}

impl ExecutionObserver for Routes {
    type Error = Infallible;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        let adapter = match observation {
            ExecutionObservation::ParallelProposals { workers } => {
                self.proposal_workers = Some(workers);
                None
            }
            ExecutionObservation::DeviceTight { adapter, .. } => {
                self.device_tight += 1;
                Some(adapter)
            }
            ExecutionObservation::DeviceClosure { adapter, .. } => {
                self.device_closure += 1;
                Some(adapter)
            }
            ExecutionObservation::DeviceFormula {
                adapter,
                projection,
                ..
            } => {
                self.device_formula += 1;
                self.observed_projection = Some(projection);
                Some(adapter)
            }
            ExecutionObservation::CpuClosure { .. } => {
                self.cpu_closure += 1;
                None
            }
            ExecutionObservation::CpuFormula { .. } => {
                self.cpu_formula += 1;
                None
            }
            _ => None,
        };
        if let Some(adapter) = adapter {
            assert!(matches!(
                adapter.category,
                AdapterCategory::IntegratedGpu | AdapterCategory::DiscreteGpu
            ));
            self.observed_backend = Some(adapter.backend);
        }
        Ok(())
    }
}

struct Capture {
    records: Vec<Record>,
    outcome: SemanticOutcome,
    routes: Routes,
}

fn solve(
    input: PreparedInput<'_>,
    subject: &Subject,
    config: SolveConfig,
    resources: &ExecutionResources,
    selection: AnswerSelection,
) -> Capture {
    let mut routes = Routes::default();
    let mut session = Session::builder(input, config, Cancellation::default())
        .resources(resources)
        .selection(selection)
        .start_observed(&mut routes)
        .unwrap();
    let mut records = Vec::new();
    while let Some(answer) = session.next_observed(&mut routes) {
        let answer = answer.unwrap();
        assert!(answer.subject().same_instance(subject));
        records.push(Record::from_answer(&answer));
    }
    assert!(session.next_observed(&mut routes).is_none());
    let outcome = session.outcome().unwrap();
    assert!(outcome.subject().unwrap().same_instance(subject));
    records.sort();
    Capture {
        records,
        outcome,
        routes,
    }
}

fn require_device(capture: &Capture, device: GpuApi, profile: Profile) {
    assert_eq!(capture.routes.observed_backend, Some(device.observed()));
    assert_eq!(capture.routes.cpu_closure + capture.routes.cpu_formula, 0);
    match profile {
        Profile::Eager => assert_eq!(capture.routes.device_closure, 1),
        Profile::Lazy => {
            assert_eq!(capture.routes.device_closure, 1);
            assert!(capture.outcome.lazy_execution().unwrap().dispatches > 0);
        }
        Profile::Formula => {
            assert_eq!(capture.routes.device_formula, 1);
            let execution = capture.outcome.formula_execution().unwrap();
            assert!(execution.gpu_batches > 0);
            assert!(execution.gpu_work > 0);
            assert!(execution.gpu_candidates > 0);
            assert_eq!(execution.pending_candidates + execution.queued_models, 0);
        }
    }
}

fn require_complete(capture: &Capture, expected: &[Record]) {
    assert_eq!(capture.outcome.completion(), Some(Completion::Exhausted));
    assert!(capture.outcome.interruption().is_none());
    assert_eq!(capture.records, expected);
    assert!(!capture.outcome.unsatisfiable());
}

const NORMAL_NUMBER: &str = "a :- not b. b :- not a. value(7). -tag(\"7\"). #show.";
const NORMAL_STRING: &str = "a :- not b. b :- not a. value(\"7\"). -tag(\"7\"). #show.";
const FORMULA_FIRST: &str = "1 {a;b} 1. value(7). -tag(\"7\"). #minimize {1@2,a:a;2@2,b:b}. #show.";
const FORMULA_SECOND: &str =
    "1 {a;b} 1. value(7). -tag(\"7\"). #minimize {9@2,a:a;4@2,b:b}. #show.";

fn independent_closures(resources: &ExecutionResources, device: GpuApi) {
    let first = normal(NORMAL_NUMBER);
    let second = normal(NORMAL_STRING);
    assert!(!first.program().same_instance(second.program()));
    for profile in [Profile::Eager, Profile::Lazy] {
        let subject = Subject::Program(first.program().clone());
        let mut limited = config(device.backend(), profile);
        limited.max_candidates = 0;
        let stopped = solve(
            PreparedInput::admitted(&first),
            &subject,
            limited,
            resources,
            AnswerSelection::All,
        );
        assert!(stopped.records.is_empty());
        assert_eq!(stopped.outcome.verified_models(), 0);
        assert_eq!(stopped.outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(
            stopped.outcome.interruption(),
            Some(Interruption::Oracle(Stop::CandidateLimit)),
        );
        for (owner, value, closure_budget) in [
            (
                &first,
                Value::Number(7),
                SolveConfig::DEFAULT.max_closure_batch_bytes,
            ),
            (&second, Value::String("7".into()), 0),
        ] {
            let subject = Subject::Program(owner.program().clone());
            let captured = solve(
                PreparedInput::admitted(owner),
                &subject,
                SolveConfig {
                    // A device session does not reserve CPU worker closures.
                    // Its candidate preparation still retains the per-closure bound.
                    max_closure_batch_bytes: closure_budget,
                    ..config(device.backend(), profile)
                },
                resources,
                AnswerSelection::All,
            );
            require_device(&captured, device, profile);
            assert_eq!(captured.outcome.verified_models(), 2);
            assert!(captured.outcome.incumbent().is_none());
            require_complete(
                &captured,
                &[
                    Record::expected("a", value.clone(), None),
                    Record::expected("b", value, None),
                ],
            );
        }
    }
}

fn independent_formulas(resources: &ExecutionResources, device: GpuApi) {
    let first = formula(FORMULA_FIRST);
    let second = formula(FORMULA_SECOND);
    assert!(!first.theory().same_instance(second.theory()));
    for (owner, choice, cost) in [(&first, "a", 1), (&second, "b", 4), (&first, "a", 1)] {
        let subject = Subject::Theory(owner.theory().clone());
        let captured = solve(
            PreparedInput::formula(owner),
            &subject,
            config(device.backend(), Profile::Formula),
            resources,
            AnswerSelection::Optimal,
        );
        let reference = solve(
            PreparedInput::formula(owner),
            &subject,
            config(Backend::Cpu, Profile::Formula),
            &ExecutionResources::default(),
            AnswerSelection::Optimal,
        );
        assert_eq!(captured.records, reference.records);
        require_device(&captured, device, Profile::Formula);
        assert!(captured.outcome.optimum_proved());
        assert_eq!(
            captured.outcome.incumbent().unwrap().score.costs(),
            [(2, cost)]
        );
        assert_eq!(captured.outcome.incumbent().unwrap().tied_models, 1);
        require_complete(
            &captured,
            &[Record::expected(choice, Value::Number(7), Some(cost))],
        );
    }
    let all = solve(
        PreparedInput::formula(&second),
        &Subject::Theory(second.theory().clone()),
        config(device.backend(), Profile::Formula),
        resources,
        AnswerSelection::All,
    );
    require_device(&all, device, Profile::Formula);
    assert_eq!(all.outcome.selection(), Some(AnswerSelection::All));
    assert_eq!(all.outcome.retained_models(), 0);
    assert!(all.outcome.incumbent().is_none());
    assert!(!all.outcome.optimum_proved());
    require_complete(
        &all,
        &[
            Record::expected("a", Value::Number(7), Some(9)),
            Record::expected("b", Value::Number(7), Some(4)),
        ],
    );
}

fn independent_sessions(device: GpuApi) {
    let context = device.context();
    let resources = ExecutionResources::with_gpu(&context);
    let retained = resources.clone();
    assert!(context.same_instance(retained.gpu_context().unwrap()));
    let predicate = Predicate::new("value", 1).unwrap();
    let atoms = [
        Atom::new(predicate.clone(), vec![Value::Number(7)]).unwrap(),
        Atom::new(predicate.clone(), vec![Value::String("7".into())]).unwrap(),
    ];
    let rows = [1, 0, 0];
    let relation = Relation::from_catalog(&predicate, &atoms, &rows, Limits::default()).unwrap();
    let mut executor = GpuRelationExecutor::from_context(&context).unwrap();
    let mut prepared = executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    drop(resources);
    drop(context);
    independent_closures(&retained, device);
    independent_formulas(&retained, device);
    let value = Value::Number(7);
    let query = relation
        .query(&[(0, (&value).into())], Limits::default())
        .unwrap();
    let masks = prepared
        .filter(
            &[query],
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(relation.same_owner(masks.relation()));
    let selected = masks.selection(0, Limits::default()).unwrap();
    assert_eq!(selected.positions(), [1, 2]);
    assert_eq!(selected.row(0).unwrap().source_index(), 0);
    assert_eq!(selected.row(1).unwrap().source_index(), 0);
    assert_eq!(prepared.activity().submissions, 1);
    assert_eq!(prepared.activity().completed_queries, 1);
}

fn resource_variants(device: GpuApi) -> [ExecutionResources; 2] {
    let context = device.context();
    let profile = GpuFormulaProfile::from_context(&context).unwrap();
    [
        ExecutionResources::with_gpu(&context),
        ExecutionResources::with_formula_profile(&profile),
    ]
}

fn independent_profile_sessions(device: GpuApi) {
    let context = device.context();
    for projection in GateProjection::ALL {
        let profile =
            GpuFormulaProfile::from_context_with_projection(&context, projection).unwrap();
        let resources = ExecutionResources::with_formula_profile(&profile);
        let retained = resources.clone();
        assert!(retained.formula_profile().unwrap().same_instance(&profile));
        assert!(retained.gpu_context().unwrap().same_instance(&context));
        drop(resources);
        drop(profile);
        let owner = formula(FORMULA_FIRST);
        let mut limited = config(device.backend(), Profile::Formula);
        limited.max_candidates = 0;
        let stopped = solve(
            PreparedInput::formula(&owner),
            &Subject::Theory(owner.theory().clone()),
            limited,
            &retained,
            AnswerSelection::Optimal,
        );
        assert!(stopped.records.is_empty());
        assert_eq!(stopped.outcome.verified_models(), 0);
        assert_eq!(stopped.outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(
            stopped.outcome.interruption(),
            Some(Interruption::Countermodel(
                zetesis_sat::Incomplete::CandidateLimit
            ))
        );
        assert!(!stopped.outcome.optimum_proved());
        assert!(!stopped.outcome.unsatisfiable());
        assert!(stopped.outcome.incumbent().is_none());
        assert_eq!(stopped.outcome.formula_execution().unwrap().gpu_batches, 0);
        // Setup observations identify the actual compiled evaluator even when
        // a candidate limit prevents the first dispatch.
        assert_eq!(stopped.routes.observed_projection, Some(projection));
        independent_formulas(&retained, device);
    }
}

fn policy_refusal(device: GpuApi) {
    for resources in resource_variants(device) {
        policy_refusal_with(&resources, device);
    }
}

fn policy_refusal_with(resources: &ExecutionResources, device: GpuApi) {
    let owner = normal(NORMAL_NUMBER);
    let formula = formula(FORMULA_FIRST);
    for (profile, input, subject) in [
        (
            Profile::Eager,
            PreparedInput::admitted(&owner),
            Subject::Program(owner.program().clone()),
        ),
        (
            Profile::Lazy,
            PreparedInput::admitted(&owner),
            Subject::Program(owner.program().clone()),
        ),
        (
            Profile::Formula,
            PreparedInput::formula(&formula),
            Subject::Theory(formula.theory().clone()),
        ),
    ] {
        let mut routes = Routes::default();
        let failure = Session::builder(
            input,
            config(device.other().backend(), profile),
            Cancellation::default(),
        )
        .resources(resources)
        .start_observed(&mut routes)
        .err()
        .expect("a supplied context cannot switch backend");
        assert!(matches!(failure.cause.as_ref(), SolveError::Gpu(error)
            if error.kind() == GpuErrorKind::AdapterRefused));
        assert!(failure.subject().unwrap().same_instance(&subject));
        assert!(failure.semantic().is_none());
        assert_eq!(
            routes.device_closure + routes.cpu_closure + routes.device_formula + routes.cpu_formula,
            0
        );
        let complete = solve(
            input,
            &subject,
            config(device.backend(), profile),
            resources,
            AnswerSelection::Optimal,
        );
        require_device(&complete, device, profile);
        require_complete(&complete, &expected(profile));
    }
}

fn cpu_policies(device: GpuApi) {
    for resources in resource_variants(device) {
        cpu_policies_with(&resources);
    }
}

fn cpu_policies_with(resources: &ExecutionResources) {
    let owner = normal(NORMAL_NUMBER);
    for profile in [Profile::Eager, Profile::Lazy] {
        let capture = solve(
            PreparedInput::admitted(&owner),
            &Subject::Program(owner.program().clone()),
            config(Backend::Cpu, profile),
            resources,
            AnswerSelection::All,
        );
        assert_eq!(capture.routes.cpu_closure, 1);
        assert!(capture.routes.observed_backend.is_none());
        assert!(capture.outcome.lazy_execution().is_none());
        require_complete(
            &capture,
            &[
                Record::expected("a", Value::Number(7), None),
                Record::expected("b", Value::Number(7), None),
            ],
        );
    }
    let formula = formula(FORMULA_FIRST);
    let capture = solve(
        PreparedInput::formula(&formula),
        &Subject::Theory(formula.theory().clone()),
        config(Backend::Cpu, Profile::Formula),
        resources,
        AnswerSelection::Optimal,
    );
    assert_eq!(capture.routes.cpu_formula, 1);
    assert!(capture.routes.observed_backend.is_none());
    assert!(capture.outcome.formula_execution().is_none());
    assert!(capture.outcome.optimum_proved());
    require_complete(
        &capture,
        &[Record::expected("a", Value::Number(7), Some(1))],
    );
}

struct RejectDeviceObservation {
    cause: GpuError,
    calls: usize,
}

impl ExecutionObserver for RejectDeviceObservation {
    type Error = GpuError;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if matches!(
            observation,
            ExecutionObservation::DeviceClosure { .. } | ExecutionObservation::DeviceFormula { .. }
        ) {
            self.calls += 1;
            return Err(self.cause.clone());
        }
        Ok(())
    }
}

fn observer_failure(device: GpuApi) {
    for resources in resource_variants(device) {
        observer_failure_with(&resources, device);
    }
}

fn observer_failure_with(resources: &ExecutionResources, device: GpuApi) {
    let context = resources.gpu_context().unwrap();
    let owner = normal(NORMAL_NUMBER);
    let formula = formula(FORMULA_FIRST);
    // Reuse a real typed policy refusal as an external callback error. Its type
    // must not turn the observation failure into device fallback evidence.
    let cause = context
        .check_selection(GpuOptions::default(), device.other().selection())
        .unwrap_err();
    for (profile, input, subject) in [
        (
            Profile::Eager,
            PreparedInput::admitted(&owner),
            Subject::Program(owner.program().clone()),
        ),
        (
            Profile::Lazy,
            PreparedInput::admitted(&owner),
            Subject::Program(owner.program().clone()),
        ),
        (
            Profile::Formula,
            PreparedInput::formula(&formula),
            Subject::Theory(formula.theory().clone()),
        ),
    ] {
        let mut observer = RejectDeviceObservation {
            cause: cause.clone(),
            calls: 0,
        };
        let failure = Session::builder(
            input,
            config(device.backend(), profile),
            Cancellation::default(),
        )
        .resources(resources)
        .start_observed(&mut observer)
        .err()
        .expect("the preparation observer refuses the session");
        let SolveError::ExecutionObservation(error) = failure.cause.as_ref() else {
            panic!("external failure changed class: {failure:?}");
        };
        assert_eq!(error.downcast_ref::<GpuError>(), Some(&cause));
        assert_eq!(observer.calls, 1);
        assert!(failure.subject().unwrap().same_instance(&subject));
        assert!(failure.semantic().is_none());
        let complete = solve(
            input,
            &subject,
            config(device.backend(), profile),
            resources,
            AnswerSelection::Optimal,
        );
        require_device(&complete, device, profile);
        require_complete(&complete, &expected(profile));
    }
}

fn expected(profile: Profile) -> Vec<Record> {
    match profile {
        Profile::Eager | Profile::Lazy => vec![
            Record::expected("a", Value::Number(7), None),
            Record::expected("b", Value::Number(7), None),
        ],
        Profile::Formula => vec![Record::expected("a", Value::Number(7), Some(1))],
    }
}

#[test]
fn session_resource_fixture_families_are_exact() {
    let resources = ExecutionResources::default();
    for (source, value) in [
        (NORMAL_NUMBER, Value::Number(7)),
        (NORMAL_STRING, Value::String("7".into())),
    ] {
        let owner = normal(source);
        for profile in [Profile::Eager, Profile::Lazy] {
            let capture = solve(
                PreparedInput::admitted(&owner),
                &Subject::Program(owner.program().clone()),
                config(Backend::Cpu, profile),
                &resources,
                AnswerSelection::All,
            );
            require_complete(
                &capture,
                &[
                    Record::expected("a", value.clone(), None),
                    Record::expected("b", value.clone(), None),
                ],
            );
        }
    }
    for (source, choice, cost) in [(FORMULA_FIRST, "a", 1), (FORMULA_SECOND, "b", 4)] {
        let owner = formula(source);
        let capture = solve(
            PreparedInput::formula(&owner),
            &Subject::Theory(owner.theory().clone()),
            config(Backend::Cpu, Profile::Formula),
            &resources,
            AnswerSelection::Optimal,
        );
        assert!(capture.outcome.optimum_proved());
        require_complete(
            &capture,
            &[Record::expected(choice, Value::Number(7), Some(cost))],
        );
    }
}

#[test]
#[ignore = "requires Metal: independent ordinary sessions share one context"]
fn metal_resources_preserve_independent_sessions() {
    independent_sessions(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: independent ordinary sessions share one context"]
fn vulkan_resources_preserve_independent_sessions() {
    independent_sessions(GpuApi::Vulkan);
}

#[test]
#[ignore = "requires Metal: policy refusal cannot replace the supplied context"]
fn metal_resource_policy_refusal_preserves_reuse() {
    policy_refusal(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: policy refusal cannot replace the supplied context"]
fn vulkan_resource_policy_refusal_preserves_reuse() {
    policy_refusal(GpuApi::Vulkan);
}

#[test]
#[ignore = "requires Metal: a supplied context does not select the device policy"]
fn metal_resources_preserve_cpu_policies() {
    cpu_policies(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: a supplied context does not select the device policy"]
fn vulkan_resources_preserve_cpu_policies() {
    cpu_policies(GpuApi::Vulkan);
}

#[test]
#[ignore = "requires Metal: observation failure does not poison shared resources"]
fn metal_observer_failure_preserves_resource_reuse() {
    observer_failure(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: observation failure does not poison shared resources"]
fn vulkan_observer_failure_preserves_resource_reuse() {
    observer_failure(GpuApi::Vulkan);
}

#[test]
#[ignore = "requires Metal: independent formula sessions share one compilation"]
fn metal_formula_profiles_preserve_independent_sessions() {
    independent_profile_sessions(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: independent formula sessions share one compilation"]
fn vulkan_formula_profiles_preserve_independent_sessions() {
    independent_profile_sessions(GpuApi::Vulkan);
}

const UNSEEDED_CYCLE: &str = "a :- b. b :- a.";
const SEEDED_CYCLE: &str = "a. b :- a. a :- b.";

const TIGHT_FAMILIES: [&str; 7] = [
    "",
    UNSEEDED_CYCLE,
    "{a;b}. :- a,b.",
    "a :- not b. b :- not a.",
    "a. b :- a. :- not b.",
    "a. :- a.",
    "1 {a;b} 1. #minimize {1@2,a:a;1@2,b:b}.",
];

#[test]
fn ordinary_tight_fixtures_have_complete_certificates() {
    for source in TIGHT_FAMILIES {
        let owner = formula(source);
        zetesis_ferraris::TightPlan::compile(
            owner.theory(),
            zetesis_ferraris::TightPlanLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    }
}

#[test]
fn unseeded_cycle_has_one_empty_answer_set() {
    let owner = formula(UNSEEDED_CYCLE);
    assert_eq!(owner.theory().atom_count(), 0);
    zetesis_ferraris::TightPlan::compile(
        owner.theory(),
        zetesis_ferraris::TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let subject = Subject::Theory(owner.theory().clone());
    let capture = solve(
        PreparedInput::formula(&owner),
        &subject,
        config(Backend::Cpu, Profile::Formula),
        &ExecutionResources::default(),
        AnswerSelection::All,
    );
    require_complete(
        &capture,
        &[Record {
            atoms: Model::default(),
            costs: None,
        }],
    );
}

#[test]
fn seeded_cycle_preserves_positive_nontight_class() {
    let owner = formula(SEEDED_CYCLE);
    assert_eq!(owner.theory().atom_count(), 2);
    zetesis_ferraris::PositivePlan::compile(
        owner.theory(),
        zetesis_ferraris::PositivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        zetesis_ferraris::TightPlan::compile(
            owner.theory(),
            zetesis_ferraris::TightPlanLimits::default(),
            &Cancellation::default(),
        ),
        Err(zetesis_ferraris::TightError::PositiveCycle { .. })
    ));
}

fn tight_families(device: GpuApi) {
    let context = device.context();
    let resources = ExecutionResources::with_gpu(&context);
    for source in TIGHT_FAMILIES {
        let owner = formula(source);
        let subject = Subject::Theory(owner.theory().clone());
        let cpu = solve(
            PreparedInput::formula(&owner),
            &subject,
            config(Backend::Cpu, Profile::Formula),
            &ExecutionResources::default(),
            AnswerSelection::Optimal,
        );
        assert_eq!(cpu.outcome.completion(), Some(Completion::Exhausted));
        for workers in [1, 4] {
            let mut options = config(device.backend(), Profile::Formula);
            options.oracle = Oracle::Auto;
            options.workers = NonZeroUsize::new(workers).unwrap();
            options.completion_workers = NonZeroUsize::new(4).unwrap();
            // Exercise a partial last batch as well as multiple full batches.
            options.batch_size = NonZeroUsize::new(2).unwrap();
            let gpu = solve(
                PreparedInput::formula(&owner),
                &subject,
                options,
                &resources,
                AnswerSelection::Optimal,
            );
            assert_eq!(gpu.records, cpu.records, "{source}");
            assert_eq!(gpu.outcome.completion(), Some(Completion::Exhausted));
            assert_eq!(gpu.routes.device_tight, 1, "{source}");
            assert_eq!(gpu.routes.device_formula + gpu.routes.cpu_formula, 0);
            assert_eq!(gpu.routes.observed_backend, Some(device.observed()));
            assert_eq!(
                gpu.routes.proposal_workers,
                (workers > 1).then(|| NonZeroUsize::new(workers).unwrap())
            );
            let stats = gpu.outcome.formula_execution().unwrap();
            assert_eq!(stats.gpu_limits, None);
            assert_eq!(stats.gpu_residuals, None);
            assert!(stats.tight_work_per_candidate.is_some());
            assert_eq!(stats.cpu_residuals, 0);
            assert_eq!(stats.completion.requested_workers, 1);
            assert_eq!(stats.completion.effective_workers, 0);
            assert_eq!(stats.completion.residuals, 0);
            assert_eq!(stats.gpu_rounds, 0);
            assert_eq!(stats.gpu_scheduled_work, Some(stats.gpu_work));
            let search = gpu.outcome.countermodel_statistics().unwrap();
            assert_eq!(
                search.certified.unwrap().checks,
                0,
                "device policy must not perform hidden CPU certificate checks"
            );
            assert_eq!(search.countermodel_queries, 0);
            if !gpu.records.is_empty() {
                assert!(stats.gpu_batches > 0);
                assert!(stats.gpu_candidates > 0);
                assert!(stats.gpu_work > 0);
            }
        }
    }
}

#[test]
#[ignore = "requires Metal: tight sessions preserve the complete CPU families on the device's tight route"]
fn metal_tight_sessions_preserve_complete_families() {
    tight_families(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: tight sessions preserve the complete CPU families on the device's tight route"]
fn vulkan_tight_sessions_preserve_complete_families() {
    tight_families(GpuApi::Vulkan);
}

fn general_formula_selection(device: GpuApi) {
    let resources = ExecutionResources::with_gpu(&device.context());
    for (source, oracle) in [
        ("{a}.", Oracle::Countermodel),
        (SEEDED_CYCLE, Oracle::Auto),
        ("a | b.", Oracle::Auto),
    ] {
        let owner = formula(source);
        if oracle == Oracle::Auto {
            assert!(
                matches!(
                    zetesis_ferraris::TightPlan::compile(
                        owner.theory(),
                        zetesis_ferraris::TightPlanLimits::default(),
                        &Cancellation::default(),
                    ),
                    Err(zetesis_ferraris::TightError::PositiveCycle { .. }
                        | zetesis_ferraris::TightError::UnsupportedRoot { .. })
                ),
                "general-device fixture must retain its non-tight theory: {source}"
            );
        }
        let subject = Subject::Theory(owner.theory().clone());
        let cpu = solve(
            PreparedInput::formula(&owner),
            &subject,
            config(Backend::Cpu, Profile::Formula),
            &ExecutionResources::default(),
            AnswerSelection::All,
        );
        let mut options = config(device.backend(), Profile::Formula);
        options.oracle = oracle;
        let gpu = solve(
            PreparedInput::formula(&owner),
            &subject,
            options,
            &resources,
            AnswerSelection::All,
        );
        assert_eq!(
            gpu.records, cpu.records,
            "source={source}, oracle={oracle:?}"
        );
        assert_eq!(
            (gpu.routes.device_formula, gpu.routes.device_tight),
            (1, 0),
            "source={source}, oracle={oracle:?}"
        );
        require_device(&gpu, device, Profile::Formula);
        assert_eq!(gpu.outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(
            gpu.outcome
                .formula_execution()
                .unwrap()
                .tight_work_per_candidate,
            None
        );
    }
}

#[test]
#[ignore = "requires Metal: general formulas never use a CPU certificate fallback"]
fn metal_general_formulas_keep_device_execution() {
    general_formula_selection(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: general formulas never use a CPU certificate fallback"]
fn vulkan_general_formulas_keep_device_execution() {
    general_formula_selection(GpuApi::Vulkan);
}

fn tight_refusal(device: GpuApi) {
    let owner = formula("{a}.");
    let resources = ExecutionResources::with_gpu(&device.context());
    let mut options = config(device.backend(), Profile::Formula);
    options.oracle = Oracle::Auto;
    options.gpu_formula_work = 0;
    let mut routes = Routes::default();
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        options,
        Cancellation::default(),
    )
    .resources(&resources)
    .start_observed(&mut routes)
    .unwrap();
    let failure = session.next_observed(&mut routes).unwrap().unwrap_err();
    assert!(matches!(failure.cause.as_ref(), SolveError::Gpu(_)));
    assert!(session.next_observed(&mut routes).is_none());
    assert_eq!(routes.device_tight, 1);
    assert_eq!(routes.device_formula + routes.cpu_formula, 0);
    let outcome = session.outcome().unwrap();
    assert_ne!(outcome.completion(), Some(Completion::Exhausted));
    let device = outcome.formula_execution().unwrap();
    assert_eq!(device.gpu_submitted_candidates, 0);
    assert_eq!(device.gpu_candidates, 0);
    assert_eq!(device.gpu_scheduled_work, Some(0));
    assert!(device.pending_candidates > 0);
    assert_eq!(
        outcome
            .countermodel_statistics()
            .unwrap()
            .certified
            .unwrap()
            .checks,
        0
    );
    // Admission before submission leaves the exact supplied context reusable.
    let mut options = config(options.backend, Profile::Formula);
    options.oracle = Oracle::Auto;
    let complete = solve(
        PreparedInput::formula(&owner),
        &Subject::Theory(owner.theory().clone()),
        options,
        &resources,
        AnswerSelection::All,
    );
    assert_eq!(complete.outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(complete.records.len(), 2);
    assert!(complete.outcome.formula_execution().unwrap().gpu_candidates > 0);
}

#[test]
#[ignore = "requires Metal: tight work refusal cannot become CPU success"]
fn metal_tight_refusal_preserves_pending_coverage() {
    tight_refusal(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: tight work refusal cannot become CPU success"]
fn vulkan_tight_refusal_preserves_pending_coverage() {
    tight_refusal(GpuApi::Vulkan);
}

fn terminal_families(device: GpuApi) {
    const SOURCE: &str = "{seed(1);seed(2)}. receipt(X):-seed(X). #show X:receipt(X).";
    let reference_owner = formula(SOURCE);
    let reference = solve(
        PreparedInput::formula(&reference_owner),
        &Subject::Theory(reference_owner.theory().clone()),
        config(Backend::Cpu, Profile::Formula),
        &ExecutionResources::default(),
        AnswerSelection::All,
    );
    assert_eq!(reference.outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(reference.records.len(), 4);

    let materialized = zetesis_themelios::prepare_formula(
        SOURCE.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_adaptive()
    .unwrap();
    let zetesis_themelios::FormulaMaterialization::Terminal(owner) = materialized else {
        panic!("fixture requires actual terminal source materialization");
    };
    assert_eq!(owner.deferred_templates(), 1);
    let subject = Subject::TerminalDefinitions(owner.clone());
    assert!(!subject.same_instance(&Subject::Theory(owner.base_theory().unwrap().clone())));
    let resources = ExecutionResources::with_gpu(&device.context());
    let capture = solve(
        PreparedInput::terminal(&owner),
        &subject,
        SolveConfig {
            grounder: Grounder::Auto,
            ..config(device.backend(), Profile::Formula)
        },
        &resources,
        AnswerSelection::All,
    );
    // The common helper checks every answer and final outcome against the
    // original terminal subject; this additionally requires real device work.
    require_device(&capture, device, Profile::Formula);
    require_complete(&capture, &reference.records);
    let terminal = capture.outcome.terminal_execution().unwrap();
    let construction = capture.outcome.model_construction().unwrap();
    assert!(construction.prepared_bytes > 0);
    assert_eq!(construction.constructed, terminal.base_answers);
    assert_eq!(
        (
            terminal.base_answers,
            terminal.reconstructed,
            terminal.pending
        ),
        (4, 4, 0)
    );
    assert_eq!(terminal.reconstruction.completed, 4);
    assert_eq!(capture.outcome.verified_models(), 4);
    for record in &capture.records {
        let shown = owner
            .metadata()
            .observations()
            .evaluate(
                &record.atoms,
                zetesis_themelios::observation::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let expected: Vec<_> = (1..=2)
            .filter(|number| {
                record
                    .atoms
                    .contains(&atom("seed", Sign::Positive, vec![Value::Number(*number)]))
            })
            .map(zetesis_themelios::observation::Symbol::Number)
            .collect();
        assert_eq!(shown.symbols(), expected);
    }
}

#[test]
#[ignore = "requires Metal: terminal reconstruction preserves complete original families"]
fn metal_terminal_sessions_preserve_complete_families() {
    terminal_families(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: terminal reconstruction preserves complete original families"]
fn vulkan_terminal_sessions_preserve_complete_families() {
    terminal_families(GpuApi::Vulkan);
}
