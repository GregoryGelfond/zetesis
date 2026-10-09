use super::*;
use crate::FormulaResource;

fn retained_bytes(prepared: &PreparedConstraints<'_>) -> u128 {
    let limits = FormulaLimits {
        max_support_bytes: 0,
        ..prepared.limits
    };
    match prepared.completed.admit_workspace(
        0,
        &limits,
        &Counters::default(),
        prepared.source.location,
    ) {
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            observed,
            limit: 0,
            ..
        }) => observed,
        other => panic!("expected retained-capacity receipt, got {other:?}"),
    }
}

fn prepare<'a>(checker: &mut crate::ConstraintChecker<'a>, counters: &mut Counters) -> &'a Plan {
    let prepared = checker.prepared.as_mut().unwrap();
    prepared.prepare_selection(checker.owner, counters).unwrap();
    prepared.prepare_incremental_plan(counters).unwrap()
}

#[test]
fn shared_plan_admission_counts_retained_storage_once() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/signed-plan.lp"
    ));
    let mut first = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut second = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut counters = Counters::default();
    let plan = prepare(&mut first, &mut counters);
    assert_eq!(counters.workspace_bytes(), 0, "builder scratch is retired");
    let prepared = second.prepared.as_mut().unwrap();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let retained = retained_bytes(prepared);
    let before = counters.accounting.work;
    assert!(std::ptr::eq(
        plan,
        prepared.prepare_incremental_plan(&mut counters).unwrap()
    ));
    assert_eq!(
        counters.accounting.work,
        before + 1,
        "borrow does no mapping work"
    );
    assert_eq!(retained_bytes(prepared), retained + plan.retained_bytes());
    let before = counters.accounting.work;
    assert!(std::ptr::eq(
        plan,
        prepared.prepare_incremental_plan(&mut counters).unwrap()
    ));
    assert_eq!(counters.accounting.work, before);
    assert_eq!(retained_bytes(prepared), retained + plan.retained_bytes());
    assert_eq!(counters.workspace_bytes(), 0);
}

#[test]
fn racing_checkers_publish_one_complete_plan() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/signed-plan.lp"
    ));
    let barrier = std::sync::Barrier::new(4);
    let plans = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
                    let mut counters = Counters::default();
                    checker
                        .prepared
                        .as_mut()
                        .unwrap()
                        .prepare_selection(owner.core(), &mut counters)
                        .unwrap();
                    barrier.wait();
                    let plan = prepare(&mut checker, &mut counters);
                    assert_eq!(plan.offsets.len(), 2);
                    assert_eq!(plan.dependencies.len(), 2);
                    assert_eq!(
                        plan.predicate_count, 2,
                        "strong negation is a distinct group"
                    );
                    assert_eq!(
                        plan.atom_predicates.len(),
                        owner.atom_catalog().atoms().len()
                    );
                    assert!(plan.atom_predicates.iter().all(Option::is_some));
                    assert_eq!(counters.workspace_bytes(), 0);
                    std::ptr::from_ref(plan) as usize
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(plans.iter().all(|&plan| plan == plans[0]));
}

#[test]
fn equal_contents_do_not_share_plan_authority() {
    let first = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/one-atom-unit.lp"
    ));
    let second = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/one-atom-unit.lp"
    ));
    let mut a = first.checker(ConstraintCheckLimits::default()).unwrap();
    let mut b = second.checker(ConstraintCheckLimits::default()).unwrap();
    let mut counters = Counters::default();
    let a = prepare(&mut a, &mut counters);
    assert!(second.core().0.incremental.get().is_none());
    let b = prepare(&mut b, &mut counters);
    assert!(!std::ptr::eq(a, b));
    assert_eq!(a.dependencies, b.dependencies);
    assert_eq!(a.atom_predicates, b.atom_predicates);
}

