//! The shared route agrees with composition of the retained public operations.

use std::fmt::Write;
use std::path::{Path, PathBuf};

use super::*;
use crate::{AdmissionOptions, BundleLimits, MetadataStorageLimits};

const LATE_FORMULA: &str = "p(1). q(X) :- p(X), X+1=2.";
const CASES: &[&str] = &[
    "a.",
    "#const n=2. p(1..n).",
    "{p}.",
    "1{p;q}1.",
    "p|q.",
    LATE_FORMULA,
    "#show p/0. #show p/0. p.",
    "#show p(X) : p(X). p(1).",
    "#project p/0. {p}.",
    ":~ p. [1@0] {p}.",
    "p(X) :- not q(X).",
    "#const a=b. #const b=a. p(a).",
    "p(1/0).",
    "p(2147483648).",
    "#program step(t). p(t).",
];

type SourceResult = Result<
    AutomaticAdmission<Admitted, PreparedFormula>,
    AutomaticAdmissionFailure<SourceFailure<ExpansionFailure>, SourceFailure<FormulaFailure>>,
>;
type BundleResult = Result<
    AutomaticAdmission<AdmittedBundle, PreparedFormulaBundle>,
    AutomaticAdmissionFailure<BundleAdmissionFailure, FormulaBundleFailure>,
>;

fn parsed(text: &str) -> ParsedSource {
    ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap()
}

fn source_reference(
    text: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> SourceResult {
    match parsed(text).admit_extended(expansion) {
        Ok(admitted) => Ok(AutomaticAdmission::Extended(admitted)),
        Err(error) if error.error().needs_formula_admission() => error
            .into_source()
            .prepare_formula(expansion, *limits)
            .map(AutomaticAdmission::Formula)
            .map_err(AutomaticAdmissionFailure::Formula),
        Err(error) => Err(AutomaticAdmissionFailure::Extended(error)),
    }
}

fn source_automatic(
    text: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> SourceResult {
    parsed(text).admit_automatic_with_cancellation(expansion, *limits, &Cancellation::default())
}

/// Compare source payloads and semantic public views, never a catalog's
/// implementation Debug (which contains unordered hash-index buckets).
#[derive(Debug)]
struct SourceReceipt(SourceResult);

fn source_receipt(result: SourceResult) -> SourceReceipt {
    if let Ok(AutomaticAdmission::Formula(prepared)) = &result {
        assert!(
            prepared.preparation().options.domains.is_none(),
            "library domain analysis stays opt-in"
        );
    }
    SourceReceipt(result)
}

fn same_source(
    left: &themelios_base::source::Source,
    right: &themelios_base::source::Source,
) -> bool {
    left.id() == right.id() && left.span() == right.span() && left.text() == right.text()
}

/// Canonical logical equality ignores provenance, so retain the full nested
/// provenance comparison through the logical program's deterministic Debug.
fn same_logical(left: &Program, right: &Program) -> bool {
    left == right && format!("{left:?}") == format!("{right:?}")
}

impl PartialEq for SourceReceipt {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Ok(AutomaticAdmission::Extended(left)), Ok(AutomaticAdmission::Extended(right))) => {
                zetesis_test_support::programs::rules(left.program())
                    .eq(zetesis_test_support::programs::rules(right.program()))
                    && left.template_origins() == right.template_origins()
                    && left.fact_origins() == right.fact_origins()
                    && left.metadata() == right.metadata()
                    && left.expansion_usage() == right.expansion_usage()
                    && same_source(left.source(), right.source())
            }
            (Ok(AutomaticAdmission::Formula(left)), Ok(AutomaticAdmission::Formula(right))) => {
                same_logical(left.original_program(), right.original_program())
                    && same_logical(left.analyzed_program(), right.analyzed_program())
                    && left.metadata() == right.metadata()
                    && left.comparison_forms() == right.comparison_forms()
                    && left.preparation().budget.usage() == right.preparation().budget.usage()
                    && same_source(left.source().unwrap(), right.source().unwrap())
            }
            (
                Err(AutomaticAdmissionFailure::Extended(left)),
                Err(AutomaticAdmissionFailure::Extended(right)),
            ) => {
                format!("{:?}", left.error()) == format!("{:?}", right.error())
                    && same_source(left.source().source(), right.source().source())
            }
            (
                Err(AutomaticAdmissionFailure::Formula(left)),
                Err(AutomaticAdmissionFailure::Formula(right)),
            ) => {
                format!("{:?}", left.error()) == format!("{:?}", right.error())
                    && same_source(left.source().source(), right.source().source())
            }
            _ => false,
        }
    }
}

