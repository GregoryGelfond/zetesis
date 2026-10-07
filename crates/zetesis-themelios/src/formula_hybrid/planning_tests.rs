//! A streamed join plan uses its checker's work and current control.

use super::{ConstraintCheckLimits, HybridFormula, check_budget};
use crate::formula_support::{Counters, PreparedRule};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure};
use zetesis_cpu::{Cancellation, Stop};

fn owner() -> HybridFormula {
    crate::prepare_formula(
        "p. q. :-p,q.".into(),
        crate::AdmissionOptions::default(),
        crate::ExpansionLimits::default(),
        crate::FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn plan_work(
    owner: &HybridFormula,
    ceiling: u64,
    cancellation: &Cancellation,
) -> Result<usize, FormulaFailure> {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut budget = check_budget(
        ConstraintCheckLimits {
            max_work: ceiling,
            ..Default::default()
        },
        None,
    );
    budget.set_cancellation(Some(cancellation.clone()));
    let _plan = PreparedRule::new(
        &prepared.source.rules[0],
        &mut prepared.completed,
        &prepared.limits,
        &mut budget,
        &mut Counters::default(),
    )?;
    Ok(budget.usage().term_work)
}

#[test]
fn planning_uses_the_explicit_checker_allowance() {
    let owner = owner();
    let cancellation = Cancellation::default();
    let needed = plan_work(&owner, u64::MAX, &cancellation).unwrap();
    assert!(needed > 0);
    let exact = u64::try_from(needed).unwrap();
    assert_eq!(plan_work(&owner, exact, &cancellation).unwrap(), needed);
    let error = plan_work(&owner, exact - 1, &cancellation).unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::TermWork,
            limit,
            observed,
            ..
        }) if limit == u128::from(exact - 1) && observed > limit
    ));
}

#[test]
fn planning_observes_the_attached_control() {
    let owner = owner();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = plan_work(&owner, u64::MAX, &cancellation).unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        })
    ));
}
