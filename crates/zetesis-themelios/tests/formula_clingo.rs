//! Recorded adversarial source programs checked through the exhaustive reduct
//! oracle. The optional external campaign also reruns complete clingo model
//! enumeration; neither test uses the native SAT search implementation.
//!
//! The fixture records 29 selected programs and 400 generated programs over
//! three propositional atoms. It includes recursive choice eligibility,
//! repeated heads, default/double negation, empty choices, and local/global
//! variable scopes. Its 403 valid and 26 invalid cases were independently
//! evaluated with clingo 5.8.2. Recorded cases make the campaign reproducible
//! without depending on a particular random-generator implementation.

use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value as Json;
use zetesis_core::{Atom, Predicate, TemplateTerm, ValueLimits};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit, admit_formula,
};

type Models = BTreeSet<BTreeSet<Atom>>;

struct Case {
    source: String,
    models: Option<Models>,
}

fn cases() -> Vec<Case> {
    include_str!("fixtures/formula-campaign.jsonl")
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).expect("recorded campaign row");
            Case {
                source: row[0].as_str().expect("original source").to_owned(),
                models: row[1].as_array().map(|models| {
                    let result: Models = models.iter().map(model).collect();
                    assert_eq!(result.len(), models.len(), "distinct recorded models");
                    result
                }),
            }
        })
        .collect()
}

fn model(atoms: &Json) -> BTreeSet<Atom> {
    let items = atoms.as_array().expect("whole atom identities");
    let result: BTreeSet<_> = items
        .iter()
        .map(|item| atom(item.as_str().expect("canonical atom string")))
        .collect();
    assert_eq!(result.len(), items.len(), "distinct atoms within a model");
    result
}

fn atom(source: &str) -> Atom {
    // Parsing complete atoms preserves strings and numeric identities; output
    // atoms are never split at whitespace or compared through display filters.
    let input = admit(format!("{source}."), AdmissionOptions::default())
        .expect("campaign atoms belong to the scalar profile");
    assert_eq!(input.program().templates().len(), 1);
    let head = input
        .program()
        .templates()
        .at(0)
        .unwrap()
        .head()
        .expect("ground atom");
    let values = head
        .terms()
        .iter()
        .map(|term| match term {
            TemplateTerm::Constant(value) => value.to_value(ValueLimits::default()).unwrap(),
            TemplateTerm::Variable(_) => panic!("oracle atom must be ground"),
        })
        .collect();
    let predicate = head.predicate();
    Atom::new(
        Predicate::with_sign(predicate.name(), predicate.arity(), predicate.sign()).unwrap(),
        values,
    )
    .expect("canonical atom arity")
}

fn exhaustive(input: &AdmittedFormula) -> Models {
    let count = input.atoms().len();
    assert!(count <= 16, "recorded campaign has a small finite carrier");
    let cancellation = Cancellation::default();
    let mut result = Models::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .expect("same-theory candidate");
        if check(input.theory(), &candidate, Limits::default(), &cancellation)
            .expect("complete independent subset enumeration")
            .accepted()
        {
            let atoms = candidate
                .atoms()
                .map(|atom| {
                    input
                        .atoms()
                        .at(atom)
                        .unwrap()
                        .to_atom(ValueLimits::default())
                        .unwrap()
                })
                .collect();
            assert!(result.insert(atoms), "unique semantic model");
        }
    }
    result
}

fn native(case: &Case) -> Option<Models> {
    match admit_formula(
        case.source.clone(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    ) {
        Ok(input) => Some(exhaustive(&input)),
        Err(error) => {
            assert!(
                case.models.is_none(),
                "valid source refused: {}\n{error}",
                case.source
            );
            None
        }
    }
}

fn external(source: &str) -> Option<Models> {
    let mut child = Command::new("clingo")
        .args(["-", "0", "--outf=2", "--warn=none"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("independently installed clingo executable");
    child
        .stdin
        .take()
        .expect("source input pipe")
        .write_all(source.as_bytes())
        .expect("write bounded source fixture");
    let output = child.wait_with_output().expect("complete external process");
    let json: Json = serde_json::from_slice(&output.stdout).expect("clingo JSON response");
    if json["Result"] == "UNKNOWN" {
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("error:"),
            "source refusal requires a reported error: {source}"
        );
        return None;
    }
    assert!(
        matches!(output.status.code(), Some(10 | 20 | 30)),
        "clingo failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(matches!(
        json["Result"].as_str(),
        Some("SATISFIABLE" | "UNSATISFIABLE")
    ));
    assert_eq!(json["Models"]["More"], "no", "exhaustive oracle coverage");
    let mut models = Models::new();
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                assert!(
                    models.insert(model(&witness["Value"])),
                    "unique oracle model"
                );
            }
        }
    }
    assert_eq!(
        u64::try_from(models.len()).expect("bounded model count"),
        json["Models"]["Number"].as_u64().expect("oracle count"),
        "complete identity-preserving model count"
    );
    Some(models)
}

#[test]
fn recorded_sources_match_exhaustive_reduct_models() {
    let cases = cases();
    assert_eq!(cases.len(), 429);
    assert_eq!(
        cases.iter().filter(|case| case.models.is_some()).count(),
        403
    );
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(native(case), case.models, "case {index}: {}", case.source);
    }
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn recursive_conditional_choices_match_complete_clingo_models() {
    for (index, case) in cases().iter().enumerate() {
        let reference = external(&case.source);
        assert_eq!(
            reference, case.models,
            "recorded case {index}: {}",
            case.source
        );
        assert_eq!(
            native(case),
            reference,
            "native case {index}: {}",
            case.source
        );
    }
}