fn bundle_reference(bundle: SourceBundle, limits: &FormulaLimits) -> BundleResult {
    match crate::admit_bundle_extended(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    ) {
        Ok(admitted) => Ok(AutomaticAdmission::Extended(admitted)),
        Err(error) if matches!(error.error(), BundleAdmissionError::Expansion(error) if error.needs_formula_admission()) => {
            crate::prepare_bundle_formula(
                error.into_bundle(),
                BundleAdmissionOptions::default(),
                ExpansionLimits::default(),
                *limits,
            )
            .map(AutomaticAdmission::Formula)
            .map_err(AutomaticAdmissionFailure::Formula)
        }
        Err(error) => Err(AutomaticAdmissionFailure::Extended(error)),
    }
}

fn bundle_automatic(bundle: SourceBundle, limits: &FormulaLimits) -> BundleResult {
    admit_bundle_automatic_with_cancellation(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
        &Cancellation::default(),
    )
}

#[derive(Debug)]
struct BundleReceipt(BundleResult);

fn bundle_receipt(result: BundleResult) -> BundleReceipt {
    BundleReceipt(result)
}

fn same_bundle(left: &SourceBundle, right: &SourceBundle) -> bool {
    left.entry() == right.entry()
        && left.working_directory() == right.working_directory()
        && format!("{:?}", left.roots()) == format!("{:?}", right.roots())
        && left.sources().len() == right.sources().len()
        && left
            .sources()
            .iter()
            .zip(right.sources())
            .all(|(left, right)| {
                left.path() == right.path()
                    && left.loaded_path() == right.loaded_path()
                    && same_source(left.source(), right.source())
                    && format!("{:?}", left.includes()) == format!("{:?}", right.includes())
            })
}

impl PartialEq for BundleReceipt {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Ok(AutomaticAdmission::Extended(left)), Ok(AutomaticAdmission::Extended(right))) => {
                zetesis_test_support::programs::rules(left.program())
                    .eq(zetesis_test_support::programs::rules(right.program()))
                    && left.template_origins() == right.template_origins()
                    && left.fact_origins() == right.fact_origins()
                    && left.metadata() == right.metadata()
                    && left.expansion_usage() == right.expansion_usage()
                    && same_bundle(left.bundle(), right.bundle())
            }
            (Ok(AutomaticAdmission::Formula(left)), Ok(AutomaticAdmission::Formula(right))) => {
                same_logical(left.original_program(), right.original_program())
                    && same_logical(left.analyzed_program(), right.analyzed_program())
                    && left.metadata() == right.metadata()
                    && left.comparison_forms() == right.comparison_forms()
                    && left.preparation().budget.usage() == right.preparation().budget.usage()
                    && same_bundle(left.bundle(), right.bundle())
            }
            (
                Err(AutomaticAdmissionFailure::Extended(left)),
                Err(AutomaticAdmissionFailure::Extended(right)),
            ) => {
                format!("{:?}", left.error()) == format!("{:?}", right.error())
                    && same_bundle(left.bundle(), right.bundle())
            }
            (
                Err(AutomaticAdmissionFailure::Formula(left)),
                Err(AutomaticAdmissionFailure::Formula(right)),
            ) => {
                format!("{:?}", left.error()) == format!("{:?}", right.error())
                    && same_bundle(left.bundle(), right.bundle())
            }
            _ => false,
        }
    }
}

#[test]
fn routes_failures_provenance_and_expansion_receipts_match_retained_source_operations() {
    for text in CASES {
        for max_term_work in [0, 1, 8, 100, ExpansionLimits::default().max_term_work] {
            let expansion = ExpansionLimits {
                max_term_work,
                ..ExpansionLimits::default()
            };
            let limits = FormulaLimits::default();
            assert_eq!(
                source_receipt(source_automatic(text, expansion, &limits)),
                source_receipt(source_reference(text, expansion, &limits)),
                "{text}, term ceiling {max_term_work}"
            );
        }
    }
}

#[test]
fn late_source_fallback_raises_once_and_reuses_the_guarded_handoff() {
    let before = testing::counts();
    let actual = source_automatic(
        LATE_FORMULA,
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    );
    let after = testing::counts();
    assert!(matches!(&actual, Ok(AutomaticAdmission::Formula(_))));
    assert_eq!(after.raises - before.raises, 1);
    assert_eq!(after.replays - before.replays, 1);
    assert_eq!(after.formula_attempts - before.formula_attempts, 1);
    assert_eq!(
        source_receipt(actual),
        source_receipt(source_reference(
            LATE_FORMULA,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ))
    );
}

