//! A syntax refusal retains enough information for a caller-owned source view.

use themelios_base::{diagnostic::ToDiagnostic, source::SourceId};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, SyntaxFailure, admit, admit_extended, prepare_formula,
};

const SOURCE: &str = "% original é\r\n{a,b :- q.\r\nq :- c.\r\n";

fn failures() -> [AdmissionFailure; 3] {
    let options = AdmissionOptions {
        source_id: SourceId::new(7),
        ..AdmissionOptions::default()
    };
    let direct = admit(SOURCE.to_owned(), options).unwrap_err();
    let ExpansionFailure::Admission(extended) =
        admit_extended(SOURCE.to_owned(), options, ExpansionLimits::default()).unwrap_err()
    else {
        panic!("malformed source must stop in syntax admission")
    };
    let FormulaFailure::Expansion(ExpansionFailure::Admission(formula)) = prepare_formula(
        SOURCE.to_owned(),
        options,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err() else {
        panic!("malformed source must stop before formula preparation")
    };
    [direct, extended, formula]
}

fn syntax(failure: &AdmissionFailure) -> &SyntaxFailure {
    let AdmissionFailure::Syntax(syntax) = failure else {
        panic!("expected original parser refusal")
    };
    syntax
}

#[test]
fn syntax_refusal_retains_original_source_bytes() {
    for failure in failures() {
        assert_eq!(syntax(&failure).source().text(), SOURCE);
    }
}

#[test]
fn syntax_labels_retain_the_callers_source_identity() {
    for failure in failures() {
        let syntax = syntax(&failure);
        assert_eq!(syntax.source().id(), SourceId::new(7));
        assert!(syntax.diagnostics().iter().all(|error| {
            error.to_diagnostic().primary().location.source == syntax.source().id()
        }));
    }
}

#[test]
fn syntax_view_resolves_the_retained_source() {
    for failure in failures() {
        let text = failure.to_string();
        assert_eq!(text.matches("error[syntax::").count(), 3, "{text}");
        assert!(text.contains("<input>:2:3"), "{text}");
        assert!(text.contains("2 | {a,b :- q."), "{text}");
        assert!(!text.contains('\u{1b}'));
    }
}

#[test]
fn typed_diagnostic_view_preserves_parser_order() {
    for failure in failures() {
        let originals: Vec<_> = syntax(&failure)
            .diagnostics()
            .iter()
            .map(ToDiagnostic::to_diagnostic)
            .collect();
        assert_eq!(failure.diagnostics(), originals);
    }
}

#[test]
fn syntax_refusal_exposes_its_error_cause() {
    for failure in failures() {
        let cause = std::error::Error::source(&failure).unwrap();
        assert!(cause.downcast_ref::<SyntaxFailure>().is_some());
    }
}
