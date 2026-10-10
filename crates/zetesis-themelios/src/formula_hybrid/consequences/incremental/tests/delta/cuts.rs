//! Positive cuts disable instances of a previously unproductive source rule.

use super::*;
use themelios_program::program::DefaultNegation;

const SIMPLE: &str =
    include_str!("../../../../../../tests/fixtures/streamed-consequences/irrelevant-choice.lp");
const ROWS: &str =
    include_str!("../../../../../../tests/fixtures/streamed-consequences/delta/late-anchor.lp");
const NEGATIVE_ALIAS: &str = include_str!(
    "../../../../../../tests/fixtures/streamed-consequences/delta/positive-negative-alias.lp"
);
const DOUBLE_ALIAS: &str = include_str!(
    "../../../../../../tests/fixtures/streamed-consequences/delta/positive-double-alias.lp"
);

fn completed(owner: &HybridFormula) -> crate::ConstraintChecker<'_> {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert_eq!(state(&checker).scans, [Scan::Clean]);
    checker
}

#[test]
fn positive_cuts_keep_completed_evidence() {
    let owner = admit(SIMPLE);
    let mut checker = completed(&owner);
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.cut(atom(&owner, "p")));
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::Clean]);
    let (full, _) = scan(&owner, &region, None);
    assert_eq!(
        full,
        Outcome {
            refuted: false,
            units: BTreeSet::new(),
        }
    );
    let substitutions = checker.statistics().substitutions;
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    assert_eq!(checker.statistics().substitutions, substitutions);
}

#[test]
fn positive_cuts_leave_held_anchors_available() {
    let owner = admit(ROWS);
    let mut checker = completed(&owner);
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let mut held = None;
    let mut cut = None;
    for (position, atom) in owner.atom_catalog().atoms().iter().enumerate() {
        if atom.predicate().name() == "p" {
            match atom.values().get(0).unwrap().descriptor() {
                zetesis_core::ValueNodeRef::Number(1) => {
                    assert!(region.cut(position));
                    cut = Some(position);
                }
                zetesis_core::ValueNodeRef::Number(16) => {
                    assert!(region.hold(position));
                    held = Some(position);
                }
                _ => {}
            }
        }
    }
    let held = held.unwrap();
    assert_ne!(cut.unwrap(), held);
    synchronized(&mut checker, &region);
    assert_eq!(
        state(&checker).scans,
        [Scan::PositiveDelta(ChangedAtoms::one(held))]
    );
    let (anchored, _) = scan(&owner, &region, Some(&[held]));
    assert_eq!(anchored, scan(&owner, &region, None).0);
    let q = atom(&owner, "q");
    assert_eq!(anchored.units, BTreeSet::from([(q, false)]));
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { atom, .. } if atom == q
    ));
}

#[test]
fn positive_cuts_preserve_later_default_read_effects() {
    for (source, expected) in [(NEGATIVE_ALIAS, Scan::Full), (DOUBLE_ALIAS, Scan::Clean)] {
        let owner = admit(source);
        let mut checker = completed(&owner);
        let prepared = checker.prepared.as_ref().unwrap();
        let body = &prepared.source.rules[0].body;
        let plan = state(&checker).plan;
        let affected = plan.atom_predicates[atom(&owner, "p")].unwrap();
        let dependencies = &plan.dependencies[plan.offsets[0]..plan.offsets[1]];
        // Ignore the unrelated q occurrence: the affected positive p must
        // precede the default p read in the actual prepared order.
        let mut affected_reads = body
            .iter()
            .enumerate()
            .filter_map(|(position, literal)| {
                literal_atom(literal).map(|(sign, _)| (position, sign))
            })
            .zip(dependencies)
            .filter_map(|((position, sign), &group)| {
                (group == affected).then_some((position, sign))
            });
        let positive_read = affected_reads
            .clone()
            .find_map(|(position, sign)| (sign == DefaultNegation::None).then_some(position))
            .unwrap();
        let default_read = affected_reads
            .find_map(|(position, sign)| {
                matches!(sign, DefaultNegation::Not | DefaultNegation::NotNot).then_some(position)
            })
            .unwrap();
        assert!(positive_read < default_read);
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        assert!(region.cut(atom(&owner, "p")));
        synchronized(&mut checker, &region);
        assert_eq!(state(&checker).scans, [expected]);
    }
}

