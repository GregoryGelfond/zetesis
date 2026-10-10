//! Exhaustion plus authenticated application discharges a productive scan.

use super::*;
use zetesis_core::ValueNodeRef;

const TWO: &str =
    include_str!("../../../../../tests/fixtures/streamed-consequences/two-row-units.lp");

fn apply(region: &mut Region, consequence: ConstraintConsequence) {
    assert!(match consequence {
        ConstraintConsequence::Hold { atom, .. } => region.hold(atom),
        ConstraintConsequence::Cut { atom, .. } => region.cut(atom),
        _ => panic!("expected a unit"),
    });
}

fn numbered(owner: &HybridFormula, predicate: &str, value: i32) -> usize {
    owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == predicate
                && atom.values().get(0).unwrap().descriptor() == ValueNodeRef::Number(value)
        })
        .unwrap()
}

#[test]
fn applied_batches_avoid_repeating_the_rule_scan() {
    let owner = admit(TWO);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let first = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert_eq!(state(&checker).completed_batch, Some(0));
    assert_eq!(state(&checker).pending.len(), 2);
    let substitutions = checker.statistics().substitutions;
    apply(&mut region, first);
    let second = pass(&mut checker, &region, ConstraintRegionPass::Continue);
    apply(&mut region, second);
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ConstraintConsequence::NoConsequence
    );
    assert_eq!(checker.statistics().substitutions, substitutions);
    assert_eq!(state(&checker).scans, [Scan::Clean]);
    assert!(state(&checker).completed_batch.is_none());
    let mut reference = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert_eq!(
        pass(&mut reference, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
}

#[test]
fn applied_negative_units_discharge_the_batch() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/negative-unit.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let unit = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert!(matches!(unit, ConstraintConsequence::Hold { .. }));
    assert_eq!(state(&checker).completed_batch, Some(0));
    apply(&mut region, unit);
    let prepared = checker.prepared.as_mut().unwrap();
    let mut incremental = prepared.incremental.take().unwrap();
    incremental
        .synchronize(prepared, &region, &mut Counters::default())
        .unwrap();
    assert_eq!(incremental.completed_batch, Some(0));
    assert_eq!(
        incremental
            .pop(&region, &prepared.limits, &mut Counters::default())
            .unwrap(),
        None
    );
    assert_eq!(incremental.scans, [Scan::Clean]);
}

#[test]
fn an_ignored_delivered_unit_is_rediscovered() {
    let owner = admit(TWO);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let ignored = pass(&mut checker, &region, ConstraintRegionPass::First);
    let second = pass(&mut checker, &region, ConstraintRegionPass::Continue);
    assert_ne!(ignored, second);
    apply(&mut region, second);
    let substitutions = checker.statistics().substitutions;
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ignored
    );
    assert!(checker.statistics().substitutions > substitutions);
    assert_eq!(state(&checker).scans, [Scan::Full]);
}

#[test]
fn a_contradicted_delivered_unit_refutes_the_region() {
    let owner = admit(TWO);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let ConstraintConsequence::Cut { atom, .. } =
        pass(&mut checker, &region, ConstraintRegionPass::First)
    else {
        panic!("expected cut")
    };
    let second = pass(&mut checker, &region, ConstraintRegionPass::Continue);
    apply(&mut region, second);
    assert!(region.hold(atom));
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ConstraintConsequence::Refuted { .. }
    ));
    assert!(!state(&checker).valid);
    assert!(state(&checker).completed_batch.is_none());
}

#[test]
fn enabling_changes_during_a_batch_produce_new_units() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/batches/independent-rows.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.hold(numbered(&owner, "p", 1)));
    let first = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert!(
        matches!(first, ConstraintConsequence::Cut { atom, .. } if atom == numbered(&owner, "q", 1))
    );
    apply(&mut region, first);
    assert!(region.hold(numbered(&owner, "p", 2)));
    let substitutions = checker.statistics().substitutions;
    assert!(
        matches!(pass(&mut checker, &region, ConstraintRegionPass::Continue), ConstraintConsequence::Cut { atom, .. } if atom == numbered(&owner, "q", 2))
    );
    assert!(checker.statistics().substitutions > substitutions);
}

