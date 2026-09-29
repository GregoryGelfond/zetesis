//! Structural aggregate observers preserve every stable model's objective,
//! including absent vectors, fixed zero slots and global tuple deduplication.
//! The recorded clingo 5.8.2 campaign uses unbounded `--opt-mode=enum` to obtain
//! all models and their costs; its Result label is not an optimality certificate.

use std::collections::BTreeSet;

use crate::support::unsigned_spellings::atom_text;
use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_test_support::records::Records;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

struct Case {
    name: String,
    source: String,
    expected: Records,
}

fn cases() -> Vec<Case> {
    include_str!("../fixtures/aggregate-objective-observers.jsonl")
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).expect("recorded observer case");
            assert!(
                row["refusal"].is_null(),
                "every recorded observer is admitted"
            );
            Case {
                name: row["name"].as_str().expect("stable case name").to_owned(),
                source: row["source"].as_str().expect("unchanged source").to_owned(),
                expected: row["models"]
                    .as_array()
                    .expect("complete recorded model set")
                    .iter()
                    .map(|record| (atoms(&record[0]), costs(&record[1])))
                    .collect(),
            }
        })
        .collect()
}

fn atoms(values: &Json) -> BTreeSet<String> {
    values
        .as_array()
        .expect("whole canonical atom identities")
        .iter()
        .map(|value| value.as_str().expect("canonical ground atom").to_owned())
        .collect()
}
// The fixed source alphabet uses scalar numbers, simple symbols and ordinary
// quoted strings. Keep infinity and nonnumeric weight identities distinct.

fn costs(values: &Json) -> Option<Vec<i64>> {
    values.as_array().map(|values| {
        values
            .iter()
            .map(|value| value.as_i64().expect("integer objective cost"))
            .collect()
    })
}

fn exhaustive(input: &AdmittedFormula) -> Records {
    let count = input.atoms().len();
    assert!(count <= 16, "bounded independent subset enumeration");
    let cancellation = Cancellation::default();
    let mut records = Records::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .expect("same-theory candidate");
        if !check(input.theory(), &candidate, Limits::default(), &cancellation)
            .expect("complete independent reduct check")
            .accepted()
        {
            continue;
        }
        let atoms: BTreeSet<_> = candidate
            .atoms()
            .map(|atom| input.atoms().at(atom).unwrap())
            .collect();
        let evaluated_model =
            Model::from_positions(input.atom_catalog(), candidate.atoms()).unwrap();
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &evaluated_model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .expect("objective of a verified stable model");
        let score = evaluation.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, value)| value).collect());
        assert!(
            records.insert((atoms.iter().copied().map(atom_text).collect(), costs)),
            "unique full model identity"
        );
    }
    records
}

#[test]
fn observer_cases_preserve_recorded_contracts() {
    let cases = cases();
    assert_eq!(cases.len(), 92);
    for case in cases {
        let input = admit_formula(
            case.source.clone(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(
            exhaustive(&input),
            case.expected,
            "{}: {}",
            case.name,
            case.source
        );
    }
}

#[test]
#[ignore = "requires independent clingo; 92 bounded all-model/cost reference programs"]
fn recorded_observer_contracts_match_fresh_clingo() {
    for case in cases() {
        assert_eq!(
            oracle::records(&case.source),
            case.expected,
            "{}: {}",
            case.name,
            case.source
        );
    }
}
