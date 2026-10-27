//! Source raising preserves counted content, diagnostics and metadata.

use crate::ProgramSite;
use themelios_base::source::{Source, SourceId};
use themelios_program::program::{Head, Statement};
use themelios_program::raise::raise as raise_program;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::parse::parse;

use super::*;
use crate::ExpansionFailure;

fn source(text: &str) -> Source {
    Source::new(SourceId::new(9), text.into()).expect("bounded source")
}

fn fallback(source: &Source) -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: source.id(),
        span: source.span(),
    })
}

#[test]
fn counted_rules_keep_unequal_element_multiplicities() {
    for text in ["1{#true}1.1{#true;#true}1.", "1{#true;#true}1.1{#true}1."] {
        let source = source(text);
        let parsed = parse(&source, Dialect::Clingo);
        let program = raise(&parsed, &mut metadata::Builder::default()).unwrap();
        let mut counts: Vec<_> = program
            .statements()
            .map(|statement| {
                let Statement::Rule(rule) = statement.get() else {
                    panic!("rule")
                };
                let Head::Choice(choice) = rule.head().get() else {
                    panic!("choice")
                };
                choice.elements().count()
            })
            .collect();
        counts.sort_unstable();
        assert_eq!(counts, [1, 2]);
    }
}

#[test]
fn raising_preserves_the_original_program_parts() {
    let source = source("1{#true}1. #program step(t). 1{#false}1. #program base. 1{#true;#true}1.");
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let actual = raise(&parsed, &mut metadata::Builder::default()).unwrap();
    let expected = raise_program(&parsed).into_program();
    assert_eq!(actual, expected);
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(actual.parts().count(), 2);
}

#[test]
fn metadata_matches_collection_from_the_program() {
    let source = source("#show z/0. #defined a/0. 1{#true}1. #show. #show z/0. #show 7.");
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let mut actual = metadata::Builder::default();
    let program = raise(&parsed, &mut actual).unwrap();
    let mut expected = metadata::Builder::default();
    metadata::collect_profile_at(&program, &mut expected, crate::ProgramSite::program()).unwrap();
    assert_eq!(
        actual.finish(fallback(&source)).unwrap(),
        expected.finish(fallback(&source)).unwrap()
    );
}

#[test]
fn raise_diagnostics_precede_metadata_admission() {
    let source = source("#show z/0. 1{#true}1. p(2147483648).");
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let expected = raise_program(&parsed).diagnostics().to_vec();
    assert!(!expected.is_empty());
    let mut metadata = metadata::Builder::default();
    let error = raise(&parsed, &mut metadata).unwrap_err();
    let FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Raise(actual))) =
        error
    else {
        panic!("expected original raise diagnostics: {error}");
    };
    assert_eq!(actual, expected);
    assert_eq!(
        metadata.finish(fallback(&source)).unwrap(),
        metadata::Builder::default()
            .finish(fallback(&source))
            .unwrap()
    );
}

fn occurrence_raise_agrees(text: &str) {
    let Ok(parsed) = crate::ParsedSource::new(text.into(), crate::AdmissionOptions::default())
    else {
        return;
    };
    let expected = raise_program(parsed.parsed());
    let actual = themelios_program::raise::raise_occurrences(parsed.parsed()).into_raised();
    assert_eq!(actual.diagnostics(), expected.diagnostics());
    assert_eq!(actual.program(), expected.program());
    assert_eq!(
        format!("{:?}", actual.program()),
        format!("{:?}", expected.program()),
        "nested provenance: {text}"
    );
}

fn files_under(directory: &std::path::Path, paths: &mut Vec<std::path::PathBuf>) {
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
fn occurrence_raising_preserves_corpus_and_refusal_fixture_verdicts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    files_under(&root.join("../../validation/corpus/kr-domains"), &mut paths);
    files_under(&root.join("tests/fixtures"), &mut paths);
    paths.sort();
    let mut sources = 0;
    for path in paths {
        match path.extension().and_then(|value| value.to_str()) {
            Some("lp") => {
                occurrence_raise_agrees(&std::fs::read_to_string(path).unwrap());
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
                        occurrence_raise_agrees(source);
                        sources += 1;
                    }
                }
            }
            _ => {}
        }
    }
    assert!(sources >= 108);
    for text in crate::test_support::PREPARATION_CASES {
        occurrence_raise_agrees(text);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn generated_fact_and_comparison_sources_preserve_occurrence_raising(values in proptest::collection::vec(-20i32..20, 0..25)) {
        use std::fmt::Write;
        let mut text = String::new();
        for value in values {
            write!(&mut text, "p({value}).").unwrap();
        }
        text.push_str("q(X) :- p(X), X+1=2.");
        occurrence_raise_agrees(&text);
    }
}
