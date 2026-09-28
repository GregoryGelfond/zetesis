//! Source validation is independent of whether a rule can contribute an answer.

use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, ProfileFeature, admit_extended, prepare_formula,
};

#[test]
fn authored_terms_reject_undefined_arithmetic() {
    for source in [
        "p(f(1/0)) :- 1=0.",
        "p(f(2147483647+1)) :- 1=0.",
        "p :- 1=0, f(1/0)=f(0).",
        "#const unused=f(1/0). q.",
    ] {
        let error = admit_extended(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, ExpansionFailure::Evaluation { .. }),
            "{source}: {error}"
        );
        let error = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
    }
}

#[test]
fn authored_terms_reject_nested_nul_values() {
    for source in [
        "p(f(\"bad\0value\")) :- 1=0.",
        "#const unused=f(\"bad\0value\"). q.",
    ] {
        let error = admit_extended(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::NulString,
                    ..
                })
            ),
            "{error}"
        );
        let error = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::NulString,
                    ..
                }))
            ),
            "{error}"
        );
    }
}
