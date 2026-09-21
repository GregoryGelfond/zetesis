//! Scoped universal body conditionals retain their original implication reducts.
//! Exact external records complement a separate finite definition evaluator.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_core::{Atom, Sign, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature, SourceBundle, admit_bundle_formula, admit_formula,
};

type Models = BTreeSet<BTreeSet<String>>;
const SOURCE: SourceId = SourceId::new(97);

fn options() -> AdmissionOptions {
    AdmissionOptions {
        source_id: SOURCE,
        ..Default::default()
    }
}

fn input(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}

fn cases() -> Vec<Json> {
    include_str!("fixtures/conditional-body.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn expected(row: &Json) -> Models {
    row["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(json_model)
        .collect()
}

fn json_model(values: &Json) -> BTreeSet<String> {
    let atoms = values.as_array().unwrap();
    let result: BTreeSet<_> = atoms
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(result.len(), atoms.len(), "full atom identities are unique");
    result
}

fn atom_text(atom: &Atom) -> String {
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let name = format!("{sign}{}", atom.predicate().name());
    if atom.values().is_empty() {
        return name;
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Number(number) => number.to_string(),
            Value::Symbol(value) => value.clone(),
            Value::String(value) => serde_json::to_string(value).unwrap(),
            Value::Infimum => "#inf".into(),
            Value::Supremum => "#sup".into(),
            Value::Structured(value) => value.to_string(),
        })
        .collect();
    format!("{name}({})", values.join(","))
}

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

fn exhaustive(input: &AdmittedFormula) -> Models {
    assert!(input.atoms().len() <= 12, "bounded independent carrier");
    let mut result = BTreeSet::new();
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
            assert!(
                result.insert(
                    input
                        .atoms()
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| mask & (1 << index) != 0)
                        .map(|(_, atom)| atom_text(atom))
                        .collect()
                )
            );
        }
    }
    result
}

fn native(input: &AdmittedFormula) -> Models {
    let mut search = zetesis_sat::StableModels::new(
        input.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut result = BTreeSet::new();
    for model in search.by_ref() {
        assert!(
            result.insert(
                model
                    .unwrap()
                    .atoms()
                    .map(|index| atom_text(&input.atoms()[index]))
                    .collect()
            )
        );
    }
    assert!(search.exhausted(), "complete original-theory enumeration");
    result
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-conditional-body-{}-{}",
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

fn external(source: &str, valid: bool) -> Json {
    let directory = Directory::new();
    let source_path = directory.0.join("source.lp");
    let out = directory.0.join("stdout");
    let err = directory.0.join("stderr");
    fs::write(&source_path, source).unwrap();
    let mut child = Command::new(std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into()))
        .args([
            "--models=0",
            "--outf=2",
            "--parallel-mode=1",
            "--opt-mode=enum",
            "-",
        ])
        .stdin(File::open(source_path).unwrap())
        .stdout(File::create(&out).unwrap())
        .stderr(File::create(&err).unwrap())
        .spawn()
        .expect("independently installed clingo");
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
            let diagnostics = fs::read_to_string(&err).unwrap();
            assert!(
                if valid {
                    matches!(status.code(), Some(10 | 20 | 30))
                } else {
                    status.code() == Some(65) && diagnostics.contains("unsafe variables")
                },
                "{diagnostics}"
            );
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

fn refusal(error: &FormulaFailure, expected: &str) {
    assert!(!error.diagnostics().is_empty());
    assert!(
        error
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
    );
    match expected {
        "UnsafeVariable" => assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{error}"
        ),
        "Evaluation" => assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{error}"
        ),
        "AggregateAssignment" => assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::AggregateAssignment,
                    ..
                }))
            ),
            "{error}"
        ),
        other => panic!("unreviewed conditional refusal {other}"),
    }
}

#[test]
fn original_sources_match_complete_models_or_reviewed_profile_refusals() {
    // The original generative_double_condition source is now admitted by the
    // finite comparison binding plan. The negative anonymous consequent now
    // admits a complete projection. These records describe complete model families.
    let cases = cases();
    assert_eq!(cases.len(), 91);
    let mut admitted = 0;
    let mut refused = 0;
    let mut models = 0;
    let mut valid_refused = 0;
    for row in cases {
        let source = row["source"].as_str().unwrap();
        let result = input(source);
        if row["native"] == "admit" {
            assert_eq!(row["valid"], true);
            let program = result.unwrap_or_else(|error| panic!("{}: {error}", row["name"]));
            assert_eq!(program.source().text(), source);
            let expected = expected(&row);
            assert_eq!(exhaustive(&program), expected, "{}", row["name"]);
            assert_eq!(native(&program), expected, "{}", row["name"]);
            admitted += 1;
            models += expected.len();
        } else {
            let error = result.unwrap_err();
            refusal(&error, row["native"].as_str().unwrap());
            refused += 1;
            valid_refused += usize::from(row["valid"] == true);
        }
    }
    assert_eq!((admitted, refused, models, valid_refused), (85, 6, 229, 2));
}

