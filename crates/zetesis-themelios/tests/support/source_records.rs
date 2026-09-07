//! Shared complete source/model evidence for bounded aggregate campaigns.
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
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

pub type Records = BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)>;
pub struct Case {
    pub name: String,
    pub source: String,
    pub records: Records,
}
pub fn cases(fixture: &str) -> Vec<Case> {
    fixture
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).unwrap();
            Case {
                name: row["name"].as_str().unwrap().into(),
                source: row["source"].as_str().unwrap().into(),
                records: row["records"]
                    .as_array()
                    .unwrap()
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
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().into())
        .collect()
}
fn costs(values: &Json) -> Option<Vec<i64>> {
    values
        .as_array()
        .map(|values| values.iter().map(|v| v.as_i64().unwrap()).collect())
}
fn canonical(atom: &Atom) -> String {
    let arguments: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Infimum => "#inf".into(),
            Value::Supremum => "#sup".into(),
            Value::Structured(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            Value::Symbol(value) => value.clone(),
            Value::String(value) => serde_json::to_string(value).unwrap(),
        })
        .collect();
    if arguments.is_empty() {
        atom.predicate().name().into()
    } else {
        format!("{}({})", atom.predicate().name(), arguments.join(","))
    }
}
pub fn admit(source: &str, limits: FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
}
pub fn exhaustive(input: &AdmittedFormula) -> Records {
    let count = input.atoms().len();
    assert!(count <= 12, "small independent subset enumeration");
    let control = Control::default();
    let mut records = Records::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .unwrap();
        if !check(input.theory(), &candidate, Limits::default(), &control)
            .unwrap()
            .accepted()
        {
            continue;
        }
        let atoms: Vec<_> = candidate
            .atoms()
            .map(|atom| input.atoms()[atom].clone())
            .collect();
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &Model::new(atoms.iter().cloned()),
            zetesis_objective::Limits::default(),
            &control,
        )
        .unwrap();
        let score = evaluation.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, value)| value).collect());
        assert!(records.insert((atoms.iter().map(canonical).collect(), costs)));
    }
    records
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zetesis-source-record-oracle-{}-{id}",
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

pub fn clingo(source: &str) -> Records {
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
