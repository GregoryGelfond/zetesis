//! Strong predicate signs remain distinct atoms, with explicit coherence.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use zetesis_core::{Atom, Model, Sign, Value};
use zetesis_cpu::{CandidateLimits, Candidates, Control};
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature, SourceBundle, admit, admit_bundle_formula, admit_extended, admit_formula,
};

type Record = (Vec<String>, Option<Vec<i64>>);
const SIGNED_ANONYMOUS_PROJECTIONS: [(&str, &str); 2] = [
    ("p(1). #show x : not -p(_).", "p(1)"),
    ("-p(1). #show x : not not -p(_).", "-p(1)"),
];

fn cases() -> Vec<Json> {
    include_str!("fixtures/strong-negation.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn input(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

// This evaluator implements the finite definition independently of the
// production oracle, reduct mask and SAT search. Signed atoms have ordinary
// distinct indices; coherence must therefore be present in the supplied roots.
fn values(theory: &Theory, mask: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut result = Vec::new();
    for (index, node) in theory.nodes().iter().enumerate() {
        let value = match *node {
            Node::False => false,
            Node::Atom(atom) => mask & (1 << atom) != 0,
            Node::And(left, right) => result[left] && result[right],
            Node::Or(left, right) => result[left] || result[right],
            Node::Implies(left, right) => !result[left] || result[right],
        };
        result.push(value && frozen.is_none_or(|outer| outer[index]));
    }
    result
}

fn holds(theory: &Theory, values: &[bool]) -> bool {
    theory.roots().iter().all(|&root| values[root])
}

fn stable_models(input: &AdmittedFormula) -> Vec<Model> {
    assert!(
        input.atoms().len() <= 12,
        "bounded independent minimality check"
    );
    let mut result = Vec::new();
    for mask in 0..1_usize << input.atoms().len() {
        let outer = values(input.theory(), mask, None);
        if !holds(input.theory(), &outer) {
            continue;
        }
        let mut subset = mask;
        let mut countermodel = false;
        while subset != 0 {
            subset = (subset - 1) & mask;
            if holds(
                input.theory(),
                &values(input.theory(), subset, Some(&outer)),
            ) {
                countermodel = true;
                break;
            }
        }
        if !countermodel {
            result.push(Model::new(
                input
                    .atoms()
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, atom)| atom.clone()),
            ));
        }
    }
    result
}

// Full-model records do not use the production output renderer. Numeric minus
// belongs to a value; a predicate sign is serialized before its name.
fn atom_text(atom: &Atom) -> String {
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let mut text = format!("{sign}{}", atom.predicate().name());
    if !atom.values().is_empty() {
        let args: Vec<_> = atom
            .values()
            .iter()
            .map(|value| match value {
                Value::Number(number) => number.to_string(),
                Value::String(value) => serde_json::to_string(value).unwrap(),
                Value::Symbol(value) => value.clone(),
                Value::Infimum => "#inf".into(),
                Value::Supremum => "#sup".into(),
                Value::Structured(value) => value.to_string(),
            })
            .collect();
        text.push('(');
        text.push_str(&args.join(","));
        text.push(')');
    }
    text
}

