//! Canonical completed results and explicit original contract checks.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::corpus_comparison::corpus::Case;

// A display is a multiset: an atom and a shown term can print the same symbol.
// Sorted vectors retain those occurrences while ignoring output order.
type Model = Vec<String>;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Answer {
    pub(crate) satisfiable: bool,
    pub(crate) cost: Option<Vec<i64>>,
    pub(crate) models: BTreeSet<Model>,
    /// Canonical displayed records and their multiplicities, after optN replay removal.
    pub(crate) model_multiplicities: Vec<(Model, u64)>,
    pub(crate) model_count: u64,
    pub(crate) solver: String,
    #[serde(skip)]
    pub(super) reported: crate::answers::ReportedAnswers,
}

pub(crate) fn reference(text: &str) -> Result<Answer, String> {
    crate::answers::clingo_json(
        text.as_bytes(),
        crate::answers::Limits::for_bytes(text.len()),
    )
    .map(adapt)
    .map_err(|error| error.to_string())
}

pub(crate) fn native(text: &str, optimized: bool) -> Result<Answer, crate::answers::Error> {
    crate::answers::native_text(
        text.as_bytes(),
        optimized,
        crate::answers::Limits::for_bytes(text.len()),
    )
    .map(adapt)
}

fn adapt(answer: crate::answers::ReportedAnswers) -> Answer {
    Answer {
        satisfiable: answer.satisfiable(),
        cost: answer.cost().map(<[i64]>::to_vec),
        models: answer
            .displays()
            .iter()
            .map(|(display, _)| display.clone())
            .collect(),
        model_multiplicities: answer.displays().to_vec(),
        model_count: answer.model_count(),
        solver: answer.solver().to_owned(),
        reported: answer,
    }
}

pub(crate) fn same(reference: &Answer, native: &Answer) -> bool {
    reference.satisfiable == native.satisfiable
        && reference.cost == native.cost
        && reference.models == native.models
        && reference.model_multiplicities == native.model_multiplicities
        && reference.model_count == native.model_count
}

