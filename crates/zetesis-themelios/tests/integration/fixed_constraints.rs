//! Fixed witness specialization retains the original full-answer vocabulary.

use crate::support::stable_models::stable;
use themelios_program::provenance::Origin;
use zetesis_clingo_support as oracle;
use zetesis_reference_support::{exhaustive, formula};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, prepare_formula};

const SOURCE: &str = include_str!("../fixtures/fixed-constraints/projection.lp");
const EXPECTED: &str = include_str!("../fixtures/fixed-constraints/expected.lp");

#[test]
fn fixed_witness_specialization_preserves_the_full_family() {
    assert_eq!(stable(&formula(SOURCE)), stable(&formula(EXPECTED)));
}

#[test]
fn structural_specialization_preserves_the_full_family() {
    let source = include_str!("../fixtures/fixed-constraints/nested.lp");
    let expected = include_str!("../fixtures/fixed-constraints/expected-nested.lp");
    assert_eq!(stable(&formula(source)), stable(&formula(expected)));
}

#[test]
fn preparation_retains_the_original_program() {
    let prepared = prepare_formula(
        SOURCE.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let original = prepared.original_program().clone();
    let admitted = prepared.ground().unwrap();
    assert_eq!(admitted.original_program(), &original);
}

#[test]
fn normalized_program_exposes_the_replacement_family() {
    let prepare = |source: &str| {
        prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
    };
    assert_eq!(
        prepare(SOURCE).analyzed_program(),
        prepare(EXPECTED).analyzed_program()
    );
}

/// Compare the original source with the independent external solver, requiring
/// that the native preparation actually specialized its fixed witnesses.
fn matches_original_clingo(source: &str) {
    let input = formula(source);
    assert!(
        input.analyzed_program().statements().any(|carrier| {
            carrier.provenance().origins().any(|origin| {
            matches!(origin, Origin::Transformed(tag) if tag.as_str() == "zetesis-fixed-constraint")
        })
        }),
        "the fixed-witness rewrite must execute"
    );
    // Exhaustive reduct membership checks every native candidate. The oracle
    // receives the original source, without a handwritten replacement family.
    // These fixtures have no display/projection directives or objectives.
    assert_eq!(exhaustive(&input), oracle::records(source), "{source}");
}

#[test]
#[ignore = "requires clingo: fixed witnesses preserve the original complete full-model family"]
fn fixed_witnesses_match_original_clingo_family() {
    matches_original_clingo(SOURCE);
}

#[test]
#[ignore = "requires clingo: correlated fixed guards preserve the original complete full-model family"]
fn correlated_witnesses_match_original_clingo_family() {
    matches_original_clingo(include_str!("../fixtures/fixed-constraints/correlation.lp"));
}

#[test]
#[ignore = "requires clingo: signed constructor specialization preserves the original complete full-model family"]
fn signed_constructor_witnesses_match_original_clingo_family() {
    matches_original_clingo(include_str!(
        "../fixtures/fixed-constraints/signed-constructor-oracle.lp"
    ));
}