#[test]
fn failed_recorded_formula_verdict_forces_ordinary_preparation() {
    let before = testing::counts();
    let actual = testing::with_rejected_profile(|| {
        source_automatic(
            LATE_FORMULA,
            ExpansionLimits::default(),
            &FormulaLimits::default(),
        )
    });
    let after = testing::counts();
    assert_eq!(after.raises - before.raises, 2);
    assert_eq!(after.replays - before.replays, 0);
    assert!(matches!(&actual, Ok(AutomaticAdmission::Formula(_))));
    assert_eq!(
        source_receipt(actual),
        source_receipt(source_reference(
            LATE_FORMULA,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ))
    );
}

fn no_metadata_storage() -> FormulaLimits {
    FormulaLimits {
        metadata_storage: MetadataStorageLimits { max_bytes: 0 },
        ..FormulaLimits::default()
    }
}

#[test]
fn source_metadata_refusal_is_replayed_at_its_original_step() {
    let text = format!("{LATE_FORMULA} #show p/1.");
    let before = testing::counts();
    let actual = source_automatic(&text, ExpansionLimits::default(), &no_metadata_storage());
    let after = testing::counts();
    assert_eq!(after.raises - before.raises, 1);
    assert_eq!(after.replays - before.replays, 1);
    let Err(AutomaticAdmissionFailure::Formula(error)) = &actual else {
        panic!("formula metadata refusal: {actual:?}")
    };
    assert!(matches!(
        error.error(),
        FormulaFailure::Expansion(ExpansionFailure::Admission(
            crate::AdmissionFailure::Metadata { .. }
        ))
    ));
    assert_eq!(
        source_receipt(actual),
        source_receipt(source_reference(
            &text,
            ExpansionLimits::default(),
            &no_metadata_storage()
        ))
    );
}

#[test]
fn second_of_three_sources_replays_its_metadata_refusal() {
    let directory = tempfile::tempdir().unwrap();
    let paths: Vec<_> = [LATE_FORMULA, "#show p/1.", "r(1)."]
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let path = directory.path().join(format!("{index}.lp"));
            std::fs::write(&path, text).unwrap();
            path
        })
        .collect();
    let load = || SourceBundle::load_many(&paths, BundleLimits::default()).unwrap();
    let before = testing::counts();
    let actual = bundle_automatic(load(), &no_metadata_storage());
    let after = testing::counts();
    assert_eq!(after.raises - before.raises, 3);
    assert_eq!(after.replays - before.replays, 1);
    let Err(AutomaticAdmissionFailure::Formula(error)) = &actual else {
        panic!("formula metadata refusal: {actual:?}")
    };
    let FormulaFailure::Expansion(ExpansionFailure::Admission(crate::AdmissionFailure::Metadata {
        location,
        ..
    })) = error.error()
    else {
        panic!("metadata failure: {error:?}")
    };
    assert_eq!(
        location.location().unwrap().source,
        error.bundle().sources()[1].id()
    );
    assert_eq!(
        bundle_receipt(actual),
        bundle_receipt(bundle_reference(load(), &no_metadata_storage()))
    );
}

#[test]
fn pre_cancelled_extended_attempt_never_starts_formula_preparation() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let before = testing::counts();
    let error = parsed(LATE_FORMULA)
        .admit_automatic_with_cancellation(
            ExpansionLimits::default(),
            FormulaLimits::default(),
            &cancellation,
        )
        .unwrap_err();
    assert!(matches!(&error, AutomaticAdmissionFailure::Extended(_)));
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
    assert_eq!(testing::counts(), before);
}

#[test]
fn program_door_keeps_original_identity_and_one_limit_set_on_both_routes() {
    for (text, relational) in [("p.", true), ("p|q.", false)] {
        let source = parsed(text);
        let original = Arc::new(themelios_program::raise::raise(source.parsed()).into_program());
        let options = ProgramFormulaOptions {
            purpose: crate::FormulaPurpose::AnswerSets,
            ..ProgramFormulaOptions::default()
        };
        match prepare_program_automatic(Arc::clone(&original), options).unwrap() {
            AutomaticAdmission::Extended(prepared) => {
                assert!(relational);
                assert!(std::ptr::eq(prepared.original_program(), original.as_ref()));
            }
            AutomaticAdmission::Formula(prepared) => {
                assert!(!relational);
                assert!(std::ptr::eq(prepared.original_program(), original.as_ref()));
                assert!(prepared.source().is_none());
                assert!(prepared.preparation().options.domains.is_none());
            }
        }
    }
    let source = parsed("#show p/0. p.");
    let original = Arc::new(themelios_program::raise::raise(source.parsed()).into_program());
    let error = prepare_program_automatic(
        original,
        ProgramFormulaOptions {
            formula: no_metadata_storage(),
            ..ProgramFormulaOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error, AutomaticAdmissionFailure::Extended(_)),
        "relational metadata uses the formula limit set"
    );
}

