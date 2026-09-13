//! Signed unconditional source disjunctions retain their original reduct.
//! Hand-written formula trees are independent of source normalization and SAT.

#[path = "support/objective_dependency_records.rs"]
mod objective_dependencies;

use std::collections::BTreeSet;

use serde_json::Value as Json;
use zetesis_core::{Atom, Model, Sign, Value};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Limits as OracleLimits, models, models_reduct};
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit_extended, admit_formula,
};

type Names = BTreeSet<String>;
type Models = BTreeSet<Names>;

fn cases() -> Vec<Json> {
    serde_json::from_str::<Json>(include_str!("fixtures/disjunction.json")).unwrap()["cases"]
        .as_array()
        .unwrap()
        .clone()
}
fn input(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}
fn name(atom: &Atom) -> String {
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    if atom.values().is_empty() {
        return format!("{sign}{}", atom.predicate().name());
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Infimum => "#inf".to_owned(),
            Value::Supremum => "#sup".to_owned(),
            Value::Structured(value) => value.to_string(),
            Value::Number(number) => number.to_string(),
            Value::String(string) => serde_json::to_string(string).unwrap(),
            Value::Symbol(symbol) => symbol.clone(),
        })
        .collect();
    format!("{sign}{}({})", atom.predicate().name(), values.join(","))
}
fn names(value: &Json) -> Names {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect()
}
fn expected(value: &Json) -> Models {
    value.as_array().unwrap().iter().map(names).collect()
}
fn subset(atoms: &[String], mask: usize) -> Names {
    atoms
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, atom)| atom.clone())
        .collect()
}

// Every M-false subtree becomes falsum, including non-atomic implications.
// This evaluates JSON trees directly, without a production DAG or compiler.
fn truth(formula: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    if frozen.is_some_and(|outer| !truth(formula, outer, None)) {
        return false;
    }
    if let Some(atom) = formula.as_str() {
        return tested.contains(atom);
    }
    if formula == &Json::Bool(false) {
        return false;
    }
    let left = truth(&formula[1], tested, frozen);
    let right = truth(&formula[2], tested, frozen);
    match formula[0].as_str().unwrap() {
        "and" => left && right,
        "or" => left || right,
        "imp" => !left || right,
        other => panic!("unknown manual formula {other}"),
    }
}
fn holds(theory: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    theory["roots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|root| truth(root, tested, frozen))
}
fn manual_models(theory: &Json) -> Models {
    let atoms: Vec<_> = names(&theory["atoms"]).into_iter().collect();
    assert!(atoms.len() <= 8, "tiny independent exhaustive carrier");
    (0..1 << atoms.len())
        .filter_map(|mask| {
            let outer = subset(&atoms, mask);
            if !holds(theory, &outer, None) {
                return None;
            }
            let true_atoms: Vec<_> = outer.iter().cloned().collect();
            let proper_model = (0..(1 << true_atoms.len()) - 1)
                .any(|bits| holds(theory, &subset(&true_atoms, bits), Some(&outer)));
            (!proper_model).then_some(outer)
        })
        .collect()
}
fn interpretations(input: &AdmittedFormula) -> Vec<Interpretation> {
    let count = input.atoms().len();
    assert!(count <= 8);
    (0..1_usize << count)
        .map(|bits| {
            Interpretation::new(input.theory(), (0..count).filter(|i| bits & (1 << i) != 0))
                .unwrap()
        })
        .collect()
}
fn projected(input: &AdmittedFormula, model: &Interpretation) -> Names {
    model.atoms().map(|i| name(&input.atoms()[i])).collect()
}

#[test]
fn complete_models_match_manual_source_theories_and_recorded_clingo() {
    for case in cases()
        .iter()
        .filter(|case| case["expected_native"] == "admit")
    {
        let label = case["name"].as_str().unwrap();
        let predicted = expected(&case["expected_stable_models"]);
        assert_eq!(manual_models(&case["manual_theory"]), predicted, "{label}");
        let admitted = input(case["source"].as_str().unwrap())
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let mut search =
            StableModels::new(admitted.theory(), Limits::default(), Control::default()).unwrap();
        let mut found = Models::new();
        for model in search.by_ref() {
            assert!(
                found.insert(projected(&admitted, &model.unwrap())),
                "unique semantic model"
            );
        }
        assert!(search.exhausted(), "{label}: complete coverage");
        assert_eq!(found, predicted, "{label}: full original atom carrier");
        let reference = case["reference"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["mode"] == "ignore")
            .unwrap_or(&case["reference"][0]);
        assert_eq!(reference["normalized"]["status"], "complete");
        assert_eq!(
            found,
            expected(&reference["normalized"]["models"]),
            "{label}: clingo"
        );
        assert_eq!(
            admitted.formula_origins().len(),
            admitted.theory().roots().len()
        );
        assert!(
            admitted
                .formula_origins()
                .iter()
                .all(|origins| !origins.is_empty())
        );
    }
}

#[test]
fn every_frozen_subset_matches_manual_formulas_and_necessary_support() {
    let control = Control::default();
    for case in cases()
        .iter()
        .filter(|case| case["expected_native"] == "admit")
    {
        let label = case["name"].as_str().unwrap();
        let admitted = input(case["source"].as_str().unwrap()).unwrap();
        let manual = &case["manual_theory"];
        let all = interpretations(&admitted);
        for candidate in &all {
            let outer = projected(&admitted, candidate);
            // Producer guards are independently read from the hand-written
            // ground rules. They are necessary classical conditions whose
            // double negation has a tautological reduct when true in M.
            let supported = outer.iter().all(|atom| {
                manual["producer_bodies"][atom]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|body| truth(body, &outer, None))
            });
            assert_eq!(
                models(
                    admitted.theory(),
                    candidate,
                    OracleLimits::default(),
                    &control
                )
                .unwrap(),
                supported && holds(manual, &outer, None),
                "{label}: original M={outer:?}"
            );
            for tested in &all {
                let inner = projected(&admitted, tested);
                if !inner.is_subset(&outer) {
                    continue;
                }
                assert_eq!(
                    models_reduct(
                        admitted.theory(),
                        candidate,
                        tested,
                        OracleLimits::default(),
                        &control
                    )
                    .unwrap(),
                    supported && holds(manual, &inner, Some(&outer)),
                    "{label}: frozen M={outer:?}, J={inner:?}"
                );
            }
        }
    }
}

