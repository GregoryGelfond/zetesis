//! Original-region refutation preserves all unrefuted candidate completions.

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop, regions::Region};
use zetesis_themelios::{
    AdmissionOptions, ConstraintAllowance, ConstraintCheckCause, ConstraintCheckLimits,
    ConstraintCheckStatistics, ConstraintRegionVerdict, ConstraintVerdict, ExpansionLimits,
    FormulaLimits, HybridFormula, prepare_formula,
};

fn admit(source: &str) -> HybridFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn ternary_region(count: usize, mut code: usize) -> Region {
    let mut region = Region::all_open(count);
    for atom in 0..count {
        match code % 3 {
            0 => {}
            1 => assert!(region.hold(atom)),
            2 => assert!(region.cut(atom)),
            _ => unreachable!(),
        }
        code /= 3;
    }
    region
}

fn completion(owner: &HybridFormula, mask: usize) -> Model {
    Model::from_positions(
        owner.atom_catalog(),
        (0..owner.atom_catalog().atoms().len()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

#[test]
fn region_refutation_excludes_every_completion() {
    for source in [
        "{p;q}. :-p,not q.",
        "{p;q}. :-not not p,not q.",
        "{p(1);p(\"1\");-p(1)}. :-p(1),not p(\"1\"),not not -p(1).",
        "{p(f(1),f(1));p(f(1),f(2));-p(f(1),f(1))}. :-p(f(X),f(X)),not -p(f(X),f(X)).",
        "{p}. :-p,not missing.",
        "{p(1)}. :-p(1),not -p(1).",
        "d(0;1). {p(X)}:-d(X). :-p(X),1/X=1.",
        "d(1..2). {p(2..3)}. :-d(X),Y=X+1,p(Y),Y>2.",
        "{p(4,4);p(4,5);p(5,5)}.q(2). :-p(X,X),q(Y),X/2=Y.",
        "{p(1..3)}. X=2 :-p(X),X/2=1.",
        ":-.",
    ] {
        let owner = admit(source);
        let count = owner.atom_catalog().atoms().len();
        assert!(count <= 5, "finite exhaustive fixture");
        let mut regions = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let mut models = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let cancellation = Cancellation::default();
        let mut refuted = 0;
        for code in 0..3_usize.pow(u32::try_from(count).unwrap()) {
            let region = ternary_region(count, code);
            if regions
                .check_region(owner.core_theory(), &region, &cancellation)
                .unwrap()
                == ConstraintRegionVerdict::NotRefuted
            {
                continue;
            }
            refuted += 1;
            for mask in 0..1_usize << count {
                if (0..count).any(|atom| {
                    region
                        .decision(atom)
                        .is_some_and(|held| held != (mask & (1 << atom) != 0))
                }) {
                    continue;
                }
                assert!(
                    matches!(
                        models
                            .check(&completion(&owner, mask), &cancellation)
                            .unwrap(),
                        ConstraintVerdict::Violated { .. }
                    ),
                    "{source}: region {code}, completion {mask}"
                );
            }
        }
        assert!(refuted > 0, "{source}: exercised genuine refutation");
    }
}

#[test]
fn decided_regions_agree_with_full_model_checks() {
    let owner = admit("{p;q;r}. :-p,not q. :-not not q,r.");
    let count = owner.atom_catalog().atoms().len();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let cancellation = Cancellation::default();
    for mask in 0..1_usize << count {
        let mut region = Region::all_open(count);
        for atom in 0..count {
            assert!(if mask & (1 << atom) != 0 {
                region.hold(atom)
            } else {
                region.cut(atom)
            });
        }
        let region = checker
            .check_region(owner.core_theory(), &region, &cancellation)
            .unwrap();
        let model = checker
            .check(&completion(&owner, mask), &cancellation)
            .unwrap();
        assert_eq!(
            matches!(region, ConstraintRegionVerdict::Refuted { .. }),
            matches!(model, ConstraintVerdict::Violated { .. }),
            "mask {mask}"
        );
    }
}

#[test]
fn not_refuted_does_not_establish_any_satisfying_completion() {
    let owner = admit("{p}. :-p. :-not p.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let cancellation = Cancellation::default();
    assert_eq!(
        checker
            .check_region(owner.core_theory(), &Region::all_open(1), &cancellation)
            .unwrap(),
        ConstraintRegionVerdict::NotRefuted
    );
    for mask in 0..2 {
        assert!(matches!(
            checker
                .check(&completion(&owner, mask), &cancellation)
                .unwrap(),
            ConstraintVerdict::Violated { .. }
        ));
    }
}

#[test]
fn region_theory_identity_is_checked_before_decoding() {
    let owner = admit("{p}. :-p.");
    let other = admit("{p}. :-p.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let before = checker.statistics();
    let failure = checker
        .check_region(
            other.core_theory(),
            &Region::all_open(1),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert!(matches!(failure.cause, ConstraintCheckCause::WrongProgram));
    assert_eq!(failure.statistics, before);
}

#[test]
fn region_dimension_is_checked_before_decoding() {
    let owner = admit("{p}. :-p.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let before = checker.statistics();
    for actual in [0, 2] {
        let failure = checker
            .check_region(
                owner.core_theory(),
                &Region::all_open(actual),
                &Cancellation::default(),
            )
            .unwrap_err();
        assert!(matches!(failure.cause,
            ConstraintCheckCause::WrongRegionSize { expected: 1, actual: got } if got == actual
        ));
        assert_eq!(failure.statistics, before);
    }
}

#[test]
fn cancellation_precedes_region_index_preparation() {
    let owner = admit("{p}. :-p.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let before = checker.statistics();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let failure = checker
        .check_region(owner.core_theory(), &Region::all_open(1), &cancellation)
        .unwrap_err();
    assert_eq!(failure.stop(), Some(Stop::Cancelled));
    assert_eq!(failure.statistics, before);
}

#[test]
fn region_index_is_reused_between_checks() {
    let owner = admit("{p;q}. :-p,not q.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let before = checker.statistics().work;
    let region = Region::all_open(2);
    let cancellation = Cancellation::default();
    checker
        .check_region(owner.core_theory(), &region, &cancellation)
        .unwrap();
    let first = checker.statistics().work;
    checker
        .check_region(owner.core_theory(), &region, &cancellation)
        .unwrap();
    let second = checker.statistics().work;
    checker
        .check_region(owner.core_theory(), &region, &cancellation)
        .unwrap();
    let third = checker.statistics().work;
    assert!(first - before > second - first);
    assert_eq!(second - first, third - second);
}

#[test]
fn checkers_sharing_an_allowance_each_check_within_its_ceiling() {
    // A region check and a final check each make one substitution under a
    // one-substitution ceiling; the shared receipt holds both.
    let owner = admit("{p}. :-p.");
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_substitutions: 1,
        ..Default::default()
    });
    let cancellation = Cancellation::default();
    let mut region_checker = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    let mut final_checker = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    let mut held = Region::all_open(1);
    assert!(held.hold(0));
    assert!(matches!(
        region_checker
            .check_region(owner.core_theory(), &held, &cancellation)
            .unwrap(),
        ConstraintRegionVerdict::Refuted { .. }
    ));
    assert_eq!(
        final_checker
            .check(&completion(&owner, 0), &cancellation)
            .unwrap(),
        ConstraintVerdict::Satisfied
    );
    assert_eq!(allowance.statistics().substitutions, 2);
}

#[test]
fn unheld_rows_do_not_consume_substitutions() {
    let owner = admit("{p(1..8)}. :-p(X),X>0.");
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_substitutions: 1,
            ..Default::default()
        })
        .unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    let last = owner
        .atom_catalog()
        .atoms()
        .iter()
        .position(|atom| {
            atom.predicate().name() == "p"
                && atom
                    .values()
                    .iter()
                    .eq([zetesis_core::catalog::TermRef::from(
                        &zetesis_core::Value::Number(8),
                    )])
        })
        .unwrap();
    assert!(region.hold(last));
    assert!(matches!(
        checker
            .check_region(owner.core_theory(), &region, &Cancellation::default())
            .unwrap(),
        ConstraintRegionVerdict::Refuted { .. }
    ));
    // The broad predicate gate passes because p(8) is held. The row filter
    // excludes p(1)..p(7) before binding; only p(8) consumes a substitution.
    // Without that selection, p(1) spends the quota and p(2) exceeds it.
    assert_eq!(checker.statistics().substitutions, 1);
}

#[test]
fn held_negative_predicate_skips_join_substitutions() {
    let owner = admit("d(1..4). p(X,Y):-d(X),d(Y). {a}. :-p(X,Y),p(Y,Z),not p(X,Z).");
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_substitutions: 0,
            ..Default::default()
        })
        .unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    for (position, atom) in owner.atom_catalog().atoms().iter().enumerate() {
        if atom.predicate().name() == "p" {
            assert!(region.hold(position));
        }
    }
    assert_eq!(
        checker
            .check_region(owner.core_theory(), &region, &Cancellation::default())
            .unwrap(),
        ConstraintRegionVerdict::NotRefuted
    );
    assert_eq!(checker.statistics().substitutions, 0);
}

#[test]
fn shared_setup_honors_precancellation() {
    let owner = admit("{p}. :-p.");
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let failure = owner
        .checker_with_allowance(&allowance, &cancellation)
        .err()
        .unwrap();
    assert_eq!(failure.stop(), Some(Stop::Cancelled));
    assert_eq!(allowance.statistics(), ConstraintCheckStatistics::default());
}

#[test]
fn a_later_checker_prepares_under_its_own_ceiling() {
    let owner = admit("{p}. :-p.");
    let cancellation = Cancellation::default();
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    baseline
        .check(&completion(&owner, 0), &cancellation)
        .unwrap();
    let one = baseline.statistics();
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_work: one.work,
        ..Default::default()
    });
    for _ in 0..2 {
        let mut checker = owner
            .checker_with_allowance(&allowance, &cancellation)
            .unwrap();
        checker
            .check(&completion(&owner, 0), &cancellation)
            .unwrap();
        assert_eq!(checker.statistics(), one);
    }
    assert_eq!(allowance.statistics().work, 2 * one.work);
}

#[test]
fn independent_checkers_reuse_canonical_constructors() {
    let owner = admit("d(1..2). {p(f(1));p(f(2))}. :-d(X),Y=f(X),p(Y),X>1.");
    let cancellation = Cancellation::default();
    let candidate =
        Model::from_positions(owner.atom_catalog(), 0..owner.atom_catalog().atoms().len()).unwrap();
    // The frozen vocabulary already contains f(1) and f(2). Looking up those
    // constructors must not rebuild their payload in each independent checker.
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_scalar_bytes: 0,
        ..Default::default()
    });
    let mut first = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    let mut second = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    for checker in [&mut first, &mut second] {
        assert!(matches!(
            checker.check(&candidate, &cancellation).unwrap(),
            ConstraintVerdict::Violated { .. }
        ));
        assert!(
            checker.statistics().substitutions > 0,
            "constructor-dependent body was checked"
        );
        assert_eq!(checker.statistics().scalar_bytes, 0);
    }
    assert_eq!(allowance.statistics().scalar_bytes, 0);
}

#[test]
fn the_shared_receipt_sums_every_workers_charges() {
    let owner = admit("{p}. :-p.");
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_substitutions: 1,
        ..Default::default()
    });
    let receipts = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let owner = &owner;
                let allowance = &allowance;
                scope.spawn(move || {
                    let cancellation = Cancellation::default();
                    let mut checker = owner
                        .checker_with_allowance(allowance, &cancellation)
                        .unwrap();
                    for _ in 0..3 {
                        assert_eq!(
                            checker.check(&completion(owner, 0), &cancellation).unwrap(),
                            ConstraintVerdict::Satisfied
                        );
                    }
                    checker.statistics()
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(allowance.statistics().substitutions, 12);
    assert_eq!(
        allowance.statistics(),
        receipts
            .into_iter()
            .fold(ConstraintCheckStatistics::default(), |sum, receipt| {
                ConstraintCheckStatistics {
                    work: sum.work + receipt.work,
                    substitutions: sum.substitutions + receipt.substitutions,
                    scalar_bytes: sum.scalar_bytes + receipt.scalar_bytes,
                }
            },)
    );
}

mod consequences;
