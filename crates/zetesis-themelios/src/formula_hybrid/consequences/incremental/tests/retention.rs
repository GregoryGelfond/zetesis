//! Completed negative scans retain only dependency-qualified evidence.

use super::*;

fn fixture() -> HybridFormula {
    admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/irrelevant-choice.lp"
    ))
}

fn completed(owner: &HybridFormula) -> crate::ConstraintChecker<'_> {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(state(&checker).valid);
    assert_eq!(state(&checker).clean, [true]);
    assert!(state(&checker).pending.is_empty());
    assert!(!checker.consequence_active);
    checker
}

#[test]
fn irrelevant_decisions_reuse_a_completed_scan() {
    let owner = fixture();
    let mut retained = completed(&owner);
    let mut rescanned = completed(&owner);
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.hold(atom(&owner, "r")));
    // Emulate the former finish boundary with equally warmed preparation.
    rescanned
        .prepared
        .as_mut()
        .unwrap()
        .incremental
        .as_mut()
        .unwrap()
        .reset();
    let before = [retained.statistics(), rescanned.statistics()];
    for checker in [&mut retained, &mut rescanned] {
        assert_eq!(
            pass(checker, &region, ConstraintRegionPass::First),
            ConstraintConsequence::NoConsequence
        );
        assert!(state(checker).valid);
        assert_eq!(state(checker).clean, [true]);
        assert!(state(checker).pending.is_empty());
    }
    assert!(
        retained.statistics().work - before[0].work < rescanned.statistics().work - before[1].work
    );
}

#[test]
fn relevant_decisions_wake_a_completed_scan() {
    let owner = fixture();
    let mut checker = completed(&owner);
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.hold(atom(&owner, "p")));
    assert!(
        matches!(pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { atom: selected, .. } if selected == atom(&owner, "q"))
    );
    assert!(!state(&checker).clean[0]);
    assert!(checker.consequence_active);
}