#[test]
fn unrelated_objectives_preserve_presence_priorities_and_tuple_identity() {
    for case in cases()
        .iter()
        .filter(|case| case["expected_native"] == "admit" && !case["objective"].is_null())
    {
        let admitted = input(case["source"].as_str().unwrap()).unwrap();
        let mut search =
            StableModels::new(admitted.theory(), Limits::default(), Control::default()).unwrap();
        let mut scored = Vec::new();
        for result in search.by_ref() {
            let model = result.unwrap();
            let evaluated = zetesis_objective::evaluate(
                admitted.objectives(),
                &Model::new(model.atoms().map(|i| admitted.atoms()[i].clone())),
                zetesis_objective::Limits::default(),
                &Control::default(),
            )
            .unwrap();
            assert!(evaluated.score().is_present());
            let cost: Vec<_> = evaluated
                .score()
                .costs()
                .iter()
                .map(|&(_, value)| value)
                .collect();
            scored.push((cost, projected(&admitted, &model)));
        }
        assert!(search.exhausted());
        let best = scored.iter().map(|(cost, _)| cost).min().unwrap();
        let expected_cost: Vec<_> = case["objective"]["expected_optimum_cost"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cost| cost.as_i64().unwrap())
            .collect();
        assert_eq!(best, &expected_cost);
        let optimal: Models = scored
            .iter()
            .filter(|(cost, _)| cost == best)
            .map(|(_, model)| model.clone())
            .collect();
        assert_eq!(
            optimal,
            expected(&case["objective"]["expected_optimal_models"])
        );
        assert_eq!(
            case["reference"][0]["normalized"]["cost"],
            case["objective"]["expected_optimum_cost"]
        );
    }
}

#[test]
fn excluded_heads_remain_located_refusals() {
    for case in cases()
        .iter()
        // The immutable fixture records a historical arithmetic refusal.
        // Current scalar/interval, negated-head, finite-pool and true/empty-condition semantics
        // have dedicated tests.
        .filter(|case| {
            case["expected_native"] == "refuse"
                && case["expected_refusal"] != "ObjectiveDisjunctionDependency"
                && !matches!(
                    case["name"].as_str().unwrap(),
                    "variable-arithmetic-head"
                        | "interval-head"
                        | "default-negated-head"
                        | "conditional-empty-head"
                        | "pooled-head"
                        | "infinite-head"
                )
        })
    {
        let error =
            input(case["source"].as_str().unwrap()).expect_err(case["name"].as_str().unwrap());
        assert!(!error.diagnostics().is_empty());
        if case["expected_refusal"] == "UnsafeVariable" {
            assert!(
                matches!(error, FormulaFailure::UnsafeVariable { .. }),
                "{error}"
            );
        } else {
            let feature = match case["expected_refusal"].as_str().unwrap() {
                "ConditionalDisjunction" | "conditional disjunction element" => {
                    ProfileFeature::ConditionalDisjunction
                }
                "NegatedHead" => ProfileFeature::NegatedHead,
                "PooledArguments" => ProfileFeature::PooledArguments,
                "generated/interval disjunction head" | "generated disjunction head" => {
                    ProfileFeature::Term
                }
                other => panic!("unclassified refusal {other}"),
            };
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(
                AdmissionFailure::Profile { feature: actual, .. }
            )) if actual == feature),
                "{}: expected {feature:?}, got {error:?}",
                case["name"]
            );
        }
    }
    assert!(
        admit_extended(
            "a | b.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err(),
        "relational S0 remains explicit"
    );
}

