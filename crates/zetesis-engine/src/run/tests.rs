//! Real native searches feed the upstream query tier; no synthetic backend.

use std::{
    collections::BTreeSet, convert::Infallible, error::Error, io, num::NonZeroUsize, sync::Arc,
    time::Instant,
};

use themelios_program::{
    AnswerSet, Name, Sign, Symbol, Term,
    program::{Atom, Choice, ChoiceElement, Condition, Program, Rule, Show, Statement},
    symbol::VarName,
};
use themelios_query::{Answer, BindingPattern, Query, Snapshot, WorldView};
use themelios_solve::{
    agent::Scenario,
    contract::{Locus, Presupposition, Refused},
    outcome::{Conclusion, Determination, Run as _, ShowRule, Solved, Truncation},
};
use zetesis_cpu::{CancellationRun, CancellationSlot, Stop};
use zetesis_solve::{
    Backend, ExecutionObservation, ExecutionObserver, Grounder, PreparedInput, Session,
    SolveConfig, SolveError, SolveFailure,
};
use zetesis_themelios::{
    AdmittedFormula, FormulaPurpose, MetadataLimits, ProgramAdmissionOptions,
    ProgramFormulaOptions, ProgramSite, SourceMetadata, admit_program, observation,
    prepare_program_formula_with,
};

use super::Run;
use crate::model;

fn atom(value: i32, sign: Sign) -> Atom {
    let mut atom = Atom::new(Name::new("p").unwrap(), [Term::from(value)]);
    atom.sign = sign;
    atom
}

fn symbol(value: i32, sign: Sign) -> Symbol {
    Symbol::function(Name::new("p").unwrap(), [Symbol::Number(value)], sign)
}

fn program() -> Program {
    Program::of([
        Rule::fact(atom(1, Sign::Positive)),
        Rule::fact(atom(2, Sign::Negative)),
        Rule::fact(Choice::new(
            None,
            [ChoiceElement::new(
                atom(3, Sign::Positive).into(),
                Condition::empty(),
            )],
            None,
        )),
    ])
}

fn admit(program: Program) -> AdmittedFormula {
    prepare_program_formula_with(
        Arc::new(program),
        ProgramFormulaOptions {
            purpose: FormulaPurpose::AnswerSets,
            ..Default::default()
        },
    )
    .unwrap()
    .ground()
    .unwrap()
}

fn window() -> CancellationRun {
    CancellationSlot::default().open(None).unwrap()
}

fn run(
    admitted: &AdmittedFormula,
    window: CancellationRun,
    limits: model::OutputLimits,
) -> Run<'_> {
    let session = Session::enumerate(
        PreparedInput::formula(admitted),
        SolveConfig {
            backend: Backend::Cpu,
            models: 0,
            ..Default::default()
        },
        window.cancellation().clone(),
    )
    .unwrap();
    Run::new(
        session,
        admitted.original_program(),
        admitted.metadata(),
        window,
        limits,
    )
}

fn solved(admitted: &AdmittedFormula, window: CancellationRun) -> Solved<'_> {
    let show = ShowRule::of(
        admitted
            .original_program()
            .statements()
            .filter_map(|statement| match statement.get() {
                Statement::Show(show) => Some(show),
                _ => None,
            }),
    );
    Solved::running(
        Box::new(run(admitted, window, model::OutputLimits::default())),
        Scenario::default(),
        show,
    )
}

fn snapshot(program: Program) -> Snapshot {
    let admitted = admit(program);
    let Determination::Consistent(models) = solved(&admitted, window()).into_determination() else {
        panic!("fixture has a complete nonempty answer family");
    };
    WorldView::of(models).materialize().unwrap()
}

#[test]
fn native_query_answers_preserve_strong_negation() {
    let snapshot = snapshot(program());
    for (value, expected) in [
        (1, Answer::Yes),
        (2, Answer::No),
        (3, Answer::Unknown),
        (4, Answer::Unknown),
    ] {
        let query = Query::of(atom(value, Sign::Positive)).unwrap();
        assert_eq!(snapshot.answer(&query), expected);
    }
    assert_eq!(
        snapshot.answer(&Query::of(atom(2, Sign::Negative)).unwrap()),
        Answer::Yes
    );
}

#[test]
fn native_bindings_partition_the_complete_family() {
    let snapshot = snapshot(program());
    let pattern = BindingPattern::of(Atom::new(
        Name::new("p").unwrap(),
        [Term::variable(VarName::new("X").unwrap())],
    ))
    .unwrap();
    let bindings = snapshot.bindings(&pattern);
    assert_eq!(
        bindings.yes().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([symbol(1, Sign::Positive)])
    );
    assert_eq!(
        bindings.no().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([symbol(2, Sign::Positive)])
    );
    assert_eq!(
        bindings.unknown().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([symbol(3, Sign::Positive)])
    );
}

