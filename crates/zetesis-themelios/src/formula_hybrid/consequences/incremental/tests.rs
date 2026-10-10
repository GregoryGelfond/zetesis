mod batches;
mod delta;
mod preparation;
mod retention;
mod wakeups;

use super::*;
use crate::{
    AdmissionOptions, ConstraintCheckLimits, ConstraintRegionPass, ExpansionLimits, HybridFormula,
};
use zetesis_cpu::Cancellation;

fn admit(source: &str) -> HybridFormula {
    crate::prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}
fn atom(owner: &HybridFormula, text: &str) -> usize {
    owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| atom.predicate().arity() == 0 && atom.predicate().name() == text)
        .unwrap()
}
fn pass(
    checker: &mut crate::ConstraintChecker<'_>,
    region: &Region,
    pass: ConstraintRegionPass,
) -> ConstraintConsequence {
    checker
        .consequence_region(
            checker.owner.core_theory(),
            region,
            &Cancellation::default(),
            pass,
        )
        .unwrap()
}
fn state<'a, 'b>(checker: &'a crate::ConstraintChecker<'b>) -> &'a Incremental<'b> {
    checker
        .prepared
        .as_ref()
        .unwrap()
        .incremental
        .as_ref()
        .expect("incremental route")
}

#[test]
fn one_rule_collects_distinct_consequences() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let first = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert_eq!(state(&checker).pending.len(), 3);
    let substitutions = checker.statistics().substitutions;
    let ConstraintConsequence::Cut { atom, .. } = first else {
        panic!("expected cut");
    };
    assert!(region.cut(atom));
    let ConstraintConsequence::Cut { atom: second, .. } =
        pass(&mut checker, &region, ConstraintRegionPass::Continue)
    else {
        panic!("expected cut");
    };
    assert_ne!(atom, second);
    assert_eq!(
        checker.statistics().substitutions,
        substitutions,
        "draining uses no join"
    );
}

#[test]
fn unchanged_regions_cannot_forget_unapplied_units() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/two-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    for index in 0..5 {
        assert!(matches!(
            pass(
                &mut checker,
                &region,
                if index == 0 {
                    ConstraintRegionPass::First
                } else {
                    ConstraintRegionPass::Continue
                }
            ),
            ConstraintConsequence::Cut { .. }
        ));
    }
    assert!(state(&checker).scans.iter().all(|scan| *scan == Scan::Full));
}

#[test]
fn equal_decided_unions_do_not_authenticate_siblings() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/two-row-gated-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let p = atom(&owner, "p");
    assert!(region.hold(p));
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    assert_eq!(state(&checker).pending.len(), 2);
    let mut sibling = Region::all_open(region.len());
    assert!(sibling.cut(p));
    assert_eq!(
        pass(&mut checker, &sibling, ConstraintRegionPass::Continue),
        ConstraintConsequence::NoConsequence
    );
}

#[test]
fn completed_rules_wake_on_each_default_polarity() {
    for (source, held) in [
        (
            include_str!("../../../../tests/fixtures/streamed-consequences/wake-negative.lp"),
            false,
        ),
        (
            include_str!(
                "../../../../tests/fixtures/streamed-consequences/wake-double-negative.lp"
            ),
            true,
        ),
    ] {
        let owner = admit(source);
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        let r = atom(&owner, "r");
        assert!(
            matches!(pass(&mut checker, &region, ConstraintRegionPass::First), ConstraintConsequence::Hold { atom, .. } if atom == r)
        );
        assert_eq!(state(&checker).scans[0], Scan::Clean);
        assert!(region.hold(r));
        let q = atom(&owner, "q");
        assert!(if held { region.hold(q) } else { region.cut(q) });
        let p = atom(&owner, "p");
        assert!(
            matches!(pass(&mut checker, &region, ConstraintRegionPass::Continue), ConstraintConsequence::Cut { atom, .. } if atom == p)
        );
    }
}

#[test]
fn decided_pending_units_are_skipped() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    for atom in 0..region.len() {
        assert!(region.cut(atom));
    }
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ConstraintConsequence::NoConsequence
    );
}

