//! Source and canonical formula doors preserve fact occurrences and evidence.

use std::sync::Arc;

use crate::support::root_emission::{self, Roots};
use zetesis_reference_support::{canonical, exhaustive_models};
use zetesis_themelios::GroundingPhase;
use zetesis_themelios::logical::{program::Program, raise::raise};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, ParsedSource,
    ProgramAdmissionOptions, admit_program_formula,
};

fn raised(text: &str) -> Program {
    let parsed = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let raised = raise(parsed.parsed());
    assert!(raised.diagnostics().is_empty());
    raised.into_program()
}

fn typed(logical: Arc<Program>) -> (AdmittedFormula, Roots) {
    root_emission::ground(
        zetesis_themelios::prepare_program_formula(
            logical,
            ProgramAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap(),
    )
}

#[test]
fn source_and_typed_doors_preserve_original_statement_order() {
    let text = "a(1). b(X) :- a(X). c(f(2)). {d(X)} :- b(X).";
    let logical = Arc::new(raised(text));
    let (source, source_roots) = root_emission::source(text);
    let (prepared, typed_roots) = typed(Arc::clone(&logical));
    assert!(std::ptr::eq(prepared.original_program(), logical.as_ref()));
    let expected = std::collections::BTreeSet::from([
        std::collections::BTreeSet::from([
            "a(1)".to_owned(),
            "b(1)".to_owned(),
            "c(f(2))".to_owned(),
        ]),
        std::collections::BTreeSet::from([
            "a(1)".to_owned(),
            "b(1)".to_owned(),
            "c(f(2))".to_owned(),
            "d(1)".to_owned(),
        ]),
    ]);
    for (input, roots) in [(&source, &source_roots), (&prepared, &typed_roots)] {
        let actual = exhaustive_models(input)
            .iter()
            .map(|model| model.atoms().iter().map(canonical).collect())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual, expected);
        let authored = roots.indices(GroundingPhase::RuleInstantiation);
        assert_eq!(authored.len(), 4);
        let sites = authored
            .iter()
            .map(|&index| &input.formula_origins()[index])
            .map(|sites| {
                assert_eq!(sites.len(), 1);
                sites[0].statement_id().unwrap().index()
            })
            .collect::<Vec<_>>();
        assert_eq!(sites, (0..logical.statements().count()).collect::<Vec<_>>());
    }
}

