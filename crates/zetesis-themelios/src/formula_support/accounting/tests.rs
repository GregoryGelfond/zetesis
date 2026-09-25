use super::Accounting;
use crate::formula_support::{Computation, Support, SupportCatalog};
use crate::{
    FormulaFailure, FormulaLimits, FormulaResource, GroundingObserver, GroundingOutcome,
    GroundingPhase, GroundingWork,
};
use std::cell::RefCell;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::Value;
use zetesis_cpu::{Cancellation, Stop};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

#[test]
fn refused_checks_retain_their_accepted_prefix() {
    let mut accounting = Accounting::default();
    let limits = FormulaLimits {
        max_work: 3,
        max_substitutions: 1,
        ..FormulaLimits::default()
    };
    let result = accounting.with_cancellation(&Cancellation::default(), |counters| {
        counters.charge_work(2, &limits, location())?;
        counters.substitution(&limits, location())?;
        counters.charge_work(2, &limits, location())
    });
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 4,
            limit: 3,
            ..
        })
    ));
    assert_eq!(accounting.work, 2);
    assert_eq!(accounting.substitutions, 1);
    let next = accounting.with_cancellation(&Cancellation::default(), |counters| {
        counters.substitution(&limits, location())
    });
    assert!(matches!(
        next,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Substitutions,
            observed: 2,
            limit: 1,
            ..
        })
    ));
    assert_eq!(accounting.work, 2);
    assert_eq!(accounting.substitutions, 1);
}

#[test]
fn generated_identities_survive_check_boundaries() {
    let mut accounting = Accounting::default();
    let mut owner = SupportCatalog::default();
    let limits = FormulaLimits {
        max_generated_values: 1,
        ..FormulaLimits::default()
    };
    let value = Value::Symbol("generated".into());
    accounting.with_cancellation(&Cancellation::default(), |counters| {
        let (relations, mut append) = owner.split(&limits, counters, location()).unwrap();
        let support = Support::indexed(&relations, &limits, counters, location()).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        let key = computation
            .import((&value).into(), &limits, counters, location())
            .unwrap();
        counters
            .generated(&key, &computation, &limits, location())
            .unwrap();
    });
    assert_eq!(
        accounting.generated_values.as_ref().unwrap().values.len(),
        1
    );
    let result = accounting.with_cancellation(&Cancellation::default(), |counters| {
        let (relations, mut append) = owner.split(&limits, counters, location())?;
        let support = Support::indexed(&relations, &limits, counters, location())?;
        let mut computation = Computation::new(&mut append, &support);
        let repeated = computation.import((&value).into(), &limits, counters, location())?;
        counters.generated(&repeated, &computation, &limits, location())?;
        let typed = Value::String("generated".into());
        let distinct = computation.import((&typed).into(), &limits, counters, location())?;
        counters.generated(&distinct, &computation, &limits, location())
    });
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::GeneratedValues,
            observed: 2,
            limit: 1,
            ..
        })
    ));
    assert_eq!(
        accounting.generated_values.as_ref().unwrap().values.len(),
        1
    );
}

#[test]
fn stopped_checks_do_not_retain_the_previous_control() {
    let mut accounting = Accounting::default();
    let limits = FormulaLimits::default();
    accounting.with_cancellation(&Cancellation::default(), |counters| {
        counters.work(&limits, location()).unwrap();
    });
    let stopped = Cancellation::default();
    stopped.cancel();
    let result =
        accounting.with_cancellation(&stopped, |counters| counters.work(&limits, location()));
    assert!(matches!(
        result,
        Err(FormulaFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        })
    ));
    assert_eq!(accounting.work, 1);
    accounting.with_cancellation(&Cancellation::default(), |counters| {
        counters.work(&limits, location()).unwrap();
    });
    assert_eq!(accounting.work, 2);
}

#[test]
fn unwind_restores_accepted_history() {
    let mut accounting = Accounting::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        accounting.with_cancellation(&Cancellation::default(), |counters| {
            counters
                .work(&FormulaLimits::default(), location())
                .unwrap();
            panic!("controlled unwind after an accepted charge");
        });
    }));
    assert!(result.is_err());
    assert_eq!(accounting.work, 1);
}

#[test]
fn unwind_preserves_shared_admission_charges() {
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits {
        max_work: 1,
        ..Default::default()
    });
    let cancellation = Cancellation::default();
    let mut accounting =
        super::Counters::with_allowance(allowance.clone(), &cancellation).into_accounting();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        accounting.with_cancellation(&cancellation, |counters| {
            counters
                .work(&FormulaLimits::default(), location())
                .unwrap();
            panic!("controlled unwind after shared admission");
        });
    }));
    assert!(result.is_err());
    assert_eq!(accounting.work, 1);
    assert_eq!(allowance.statistics().work, 1);
    let refused = accounting.with_cancellation(&cancellation, |counters| {
        counters.work(&FormulaLimits::default(), location())
    });
    assert!(matches!(
        refused,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 2,
            limit: 1,
            ..
        })
    ));
    assert_eq!(accounting.work, 1);
    assert_eq!(allowance.statistics().work, 1);
}

#[test]
fn zero_work_preserves_an_exhausted_allowance() {
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits {
        max_work: 1,
        ..Default::default()
    });
    let mut counters = super::Counters::with_allowance(allowance.clone(), &Cancellation::default());
    let limits = FormulaLimits {
        max_work: 1,
        ..FormulaLimits::default()
    };
    counters.work(&limits, location()).unwrap();
    counters.charge_work(0, &limits, location()).unwrap();
    assert_eq!(counters.accounting.work, 1);
    assert_eq!(allowance.statistics().work, 1);
}