#[test]
fn opposite_pending_units_refute_the_narrowing() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/two-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    let queued = state(&checker).pending[1].atom;
    assert!(region.hold(queued));
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ConstraintConsequence::Refuted { .. }
    ));
}

#[test]
fn partial_arithmetic_keeps_the_legacy_scan() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/partial-division.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
    let prepared = checker.prepared.as_ref().unwrap();
    assert_eq!(prepared.incremental_eligible, Some(false));
    assert!(prepared.incremental.is_none());
}

#[test]
fn nested_constraint_patterns_use_the_incremental_route() {
    // These are the positive structural and negative constructor forms in USA;
    // producer arithmetic is intentionally outside the streamed partition.
    for source in [
        include_str!(
            "../../../../tests/fixtures/streamed-consequences/nested-negative-constructor.lp"
        ),
        include_str!(
            "../../../../tests/fixtures/streamed-consequences/nested-positive-constructor.lp"
        ),
        include_str!(
            "../../../../tests/fixtures/streamed-consequences/nested-constructor-guard.lp"
        ),
        include_str!("../../../../tests/fixtures/streamed-consequences/nested-time-comparison.lp"),
    ] {
        let owner = admit(source);
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        // Facts are already held after core closure in the real route.
        for position in 0..region.len() {
            if owner
                .atom_catalog()
                .atoms()
                .at(position)
                .unwrap()
                .predicate()
                .name()
                == "valve"
            {
                assert!(region.hold(position));
            }
        }
        let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
        assert_eq!(
            checker.prepared.as_ref().unwrap().incremental_eligible,
            Some(true),
            "{source}"
        );
    }
}

#[test]
fn draining_a_batch_keeps_the_original_work_ceiling() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let mut probe = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert!(matches!(
        pass(&mut probe, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    // Use the now-shared index for both receipt measurements.
    let mut probe = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert!(matches!(
        pass(&mut probe, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    let limit = probe.statistics().work;
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_work: limit,
            ..ConstraintCheckLimits::default()
        })
        .unwrap();
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    assert_eq!(checker.statistics().work, limit);
    let failure = checker
        .consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::Continue,
        )
        .unwrap_err();
    assert!(
        matches!(failure.cause, crate::ConstraintCheckCause::Source(ref error) if matches!(error.as_ref(), FormulaFailure::Limit { resource: crate::FormulaResource::Work, observed, limit: ceiling, .. } if *observed == u128::from(limit) + 1 && *ceiling == u128::from(limit)))
    );
    assert_eq!(failure.statistics.work, limit);
    assert!(!state(&checker).valid);
    assert!(!checker.consequence_active);
}

#[test]
fn ordinary_checks_invalidate_candidate_evidence() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/two-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    assert!(state(&checker).valid);
    checker
        .check_region(owner.core_theory(), &region, &Cancellation::default())
        .unwrap();
    assert!(!state(&checker).valid);
    let failure = checker
        .consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::Continue,
        )
        .unwrap_err();
    assert!(matches!(
        failure.cause,
        crate::ConstraintCheckCause::NoActiveRegion
    ));
}

#[test]
fn foreign_theories_cannot_drain_a_pending_batch() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/two-row-units.lp"
    ));
    let foreign = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/two-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    let failure = checker
        .consequence_region(
            foreign.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::Continue,
        )
        .unwrap_err();
    assert!(matches!(
        failure.cause,
        crate::ConstraintCheckCause::WrongProgram
    ));
    assert!(!state(&checker).valid);
}

#[test]
fn arithmetic_inside_constructors_disables_batching() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/nested-arithmetic.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert_eq!(
        checker.prepared.as_ref().unwrap().incremental_eligible,
        Some(false)
    );
}