fn record(input: &AdmittedFormula, model: &Model, display: bool) -> Record {
    let mut atoms: Vec<_> = if display {
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
        // Display fixtures contain no quoted/compound terms. The quoted scalar
        // fixture instead uses the independent full-model serializer above.
        assert!(!rendered.text().contains(['"', '\\']));
        rendered
            .text()
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect()
    } else {
        model.atoms().iter().map(atom_text).collect()
    };
    atoms.sort();
    let evaluated = zetesis_objective::evaluate(
        input.objectives(),
        model,
        zetesis_objective::Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let score = evaluated.score();
    let costs = score
        .is_present()
        .then(|| score.costs().iter().map(|&(_, cost)| cost).collect());
    (atoms, costs)
}

fn best(mut records: Vec<Record>) -> Vec<Record> {
    if let Some(cost) = records.iter().map(|(_, cost)| cost).min().cloned() {
        records.retain(|(_, actual)| actual == &cost);
    }
    records.sort();
    records
}

fn reference(raw: &Json) -> Vec<Record> {
    assert_eq!(raw["Models"]["More"], "no");
    let mut records: Vec<Record> = raw["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        .map(|witness| {
            let mut atoms: Vec<_> = witness["Value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|atom| atom.as_str().unwrap().to_owned())
                .collect();
            atoms.sort();
            let costs = witness["Costs"]
                .as_array()
                .map(|costs| costs.iter().map(|cost| cost.as_i64().unwrap()).collect());
            (atoms, costs)
        })
        .collect();
    assert_eq!(
        raw["Models"]["Number"].as_u64().unwrap(),
        records.len() as u64
    );
    match raw["Result"].as_str().unwrap() {
        "SATISFIABLE" => assert!(records.iter().all(|(_, costs)| costs.is_none())),
        "UNSATISFIABLE" => assert!(records.is_empty()),
        "OPTIMUM FOUND" => {
            let cost: Vec<_> = raw["Models"]["Costs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap())
                .collect();
            let first = records
                .iter()
                .position(|(_, found)| found.as_ref() == Some(&cost))
                .unwrap();
            assert!(
                records[first + 1..].contains(&records[first]),
                "one optN incumbent replay"
            );
            records = records.into_iter().skip(first + 1).collect();
            assert!(
                records
                    .iter()
                    .all(|(_, found)| found.as_ref() == Some(&cost))
            );
            assert_eq!(
                raw["Models"]["Optimal"].as_u64().unwrap(),
                records.len() as u64
            );
        }
        other => panic!("incomplete reference: {other}"),
    }
    records.sort();
    records
}

#[test]
fn independent_reduct_models_match_signed_reference_models_costs_and_displays() {
    let mut count = 0;
    for case in cases().iter().filter(|case| case["native"] == "admit") {
        let source = case["source"].as_str().unwrap();
        let admitted = input(source).unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let models = stable_models(&admitted);
        let mut unique = BTreeSet::new();
        for model in &models {
            assert!(
                unique.insert(model.atoms().iter().cloned().collect::<BTreeSet<_>>()),
                "unique full models"
            );
            for atom in model.atoms() {
                assert!(
                    !model.atoms().iter().any(|other| {
                        atom.predicate().sign() != other.predicate().sign()
                            && atom.predicate().name() == other.predicate().name()
                            && atom.values() == other.values()
                    }),
                    "coherence is a semantic acceptance condition"
                );
            }
        }
        let actual = best(
            models
                .iter()
                .map(|model| record(&admitted, model, case["record_scope"] == "display"))
                .collect(),
        );
        assert_eq!(
            actual,
            reference(&case["reference"]),
            "{}: {source}",
            case["name"]
        );
        count += 1;
    }
    assert_eq!(count, 80);
}

#[test]
fn coherence_and_signed_choices_match_a_manual_theory_in_every_frozen_world() {
    let input = input("{p;-p}.").unwrap();
    assert_eq!(input.atoms().len(), 2);
    let positive = input
        .atoms()
        .iter()
        .position(|atom| atom.predicate().sign() == Sign::Positive)
        .unwrap();
    let negative = input
        .atoms()
        .iter()
        .position(|atom| atom.predicate().sign() == Sign::Negative)
        .unwrap();
    assert_ne!(positive, negative);
    let expected = Theory::new(
        2,
        vec![
            Node::False,
            Node::Atom(positive),
            Node::Atom(negative),
            Node::Implies(1, 0),
            Node::Implies(2, 0),
            Node::Or(1, 3),
            Node::Or(2, 4),
            Node::And(1, 2),
            Node::Implies(7, 0),
        ],
        vec![5, 6, 8],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    for candidate in 0..4 {
        let actual_mask = values(input.theory(), candidate, None);
        let expected_mask = values(&expected, candidate, None);
        assert_eq!(
            holds(input.theory(), &actual_mask),
            holds(&expected, &expected_mask)
        );
        for inner in 0..4 {
            assert_eq!(
                holds(
                    input.theory(),
                    &values(input.theory(), inner, Some(&actual_mask))
                ),
                holds(&expected, &values(&expected, inner, Some(&expected_mask))),
                "M={candidate}, J={inner}"
            );
        }
    }
}

#[test]
fn ordinary_and_extended_closure_routes_enforce_the_same_signed_coherence() {
    for source in [
        "-p.",
        "p.-p.",
        "p(1).-p(2).",
        "p:-not -p.-p:-not p.",
        "{-p}.p.",
        "-p(1).q(X):- -p(X).",
    ] {
        let expected: BTreeSet<_> = stable_models(&input(source).unwrap())
            .into_iter()
            .map(|model| model.atoms().iter().cloned().collect::<BTreeSet<_>>())
            .collect();
        for admitted in [
            admit(source.into(), AdmissionOptions::default()).unwrap(),
            admit_extended(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
            )
            .unwrap(),
        ] {
            let mut actual = BTreeSet::new();
            for seed in Candidates::new(
                admitted.program(),
                CandidateLimits::default(),
                Control::default(),
            ) {
                let checked = zetesis_cpu::check(
                    admitted.program(),
                    &seed.unwrap(),
                    zetesis_cpu::Limits::default(),
                    &Control::default(),
                )
                .unwrap();
                if checked.accepted() {
                    assert!(
                        actual.insert(
                            checked
                                .closure()
                                .atoms()
                                .iter()
                                .cloned()
                                .collect::<BTreeSet<_>>()
                        )
                    );
                }
            }
            assert_eq!(actual, expected, "{source}");
        }
    }
}

fn refusal(error: &FormulaFailure, expected: &str) -> bool {
    match error {
        FormulaFailure::UnsafeVariable { .. } => expected == "UnsafeVariable",
        FormulaFailure::Expansion(ExpansionFailure::Evaluation {
            error: themelios_program::term::EvalError::Undefined,
            ..
        }) => expected == "Undefined",
        FormulaFailure::Expansion(ExpansionFailure::Evaluation {
            error: themelios_program::term::EvalError::NotGround { .. },
            ..
        }) => expected == "NotGround",
        FormulaFailure::Observation { error } => {
            expected == "ObservationUnsafeVariable"
                && error.kind()
                    == &zetesis_themelios::observation::ErrorKind::Unsupported(
                        zetesis_themelios::observation::Feature::UnsafeVariable,
                    )
        }
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
            feature,
            ..
        })) => {
            let expected = match expected {
                "Term" => ProfileFeature::Term,
                _ => return false,
            };
            *feature == expected
        }
        _ => false,
    }
}