#[test]
fn mixed_polarity_changes_produce_new_units() {
    for held in [false, true] {
        let owner = admit(include_str!(
            "../../../../../tests/fixtures/streamed-consequences/batches/mixed-polarities.lp"
        ));
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        for (position, atom) in owner.atom_catalog().atoms().iter().enumerate() {
            if atom.predicate().name() == "other" {
                assert!(region.hold(position));
            }
        }
        // Holding the first atom propagates forwards; cutting the last
        // propagates backwards. In either direction the middle decision
        // disables one occurrence but enables another in the same rule.
        let first = numbered(&owner, "p", if held { 1 } else { 3 });
        assert!(if held {
            region.hold(first)
        } else {
            region.cut(first)
        });
        let unit = pass(&mut checker, &region, ConstraintRegionPass::First);
        let second = numbered(&owner, "p", 2);
        assert!(match unit {
            ConstraintConsequence::Hold { atom, .. } => held && atom == second,
            ConstraintConsequence::Cut { atom, .. } => !held && atom == second,
            _ => false,
        });
        assert_eq!(state(&checker).completed_batch, Some(0));
        apply(&mut region, unit);
        let prepared = checker.prepared.as_mut().unwrap();
        let mut incremental = prepared.incremental.take().unwrap();
        incremental
            .synchronize(prepared, &region, &mut Counters::default())
            .unwrap();
        assert!(incremental.completed_batch.is_none());
        assert_eq!(incremental.scans, [Scan::Full]);
        prepared.incremental = Some(incremental);

        let next = pass(&mut checker, &region, ConstraintRegionPass::Continue);
        let third = numbered(&owner, "p", if held { 3 } else { 1 });
        assert!(match next {
            ConstraintConsequence::Hold { atom, .. } => held && atom == third,
            ConstraintConsequence::Cut { atom, .. } => !held && atom == third,
            _ => false,
        });
        let mut reference = owner.checker(ConstraintCheckLimits::default()).unwrap();
        assert_eq!(
            pass(&mut reference, &region, ConstraintRegionPass::First),
            next
        );
        apply(&mut region, next);
        assert_eq!(
            pass(&mut checker, &region, ConstraintRegionPass::Continue),
            ConstraintConsequence::NoConsequence
        );
    }
}

#[test]
fn applied_delta_batches_avoid_repeating_the_rule_scan() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/batches/independent-rows.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert_eq!(state(&checker).scans, [Scan::Clean]);
    let changed = numbered(&owner, "p", 1);
    assert!(region.hold(changed));
    let prepared = checker.prepared.as_mut().unwrap();
    let mut incremental = prepared.incremental.take().unwrap();
    incremental
        .synchronize(prepared, &region, &mut Counters::default())
        .unwrap();
    assert_eq!(
        incremental.scans,
        [Scan::PositiveDelta(ChangedAtoms::one(changed))]
    );
    prepared.incremental = Some(incremental);
    // The preceding NoConsequence closed its allowance. Start a new closure
    // while retaining the authenticated negative scan and its positive delta.
    let unit = pass(&mut checker, &region, ConstraintRegionPass::First);
    assert!(
        matches!(unit, ConstraintConsequence::Cut { atom, .. } if atom == numbered(&owner, "q", 1))
    );
    assert_eq!(state(&checker).completed_batch, Some(0));
    let substitutions = checker.statistics().substitutions;
    apply(&mut region, unit);
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::Continue),
        ConstraintConsequence::NoConsequence
    );
    assert_eq!(checker.statistics().substitutions, substitutions);
    assert_eq!(state(&checker).scans, [Scan::Clean]);
    let mut reference = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert_eq!(
        pass(&mut reference, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
}

#[test]
fn receipt_validation_refusal_keeps_the_full_scan() {
    let owner = admit(TWO);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
    for atom in 0..region.len() {
        assert!(region.cut(atom));
    }
    let prepared = checker.prepared.as_mut().unwrap();
    let mut incremental = prepared.incremental.take().unwrap();
    incremental
        .synchronize(prepared, &region, &mut Counters::default())
        .unwrap();
    assert_eq!(incremental.completed_batch, Some(0));
    assert_eq!(incremental.pending.len() - incremental.next, 1);
    let limits = FormulaLimits {
        max_work: 1,
        ..FormulaLimits::default()
    };
    let mut counters = Counters::default();
    assert!(matches!(
        incremental.pop(&region, &limits, &mut counters),
        Err(FormulaFailure::Limit {
            resource: crate::FormulaResource::Work,
            ..
        })
    ));
    assert_eq!(counters.accounting.work, 1);
    assert_eq!(incremental.scans, [Scan::Full]);
    assert!(incremental.completed_batch.is_none());
}
