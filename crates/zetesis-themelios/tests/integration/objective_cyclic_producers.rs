//! Finite ordinary cyclic producers retain sound possible source rows.

use crate::support::source_cases;

use zetesis_clingo_support as oracle;
use zetesis_reference_support as reference;
use zetesis_themelios::FormulaLimits;

const CASES: &str = include_str!("../fixtures/objective-cyclic-producers.jsonl");

#[test]
fn cyclic_producers_preserve_full_scored_answers() {
    let cases = source_cases::cases(CASES);
    assert_eq!(cases.len(), 15);
    for (case, row) in cases.into_iter().zip(CASES.lines()) {
        let expected: serde_json::Value = serde_json::from_str(row).unwrap();
        let input = reference::admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(reference::exhaustive(&input), case.records, "{}", case.name);
        assert_eq!(
            serde_json::to_value(input.objectives().priorities()).unwrap(),
            expected["priorities"],
            "{}",
            case.name
        );
    }
}

#[test]
fn cyclic_producers_keeps_the_original_reduct_subject() {
    for case in source_cases::cases(CASES) {
        let program = case
            .source
            .split("#minimize")
            .next()
            .unwrap()
            .split("#maximize")
            .next()
            .unwrap()
            .split(":~")
            .next()
            .unwrap();
        let original = reference::admit(program, &FormulaLimits::default()).unwrap();
        let observed = reference::admit(&case.source, &FormulaLimits::default()).unwrap();
        assert_eq!(original.atoms(), observed.atoms(), "{}", case.name);
        assert_eq!(
            (original.theory().nodes(), original.theory().operands()),
            (observed.theory().nodes(), observed.theory().operands()),
            "{}",
            case.name
        );
        assert_eq!(
            original.theory().roots(),
            observed.theory().roots(),
            "{}",
            case.name
        );
        assert_eq!(
            original.formula_origins(),
            observed.formula_origins(),
            "{}",
            case.name
        );
    }
}

#[test]
#[ignore = "requires clingo: cyclic producers match fresh clingo; 15 original cyclic sources"]
fn cyclic_producers_match_fresh_clingo() {
    for case in source_cases::cases(CASES) {
        assert_eq!(oracle::records(&case.source), case.records, "{}", case.name);
    }
}

#[test]
fn unproductive_aggregate_cycle_retains_only_the_negative_cost() {
    let source = "n(N):-a,N=#count{1:a}.a:-n(0).#minimize{1:not a}.";
    let input = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(input.objectives().priorities(), [0]);
    assert_eq!(
        reference::exhaustive(&input),
        [(std::collections::BTreeSet::new(), Some(vec![1]))].into()
    );
}