#[test]
fn refused_preparation_preserves_retryability() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let retained = retained_bytes(prepared);
    let original = prepared.limits;
    let before = counters.accounting.work;
    prepared.limits.max_work = before + 3;
    assert!(matches!(
        prepared.prepare_incremental_plan(&mut counters),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    assert_eq!(counters.accounting.work, before + 3);
    assert!(prepared.incremental_plan.is_none());
    assert!(owner.core().0.incremental.get().is_none());
    assert_eq!(counters.workspace_bytes(), 0);
    assert_eq!(retained_bytes(prepared), retained);
    prepared.limits = original;
    let plan = prepared.prepare_incremental_plan(&mut counters).unwrap();
    assert!(counters.accounting.work > before + 3);
    assert_eq!(retained_bytes(prepared), retained + plan.retained_bytes());
    assert_eq!(counters.workspace_bytes(), 0);
}

#[test]
fn shared_borrow_still_refuses_insufficient_storage() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let mut first = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut second = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut counters = Counters::default();
    let plan = prepare(&mut first, &mut counters);
    let prepared = second.prepared.as_mut().unwrap();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let retained = retained_bytes(prepared);
    let original = prepared.limits.max_support_bytes;
    prepared.limits.max_support_bytes =
        usize::try_from(retained + plan.retained_bytes() - 1).unwrap();
    assert!(matches!(
        prepared.prepare_incremental_plan(&mut counters),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert!(prepared.incremental_plan.is_none());
    assert_eq!(retained_bytes(prepared), retained);
    assert_eq!(counters.workspace_bytes(), 0);
    prepared.limits.max_support_bytes = original;
    assert!(std::ptr::eq(
        plan,
        prepared.prepare_incremental_plan(&mut counters).unwrap()
    ));
    assert_eq!(retained_bytes(prepared), retained + plan.retained_bytes());
}

#[test]
fn cancelled_borrow_does_not_admit_shared_bytes() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/one-atom-unit.lp"
    ));
    let mut first = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut second = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut counters = Counters::default();
    let _ = prepare(&mut first, &mut counters);
    let prepared = second.prepared.as_mut().unwrap();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let retained = retained_bytes(prepared);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut counters = Counters::default().with_cancellation(Some(&cancellation));
    let Err(error) = prepared.prepare_incremental_plan(&mut counters) else {
        panic!("cancelled borrowing must fail");
    };
    assert_eq!(error.interruption(), Some(zetesis_cpu::Stop::Cancelled));
    assert_eq!(counters.accounting.work, 0);
    assert!(prepared.incremental_plan.is_none());
    assert_eq!(retained_bytes(prepared), retained);
}

#[test]
fn shared_plan_does_not_share_candidate_progress() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/two-row-gated-units.lp"
    ));
    let mut first = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut second = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut held = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(held.hold(atom(&owner, "p")));
    let mut cut = Region::all_open(held.len());
    assert!(cut.cut(atom(&owner, "p")));
    assert!(matches!(
        pass(&mut first, &held, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    assert_eq!(
        pass(&mut second, &cut, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(std::ptr::eq(state(&first).plan, state(&second).plan));
    assert!(!state(&first).pending.is_empty());
    assert!(state(&second).pending.is_empty());
    assert_ne!(state(&first).held, state(&second).held);
    assert_ne!(state(&first).scans, state(&second).scans);
}

#[test]
fn discarded_candidate_state_keeps_one_plan_admission() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let mut counters = Counters::default();
    let plan = prepare(&mut checker, &mut counters);
    let prepared = checker.prepared.as_mut().unwrap();
    let retained = retained_bytes(prepared);
    let state = Incremental::new(prepared, &mut counters, &region).unwrap();
    assert!(counters.workspace_bytes() > 0);
    drop(state);
    assert_eq!(counters.workspace_bytes(), 0);
    let state = Incremental::new(prepared, &mut counters, &region).unwrap();
    assert!(std::ptr::eq(plan, state.plan));
    assert_eq!(retained_bytes(prepared), retained);
    drop(state);
    assert_eq!(counters.workspace_bytes(), 0);
}