fn changed_bound_detects_unit(reopen: bool) {
    let owner = fixture();
    let p = atom(&owner, "p");
    let q = atom(&owner, "q");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut old = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(old.cut(p));
    if reopen {
        assert!(old.hold(q));
    }
    assert_eq!(
        pass(&mut checker, &old, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(state(&checker).valid && state(&checker).clean[0]);
    let mut next = Region::all_open(old.len());
    let expected = if reopen {
        assert!(next.hold(q));
        p
    } else {
        assert!(next.hold(p));
        q
    };
    assert!(
        matches!(pass(&mut checker, &next, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { atom, .. } if atom == expected)
    );
}

#[test]
fn reopened_bounds_discard_completed_scans() {
    changed_bound_detects_unit(true);
}

#[test]
fn opposite_polarity_siblings_discard_completed_scans() {
    changed_bound_detects_unit(false);
}

#[test]
fn default_negated_dependencies_wake_across_closures() {
    for (source, hold_q) in [
        (
            include_str!("../../../../../tests/fixtures/streamed-consequences/two-negative.lp"),
            false,
        ),
        (
            include_str!(
                "../../../../../tests/fixtures/streamed-consequences/negative-double-negative.lp"
            ),
            true,
        ),
    ] {
        let owner = admit(source);
        let mut checker = completed(&owner);
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        let q = atom(&owner, "q");
        assert!(if hold_q {
            region.hold(q)
        } else {
            region.cut(q)
        });
        let consequence = pass(&mut checker, &region, ConstraintRegionPass::First);
        let ConstraintConsequence::Hold { atom: selected, .. } = consequence else {
            panic!("expected hold");
        };
        assert_eq!(selected, atom(&owner, "p"));
    }
}

#[test]
fn a_new_first_retires_an_unapplied_batch() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/two-row-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let first = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert!(matches!(first, ConstraintConsequence::Cut { .. }));
    let before = checker.statistics().substitutions;
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        first
    );
    assert!(checker.statistics().substitutions > before);
    assert!(state(&checker).clean.iter().all(|clean| !clean));
    assert_eq!(state(&checker).next, 1);
}

fn invalidation(action: impl FnOnce(&HybridFormula, &mut crate::ConstraintChecker<'_>, &Region)) {
    let owner = fixture();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let mut checker = completed(&owner);
    action(&owner, &mut checker, &region);
    assert!(!state(&checker).valid);
    assert!(!checker.consequence_active);
}

#[test]
fn ordinary_region_checks_invalidate_completed_scans() {
    invalidation(|owner, checker, region| {
        checker
            .check_region(owner.core_theory(), region, &Cancellation::default())
            .unwrap();
    });
}

#[test]
fn final_model_checks_invalidate_completed_scans() {
    invalidation(|owner, checker, _| {
        let model = zetesis_core::Model::from_positions(owner.atom_catalog(), []).unwrap();
        checker.check(&model, &Cancellation::default()).unwrap();
    });
}

#[test]
fn foreign_owner_errors_invalidate_completed_scans() {
    invalidation(|_, checker, region| {
        let foreign = fixture();
        let error = checker
            .consequence_region(
                foreign.core_theory(),
                region,
                &Cancellation::default(),
                ConstraintRegionPass::First,
            )
            .unwrap_err();
        assert!(matches!(
            error.cause,
            crate::ConstraintCheckCause::WrongProgram
        ));
    });
}

#[test]
fn cancellation_invalidates_completed_scans() {
    invalidation(|owner, checker, region| {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let error = checker
            .consequence_region(
                owner.core_theory(),
                region,
                &cancellation,
                ConstraintRegionPass::First,
            )
            .unwrap_err();
        assert!(matches!(
            error.cause,
            crate::ConstraintCheckCause::Stopped(zetesis_cpu::Stop::Cancelled)
        ));
    });
}

#[test]
fn inactive_continuation_errors_invalidate_completed_scans() {
    invalidation(|owner, checker, region| {
        let error = checker
            .consequence_region(
                owner.core_theory(),
                region,
                &Cancellation::default(),
                ConstraintRegionPass::Continue,
            )
            .unwrap_err();
        assert!(matches!(
            error.cause,
            crate::ConstraintCheckCause::NoActiveRegion
        ));
    });
}

#[test]
fn refutations_invalidate_completed_scans() {
    invalidation(|owner, checker, region| {
        let mut violating = region.clone();
        assert!(violating.hold(atom(owner, "p")));
        assert!(violating.hold(atom(owner, "q")));
        assert!(matches!(
            pass(checker, &violating, ConstraintRegionPass::First),
            ConstraintConsequence::Refuted { .. }
        ));
    });
}

#[test]
fn no_consequence_renews_only_the_finished_closure_allowance() {
    let owner = fixture();
    // Warm the shared immutable owners before measuring an independent checker.
    let _ = completed(&owner);
    let probe = completed(&owner);
    let ceiling = probe.statistics().work;
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_work: ceiling,
            ..ConstraintCheckLimits::default()
        })
        .unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert_eq!(checker.statistics().work, ceiling);
    assert_eq!(
        checker.settled,
        (
            checker.statistics().work,
            checker.statistics().substitutions
        )
    );
    assert_eq!(checker.budget.usage().scalar_bytes, 0);
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(
        checker.statistics().work > ceiling,
        "the renewed closure has its own work ceiling"
    );
    assert!(state(&checker).valid);
}

#[test]
fn inactive_continuation_does_not_renew_preparation_allowance() {
    let owner = fixture();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let before = checker.statistics();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let error = checker
        .consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::Continue,
        )
        .unwrap_err();
    assert!(matches!(
        error.cause,
        crate::ConstraintCheckCause::NoActiveRegion
    ));
    assert_eq!(checker.statistics(), before);
    assert_eq!(checker.settled, (0, 0));
}