fn integers(text: &str) -> Result<Vec<i64>, String> {
    crate::answers::parse_costs(text, crate::answers::Limits::for_bytes(text.len()))
        .map_err(|error| error.to_string())
}
fn split_atoms(text: &str, comma_separated: bool) -> Result<Vec<String>, String> {
    crate::answers::split_display(
        text,
        comma_separated,
        crate::answers::Limits::for_bytes(text.len()),
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn contracts(case: &Case, answer: &Answer) -> Result<(), String> {
    if let Some(contract) = &case.example_contract {
        return contract
            .check(&answer.reported)
            .map_err(|error| error.to_string());
    }
    if answer.satisfiable != (case.expected_satisfiability == "sat") {
        return Err("@expect mismatch".into());
    }
    for contract in &case.contracts {
        let argument = contract.arguments.trim();
        match contract.tag.as_str() {
            "expect" | "note" => (),
            "cost" => {
                if answer.cost.as_ref() != Some(&integers(braces(argument)?)?) {
                    return Err("@cost mismatch".into());
                }
            }
            "count" => {
                let expected = argument
                    .strip_prefix("optimal")
                    .unwrap_or(argument)
                    .trim()
                    .parse::<u64>()
                    .map_err(|error| error.to_string())?;
                if answer.model_count != expected {
                    return Err("@count mismatch".into());
                }
            }
            "model" | "optimal" => {
                if !answer
                    .models
                    .contains(&split_atoms(braces(argument)?, true)?)
                {
                    return Err(format!("@{} witness mismatch", contract.tag));
                }
            }
            "cautious" => {
                let expected = split_atoms(
                    braces(
                        argument
                            .strip_prefix("optimal")
                            .ok_or("unsupported cautious scope")?
                            .trim(),
                    )?,
                    true,
                )?;
                if answer.models.is_empty()
                    || !answer.models.iter().all(|model| {
                        expected
                            .iter()
                            .all(|atom| model.binary_search(atom).is_ok())
                    })
                {
                    return Err("@cautious optimal mismatch".into());
                }
            }
            unknown => return Err(format!("unimplemented contract tag @{unknown}")),
        }
    }
    Ok(())
}

fn braces(text: &str) -> Result<&str, String> {
    text.trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .map(str::trim)
        .ok_or_else(|| "contract needs a braced value".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{native, reference, same, split_atoms};

    #[test]
    fn quoted_strings_and_nested_tuples_remain_single_atoms() {
        let atoms = split_atoms(r#"p("a b,c",f(1,2)) q("a\" b")"#, false).unwrap();
        assert_eq!(atoms.len(), 2);
        assert!(atoms.iter().any(|atom| atom == r#"p("a b,c",f(1,2))"#));
        assert!(split_atoms("p(1", false).is_err());
        assert_eq!(
            split_atoms(r#"p("a b,c",f(1,2)), q("a\" b")"#, true).unwrap(),
            atoms
        );
    }

    #[test]
    fn optn_filters_incumbents_and_deduplicates_replayed_optimal_witness() {
        let output = r#"{"Solver":"clingo test","Call":[{"Witnesses":[{"Value":["a"],"Costs":[2]},{"Value":["b"],"Costs":[1]},{"Value":["b"],"Costs":[1]},{"Value":["c"],"Costs":[1]}]}],"Result":"OPTIMUM FOUND","Models":{"More":"no","Number":4,"Optimum":"yes","Optimal":2,"Costs":[1]}}"#;
        let answer = reference(output).unwrap();
        assert_eq!(answer.models.len(), 2);
        assert_eq!(answer.model_count, 2);
        assert_eq!(answer.cost, Some(vec![1]));
    }

    #[test]
    fn incomplete_and_missing_native_objectives_cannot_pass() {
        let output = "Answer: 1\na\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n";
        assert!(native(output, false).is_ok());
        assert!(native(output, true).is_err());
        assert!(native("SATISFIABLE\nCoverage: partial\n", false).is_err());
        assert!(native("UNSATISFIABLE\nCoverage: exhausted\nModels: 0\n", true).is_ok());
    }

    #[test]
    fn native_final_optimum_preserves_the_complete_cost_vector() {
        let output = "Answer: 1\nb\nOptimization: 1 7\nAnswer: 2\nc\nOptimization: 1 7\nOPTIMUM FOUND\nCoverage: exhausted\nModels: 2\n";
        let answer = native(output, true).unwrap();
        assert_eq!(answer.cost, Some(vec![1, 7]));
    }

    #[test]
    fn contradictory_native_summaries_and_unexpected_objectives_are_refused() {
        let good = "Answer: 1\na\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n";
        for bad in [
            good.replace(
                "Coverage: exhausted",
                "Coverage: partial\nCoverage: exhausted",
            ),
            good.replace("Models: 1", "Models: 2"),
            good.replace("SATISFIABLE", "SATISFIABLE\nSATISFIABLE"),
            good.replace("SATISFIABLE", "OPTIMUM FOUND"),
            good.replace("SATISFIABLE", "Optimization:\nSATISFIABLE"),
        ] {
            assert!(native(&bad, false).is_err(), "{bad}");
        }
    }

    #[test]
    fn reference_raw_count_and_unexpected_objective_records_are_refused() {
        let output = r#"{"Solver":"clingo test","Call":[{"Witnesses":[{"Value":["a"]}]}],"Result":"SATISFIABLE","Models":{"More":"no","Number":1}}"#;
        assert!(reference(output).is_ok());
        assert!(reference(&output.replace("\"Number\":1", "\"Number\":2")).is_err());
        assert!(
            reference(&output.replace("\"Value\":[\"a\"]", "\"Value\":[\"a\"],\"Costs\":[]"))
                .is_err()
        );
    }

    #[test]
    fn equal_display_sets_and_total_counts_do_not_hide_different_multiplicities() {
        let reference = reference(
            r#"{"Call":[{"Witnesses":[{"Value":["a"]},{"Value":["b"]},{"Value":["b"]}]}],"Result":"SATISFIABLE","Models":{"More":"no","Number":3}}"#,
        )
        .unwrap();
        let wrong = native(
            "Answer: 1\na\nAnswer: 2\na\nAnswer: 3\nb\nSATISFIABLE\nCoverage: exhausted\nModels: 3\n",
            false,
        )
        .unwrap();
        assert_eq!(reference.models, wrong.models);
        assert_eq!(reference.model_count, wrong.model_count);
        assert!(!same(&reference, &wrong));
        let reordered = native(
            "Answer: 1\nb\nAnswer: 2\na\nAnswer: 3\nb\nSATISFIABLE\nCoverage: exhausted\nModels: 3\n",
            false,
        )
        .unwrap();
        assert!(same(&reference, &reordered));
    }

    #[test]
    fn optimal_replay_removes_one_discovery_and_preserves_equal_empty_displays() {
        let reference = reference(
            r#"{"Call":[{"Witnesses":[{"Value":["worse"],"Costs":[2]},{"Value":[],"Costs":[1]},{"Value":[],"Costs":[1]},{"Value":[],"Costs":[1]},{"Value":["b"],"Costs":[1]}]}],"Result":"OPTIMUM FOUND","Models":{"More":"no","Number":5,"Optimum":"yes","Optimal":3,"Costs":[1]}}"#,
        )
        .unwrap();
        let native = native(
            "Answer: 1\n\nOptimization: 1\nAnswer: 2\nb\nOptimization: 1\nAnswer: 3\n\nOptimization: 1\nOPTIMUM FOUND\nCoverage: exhausted\nModels: 3\n",
            true,
        )
        .unwrap();
        assert_eq!(reference.model_count, 3);
        assert_eq!(reference.model_multiplicities[0].1, 2);
        assert!(same(&reference, &native));
    }

    #[test]
    fn duplicate_symbols_inside_one_display_are_preserved() {
        let expected = reference(
            r#"{"Call":[{"Witnesses":[{"Value":["b","a","a"]}]}],"Result":"SATISFIABLE","Models":{"More":"no","Number":1}}"#,
        )
        .unwrap();
        let actual = |display| {
            native(
                &format!("Answer: 1\n{display}\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n"),
                false,
            )
            .unwrap()
        };
        assert!(same(&expected, &actual("a b a")));
        assert!(!same(&expected, &actual("a b")));
        assert!(!same(&expected, &actual("a b b")));
        assert_eq!(
            split_atoms(r#"p("a b") p("a b")"#, false).unwrap(),
            vec![r#"p("a b")"#.to_owned(), r#"p("a b")"#.to_owned()]
        );
    }

    #[test]
    fn symbol_multiplicity_distinguishes_optimal_displays_and_incumbent_replay() {
        let output = r#"{"Call":[{"Witnesses":[{"Value":["a","a"],"Costs":[1]},{"Value":["a"],"Costs":[1]},{"Value":["a","a"],"Costs":[1]}]}],"Result":"OPTIMUM FOUND","Models":{"More":"no","Number":3,"Optimum":"yes","Optimal":2,"Costs":[1]}}"#;
        let expected = reference(output).unwrap();
        let good = native(
            "Answer: 1\na a\nOptimization: 1\nAnswer: 2\na\nOptimization: 1\nOPTIMUM FOUND\nCoverage: exhausted\nModels: 2\n",
            true,
        )
        .unwrap();
        assert!(same(&expected, &good));
        assert_eq!(expected.models.len(), 2);
        assert_eq!(expected.model_count, 2);
        let missing_replay = output.replace(
            "{\"Value\":[\"a\",\"a\"],\"Costs\":[1]}]}",
            "{\"Value\":[\"a\"],\"Costs\":[1]}]}",
        );
        assert!(reference(&missing_replay).is_err());
    }

    #[test]
    fn malformed_calls_and_nonoptimal_witnesses_after_the_final_incumbent_are_refused() {
        for calls in ["[null]", "[{\"Witnesses\":null}]"] {
            let output = format!(
                r#"{{"Call":{calls},"Result":"UNSATISFIABLE","Models":{{"More":"no","Number":0}}}}"#,
            );
            assert!(reference(&output).is_err());
        }
        let output = r#"{"Call":[{"Witnesses":[{"Value":["a"],"Costs":[1]},{"Value":["b"],"Costs":[2]},{"Value":["a"],"Costs":[1]}]}],"Result":"OPTIMUM FOUND","Models":{"More":"no","Number":3,"Optimum":"yes","Optimal":1,"Costs":[1]}}"#;
        assert!(reference(output).is_err());
        let missing_replay = r#"{"Call":[{"Witnesses":[{"Value":["a"],"Costs":[1]},{"Value":["b"],"Costs":[1]}]}],"Result":"OPTIMUM FOUND","Models":{"More":"no","Number":2,"Optimum":"yes","Optimal":1,"Costs":[1]}}"#;
        assert!(reference(missing_replay).is_err());
        let malformed_incumbent = r#"{"Call":[{"Witnesses":[{"Value":null,"Costs":[2]},{"Value":["a"],"Costs":[1]},{"Value":["a"],"Costs":[1]}]}],"Result":"OPTIMUM FOUND","Models":{"More":"no","Number":3,"Optimum":"yes","Optimal":1,"Costs":[1]}}"#;
        assert!(reference(malformed_incumbent).is_err());
    }
}