#[test]
fn all_corpus_bundle_routes_and_preparations_match_retained_operations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
            .unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 94);
    for case in cases {
        let path = root.join("kr-domains").join(case["path"].as_str().unwrap());
        let load = || SourceBundle::load(&path, BundleLimits::default()).unwrap();
        assert_eq!(
            bundle_receipt(bundle_automatic(load(), &FormulaLimits::default())),
            bundle_receipt(bundle_reference(load(), &FormulaLimits::default())),
            "{}",
            path.display()
        );
    }
}

fn profile_agrees(text: &str) {
    let Ok(source) = ParsedSource::new(text.into(), AdmissionOptions::default()) else {
        return;
    };
    for includes in [false, true] {
        for max_syntax_nodes in [0, 8, AdmissionOptions::default().max_syntax_nodes] {
            let options = AdmissionOptions {
                max_syntax_nodes,
                ..source.options()
            };
            let [extended, formula] =
                crate::profile::check_automatic(source.parsed(), options, includes);
            let expected_formula =
                crate::profile::check_formula(source.parsed(), options, includes);
            assert_eq!(
                format!("{formula:?}"),
                format!("{expected_formula:?}"),
                "{text}"
            );
            let expected_extended = if includes {
                crate::profile::check_bundle(source.parsed(), options).map(|_| ())
            } else {
                crate::profile::check_extended(source.parsed(), options)
            };
            assert_eq!(
                format!("{:?}", extended.as_ref().map(|_| ())),
                format!("{expected_extended:?}"),
                "{text}"
            );
            if let Ok(nodes) = extended {
                assert_eq!(
                    formula.unwrap(),
                    nodes,
                    "extended acceptance implies formula acceptance: {text}"
                );
                assert!(
                    !source
                        .parsed()
                        .tree()
                        .statements()
                        .any(|statement| matches!(
                            statement,
                            themelios_syntax::ast::Statement::WeakConstraint(_)
                                | themelios_syntax::ast::Statement::Optimize(_)
                        ))
                );
            }
        }
    }
    let expected = themelios_program::raise::raise(source.parsed());
    let actual = themelios_program::raise::raise_occurrences(source.parsed()).into_raised();
    assert_eq!(actual.diagnostics(), expected.diagnostics());
    assert_eq!(actual.program(), expected.program());
    assert_eq!(
        format!("{:?}", actual.program()),
        format!("{:?}", expected.program()),
        "nested provenance: {text}"
    );
}

fn files_under(directory: &Path, paths: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_under(&path, paths);
        } else {
            paths.push(path);
        }
    }
}

#[test]
fn paired_profiles_and_raise_preserve_corpus_and_refusal_fixture_verdicts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    files_under(&root.join("../../validation/corpus/kr-domains"), &mut paths);
    files_under(&root.join("tests/fixtures"), &mut paths);
    paths.sort();
    let mut sources = 0;
    for path in paths {
        match path.extension().and_then(|value| value.to_str()) {
            Some("lp") => {
                profile_agrees(&std::fs::read_to_string(path).unwrap());
                sources += 1;
            }
            Some("jsonl") => {
                for line in std::fs::read_to_string(path)
                    .unwrap()
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                {
                    let value: serde_json::Value = serde_json::from_str(line).unwrap();
                    if let Some(source) = value.get("source").and_then(serde_json::Value::as_str) {
                        profile_agrees(source);
                        sources += 1;
                    }
                }
            }
            _ => {}
        }
    }
    assert!(sources >= 108);
    for text in CASES {
        profile_agrees(text);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn generated_fact_and_comparison_sources_preserve_profiles_and_preparations(values in proptest::collection::vec(-20i32..20, 0..25)) {
        let mut text = String::new();
        for value in values {
            write!(&mut text, "p({value}).").unwrap();
        }
        text.push_str("q(X) :- p(X), X+1=2.");
        profile_agrees(&text);
        assert_eq!(source_receipt(source_automatic(&text, ExpansionLimits::default(), &FormulaLimits::default())), source_receipt(source_reference(&text, ExpansionLimits::default(), &FormulaLimits::default())));
    }
}
