//! Ordinary sessions share device ownership, never search or answer-set evidence.

use std::{convert::Infallible, num::NonZeroUsize};

use zetesis_algebra::Cancellation;
use zetesis_core::{
    Atom, Model, Predicate, Sign, Value,
    relation::{Limits, Relation},
};
use zetesis_reference_support::{formula, prepared};
use zetesis_solve::{
    AnswerSelection, AnswerSet, Backend, Completion, ExecutionObservation, ExecutionObserver,
    ExecutionResources, GpuApi, Grounder, Interruption, Oracle, Original, PreparedInput,
    SemanticOutcome, Session, SolveConfig, SolveError,
};
use zetesis_test_support::programs::signed as atom;
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

/// The two program shapes a session test runs: a normal program whose answer
/// sets are enumerated, and a choice whose optimum is proved.
#[derive(Clone, Copy)]
enum Profile {
    Normal,
    Optimization,
}

fn config(backend: Backend) -> SolveConfig {
    SolveConfig {
        backend,
        grounder: Grounder::Eager,
        oracle: Oracle::Countermodel,
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
    device_formula: usize,
    device_tight: usize,
    proposal_workers: Option<NonZeroUsize>,
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
            ExecutionObservation::DeviceFormula {
                adapter,
                projection,
                ..
            } => {
                self.device_formula += 1;
                self.observed_projection = Some(projection);
                Some(adapter)
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
    original: &Original,
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
        assert!(answer.original().same_instance(original));
        records.push(Record::from_answer(&answer));
    }
    assert!(session.next_observed(&mut routes).is_none());
    let outcome = session.outcome().unwrap();
    assert!(outcome.original().unwrap().same_instance(original));
    records.sort();
    Capture {
        records,
        outcome,
        routes,
    }
}

fn require_device(capture: &Capture, device: GpuApi) {
    assert_eq!(capture.routes.observed_backend, Some(device.observed()));
    assert_eq!(capture.routes.cpu_formula, 0);
    assert_eq!(capture.routes.device_formula, 1);
    let execution = capture.outcome.formula_execution().unwrap();
    assert!(execution.gpu_batches > 0);
    assert!(execution.gpu_work > 0);
    assert!(execution.gpu_candidates > 0);
    assert_eq!(execution.pending_candidates + execution.queued_models, 0);
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
// Sixty-four answers in batches of four: an answer arrives before the last batch.
const FORMULA_MANY: &str = "{a;b;c;d;e;f}.";

fn independent_normal_sessions(resources: &ExecutionResources, device: GpuApi) {
    let first = formula(NORMAL_NUMBER);
    let second = formula(NORMAL_STRING);
    assert!(!first.theory().same_instance(second.theory()));
    let original = Original::Theory(first.theory().clone());
    let mut limited = config(device.backend());
    limited.max_candidates = 0;
    let stopped = solve(
        PreparedInput::formula(&first),
        &original,
        limited,
        resources,
        AnswerSelection::All,
    );
    assert!(stopped.records.is_empty());
    assert_eq!(stopped.outcome.verified_models(), 0);
    assert_eq!(stopped.outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        stopped.outcome.interruption(),
        Some(Interruption::Countermodel(
            zetesis_stable::Incomplete::CandidateLimit
        )),
    );
    for (owner, value) in [
        (&first, Value::Number(7)),
        (&second, Value::String("7".into())),
    ] {
        let original = Original::Theory(owner.theory().clone());
        let captured = solve(
            PreparedInput::formula(owner),
            &original,
            config(device.backend()),
            resources,
            AnswerSelection::All,
        );
        require_device(&captured, device);
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

fn independent_formulas(resources: &ExecutionResources, device: GpuApi) {
    let first = formula(FORMULA_FIRST);
    let second = formula(FORMULA_SECOND);
    assert!(!first.theory().same_instance(second.theory()));
    for (owner, choice, cost) in [(&first, "a", 1), (&second, "b", 4), (&first, "a", 1)] {
        let original = Original::Theory(owner.theory().clone());
        let captured = solve(
            PreparedInput::formula(owner),
            &original,
            config(device.backend()),
            resources,
            AnswerSelection::Optimal,
        );
        let reference = solve(
            PreparedInput::formula(owner),
            &original,
            config(Backend::Cpu),
            &ExecutionResources::default(),
            AnswerSelection::Optimal,
        );
        assert_eq!(captured.records, reference.records);
        require_device(&captured, device);
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
        &Original::Theory(second.theory().clone()),
        config(device.backend()),
        resources,
        AnswerSelection::All,
    );
    require_device(&all, device);
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
    independent_normal_sessions(&retained, device);
    independent_formulas(&retained, device);
    stopped_timed_session(device);
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

// Each decoded batch is reported once: measured, unavailable, or unresolved
// while its execution has not settled.
fn assert_timing_identity(outcome: &SemanticOutcome) {
    let execution = outcome.formula_execution().unwrap();
    let timing = execution.gpu_compute_timing;
    assert_eq!(
        timing.measured_batches + timing.unavailable_batches + timing.unresolved_batches,
        execution.gpu_batches
    );
}

// A session stopped by its control after a completed device batch leaves that
// batch's interval unresolved, and the supplied timed context keeps serving.
fn stopped_timed_session(device: GpuApi) {
    let context =
        GpuContext::new_selected_with_timing(GpuOptions::default(), device.selection()).unwrap();
    let enabled = context.compute_timing() == zetesis_backend::ComputeTimingStatus::Enabled;
    let resources = ExecutionResources::with_gpu(&context);
    let owner = formula(FORMULA_MANY);
    let control = Cancellation::default();
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(device.backend()),
        control.clone(),
    )
    .resources(&resources)
    .selection(AnswerSelection::All)
    .start()
    .unwrap();
    session.next().unwrap().unwrap();
    control.cancel();
    let stopped = session.stop();
    assert_timing_identity(&stopped);
    let timing = stopped.formula_execution().unwrap().gpu_compute_timing;
    assert_eq!(timing.flush_submissions, 0);
    assert_eq!(timing.unresolved_batches, u64::from(enabled));
    let original = Original::Theory(owner.theory().clone());
    let completed = solve(
        PreparedInput::formula(&owner),
        &original,
        config(device.backend()),
        &resources,
        AnswerSelection::All,
    );
    require_device(&completed, device);
    assert_eq!(completed.records.len(), 64);
    assert_timing_identity(&completed.outcome);
    let timing = completed
        .outcome
        .formula_execution()
        .unwrap()
        .gpu_compute_timing;
    assert_eq!(timing.unresolved_batches, 0);
    assert_eq!(timing.flush_submissions, u64::from(enabled));
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
        let mut limited = config(device.backend());
        limited.max_candidates = 0;
        let stopped = solve(
            PreparedInput::formula(&owner),
            &Original::Theory(owner.theory().clone()),
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
                zetesis_stable::Incomplete::CandidateLimit
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
    let owner = formula(NORMAL_NUMBER);
    let formula = formula(FORMULA_FIRST);
    for (profile, input, original) in [
        (
            Profile::Normal,
            PreparedInput::formula(&owner),
            Original::Theory(owner.theory().clone()),
        ),
        (
            Profile::Optimization,
            PreparedInput::formula(&formula),
            Original::Theory(formula.theory().clone()),
        ),
    ] {
        let mut routes = Routes::default();
        let failure = Session::builder(
            input,
            config(device.other().backend()),
            Cancellation::default(),
        )
        .resources(resources)
        .start_observed(&mut routes)
        .err()
        .expect("a supplied context cannot switch backend");
        assert!(matches!(failure.cause.as_ref(), SolveError::Gpu(error)
            if error.kind() == GpuErrorKind::AdapterRefused));
        assert!(failure.original().unwrap().same_instance(&original));
        assert!(failure.semantic().is_none());
        assert_eq!(
            routes.device_tight + routes.device_formula + routes.cpu_formula,
            0
        );
        let complete = solve(
            input,
            &original,
            config(device.backend()),
            resources,
            AnswerSelection::Optimal,
        );
        require_device(&complete, device);
        require_complete(&complete, &expected(profile));
    }
}

fn cpu_policies(device: GpuApi) {
    for resources in resource_variants(device) {
        cpu_policies_with(&resources);
    }
}

fn cpu_policies_with(resources: &ExecutionResources) {
    let owner = formula(NORMAL_NUMBER);
    let capture = solve(
        PreparedInput::formula(&owner),
        &Original::Theory(owner.theory().clone()),
        config(Backend::Cpu),
        resources,
        AnswerSelection::All,
    );
    assert_eq!(capture.routes.cpu_formula, 1);
    assert!(capture.routes.observed_backend.is_none());
    require_complete(
        &capture,
        &[
            Record::expected("a", Value::Number(7), None),
            Record::expected("b", Value::Number(7), None),
        ],
    );
    let formula = formula(FORMULA_FIRST);
    let capture = solve(
        PreparedInput::formula(&formula),
        &Original::Theory(formula.theory().clone()),
        config(Backend::Cpu),
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
            ExecutionObservation::DeviceTight { .. } | ExecutionObservation::DeviceFormula { .. }
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
    let owner = formula(NORMAL_NUMBER);
    let formula = formula(FORMULA_FIRST);
    // Reuse a real typed policy refusal as an external callback error. Its type
    // must not turn the observation failure into device fallback evidence.
    let cause = context
        .check_selection(GpuOptions::default(), device.other().selection())
        .unwrap_err();
    for (profile, input, original) in [
        (
            Profile::Normal,
            PreparedInput::formula(&owner),
            Original::Theory(owner.theory().clone()),
        ),
        (
            Profile::Optimization,
            PreparedInput::formula(&formula),
            Original::Theory(formula.theory().clone()),
        ),
    ] {
        let mut observer = RejectDeviceObservation {
            cause: cause.clone(),
            calls: 0,
        };
        let failure = Session::builder(input, config(device.backend()), Cancellation::default())
            .resources(resources)
            .start_observed(&mut observer)
            .err()
            .expect("the preparation observer refuses the session");
        let SolveError::ExecutionObservation(error) = failure.cause.as_ref() else {
            panic!("external failure changed class: {failure:?}");
        };
        assert_eq!(error.downcast_ref::<GpuError>(), Some(&cause));
        assert_eq!(observer.calls, 1);
        assert!(failure.original().unwrap().same_instance(&original));
        assert!(failure.semantic().is_none());
        let complete = solve(
            input,
            &original,
            config(device.backend()),
            resources,
            AnswerSelection::Optimal,
        );
        require_device(&complete, device);
        require_complete(&complete, &expected(profile));
    }
}

fn expected(profile: Profile) -> Vec<Record> {
    match profile {
        Profile::Normal => vec![
            Record::expected("a", Value::Number(7), None),
            Record::expected("b", Value::Number(7), None),
        ],
        Profile::Optimization => vec![Record::expected("a", Value::Number(7), Some(1))],
    }
}

#[test]
fn session_resource_fixture_families_are_exact() {
    let resources = ExecutionResources::default();
    for (source, value) in [
        (NORMAL_NUMBER, Value::Number(7)),
        (NORMAL_STRING, Value::String("7".into())),
    ] {
        let owner = formula(source);
        let capture = solve(
            PreparedInput::formula(&owner),
            &Original::Theory(owner.theory().clone()),
            config(Backend::Cpu),
            &resources,
            AnswerSelection::All,
        );
        require_complete(
            &capture,
            &[
                Record::expected("a", value.clone(), None),
                Record::expected("b", value, None),
            ],
        );
    }
    for (source, choice, cost) in [(FORMULA_FIRST, "a", 1), (FORMULA_SECOND, "b", 4)] {
        let owner = formula(source);
        let capture = solve(
            PreparedInput::formula(&owner),
            &Original::Theory(owner.theory().clone()),
            config(Backend::Cpu),
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
// The seed reaches the cycle through a derived atom. Substituting explicit
// facts therefore leaves the a/b dependency cycle intact for general checking.
const SEEDED_CYCLE: &str = "seed. a :- seed. b :- a. a :- b.";

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
    let original = Original::Theory(owner.theory().clone());
    let capture = solve(
        PreparedInput::formula(&owner),
        &original,
        config(Backend::Cpu),
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
    assert_eq!(owner.theory().atom_count(), 3);
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
        let original = Original::Theory(owner.theory().clone());
        let cpu = solve(
            PreparedInput::formula(&owner),
            &original,
            config(Backend::Cpu),
            &ExecutionResources::default(),
            AnswerSelection::Optimal,
        );
        assert_eq!(cpu.outcome.completion(), Some(Completion::Exhausted));
        for workers in [1, 4] {
            let mut options = config(device.backend());
            options.oracle = Oracle::Auto;
            options.workers = NonZeroUsize::new(workers).unwrap();
            options.completion_workers = NonZeroUsize::new(4).unwrap();
            // Exercise a partial last batch as well as multiple full batches.
            options.batch_size = NonZeroUsize::new(2).unwrap();
            let gpu = solve(
                PreparedInput::formula(&owner),
                &original,
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

/// An enumeration proposes its next slate while the device checks the
/// current one, on each device route, and returns the CPU's family.
fn enumeration_lookahead(device: GpuApi) {
    let resources = ExecutionResources::with_gpu(&device.context());
    // Thirty-two answers over slates of four and more.
    let owner = formula("{a;b;c;d;e}.");
    let original = Original::Theory(owner.theory().clone());
    let cpu = solve(
        PreparedInput::formula(&owner),
        &original,
        config(Backend::Cpu),
        &ExecutionResources::default(),
        AnswerSelection::All,
    );
    for (oracle, routes) in [(Oracle::Countermodel, (1, 0)), (Oracle::Auto, (0, 1))] {
        let mut options = config(device.backend());
        options.oracle = oracle;
        let gpu = solve(
            PreparedInput::formula(&owner),
            &original,
            options,
            &resources,
            AnswerSelection::All,
        );
        assert_eq!(gpu.records, cpu.records, "oracle={oracle:?}");
        assert_eq!(
            (gpu.routes.device_formula, gpu.routes.device_tight),
            routes,
            "oracle={oracle:?}"
        );
        assert_eq!(gpu.outcome.completion(), Some(Completion::Exhausted));
        let stats = gpu.outcome.formula_execution().unwrap();
        assert!(stats.gpu_lookahead_slates > 0, "oracle={oracle:?}");
        assert_eq!(
            stats.gpu_lookahead,
            Some(zetesis_solve::GpuLookahead::Engaged),
            "oracle={oracle:?}"
        );
    }
}

#[test]
#[ignore = "requires Metal: an enumeration proposes slates ahead on the device routes with the CPU's family"]
fn metal_enumeration_looks_ahead_with_the_same_family() {
    enumeration_lookahead(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: an enumeration proposes slates ahead on the device routes with the CPU's family"]
fn vulkan_enumeration_looks_ahead_with_the_same_family() {
    enumeration_lookahead(GpuApi::Vulkan);
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
        let original = Original::Theory(owner.theory().clone());
        let cpu = solve(
            PreparedInput::formula(&owner),
            &original,
            config(Backend::Cpu),
            &ExecutionResources::default(),
            AnswerSelection::All,
        );
        let mut options = config(device.backend());
        options.oracle = oracle;
        let gpu = solve(
            PreparedInput::formula(&owner),
            &original,
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
        require_device(&gpu, device);
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
    let mut options = config(device.backend());
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
    let mut options = config(options.backend);
    options.oracle = Oracle::Auto;
    let complete = solve(
        PreparedInput::formula(&owner),
        &Original::Theory(owner.theory().clone()),
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
        &Original::Theory(reference_owner.theory().clone()),
        config(Backend::Cpu),
        &ExecutionResources::default(),
        AnswerSelection::All,
    );
    assert_eq!(reference.outcome.completion(), Some(Completion::Exhausted));
    assert_eq!(reference.records.len(), 4);

    let materialized = prepared(SOURCE).ground_adaptive().unwrap();
    let zetesis_themelios::FormulaMaterialization::Terminal(owner) = materialized else {
        panic!("fixture requires actual terminal source materialization");
    };
    assert_eq!(owner.deferred_templates(), 1);
    let original = Original::TerminalDefinitions(owner.clone());
    assert!(!original.same_instance(&Original::Theory(owner.base_theory().unwrap().clone())));
    let resources = ExecutionResources::with_gpu(&device.context());
    let capture = solve(
        PreparedInput::terminal(&owner),
        &original,
        SolveConfig {
            grounder: Grounder::Auto,
            ..config(device.backend())
        },
        &resources,
        AnswerSelection::All,
    );
    // The common helper checks every answer and final outcome against the
    // original terminal input; this additionally requires real device work.
    require_device(&capture, device);
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
    // A terminal-definition solve ending at its model limit settles its base
    // execution, which reads the final batch's interval.
    let timed =
        GpuContext::new_selected_with_timing(GpuOptions::default(), device.selection()).unwrap();
    let enabled = timed.compute_timing() == zetesis_backend::ComputeTimingStatus::Enabled;
    let limited = solve(
        PreparedInput::terminal(&owner),
        &original,
        SolveConfig {
            grounder: Grounder::Auto,
            models: 2,
            ..config(device.backend())
        },
        &ExecutionResources::with_gpu(&timed),
        AnswerSelection::All,
    );
    assert_eq!(limited.records.len(), 2);
    assert_eq!(
        limited.outcome.completion(),
        Some(Completion::RequestedModels)
    );
    // A limited solve leaves later candidates queued; only the route is shared
    // with the complete solve's checks.
    assert_eq!(limited.routes.device_formula, 1);
    assert!(limited.outcome.formula_execution().unwrap().gpu_batches > 0);
    assert_timing_identity(&limited.outcome);
    let timing = limited
        .outcome
        .formula_execution()
        .unwrap()
        .gpu_compute_timing;
    assert_eq!(timing.unresolved_batches, 0);
    assert_eq!(timing.flush_submissions, u64::from(enabled));
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
