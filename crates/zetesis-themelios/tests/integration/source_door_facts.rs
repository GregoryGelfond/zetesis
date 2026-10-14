//! Every relational admission door publishes the same ordered fact representation.

use std::sync::Arc;

use zetesis_core::{AdmissionError, AdmissionResource, Program as NativeProgram, RuleRef};
use zetesis_reference_support::{canonical, normal_answer_sets, resource_limits};
use zetesis_test_support::programs::rules;
use zetesis_themelios::logical::{program::Program, raise::raise};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, ParsedSource,
    ProgramAdmissionOptions, ProgramFailureKind, ProgramRelationalOptions, ProgramSubject, admit,
    admit_extended, admit_program, prepare_program_relational,
};

fn raised(text: &str) -> Program {
    let parsed = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let raised = raise(parsed.parsed());
    assert!(raised.diagnostics().is_empty());
    raised.into_program()
}

fn same_rules(actual: &NativeProgram, expected: &NativeProgram) {
    assert_eq!(actual.facts().len(), expected.facts().len());
    assert_eq!(actual.templates().len(), expected.templates().len());
    assert_eq!(
        rules(actual).collect::<Vec<_>>(),
        rules(expected).collect::<Vec<_>>()
    );
    assert_eq!(
        actual
            .rules()
            .map(|rule| matches!(rule, RuleRef::Fact(_)))
            .collect::<Vec<_>>(),
        expected
            .rules()
            .map(|rule| matches!(rule, RuleRef::Fact(_)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn four_doors_publish_the_same_facts_templates_and_source_order() {
    let text = "a(1). b(X) :- a(X). c(f(2)). {d(X)} :- b(X).";
    let logical = Arc::new(raised(text));
    let strict = admit(text.into(), AdmissionOptions::default()).unwrap();
    let extended = admit_extended(
        text.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let typed = admit_program(&logical, ProgramAdmissionOptions::default()).unwrap();
    let prepared =
        prepare_program_relational(Arc::clone(&logical), ProgramRelationalOptions::default())
            .unwrap();
    assert_eq!(strict.program().facts().len(), 2);
    assert_eq!(strict.program().templates().len(), 2);
    for program in [extended.program(), typed.program(), prepared.program()] {
        same_rules(program, strict.program());
    }
    assert_eq!(normal_answer_sets(strict.program()).len(), 2);
    assert_eq!(strict.fact_origins().len(), 2);
    assert_eq!(extended.fact_origins().len(), 2);
    assert_eq!(typed.fact_statements().len(), 2);
    assert_eq!(prepared.fact_sites().len(), 2);
    // The mixed traversal must retain each original statement position, not
    // derive it from the compressed template-only or fact-only index.
    let sites = prepared
        .program()
        .rules()
        .map(|rule| match rule {
            RuleRef::Template { index, .. } => &prepared.template_sites()[index],
            RuleRef::Fact(fact) => &prepared.fact_sites()[fact.index()],
        })
        .map(|sites| sites[0].statement_id().unwrap().index())
        .collect::<Vec<_>>();
    assert_eq!(sites, (0..logical.statements().count()).collect::<Vec<_>>());
    for evidence in typed
        .fact_statements()
        .iter()
        .chain(typed.template_statements())
    {
        assert_eq!(evidence.len(), 1);
        assert!(
            logical
                .statements()
                .any(|original| std::ptr::eq(original, evidence[0]))
        );
    }
}

fn incoherent(text: &str) {
    let logical = Arc::new(raised(text));
    let strict = admit(text.into(), AdmissionOptions::default()).unwrap();
    let extended = admit_extended(
        text.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let typed = admit_program(&logical, ProgramAdmissionOptions::default()).unwrap();
    let prepared =
        prepare_program_relational(Arc::clone(&logical), ProgramRelationalOptions::default())
            .unwrap();
    for program in [
        strict.program(),
        extended.program(),
        typed.program(),
        prepared.program(),
    ] {
        same_rules(program, strict.program());
        let constraints = program
            .templates()
            .iter()
            .filter(|template| template.head().is_none())
            .collect::<Vec<_>>();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].positive().len(), 2);
        assert!(
            normal_answer_sets(program).is_empty(),
            "opposite signed support: {text}"
        );
    }
    for lengths in [
        strict.template_origins().last().unwrap().len(),
        extended.template_origins().last().unwrap().len(),
        typed.template_statements().last().unwrap().len(),
        prepared.template_sites().last().unwrap().len(),
    ] {
        assert_eq!(lengths, 2, "coherence retains both signature origins");
    }
    let evidence = typed.template_statements().last().unwrap();
    assert_ne!(evidence[0], evidence[1]);
    for &statement in evidence {
        assert!(
            logical
                .statements()
                .any(|original| std::ptr::eq(original, statement))
        );
    }
}

#[test]
fn coherence_reads_opposite_signs_from_two_fact_blocks() {
    incoherent("p(1). -p(1).");
}

#[test]
fn coherence_reads_negative_facts_and_positive_templates() {
    incoherent("-p(1). p(X) :- q(X). q(1).");
}

#[test]
fn coherence_reads_positive_facts_and_negative_templates() {
    incoherent("p(1). -p(X) :- q(X). q(1).");
}

#[test]
fn overlapping_intervals_keep_duplicate_occurrences_and_family_evidence() {
    let text = "p(1..2;2..3). q(8). r(X) :- p(X).";
    let logical = Arc::new(raised(text));
    let extended = admit_extended(
        text.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let prepared =
        prepare_program_relational(Arc::clone(&logical), ProgramRelationalOptions::default())
            .unwrap();
    same_rules(prepared.program(), extended.program());
    assert_eq!(extended.program().templates().len(), 1);
    let facts = extended
        .program()
        .facts()
        .into_iter()
        .map(|fact| canonical(fact.atom()))
        .collect::<Vec<_>>();
    assert_eq!(facts, ["p(1)", "p(2)", "p(2)", "p(3)", "q(8)"]);
    assert_eq!(extended.expansion_usage().templates, 6);
    assert_eq!(extended.fact_origins().len(), 5);
    assert_eq!(prepared.fact_sites().len(), 5);
    for (index, origins) in extended.fact_origins().iter().enumerate() {
        assert_eq!(origins.len(), 1);
        let expected = if index < 4 { "p(1..2;2..3)." } else { "q(8)." };
        assert_eq!(extended.source().slice(origins[0].span).unwrap(), expected);
    }
    let original = logical.statements().collect::<Vec<_>>();
    for sites in prepared.fact_sites().iter().take(4) {
        assert_eq!(sites.len(), 1);
        assert!(std::ptr::eq(
            original[sites[0].statement_id().unwrap().index()],
            original[0]
        ));
    }
    assert_eq!(normal_answer_sets(extended.program()).len(), 1);
}

fn arity_failure(error: &AdmissionError) {
    assert!(matches!(
        error,
        AdmissionError::FactLimitExceeded {
            resource: AdmissionResource::PredicateArity,
            limit: 1,
            actual: 2,
            fact: 1,
        }
    ));
    assert_eq!(error.fact_index(), Some(1));
    assert_eq!(error.template_index(), None);
}

#[test]
fn fact_admission_refusals_resolve_the_original_fact_evidence() {
    let text = "p(1). q(2,3).";
    let parsed = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let logical = Arc::new(raised(text));
    let options = resource_limits::admission_core(AdmissionResource::PredicateArity, 1);
    let strict = admit(text.into(), options).unwrap_err();
    let extended = admit_extended(text.into(), options, ExpansionLimits::default()).unwrap_err();
    let ExpansionFailure::Admission(extended) = extended else {
        panic!("core fact refusal");
    };
    for failure in [strict, extended] {
        let AdmissionFailure::Core { error, location } = failure else {
            panic!("core fact refusal");
        };
        arity_failure(&error);
        assert_eq!(
            parsed
                .source()
                .slice(location.location().unwrap().span)
                .unwrap(),
            "q(2,3)."
        );
    }
    let options = resource_limits::program_core(AdmissionResource::PredicateArity, 1);
    let failure = admit_program(&logical, options).unwrap_err();
    let ProgramFailureKind::Core(error) = failure.kind else {
        panic!("typed core fact refusal");
    };
    arity_failure(&error);
    let ProgramSubject::Statement(statement) = failure.subject else {
        panic!("original fact statement");
    };
    assert!(std::ptr::eq(
        statement,
        logical.statements().nth(1).unwrap()
    ));
    let failure = prepare_program_relational(
        Arc::clone(&logical),
        ProgramRelationalOptions {
            admission: options,
            ..ProgramRelationalOptions::default()
        },
    )
    .unwrap_err();
    let Some(ProgramSubject::Statement(statement)) = failure.subject() else {
        panic!("original fact statement");
    };
    assert!(std::ptr::eq(
        statement,
        logical.statements().nth(1).unwrap()
    ));
}

#[test]
fn typed_rule_count_includes_fact_occurrences_before_later_rules() {
    let logical = raised("p(1). q(2). r(X) :- p(X).");
    let original = logical.statements().collect::<Vec<_>>();
    let options = resource_limits::program_core(AdmissionResource::Templates, 1);
    let failure = admit_program(&logical, options).unwrap_err();
    assert!(matches!(
        failure.kind,
        ProgramFailureKind::Core(AdmissionError::LimitExceeded {
            resource: AdmissionResource::Templates,
            limit: 1,
            actual: 2,
            template: None,
        })
    ));
    let ProgramSubject::Statement(statement) = failure.subject else {
        panic!("second original rule");
    };
    assert!(std::ptr::eq(statement, original[1]));
}
