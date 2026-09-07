//! Upstream semantic gaps must not be mislabeled as frontend parsing failures.

#[path = "support/upstream.rs"]
mod upstream;
use themelios_base::source::{Source, SourceId};
use themelios_syntax::{dialect::Dialect, parse::parse};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[test]
fn every_selected_upstream_program_admits() {
    let cases = upstream::corpus().cases();
    assert_eq!(cases.len(), 24);
    for case in cases {
        admit_formula(
            case.source().to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.id()));
    }
}

#[test]
fn every_selected_upstream_program_parses_cleanly() {
    let cases = upstream::corpus().cases();
    assert_eq!(cases.len(), 24);
    for case in cases {
        let source = Source::new(SourceId::new(0), case.source().to_owned()).unwrap();
        let parsed = parse(&source, Dialect::Clingo);
        assert!(
            parsed.diagnostics().is_empty(),
            "{}: {:?}",
            case.id(),
            parsed.diagnostics()
        );
    }
}

#[test]
fn every_selected_upstream_program_raises_cleanly() {
    let cases = upstream::corpus().cases();
    assert_eq!(cases.len(), 24);
    for case in cases {
        let source = Source::new(SourceId::new(0), case.source().to_owned()).unwrap();
        let parsed = parse(&source, Dialect::Clingo);
        let raised = themelios_program::raise::raise(&parsed);
        assert!(
            raised.diagnostics().is_empty(),
            "{}: {:?}",
            case.id(),
            raised.diagnostics()
        );
    }
}
