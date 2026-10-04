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
    metadata::collect_profile(&program, &mut expected, true).unwrap();
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