#[test]
fn native_snapshot_contains_the_complete_family() {
    // The helper drops both the native admitted owner and run before returning.
    let snapshot = snapshot(program());
    let fixed = AnswerSet::from([symbol(1, Sign::Positive), symbol(2, Sign::Negative)]);
    let mut optional = fixed.clone();
    optional.insert(symbol(3, Sign::Positive));
    assert_eq!(
        snapshot
            .members()
            .map(|model| model.atoms().clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([fixed, optional])
    );
    assert_eq!(snapshot.members().count(), 2);
}

#[test]
fn display_selection_keeps_hidden_atoms_queryable() {
    let program = Program::of([
        Statement::from(Rule::fact(atom(1, Sign::Positive))),
        Statement::Show(Show::All),
        Statement::Show(Show::Term(Term::from(7))),
    ]);
    let snapshot = snapshot(program);
    let model = snapshot.members().next().unwrap();
    assert_eq!(
        model.shown().symbols(),
        &BTreeSet::from([Symbol::Number(7)])
    );
    assert_eq!(
        snapshot.answer(&Query::of(atom(1, Sign::Positive)).unwrap()),
        Answer::Yes
    );
}

#[test]
fn cancelled_native_family_cannot_be_materialized() {
    let admitted = admit(program());
    let window = window();
    let cancellation = window.cancellation().clone();
    // Determination obtains its witness without spending the complete-family door.
    let Determination::Consistent(models) = solved(&admitted, window).into_determination() else {
        panic!("fixture has an answer");
    };
    cancellation.cancel();
    let fault = WorldView::of(models).materialize().unwrap_err();
    assert!(matches!(
        fault.refused(),
        Refused::Request(Presupposition::Unclosed(Truncation::Interrupted))
    ));
}

#[test]
fn expired_request_has_a_budget_conclusion() {
    let admitted = admit(program());
    let window = CancellationSlot::default()
        .open(Some(Instant::now()))
        .unwrap();
    let mut run = run(&admitted, window, model::OutputLimits::default());
    assert_eq!(run.conclusion(), None);
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), Some(Conclusion::Budget));
    assert!(run.next_model().is_none());
}

#[test]
fn exhausted_native_run_is_fused() {
    let admitted = admit(program());
    let mut run = run(&admitted, window(), model::OutputLimits::default());
    assert_eq!(run.conclusion(), None);
    assert!(run.next_model().unwrap().is_ok());
    assert!(run.next_model().unwrap().is_ok());
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), Some(Conclusion::Exhausted));
    assert!(run.next_model().is_none());
}

#[test]
fn export_resource_fault_has_no_conclusion() {
    let admitted = admit(program());
    let mut run = run(
        &admitted,
        window(),
        model::OutputLimits {
            max_symbol_work: 0,
            ..Default::default()
        },
    );
    let fault = run.next_model().unwrap().unwrap_err();
    assert_eq!(fault.locus(), Locus::Resource);
    assert!(matches!(
        fault.source().unwrap().downcast_ref::<model::OutputError>(),
        Some(model::OutputError::Symbol(
            zetesis_themelios::symbols::Failure::Stopped(model::SymbolStop::Work {
                observed: 1,
                limit: 0
            })
        ))
    ));
    assert!(run.next_model().is_none());
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), None);
}

#[test]
fn observation_fault_names_the_original_directive() {
    let admitted = admit(Program::of([
        Statement::from(Rule::fact(atom(1, Sign::Positive))),
        Statement::Show(Show::Term(Term::from(1) / Term::from(0))),
    ]));
    let original = admitted
        .original_program()
        .statements()
        .find(|statement| matches!(statement.get(), Statement::Show(_)))
        .unwrap();
    let mut run = run(&admitted, window(), model::OutputLimits::default());
    let fault = run.next_model().unwrap().unwrap_err();
    let Refused::Statement(refused) = fault.refused() else {
        panic!("observation refusal must name its statement");
    };
    assert_eq!(refused, original);
    assert!(matches!(
        fault
            .source()
            .unwrap()
            .downcast_ref::<observation::Error>()
            .unwrap()
            .kind(),
        observation::ErrorKind::Evaluation(observation::EvaluationError::Undefined)
    ));
    assert!(fault.diagnostics().is_empty());
}

#[test]
fn preparation_control_concludes_on_the_first_pull() {
    let window = window();
    let cancellation = window.cancellation().clone();
    cancellation.cancel();
    let mut run = Run::pending(
        crate::faults::control(cancellation.poll().unwrap_err()).unwrap(),
        window,
    );
    assert_eq!(run.conclusion(), None);
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), Some(Conclusion::Interrupted));
}

#[derive(Default)]
struct ClosureRoute(Option<Grounder>);

impl ExecutionObserver for ClosureRoute {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::CpuClosure { grounder, .. } = event {
            self.0 = Some(grounder);
        }
        Ok(())
    }
}

