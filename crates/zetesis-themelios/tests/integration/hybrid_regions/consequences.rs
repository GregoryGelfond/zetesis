use super::*;
use zetesis_themelios::{
    ConstraintChecker, ConstraintConsequence, ConstraintRegionPass, FormulaFailure, FormulaResource,
};

fn apply(region: &mut Region, consequence: ConstraintConsequence) -> usize {
    match consequence {
        ConstraintConsequence::Hold { atom, .. } => {
            assert!(region.is_open(atom));
            assert!(region.hold(atom));
            atom
        }
        ConstraintConsequence::Cut { atom, .. } => {
            assert!(region.is_open(atom));
            assert!(region.cut(atom));
            atom
        }
        other => panic!("expected an open-atom consequence, got {other:?}"),
    }
}

fn first(
    owner: &HybridFormula,
    checker: &mut ConstraintChecker<'_>,
    region: &Region,
) -> ConstraintConsequence {
    checker
        .consequence_region(
            owner.core_theory(),
            region,
            &Cancellation::default(),
            ConstraintRegionPass::First,
        )
        .unwrap()
}

#[test]
fn consequences_preserve_every_satisfying_completion() {
    let mut forced = 0;
    let mut refuted = 0;
    for source in [
        include_str!("../../fixtures/streamed-consequences/two-positive.lp"),
        include_str!("../../fixtures/streamed-consequences/positive-negative.lp"),
        include_str!("../../fixtures/streamed-consequences/double-negative-negative.lp"),
        include_str!("../../fixtures/streamed-consequences/two-negative.lp"),
        include_str!("../../fixtures/streamed-consequences/negative-unit.lp"),
        include_str!("../../fixtures/streamed-consequences/strong-negative-pivot.lp"),
        include_str!("../../fixtures/streamed-consequences/nested-guarded-negative.lp"),
        include_str!("../../fixtures/streamed-consequences/three-row-ordered-pair.lp"),
        include_str!("../../fixtures/streamed-consequences/partial-division.lp"),
        include_str!("../../fixtures/streamed-consequences/empty-constraint.lp"),
    ] {
        let owner = admit(source);
        let count = owner.atom_catalog().atoms().len();
        assert!(count <= 5, "finite exhaustive fixture");
        let mut regions = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let mut models = owner.checker(ConstraintCheckLimits::default()).unwrap();
        for code in 0..3_usize.pow(u32::try_from(count).unwrap()) {
            let region = ternary_region(count, code);
            let result = first(&owner, &mut regions, &region);
            let decision = match result {
                ConstraintConsequence::NoConsequence => continue,
                ConstraintConsequence::Refuted { .. } => {
                    refuted += 1;
                    None
                }
                ConstraintConsequence::Hold { atom, .. } => {
                    forced += 1;
                    Some((atom, true))
                }
                ConstraintConsequence::Cut { atom, .. } => {
                    forced += 1;
                    Some((atom, false))
                }
            };
            if let Some((atom, _)) = decision {
                assert!(region.is_open(atom));
            }
            for mask in 0..1_usize << count {
                if (0..count).any(|atom| {
                    region
                        .decision(atom)
                        .is_some_and(|held| held != (mask & (1 << atom) != 0))
                }) {
                    continue;
                }
                if models
                    .check(&completion(&owner, mask), &Cancellation::default())
                    .unwrap()
                    == ConstraintVerdict::Satisfied
                {
                    let (atom, held) = decision.unwrap_or_else(|| {
                        panic!(
                            "refuted a satisfying completion: {source}, region {code}, mask {mask}"
                        )
                    });
                    assert_eq!(
                        mask & (1 << atom) != 0,
                        held,
                        "{source}, region {code}, mask {mask}"
                    );
                }
            }
        }
    }
    assert!(forced > 0);
    assert!(refuted > 0);
}

#[test]
fn same_predicate_pivots_follow_original_occurrences() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/two-row-ordered-pair.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    for held in 0..2 {
        let mut region = Region::all_open(2);
        assert!(region.hold(held));
        assert!(
            matches!(first(&owner, &mut checker, &region), ConstraintConsequence::Cut { atom, .. } if atom == 1 - held)
        );
    }
}

#[test]
fn multiple_open_occurrences_may_decline() {
    for source in [
        include_str!("../../fixtures/streamed-consequences/opposite-alias.lp"),
        include_str!("../../fixtures/streamed-consequences/one-row-alias.lp"),
        include_str!("../../fixtures/streamed-consequences/two-positive.lp"),
    ] {
        let owner = admit(source);
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let region = Region::all_open(owner.atom_catalog().atoms().len());
        assert_eq!(
            first(&owner, &mut checker, &region),
            ConstraintConsequence::NoConsequence,
            "{source}"
        );
    }
}

