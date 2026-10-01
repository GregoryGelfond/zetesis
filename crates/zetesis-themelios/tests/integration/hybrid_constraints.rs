//! Hybrid admission preserves original source validation and candidate identity.

use std::fmt::Write;

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{
    AdmissionOptions, ConstraintCheckCause, ConstraintCheckLimits, ConstraintCheckStatistics,
    ConstraintVerdict, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaLimits, FormulaResource, GroundingOptions, HybridFeature, HybridFormula, JoinStrategy,
    prepare_formula,
};

fn prepare(source: &str) -> zetesis_themelios::PreparedFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
fn admit(source: &str) -> HybridFormula {
    prepare(source).ground_hybrid().unwrap()
}
fn model(owner: &HybridFormula, selected: &[&str]) -> Model {
    // Parse fixture names through the existing source door, then select by
    // complete typed identity. This helper does not decide model membership.
    let mut facts = String::new();
    for atom in selected {
        write!(facts, "{atom}. ").unwrap();
    }
    let expected = prepare(&facts).ground().unwrap();
    let positions: Vec<_> = owner
        .atom_catalog()
        .atoms()
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            expected
                .atoms()
                .iter()
                .any(|other| atom == other)
                .then_some(index)
        })
        .collect();
    assert_eq!(
        positions.len(),
        selected.len(),
        "selected atoms exist in the original catalog"
    );
    Model::from_positions(owner.atom_catalog(), positions).unwrap()
}
fn verdict(owner: &HybridFormula, selected: &[&str]) -> ConstraintVerdict {
    owner
        .checker(ConstraintCheckLimits::default())
        .unwrap()
        .check(&model(owner, selected), &Cancellation::default())
        .unwrap()
}

#[test]
fn candidate_truth_is_distinct_from_possible_support() {
    let owner = admit("d(1..2). {p(X)}:-d(X). :-p(X),X=2.");
    assert!(owner.streamed_templates() > 0);
    assert_eq!(
        verdict(&owner, &["d(1)", "d(2)", "p(1)"]),
        ConstraintVerdict::Satisfied
    );
    assert!(matches!(
        verdict(&owner, &["d(1)", "d(2)", "p(2)"]),
        ConstraintVerdict::Violated { .. }
    ));
}

#[test]
fn default_negation_preserves_typed_signed_identity() {
    let owner = admit("{p(1);p(\"1\");-p(1)}. :-not p(1),not not p(\"1\"). :- -p(1),not p(1).");
    for selected in [vec!["p(\"1\")"], vec!["-p(1)"]] {
        assert!(matches!(
            verdict(&owner, &selected),
            ConstraintVerdict::Violated { .. }
        ));
    }
    for selected in [vec![], vec!["p(1)", "p(\"1\")"], vec!["p(1)", "-p(1)"]] {
        // Checker establishes only constraint satisfaction. The last model is
        // deliberately incoherent and must still be rejected by core membership.
        assert_eq!(verdict(&owner, &selected), ConstraintVerdict::Satisfied);
    }
}

#[test]
fn unsupported_atoms_retain_catalog_positions() {
    let owner = admit(":-not missing.");
    assert_eq!(owner.atom_catalog().atoms().len(), 1);
    assert_eq!(
        owner
            .atom_catalog()
            .atoms()
            .at(0)
            .unwrap()
            .predicate()
            .name(),
        "missing"
    );
    assert!(
        owner
            .atom_catalog()
            .atoms()
            .at(0)
            .unwrap()
            .values()
            .is_empty()
    );
    assert!(matches!(
        verdict(&owner, &[]),
        ConstraintVerdict::Violated { .. }
    ));
}

#[test]
fn mixed_undefined_instances_warn_before_checking() {
    let owner = admit("d(0;1). {p(X)}:-d(X). :-p(X),1/X=1.");
    assert_eq!(owner.warnings().len(), 1);
    assert!(owner.warning_view().to_string().contains("zero-divisor"));
    assert_eq!(
        verdict(&owner, &["d(0)", "d(1)", "p(0)"]),
        ConstraintVerdict::Satisfied
    );
    assert!(matches!(
        verdict(&owner, &["d(0)", "d(1)", "p(1)"]),
        ConstraintVerdict::Violated { .. }
    ));
}

#[test]
fn defined_false_instances_witness_arithmetic_admission() {
    let owner = admit("d(0;2). {p(X)}:-d(X). :-p(X),1/X=1.");
    assert_eq!(owner.warnings().len(), 1);
    assert_eq!(
        verdict(&owner, &["d(0)", "d(2)", "p(0)", "p(2)"]),
        ConstraintVerdict::Satisfied
    );
}

