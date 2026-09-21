//! Original-region refutation preserves all unrefuted candidate completions.

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop, regions::Region};
use zetesis_themelios::{
    AdmissionOptions, ConstraintAllowance, ConstraintCheckCause, ConstraintCheckLimits,
    ConstraintCheckStatistics, ConstraintRegionVerdict, ConstraintVerdict, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, HybridFormula, prepare_formula,
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
        "{p}. :-p,not missing.",
        "d(0;1). {p(X)}:-d(X). :-p(X),1/X=1.",
        "d(1..2). {p(2..3)}. :-d(X),Y=X+1,p(Y),Y>2.",
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
fn shared_substitution_limit_spans_all_checkers() {
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
    assert_eq!(
        region_checker
            .check_region(owner.core_theory(), &Region::all_open(1), &cancellation,)
            .unwrap(),
        ConstraintRegionVerdict::NotRefuted
    );
    let failure = final_checker
        .check(&completion(&owner, 0), &cancellation)
        .unwrap_err();
    assert!(matches!(failure.cause, ConstraintCheckCause::Source(error)
        if matches!(error.as_ref(), FormulaFailure::Limit {
            resource: FormulaResource::Substitutions, observed: 2, limit: 1, ..
        })
    ));
    assert_eq!(failure.statistics.substitutions, 0);
    assert_eq!(allowance.statistics().substitutions, 1);
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
fn shared_work_limit_covers_later_checker_preparation() {
    let owner = admit("{p}. :-p.");
    let cancellation = Cancellation::default();
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    baseline
        .check(&completion(&owner, 0), &cancellation)
        .unwrap();
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_work: baseline.statistics().work,
        ..Default::default()
    });
    let mut first = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    first.check(&completion(&owner, 0), &cancellation).unwrap();
    let before = allowance.statistics();
    let failure = owner
        .checker_with_allowance(&allowance, &cancellation)
        .err()
        .unwrap();
    assert!(matches!(failure.cause, ConstraintCheckCause::Source(error)
        if matches!(error.as_ref(), FormulaFailure::Limit {
            resource: FormulaResource::Work, observed, limit, ..
        } if *observed == u128::from(before.work) + 1 && *limit == u128::from(before.work))
    ));
    assert_eq!(failure.statistics, ConstraintCheckStatistics::default());
    assert_eq!(allowance.statistics(), before);
}

#[test]
fn shared_scalar_limit_covers_independent_checkers() {
    let owner = admit("d(1..2). {p(f(1));p(f(2))}. :-d(X),Y=f(X),p(Y),X>1.");
    let cancellation = Cancellation::default();
    let candidate = completion(&owner, 0);
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    baseline.check(&candidate, &cancellation).unwrap();
    let bytes = baseline.statistics().scalar_bytes;
    assert!(bytes > 0, "structured values require copied payload");
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_scalar_bytes: bytes,
        ..Default::default()
    });
    let mut first = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    let mut second = owner
        .checker_with_allowance(&allowance, &cancellation)
        .unwrap();
    first.check(&candidate, &cancellation).unwrap();
    let failure = second.check(&candidate, &cancellation).unwrap_err();
    assert!(matches!(failure.cause, ConstraintCheckCause::Source(error)
        if matches!(error.as_ref(), FormulaFailure::Expansion(
            zetesis_themelios::ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::ScalarBytes, ..
            }
        ))
    ));
    assert_eq!(failure.statistics.scalar_bytes, 0);
    assert_eq!(allowance.statistics().scalar_bytes, bytes);
}

#[test]
fn shared_worker_quota_is_not_multiplied_by_workers() {
    let owner = admit("{p}. :-p.");
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits {
        max_substitutions: 5,
        ..Default::default()
    });
    let receipts = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4).map(|_| {
            let owner = &owner;
            let allowance = &allowance;
            scope.spawn(move || {
                let cancellation = Cancellation::default();
                let mut checker = owner.checker_with_allowance(allowance, &cancellation).unwrap();
                loop {
                    match checker.check(&completion(owner, 0), &cancellation) {
                        Ok(verdict) => assert_eq!(verdict, ConstraintVerdict::Satisfied),
                        Err(failure) => {
                            assert!(matches!(failure.cause, ConstraintCheckCause::Source(error)
                                if matches!(error.as_ref(), FormulaFailure::Limit {
                                    resource: FormulaResource::Substitutions, observed: 6, limit: 5, ..
                                })
                            ));
                            return checker.statistics();
                        }
                    }
                }
            })
        }).collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(allowance.statistics().substitutions, 5);
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
