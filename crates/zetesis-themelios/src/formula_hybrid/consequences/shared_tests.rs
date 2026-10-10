use super::*;
use crate::formula_support::testing::budget;
use crate::{
    AdmissionOptions, ConstraintCheckLimits, ExpansionLimits, FormulaLimits, HybridFormula,
};
use std::collections::BTreeSet;

fn owner(source: &str) -> HybridFormula {
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

#[derive(Debug)]
struct Scan {
    refuted: bool,
    units: BTreeSet<(usize, bool)>,
    work: u64,
    substitutions: u64,
    planned: usize,
}

/// Compare two consumers of the same admitted rule and immutable region.
fn run(owner: &HybridFormula, region: &Region, shared: bool) -> Scan {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    assert_eq!(prepared.source.rules.len(), 1);
    if shared {
        assert!(incremental::eligible(prepared, &mut budget(), &mut counters).unwrap());
    }
    prepared.incremental_eligible = Some(shared);
    let before = counters.accounting.work;
    let mut units = BTreeSet::new();
    let mut clash = false;
    let result = scan_rule(
        prepared,
        &mut budget(),
        &mut counters,
        region,
        0,
        |value, _| {
            let (atom, held) = match value {
                ConstraintConsequence::Hold { atom, .. } => (atom, true),
                ConstraintConsequence::Cut { atom, .. } => (atom, false),
                ConstraintConsequence::Refuted { .. } => return Ok(true),
                ConstraintConsequence::NoConsequence => unreachable!(),
            };
            clash |= units.contains(&(atom, !held));
            units.insert((atom, held));
            Ok(clash)
        },
    )
    .unwrap();
    let refuted = clash || matches!(result, ConstraintConsequence::Refuted { .. });
    if refuted {
        units.clear();
    }
    Scan {
        refuted,
        units,
        work: counters.accounting.work - before,
        substitutions: counters.accounting.substitutions,
        planned: prepared.plans.iter().filter(|plan| plan.is_some()).count(),
    }
}

#[test]
fn shared_queries_preserve_signed_region_consequences() {
    for source in [
        include_str!("../../../tests/fixtures/streamed-consequences/two-positive-negative.lp"),
        include_str!(
            "../../../tests/fixtures/streamed-consequences/two-positive-double-negative.lp"
        ),
        include_str!("../../../tests/fixtures/streamed-consequences/same-predicate-distinct.lp"),
        include_str!("../../../tests/fixtures/streamed-consequences/same-predicate-alias.lp"),
        include_str!("../../../tests/fixtures/streamed-consequences/nested-positive-join.lp"),
        include_str!("../../../tests/fixtures/streamed-consequences/negative-double-negative.lp"),
        include_str!("../../../tests/fixtures/streamed-consequences/guarded-negative.lp"),
    ] {
        let owner = owner(source);
        let atoms = owner.atom_catalog().atoms().len();
        assert!(atoms <= 4);
        for encoding in 0..3_usize.pow(u32::try_from(atoms).unwrap()) {
            let mut region = Region::all_open(atoms);
            let mut digits = encoding;
            for atom in 0..atoms {
                match digits % 3 {
                    1 => assert!(region.hold(atom)),
                    2 => assert!(region.cut(atom)),
                    _ => {}
                }
                digits /= 3;
            }
            let old = run(&owner, &region, false);
            let shared = run(&owner, &region, true);
            assert_eq!(
                (shared.refuted, &shared.units),
                (old.refuted, &old.units),
                "source {source}, region {encoding}"
            );
        }
    }
}

#[test]
fn shared_prefixes_reduce_charged_work() {
    let owner = owner(include_str!(
        "../../../tests/fixtures/streamed-consequences/independent-prefixes.lp"
    ));
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    for (atom, value) in owner.atom_catalog().atoms().iter().enumerate() {
        if value.arguments().get(0).unwrap().descriptor() == zetesis_core::ValueNodeRef::Number(1) {
            assert!(region.hold(atom));
        }
    }
    let old = run(&owner, &region, false);
    let shared = run(&owner, &region, true);
    assert!(!shared.refuted);
    assert_eq!(shared.units, old.units);
    assert_eq!(shared.units.len(), 2);
    assert_eq!(shared.substitutions, old.substitutions);
    assert!(shared.work < old.work, "{shared:?} versus {old:?}");
}

#[test]
fn two_forced_open_occurrences_skip_the_join() {
    let owner = owner(include_str!(
        "../../../tests/fixtures/streamed-consequences/two-positive.lp"
    ));
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let shared = run(&owner, &region, true);
    let old = run(&owner, &region, false);
    assert!(!shared.refuted);
    assert_eq!(shared.units, old.units);
    assert!(shared.units.is_empty());
    assert_eq!(shared.substitutions, 0);
    assert_eq!(shared.planned, 0);
}

#[test]
fn generated_rules_retain_mode_traversal() {
    let owner = owner(include_str!(
        "../../../tests/fixtures/streamed-consequences/generated-negative.lp"
    ));
    let checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert!(
        checker.prepared.as_ref().unwrap().source.rules[0]
            .body
            .iter()
            .any(|literal| crate::formula_binding_cursor::target(literal).is_some())
    );
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    for (atom, value) in owner.atom_catalog().atoms().iter().enumerate() {
        if value.predicate().name() == "p" {
            assert!(region.hold(atom));
        }
    }
    let shared = run(&owner, &region, true);
    let old = run(&owner, &region, false);
    assert_eq!((shared.refuted, &shared.units), (old.refuted, &old.units));
    assert_eq!(shared.substitutions, old.substitutions);
    assert_eq!(shared.units.len(), 2);
}
