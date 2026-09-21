//! An external consumer implements the public batch protocol without search internals.

use std::{
    collections::BTreeSet,
    io,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{Theory, TightPlan, TightVerdict, Verdict};
use zetesis_solve::{
    AnswerSelection, Backend, BatchExecutor, BatchResult, BatchVerdict, CandidateBatch, Completion,
    ExecutorCapabilities, ExecutorError, ExecutorFailure, Grounder, Interruption,
    MembershipOperation, MembershipPlan, Oracle, PreparedInput, Session, SolveConfig, SolveError,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

type Family = BTreeSet<(Model, Option<Vec<(i32, i64)>>)>;

#[derive(Clone, Copy)]
enum Behavior {
    Reference,
    Residual,
    WrongShape,
    Failed,
    FailedAfterFirst,
    Refused,
    Cancel,
    CancelDecisive,
    NotModel,
    Mixed,
}

#[derive(Default)]
struct Activity {
    prepared: Option<MembershipOperation>,
    calls: usize,
    candidates: usize,
}

struct ReferenceExecutor {
    capabilities: ExecutorCapabilities,
    behavior: Behavior,
    theory: Option<Theory>,
    tight: Option<Arc<TightPlan>>,
    activity: Arc<Mutex<Activity>>,
}

impl ReferenceExecutor {
    fn new(capabilities: ExecutorCapabilities, behavior: Behavior) -> Self {
        Self {
            capabilities,
            behavior,
            theory: None,
            tight: None,
            activity: Arc::default(),
        }
    }
}

impl BatchExecutor for ReferenceExecutor {
    type Error = io::Error;

    fn capabilities(&self) -> ExecutorCapabilities {
        self.capabilities
    }

    fn prepare(
        &mut self,
        plan: MembershipPlan<'_>,
        cancellation: &Cancellation,
    ) -> Result<(), ExecutorFailure<Self::Error>> {
        cancellation.poll()?;
        self.theory = Some(plan.theory().clone());
        self.activity.lock().unwrap().prepared = Some(plan.operation());
        if matches!(self.behavior, Behavior::Refused) {
            return Err(ExecutorFailure::Unsupported);
        }
        if let MembershipPlan::Tight(plan) = plan {
            self.tight = Some(Arc::clone(plan));
        }
        Ok(())
    }

    fn check<'a>(
        &mut self,
        batch: CandidateBatch<'a>,
        cancellation: &Cancellation,
    ) -> Result<BatchResult<'a>, ExecutorFailure<Self::Error>> {
        assert!(self.theory.as_ref().unwrap().same_instance(batch.theory()));
        let ordinal = {
            let mut activity = self.activity.lock().unwrap();
            activity.calls += 1;
            activity.candidates += batch.candidates().len();
            activity.calls
        };
        if matches!(self.behavior, Behavior::Failed)
            || (matches!(self.behavior, Behavior::FailedAfterFirst) && ordinal > 1)
        {
            return Err(ExecutorFailure::Failed(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "external executor transport",
            )));
        }
        if matches!(self.behavior, Behavior::WrongShape) {
            return Ok(batch.finish(Vec::new())?);
        }
        let mut verdicts = Vec::new();
        verdicts
            .try_reserve_exact(batch.candidates().len())
            .map_err(|_| Stop::Allocation)?;
        for candidate in batch.candidates() {
            cancellation.poll()?;
            assert!(zetesis_ferraris::models(
                batch.theory(),
                candidate,
                zetesis_ferraris::Limits::default(),
                cancellation
            )?);
            let verdict = if matches!(self.behavior, Behavior::NotModel) {
                BatchVerdict::NotModel
            } else if matches!(self.behavior, Behavior::Residual | Behavior::Cancel)
                || (matches!(self.behavior, Behavior::Mixed) && verdicts.len() % 2 == 1)
            {
                BatchVerdict::Residual
            } else if let Some(plan) = &self.tight {
                match plan
                    .check(
                        candidate,
                        zetesis_ferraris::TightCheckLimits::default(),
                        cancellation,
                    )
                    .unwrap()
                    .verdict
                {
                    TightVerdict::Stable => BatchVerdict::NoProperSubset,
                    TightVerdict::NotModel { .. } => BatchVerdict::NotModel,
                    TightVerdict::Residual { .. } => BatchVerdict::Refuted,
                }
            } else {
                match zetesis_ferraris::check(
                    batch.theory(),
                    candidate,
                    zetesis_ferraris::Limits::default(),
                    cancellation,
                )?
                .verdict()
                {
                    Verdict::Stable => BatchVerdict::NoProperSubset,
                    Verdict::NotModel { .. } => BatchVerdict::NotModel,
                    Verdict::NonMinimal { .. } => BatchVerdict::Refuted,
                }
            };
            verdicts.push(verdict);
        }
        if matches!(self.behavior, Behavior::Cancel | Behavior::CancelDecisive) {
            cancellation.cancel();
        }
        Ok(batch.finish(verdicts)?)
    }
}

fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Auto,
        grounder: Grounder::Eager,
        models: 0,
        workers: NonZeroUsize::new(3).unwrap(),
        completion_workers: NonZeroUsize::new(2).unwrap(),
        batch_size: NonZeroUsize::new(3).unwrap(),
        ..SolveConfig::DEFAULT
    }
}

fn family(session: &mut Session<'_>) -> Family {
    session
        .by_ref()
        .map(|answer| {
            let answer = answer.unwrap();
            (
                answer.interpretation().clone(),
                answer.score().map(|score| score.costs().to_vec()),
            )
        })
        .collect()
}

#[test]
fn external_reference_preserves_complete_families() {
    for source in [
        "{a;b;c}.",
        "a :- not b. b :- not a.",
        "{seed}. p :- seed. p :- p.",
        ":-.",
    ] {
        let owner = input(source);
        let mut native = Session::new(
            PreparedInput::formula(&owner),
            SolveConfig {
                backend: Backend::Cpu,
                ..config()
            },
            Cancellation::default(),
        )
        .unwrap();
        let expected = family(&mut native);
        for behavior in [Behavior::Reference, Behavior::Residual] {
            let executor = ReferenceExecutor::new(ExecutorCapabilities::General, behavior);
            let activity = Arc::clone(&executor.activity);
            let mut session = Session::builder(
                PreparedInput::formula(&owner),
                config(),
                Cancellation::default(),
            )
            .executor(executor)
            .start()
            .unwrap();
            assert_eq!(family(&mut session), expected, "{source}");
            let outcome = session.outcome().unwrap();
            assert_eq!(outcome.completion(), Some(Completion::Exhausted));
            assert!(outcome.formula_execution().is_none());
            let receipt = outcome.batch_execution().unwrap();
            assert_eq!(receipt.operation, MembershipOperation::General);
            assert_eq!(receipt.batches.pending, 0);
            assert_eq!(receipt.queued_models, 0);
            assert_eq!(
                usize::try_from(receipt.batches.checker_calls).unwrap(),
                activity.lock().unwrap().calls
            );
            assert_eq!(
                usize::try_from(receipt.batches.committed).unwrap(),
                activity.lock().unwrap().candidates
            );
        }
    }
}

#[test]
fn external_formula_execution_ignores_closure_reservations() {
    let owner = input("{a;b}.");
    let mut native = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            backend: Backend::Cpu,
            ..config()
        },
        Cancellation::default(),
    )
    .start()
    .unwrap();
    let expected = family(&mut native);
    assert_eq!(expected.len(), 4);
    let executor = ReferenceExecutor::new(ExecutorCapabilities::General, Behavior::Reference);
    let activity = Arc::clone(&executor.activity);
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            max_closure_bytes: usize::MAX,
            max_closure_batch_bytes: 0,
            ..config()
        },
        Cancellation::default(),
    )
    .executor(executor)
    .start()
    .unwrap();
    assert_eq!(family(&mut session), expected);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(outcome.batch_execution().is_some());
    assert!(outcome.closure_execution().is_none());
    assert!(activity.lock().unwrap().calls > 0);
}

#[test]
fn external_execution_preserves_objective_selection() {
    let owner = input("1 {a;b;c} 1. #minimize {1,a:a;1,b:b;2,c:c}.");
    for selection in [AnswerSelection::All, AnswerSelection::Optimal] {
        let mut native = Session::builder(
            PreparedInput::formula(&owner),
            SolveConfig {
                backend: Backend::Cpu,
                ..config()
            },
            Cancellation::default(),
        )
        .selection(selection)
        .start()
        .unwrap();
        let expected = family(&mut native);
        let mut session = Session::builder(
            PreparedInput::formula(&owner),
            config(),
            Cancellation::default(),
        )
        .selection(selection)
        .executor(ReferenceExecutor::new(
            ExecutorCapabilities::General,
            Behavior::Reference,
        ))
        .start()
        .unwrap();
        assert_eq!(family(&mut session), expected);
        assert_eq!(
            session.outcome().unwrap().optimum_proved(),
            selection == AnswerSelection::Optimal
        );
        assert_eq!(
            session.outcome().unwrap().completion(),
            Some(Completion::Exhausted)
        );
    }
}

