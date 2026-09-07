//! Canonical completed results and explicit original contract checks.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::corpus::Case;

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
}

pub(crate) fn reference(text: &str) -> Result<Answer, String> {
    let document: Value =
        serde_json::from_str(text).map_err(|error| format!("clingo JSON: {error}"))?;
    let result = document["Result"].as_str().ok_or("missing clingo Result")?;
    if !matches!(result, "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND")
        || document["Models"]["More"] != "no"
    {
        return Err("reference did not complete enumeration/optimization".into());
    }
    let optimum = result == "OPTIMUM FOUND";
    if optimum && document["Models"]["Optimum"] != "yes" {
        return Err("reference lacks completed optimality evidence".into());
    }
    let cost = if optimum {
        Some(costs(&document["Models"]["Costs"])?)
    } else {
        None
    };
    if !optimum && document["Models"].get("Costs").is_some() {
        return Err("nonoptimized reference result contains objective costs".into());
    }
    let mut selected = Vec::new();
    let mut raw_count = 0u64;
    let mut best_count = 0u64;
    for call in document["Call"].as_array().ok_or("missing clingo calls")? {
        let call = call.as_object().ok_or("invalid clingo call")?;
        let Some(witnesses) = call.get("Witnesses") else {
            continue;
        };
        let witnesses = witnesses.as_array().ok_or("invalid clingo witnesses")?;
        for witness in witnesses {
            raw_count = raw_count.checked_add(1).ok_or("witness count overflow")?;
            let model = witness_symbols(&witness["Value"])?;
            if let Some(best) = &cost {
                let actual = costs(&witness["Costs"])?;
                if actual.len() != best.len() || actual < *best {
                    return Err("witness contradicts the reported final cost vector".into());
                }
                if actual != *best {
                    if best_count != 0 {
                        return Err("nonoptimal witness follows the final incumbent".into());
                    }
                    continue;
                }
            } else if witness.get("Costs").is_some() {
                return Err("unexpected objective vector in nonoptimized witness".into());
            }
            best_count = best_count.checked_add(1).ok_or("witness count overflow")?;
            selected.push(model);
        }
    }
    let satisfiable = result != "UNSATISFIABLE";
    let count_key = if optimum { "Optimal" } else { "Number" };
    let model_count = document["Models"][count_key]
        .as_u64()
        .ok_or("missing completed model count")?;
    if document["Models"]["Number"].as_u64() != Some(raw_count) {
        return Err("reference witness count disagrees with Models.Number".into());
    }
    if optimum && model_count.checked_add(1) != Some(best_count) {
        return Err(
            "reference optN requires final optimal witnesses plus one incumbent replay".into(),
        );
    }
    if optimum {
        let (incumbent, optimal) = selected.split_first().ok_or("missing final incumbent")?;
        if !optimal.contains(incumbent) {
            return Err("final incumbent is absent from optimal enumeration".into());
        }
    }
    // optN first discovers the final incumbent, then enumerates every optimal
    // model. Remove precisely that first discovery, never every equal display.
    let model_multiplicities = multiplicities(selected.into_iter().skip(usize::from(optimum)))?;
    let models: BTreeSet<_> = model_multiplicities
        .iter()
        .map(|(model, _)| model.clone())
        .collect();
    if satisfiable == models.is_empty()
        || (satisfiable && model_count == 0)
        || (!satisfiable && model_count != 0)
    {
        return Err("inconsistent clingo status, witnesses, or counts".into());
    }
    Ok(Answer {
        satisfiable,
        cost,
        models,
        model_multiplicities,
        model_count,
        solver: document["Solver"]
            .as_str()
            .unwrap_or("unreported")
            .to_owned(),
    })
}

fn witness_symbols(value: &Value) -> Result<Model, String> {
    let mut symbols = value
        .as_array()
        .ok_or("missing witness atoms")?
        .iter()
        .map(|atom| {
            atom.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "non-string witness atom".to_owned())
        })
        .collect::<Result<Model, _>>()?;
    symbols.sort_unstable();
    Ok(symbols)
}

fn costs(value: &Value) -> Result<Vec<i64>, String> {
    value
        .as_array()
        .ok_or_else(|| "missing cost vector".to_owned())?
        .iter()
        .map(|cost| {
            cost.as_i64()
                .ok_or_else(|| "cost outside signed 64-bit report representation".to_owned())
        })
        .collect()
}

