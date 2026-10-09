//! A completed negative scan covers unchanged witnesses; the occurrence union
//! supplies every new witness without changing the conservative unit operation.

use super::*;
use crate::GroundingPhase;
use crate::formula_support::{Accounting, testing::budget};
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    refuted: bool,
    units: BTreeSet<(usize, bool)>,
}

fn scan(owner: &HybridFormula, region: &Region, delta: Option<usize>) -> (Outcome, u64) {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(Accounting::default(), profile.work());
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    assert!(eligible(prepared, &mut counters).unwrap());
    prepared.incremental_eligible = Some(true);
    assert_eq!(prepared.source.rules.len(), 1);
    let mut outcome = Outcome {
        refuted: false,
        units: BTreeSet::new(),
    };
    profile
        .phase(GroundingPhase::RuleInstantiation, None, || {
            let result = scan_selected_rule(
                prepared,
                &mut budget(),
                &mut counters,
                region,
                0,
                delta,
                |value, _| {
                    let (atom, held) = match value {
                        ConstraintConsequence::Hold { atom, .. } => (atom, true),
                        ConstraintConsequence::Cut { atom, .. } => (atom, false),
                        ConstraintConsequence::Refuted { .. } => return Ok(true),
                        ConstraintConsequence::NoConsequence => unreachable!(),
                    };
                    outcome.refuted |= outcome.units.contains(&(atom, !held));
                    outcome.units.insert((atom, held));
                    Ok(outcome.refuted)
                },
            )
            .unwrap();
            outcome.refuted |= matches!(result, ConstraintConsequence::Refuted { .. });
            Ok::<_, FormulaFailure>(())
        })
        .unwrap();
    if outcome.refuted {
        outcome.units.clear();
    }
    (outcome, observer.0.get().join_rows.unwrap_or(0))
}

fn synchronized(checker: &mut crate::ConstraintChecker<'_>, region: &Region) {
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    let mut incremental = prepared.incremental.take().unwrap();
    incremental
        .synchronize(prepared, region, &mut counters)
        .unwrap();
    prepared.incremental = Some(incremental);
}

#[test]
fn singleton_unions_match_complete_region_scans() {
    for source in [
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/two-positive-negative.lp"
        ),
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/two-positive-double-negative.lp"
        ),
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/same-predicate-distinct.lp"
        ),
        include_str!("../../../../../tests/fixtures/streamed-consequences/same-predicate-alias.lp"),
        include_str!("../../../../../tests/fixtures/streamed-consequences/nested-positive-join.lp"),
        include_str!("../../../../../tests/fixtures/streamed-consequences/delta/alias-unit.lp"),
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/delta/repeated-pivots.lp"
        ),
    ] {
        let owner = admit(source);
        let atoms = owner.atom_catalog().atoms().len();
        assert!(atoms <= 4);
        let mut eligible_changes = 0;
        for encoding in 0..3_usize.pow(u32::try_from(atoms).unwrap()) {
            let mut old = Region::all_open(atoms);
            let mut digits = encoding;
            for atom in 0..atoms {
                match digits % 3 {
                    1 => {
                        assert!(old.hold(atom));
                    }
                    2 => {
                        assert!(old.cut(atom));
                    }
                    _ => {}
                }
                digits /= 3;
            }
            let (prior, _) = scan(&owner, &old, None);
            if prior.refuted || !prior.units.is_empty() {
                continue;
            }
            for changed in old.open() {
                let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
                assert_eq!(
                    pass(&mut checker, &old, ConstraintRegionPass::First),
                    ConstraintConsequence::NoConsequence
                );
                let mut next = old.clone();
                assert!(next.hold(changed));
                synchronized(&mut checker, &next);
                if state(&checker).scans[0] != Scan::PositiveDelta(changed) {
                    continue;
                }
                eligible_changes += 1;
                let (full, _) = scan(&owner, &next, None);
                let (anchored, _) = scan(&owner, &next, Some(changed));
                assert_eq!(
                    anchored, full,
                    "{source}: region {encoding}, atom {changed}"
                );
            }
        }
        assert!(
            eligible_changes > 0,
            "fixture must exercise the delta route: {source}"
        );
    }
}

#[test]
fn an_anchor_reduces_actual_source_visits() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/delta/late-anchor.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    let changed = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == "p"
                && atom.values().get(0).unwrap().descriptor()
                    == zetesis_core::ValueNodeRef::Number(16)
        })
        .unwrap();
    assert!(region.hold(changed));
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::PositiveDelta(changed)]);
    let (full, full_rows) = scan(&owner, &region, None);
    let (anchored, anchor_rows) = scan(&owner, &region, Some(changed));
    assert_eq!(anchored, full);
    assert_eq!(anchored.units, BTreeSet::from([(atom(&owner, "q"), false)]));
    assert!(anchor_rows < full_rows, "{anchor_rows} versus {full_rows}");
}

#[test]
fn deferred_deltas_survive_draining_other_units() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/delta/deferred.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    let r = atom(&owner, "r");
    assert!(region.hold(atom(&owner, "q")));
    assert!(region.hold(r));
    let first = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert_eq!(state(&checker).scans[1], Scan::PositiveDelta(r));
    let second = pass(&mut checker, &region, ConstraintRegionPass::Continue);
    assert_eq!(state(&checker).scans[1], Scan::PositiveDelta(r));
    for unit in [first, second] {
        let ConstraintConsequence::Cut { atom, .. } = unit else {
            panic!("expected queued cut")
        };
        assert!(region.cut(atom));
    }
    assert!(
        matches!(pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ConstraintConsequence::Cut { atom: cut, .. } if cut == atom(&owner, "s"))
    );
    assert_eq!(
        state(&checker).scans[1],
        Scan::Full,
        "productive scans cannot retain a negative premise"
    );
}