#[test]
fn normalized_duplicate_literals_still_force() {
    // The source body's canonical set coalesces the identical occurrences.
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/normalized-duplicate.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert!(matches!(
        first(&owner, &mut checker, &Region::all_open(1)),
        ConstraintConsequence::Cut { atom: 0, .. }
    ));
}

#[test]
fn no_consequence_does_not_certify_model_satisfaction() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/two-positive.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert_eq!(
        first(&owner, &mut checker, &Region::all_open(2)),
        ConstraintConsequence::NoConsequence
    );
    assert!(matches!(
        checker
            .check(&completion(&owner, 3), &Cancellation::default())
            .unwrap(),
        ConstraintVerdict::Violated { .. }
    ));
}

#[test]
fn continuation_shares_the_substitution_allowance() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/two-negative-units.lp"
    ));
    let cancellation = Cancellation::default();
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_substitutions: 1,
        ..Default::default()
    });
    let mut checker = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    let mut region = Region::all_open(2);
    let consequence = first(&owner, &mut checker, &region);
    apply(&mut region, consequence);
    let before = checker.statistics();
    assert_eq!(before.substitutions, 1);
    let failure = checker
        .consequence_region(
            owner.core_theory(),
            &region,
            &cancellation,
            ConstraintRegionPass::Continue,
        )
        .unwrap_err();
    assert!(
        matches!(failure.cause, ConstraintCheckCause::Source(ref error) if matches!(error.as_ref(), FormulaFailure::Limit { resource: FormulaResource::Substitutions, limit: 1, observed: 2, .. }))
    );
    assert_eq!(failure.statistics.substitutions, 1);
    assert_eq!(allowance.statistics(), failure.statistics);
    assert!(matches!(
        checker
            .consequence_region(
                owner.core_theory(),
                &region,
                &cancellation,
                ConstraintRegionPass::Continue
            )
            .unwrap_err()
            .cause,
        ConstraintCheckCause::NoActiveRegion
    ));
    let consequence = first(&owner, &mut checker, &region);
    apply(&mut region, consequence);
    assert_eq!(checker.statistics().substitutions, 2);
    assert_eq!(allowance.statistics(), checker.statistics());
}

#[test]
fn continuation_shares_the_scalar_allowance() {
    // Separate rules require a new traversal after the first deduction;
    // draining a prepared batch correctly needs no new scalar allocation.
    let source = include_str!("../../fixtures/streamed-consequences/separate-scalar-rules.lp");
    let owner = admit(source);
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    first(&owner, &mut baseline, &Region::all_open(2));
    let scalar = baseline.statistics().scalar_bytes;
    assert!(scalar > 0, "structural capture consumes the scalar budget");
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_scalar_bytes: scalar,
            ..Default::default()
        })
        .unwrap();
    let mut region = Region::all_open(2);
    let consequence = first(&owner, &mut checker, &region);
    apply(&mut region, consequence);
    let failure = checker
        .consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::Continue,
        )
        .unwrap_err();
    assert!(
        matches!(failure.cause, ConstraintCheckCause::Source(ref error) if matches!(error.as_ref(), FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Limit { resource: zetesis_themelios::ExpansionResource::ScalarBytes, .. })))
    );
    assert_eq!(failure.statistics.scalar_bytes, scalar);
}

#[test]
fn continuation_requires_an_unfinished_closure() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/two-positive.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(2);
    let before = checker.statistics();
    assert!(matches!(
        checker
            .consequence_region(
                owner.core_theory(),
                &region,
                &Cancellation::default(),
                ConstraintRegionPass::Continue
            )
            .unwrap_err()
            .cause,
        ConstraintCheckCause::NoActiveRegion
    ));
    assert_eq!(checker.statistics(), before);
    assert_eq!(
        first(&owner, &mut checker, &region),
        ConstraintConsequence::NoConsequence
    );
    assert!(matches!(
        checker
            .consequence_region(
                owner.core_theory(),
                &region,
                &Cancellation::default(),
                ConstraintRegionPass::Continue
            )
            .unwrap_err()
            .cause,
        ConstraintCheckCause::NoActiveRegion
    ));
}

#[test]
fn ordinary_checks_retire_consequence_closures() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/negative-unit.lp"
    ));
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_substitutions: 1,
            ..Default::default()
        })
        .unwrap();
    let region = Region::all_open(1);
    for model_check in [false, true] {
        assert!(matches!(
            first(&owner, &mut checker, &region),
            ConstraintConsequence::Hold { .. }
        ));
        if model_check {
            assert!(matches!(
                checker
                    .check(&completion(&owner, 0), &Cancellation::default())
                    .unwrap(),
                ConstraintVerdict::Violated { .. }
            ));
        } else {
            assert_eq!(
                checker
                    .check_region(owner.core_theory(), &region, &Cancellation::default())
                    .unwrap(),
                ConstraintRegionVerdict::NotRefuted
            );
        }
        assert!(matches!(
            checker
                .consequence_region(
                    owner.core_theory(),
                    &region,
                    &Cancellation::default(),
                    ConstraintRegionPass::Continue
                )
                .unwrap_err()
                .cause,
            ConstraintCheckCause::NoActiveRegion
        ));
    }
}

