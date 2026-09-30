//! The retained upstream analysis describes the exact bounded source projection;
//! its class/safety verdicts neither replace native scope checks nor lift quotas.

use std::collections::BTreeSet;
use std::fs;

use themelios_analysis::{Analysis, Verdict};
use themelios_base::source::SourceId;
use themelios_program::program::Statement;
use themelios_program::provenance::Origin;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, SourceBundle, admit_bundle_formula,
    admit_formula,
};

fn admit_with(text: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        text.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
    )
}

#[test]
fn empty_and_exact_structure_ceilings_are_inclusive() {
    admit_with(
        "",
        &FormulaLimits {
            max_analysis_nodes: 0,
            max_analysis_edges: 0,
            ..FormulaLimits::default()
        },
    )
    .expect("zero budget admits zero structure");
    let limits = FormulaLimits {
        max_analysis_nodes: 4,
        max_analysis_edges: 2,
        ..FormulaLimits::default()
    };
    let source = "a :- b,c.";
    admit_with(source, &limits).expect("statement plus three atoms and two edges");
    for (limited, expected) in [
        (
            FormulaLimits {
                max_analysis_nodes: 3,
                ..limits
            },
            FormulaResource::AnalysisNodes,
        ),
        (
            FormulaLimits {
                max_analysis_edges: 1,
                ..limits
            },
            FormulaResource::AnalysisEdges,
        ),
    ] {
        let error = admit_with(source, &limited).expect_err("one below inclusive ceiling");
        assert!(matches!(error, FormulaFailure::Limit {
            resource, observed, limit, ..
        } if resource == expected && observed == limit + 1));
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn analysis_is_for_the_normalized_projection_with_original_fact_origins() {
    let input = admit_with("#const n=2. #show p/1. p(1..n).", &FormulaLimits::default())
        .expect("bounded constant and fact expansion");
    let program = input.analyzed_program();
    assert_eq!(input.source_analysis(), &Analysis::of(program));
    assert_eq!(program.statements().count(), 2);
    for statement in program.statements() {
        assert!(matches!(statement.get(), Statement::Rule(_)));
        let locations: Vec<_> = statement
            .provenance()
            .origins()
            .filter_map(|origin| match origin {
                Origin::Parsed(location) => Some(*location),
                Origin::Constructed | Origin::Transformed(_) => None,
            })
            .collect();
        assert_eq!(locations.len(), 1);
        assert_eq!(
            input
                .source()
                .slice(locations[0].span)
                .expect("original span"),
            "p(1..n)."
        );
    }
}

#[test]
fn upstream_safety_disagreement_is_visible_and_does_not_weaken_native_scope_checks() {
    let input = admit_with("n(N) :- N=#sum{}.", &FormulaLimits::default())
        .expect("separately checked clingo aggregate equality binder");
    assert_eq!(
        input.source_analysis(),
        &Analysis::of(input.analyzed_program())
    );
    assert!(
        !input.source_analysis().safety().is_safe(),
        "pinned ASP-Core-2 reading is retained"
    );
    for text in [
        "n(N) :- not p(N).",
        "n(N) :- X=#sum{}.",
        "n(N) :- N=#sum{X:p(Y)}.",
    ] {
        assert!(
            admit_with(text, &FormulaLimits::default()).is_err(),
            "unsafe source: {text}"
        );
    }
}

#[test]
fn unknown_class_verdict_does_not_remove_the_support_round_ceiling() {
    let input = admit_with("a :- a.", &FormulaLimits::default()).expect("finite unsupported cycle");
    assert!(matches!(
        input.source_analysis().classes().tightness(),
        Verdict::Unknown { .. }
    ));
    assert!(input.atoms().is_empty(), "analysis cannot invent support");
    let error = admit_with(
        "a :- a.",
        &FormulaLimits {
            max_support_rounds: 0,
            ..FormulaLimits::default()
        },
    )
    .expect_err("analysis cannot authorize incomplete support closure");
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            ..
        }
    ));
}

#[test]
fn duplicate_bundle_facts_keep_every_source_origin_in_the_analyzed_projection() {
    let directory = tempfile::tempdir().expect("fixture directory");
    fs::write(
        directory.path().join("entry.lp"),
        "#include \"child.lp\". p(1). q(X):-p(X).",
    )
    .expect("entry source");
    fs::write(directory.path().join("child.lp"), "p(1). #show p/1.").expect("child source");
    let bundle = SourceBundle::load(directory.path().join("entry.lp"), BundleLimits::default())
        .expect("bounded includes");
    let input = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("bundle analysis");
    assert_eq!(
        input.source_analysis(),
        &Analysis::of(input.analyzed_program())
    );
    let mut duplicate = false;
    for statement in input.analyzed_program().statements() {
        let origins: Vec<_> = statement
            .provenance()
            .origins()
            .filter_map(|origin| match origin {
                Origin::Parsed(location) => Some(*location),
                Origin::Constructed | Origin::Transformed(_) => None,
            })
            .collect();
        if origins.len() == 2 {
            duplicate = true;
            assert_eq!(
                origins
                    .iter()
                    .map(|origin| origin.source)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([SourceId::new(0), SourceId::new(1)])
            );
            for location in origins {
                let source = input
                    .bundle()
                    .get(location.source)
                    .expect("retained original source");
                assert_eq!(
                    source.source().slice(location.span).expect("source span"),
                    "p(1)."
                );
            }
        }
    }
    assert!(duplicate, "canonical fact merges both original locations");
    directory.close().expect("fixture cleanup");
}