#[test]
fn candidate_gates_cannot_hide_fatal_source_arithmetic() {
    for source in [
        "d(0). {p(X)}:-d(X). :-p(X),1/X=1.",
        "d(1). {p(X)}:-d(X). :-p(X),X+2147483647=0.",
        "d(0). {p(X)}:-d(X). :-p(X),1/X-(2147483647+1)=0.",
    ] {
        // An interpretation can omit p, but original arithmetic validation
        // precedes every candidate and must refuse the source itself.
        let result = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .and_then(zetesis_themelios::PreparedFormula::ground_hybrid);
        assert!(
            matches!(
                result,
                Err(FormulaFailure::Expansion(
                    ExpansionFailure::Evaluation { .. }
                ))
            ),
            "{source}"
        );
    }
}

#[test]
fn independent_constraint_families_do_not_rescue_undefined_ones() {
    let result = prepare("d(0). {p(X)}:-d(X). :-p(X),1/X=1. :-p(X),X=1.").ground_hybrid();
    assert!(matches!(
        result,
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Evaluation { .. }
        ))
    ));
}

#[test]
fn generators_reuse_the_checked_join() {
    let owner = admit("d(1..2). {p(2..3)}. :-d(X),Y=X+1,p(Y),Y>2.");
    assert_eq!(
        verdict(&owner, &["d(1)", "d(2)", "p(2)"]),
        ConstraintVerdict::Satisfied
    );
    assert!(matches!(
        verdict(&owner, &["d(1)", "d(2)", "p(3)"]),
        ConstraintVerdict::Violated { .. }
    ));
}

#[test]
fn scoped_constraints_remain_eager() {
    let owner = admit("{p(1);p(2)}. :-#count{X:p(X)}>1. :-p(1),not p(2).");
    assert_eq!(owner.streamed_templates(), 1);
    assert!(owner.core_theory().roots().len() > 2);
}

#[test]
fn objective_programs_refuse_the_hybrid_schedule() {
    let result = prepare("{p}. #minimize{1:p}.").ground_hybrid();
    assert!(matches!(
        result,
        Err(FormulaFailure::HybridUnsupported {
            feature: HybridFeature::Objectives,
            ..
        })
    ));
}

#[test]
fn table_policy_is_not_silently_replaced() {
    let result = prepare("{p}. :-p.")
        .with_grounding_options(GroundingOptions {
            joins: JoinStrategy::Table,
        })
        .ground_hybrid();
    assert!(matches!(
        result,
        Err(FormulaFailure::HybridUnsupported {
            feature: HybridFeature::TableJoins,
            ..
        })
    ));
}

#[test]
fn empty_stream_requires_no_check_budget() {
    let owner = admit("a.");
    assert_eq!(owner.streamed_templates(), 0);
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_work: 0,
            max_substitutions: 0,
            max_scalar_bytes: 0,
        })
        .unwrap();
    assert_eq!(
        checker
            .check(&model(&owner, &["a"]), &Cancellation::default())
            .unwrap(),
        ConstraintVerdict::Satisfied
    );
    assert_eq!(checker.statistics(), ConstraintCheckStatistics::default());
}

#[test]
fn source_equivalence_does_not_replace_owner_identity() {
    let owner = admit("{p}. :-p.");
    let other = admit("{p}. :-p.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let failure = checker
        .check(&model(&other, &[]), &Cancellation::default())
        .unwrap_err();
    assert!(matches!(failure.cause, ConstraintCheckCause::WrongProgram));
    assert_eq!(
        checker
            .check(&model(&owner.clone(), &[]), &Cancellation::default())
            .unwrap(),
        ConstraintVerdict::Satisfied
    );
}

#[test]
fn cancellation_preserves_the_unchecked_verdict() {
    let owner = admit("{p}. :-p.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let before = checker.statistics();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let failure = checker
        .check(&model(&owner, &[]), &cancellation)
        .unwrap_err();
    assert_eq!(failure.stop(), Some(Stop::Cancelled));
    assert_eq!(failure.statistics, before);
}

#[test]
fn repeated_checks_share_a_cumulative_substitution_ceiling() {
    let owner = admit("{p}. :-p.");
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_substitutions: 1,
            ..Default::default()
        })
        .unwrap();
    let candidate = model(&owner, &[]);
    assert_eq!(
        checker.check(&candidate, &Cancellation::default()).unwrap(),
        ConstraintVerdict::Satisfied
    );
    let failure = checker
        .check(&candidate, &Cancellation::default())
        .unwrap_err();
    assert!(matches!(
        failure.cause,
        ConstraintCheckCause::Source(error) if matches!(error.as_ref(), FormulaFailure::Limit {
            resource: FormulaResource::Substitutions,
            observed: 2,
            limit: 1,
            ..
        })
    ));
    assert_eq!(failure.statistics.substitutions, 1);
}

