//! Independent complete model families exercise finite observation consumers.

use serde_json::Value as Json;
use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, check};
use zetesis_themelios::observation::{ErrorKind, Limits, Resource};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

const BASE: &str = "p(1).p(2).e(f(1,2)).e(f(2,1)).{q(1);q(2);hidden}.";
const TERMS: [&str; 6] = ["X", "X+10", "f(X,(1;2))", "(X,1..2)", "p(X)", "-f(X)"];
const BODIES: [&str; 8] = [
    "p(X)",
    "p(X),#count{Y:q(Y)}=1",
    "p(X),#sum{Y,a:q(Y);Y,b:q(Y)}>1",
    "p(X),q(Y):p(Y)",
    "e(f(X,_)),not q(X)",
    "p(X),Y=1..2,q(Y),X<=Y",
    "p(X),not #min{Y:q(Y)}<2",
    "p(X),not not q((1;2))",
];
const POLICIES: [&str; 3] = ["", "#show.", "#show p/1."];
fn source(index: usize) -> String {
    let body = BODIES[index / (TERMS.len() * POLICIES.len())];
    let term = TERMS[(index / POLICIES.len()) % TERMS.len()];
    let policy = POLICIES[index % POLICIES.len()];
    format!("{BASE}\n{policy}\n#show {term}:{body}.\n")
}
fn cases() -> impl Iterator<Item = Json> {
    include_str!("fixtures/observation-families.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
}
fn admit(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
fn basis() -> (AdmittedFormula, Vec<Model>) {
    let input = admit(BASE);
    let mut models = Vec::new();
    for mask in 0..1_usize << input.atoms().len() {
        let candidate = Interpretation::new(
            input.theory(),
            (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
        )
        .unwrap();
        if check(
            input.theory(),
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Control::default(),
        )
        .unwrap()
        .accepted()
        {
            models.push(Model::new(
                candidate.atoms().map(|index| input.atoms()[index].clone()),
            ));
        }
    }
    assert_eq!(models.len(), 8);
    (input, models)
}
fn records(witnesses: &Json) -> Vec<Vec<String>> {
    let mut values: Vec<Vec<String>> = witnesses
        .as_array()
        .unwrap()
        .iter()
        .map(|witness| {
            let mut symbols: Vec<_> = witness
                .as_array()
                .unwrap()
                .iter()
                .map(|symbol| symbol.as_str().unwrap().to_owned())
                .collect();
            symbols.sort();
            symbols
        })
        .collect();
    values.sort();
    values
}
#[test]
fn finite_queries_preserve_the_original_formula() {
    let (plain, _) = basis();
    for index in 0..144 {
        let input = admit(&source(index));
        assert_eq!(input.atoms(), plain.atoms(), "case {index}");
        assert_eq!(
            input.theory().nodes(),
            plain.theory().nodes(),
            "case {index}"
        );
        assert_eq!(
            input.theory().roots(),
            plain.theory().roots(),
            "case {index}"
        );
    }
}
#[test]
fn complete_observation_multisets_preserve_hidden_family_multiplicity() {
    let (_, models) = basis();
    let mut count = 0;
    for (index, case) in cases().enumerate() {
        assert_eq!(case["source"], source(index));
        let input = admit(case["source"].as_str().unwrap());
        let mut actual: Vec<Vec<String>> = models
            .iter()
            .map(|model| {
                let rendered = input
                    .metadata()
                    .observations()
                    .render(
                        model,
                        input.metadata().output(),
                        Limits::default(),
                        &Control::default(),
                    )
                    .unwrap();
                let mut symbols: Vec<_> = rendered
                    .text()
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                symbols.sort();
                symbols
            })
            .collect();
        actual.sort();
        assert_eq!(
            actual,
            records(&case["witnesses"]),
            "case {index}: {}",
            case["source"]
        );
        count += actual.len();
    }
    assert_eq!(count, 1_152);
}
#[test]
fn complete_family_evaluations_obey_their_exact_work_boundary() {
    let (_, models) = basis();
    for index in 0..144 {
        let input = admit(&source(index));
        for model in &models {
            let run = |max_work| {
                input.metadata().observations().render(
                    model,
                    input.metadata().output(),
                    Limits {
                        max_work,
                        ..Limits::default()
                    },
                    &Control::default(),
                )
            };
            let full = run(Limits::default().max_work).unwrap();
            let exact = full.statistics().work;
            assert_eq!(run(exact).unwrap().text(), full.text());
            assert!(
                matches!(
                    run(exact - 1).unwrap_err().kind(),
                    ErrorKind::Limit {
                        resource: Resource::Work,
                        ..
                    }
                ),
                "case {index}"
            );
        }
    }
}
#[test]
#[ignore = "requires absolute CLINGO; bounded complete reference family capture"]
fn unchanged_finite_query_families_match_fresh_clingo() {
    use std::ffi::OsString;
    use zetesis_validation::process::{
        Invocation, Limits as ProcessLimits, Stop as ProcessStop, invoke,
    };
    let executable =
        std::path::PathBuf::from(std::env::var_os("CLINGO").expect("set absolute CLINGO"));
    let directory = tempfile::tempdir().unwrap();
    for case in cases() {
        let input = directory.path().join("source.lp");
        std::fs::write(&input, case["source"].as_str().unwrap()).unwrap();
        let arguments = [
            input.into_os_string(),
            OsString::from("0"),
            OsString::from("--outf=2"),
        ];
        let (capture, pending) = invoke(
            Invocation {
                executable: &executable,
                arguments: &arguments,
                directory: directory.path(),
            },
            ProcessLimits::default(),
        )
        .unwrap()
        .into_parts();
        if let Some(pending) = pending {
            panic!(
                "pending cleanup: {:?}",
                pending.retry(std::time::Duration::from_secs(1))
            );
        }
        assert_eq!(capture.stop(), ProcessStop::Completed);
        let actual: Json = serde_json::from_slice(capture.stdout()).unwrap();
        assert_eq!(actual["Models"]["More"], "no");
        let witnesses: Vec<_> = actual["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().unwrap())
            .map(|witness| witness["Value"].clone())
            .collect();
        assert_eq!(
            records(&Json::Array(witnesses)),
            records(&case["witnesses"]),
            "{}",
            case["source"]
        );
    }
}
