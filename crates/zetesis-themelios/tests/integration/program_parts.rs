//! Explicit base sections preserve the original single-shot program.

use crate::support::finite_bindings as reference;

use std::collections::BTreeSet;

use reference::{Models, exhaustive, external, native};
use themelios_base::source::SourceId;
use zetesis_reference_support::formula;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits,
    InputLimit, ProfileFeature, admit, admit_extended, admit_formula, prepare_formula,
};

const CASES: &[(&str, &str)] = &[
    ("#program base.", ""),
    ("#program base().", ""),
    ("#program base. a.", "a."),
    ("a. #program base. b. #program base. c.", "a. b. c."),
    (
        "#program base. a :- b. #program base. b :- a.",
        "a :- b. b :- a.",
    ),
    (
        "#program base. {a}. #program base. b :- not a.",
        "{a}. b :- not a.",
    ),
    (
        "#program base. a | b. #program base. :- a,b.",
        "a | b. :- a,b.",
    ),
    (
        "#program base. 1{a;b}1. #program base. c :- a.",
        "1{a;b}1. c :- a.",
    ),
    ("#program base. -a. #program base. a.", "-a. a."),
    (
        "#const n=2. #program base. p(n). #program base.",
        "#const n=2. p(n).",
    ),
];

#[test]
fn explicit_base_preserves_stable_models() {
    for &(source, implicit) in CASES {
        assert_eq!(
            native(&formula(source)),
            native(&formula(implicit)),
            "{source}"
        );
    }
}

#[test]
fn explicit_base_matches_exhaustive_reduct_checking() {
    for &(source, _) in CASES {
        let admitted = formula(source);
        assert_eq!(native(&admitted), exhaustive(&admitted), "{source}");
    }
}

#[test]
#[ignore = "requires clingo: explicit base matches clingo full models"]
fn explicit_base_matches_clingo_full_models() {
    for &(source, _) in CASES {
        let result = external(source, true);
        assert_eq!(result["Models"]["More"], "no");
        let mut expected = Models::new();
        for call in result["Call"].as_array().unwrap() {
            if let Some(witnesses) = call["Witnesses"].as_array() {
                for witness in witnesses {
                    let model: BTreeSet<_> = witness["Value"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|atom| atom.as_str().unwrap().to_owned())
                        .collect();
                    assert!(expected.insert(model), "duplicate full model");
                }
            }
        }
        assert_eq!(
            result["Models"]["Number"].as_u64(),
            Some(expected.len() as u64)
        );
        assert_eq!(native(&formula(source)), expected, "{source}");
    }
}

#[test]
fn scalar_admission_preserves_explicit_base_rules() {
    for source in [
        "#program base.",
        "#program base().",
        "a. #program base. {b}. c :- b.",
    ] {
        let direct = admit(source.into(), AdmissionOptions::default()).unwrap();
        let extended = admit_extended(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .unwrap();
        assert_eq!(
            direct.program().templates().iter().collect::<Vec<_>>(),
            extended.program().templates().iter().collect::<Vec<_>>()
        );
        assert_eq!(direct.source().text(), source);
    }
}

#[test]
fn preparation_retains_original_base_sections() {
    let source = "#program base. p. #program base. q :- p.";
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(prepared.source().text(), source);
    assert!(
        prepared
            .analyzed_program()
            .parts()
            .all(|part| part.key().name.as_str() == "base" && part.key().formals.is_empty())
    );
    let admitted = prepared.ground().unwrap();
    for origins in admitted.formula_origins() {
        for origin in origins {
            let original = admitted.source().slice(origin.span).unwrap();
            assert!(matches!(original, "p." | "q :- p."), "{original}");
        }
    }
}

#[test]
fn base_delimiters_preserve_objective_templates() {
    let implicit = formula("{a}. #minimize{2@1,k:a}.");
    let explicit = formula("#program base. {a}. #program base. #minimize{2@1,k:a}.");
    assert_eq!(
        explicit.objectives().templates().iter().collect::<Vec<_>>(),
        implicit.objectives().templates().iter().collect::<Vec<_>>()
    );
    let declarations = |admitted: &AdmittedFormula| {
        admitted
            .objective_declarations()
            .iter()
            .map(|location| admitted.source().slice(location.span).unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(declarations(&explicit), declarations(&implicit));
}

#[test]
fn nonbase_parts_remain_located_refusals() {
    for delimiter in ["#program step.", "#program step(t).", "#program base(x)."] {
        for source in [
            delimiter.to_owned(),
            format!("p. {delimiter} #program base. q."),
        ] {
            let error = admit(
                source.clone(),
                AdmissionOptions {
                    source_id: SourceId::new(61),
                    ..AdmissionOptions::default()
                },
            )
            .unwrap_err();
            let AdmissionFailure::Profile { feature, location } = error else {
                panic!("expected a program-part refusal: {error}");
            };
            assert_eq!(feature, ProfileFeature::ProgramPart);
            assert_eq!(location.source, SourceId::new(61));
            let parsed = themelios_base::source::Source::new(SourceId::new(61), source).unwrap();
            assert_eq!(parsed.slice(location.span).unwrap(), delimiter);
        }
    }
}

#[test]
fn base_delimiters_consume_the_syntax_budget() {
    let source = "#program base. p.";
    let original = themelios_base::source::Source::new(SourceId::new(0), source.into()).unwrap();
    let parsed = zetesis_themelios::syntax::parse::parse(
        &original,
        zetesis_themelios::syntax::dialect::Dialect::Clingo,
    );
    let nodes = parsed.syntax().descendants().count();
    let options = AdmissionOptions {
        max_syntax_nodes: nodes,
        ..AdmissionOptions::default()
    };
    admit(source.into(), options).unwrap();
    let options = AdmissionOptions {
        max_syntax_nodes: nodes - 1,
        ..options
    };
    assert!(matches!(
        admit(source.into(), options),
        Err(AdmissionFailure::Limit {
            resource: InputLimit::SyntaxNodes,
            limit,
            observed,
            ..
        }) if limit == nodes - 1 && observed == nodes
    ));
}

#[test]
fn base_sections_do_not_hide_invalid_rules() {
    for source in [
        "#program base. p(2147483648).",
        "#program base. p(X).",
        "#program base. p(.",
    ] {
        assert!(
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default(),
            )
            .is_err(),
            "{source}"
        );
    }
}