#[test]
fn moving_a_checker_preserves_its_cumulative_budget() {
    let owner = admit("{p}. :-p.");
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_substitutions: 1,
            ..Default::default()
        })
        .unwrap();
    let candidate = model(&owner, &[]);
    assert_eq!(
        checker.check(&candidate, &Cancellation::default()).unwrap(),
        ConstraintVerdict::Satisfied
    );
    let before = checker.statistics();
    let (checker, failure) = std::thread::scope(|scope| {
        scope
            .spawn(move || {
                let failure = checker
                    .check(&candidate, &Cancellation::default())
                    .unwrap_err();
                (checker, failure)
            })
            .join()
            .unwrap()
    });
    assert!(
        matches!(failure.cause, ConstraintCheckCause::Source(ref error)
            if matches!(error.as_ref(), FormulaFailure::Limit {
                resource: FormulaResource::Substitutions, observed: 2, limit: 1, ..
            })
        )
    );
    assert_eq!(failure.statistics.substitutions, before.substitutions);
    assert!(failure.statistics.work > before.work);
    assert_eq!(checker.statistics(), failure.statistics);
}

#[test]
fn repeated_checks_share_a_cumulative_work_ceiling() {
    let owner = admit("{p}. :-p.");
    let candidate = model(&owner, &[]);
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    baseline
        .check(&candidate, &Cancellation::default())
        .unwrap();
    let complete = baseline.statistics();
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_work: complete.work,
            ..Default::default()
        })
        .unwrap();
    checker.check(&candidate, &Cancellation::default()).unwrap();
    assert_eq!(checker.statistics(), complete);
    for _ in 0..2 {
        let failure = checker
            .check(&candidate, &Cancellation::default())
            .unwrap_err();
        assert!(
            matches!(failure.cause, ConstraintCheckCause::Source(ref error)
                if matches!(error.as_ref(), FormulaFailure::Limit {
                    resource: FormulaResource::Work, ..
                })
            )
        );
        assert_eq!(failure.statistics.work, complete.work);
        assert_eq!(checker.statistics(), failure.statistics);
    }
}

#[test]
fn structural_checks_share_a_cumulative_scalar_ceiling() {
    // Structural captures reserve delta cells on each scan. Frozen constructor
    // lookups and binding ID copies do not consume this scalar-byte allowance.
    let owner = admit("{p(f(1));p(f(2))}. :-p(f(X)),X>1.");
    let candidate = model(&owner, &["p(f(1))"]);
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert_eq!(
        baseline
            .check(&candidate, &Cancellation::default())
            .unwrap(),
        ConstraintVerdict::Satisfied
    );
    let complete = baseline.statistics();
    assert!(complete.scalar_bytes > 0);
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_scalar_bytes: complete.scalar_bytes,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        checker.check(&candidate, &Cancellation::default()).unwrap(),
        ConstraintVerdict::Satisfied
    );
    assert_eq!(checker.statistics(), complete);
    let failure = checker
        .check(&candidate, &Cancellation::default())
        .unwrap_err();
    assert!(
        matches!(failure.cause, ConstraintCheckCause::Source(ref error)
            if matches!(error.as_ref(), FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::ScalarBytes, limit, observed, ..
            }) if *limit == complete.scalar_bytes as u128 && observed > limit)
        )
    );
    assert_eq!(failure.statistics.scalar_bytes, complete.scalar_bytes);
    assert_eq!(checker.statistics(), failure.statistics);
}

#[test]
fn excluded_rows_preserve_eager_source_admission() {
    for source in [
        "d(0..2). {p(X)}:-d(X). :-p(X),X!=0,1/X=1.",
        "d(0..2). {p(X)}:-d(X). :-p(X),1/X=1,X!=0.",
        "d(0). :-d(X),X!=0,1/X=1,not q(X).",
        ":-unknown(X),1/X=1.",
    ] {
        let eager = prepare(source).ground().unwrap();
        let hybrid = admit(source);
        assert!(eager.warnings().is_empty(), "{source}");
        assert_eq!(hybrid.warnings(), eager.warnings(), "{source}");
        assert_eq!(
            hybrid.atom_catalog().atoms(),
            eager.atom_catalog().atoms(),
            "{source}"
        );
    }
}

