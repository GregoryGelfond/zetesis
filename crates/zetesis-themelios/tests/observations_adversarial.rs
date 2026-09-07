//! Generated observation joins preserve the full theory and both display multisets.
//!
//! References are recorded independently from complete clingo JSON. This slice
//! deliberately has no quoted or whitespace-containing terms; the fixed
//! observation campaign separately tests escaping and its richer token parser.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, check};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

type Record = (Vec<String>, Option<Vec<i64>>);

const BASE: &str = "p(1).p(2).e(1,2).e(2,1).e(2,2). {q(1);q(2);r(1);r(2)}.";
const BODIES: [&str; 13] = [
    "p(X),p(Y)",
    "p(X),p(Y),e(X,Y),e(Y,X)",
    "e(X,X),p(Y)",
    "p(X),p(Y),e(X,_)",
    "p(X),p(Y),e(_,Y)",
    "p(X),p(Y),not q(_)",
    "p(X),p(Y),not not e(_,Y)",
    "p(X),p(Y),q(X),X=Y",
    "p(X),p(Y),q(X),q(Y),X!=Y",
    "p(X),p(Y),e(X,Y),X<Y",
    "p(X),p(Y),q(Y),X<=Y",
    "p(X),p(Y),e(Y,X),X>Y",
    "p(X),p(Y),q(X),X>=Y",
];
const TERMS: [&str; 5] = ["pair(X,Y)", "(X,Y)", "nest(X,f(Y))", "q(X)", "x"];
const POLICIES: [&str; 3] = ["", "#show.", "#show q/1."];

fn cases() -> Vec<Json> {
    include_str!("fixtures/observations-adversarial.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn expected_source(index: usize) -> String {
    let body = BODIES[index / (TERMS.len() * POLICIES.len())];
    let term = TERMS[(index / POLICIES.len()) % TERMS.len()];
    let policy = POLICIES[index % POLICIES.len()];
    let mut source = format!("{BASE}\n{policy}\n#show {term}:{body}.\n#show q(X):r(X).\n");
    if index.is_multiple_of(13) {
        source.push_str("#maximize {0@7,k:q(1)}.\n");
    }
    source
}

fn admit(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}\n{error}"))
}

fn basis() -> (AdmittedFormula, Vec<Model>) {
    let input = admit(BASE);
    assert_eq!(input.atoms().len(), 9);
    let mut models = Vec::new();
    for bits in 0..1_usize << input.atoms().len() {
        let candidate = Interpretation::new(
            input.theory(),
            (0..input.atoms().len()).filter(|index| bits & (1 << index) != 0),
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
    assert_eq!(models.len(), 16);
    (input, models)
}

fn reference(result: &str, summary: &Json, witnesses: &[Json]) -> Vec<Record> {
    assert_eq!(summary["More"], "no");
    assert_eq!(summary["Number"].as_u64(), Some(witnesses.len() as u64));
    let mut records: Vec<Record> = witnesses
        .iter()
        .map(|witness| {
            let mut values: Vec<String> = witness["Value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    let text = value.as_str().unwrap();
                    assert!(!text.chars().any(char::is_whitespace));
                    assert!(!text.contains(['"', '\\']));
                    text.into()
                })
                .collect();
            values.sort();
            let costs = witness["Costs"]
                .as_array()
                .map(|costs| costs.iter().map(|cost| cost.as_i64().unwrap()).collect());
            (values, costs)
        })
        .collect();
    match result {
        "SATISFIABLE" => {
            assert_eq!(records.len(), 16);
            assert!(records.iter().all(|(_, costs)| costs.is_none()));
        }
        "OPTIMUM FOUND" => {
            assert_eq!(summary["Costs"], serde_json::json!([0]));
            assert_eq!(summary["Optimal"], 16);
            assert_eq!(records.len(), 17);
            assert!(
                records
                    .iter()
                    .all(|(_, costs)| costs.as_deref() == Some(&[0]))
            );
            // optN reports its final incumbent once before the complete tie pass.
            // Drop exactly that one record, even when many displays are equal.
            assert!(records[1..].contains(&records[0]));
            records.remove(0);
        }
        other => panic!("incomplete or unexpected reference result: {other}"),
    }
    records.sort();
    records
}

fn recorded(case: &Json) -> Vec<Record> {
    reference(
        case["reference_result"].as_str().unwrap(),
        &case["reference_summary"],
        case["witnesses"].as_array().unwrap(),
    )
}

fn native(input: &AdmittedFormula, models: &[Model]) -> Vec<Record> {
    let mut records = Vec::new();
    for model in models {
        let rendered = input
            .metadata()
            .observations()
            .render(
                model,
                input.metadata().output(),
                zetesis_themelios::observation::Limits::default(),
                &Control::default(),
            )
            .unwrap();
        assert!(!rendered.text().contains(['"', '\\', '\n']));
        let mut values: Vec<_> = rendered
            .text()
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect();
        values.sort();
        let objective = zetesis_objective::evaluate(
            input.objectives(),
            model,
            zetesis_objective::Limits::default(),
            &Control::default(),
        )
        .unwrap();
        let score = objective.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, cost)| cost).collect());
        records.push((values, costs));
    }
    records.sort();
    records
}