#[test]
fn tight_executor_uses_the_shared_complete_plan() {
    let owner = input("{a;b}.");
    let executor =
        ReferenceExecutor::new(ExecutorCapabilities::GeneralAndTight, Behavior::Reference);
    let activity = Arc::clone(&executor.activity);
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(executor)
    .start()
    .unwrap();
    assert_eq!(family(&mut session).len(), 4);
    let outcome = session.outcome().unwrap();
    let receipt = outcome.batch_execution().unwrap();
    assert_eq!(receipt.operation, MembershipOperation::Tight);
    assert_eq!(
        activity.lock().unwrap().prepared,
        Some(MembershipOperation::Tight)
    );
    assert_eq!(receipt.batches.residuals, 0);
    assert_eq!(receipt.batches.propagated, 4);
    assert!(
        outcome
            .countermodel_statistics()
            .unwrap()
            .certified
            .unwrap()
            .plan
            .is_some()
    );
}

#[test]
fn unsupported_plan_never_prepares_the_executor() {
    let owner = input("{seed}. p :- seed. p :- p.");
    let executor = ReferenceExecutor::new(ExecutorCapabilities::Tight, Behavior::Reference);
    let activity = Arc::clone(&executor.activity);
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(executor)
    .start()
    .err()
    .expect("positive cycle has no tight plan");
    assert!(matches!(
        *failure.cause,
        SolveError::Executor(ExecutorError::Capability(ExecutorCapabilities::Tight))
    ));
    assert!(activity.lock().unwrap().prepared.is_none());
    assert_eq!(activity.lock().unwrap().calls, 0);
}

#[test]
fn explicit_general_request_does_not_use_tight_support() {
    let owner = input("{a}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            oracle: Oracle::Countermodel,
            ..config()
        },
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::GeneralAndTight,
        Behavior::Reference,
    ))
    .start()
    .unwrap();
    assert_eq!(family(&mut session).len(), 2);
    let outcome = session.outcome().unwrap();
    assert_eq!(
        outcome.batch_execution().unwrap().operation,
        MembershipOperation::General
    );
    assert!(
        outcome
            .countermodel_statistics()
            .unwrap()
            .certified
            .is_none()
    );
}

fn failing_session(owner: &AdmittedFormula) -> Session<'_> {
    Session::builder(
        PreparedInput::formula(owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::Failed,
    ))
    .start()
    .unwrap()
}

#[test]
fn external_fault_preserves_its_typed_cause() {
    let owner = input("{a;b}.");
    let mut session = failing_session(&owner);
    let failure = session.next().unwrap().unwrap_err();
    let SolveError::Executor(ExecutorError::External(error)) = failure.cause.as_ref() else {
        panic!("original executor fault must be retained")
    };
    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().kind(),
        io::ErrorKind::ConnectionReset
    );
}

#[test]
fn external_fault_preserves_pending_coverage() {
    let owner = input("{a;b}.");
    let mut session = failing_session(&owner);
    assert!(session.next().unwrap().is_err());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.batch_execution().unwrap().batches.pending, 3);
    assert!(session.next().is_none());
}

#[test]
fn malformed_receipt_never_commits_a_candidate() {
    let owner = input("{a;b}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::WrongShape,
    ))
    .start()
    .unwrap();
    let failure = session.next().unwrap().unwrap_err();
    assert!(matches!(
        *failure.cause,
        SolveError::FormulaBatchShape {
            expected: 3,
            actual: 0
        }
    ));
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), None);
    assert_eq!(outcome.batch_execution().unwrap().batches.committed, 0);
    assert_eq!(outcome.batch_execution().unwrap().batches.pending, 3);
}

#[test]
fn cancellation_after_check_keeps_pending_coverage() {
    let owner = input("{a;b}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::Cancel,
    ))
    .start()
    .unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::Countermodel(
            zetesis_sat::Incomplete::Cancelled
        ))
    ));
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.batch_execution().unwrap().batches.pending, 3);
}

