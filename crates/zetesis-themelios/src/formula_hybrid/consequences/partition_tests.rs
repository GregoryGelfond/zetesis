//! Compare disjoint anchor ownership with the former overlapping traversal.

use super::*;
use crate::formula_support::{Accounting, testing::budget};
use crate::grounding_observer::Profile;
use crate::test_support::{Observer, hybrid};
use crate::{ConstraintCheckLimits, GroundingPhase, HybridFormula};
use std::cell::Cell;
use std::collections::BTreeSet;

thread_local! {
    static PARTITION: Cell<bool> = const { Cell::new(true) };
}

pub(super) fn enabled() -> bool {
    PARTITION.get()
}

/// Disable only pre-binding anchor ownership. Indexed anchoring, truth tests,
/// scalar evaluation, rule plans and consequence collection remain unchanged.
pub(super) fn overlapping<T>(action: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            PARTITION.set(self.0);
        }
    }
    let _restore = Restore(PARTITION.replace(false));
    action()
}

#[derive(Debug)]
struct Scan {
    units: BTreeSet<(usize, bool)>,
    visits: usize,
    rows: u64,
}

fn scan(owner: &HybridFormula, region: &Region, delta: &[usize]) -> Scan {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(Accounting::default(), profile.work());
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    assert!(incremental::eligible(prepared, &mut budget(), &mut counters).unwrap());
    prepared.incremental_eligible = Some(true);
    assert_eq!(prepared.source.rules.len(), 1);
    assert_eq!(
        prepared.source.rules[0]
            .body
            .iter()
            .filter(|literal| matches!(literal_atom(literal), Some((DefaultNegation::None, _))))
            .count(),
        3,
        "two distinct p occurrences and the open q must survive normalization"
    );
    let mut units = BTreeSet::new();
    let mut visits = 0;
    profile
        .phase(GroundingPhase::RuleInstantiation, None, || {
            let result = scan_selected_rule(
                prepared,
                &mut budget(),
                &mut counters,
                region,
                0,
                Some(delta),
                |value, _| {
                    let (atom, held) = match value {
                        ConstraintConsequence::Hold { atom, .. } => (atom, true),
                        ConstraintConsequence::Cut { atom, .. } => (atom, false),
                        other => panic!("expected a unit witness, got {other:?}"),
                    };
                    visits += 1;
                    units.insert((atom, held));
                    Ok(false)
                },
            )
            .unwrap();
            assert_eq!(result, ConstraintConsequence::NoConsequence);
            Ok::<_, FormulaFailure>(())
        })
        .unwrap();
    Scan {
        units,
        visits,
        rows: observer.0.get().join_rows.unwrap(),
    }
}

fn held_inputs(owner: &HybridFormula) -> (Region, Vec<usize>, usize) {
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let mut changed = Vec::new();
    let mut q = None;
    for (position, atom) in owner.atom_catalog().atoms().iter().enumerate() {
        match atom.predicate().name() {
            "p" => {
                assert!(region.hold(position));
                changed.push(position);
            }
            "q" => q = Some(position),
            name => panic!("unexpected predicate {name}"),
        }
    }
    (region, changed, q.unwrap())
}

#[test]
fn repeated_occurrences_have_one_anchor_owner() {
    let owner = hybrid(include_str!(
        "../../../tests/fixtures/streamed-consequences/delta/partition-alias-unit.lp"
    ));
    let (region, changed, q) = held_inputs(&owner);
    assert_eq!(changed.len(), 1);
    let before = overlapping(|| scan(&owner, &region, &changed));
    let after = scan(&owner, &region, &changed);
    assert_eq!(after.units, BTreeSet::from([(q, false)]));
    assert_eq!(after.units, before.units);
    assert_eq!((before.visits, after.visits), (2, 1));
    assert!(after.rows < before.rows, "{after:?} versus {before:?}");
}

#[test]
fn several_changed_atoms_share_one_binding_owner() {
    let owner = hybrid(include_str!(
        "../../../tests/fixtures/streamed-consequences/delta/repeated-pivots.lp"
    ));
    let (region, changed, q) = held_inputs(&owner);
    assert_eq!(changed.len(), 2);
    let before = overlapping(|| scan(&owner, &region, &changed));
    let after = scan(&owner, &region, &changed);
    assert_eq!(after.units, BTreeSet::from([(q, false)]));
    assert_eq!(after.units, before.units);
    assert_eq!((before.visits, after.visits), (4, 2));
    assert!(after.rows < before.rows, "{after:?} versus {before:?}");
}

#[test]
fn anchor_order_preserves_structured_bindings() {
    let owner = hybrid(include_str!(
        "../../../tests/fixtures/streamed-consequences/delta/partition-structured.lp"
    ));
    let (region, mut changed, q) = held_inputs(&owner);
    assert_eq!(changed.len(), 2);
    for _ in 0..2 {
        let before = overlapping(|| scan(&owner, &region, &changed));
        let after = scan(&owner, &region, &changed);
        assert_eq!(after.units, BTreeSet::from([(q, false)]));
        assert_eq!(after.units, before.units);
        assert_eq!((before.visits, after.visits), (4, 2));
        assert!(after.rows < before.rows, "{after:?} versus {before:?}");
        changed.reverse();
    }
}
