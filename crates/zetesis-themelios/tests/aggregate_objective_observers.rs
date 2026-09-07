//! Structural aggregate observers preserve every stable model's objective,
//! including absent vectors, fixed zero slots and global tuple deduplication.
//! The recorded clingo 5.8.2 campaign uses unbounded `--opt-mode=enum` to obtain
//! all models and their costs; its Result label is not an optimality certificate.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use zetesis_core::{Atom, Model, Value};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, ProfileFeature, admit_formula,
};

type Records = BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)>;
struct Case {
    name: String,
    source: String,
    refusal: Option<ProfileFeature>,
    expected: Records,
}

fn cases() -> Vec<Case> {
    include_str!("fixtures/aggregate-objective-observers.jsonl")
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).expect("recorded observer case");
            Case {
                name: row["name"].as_str().expect("stable case name").to_owned(),
                source: row["source"].as_str().expect("unchanged source").to_owned(),
                refusal: match row["refusal"].as_str() {
                    None => None,
                    Some("ObjectiveAggregateDependency") => {
                        Some(ProfileFeature::ObjectiveAggregateDependency)
                    }
                    Some("ObjectiveNegativeDependency") => {
                        Some(ProfileFeature::ObjectiveNegativeDependency)
                    }
                    Some(other) => panic!("unreviewed refusal: {other}"),
                },
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
fn atom_text(atom: &Atom) -> String {
    let name = atom.predicate().name();
    if atom.values().is_empty() {
        return name.to_owned();
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Number(number) => number.to_string(),
            Value::Symbol(symbol) => symbol.clone(),
            Value::String(value) => serde_json::to_string(value).expect("quoted scalar string"),
            Value::Infimum => "#inf".to_owned(),
            Value::Supremum => "#sup".to_owned(),
            Value::Structured(value) => value.to_string(),
        })
        .collect();
    format!("{name}({})", values.join(","))
}

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
    let control = Control::default();
    let mut records = Records::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .expect("same-theory candidate");
        if !check(input.theory(), &candidate, Limits::default(), &control)
            .expect("complete independent reduct check")
            .accepted()
        {
            continue;
        }
        let atoms: BTreeSet<_> = candidate
            .atoms()
            .map(|atom| input.atoms()[atom].clone())
            .collect();
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &Model::new(atoms.iter().cloned()),
            zetesis_objective::Limits::default(),
            &control,
        )
        .expect("objective of a verified stable model");
        let score = evaluation.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, value)| value).collect());
        assert!(
            records.insert((atoms.iter().map(atom_text).collect(), costs)),
            "unique full model identity"
        );
    }
    records
}

#[test]
fn admitted_observers_preserve_all_model_costs_and_refusals_are_explicit() {
    let cases = cases();
    assert_eq!(cases.len(), 92);
    let mut admitted = 0;
    let mut refused = 0;
    for case in cases {
        let result = admit_formula(
            case.source.clone(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        );
        if let Some(expected) = case.refusal {
            let Err(error) = result else {
                panic!("{}: expected reviewed refusal", case.name);
            };
            assert!(!error.diagnostics().is_empty(), "located refusal");
            assert!(
                matches!(error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile { feature, .. }))
                    if feature == expected),
                "{}: {error}",
                case.name
            );
            refused += 1;
        } else {
            let input = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
            assert_eq!(
                exhaustive(&input),
                case.expected,
                "{}: {}",
                case.name,
                case.source
            );
            admitted += 1;
        }
    }
    assert_eq!((admitted, refused), (72, 20));
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zetesis-observer-oracle-{}-{id}",
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
    let input = directory.0.join("case.lp");
    let output = directory.0.join("models.json");
    let errors = directory.0.join("stderr.txt");
    fs::write(&input, source).expect("original source");
    let stdout = File::create(&output).expect("oracle output");
    let stderr = File::create(&errors).expect("oracle diagnostics");
    let start = Instant::now();
    let mut child = Command::new("clingo")
        .args(["0", "--outf=2", "--opt-mode=enum", "--warn=none"])
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().expect("output handle"))
        .stderr(stderr.try_clone().expect("diagnostic handle"))
        .spawn()
        .expect("independent clingo on PATH");
    let status = loop {
        if start.elapsed() > Duration::from_secs(5)
            || stdout.metadata().expect("output size").len()
                + stderr.metadata().expect("diagnostic size").len()
                > 65_536
        {
            let _ = child.kill();
            let _ = child.wait();
            panic!("oracle exceeded time or output limit: {source}");
        }
        if let Some(status) = child.try_wait().expect("oracle status") {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(
        matches!(status.code(), Some(10 | 20 | 30)),
        "{}",
        fs::read_to_string(errors).expect("oracle diagnostics")
    );
    let bytes = fs::read(output).expect("oracle JSON");
    assert!(bytes.len() <= 65_536);
    let json: Json = serde_json::from_slice(&bytes).expect("complete oracle output");
    assert_eq!(json["Models"]["More"].as_str(), Some("no"));
    assert!(matches!(
        json["Result"].as_str(),
        Some("SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND")
    ));
    let mut records = Records::new();
    let mut count = 0;
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                count += 1;
                assert!(
                    records.insert((atoms(&witness["Value"]), costs(&witness["Costs"]))),
                    "duplicate full model"
                );
            }
        }
    }
    assert_eq!(json["Models"]["Number"].as_u64(), Some(count));
    records
}

#[test]
#[ignore = "requires independent clingo; 92 bounded all-model/cost reference programs"]
fn recorded_observer_contracts_match_fresh_clingo() {
    for case in cases() {
        assert_eq!(
            clingo(&case.source),
            case.expected,
            "{}: {}",
            case.name,
            case.source
        );
    }
}
