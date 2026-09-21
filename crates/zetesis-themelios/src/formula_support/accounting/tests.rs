use super::Accounting;
use crate::expansion::Budget;
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource};
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
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let limits = FormulaLimits {
        max_generated_values: 1,
        ..FormulaLimits::default()
    };
    // Inline numbers carry no variable payload. A symbol makes the copied-byte
    // assertion meaningful; a same-spelling string is a different typed value.
    let value = Value::Symbol("generated".into());
    accounting.with_cancellation(&Cancellation::default(), |counters| {
        counters
            .generated(&value, &limits, &mut budget, location())
            .unwrap();
    });
    let copied = budget.usage().scalar_bytes;
    assert!(copied > 0);
    let result = accounting.with_cancellation(&Cancellation::default(), |counters| {
        counters.generated(&value, &limits, &mut budget, location())?;
        counters.generated(
            &Value::String("generated".into()),
            &limits,
            &mut budget,
            location(),
        )
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
    assert_eq!(budget.usage().scalar_bytes, copied);
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
