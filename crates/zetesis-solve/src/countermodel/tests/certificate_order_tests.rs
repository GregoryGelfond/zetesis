//! Analysis orders bounded attempts; it does not establish their applicability.

use super::{certificate_order, test_harness::admitted};
use zetesis_sat::CertificateOrder;
use zetesis_themelios::AnalysisBasis;

#[test]
fn complete_horn_source_prefers_positive_checking() {
    let owner = admitted("p. q :- p.");
    assert_eq!(owner.analysis_basis(), AnalysisBasis::NormalizedProgram);
    assert_eq!(
        certificate_order(owner.source_analysis(), owner.analysis_basis()),
        CertificateOrder::PositiveFirst,
    );
}

#[test]
fn dependency_projection_does_not_select_positive_first() {
    let owner = admitted("p. q :- p.");
    // The same Horn verdict cannot authorize a source-class shortcut when
    // its stated subject is only a dependency projection.
    assert_eq!(
        certificate_order(owner.source_analysis(), AnalysisBasis::DependencyProjection),
        CertificateOrder::TightFirst,
    );
}

#[test]
fn choice_source_keeps_tight_first() {
    let owner = admitted("{p}.");
    assert_eq!(
        certificate_order(owner.source_analysis(), owner.analysis_basis()),
        CertificateOrder::TightFirst,
    );
}

#[test]
fn complete_stratified_source_prefers_direct_evaluation() {
    let owner = admitted(include_str!("../../../tests/fixtures/stratified/cycle.lp"));
    assert_eq!(
        certificate_order(owner.source_analysis(), owner.analysis_basis()),
        CertificateOrder::StratifiedFirst
    );
}

#[test]
fn dependency_projection_does_not_select_stratified_first() {
    let owner = admitted(include_str!("../../../tests/fixtures/stratified/cycle.lp"));
    assert_eq!(
        certificate_order(owner.source_analysis(), AnalysisBasis::DependencyProjection),
        CertificateOrder::TightFirst
    );
}
