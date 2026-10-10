//! Completed scans depend on whether each signed occurrence can become true.

use super::*;

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
fn disabling_default_reads_keep_completed_evidence() {
    for (source, held) in [
        (
            include_str!("../../../../../../tests/fixtures/streamed-consequences/two-negative.lp"),
            true,
        ),
        (
            include_str!(
                "../../../../../../tests/fixtures/streamed-consequences/negative-double-negative.lp"
            ),
            false,
        ),
    ] {
        let owner = admit(source);
        let mut checker = completed(&owner);
        let mut region = Region::all_open(owner.atom_catalog().atoms().len());
        let q = atom(&owner, "q");
        assert!(if held { region.hold(q) } else { region.cut(q) });
        synchronized(&mut checker, &region);
        assert_eq!(state(&checker).scans, [Scan::Clean]);
        let substitutions = checker.statistics().substitutions;
        assert_eq!(
            pass(&mut checker, &region, ConstraintRegionPass::First),
            ConstraintConsequence::NoConsequence
        );
        assert_eq!(checker.statistics().substitutions, substitutions);
        assert_eq!(
            scan(&owner, &region, None).0,
            Outcome {
                refuted: false,
                units: BTreeSet::new(),
            }
        );
    }
}

#[test]
fn disabling_default_reads_preserve_positive_anchors() {
    let owner = admit(include_str!(
        "../../../../../../tests/fixtures/streamed-consequences/delta/positive-negative-alias.lp"
    ));
    let mut checker = completed(&owner);
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let p = atom(&owner, "p");
    assert!(region.hold(p));
    synchronized(&mut checker, &region);
    assert_eq!(
        state(&checker).scans,
        [Scan::PositiveDelta(ChangedAtoms::one(p))]
    );
    assert_eq!(
        scan(&owner, &region, Some(&[p])).0,
        scan(&owner, &region, None).0
    );
}