#[test]
fn deferred_distinct_changes_require_a_full_scan() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/delta/deferred.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    let r = atom(&owner, "r");
    assert!(region.hold(atom(&owner, "q")));
    assert!(region.hold(r));
    let first = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert_eq!(state(&checker).scans[1], Scan::PositiveDelta(r));
    // The next pass drains an earlier rule's pending unit, but it must first
    // accumulate this second relevant change into the deferred rule's state.
    assert!(region.hold(atom(&owner, "s")));
    let second = pass(&mut checker, &region, ConstraintRegionPass::Continue);
    assert_eq!(state(&checker).scans[1], Scan::Full);
    for unit in [first, second] {
        let ConstraintConsequence::Cut { atom, .. } = unit else {
            panic!("expected queued cut")
        };
        assert!(region.cut(atom));
    }
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ConstraintConsequence::Refuted { .. }
    ));
}

#[test]
fn affected_nonpositive_occurrences_decline_anchors() {
    for source in [
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/delta/positive-negative-alias.lp"
        ),
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/delta/positive-double-alias.lp"
        ),
    ] {
        let owner = admit(source);
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        assert_eq!(
            pass(&mut checker, &region, ConstraintRegionPass::First),
            ConstraintConsequence::NoConsequence
        );
        assert!(region.hold(atom(&owner, "p")));
        synchronized(&mut checker, &region);
        assert_eq!(state(&checker).scans, [Scan::Full]);
    }
}

#[test]
fn positive_cuts_decline_anchors() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/irrelevant-choice.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(region.cut(atom(&owner, "p")));
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::Full]);
}

#[test]
fn generated_rules_decline_anchors() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/generated-negative.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert!(
        checker.prepared.as_ref().unwrap().source.rules[0]
            .body
            .iter()
            .any(|literal| crate::formula_binding_cursor::target(literal).is_some())
    );
    let p = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == "p")
        .unwrap();
    assert!(region.hold(p));
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::Full]);
    assert_eq!(
        scan(&owner, &region, Some(p)).0,
        scan(&owner, &region, None).0
    );
}

#[test]
fn repeated_occurrences_retain_one_delta() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/delta/alias-unit.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    let p = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == "p")
        .unwrap();
    assert!(region.hold(p));
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::PositiveDelta(p)]);
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::PositiveDelta(p)]);
    assert!(
        matches!(pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { atom: cut, .. } if cut == atom(&owner, "q"))
    );
    assert_eq!(
        state(&checker).pending.len(),
        1,
        "overlapping anchors coalesce the same unit"
    );
}

#[test]
fn several_positive_atoms_require_a_full_scan() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/delta/repeated-pivots.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    for (position, atom) in owner.atom_catalog().atoms().iter().enumerate() {
        if atom.predicate().name() == "p" {
            assert!(region.hold(position));
        }
    }
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::Full]);
}

#[test]
fn interrupted_anchor_unions_retire_their_premise() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/delta/late-anchor.lp"
    ));
    let old = Region::all_open(owner.atom_catalog().atoms().len());
    let mut next = old.clone();
    let changed = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == "p"
                && atom.values().get(0).unwrap().descriptor()
                    == zetesis_core::ValueNodeRef::Number(16)
        })
        .unwrap();
    assert!(next.hold(changed));
    let warmed = || {
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        assert_eq!(
            pass(&mut checker, &old, ConstraintRegionPass::First),
            ConstraintConsequence::NoConsequence
        );
        checker
    };
    let mut baseline = warmed();
    let before = baseline.statistics().work;
    let expected = pass(&mut baseline, &next, ConstraintRegionPass::First);
    let work = baseline.statistics().work - before;
    assert!(work > 0);
    for limit in 0..work {
        let mut checker = warmed();
        checker.limits.max_work = limit;
        let before = checker.statistics().work;
        let error = checker
            .consequence_region(
                owner.core_theory(),
                &next,
                &Cancellation::default(),
                ConstraintRegionPass::First,
            )
            .unwrap_err();
        assert!(matches!(
            error.cause,
            crate::ConstraintCheckCause::Source(_)
        ));
        assert!(error.statistics.work - before <= limit);
        assert!(!state(&checker).valid);
        checker.limits.max_work = ConstraintCheckLimits::default().max_work;
        checker.settle_check();
        assert_eq!(
            pass(&mut checker, &next, ConstraintRegionPass::First),
            expected
        );
    }
}

#[test]
fn an_anchor_keeps_structural_pattern_matching() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/delta/structural-anchor.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    let other = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == "p"
                && matches!(
                    atom.values().get(0).unwrap().descriptor(),
                    zetesis_core::ValueNodeRef::Function { name: "g", .. }
                )
        })
        .unwrap();
    assert!(region.hold(other));
    synchronized(&mut checker, &region);
    assert_eq!(
        state(&checker).scans,
        [Scan::Clean],
        "the conservative classifier excludes g"
    );
    // Exercise the underlying anchor independently: bypassing a necessary
    // prepared posting must not turn p(g(1)) into a p(f(X)) match.
    let (anchored, _) = scan(&owner, &region, Some(other));
    assert_eq!(
        anchored,
        Outcome {
            refuted: false,
            units: BTreeSet::new()
        }
    );
}
