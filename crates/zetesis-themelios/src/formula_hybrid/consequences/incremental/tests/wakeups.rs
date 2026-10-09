//! Single changed atoms can be outside a completed template's possible reads.

use super::super::wakeups::{Change, classify};
use super::*;
use zetesis_core::{Sign, ValueNodeRef};

const CONSTRUCTOR: &str =
    include_str!("../../../../../tests/fixtures/streamed-consequences/wakeups/constructor.lp");
const NESTED: &str =
    include_str!("../../../../../tests/fixtures/streamed-consequences/wakeups/nested.lp");

fn numbered(owner: &HybridFormula, name: &str, column: usize, value: i32) -> usize {
    owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == name
                && atom
                    .values()
                    .get(column)
                    .is_some_and(|term| term.descriptor() == ValueNodeRef::Number(value))
        })
        .unwrap()
}

fn completed(owner: &HybridFormula) -> crate::ConstraintChecker<'_> {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(state(&checker).valid);
    assert_eq!(state(&checker).scans, [Scan::Clean]);
    checker
}

fn changed_group(
    checker: &crate::ConstraintChecker<'_>,
    atoms: &[usize],
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    let state = state(checker);
    let prepared = checker.prepared.as_ref().unwrap();
    let mut changes = vec![Change::None; state.plan.predicate_count];
    for &atom in atoms {
        if let Some(group) = state.plan.atom_predicates[atom] {
            changes[group].include(atom);
        }
    }
    classify(
        &prepared.source.rules[0],
        &state.plan.dependencies,
        &changes,
        prepared,
        &Region::all_open(state.queued.len()),
        counters,
    )
    .map(|scan| scan != Scan::Clean)
}

#[test]
fn nonmatching_changes_skip_the_source_scan() {
    for (source, irrelevant) in [(CONSTRUCTOR, 1), (CONSTRUCTOR, 2), (NESTED, 1), (NESTED, 2)] {
        let owner = admit(source);
        let mut retained = completed(&owner);
        let mut rescanned = completed(&owner);
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        assert!(region.hold(numbered(&owner, "h", 1, irrelevant)));
        // Same warmed owners and valid masks; force only this rule's scan.
        rescanned
            .prepared
            .as_mut()
            .unwrap()
            .incremental
            .as_mut()
            .unwrap()
            .scans[0] = Scan::Full;
        let before = [retained.statistics(), rescanned.statistics()];
        for checker in [&mut retained, &mut rescanned] {
            assert_eq!(
                pass(checker, &region, ConstraintRegionPass::First),
                ConstraintConsequence::NoConsequence
            );
            assert!(state(checker).valid && state(checker).scans[0] == Scan::Clean);
        }
        assert!(
            retained.statistics().work - before[0].work
                < rescanned.statistics().work - before[1].work
        );
    }
}

#[test]
fn matching_nested_changes_wake_the_source_scan() {
    for (source, matching) in [(CONSTRUCTOR, 0), (CONSTRUCTOR, 3), (NESTED, 0)] {
        let owner = admit(source);
        let mut checker = completed(&owner);
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        assert!(region.hold(numbered(&owner, "h", 1, matching)));
        let expected = numbered(&owner, "q", 0, matching);
        assert!(
            matches!(pass(&mut checker, &region, ConstraintRegionPass::First), ConstraintConsequence::Cut { atom, .. } if atom == expected)
        );
        assert_ne!(state(&checker).scans[0], Scan::Clean);
    }
}

#[test]
fn repeated_variables_conservatively_wake() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/wakeups/repeated.lp"
    ));
    let mut checker = completed(&owner);
    let mismatch = numbered(&owner, "h", 1, 1);
    assert!(changed_group(&checker, &[mismatch], &mut Counters::default()).unwrap());
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.hold(mismatch));
    // The ordinary matcher, not read-set refinement, rejects the unequal pair.
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(region.hold(numbered(&owner, "h", 1, 0)));
    let q = numbered(&owner, "q", 0, 0);
    assert!(
        matches!(pass(&mut checker, &region, ConstraintRegionPass::First), ConstraintConsequence::Cut { atom, .. } if atom == q)
    );
}

#[test]
fn lowered_constructor_slots_conservatively_wake() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/wakeups/unknown.lp"
    ));
    let checker = completed(&owner);
    let other = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == "h"
                && matches!(
                    atom.values().get(0).unwrap().descriptor(),
                    ValueNodeRef::Function { name: "other", .. }
                )
        })
        .unwrap();
    assert!(changed_group(&checker, &[other], &mut Counters::default()).unwrap());
}