#[test]
fn disjunction_element_ceiling_is_inclusive_and_independent_of_body_limits() {
    let admit = |text: &str, count| {
        admit_formula(
            text.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_disjunction_elements: count,
                ..FormulaLimits::default()
            },
        )
    };
    assert!(admit("a | b.", 2).is_ok());
    assert!(
        admit("a | a.", 1).is_ok(),
        "owned duplicate elements coalesce"
    );
    for (source, limit, observed) in [("a | b.", 1, 2), ("a | a.", 0, 1)] {
        assert!(matches!(admit(source, limit), Err(FormulaFailure::Limit {
            resource: FormulaResource::DisjunctionElements, observed: actual, ..
        }) if actual == observed));
    }
    assert!(admit("a.", 0).is_ok());
}

struct Capture(std::path::PathBuf);
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn reference(source: &str, mode: &str) -> Json {
    use std::fs::{self, File};
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let capture = Capture(std::env::temp_dir().join(format!(
        "zetesis-disjunction-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&capture.0).unwrap();
    let output = capture.0.join("stdout");
    let errors = capture.0.join("stderr");
    let executable = std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into());
    let mut child = Command::new(executable)
        .args([
            "-",
            "--models=0",
            "--outf=2",
            "--warn=none",
            &format!("--opt-mode={mode}"),
        ])
        .stdin(Stdio::piped())
        .stdout(File::create(&output).unwrap())
        .stderr(File::create(&errors).unwrap())
        .spawn()
        .expect("external clingo oracle");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let bytes = fs::metadata(&output).unwrap().len() + fs::metadata(&errors).unwrap().len();
        if Instant::now() >= deadline || bytes > 8 * 1_024 * 1_024 {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("clingo timed out or exceeded bounded capture");
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    serde_json::from_slice(&fs::read(output).unwrap())
        .expect("clingo JSON, including explicit refusal")
}
fn normalized_reference(raw: &Json) -> (String, Models, Option<Vec<i64>>) {
    let result = raw["Result"].as_str().unwrap().to_owned();
    let witnesses: Vec<_> = raw["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(
        raw["Models"]["Number"].as_u64().unwrap(),
        u64::try_from(witnesses.len()).unwrap()
    );
    if result == "UNKNOWN" {
        assert!(
            witnesses.is_empty(),
            "source refusal is never model coverage"
        );
        return (result, Models::new(), None);
    }
    assert_eq!(raw["Models"]["More"], "no");
    assert!(matches!(
        result.as_str(),
        "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND"
    ));
    let costs: Vec<Option<Vec<i64>>> = witnesses
        .iter()
        .map(|witness| {
            witness["Costs"]
                .as_array()
                .map(|costs| costs.iter().map(|x| x.as_i64().unwrap()).collect())
        })
        .collect();
    let best = costs.iter().min().cloned().flatten();
    assert!(
        costs
            .iter()
            .all(|cost| cost.as_ref().map(Vec::len) == best.as_ref().map(Vec::len))
    );
    let models = witnesses
        .iter()
        .zip(costs)
        .filter(|(_, cost)| cost == &best)
        .map(|(witness, _)| names(&witness["Value"]))
        .collect();
    (result, models, best)
}

#[test]
#[ignore = "requires external clingo; records source refusals separately from admitted parity"]
fn replay_recorded_clingo_models_optimum_slots_and_refusals() {
    let mut runs = 0;
    for case in cases() {
        for old in case["reference"].as_array().unwrap() {
            let mode = old["mode"].as_str().unwrap();
            let current = reference(case["source"].as_str().unwrap(), mode);
            let recorded: Json =
                serde_json::from_str(old["raw"]["stdout"].as_str().unwrap()).unwrap();
            assert_eq!(
                normalized_reference(&current),
                normalized_reference(&recorded),
                "{} / {mode}",
                case["name"]
            );
            runs += 1;
        }
    }
    assert_eq!(runs, 62);
}

#[test]
fn disjunctive_producers_preserve_scored_answers() {
    let sources: Vec<_> = cases()
        .into_iter()
        .filter(|case| case["expected_refusal"] == "ObjectiveDisjunctionDependency")
        .collect();
    assert_eq!(sources.len(), 12);
    for case in sources {
        objective_dependencies::check(case["source"].as_str().unwrap());
    }
}

#[test]
fn extremal_head_preserves_the_recorded_complete_family() {
    let cases = cases();
    let case = cases
        .iter()
        .find(|case| case["name"] == "infinite-head")
        .unwrap();
    // Keep the historical refusal label and raw clingo capture unchanged.
    let admitted = input(case["source"].as_str().unwrap()).unwrap();
    let mut search =
        StableModels::new(admitted.theory(), Limits::default(), Control::default()).unwrap();
    let found: Models = search
        .by_ref()
        .map(|model| projected(&admitted, &model.unwrap()))
        .collect();
    assert!(search.exhausted());
    assert_eq!(found, expected(&case["expected_stable_models"]));
    assert_eq!(
        found,
        expected(&case["reference"][0]["normalized"]["models"])
    );
}
