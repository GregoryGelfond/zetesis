//! Complete scalar/range source regressions, independently recorded with clingo
//! 5.8.2. Of 118 sources, 93 currently have exact native model parity, 9 valid
//! sources have explicit refused boundaries, and 16 sources are unsafe. The
//! historical integer-maximum singleton-range timeout is deliberately excluded.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use themelios_program::term::EvalError;
use zetesis_core::{Atom, Value};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit_formula,
};

type Models = BTreeSet<BTreeSet<String>>;
struct Case {
    name: String,
    source: String,
    valid: bool,
    native: String,
    expected: Models,
}
fn cases() -> Vec<Case> {
    include_str!("fixtures/scalar-bindings.jsonl")
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).expect("recorded scalar case");
            Case {
                name: row["name"].as_str().expect("case name").to_owned(),
                source: row["source"].as_str().expect("unchanged source").to_owned(),
                valid: row["valid"].as_bool().expect("source validity"),
                native: row["native"]
                    .as_str()
                    .expect("reviewed native boundary")
                    .to_owned(),
                expected: row["models"]
                    .as_array()
                    .expect("recorded models")
                    .iter()
                    .map(atoms)
                    .collect(),
            }
        })
        .collect()
}
fn atoms(values: &Json) -> BTreeSet<String> {
    values
        .as_array()
        .expect("complete canonical atom identities")
        .iter()
        .map(|value| value.as_str().expect("canonical ground atom").to_owned())
        .collect()
}
// This fixed corpus has scalar numbers, simple symbols and ordinary quoted
// strings. Preserve their complete identities without reraising clingo's i32
// minimum spelling, which the pinned source tier itself rejects as a literal.
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
fn exhaustive(input: &AdmittedFormula) -> Models {
    let count = input.atoms().len();
    assert!(
        count <= 16,
        "bounded independent exhaustive candidate carrier"
    );
    let control = Control::default();
    let mut models = Models::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .expect("same-theory candidate");
        if check(input.theory(), &candidate, Limits::default(), &control)
            .expect("complete independent reduct check")
            .accepted()
        {
            assert!(
                models.insert(
                    candidate
                        .atoms()
                        .map(|atom| atom_text(&input.atoms()[atom]))
                        .collect()
                )
            );
        }
    }
    models
}
fn refusal(error: &FormulaFailure, expected: &str) {
    assert!(!error.diagnostics().is_empty(), "located refusal");
    match expected {
        "UnsafeVariable" => assert!(matches!(error, FormulaFailure::UnsafeVariable { .. })),
        "Undefined" => assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Undefined,
                ..
            })
        )),
        "Overflow" => assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Overflow,
                ..
            })
        )),
        "SupportRounds" => assert!(matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::SupportRounds,
                limit: 4,
                observed: 5,
                ..
            }
        )),
        "Term" | "NegatedComparison" => {
            let expected = match expected {
                "Term" => ProfileFeature::Term,
                "NegatedComparison" => ProfileFeature::NegatedComparison,
                _ => unreachable!("matched feature"),
            };
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile { feature, .. })) if *feature == expected)
            );
        }
        other => panic!("unreviewed refusal: {other}"),
    }
}
#[test]
fn scalar_and_interval_admissions_match_complete_models_with_explicit_boundaries() {
    let cases = cases();
    assert_eq!(cases.len(), 118);
    assert_eq!(cases.iter().filter(|case| case.valid).count(), 102);
    let mut admitted = 0;
    let mut refused = 0;
    for case in cases {
        // A growing positive upper relation must refuse, despite clingo pruning
        // this particular source by a mandatory negative gate. Four rounds make
        // the resource regression small and independent of default work tuning.
        let limits = if case.native == "SupportRounds" {
            FormulaLimits {
                max_support_rounds: 4,
                ..FormulaLimits::default()
            }
        } else {
            FormulaLimits::default()
        };
        let result = admit_formula(
            case.source.clone(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        );
        // Keep historical native labels and original reference models intact.
        // Evaluated choice heads and separate bounds promote these cases.
        if case.native == "admit"
            || matches!(
                case.name.as_str(),
                "head_conditional_choice_arithmetic"
                    | "head_arithmetic_choice_global"
                    | "assign_integer_bounds"
                    | "head_double_negative_self"
                    | "range_inf_upper_empty"
                    | "range_sup_lower_empty"
                    | "interval_body_positive"
                    | "interval_body_negative"
            )
        {
            assert!(case.valid);
            let input = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
            assert_eq!(
                exhaustive(&input),
                case.expected,
                "{}: {}",
                case.name,
                case.source
            );
            admitted += 1;
        } else {
            let Err(error) = result else {
                panic!("{}: expected {}", case.name, case.native);
            };
            refusal(
                &error,
                if case.name == "range_negative_literal_unsafe" {
                    // The finite range syntax is admitted; single negation still
                    // cannot bind the source variable used by the head.
                    "UnsafeVariable"
                } else {
                    &case.native
                },
            );
            refused += 1;
        }
    }
    assert_eq!((admitted, refused), (93, 25));
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("zetesis-scalar-oracle-{}-{id}", std::process::id()));
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
fn clingo(case: &Case) -> Models {
    let directory = Directory::new();
    let input = directory.0.join("case.lp");
    let output = directory.0.join("models.json");
    let errors = directory.0.join("stderr.txt");
    fs::write(&input, &case.source).expect("original source");
    let stdout = File::create(&output).expect("oracle output");
    let stderr = File::create(&errors).expect("oracle diagnostics");
    let start = Instant::now();
    let mut child = Command::new("clingo")
        .args(["0", "--outf=2", "--warn=none"])
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
            panic!("oracle exceeded time or output limit: {}", case.name);
        }
        if let Some(status) = child.try_wait().expect("oracle status") {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let bytes = fs::read(output).expect("oracle JSON");
    assert!(bytes.len() <= 65_536);
    let json: Json = serde_json::from_slice(&bytes).expect("complete oracle output");
    if !case.valid {
        assert_eq!(json["Result"].as_str(), Some("UNKNOWN"));
        assert!(
            fs::read_to_string(errors)
                .expect("unsafe diagnostics")
                .contains("unsafe")
        );
        return Models::new();
    }
    assert!(
        matches!(status.code(), Some(10 | 20 | 30)),
        "{}",
        fs::read_to_string(errors).expect("oracle diagnostics")
    );
    assert_eq!(json["Models"]["More"].as_str(), Some("no"));
    assert!(matches!(
        json["Result"].as_str(),
        Some("SATISFIABLE" | "UNSATISFIABLE")
    ));
    let mut models = Models::new();
    let mut count = 0;
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                count += 1;
                assert!(
                    models.insert(atoms(&witness["Value"])),
                    "duplicate full model"
                );
            }
        }
    }
    assert_eq!(json["Models"]["Number"].as_u64(), Some(count));
    models
}
#[test]
#[ignore = "requires independent clingo; 118 bounded reference source programs"]
fn scalar_and_interval_reference_models_and_unsafe_cases_match_fresh_clingo() {
    for case in cases() {
        assert_eq!(
            clingo(&case),
            case.expected,
            "{}: {}",
            case.name,
            case.source
        );
    }
}

#[test]
#[ignore = "requires independent clingo; bounded interval facts"]
fn nonnumeric_fact_ranges_match_clingo() {
    for source in [
        "p(a..b).",
        "p(1..a).",
        "p(a..1).",
        "p(\"a\"..\"b\").",
        "p(#inf..#sup).",
        "p(f(a)..f(b)).",
    ] {
        let case = Case {
            name: source.into(),
            source: source.into(),
            valid: true,
            native: "admit".into(),
            expected: Models::from([BTreeSet::new()]),
        };
        let input = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .expect("defined nonnumeric endpoints");
        assert_eq!(exhaustive(&input), case.expected, "{source}");
        assert_eq!(clingo(&case), case.expected, "{source}");
    }
}
