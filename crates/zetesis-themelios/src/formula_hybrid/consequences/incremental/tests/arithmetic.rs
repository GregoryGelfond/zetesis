use super::*;

#[test]
fn total_arithmetic_enables_batched_consequences() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/arithmetic/sum-units.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    assert_eq!(
        checker.prepared.as_ref().unwrap().incremental_eligible,
        Some(true)
    );
    assert_eq!(state(&checker).pending.len(), 3);
    let units = state(&checker)
        .pending
        .iter()
        .map(|decision| {
            assert!(!decision.held);
            let atom = owner.atom_catalog().atoms().at(decision.atom).unwrap();
            assert_eq!(atom.predicate().name(), "p");
            assert_eq!(atom.predicate().arity(), 2);
            assert_eq!(atom.predicate().sign(), zetesis_core::Sign::Positive);
            [0, 1].map(|column| {
                let zetesis_core::ValueNodeRef::Number(value) =
                    atom.values().at(column).unwrap().descriptor()
                else {
                    panic!("a numeric consequence argument");
                };
                value
            })
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(units, [[1, 3], [2, 2], [3, 1]].into());
}

#[test]
fn one_partial_rule_keeps_partition_order() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/arithmetic/mixed-partition.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(matches!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::Cut { .. }
    ));
    let prepared = checker.prepared.as_ref().unwrap();
    assert_eq!(prepared.incremental_eligible, Some(false));
    assert!(prepared.incremental.is_none());
    assert!(
        prepared.plans[0]
            .as_ref()
            .unwrap()
            .has_totality_certificate()
    );
}

#[test]
fn correlated_overflow_declines_batching() {
    let owner = admit(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/arithmetic/correlated-difference.lp"
    ));
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    assert_eq!(
        pass(&mut checker, &region, ConstraintRegionPass::First),
        ConstraintConsequence::NoConsequence
    );
    let prepared = checker.prepared.as_ref().unwrap();
    assert_eq!(prepared.incremental_eligible, Some(false));
    assert!(prepared.incremental.is_none());
}

#[test]
fn queens_arithmetic_uses_incremental_scanning() {
    let corpus = zetesis_test_support::repository::correctness().join("standalone/n-queens");
    for name in ["variant-01.lp", "variant-02.lp", "variant-03.lp"] {
        let source = std::fs::read_to_string(corpus.join(name)).unwrap();
        let owner = admit(&source);
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let region = Region::all_open(owner.atom_catalog().atoms().len());
        let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
        let prepared = checker
            .prepared
            .as_ref()
            .expect("streamed original constraints");
        assert_eq!(prepared.incremental_eligible, Some(true), "{name}");
        assert!(prepared.incremental.is_some(), "{name}");
        // The claim concerns the written diagonal arithmetic, not merely a
        // constructor-only or empty partition that was already eligible.
        let arithmetic = prepared
            .plans
            .iter()
            .flatten()
            .filter(|plan| plan.has_totality_certificate())
            .count();
        assert!(
            arithmetic >= 2,
            "{name}: diagonal certificates are retained"
        );
    }
}
