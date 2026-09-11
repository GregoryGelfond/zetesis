//! Aggregate-head permission is independent of the bound's contribution.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;

use zetesis_themelios::FormulaLimits;

#[test]
fn every_positive_head_supplies_objective_eligibility() {
    for case in source_cases::cases(include_str!("fixtures/objective-head-producers.jsonl")) {
        let program = &case.source;
        let original = source_records::admit(program, &FormulaLimits::default()).unwrap();
        for (condition, enabled) in [("a", true), ("not a", false)] {
            let source = format!("{program}#minimize{{1@0:{condition}}}.");
            let observed = source_records::admit(&source, &FormulaLimits::default()).unwrap();
            assert_eq!(observed.objectives().priorities(), &[0]);
            let expected: source_records::Records = case
                .records
                .iter()
                .map(|(model, _)| {
                    let cost = i64::from(model.contains("a") == enabled);
                    (model.clone(), Some(vec![cost]))
                })
                .collect();
            assert_eq!(
                source_records::exhaustive(&observed),
                expected,
                "{}: {condition}",
                case.name
            );
            assert_eq!(observed.atoms(), original.atoms());
            assert_eq!(observed.theory().nodes(), original.theory().nodes());
            assert_eq!(observed.theory().roots(), original.theory().roots());
            assert_eq!(observed.formula_origins(), original.formula_origins());
        }
    }
}