#[test]
fn positive_cuts_do_not_reuse_generated_rules() {
    let owner = admit(include_str!(
        "../../../../../../tests/fixtures/streamed-consequences/generated-negative.lp"
    ));
    let mut checker = completed(&owner);
    assert!(
        checker.prepared.as_ref().unwrap().source.rules[0]
            .body
            .iter()
            .any(|literal| crate::formula_binding_cursor::target(literal).is_some())
    );
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let p = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == "p")
        .unwrap();
    assert!(region.cut(p));
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::Full]);
}

#[test]
fn untracked_positive_cuts_keep_the_complete_scan() {
    let owner = admit(ROWS);
    let mut checker = completed(&owner);
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let cuts: Vec<_> = owner
        .atom_catalog()
        .atoms()
        .iter()
        .enumerate()
        .filter(|(_, atom)| atom.predicate().name() == "p")
        .take(DELTA_ATOMS + 1)
        .map(|(position, _)| position)
        .collect();
    assert_eq!(cuts.len(), DELTA_ATOMS + 1);
    for cut in cuts {
        assert!(region.cut(cut));
    }
    synchronized(&mut checker, &region);
    assert_eq!(state(&checker).scans, [Scan::Full]);
}

fn region(atoms: usize, mut encoding: usize) -> Region {
    let mut region = Region::all_open(atoms);
    for atom in 0..atoms {
        match encoding % 3 {
            1 => assert!(region.hold(atom)),
            2 => assert!(region.cut(atom)),
            _ => {}
        }
        encoding /= 3;
    }
    region
}

#[test]
fn monotone_decisions_match_complete_region_scans() {
    let mut clean_changes = 0;
    let mut mixed_deltas = 0;
    for source in [
        SIMPLE,
        include_str!("../../../../../../tests/fixtures/streamed-consequences/two-negative.lp"),
        include_str!(
            "../../../../../../tests/fixtures/streamed-consequences/negative-double-negative.lp"
        ),
        NEGATIVE_ALIAS,
        DOUBLE_ALIAS,
        include_str!(
            "../../../../../../tests/fixtures/streamed-consequences/two-row-gated-units.lp"
        ),
        include_str!(
            "../../../../../../tests/fixtures/streamed-consequences/same-predicate-distinct.lp"
        ),
        include_str!(
            "../../../../../../tests/fixtures/streamed-consequences/nested-positive-join.lp"
        ),
    ] {
        let owner = admit(source);
        let atoms = owner.atom_catalog().atoms().len();
        assert!(atoms <= 4);
        let encodings = 3_usize.pow(u32::try_from(atoms).unwrap());
        for prior in 0..encodings {
            let old = region(atoms, prior);
            let (before, _) = scan(&owner, &old, None);
            if before.refuted || !before.units.is_empty() {
                continue;
            }
            for later in 0..encodings {
                let next = region(atoms, later);
                if !(0..atoms).all(|atom| {
                    old.decision(atom)
                        .is_none_or(|held| next.decision(atom) == Some(held))
                }) || !(0..atoms)
                    .any(|atom| old.decision(atom).is_none() && next.decision(atom).is_some())
                {
                    continue;
                }
                let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
                assert_eq!(
                    pass(&mut checker, &old, ConstraintRegionPass::First),
                    ConstraintConsequence::NoConsequence
                );
                synchronized(&mut checker, &next);
                let reused = match state(&checker).scans[0] {
                    Scan::Clean => {
                        clean_changes += 1;
                        Outcome {
                            refuted: false,
                            units: BTreeSet::new(),
                        }
                    }
                    Scan::PositiveDelta(changed) => {
                        mixed_deltas += 1;
                        assert!(
                            changed
                                .as_slice()
                                .iter()
                                .all(|&atom| next.decision(atom) == Some(true))
                        );
                        scan(&owner, &next, Some(changed.as_slice())).0
                    }
                    Scan::Full => continue,
                };
                assert_eq!(
                    reused,
                    scan(&owner, &next, None).0,
                    "{source}: region {prior}, strengthened to {later}"
                );
            }
        }
    }
    assert!(
        clean_changes > 0,
        "completed scans must survive disabling changes"
    );
    assert!(
        mixed_deltas > 0,
        "mixed held/cut changes must retain anchors"
    );
}