#[test]
fn signed_atoms_do_not_broaden_unsafe_or_unsupported_value_profiles() {
    let mut count = 0;
    for case in cases().iter().filter(|case| case["native"] != "admit") {
        // Preserve the original fixture's admission annotation and reference
        // capture. Current objective contracts check the complete scored family.
        if matches!(
            case["native"].as_str(),
            Some("ObjectiveDisjunctionDependency" | "ObjectiveNegativeDependency")
        ) {
            objective_boundaries::check(case["source"].as_str().unwrap());
            continue;
        }
        // These exact legacy sources are the explicit native projection
        // extension checked below; other unsafe observation forms remain refusals.
        if SIGNED_ANONYMOUS_PROJECTIONS
            .iter()
            .any(|(source, _)| case["source"] == *source)
        {
            continue;
        }
        let error = input(case["source"].as_str().unwrap()).unwrap_err();
        assert!(
            refusal(&error, case["native"].as_str().unwrap()),
            "{}: {error:?}",
            case["name"]
        );
        assert!(!error.diagnostics().is_empty());
        count += 1;
    }
    assert_eq!(count, 10);
}
#[path = "support/objective_boundaries.rs"]
mod objective_boundaries;

#[test]
fn signed_anonymous_projection_has_the_declared_model_view() {
    for (source, atom) in SIGNED_ANONYMOUS_PROJECTIONS {
        let case = cases()
            .into_iter()
            .find(|case| case["source"] == source)
            .unwrap();
        let source = case["source"].as_str().unwrap();
        let admitted = input(source).unwrap();
        let original = input(&format!("{atom}.")).unwrap();
        assert_eq!(admitted.source().text(), source);
        assert_eq!(admitted.atoms(), original.atoms());
        assert_eq!(admitted.theory().nodes(), original.theory().nodes());
        assert_eq!(admitted.theory().roots(), original.theory().roots());
        assert_eq!(admitted.formula_origins(), original.formula_origins());
        let views: Vec<_> = stable_models(&admitted)
            .iter()
            .map(|model| {
                (
                    record(&admitted, model, false),
                    record(&admitted, model, true),
                )
            })
            .collect();
        // Keep full identity paired with its display and preserve occurrences.
        assert_eq!(
            views,
            vec![(
                (vec![atom.into()], None),
                (vec![atom.into(), "x".into()], None),
            )]
        );
    }
}