#[test]
fn zero_work_observes_cancellation() {
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits::default());
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut counters = super::Counters::with_allowance(allowance.clone(), &cancellation);
    assert!(matches!(
        counters.charge_work(0, &FormulaLimits::default(), location()),
        Err(FormulaFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        })
    ));
    assert_eq!(counters.accounting.work, 0);
    assert_eq!(allowance.statistics().work, 0);
}

#[derive(Default)]
struct PhaseObserver(RefCell<Vec<GroundingWork>>);
impl GroundingObserver for PhaseObserver {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<Location>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.borrow_mut().push(work);
    }
}

#[test]
fn resume_preserves_generated_history_and_live_workspace() {
    use crate::formula_support::{Buffer, Counters};
    use crate::grounding_observer::{Event, Profile, Work};
    let limits = FormulaLimits {
        max_generated_values: 1,
        ..FormulaLimits::default()
    };
    let first_observer = PhaseObserver::default();
    let first_profile = Profile::new(Some(&first_observer));
    let mut owner = SupportCatalog::default();
    let value = Value::String("retained generated identity".into());
    let mut counters = Counters::resume(Accounting::default(), first_profile.work());
    let mut retained = first_profile
        .phase(GroundingPhase::SupportCompletion, None, || {
            let (relations, mut append) = owner.split(&limits, &mut counters, location())?;
            let support = Support::indexed(&relations, &limits, &counters, location())?;
            let mut computation = Computation::new(&mut append, &support);
            let key = computation.import((&value).into(), &limits, &mut counters, location())?;
            counters.generated(&key, &computation, &limits, location())?;
            counters.substitution(&limits, location())?;
            let mut retained = Buffer::new(&computation, &limits, &mut counters, location())?;
            retained.push(7_usize, &computation, &limits, &mut counters, location())?;
            counters.record(Event::JoinRow);
            counters.record(Event::JoinRow);
            Ok::<_, FormulaFailure>(retained)
        })
        .unwrap();
    assert_eq!(first_observer.0.borrow()[0].join_rows, Some(2));
    let accounting = counters.into_accounting();
    let workspace = accounting.workspace.clone();
    let work = accounting.work;
    let bytes = workspace.bytes();
    assert!(bytes > accounting.generated_values.as_ref().unwrap().bytes());

    let second_observer = PhaseObserver::default();
    let second_profile = Profile::new(Some(&second_observer));
    let mut counters = Counters::resume(accounting, second_profile.work());
    assert_eq!(counters.accounting.work, work);
    assert_eq!(counters.accounting.substitutions, 1);
    assert_eq!(counters.accounting.workspace.bytes(), bytes);
    second_profile
        .phase(GroundingPhase::SupportCompletion, None, || {
            let (relations, mut append) = owner.split(&limits, &mut counters, location())?;
            let support = Support::indexed(&relations, &limits, &counters, location())?;
            let mut computation = Computation::new(&mut append, &support);
            // Buffer admission authenticates its original lease against the resumed
            // computation's workspace; recreating the ledger would refuse here.
            retained.push(11, &computation, &limits, &mut counters, location())?;
            let key = computation.import((&value).into(), &limits, &mut counters, location())?;
            counters.generated(&key, &computation, &limits, location())?;
            let distinct = computation.import(
                (&Value::Symbol("other".into())).into(),
                &limits,
                &mut counters,
                location(),
            )?;
            let refused = counters.generated(&distinct, &computation, &limits, location());
            assert!(matches!(
                refused,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::GeneratedValues,
                    observed: 2,
                    limit: 1,
                    ..
                })
            ));
            counters.record(Event::JoinRow);
            Ok::<_, FormulaFailure>(())
        })
        .unwrap();
    assert_eq!(retained.slice(), [7, 11]);
    assert_eq!(second_observer.0.borrow()[0].join_rows, Some(1));
    assert_eq!(first_observer.0.borrow()[0].join_rows, Some(2));
    let accounting = counters.into_accounting();
    assert_eq!(
        accounting.generated_values.as_ref().unwrap().values.len(),
        1
    );
    drop(retained);
    assert_eq!(
        workspace.bytes(),
        accounting.generated_values.as_ref().unwrap().bytes()
    );
    // Moving history once more without observation does not retain either bank.
    drop(Counters::resume(accounting, Work::default()));
    assert_eq!(workspace.bytes(), 0);
}

#[test]
fn resume_preserves_exact_shared_allowance_without_previous_cancellation() {
    use crate::formula_support::Counters;
    use crate::grounding_observer::Work;

    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits {
        max_work: 3,
        max_substitutions: 1,
        ..Default::default()
    });
    let previous_control = Cancellation::default();
    let limits = FormulaLimits::default();
    let mut counters = Counters::with_allowance(allowance.clone(), &previous_control);
    counters.charge_work(2, &limits, location()).unwrap();
    counters.substitution(&limits, location()).unwrap();
    let accounting = counters.into_accounting();
    previous_control.cancel();
    let mut resumed = Counters::resume(accounting, Work::default());
    assert_eq!(allowance.statistics().work, 2);
    resumed.work(&limits, location()).unwrap();
    assert_eq!(resumed.accounting.work, 3);
    assert_eq!(allowance.statistics().work, 3);
    assert!(matches!(
        resumed.work(&limits, location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 4,
            limit: 3,
            ..
        })
    ));
    assert!(matches!(
        resumed.substitution(&limits, location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Substitutions,
            observed: 2,
            limit: 1,
            ..
        })
    ));
    assert_eq!(resumed.accounting.work, 3);
    assert_eq!(resumed.accounting.substitutions, 1);
    assert_eq!(allowance.statistics().work, 3);
    assert_eq!(allowance.statistics().substitutions, 1);
}