#[test]
fn generated_joins_preserve_theory_and_complete_observation_multisets() {
    let cases = cases();
    assert_eq!(cases.len(), BODIES.len() * TERMS.len() * POLICIES.len());
    let (base, models) = basis();
    let mut sources = BTreeSet::new();
    let mut records = 0;
    let mut objectives = 0;
    let mut duplicate_symbols = false;
    let mut duplicate_displays = false;
    for (index, case) in cases.iter().enumerate() {
        let source = case["source"].as_str().unwrap();
        assert_eq!(source, expected_source(index), "{}", case["name"]);
        assert!(sources.insert(source));
        let input = admit(source);
        assert_eq!(input.atoms(), base.atoms(), "{}", case["name"]);
        assert_eq!(input.theory().nodes(), base.theory().nodes());
        assert_eq!(input.theory().roots(), base.theory().roots());
        assert_eq!(input.objectives().is_present(), index.is_multiple_of(13));
        let expected = recorded(case);
        assert_eq!(
            native(&input, &models),
            expected,
            "{}: {source}",
            case["name"]
        );
        duplicate_symbols |= expected
            .iter()
            .any(|(values, _)| values.windows(2).any(|pair| pair[0] == pair[1]));
        duplicate_displays |= expected.windows(2).any(|pair| pair[0] == pair[1]);
        records += expected.len();
        objectives += usize::from(input.objectives().is_present());
    }
    assert_eq!((sources.len(), records, objectives), (195, 3_120, 15));
    assert!(duplicate_symbols && duplicate_displays);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let index = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zetesis-observation-adversarial-{}-{index}",
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

fn clingo(source: &str) -> Vec<Record> {
    let directory = Directory::new();
    let input = directory.0.join("case.lp");
    let output = directory.0.join("models.json");
    let errors = directory.0.join("stderr.txt");
    fs::write(&input, source).unwrap();
    let stdout = File::create(&output).unwrap();
    let stderr = File::create(&errors).unwrap();
    let start = Instant::now();
    let mut child = Command::new("clingo")
        .args(["0", "--outf=2", "--opt-mode=optN", "--warn=none"])
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().unwrap())
        .stderr(stderr.try_clone().unwrap())
        .spawn()
        .expect("independent clingo on PATH");
    let status = loop {
        if start.elapsed() > Duration::from_secs(5)
            || stdout.metadata().unwrap().len() + stderr.metadata().unwrap().len() > 65_536
        {
            let _ = child.kill();
            let _ = child.wait();
            panic!("oracle exceeded time or output limit: {source}");
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(
        matches!(status.code(), Some(10 | 20 | 30)),
        "{}",
        fs::read_to_string(&errors).unwrap()
    );
    assert!(stdout.metadata().unwrap().len() + stderr.metadata().unwrap().len() <= 65_536);
    let json: Json = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    let witnesses: Vec<_> = json["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten().cloned())
        .collect();
    reference(
        json["Result"].as_str().unwrap(),
        &json["Models"],
        &witnesses,
    )
}

#[test]
#[ignore = "requires independent clingo on PATH; 195 complete tiny references"]
fn fresh_clingo_matches_recorded_and_native_complete_display_multisets() {
    let (base, models) = basis();
    let cases = cases();
    assert_eq!(cases.len(), 195);
    for case in cases {
        let source = case["source"].as_str().unwrap();
        let input = admit(source);
        assert_eq!(input.atoms(), base.atoms());
        assert_eq!(input.theory().nodes(), base.theory().nodes());
        assert_eq!(input.theory().roots(), base.theory().roots());
        let expected = clingo(source);
        assert_eq!(recorded(&case), expected, "{}", case["name"]);
        assert_eq!(native(&input, &models), expected, "{}", case["name"]);
    }
}