#[test]
fn streamed_satisfaction_composes_with_the_retained_core() {
    use std::collections::BTreeSet;
    use zetesis_ferraris::{Interpretation, Limits, models};

    for source in [
        "{p;q}. :-p,not q. :-not not q,not p.",
        "{p;-p}. :-not missing. :-p,-p.",
        "d(0;1). {p(X)}:-d(X). :-p(X),1/X=1.",
        "d(1..2). {p(2..3)}. :-d(X),Y=X+1,p(Y),Y>2.",
        "{p(1);p(2)}. :-#count{X:p(X)}>1. :-p(1),not p(2).",
        "{p(f(1));p(f(2))}. :-p(f(X)),X=2.",
        "{p(4,4);p(4,5);p(5,5)}.q(2). :-p(X,X),q(Y),X/2=Y.",
        "{p(1..3)}. X=2 :-p(X),X/2=1.",
    ] {
        let eager = prepare(source).ground().unwrap();
        let hybrid = admit(source);
        assert_eq!(
            eager.atoms().iter().collect::<BTreeSet<_>>(),
            hybrid.atom_catalog().atoms().iter().collect()
        );
        let count = hybrid.atom_catalog().atoms().len();
        assert!(count <= 8, "bounded complete interpretation population");
        let mut checker = hybrid.checker(ConstraintCheckLimits::default()).unwrap();
        let cancellation = Cancellation::default();
        for mask in 0..(1_usize << count) {
            let candidate = Model::from_positions(
                hybrid.atom_catalog(),
                (0..count).filter(|bit| mask & (1 << bit) != 0),
            )
            .unwrap();
            let full = Interpretation::new(
                eager.theory(),
                eager
                    .atoms()
                    .iter()
                    .enumerate()
                    .filter_map(|(id, atom)| candidate.contains(atom).then_some(id)),
            )
            .unwrap();
            let core =
                Interpretation::new(hybrid.core_theory(), candidate.positions().iter().copied())
                    .unwrap();
            let original = models(eager.theory(), &full, Limits::default(), &cancellation).unwrap();
            let retained = models(
                hybrid.core_theory(),
                &core,
                Limits::default(),
                &cancellation,
            )
            .unwrap();
            let streamed =
                checker.check(&candidate, &cancellation).unwrap() == ConstraintVerdict::Satisfied;
            assert_eq!(original, retained && streamed, "{source}: mask {mask}");
        }
    }
}

#[test]
fn frozen_checks_reuse_speculative_column_values() {
    // X=100 never completes p(X,keep), but totality visits the whole X
    // column and needs the canonical result 50 before checking is frozen.
    for source in [
        "p(1,keep).p(100,skip). :-p(X,keep),X/2=5.",
        // A nonnumeric overapproximation declines totality without becoming
        // a source fault; the frozen retry must preserve that same boundary.
        "p(2,keep).p(symbol,skip). :-p(X,keep),X/2=5.",
    ] {
        let owner = admit(source);
        assert_eq!(owner.streamed_templates(), 1);
        let all =
            Model::from_positions(owner.atom_catalog(), 0..owner.atom_catalog().atoms().len())
                .unwrap();
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        for _ in 0..2 {
            assert_eq!(
                checker.check(&all, &Cancellation::default()).unwrap(),
                ConstraintVerdict::Satisfied,
                "{source}"
            );
        }
    }
}

#[test]
fn computed_domains_select_only_matching_substitutions() {
    let owner = admit("{p(1..6)}.q(2). :-p(X),q(Y),X/2=Y.");
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    for count in 1..=2 {
        // Omitting all candidate atoms makes checking exhaust the streamed
        // family. Only X=4 and X=5 can satisfy its computed equality.
        assert_eq!(
            checker
                .check(&model(&owner, &[]), &Cancellation::default())
                .unwrap(),
            ConstraintVerdict::Satisfied
        );
        assert_eq!(checker.statistics().substitutions, 2 * count);
    }
}

#[test]
fn computed_selection_keeps_inclusive_work_limits() {
    let owner = admit("{p(1..6)}.q(2). :-p(X),q(Y),X/2=Y.");
    let candidate = model(&owner, &[]);
    let mut baseline = owner.checker(ConstraintCheckLimits::default()).unwrap();
    assert_eq!(
        baseline
            .check(&candidate, &Cancellation::default())
            .unwrap(),
        ConstraintVerdict::Satisfied
    );
    let complete = baseline.statistics();
    for max_work in [0, 1, complete.work - 1, complete.work] {
        let result = owner.checker(ConstraintCheckLimits {
            max_work,
            ..Default::default()
        });
        let result = result.and_then(|mut checker| {
            let result = checker.check(&candidate, &Cancellation::default());
            assert!(checker.statistics().work <= max_work);
            result
        });
        if max_work == complete.work {
            assert_eq!(result.unwrap(), ConstraintVerdict::Satisfied);
        } else {
            let failure = result.unwrap_err();
            assert!(failure.statistics.work <= max_work);
            assert!(
                matches!(failure.cause, ConstraintCheckCause::Source(ref error)
                if matches!(error.as_ref(), FormulaFailure::Limit {
                    resource: FormulaResource::Work, limit, ..
                } if *limit == u128::from(max_work)))
            );
        }
    }
}