#[test]
fn oversized_constructor_prefixes_fail_during_capture() {
    // Eighteen shared DAG constructors exceed the logical node ceiling without
    // allocating a corresponding exponential source expression or owned tree.
    let failure = crate::prepare_formula(
        include_str!(
            "../../../../tests/fixtures/streamed-consequences/oversized-constructor-chain.lp"
        )
        .into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .and_then(crate::PreparedFormula::ground_hybrid)
    .expect_err("oversized constructor cannot be captured");
    assert!(
        matches!(
            failure,
            FormulaFailure::Expansion(crate::ExpansionFailure::Admission(
                crate::AdmissionFailure::Construction {
                    error: zetesis_core::ConstructionError::Value(
                        zetesis_core::ValueError::Limit {
                            resource: zetesis_core::ValueResource::Nodes,
                            observed: 524_287,
                            limit: 262_144,
                        }
                    ),
                    ..
                }
            ))
        ),
        "{failure:?}"
    );
}

#[test]
fn cancellation_retires_pending_consequences() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    assert_eq!(state(&checker).pending.len(), 3);
    let accepted = checker.statistics();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let failure = checker
        .consequence_region(
            owner.core_theory(),
            &region,
            &cancellation,
            ConstraintRegionPass::Continue,
        )
        .unwrap_err();
    assert!(matches!(
        failure.cause,
        crate::ConstraintCheckCause::Stopped(zetesis_cpu::Stop::Cancelled)
    ));
    assert_eq!(failure.statistics, accepted);
    assert!(!state(&checker).valid);
    assert!(!checker.consequence_active);
    let mut next = Region::all_open(region.len());
    for atom in 0..next.len() {
        assert!(next.cut(atom));
    }
    assert_eq!(
        pass(&mut checker, &next, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
}

#[test]
fn work_refusal_discards_a_partial_rule_batch() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/three-row-units.lp"
    ));
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let mut probe = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert!(matches!(
        pass(&mut probe, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    let complete = probe.statistics();
    let mut observed_partial = false;
    for limit in 0..complete.work {
        let Ok(mut checker) = owner.checker(ConstraintCheckLimits {
            max_work: limit,
            ..ConstraintCheckLimits::default()
        }) else {
            continue;
        };
        let result = checker.consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::First,
        );
        let Err(failure) = result else {
            continue;
        };
        if failure.statistics.substitutions == 0
            || failure.statistics.substitutions >= complete.substitutions
        {
            continue;
        }
        assert!(
            matches!(failure.cause, crate::ConstraintCheckCause::Source(ref error) if matches!(error.as_ref(), FormulaFailure::Limit { resource: crate::FormulaResource::Work, .. }))
        );
        assert_eq!(failure.statistics.work, limit);
        assert!(!state(&checker).valid);
        assert!(state(&checker).scans.iter().all(|scan| *scan == Scan::Full));
        assert!(!checker.consequence_active);
        observed_partial = true;
        break;
    }
    assert!(
        observed_partial,
        "at least one admitted row precedes the work refusal"
    );
}

#[test]
fn pending_cleanup_visits_only_undelivered_units() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/sparse-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
    let state = checker
        .prepared
        .as_mut()
        .unwrap()
        .incremental
        .as_mut()
        .unwrap();
    assert_eq!(state.pending.len() - state.next, 2);
    state.reset();
    let mut counters = Counters::default();
    state
        .retire_pending(&FormulaLimits::default(), &mut counters)
        .unwrap();
    assert_eq!(
        counters.accounting.work, 2,
        "unrelated catalog atoms need no cleanup"
    );
    assert!(state.queued.iter().all(Option::is_none));
}

#[test]
fn pending_cleanup_resumes_after_work_refusal() {
    let owner = admit(include_str!(
        "../../../../tests/fixtures/streamed-consequences/four-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
    let state = checker
        .prepared
        .as_mut()
        .unwrap()
        .incremental
        .as_mut()
        .unwrap();
    assert_eq!(state.pending.len() - state.next, 3);
    state.reset();
    let mut counters = Counters::default();
    let limits = FormulaLimits {
        max_work: 1,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        state.retire_pending(&limits, &mut counters),
        Err(FormulaFailure::Limit {
            resource: crate::FormulaResource::Work,
            ..
        })
    ));
    assert!(!state.valid);
    assert_eq!(state.pending.len() - state.next, 2);
    state
        .retire_pending(&FormulaLimits::default(), &mut counters)
        .unwrap();
    assert_eq!(
        counters.accounting.work, 3,
        "accepted cleanup is not repeated"
    );
    assert!(state.queued.iter().all(Option::is_none));
    assert!(state.pending.is_empty());
}