pub(crate) fn native(text: &str, optimized: bool) -> Result<Answer, String> {
    let (satisfiable, reported_count) = native_summary(text, optimized)?;
    let mut witnesses: Vec<(Model, Option<Vec<i64>>)> = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            let model = split_atoms(lines.next().ok_or("missing native model line")?, false)?;
            witnesses.push((model, None));
        } else if let Some(raw) = line.strip_prefix("Optimization:") {
            if !optimized {
                return Err("unexpected objective vector in nonoptimized native output".into());
            }
            let cost = integers(raw)?;
            let witness = witnesses
                .last_mut()
                .ok_or("cost without preceding native model")?;
            if witness.1.replace(cost).is_some() {
                return Err("duplicate native cost record".into());
            }
        }
    }
    if satisfiable == witnesses.is_empty() {
        return Err("native model/status mismatch".into());
    }
    if u64::try_from(witnesses.len()).map_err(|error| error.to_string())? != reported_count {
        return Err("native answer count disagrees with Models summary".into());
    }
    let cost = if optimized && satisfiable {
        if witnesses.iter().any(|(_, cost)| cost.is_none()) {
            return Err(
                "native optimized output requires an Optimization: vector for every model".into(),
            );
        }
        let dimensions = witnesses[0].1.as_ref().map(Vec::len);
        if witnesses
            .iter()
            .any(|(_, cost)| cost.as_ref().map(Vec::len) != dimensions)
        {
            return Err("native objective vectors have inconsistent dimensions".into());
        }
        witnesses
            .iter()
            .filter_map(|(_, cost)| cost.as_ref())
            .min()
            .cloned()
    } else {
        None
    };
    let selected: Vec<_> = witnesses
        .into_iter()
        .filter(|(_, value)| cost.is_none() || value == &cost)
        .collect();
    let model_count = u64::try_from(selected.len()).map_err(|error| error.to_string())?;
    let model_multiplicities = multiplicities(selected.into_iter().map(|(model, _)| model))?;
    Ok(Answer {
        satisfiable,
        cost,
        models: model_multiplicities
            .iter()
            .map(|(model, _)| model.clone())
            .collect(),
        model_multiplicities,
        model_count,
        solver: "zetesis native output".into(),
    })
}

fn multiplicities(models: impl Iterator<Item = Model>) -> Result<Vec<(Model, u64)>, String> {
    let mut counts = BTreeMap::<Model, u64>::new();
    for model in models {
        let count = counts.entry(model).or_default();
        *count = count
            .checked_add(1)
            .ok_or("display multiplicity overflow")?;
    }
    Ok(counts.into_iter().collect())
}

fn native_summary(text: &str, optimized: bool) -> Result<(bool, u64), String> {
    let coverage: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("Coverage:"))
        .collect();
    if coverage != ["Coverage: exhausted"]
        || text.lines().any(|line| line.starts_with("INCOMPLETE:"))
    {
        return Err("native output requires exactly one exhausted coverage record".into());
    }
    let statuses: Vec<_> = text
        .lines()
        .filter(|line| matches!(*line, "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND"))
        .collect();
    if statuses.len() != 1 || (!optimized && statuses[0] == "OPTIMUM FOUND") {
        return Err("native status is missing, contradictory, or unexpectedly optimized".into());
    }
    let summaries: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Models:"))
        .collect();
    if summaries.len() != 1 {
        return Err("native output requires exactly one Models summary".into());
    }
    let count = summaries[0]
        .split(';')
        .next()
        .ok_or("missing Models count")?
        .trim()
        .parse()
        .map_err(|error| format!("invalid Models count: {error}"))?;
    Ok((statuses[0] != "UNSATISFIABLE", count))
}

pub(crate) fn same(reference: &Answer, native: &Answer) -> bool {
    reference.satisfiable == native.satisfiable
        && reference.cost == native.cost
        && reference.models == native.models
        && reference.model_multiplicities == native.model_multiplicities
        && reference.model_count == native.model_count
}

pub(crate) fn contracts(case: &Case, answer: &Answer) -> Result<(), String> {
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

fn integers(text: &str) -> Result<Vec<i64>, String> {
    text.split_whitespace()
        .map(|value| {
            value
                .parse()
                .map_err(|error| format!("cost integer: {error}"))
        })
        .collect()
}

fn split_atoms(text: &str, comma_separated: bool) -> Result<Model, String> {
    let mut result = Model::new();
    let mut atom = String::new();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for character in text.chars() {
        if quoted {
            atom.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
            atom.push(character);
        } else if character == '(' {
            depth = depth.checked_add(1).ok_or("atom nesting overflow")?;
            atom.push(character);
        } else if character == ')' {
            depth = depth.checked_sub(1).ok_or("unmatched atom parenthesis")?;
            atom.push(character);
        } else if depth == 0 && (character.is_whitespace() || (comma_separated && character == ','))
        {
            if !atom.is_empty() {
                result.push(std::mem::take(&mut atom));
            }
        } else {
            atom.push(character);
        }
    }
    if quoted || depth != 0 {
        return Err("unterminated quoted atom or tuple".into());
    }
    if !atom.is_empty() {
        result.push(atom);
    }
    result.sort_unstable();
    Ok(result)
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
    fn native_complete_weighted_enumeration_selects_exact_best_vector() {
        let output = "Answer: 1\na\nOptimization: 2 0\nAnswer: 2\nb\nOptimization: 1 7\nAnswer: 3\nc\nOptimization: 1 7\nSATISFIABLE\nCoverage: exhausted\nModels: 3\n";
        let answer = native(output, true).unwrap();
        assert_eq!(answer.cost, Some(vec![1, 7]));
        assert_eq!(answer.model_count, 2);
        for replacement in ["Optimization: 2", "Optimization:"] {
            let malformed = output.replace("Optimization: 2 0", replacement);
            assert!(native(&malformed, true).is_err());
        }
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