#[test]
fn cancellation_before_setup_never_calls_the_executor() {
    let owner = input("{a}.");
    let executor = ReferenceExecutor::new(ExecutorCapabilities::General, Behavior::Reference);
    let activity = Arc::clone(&executor.activity);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut session = Session::builder(PreparedInput::formula(&owner), config(), cancellation)
        .executor(executor)
        .start()
        .unwrap();
    assert!(session.next().is_none());
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Interrupted)
    );
    assert!(activity.lock().unwrap().prepared.is_none());
    assert_eq!(activity.lock().unwrap().calls, 0);
}

#[test]
fn builtin_backend_requests_conflict_with_injection() {
    let owner = input("{a}.");
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        SolveConfig {
            backend: Backend::Cpu,
            ..config()
        },
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::Reference,
    ))
    .start()
    .err()
    .expect("explicit builtin selection must be honored");
    assert!(matches!(
        *failure.cause,
        SolveError::Executor(ExecutorError::Backend(Backend::Cpu))
    ));
}

#[test]
fn relational_inputs_are_not_silently_materialized() {
    let owner = zetesis_themelios::admit("a.".into(), AdmissionOptions::default()).unwrap();
    let failure = Session::builder(
        PreparedInput::admitted(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::Reference,
    ))
    .start()
    .err()
    .expect("formula-only capability is explicit");
    assert!(matches!(
        *failure.cause,
        SolveError::Executor(ExecutorError::Input(
            zetesis_solve::PreparedProfile::Relational
        ))
    ));
}

#[test]
fn failed_later_batch_retains_the_verified_prefix() {
    let owner = input("{a;b}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::FailedAfterFirst,
    ))
    .start()
    .unwrap();
    for _ in 0..3 {
        session.next().unwrap().unwrap();
    }
    assert!(session.next().unwrap().is_err());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.verified_models(), 3);
    assert_eq!(outcome.completion(), None);
    let receipt = outcome.batch_execution().unwrap();
    assert_eq!(receipt.batches.committed, 3);
    assert_eq!(receipt.batches.pending, 1);
    assert_eq!(receipt.batches.checker_calls, 2);
    assert_eq!(receipt.queued_models, 0);
}

#[test]
fn preparation_refusal_does_not_choose_another_executor() {
    let owner = input("{a}.");
    let executor = ReferenceExecutor::new(ExecutorCapabilities::General, Behavior::Refused);
    let activity = Arc::clone(&executor.activity);
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(executor)
    .start()
    .err()
    .expect("explicit implementation refused preparation");
    assert!(failure.subject().is_some());
    assert!(matches!(
        *failure.cause,
        SolveError::Executor(ExecutorError::Unsupported)
    ));
    assert_eq!(activity.lock().unwrap().calls, 0);
}

#[test]
fn cancellation_after_decisive_verdicts_commits_nothing() {
    let owner = input("{a;b}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::CancelDecisive,
    ))
    .start()
    .unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(outcome.verified_models(), 0);
    let receipt = outcome.batch_execution().unwrap();
    assert_eq!(receipt.batches.committed, 0);
    assert_eq!(receipt.batches.propagated, 0);
    assert_eq!(receipt.batches.pending, 3);
}

#[test]
fn conflicting_original_model_receipt_is_incomplete() {
    let owner = input("{a}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::NotModel,
    ))
    .start()
    .unwrap();
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Countermodel(
            zetesis_sat::Incomplete::InvalidWitness
        ))
    );
    assert_eq!(outcome.verified_models(), 0);
    assert_eq!(outcome.batch_execution().unwrap().batches.pending, 2);
}

#[test]
fn mixed_decisions_and_residuals_share_one_commit() {
    let owner = input("{a;b}.");
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        config(),
        Cancellation::default(),
    )
    .executor(ReferenceExecutor::new(
        ExecutorCapabilities::General,
        Behavior::Mixed,
    ))
    .start()
    .unwrap();
    assert_eq!(family(&mut session).len(), 4);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    let receipt = outcome.batch_execution().unwrap();
    assert_eq!(receipt.batches.committed, 4);
    assert_eq!(receipt.batches.residuals, 1);
    assert_eq!(receipt.batches.propagated, 3);
    assert_eq!(receipt.batches.pending, 0);
}
