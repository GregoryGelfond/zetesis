//! Weak constraints share the exact existing positive objective semantics.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use themelios_base::source::SourceId;
use themelios_program::program::Statement;
use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, InputLimit,
    ProfileFeature, SourceBundle, admit, admit_bundle_formula, admit_extended, admit_formula,
};

#[path = "support/source_records.rs"]
mod source_records;

fn input(source: &str) -> AdmittedFormula {
    source_records::admit(source, FormulaLimits::default())
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}
fn cases() -> Vec<source_records::Case> {
    source_records::cases(include_str!("fixtures/weak-objectives.jsonl"))
}
#[test]
fn complete_model_cost_records_match_independent_clingo_evidence() {
    let cases = cases();
    assert_eq!(cases.len(), 49);
    let mut count = 0;
    for case in cases {
        count += case.records.len();
        assert_eq!(
            source_records::exhaustive(&input(&case.source)),
            case.records,
            "{}",
            case.name
        );
    }
    assert_eq!(count, 186);
}

#[test]
fn original_weak_declarations_and_body_spans_survive_owned_normalization() {
    let source = "{p(1);p(2)}. :~ p(X),X!=2. [3@1,X]";
    let input = admit_formula(
        source.to_owned(),
        AdmissionOptions {
            source_id: SourceId::new(77),
            ..AdmissionOptions::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("weak objective");
    assert_eq!(input.source().text(), source);
    assert_eq!(input.objective_declarations().len(), 1);
    assert!(
        input
            .source()
            .slice(input.objective_declarations()[0].span)
            .expect("declaration span")
            .starts_with(":~")
    );
    let snippets: BTreeSet<_> = input
        .objective_origins()
        .iter()
        .flatten()
        .map(|location| {
            assert_eq!(location.source, SourceId::new(77));
            input
                .source()
                .slice(location.span)
                .expect("original byte span")
        })
        .collect();
    assert!(snippets.contains("p(X)"));
    assert!(snippets.contains("X!=2"));
    assert!(
        input
            .analyzed_program()
            .statements()
            .any(|statement| matches!(statement.get(), Statement::Optimize(_)))
    );
    assert!(
        !input
            .analyzed_program()
            .statements()
            .any(|statement| matches!(statement.get(), Statement::WeakConstraint(_)))
    );
}

#[test]
fn weak_occurrences_count_before_upstream_statement_deduplication() {
    for source in [
        "{a}. :~a.[1,k] :~a.[1,k]",
        "{a}. :~a.[1,k] #minimize{1,k:a}.",
    ] {
        let result = admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                objective: zetesis_objective::AdmissionLimits {
                    max_templates: 1,
                    ..zetesis_objective::AdmissionLimits::default()
                },
                ..FormulaLimits::default()
            },
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::ObjectiveElements,
                observed: 2,
                ..
            })
        ));
    }
    let source = "a.b. :~a,b.[1,k]";
    let result = admit_formula(
        source.to_owned(),
        AdmissionOptions {
            max_body_elements: 1,
            ..AdmissionOptions::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Limit {
                resource: InputLimit::BodyElements,
                observed: 2,
                ..
            }
        )))
    ));
}

#[test]
fn safety_and_outside_positive_profile_remain_typed_refusals() {
    for source in [":~.[X]", ":~X=1.[X]", "{p(1)}. :~p(_).[_]"] {
        assert!(
            matches!(
                admit_formula(
                    source.to_owned(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    FormulaLimits::default()
                ),
                Err(FormulaFailure::UnsafeVariable { .. })
            ),
            "{source}"
        );
    }
    for source in [
        "{a}. :~not a.[1]",
        "{a}. :~not not a.[1]",
        "{a}. :~#count{1:a}>0.[1]",
        "{a}. :~a:a.[1]",
        "{p(1)}. :~p(X),X>0.[X]",
        "{p(1)}. :~p(X).[X+1]",
        "{p(1)}. :~p(X).[1@X]",
    ] {
        assert!(
            matches!(
                admit_formula(
                    source.to_owned(),
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    FormulaLimits::default()
                ),
                Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
                    AdmissionFailure::Profile {
                        feature: ProfileFeature::Objective,
                        ..
                    }
                )))
            ),
            "{source}"
        );
    }
    let source = "{a}. :~a.[1]";
    assert!(admit(source.to_owned(), AdmissionOptions::default()).is_err());
    assert!(
        admit_extended(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "zetesis-weak-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("temporary fixture: {error}"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("fixture cleanup");
    }
}

#[test]
fn weak_and_minimize_keys_coalesce_across_original_include_sources() {
    let directory = Directory::new();
    fs::write(
        directory.0.join("main.lp"),
        "#include \"child.lp\". {a;b}. :~a.[2@1,k]",
    )
    .expect("entry source");
    fs::write(directory.0.join("child.lp"), "#minimize{2@1,k:b}.").expect("child source");
    let bundle = SourceBundle::load(directory.0.join("main.lp"), BundleLimits::default())
        .expect("original graph");
    let input = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("bundle formula");
    let sources: BTreeSet<_> = input
        .objective_declarations()
        .iter()
        .map(|location| location.source)
        .collect();
    assert_eq!(sources.len(), 2);
    for location in input.objective_origins().iter().flatten() {
        input
            .bundle()
            .get(location.source)
            .expect("original source")
            .source()
            .slice(location.span)
            .expect("original span");
    }
    let model = Model::new(input.atoms().iter().cloned());
    let evaluation = zetesis_objective::evaluate(
        input.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Control::default(),
    )
    .expect("global tuple identity");
    assert_eq!(evaluation.score().costs(), &[(1, 2)]);
    assert_eq!(evaluation.contributions().len(), 1);
}

#[test]
#[ignore = "requires independent clingo; 49 bounded complete model/cost cases"]
fn fresh_clingo_confirms_every_recorded_weak_contract() {
    for case in cases() {
        assert_eq!(
            source_records::clingo(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}
