//! Mixed objective directions preserve original theories and complete costs.

use std::collections::BTreeSet;

use themelios_base::source::SourceId;
use themelios_program::program::{Direction, Statement};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit_formula,
};

#[path = "support/source_records.rs"]
mod source_records;

fn input(source: &str) -> AdmittedFormula {
    source_records::admit(source, &FormulaLimits::default())
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn cases() -> Vec<source_records::Case> {
    source_records::cases(include_str!("fixtures/maximize.jsonl"))
}

#[test]
fn complete_mixed_direction_costs_match_independent_recorded_models() {
    let cases = cases();
    assert_eq!(cases.len(), 242);
    let mut count = 0;
    for case in cases {
        count += case.records.len();
        assert_eq!(
            source_records::exhaustive(&input(&case.source)),
            case.records,
            "{}: {}",
            case.name,
            case.source
        );
    }
    assert_eq!(count, 883);
}

#[test]
fn objective_directions_leave_the_original_atom_catalog_and_reduct_dag_unchanged() {
    for base in ["{a;b}.", "{a}. b:-a. a:-b.", "{a}. :-a."] {
        let original = input(base);
        for objectives in [
            "#maximize{2@1,k:a}.",
            "#maximize{2@1,k:a}. #minimize{-2@1,k:a}.",
            "#maximize{-2@1,k:a}. :~a.[-2@1,k]",
        ] {
            let optimized = input(&format!("{base} {objectives}"));
            assert_eq!(optimized.atoms(), original.atoms());
            assert_eq!(optimized.theory().nodes(), original.theory().nodes());
            assert_eq!(optimized.theory().roots(), original.theory().roots());
            let models = |program: &AdmittedFormula| {
                source_records::exhaustive(program)
                    .into_iter()
                    .map(|(model, _)| model)
                    .collect::<BTreeSet<_>>()
            };
            assert_eq!(models(&optimized), models(&original));
        }
    }
}

#[test]
fn maximizing_source_direction_and_original_provenance_are_retained() {
    let source = "v(-2). v(2). {s(W):v(W)}. #maximize{W@7,k:s(W),W!=0}.";
    let input = admit_formula(
        source.into(),
        AdmissionOptions {
            source_id: SourceId::new(77),
            ..AdmissionOptions::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(input.source().text(), source);
    assert_eq!(input.objective_declarations().len(), 1);
    assert!(input.analyzed_program().statements().any(|statement| {
        matches!(statement.get(), Statement::Optimize(objective)
            if objective.direction == Direction::Maximize)
    }));
    let declared = input.objective_declarations()[0];
    assert_eq!(declared.source, SourceId::new(77));
    assert!(
        input
            .source()
            .slice(declared.span)
            .unwrap()
            .starts_with("#maximize")
    );
    let fragments: BTreeSet<_> = input
        .objective_origins()
        .iter()
        .flatten()
        .map(|location| {
            assert_eq!(location.source, SourceId::new(77));
            input.source().slice(location.span).unwrap()
        })
        .collect();
    assert!(fragments.iter().any(|fragment| fragment.contains("s(W)")));
    assert!(fragments.iter().any(|fragment| fragment.contains("W!=0")));
}

#[test]
fn eligible_minimum_integer_is_a_located_compatibility_refusal() {
    for source in [
        "{a}. #maximize{(-2147483647-1)@1,k:a}.",
        "a. #maximize{(-2147483647-1)@1,k:a}.",
        "v(-2147483647-1).v(1). #maximize{W@1,k:v(W)}.",
    ] {
        let error = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::NumericOverflow,
                    ..
                }))
            ),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn minimum_integer_literal_retains_the_pinned_frontends_located_refusal() {
    // An absent or filtered objective does not bypass source raising. Arithmetic
    // expressions reaching the same integer are covered separately above and in
    // the complete model/cost fixture; the literal itself is outside this pin.
    for source in [
        "#maximize{-2147483648@1,k:absent}.",
        "v(-2147483648). #maximize{W@1,k:v(W),W!=(-2147483647-1)}.",
    ] {
        let error = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Raise(_)))
            ),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn maximizing_occurrences_obey_the_original_objective_element_ceiling() {
    for source in [
        "{a}. #maximize{2@1,k:a}. #maximize{2@1,k:a}.",
        "{a}. #maximize{2@1,k:a}. #minimize{-2@1,k:a}.",
    ] {
        let result = source_records::admit(
            source,
            &FormulaLimits {
                objective: zetesis_objective::AdmissionLimits {
                    max_templates: 1,
                    ..zetesis_objective::AdmissionLimits::default()
                },
                ..FormulaLimits::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::ObjectiveElements,
                    observed: 2,
                    ..
                })
            ),
            "{source}: {result:?}"
        );
    }
}

#[test]
#[ignore = "requires independent clingo; 242 bounded complete model/cost cases"]
fn fresh_clingo_confirms_every_recorded_mixed_direction_contract() {
    for case in cases() {
        assert_eq!(
            source_records::clingo(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}