/// A separate tree definition, with no production DAG construction or reduct API.
#[derive(Clone)]
enum Formula {
    False,
    Atom(String),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Implies(Box<Self>, Box<Self>),
}
impl Formula {
    fn atom(name: impl Into<String>) -> Self {
        Self::Atom(name.into())
    }
    fn implies(left: Self, right: Self) -> Self {
        Self::Implies(Box::new(left), Box::new(right))
    }
    fn and(left: Self, right: Self) -> Self {
        Self::And(Box::new(left), Box::new(right))
    }
    fn truth() -> Self {
        Self::implies(Self::False, Self::False)
    }
    fn sign(self, count: usize) -> Self {
        (0..count).fold(self, |value, _| Self::implies(value, Self::False))
    }
    fn original(&self, world: &BTreeSet<String>) -> bool {
        match self {
            Self::False => false,
            Self::Atom(atom) => world.contains(atom),
            Self::And(left, right) => left.original(world) && right.original(world),
            Self::Or(left, right) => left.original(world) || right.original(world),
            Self::Implies(left, right) => !left.original(world) || right.original(world),
        }
    }
    fn frozen(&self, outer: &BTreeSet<String>, inner: &BTreeSet<String>) -> bool {
        if !self.original(outer) {
            return false;
        }
        match self {
            Self::False => false,
            Self::Atom(atom) => inner.contains(atom),
            Self::And(left, right) => left.frozen(outer, inner) && right.frozen(outer, inner),
            Self::Or(left, right) => left.frozen(outer, inner) || right.frozen(outer, inner),
            Self::Implies(left, right) => !left.frozen(outer, inner) || right.frozen(outer, inner),
        }
    }
}
fn world(atoms: &[Atom], mask: usize) -> BTreeSet<String> {
    atoms
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, atom)| atom_text(atom))
        .collect()
}
fn original_rule(program: &AdmittedFormula, head: &str) -> usize {
    let atom = program
        .atoms()
        .iter()
        .position(|atom| atom_text(atom) == head)
        .unwrap();
    let matches: Vec<_> = program
        .theory()
        .roots()
        .iter()
        .copied()
        .filter(|root| {
            let Node::Implies(_, consequent) = program.theory().nodes()[*root] else {
                return false;
            };
            program.theory().nodes()[consequent] == Node::Atom(atom)
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "one original implication; producer guards are separate"
    );
    matches[0]
}

#[test]
fn finite_conditional_rule_preserves_all_original_and_frozen_interpretation_pairs() {
    for (head_count, head_sign) in ["", "not ", "not not "].into_iter().enumerate() {
        for (condition_count, condition_sign) in ["", "not ", "not not "].into_iter().enumerate() {
            let source = format!(
                "d(1..2). {{p(1);p(2);r(1);r(2)}}. q:-{head_sign}p(X):d(X),{condition_sign}r(X)."
            );
            let program = input(&source).unwrap();
            let root = original_rule(&program, "q");
            let mut universal = Formula::truth();
            for value in [1, 2] {
                let condition = Formula::and(
                    Formula::atom(format!("d({value})")),
                    Formula::atom(format!("r({value})")).sign(condition_count),
                );
                let consequent = Formula::atom(format!("p({value})")).sign(head_count);
                universal = Formula::and(universal, Formula::implies(condition, consequent));
            }
            let specified = Formula::implies(universal, Formula::atom("q"));
            assert_eq!(program.atoms().len(), 7);
            for outer in 0..1 << program.atoms().len() {
                let original = values(program.theory(), outer, None);
                let outer_world = world(program.atoms(), outer);
                assert_eq!(
                    original[root],
                    specified.original(&outer_world),
                    "{source}, M={outer}"
                );
                for inner in 0..1 << program.atoms().len() {
                    assert_eq!(
                        values(program.theory(), inner, Some(&original))[root],
                        specified.frozen(&outer_world, &world(program.atoms(), inner)),
                        "{source}, M={outer}, J={inner}"
                    );
                }
            }
        }
    }
}

#[test]
fn classical_implication_rewrite_would_lose_the_recursive_answer() {
    let program = input("p:-p:p.").unwrap();
    assert_eq!(
        native(&program),
        BTreeSet::from([BTreeSet::from(["p".to_owned()])])
    );
    let p = Formula::atom("p");
    let correct = Formula::implies(Formula::implies(p.clone(), p.clone()), p.clone());
    let classical = Formula::implies(
        Formula::Or(Box::new(p.clone().sign(1)), Box::new(p.clone())),
        p,
    );
    let outer = BTreeSet::from(["p".to_owned()]);
    let inner = BTreeSet::new();
    assert_eq!(correct.original(&outer), classical.original(&outer));
    assert!(!correct.frozen(&outer, &inner));
    assert!(classical.frozen(&outer, &inner));
    let frozen = values(program.theory(), 1, None);
    assert!(!values(program.theory(), 0, Some(&frozen))[original_rule(&program, "p")]);
}

#[test]
fn partial_local_iteration_never_certifies_vacuity_and_exact_caps_are_inclusive() {
    let source = "d(1..3).p(1..3).q:-p(X):d(X).";
    let mut first = None;
    for cap in 0..100 {
        let result = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_substitutions: cap,
                ..Default::default()
            },
        );
        match result {
            Ok(program) => {
                first = Some((cap, program));
                break;
            }
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Substitutions,
                limit,
                observed,
                location,
            }) => {
                assert_eq!(limit, u128::from(cap));
                assert_eq!(observed, limit + 1);
                assert_eq!(location.source, SOURCE);
            }
            Err(other) => panic!("unexpected resource refusal {other}"),
        }
    }
    let (cap, program) = first.expect("small finite complete instance set");
    assert!(
        cap > 3,
        "outer completion and local substitutions both consume budget"
    );
    assert!(native(&program).iter().all(|model| model.contains("q")));
    assert_eq!(native(&program), exhaustive(&program));
    assert!(matches!(
        admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_substitutions: cap - 1,
                ..Default::default()
            }
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Substitutions,
            ..
        })
    ));
    for (source, limit) in [("d(1).p(1).q:-p(X):d(X),X=1.", 1), ("q:-p(X,Y):d(X,Y).", 1)] {
        let options = if source.contains("X,Y") {
            AdmissionOptions {
                core_limits: zetesis_core::AdmissionLimits {
                    max_variables_per_template: limit,
                    ..Default::default()
                },
                ..options()
            }
        } else {
            AdmissionOptions {
                max_body_elements: limit,
                ..options()
            }
        };
        let error = admit_formula(
            source.into(),
            options,
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        if source.contains("X,Y") {
            assert!(
                matches!(error, FormulaFailure::Limit { resource: FormulaResource::Variables, limit: 1, observed: 2, location } if location.source == SOURCE)
            );
        } else {
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit { resource: zetesis_themelios::InputLimit::BodyElements, limit: 1, observed: 2, location })) if location.source == SOURCE)
            );
        }
    }
}