#[test]
fn coherence_roots_are_bounded_and_keep_both_original_source_locations() {
    let source = "p.\n-p.";
    let admitted = input(source).unwrap();
    assert_eq!(admitted.atoms().len(), 2, "signs never alias");
    let origins: BTreeSet<_> = admitted
        .formula_origins()
        .iter()
        .filter(|origins| origins.len() >= 2)
        .flatten()
        .map(|location| admitted.source().slice(location.span).unwrap())
        .collect();
    assert_eq!(origins, BTreeSet::from(["p.", "-p."]));
    let roots = admitted.theory().roots().len();
    for (limit, succeeds) in [(roots, true), (roots - 1, false)] {
        let mut limits = FormulaLimits::default();
        limits.theory.max_roots = limit;
        let result = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        );
        if succeeds {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Roots,
                    ..
                })
            ));
        }
    }
    let mut options = AdmissionOptions::default();
    options.core_limits.max_templates = 2;
    assert!(
        matches!(
            admit(source.into(), options),
            Err(AdmissionFailure::Core { .. })
        ),
        "coherence must fit the core template ceiling"
    );
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-strong-negation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn opposite_signs_across_bundle_files_share_coherence_and_original_provenance() {
    let directory = Directory::new();
    fs::write(
        directory.0.join("entry.lp"),
        "#include \"negative.lp\". p(1).",
    )
    .unwrap();
    fs::write(directory.0.join("negative.lp"), "-p(1).").unwrap();
    let bundle = SourceBundle::load(directory.0.join("entry.lp"), BundleLimits::default()).unwrap();
    let input = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let joined: Vec<_> = input
        .formula_origins()
        .iter()
        .filter(|origins| {
            origins
                .iter()
                .map(|location| location.source)
                .collect::<BTreeSet<_>>()
                .len()
                == 2
        })
        .collect();
    assert!(
        !joined.is_empty(),
        "coherence preserves both original source identities"
    );
    for origins in joined {
        let text: BTreeSet<_> = origins
            .iter()
            .map(|origin| {
                input
                    .bundle()
                    .get(origin.source)
                    .unwrap()
                    .source()
                    .slice(origin.span)
                    .unwrap()
            })
            .collect();
        assert_eq!(text, BTreeSet::from(["p(1).", "-p(1)."]));
    }
    for mask in 0..1 << input.atoms().len() {
        assert!(
            !holds(input.theory(), &values(input.theory(), mask, None)),
            "conflicting facts"
        );
    }
}

fn clingo(source: &str) -> Json {
    let directory = Directory::new();
    let source_path = directory.0.join("source.lp");
    fs::write(&source_path, source).unwrap();
    let out = directory.0.join("stdout");
    let err = directory.0.join("stderr");
    let mut child = Command::new(std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into()))
        .args(["--models=0", "--outf=2", "--opt-mode=optN", "-"])
        .stdin(File::open(source_path).unwrap())
        .stdout(File::create(&out).unwrap())
        .stderr(File::create(&err).unwrap())
        .spawn()
        .expect("external clingo executable");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let oversized = [&out, &err]
            .iter()
            .any(|path| fs::metadata(path).unwrap().len() > 65_536);
        if oversized || Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("bounded reference incomplete; no compatibility result");
        }
        if let Some(status) = child.try_wait().unwrap() {
            assert!(matches!(status.code(), Some(0 | 10 | 20 | 30 | 65)));
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        [&out, &err]
            .iter()
            .all(|path| fs::metadata(path).unwrap().len() <= 65_536)
    );
    serde_json::from_slice(&fs::read(out).unwrap()).unwrap()
}

#[test]
#[ignore = "requires external clingo; independent bounded full reference replay"]
fn fresh_clingo_replays_signed_models_objectives_and_unsafe_diagnostics() {
    for case in cases() {
        let fresh = clingo(case["source"].as_str().unwrap());
        assert_eq!(fresh["Solver"], case["reference"]["Solver"]);
        assert_eq!(
            fresh["Result"], case["reference"]["Result"],
            "{}",
            case["name"]
        );
        if fresh["Result"] == "UNKNOWN" {
            assert!(matches!(
                case["native"].as_str(),
                Some("UnsafeVariable" | "ObservationUnsafeVariable" | "NotGround")
            ));
            assert_eq!(fresh["Models"]["Number"], 0);
        } else {
            assert_eq!(
                reference(&fresh),
                reference(&case["reference"]),
                "{}",
                case["name"]
            );
        }
    }
}
