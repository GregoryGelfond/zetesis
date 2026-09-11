//! Fresh unchanged-source corroboration of complete families and optimum ties.

use super::super::clingo_report;
use super::{AnswerSelection, CASES};

#[test]
#[ignore = "requires independently installed clingo"]
fn original_count_dependencies_match_clingo_families() {
    for case in CASES {
        for selection in [AnswerSelection::All, AnswerSelection::Optimal] {
            let actual = clingo_report::complete(case.source, selection);
            let records = case.records(selection);
            // These exact original sources contain no projection directives:
            // every reference witness symbol is part of the full interpretation.
            let best = case.models.iter().map(|&(_, _, cost)| cost).min();
            let mut models: Vec<_> = case
                .models
                .iter()
                .filter(|&&(_, _, cost)| selection == AnswerSelection::All || Some(cost) == best)
                .map(|&(selected, value, _)| {
                    let mut symbols: Vec<_> = selected
                        .iter()
                        .map(|name| (*name).to_owned())
                        .chain([format!("n({value})"), format!("p({value})")])
                        .collect();
                    symbols.sort();
                    (symbols, 1)
                })
                .collect();
            models.sort();
            assert_eq!(actual.displays(), models, "{}", case.source);
            assert_eq!(actual.model_count(), u64::try_from(records.len()).unwrap());
            assert_eq!(actual.satisfiable(), !records.is_empty());
            let expected_cost = (selection == AnswerSelection::Optimal)
                .then(|| {
                    records.first().map(|(_, costs)| {
                        costs
                            .as_ref()
                            .unwrap()
                            .iter()
                            .map(|&(_, cost)| cost)
                            .collect::<Vec<_>>()
                    })
                })
                .flatten();
            assert_eq!(actual.cost(), expected_cost.as_deref());
        }
    }
}