#[test]
fn several_changes_keep_conservative_invalidation() {
    let owner = admit(CONSTRUCTOR);
    let checker = completed(&owner);
    let first = numbered(&owner, "h", 1, 1);
    let second = numbered(&owner, "h", 1, 2);
    for atom in [first, second] {
        assert!(!changed_group(&checker, &[atom], &mut Counters::default()).unwrap());
    }
    assert!(changed_group(&checker, &[first, second], &mut Counters::default()).unwrap());
}

#[test]
fn default_polarities_retain_relevant_dependencies() {
    for (source, held) in [
        (
            include_str!("../../../../../tests/fixtures/streamed-consequences/wakeups/negative.lp"),
            false,
        ),
        (
            include_str!(
                "../../../../../tests/fixtures/streamed-consequences/wakeups/double-negative.lp"
            ),
            true,
        ),
    ] {
        let owner = admit(source);
        let mut checker = completed(&owner);
        let irrelevant = numbered(&owner, "p", 0, 2);
        assert!(!changed_group(&checker, &[irrelevant], &mut Counters::default()).unwrap());
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        let relevant = numbered(&owner, "p", 0, 1);
        assert!(if held {
            region.hold(relevant)
        } else {
            region.cut(relevant)
        });
        let q = atom(&owner, "q");
        assert!(
            matches!(pass(&mut checker, &region, ConstraintRegionPass::First), ConstraintConsequence::Cut { atom, .. } if atom == q)
        );
    }
}

#[test]
fn strong_negation_retains_its_separate_dependency() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/wakeups/signed.lp"
    ));
    let mut checker = completed(&owner);
    let positive = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == "p" && atom.predicate().sign() == Sign::Positive
        })
        .unwrap();
    assert!(!changed_group(&checker, &[positive], &mut Counters::default()).unwrap());
    let negative = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == "p"
                && atom.predicate().sign() == Sign::Negative
                && atom.values().get(0).unwrap().descriptor() == ValueNodeRef::Number(1)
        })
        .unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.cut(negative));
    let q = atom(&owner, "q");
    assert!(
        matches!(pass(&mut checker, &region, ConstraintRegionPass::First), ConstraintConsequence::Cut { atom, .. } if atom == q)
    );
}

#[test]
fn a_refinement_refusal_retains_only_accepted_reads() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/wakeups/negative.lp"
    ));
    let mut checker = completed(&owner);
    let irrelevant = numbered(&owner, "p", 0, 2);
    let mut baseline = Counters::default();
    assert!(!changed_group(&checker, &[irrelevant], &mut baseline).unwrap());
    assert!(baseline.accounting.work > 0);
    for limit in 0..baseline.accounting.work {
        checker.prepared.as_mut().unwrap().limits.max_work = limit;
        let mut counters = Counters::default();
        assert!(matches!(
            changed_group(&checker, &[irrelevant], &mut counters),
            Err(FormulaFailure::Limit {
                resource: crate::FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(counters.accounting.work, limit);
        assert_eq!(counters.workspace_bytes(), 0);
    }
}

#[test]
fn cancelled_refinement_accepts_no_reads() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/wakeups/negative.lp"
    ));
    let checker = completed(&owner);
    let irrelevant = numbered(&owner, "p", 0, 2);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut counters = Counters::default().with_cancellation(Some(&cancellation));
    assert!(changed_group(&checker, &[irrelevant], &mut counters).is_err());
    assert_eq!(counters.accounting.work, 0);
}

#[test]
fn a_failed_wakeup_pass_discards_completed_evidence() {
    let owner = admit(CONSTRUCTOR);
    let mut checker = completed(&owner);
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.hold(numbered(&owner, "h", 1, 2)));
    checker.limits.max_work = 1;
    let before = checker.statistics().work;
    let error = checker
        .consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::First,
        )
        .unwrap_err();
    assert!(matches!(
        error.cause,
        crate::ConstraintCheckCause::Source(_)
    ));
    assert_eq!(error.statistics.work, before + 1);
    assert!(!state(&checker).valid);
    checker.limits.max_work = ConstraintCheckLimits::default().max_work;
    // This test changes private limits; renew their scalar-planning budget too.
    // Public checkers keep the limits chosen at construction.
    checker.settle_check();
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(state(&checker).valid && state(&checker).scans[0] == Scan::Clean);
}