fn cancel_after_last_relational_answer(grounder: Grounder) {
    let original = Program::of([Rule::fact(Choice::new(
        None,
        [ChoiceElement::new(
            atom(1, Sign::Positive).into(),
            Condition::empty(),
        )],
        None,
    ))]);
    let admitted = admit_program(&original, ProgramAdmissionOptions::default()).unwrap();
    let metadata =
        SourceMetadata::compile(&original, MetadataLimits::default(), ProgramSite::program())
            .unwrap();
    let slot = CancellationSlot::default();
    let window = slot.open(None).unwrap();
    let mut route = ClosureRoute::default();
    let session = Session::enumerate_observed(
        PreparedInput::program(admitted.program()),
        SolveConfig {
            backend: Backend::Cpu,
            grounder,
            models: 0,
            // The first native batch has one seed; the second then contains the
            // remaining seed and observes candidate exhaustion before delivery.
            batch_size: NonZeroUsize::new(4).unwrap(),
            ..Default::default()
        },
        window.cancellation().clone(),
        &mut route,
    )
    .unwrap();
    assert_eq!(route.0, Some(grounder));
    let mut run = Run::new(
        session,
        &original,
        &metadata,
        window,
        model::OutputLimits::default(),
    );
    let atoms: BTreeSet<_> = (0..2)
        .map(|_| run.next_model().unwrap().unwrap().atoms().clone())
        .collect();
    assert_eq!(
        atoms,
        BTreeSet::from([
            AnswerSet::new(),
            AnswerSet::from([symbol(1, Sign::Positive)]),
        ])
    );
    assert_eq!(run.conclusion(), None);
    // Simulate a consumer pause without sleeping or relying on a timer. The
    // native closure can now return its already-known clean end without a poll.
    slot.cancel();
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), Some(Conclusion::Interrupted));
    assert!(run.next_model().is_none());
}

#[test]
fn lazy_final_pull_observes_request_cancellation() {
    cancel_after_last_relational_answer(Grounder::Lazy);
}

#[test]
fn eager_final_pull_observes_request_cancellation() {
    cancel_after_last_relational_answer(Grounder::Eager);
}

#[test]
fn completed_run_retires_its_cancellation_window() {
    let admitted = admit(program());
    let window = window();
    let cancellation = window.cancellation().clone();
    let mut run = run(&admitted, window, model::OutputLimits::default());
    assert!(run.next_model().unwrap().is_ok());
    assert_eq!(cancellation.poll(), Ok(()));
    assert!(run.next_model().unwrap().is_ok());
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), Some(Conclusion::Exhausted));
    assert_eq!(cancellation.poll(), Err(Stop::Cancelled));
}

#[test]
fn abandoned_run_retires_its_cancellation_window() {
    let admitted = admit(program());
    let window = window();
    let cancellation = window.cancellation().clone();
    let run = run(&admitted, window, model::OutputLimits::default());
    assert_eq!(cancellation.poll(), Ok(()));
    drop(run);
    assert_eq!(cancellation.poll(), Err(Stop::Cancelled));
}

#[test]
fn faulted_run_retires_its_cancellation_window() {
    let admitted = admit(program());
    let window = window();
    let cancellation = window.cancellation().clone();
    let mut run = run(
        &admitted,
        window,
        model::OutputLimits {
            max_symbol_work: 0,
            ..Default::default()
        },
    );
    assert_eq!(cancellation.poll(), Ok(()));
    assert!(run.next_model().unwrap().is_err());
    assert_eq!(run.conclusion(), None);
    assert_eq!(cancellation.poll(), Err(Stop::Cancelled));
}

struct RefuseFormula;

impl ExecutionObserver for RefuseFormula {
    type Error = io::Error;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if matches!(event, ExecutionObservation::Formula { .. }) {
            return Err(io::Error::other("formula observation refused"));
        }
        Ok(())
    }
}

#[test]
fn native_fault_is_not_replaced_by_cancellation() {
    let admitted = admit(program());
    let slot = CancellationSlot::default();
    let window = slot.open(None).unwrap();
    let session = Session::enumerate_observed(
        PreparedInput::formula(&admitted),
        SolveConfig {
            backend: Backend::Cpu,
            models: 0,
            ..Default::default()
        },
        window.cancellation().clone(),
        &mut RefuseFormula,
    )
    .unwrap();
    let mut run = Run::new(
        session,
        admitted.original_program(),
        admitted.metadata(),
        window,
        model::OutputLimits::default(),
    );
    slot.cancel();
    let fault = run.next_model().unwrap().unwrap_err();
    assert_eq!(fault.locus(), Locus::Engine);
    let native = fault
        .source()
        .unwrap()
        .downcast_ref::<SolveFailure>()
        .unwrap();
    let SolveError::ExecutionObservation(cause) = native.cause.as_ref() else {
        panic!("the native observation fault must be preserved");
    };
    assert_eq!(cause.to_string(), "formula observation refused");
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), None);
}

#[test]
fn pending_budget_observes_request_cancellation() {
    let slot = CancellationSlot::default();
    let window = slot.open(Some(Instant::now())).unwrap();
    assert_eq!(window.cancellation().poll(), Err(Stop::Deadline));
    let mut run = Run::pending(Conclusion::Budget, window);
    assert_eq!(run.conclusion(), None);
    slot.cancel();
    assert!(run.next_model().is_none());
    assert_eq!(run.conclusion(), Some(Conclusion::Interrupted));
}
