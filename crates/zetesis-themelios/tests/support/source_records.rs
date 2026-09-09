//! Shared complete source/model evidence for bounded aggregate campaigns.
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use zetesis_core::{Atom, Model, Sign, Value};
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
pub(super) fn canonical(atom: &Atom) -> String {
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let name = format!("{sign}{}", atom.predicate().name());
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
        name
    } else {
        format!("{name}({})", arguments.join(","))
    }
}
pub fn admit(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
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
    capture_clingo(source).records
}

/// One bounded independent invocation, retaining the raw evidence used to
/// reconcile every full-model record. Process completion is checked separately
/// from solver enumeration completion below.
#[allow(
    dead_code,
    reason = "Most shared oracle harnesses consume records without retaining the raw capture."
)]
pub struct ClingoCapture {
    pub records: Records,
    pub output: Json,
    pub diagnostics: String,
    pub status: i32,
}

const ORACLE_OUTPUT_LIMIT: usize = 65_536;

/// Probe at most one byte beyond the combined ceiling, even if a child writes
/// after the last size poll. A successful read contains both complete streams.
pub(super) fn read_capture(
    stdout: impl Read,
    stderr: impl Read,
    maximum: usize,
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let output = bounded_bytes(stdout, maximum)?;
    let diagnostics = bounded_bytes(stderr, maximum - output.len())?;
    Ok((output, diagnostics))
}

fn bounded_bytes(input: impl Read, allowance: usize) -> io::Result<Vec<u8>> {
    let probe = allowance
        .checked_add(1)
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "capture limit overflow"))?;
    let mut bytes = Vec::new();
    input.take(probe).read_to_end(&mut bytes)?;
    if bytes.len() > allowance {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oracle exceeded the combined output limit",
        ));
    }
    Ok(bytes)
}

pub fn capture_clingo(source: &str) -> ClingoCapture {
    let directory = Directory::new();
    let input = directory.0.join("case.lp");
    let output = directory.0.join("models.json");
    let errors = directory.0.join("stderr.txt");
    fs::write(&input, source).expect("original source");
    let stdout = File::create(&output).expect("oracle output");
    let stderr = File::create(&errors).expect("oracle diagnostics");
    let start = Instant::now();
    let executable = std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into());
    let mut child = Command::new(executable)
        .args(["0", "--outf=2", "--opt-mode=enum", "--warn=none"])
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().expect("output handle"))
        .stderr(stderr.try_clone().expect("diagnostic handle"))
        .spawn()
        .expect("independent clingo on PATH");
    let status = loop {
        if start.elapsed() > Duration::from_secs(5)
            || stdout
                .metadata()
                .expect("output size")
                .len()
                .saturating_add(stderr.metadata().expect("diagnostic size").len())
                > ORACLE_OUTPUT_LIMIT as u64
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
    let (bytes, diagnostics) = read_capture(
        File::open(output).expect("oracle JSON"),
        File::open(errors).expect("oracle diagnostics"),
        ORACLE_OUTPUT_LIMIT,
    )
    .expect("bounded complete oracle capture");
    let diagnostics = String::from_utf8(diagnostics).expect("UTF-8 oracle diagnostics");
    assert!(matches!(status.code(), Some(10 | 20 | 30)), "{diagnostics}");
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
    ClingoCapture {
        records,
        output: json,
        diagnostics,
        status: status.code().expect("normal oracle exit checked"),
    }
}