#[test]
fn included_conditionals_preserve_original_locations_and_output_selection() {
    let directory = Directory::new();
    let root = directory.0.join("root.lp");
    let rules = directory.0.join("rules.lp");
    let source = "#include \"rules.lp\". d(1..2).p(1..2).#show q/0.";
    let conditional = "q:-p(X):d(X).";
    fs::write(&root, source).unwrap();
    fs::write(&rules, conditional).unwrap();
    let bundle = SourceBundle::load(&root, BundleLimits::default()).unwrap();
    let program = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let rule_id = program
        .bundle()
        .sources()
        .iter()
        .find(|source| source.source().text() == conditional)
        .unwrap()
        .id();
    assert!(
        program
            .formula_origins()
            .iter()
            .flatten()
            .any(|location| location.source == rule_id)
    );
    for location in program.formula_origins().iter().flatten() {
        assert!(
            program
                .bundle()
                .get(location.source)
                .unwrap()
                .source()
                .slice(location.span)
                .is_ok()
        );
    }
    let mut search = zetesis_sat::StableModels::new(
        program.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let model = search.next().unwrap().unwrap();
    let shown: Vec<_> = model
        .atoms()
        .map(|index| &program.atoms()[index])
        .filter(|atom| program.metadata().output().includes(atom))
        .map(atom_text)
        .collect();
    assert_eq!(shown, ["q"]);
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
#[ignore = "requires external clingo; exact conditional sources and complete model replay"]
fn conditional_body_matches_clingo() {
    let mut valid = 0;
    let mut unsafe_source = 0;
    for row in cases() {
        let actual = external(row["source"].as_str().unwrap(), row["valid"] == true);
        if row["valid"] != true {
            assert_eq!(actual["Result"], "UNKNOWN");
            refusal(
                &input(row["source"].as_str().unwrap()).unwrap_err(),
                row["native"].as_str().unwrap(),
            );
            unsafe_source += 1;
            continue;
        }
        valid += 1;
        assert_eq!(actual["Models"]["More"], "no");
        let witnesses: Vec<_> = actual["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .collect();
        let models: Models = witnesses
            .iter()
            .map(|witness| json_model(&witness["Value"]))
            .collect();
        assert_eq!(models.len(), witnesses.len());
        assert_eq!(
            actual["Models"]["Number"].as_u64(),
            Some(witnesses.len() as u64)
        );
        assert_eq!(models, expected(&row), "{}", row["name"]);
        if row["native"] == "admit" {
            assert!(witnesses.iter().all(|witness| witness["Costs"].is_null()));
            assert_eq!(
                native(&input(row["source"].as_str().unwrap()).unwrap()),
                models,
                "{}",
                row["name"]
            );
        } else {
            refusal(
                &input(row["source"].as_str().unwrap()).unwrap_err(),
                row["native"].as_str().unwrap(),
            );
        }
    }
    assert_eq!((valid, unsafe_source), (87, 4));
}
