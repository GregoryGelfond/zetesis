//! Independent complete-source factorization campaign: 36 unchanged programs,
//! 676 full stable models, absent costs preserved, no display projection.
//! Expected records were obtained from clingo 5.8.2. Portable checks use both
//! exhaustive Ferraris membership and native complete countermodel search.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use zetesis_core::{Atom, Model, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_sat::{Limits as SearchLimits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

type Record = (BTreeSet<String>, Option<Vec<i64>>);
type Records = BTreeSet<Record>;

struct Case {
    name: String,
    source: String,
    records: Records,
}

fn names(value: &Json) -> BTreeSet<String> {
    let raw = value.as_array().expect("complete canonical atom list");
    let names: BTreeSet<_> = raw
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names.len(), raw.len(), "no duplicate full-model atoms");
    names
}

fn costs(value: &Json) -> Option<Vec<i64>> {
    value
        .as_array()
        .map(|costs| costs.iter().map(|cost| cost.as_i64().unwrap()).collect())
}

fn cases() -> Vec<Case> {
    include_str!("fixtures/factorization-campaign.jsonl")
        .lines()
        .map(|line| {
            let value: Json = serde_json::from_str(line).expect("persisted reference fixture");
            assert_eq!(value["reference_version"], "clingo version 5.8.2");
            assert_eq!(value["reference_result"], "SATISFIABLE");
            let raw = value["models"].as_array().unwrap();
            let records: Records = raw
                .iter()
                .map(|row| (names(&row[0]), costs(&row[1])))
                .collect();
            assert_eq!(records.len(), raw.len(), "raw full-model multiplicity");
            Case {
                name: value["name"].as_str().unwrap().to_owned(),
                source: value["source"].as_str().unwrap().to_owned(),
                records,
            }
        })
        .collect()
}

fn atom_text(atom: &Atom) -> String {
    let predicate = atom.predicate().name();
    if atom.values().is_empty() {
        return predicate.to_owned();
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Infimum => "#inf".to_owned(),
            Value::Supremum => "#sup".to_owned(),
            Value::Structured(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            Value::Symbol(value) => value.clone(),
            Value::String(value) => serde_json::to_string(value).unwrap(),
        })
        .collect();
    format!("{predicate}({})", values.join(","))
}

fn record(input: &AdmittedFormula, model: &Interpretation) -> Record {
    assert!(model.theory().same_instance(input.theory()));
    let model = Model::new(model.atoms().map(|index| input.atoms()[index].clone()));
    let evaluation = zetesis_objective::evaluate(
        input.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .expect("exact objective observation of a verified stable model");
    let score = evaluation.score();
    (
        model.atoms().iter().map(atom_text).collect(),
        score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, cost)| cost).collect()),
    )
}

fn exhaustive(input: &AdmittedFormula) -> Records {
    let count = input.atoms().len();
    assert!(count <= 12, "bounded independent powerset reference");
    let mut records = Records::new();
    for mask in 0..1_usize << count {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|atom| mask & (1 << atom) != 0),
        )
        .unwrap();
        if check(
            input.theory(),
            &candidate,
            Limits::default(),
            &Cancellation::default(),
        )
        .expect("complete original Ferraris reduct check")
        .accepted()
        {
            assert!(
                records.insert(record(input, &candidate)),
                "unique complete semantic model"
            );
        }
    }
    records
}

fn native(input: &AdmittedFormula) -> Records {
    let mut models = StableModels::new(
        input.theory(),
        SearchLimits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut records = Records::new();
    for model in models.by_ref() {
        assert!(
            records.insert(record(
                input,
                &model.expect("completed native reduct search")
            )),
            "native semantic projection must be unique"
        );
    }
    assert!(models.exhausted(), "partial coverage cannot pass");
    records
}

#[test]
fn complete_factorization_sources_match_independent_reduct_and_recorded_model_costs() {
    let cases = cases();
    assert_eq!(cases.len(), 36);
    let mut total = 0;
    for case in cases {
        let input = admit_formula(
            case.source.clone(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{}: explicit admission required: {error}", case.name));
        let reference = exhaustive(&input);
        assert_eq!(reference, case.records, "{}: {}", case.name, case.source);
        assert_eq!(
            native(&input),
            reference,
            "{}: complete native enumeration",
            case.name
        );
        total += reference.len();
    }
    assert_eq!(total, 676);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let number = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zetesis-factor-oracle-{}-{number}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("oracle directory: {error}"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("oracle fixture cleanup");
    }
}

fn clingo(source: &str) -> Records {
    let directory = Directory::new();
    let input = directory.0.join("original.lp");
    let output = directory.0.join("models.json");
    let errors = directory.0.join("stderr.txt");
    fs::write(&input, source).expect("unchanged original source");
    let mut child = Command::new("clingo")
        .args(["--outf=2", "--models=0"])
        .arg(&input)
        .stdout(Stdio::from(File::create(&output).unwrap()))
        .stderr(Stdio::from(File::create(&errors).unwrap()))
        .spawn()
        .expect("installed external clingo oracle");
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() >= Duration::from_secs(5) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("external oracle timeout");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(
        matches!(status.code(), Some(0 | 10 | 20 | 30)),
        "external process failed: {status}"
    );
    assert!(
        fs::metadata(&output).unwrap().len() <= 65_536,
        "bounded complete oracle output"
    );
    assert!(
        fs::metadata(&errors).unwrap().len() <= 65_536,
        "bounded diagnostics"
    );
    let value: Json = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(value["Result"], "SATISFIABLE");
    assert_eq!(value["Models"]["More"], "no");
    let mut records = Records::new();
    let mut count = 0_u64;
    for witness in value["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
    {
        assert!(
            records.insert((names(&witness["Value"]), costs(&witness["Costs"]))),
            "raw complete model count cannot hide duplicates"
        );
        count += 1;
    }
    assert_eq!(value["Models"]["Number"].as_u64(), Some(count));
    records
}

#[test]
#[ignore = "requires installed clingo; exact complete full models and cost presence"]
fn factorization_campaign_matches_fresh_complete_clingo() {
    let cases = cases();
    assert_eq!(cases.len(), 36);
    let mut total = 0;
    for case in cases {
        let actual = clingo(&case.source);
        assert_eq!(actual, case.records, "{}: {}", case.name, case.source);
        total += actual.len();
    }
    assert_eq!(total, 676);
}