fn incoherent(text: &str) {
    let logical = Arc::new(raised(text));
    let (source, source_roots) = root_emission::source(text);
    let (prepared, typed_roots) = typed(Arc::clone(&logical));
    for (input, roots) in [(&source, &source_roots), (&prepared, &typed_roots)] {
        assert!(
            exhaustive_models(input).is_empty(),
            "opposite signed support: {text}"
        );
        let coherence = roots.indices(GroundingPhase::Coherence);
        let evidence = coherence
            .iter()
            .map(|&index| &input.formula_origins()[index])
            .collect::<Vec<_>>();
        assert_eq!(evidence.len(), 1, "one coherence constraint");
        assert_eq!(evidence[0].len(), 2, "both distinct original sites");
        assert_ne!(evidence[0][0].statement_id(), evidence[0][1].statement_id());
        for site in evidence[0] {
            let original = logical
                .statements()
                .nth(site.statement_id().unwrap().index())
                .unwrap();
            if input.source().is_none() {
                assert!(std::ptr::eq(
                    original,
                    input
                        .original_program()
                        .statements()
                        .nth(site.statement_id().unwrap().index())
                        .unwrap()
                ));
            } else {
                assert!(
                    input
                        .source()
                        .unwrap()
                        .slice(site.location().unwrap().span)
                        .is_ok()
                );
            }
        }
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
    let (source, source_roots) = root_emission::source(text);
    let (prepared, typed_roots) = typed(Arc::clone(&logical));
    for (input, roots) in [(&source, &source_roots), (&prepared, &typed_roots)] {
        assert_eq!(input.expansion_usage().templates, 6);
        let authored = roots.indices(GroundingPhase::RuleInstantiation);
        assert_eq!(
            authored.len(),
            8,
            "five fact occurrences and three grounded r rules"
        );
        let facts = authored
            .iter()
            .take(5)
            .map(|&index| {
                let atom = root_emission::fact_atom(input.theory(), input.theory().roots()[index]);
                canonical(input.atoms().at(atom).unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(facts, ["p(1)", "p(2)", "p(2)", "p(3)", "q(8)"]);
        for (index, sites) in authored
            .iter()
            .take(5)
            .map(|&index| &input.formula_origins()[index])
            .enumerate()
        {
            assert_eq!(sites.len(), 1);
            let expected = usize::from(index >= 4);
            assert_eq!(sites[0].statement_id().unwrap().index(), expected);
            if let Some(source) = input.source() {
                assert_eq!(
                    source.slice(sites[0].location().unwrap().span).unwrap(),
                    if index < 4 { "p(1..2;2..3)." } else { "q(8)." }
                );
            } else {
                assert!(std::ptr::eq(
                    input.original_program().statements().nth(expected).unwrap(),
                    logical.statements().nth(expected).unwrap()
                ));
            }
        }
        let models = exhaustive_models(input);
        assert_eq!(models.len(), 1);
        assert_eq!(
            models[0]
                .atoms()
                .iter()
                .map(canonical)
                .collect::<std::collections::BTreeSet<_>>(),
            ["p(1)", "p(2)", "p(3)", "q(8)", "r(1)", "r(2)", "r(3)"]
                .map(str::to_owned)
                .into()
        );
    }
}

#[test]
fn fact_admission_refusals_resolve_the_original_fact_evidence() {
    use zetesis_themelios::{FormulaFailure, FormulaResource, ProgramSubject, admit_formula};
    let text = "p(1). q(2,3).";
    let logical = Arc::new(raised(text));
    let limits = zetesis_core::AdmissionLimits {
        max_predicate_arity: 1,
        ..Default::default()
    };
    let source = admit_formula(
        text.into(),
        AdmissionOptions {
            core_limits: limits,
            ..Default::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    let typed = admit_program_formula(
        Arc::clone(&logical),
        ProgramAdmissionOptions {
            core_limits: limits,
            ..Default::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    for error in [&source, &typed] {
        assert!(
            matches!(error.cause(), FormulaFailure::Limit { resource: FormulaResource::Arity, limit: 1, observed: 2, location } if location.statement_id().unwrap().index() == 1)
        );
        assert_eq!(error.site().unwrap().statement_id().unwrap().index(), 1);
    }
    let location = source.site().unwrap().location().unwrap();
    assert_eq!(
        &text[usize::try_from(location.span.start().get()).unwrap()
            ..usize::try_from(location.span.end().get()).unwrap()],
        "q(2,3)."
    );
    let ProgramSubject::Statement(statement) = typed.subject().unwrap() else {
        panic!("original fact statement");
    };
    assert!(std::ptr::eq(
        statement,
        logical.statements().nth(1).unwrap()
    ));
}

#[test]
fn typed_rule_count_includes_fact_occurrences_before_later_rules() {
    use zetesis_themelios::{ExpansionFailure, ExpansionResource, FormulaFailure, ProgramSubject};
    let logical = Arc::new(raised("p(1). q(2). r(X) :- p(X)."));
    let options = ProgramAdmissionOptions {
        core_limits: zetesis_core::AdmissionLimits {
            max_templates: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let failure = admit_program_formula(
        Arc::clone(&logical),
        options,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        failure.cause(),
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            limit: 1,
            observed: 2,
            ..
        })
    ));
    let ProgramSubject::Statement(statement) = failure.subject().unwrap() else {
        panic!("second original rule");
    };
    assert!(std::ptr::eq(
        statement,
        logical.statements().nth(1).unwrap()
    ));
}