#[test]
fn consequence_authentication_precedes_source_work() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/positive-unit.lp"
    ));
    let other = admit(include_str!(
        "../../fixtures/streamed-consequences/positive-unit.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let before = checker.statistics();
    let failure = checker
        .consequence_region(
            other.core_theory(),
            &Region::all_open(1),
            &Cancellation::default(),
            ConstraintRegionPass::First,
        )
        .unwrap_err();
    assert!(matches!(failure.cause, ConstraintCheckCause::WrongProgram));
    assert_eq!(failure.statistics, before);
    let failure = checker
        .consequence_region(
            owner.core_theory(),
            &Region::all_open(2),
            &Cancellation::default(),
            ConstraintRegionPass::First,
        )
        .unwrap_err();
    assert!(matches!(
        failure.cause,
        ConstraintCheckCause::WrongRegionSize {
            expected: 1,
            actual: 2
        }
    ));
    assert_eq!(failure.statistics, before);
}

#[test]
fn cancelled_consequences_publish_no_decision() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/two-negative-units.lp"
    ));
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut checker = owner
        .checker_with_allowance(&allowance, &Cancellation::default())
        .unwrap();
    let mut region = Region::all_open(2);
    let consequence = first(&owner, &mut checker, &region);
    apply(&mut region, consequence);
    let before = checker.statistics();
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
    assert_eq!(failure.stop(), Some(Stop::Cancelled));
    assert_eq!(failure.statistics, before);
    assert_eq!(allowance.statistics(), before);
    assert_eq!(region.open().count(), 1);
}

#[test]
fn consequence_passes_share_every_work_cutoff() {
    let source = include_str!("../../fixtures/streamed-consequences/two-negative-units.lp");
    let owner = admit(source);
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let preparation = baseline.statistics().work;
    let mut region = Region::all_open(2);
    let consequence = first(&owner, &mut baseline, &region);
    apply(&mut region, consequence);
    let first_work = baseline.statistics().work;
    let consequence = baseline
        .consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::Continue,
        )
        .unwrap();
    assert!(matches!(consequence, ConstraintConsequence::Hold { .. }));
    let complete_work = baseline.statistics().work;
    assert!(preparation > 0 && first_work > preparation && complete_work > first_work);
    // Each owner is fresh so lazy index preparation is charged in the same
    // closure. The limit just below first_work detects dropped setup charges.
    for maximum in first_work - 1..=complete_work {
        let owner = admit(source);
        let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
            max_work: maximum,
            ..Default::default()
        });
        let mut checker = owner
            .checker_with_allowance(&allowance, &Cancellation::default())
            .unwrap();
        let mut region = Region::all_open(2);
        let initial = checker.consequence_region(
            owner.core_theory(),
            &region,
            &Cancellation::default(),
            ConstraintRegionPass::First,
        );
        let result = if maximum < first_work {
            initial
        } else {
            apply(&mut region, initial.unwrap());
            checker.consequence_region(
                owner.core_theory(),
                &region,
                &Cancellation::default(),
                ConstraintRegionPass::Continue,
            )
        };
        if maximum == complete_work {
            assert!(matches!(
                result.unwrap(),
                ConstraintConsequence::Hold { .. }
            ));
        } else {
            let failure = result.unwrap_err();
            assert!(
                matches!(failure.cause, ConstraintCheckCause::Source(ref error) if matches!(error.as_ref(), FormulaFailure::Limit { resource: FormulaResource::Work, limit, observed, .. } if *limit == u128::from(maximum) && *observed > *limit))
            );
            assert!(failure.statistics.work <= maximum);
        }
        assert_eq!(allowance.statistics(), checker.statistics());
    }
}

#[test]
fn narrowed_pivots_use_the_complete_scalar_filter() {
    let owner = admit(include_str!(
        "../../fixtures/streamed-consequences/reordered-positive.lp"
    ));
    let q = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == "q")
        .unwrap();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.hold(q));
    let ConstraintConsequence::Cut { atom, .. } = first(&owner, &mut checker, &region) else {
        panic!("one p row matches held q(1)");
    };
    let atom = owner.atom_catalog().atoms().at(atom).unwrap();
    assert_eq!(atom.predicate().name(), "p");
    assert_eq!(atom.values().at(0).unwrap(), zetesis_core::Value::Number(1));
}
