//! Static support selection preserves the original source family's obligations.

use super::{DomainLimits, JoinStrategy, Observation, compared, equal, prepare};

const PRIME: &str = include_str!("../../fixtures/numeric-domains/static-totality.lp");
const PREFIX: &str = include_str!("../../fixtures/numeric-domains/static-totality-prefix.lp");
const RECURSIVE: &str = include_str!("../../fixtures/numeric-domains/static-totality-recursive.lp");
const CHOICE: &str = include_str!("../../fixtures/numeric-domains/static-totality-choice-zero.lp");
const DISJUNCTION: &str =
    include_str!("../../fixtures/numeric-domains/static-totality-disjunction-zero.lp");

#[test]
fn support_selection_preserves_complete_ground_formulas() {
    for source in [PRIME, PREFIX, RECURSIVE, CHOICE, DISJUNCTION] {
        for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
            compared(source, strategy);
        }
    }
}

#[test]
fn numeric_diagnostics_do_not_enable_support_selection() {
    let plain = prepare(PRIME)
        .unwrap()
        .with_domain_analysis(Some(DomainLimits::default()))
        .ground()
        .unwrap();
    let reported = prepare(PRIME)
        .unwrap()
        .with_domain_analysis(Some(DomainLimits::default()))
        .ground_with_observer(Some(&Observation::default()))
        .unwrap();
    equal(&plain, &reported);
}

#[test]
fn hybrid_preparation_uses_the_same_static_support_contract() {
    for source in [PRIME, PREFIX, RECURSIVE] {
        let ordinary = prepare(source).unwrap().ground_hybrid().unwrap();
        let selected = prepare(source)
            .unwrap()
            .with_domain_analysis(Some(DomainLimits::default()))
            .ground_hybrid()
            .unwrap();
        assert_eq!(
            ordinary.atom_catalog().atoms(),
            selected.atom_catalog().atoms()
        );
        assert_eq!(
            ordinary.core_theory().nodes(),
            selected.core_theory().nodes()
        );
        assert_eq!(
            ordinary.core_theory().operands(),
            selected.core_theory().operands()
        );
        assert_eq!(
            ordinary.core_theory().roots(),
            selected.core_theory().roots()
        );
    }
}

#[test]
fn numeric_failures_keep_their_source_diagnostics() {
    for source in [
        include_str!("../../fixtures/numeric-domains/static-totality-mixed.lp"),
        include_str!("../../fixtures/numeric-domains/static-totality-overflow.lp"),
    ] {
        let ordinary = prepare(source).unwrap().ground().unwrap_err();
        let selected = prepare(source)
            .unwrap()
            .with_domain_analysis(Some(DomainLimits::default()))
            .ground()
            .unwrap_err();
        assert_eq!(ordinary.to_string(), selected.to_string());
    }
}
