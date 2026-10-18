use super::*;
use themelios_program::term::BinaryOp;

#[test]
fn total_arithmetic_enables_batched_consequences() {
    batched_units(
        include_str!("../../../../../tests/fixtures/streamed-consequences/arithmetic/sum-units.lp"),
        &[[1, 3], [2, 2], [3, 1]],
    );
}

#[test]
fn constant_products_enable_batched_consequences() {
    batched_units(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/product-units.lp"
        ),
        &[[1, 3], [2, 1]],
    );
}

#[test]
fn literal_quotients_enable_batched_consequences() {
    batched_units(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/quotient-units.lp"
        ),
        &[[-7, -1], [-7, 1]],
    );
}

#[test]
fn literal_remainders_enable_batched_consequences() {
    batched_units(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/remainder-units.lp"
        ),
        &[[-7, -1]],
    );
}

fn batched_units(source: &str, expected: &[[i32; 2]]) {
    let owner = hybrid(source);
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
    assert_eq!(state(&checker).pending.len(), expected.len());
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
    assert_eq!(units, expected.iter().copied().collect());
}

#[test]
fn correlated_products_keep_the_original_scan() {
    let owner = hybrid(include_str!(
        "../../../../../tests/fixtures/streamed-consequences/arithmetic/correlated-product.lp"
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
}

#[test]
fn variable_products_enable_batched_consequences() {
    batched_units(
        include_str!(
            "../../../../../tests/fixtures/streamed-consequences/arithmetic/variable-product.lp"
        ),
        &[[1, 2]],
    );
}

#[test]
fn one_partial_rule_keeps_partition_order() {
    let owner = hybrid(include_str!(
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
    let owner = hybrid(include_str!(
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
        let owner = hybrid(&source);
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

#[test]
fn keyed_send_certifies_generated_binding_scans() {
    let path =
        zetesis_test_support::repository::correctness().join("standalone/send-money/send-money.lp");
    let source = std::fs::read_to_string(path).unwrap();
    let owner = hybrid(&source);
    assert!(owner.keyed_constraints() > 0);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
    let prepared = checker.prepared.as_ref().unwrap();
    // The exact-one key rewrite replaces each written carry equation by
    // demanded atoms. Their generated arguments now receive the same
    // complete-column arithmetic certificate as direct carry expressions.
    assert!(
        prepared
            .source
            .rules
            .iter()
            .flat_map(|rule| &rule.body)
            .any(|literal| {
                let LiteralIr::Bind { value, .. } = literal else {
                    return false;
                };
                value.nodes.iter().any(|operation| {
                    matches!(
                        operation,
                        Operation::Binary(BinaryOp::Div | BinaryOp::Mod, ..)
                    )
                })
            })
    );
    assert_eq!(prepared.incremental_eligible, Some(true));
    assert!(prepared.incremental.is_some());
    assert!(
        prepared
            .plans
            .iter()
            .flatten()
            .any(crate::formula_hybrid::PreparedRule::has_totality_certificate)
    );
}

#[test]
fn unrewritten_send_arithmetic_enables_batching() {
    let path =
        zetesis_test_support::repository::correctness().join("standalone/send-money/send-money.lp");
    let source = std::fs::read_to_string(path).unwrap();
    let owner = crate::prepare_formula(
        source,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_key_work: 0,
            ..FormulaLimits::default()
        },
    )
    .unwrap()
    .ground_hybrid()
    .unwrap();
    assert_eq!(owner.keyed_constraints(), 0);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let _ = pass(&mut checker, &region, ConstraintRegionPass::First);
    let prepared = checker
        .prepared
        .as_ref()
        .expect("streamed arithmetic constraints");
    assert_eq!(prepared.incremental_eligible, Some(true));
    assert!(prepared.incremental.is_some());
    assert!(
        prepared
            .plans
            .iter()
            .flatten()
            .filter(|plan| plan.has_totality_certificate())
            .count()
            >= 4,
        "the four written carry equations retain their numeric certificates"
    );
}

#[test]
fn variable_quotients_enable_batched_consequences() {
    batched_units(
        include_str!("../../../../../tests/fixtures/column-totality/positive-quotient.lp"),
        &[[4, 2], [5, 2]],
    );
    batched_units(
        include_str!("../../../../../tests/fixtures/column-totality/negative-quotient.lp"),
        &[[-4, -2], [-5, -2]],
    );
}

#[test]
fn variable_remainders_enable_batched_consequences() {
    batched_units(
        include_str!("../../../../../tests/fixtures/column-totality/negative-remainder.lp"),
        &[[-5, -2], [-7, -3]],
    );
}

#[test]
fn zero_containing_hulls_keep_the_original_scan() {
    for source in [
        include_str!("../../../../../tests/fixtures/column-totality/zero-hull-quotient.lp"),
        include_str!("../../../../../tests/fixtures/column-totality/zero-hull-remainder.lp"),
    ] {
        let owner = hybrid(source);
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let region = Region::all_open(owner.atom_catalog().atoms().len());
        assert!(matches!(
            pass(&mut checker, &region, ConstraintRegionPass::First),
            ConstraintConsequence::Cut { .. }
        ));
        let prepared = checker.prepared.as_ref().unwrap();
        assert_eq!(prepared.incremental_eligible, Some(false));
        assert!(prepared.incremental.is_none());
    }
}
