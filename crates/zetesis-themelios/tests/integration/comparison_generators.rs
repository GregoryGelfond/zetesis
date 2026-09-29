//! Comparison generators retain the complete guards and original reduct semantics.
//! Exact external records complement a separate finite definition evaluator.

use std::collections::BTreeSet;
use std::fs::{self};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_clingo_support as oracle;
use zetesis_core::Sign;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature, SourceBundle, admit_bundle_formula, admit_formula,
};

type Models = BTreeSet<BTreeSet<String>>;
const SOURCE: SourceId = SourceId::new(101);

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
    zetesis_test_support::fixtures::COMPARISON_GENERATORS
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

fn atom_text<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
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
        .map(|value| match value.descriptor() {
            zetesis_core::ValueNodeRef::Number(number) => number.to_string(),
            zetesis_core::ValueNodeRef::Symbol(value) => value.to_owned(),
            zetesis_core::ValueNodeRef::String(value) => serde_json::to_string(value).unwrap(),
            zetesis_core::ValueNodeRef::Infimum => "#inf".into(),
            zetesis_core::ValueNodeRef::Supremum => "#sup".into(),
            zetesis_core::ValueNodeRef::Function { .. }
            | zetesis_core::ValueNodeRef::Tuple { .. } => value.to_string(),
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
                    .map(|index| atom_text(input.atoms().at(index).unwrap()))
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
            "zetesis-comparison-generators-{}-{}",
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
    let exits: &[i32] = if valid { &oracle::DECIDED } else { &[65] };
    let run = oracle::run_accepting(
        source,
        &[
            "--models=0",
            "--outf=2",
            "--parallel-mode=1",
            "--opt-mode=enum",
        ],
        exits,
        oracle::Limits {
            timeout: Duration::from_secs(3),
            max_output_bytes: 2 * 65_536,
        },
    );
    let diagnostics = String::from_utf8_lossy(run.stderr());
    // A refused source must be refused for its unsafe variables.
    assert!(
        valid || diagnostics.contains("unsafe variables"),
        "{diagnostics}"
    );
    oracle::json(&run)
}

fn refusal(error: &FormulaFailure, expected: &str) {
    assert!(!error.diagnostics().is_empty());
    assert!(
        error
            .diagnostics()
            .iter()
            .all(|diagnostic| { diagnostic.primary().location.source == SOURCE })
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
        "Term" => assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::Term,
                    ..
                }))
            ),
            "{error}"
        ),
        other => panic!("unreviewed comparison-generator refusal {other}"),
    }
}

#[test]
fn exact_sources_have_complete_models_or_distinct_reviewed_refusals() {
    let rows = cases();
    assert_eq!(rows.len(), 137);
    let mut admitted = 0;
    let mut models = 0;
    let mut valid_refused = 0;
    let mut unsafe_sources = 0;
    for row in rows {
        let source = row["source"].as_str().unwrap();
        // Retain the historical classification and original oracle record.
        // A mixed family now omits its zero-divisor instance with a warning.
        let mixed_arithmetic = row["name"] == "double_runtime_undefined";
        if row["native"] == "admit" || mixed_arithmetic {
            assert_eq!(row["valid"], true);
            let program = input(source).unwrap_or_else(|error| panic!("{}: {error}", row["name"]));
            assert_eq!(program.source().text(), source);
            if mixed_arithmetic {
                assert_eq!(program.warnings().len(), 1);
            }
            let expected = expected(&row);
            assert_eq!(exhaustive(&program), expected, "{}", row["name"]);
            assert_eq!(native(&program), expected, "{}", row["name"]);
            admitted += 1;
            models += expected.len();
        } else {
            let result = input(source);
            let error = result.unwrap_err();
            refusal(&error, row["native"].as_str().unwrap());
            valid_refused += usize::from(row["valid"] == true);
            unsafe_sources += usize::from(row["valid"] == false);
        }
    }
    assert_eq!(
        (admitted, models, valid_refused, unsafe_sources),
        (121, 140, 3, 13)
    );
}

fn remap(
    mask: usize,
    from: zetesis_core::catalog::Atoms<'_>,
    to: zetesis_core::catalog::Atoms<'_>,
) -> usize {
    from.iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .fold(0, |bits, (_, atom)| {
            bits | (1 << to.iter().position(|other| atom == other).unwrap())
        })
}

#[test]
fn finite_generated_instances_preserve_every_original_and_frozen_interpretation() {
    for (source, specified) in [
        (
            "{a;b}.p(X):-a,not not X=1<2.b:-p(1).",
            "{a;b}.p(1):-a.b:-p(1).",
        ),
        ("{a}.p(X):-0<X<3,not a.", "{a}.p(1):-not a.p(2):-not a."),
        ("{p(X):not not X=1<2}.", "{p(1)}."),
        (
            "{a}.n(N):-N=#count{X:not not X=1<2,a}.",
            "{a}.n(N):-N=#count{1:a}.",
        ),
        (
            "{a;p(1);p(2)}.q:-p(X):0<X<3,not a.",
            "{a;p(1);p(2)}.q:-p(1):not a;p(2):not a.",
        ),
        ("{p(1)}.-p(X):-not not X=1<2.", "{p(1)}.-p(1)."),
    ] {
        let generated = input(source).unwrap();
        let explicit = input(specified).unwrap();
        assert_eq!(
            generated.atoms().iter().collect::<BTreeSet<_>>(),
            explicit.atoms().iter().collect()
        );
        for outer in 0..1 << generated.atoms().len() {
            let original = values(generated.theory(), outer, None);
            let specified_original = values(
                explicit.theory(),
                remap(outer, generated.atoms(), explicit.atoms()),
                None,
            );
            assert_eq!(
                holds(generated.theory(), &original),
                holds(explicit.theory(), &specified_original),
                "{source}, M={outer}"
            );
            for inner in 0..1 << generated.atoms().len() {
                let generated_reduct = values(generated.theory(), inner, Some(&original));
                let specified_reduct = values(
                    explicit.theory(),
                    remap(inner, generated.atoms(), explicit.atoms()),
                    Some(&specified_original),
                );
                assert_eq!(
                    holds(generated.theory(), &generated_reduct),
                    holds(explicit.theory(), &specified_reduct),
                    "{source}, M={outer}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn exact_integer_domain_bounds_and_empty_endpoints_do_not_wrap() {
    let source = "p(X):-0<X<4.";
    let result = admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_assignment_values: 2,
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(FormulaFailure::Limit { resource: FormulaResource::AssignmentValues, limit: 2, observed: 3, location }) if location.source == SOURCE)
    );
    let result = admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_assignment_values: 3,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(native(&result), native(&input("p(1..3).").unwrap()));
    for source in [
        "p(X):-3<X<0.",
        "p(X):-2147483647<X<=2147483647.",
        "p(X):-(-2147483647-1)<=X<(-2147483647-1).",
    ] {
        let program = admit_formula(
            source.into(),
            options(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_assignment_values: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            native(&program),
            BTreeSet::from([BTreeSet::new()]),
            "{source}"
        );
    }
}

#[test]
fn interrupted_generator_construction_never_returns_a_partial_program() {
    let source = "{p(X):not not 0<X<4}.q:-p(X):not not 0<X<4.";
    let mut complete = None;
    for cap in 0..150 {
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
                complete = Some((cap, program));
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
            Err(other) => panic!("unexpected resource result: {other}"),
        }
    }
    let (cap, program) = complete.expect("bounded complete construction");
    assert!(cap > 3);
    assert_eq!(native(&program).len(), 8);
    assert_eq!(native(&program), exhaustive(&program));
    assert_eq!(
        native(&program),
        native(&input("{p(1);p(2);p(3)}.q:-p(1),p(2),p(3).").unwrap())
    );
}

#[test]
fn generated_include_rules_keep_original_identity_and_display_selection() {
    let directory = Directory::new();
    let root = directory.0.join("root.lp");
    let rules = directory.0.join("rules.lp");
    let source = "#include \"rules.lp\". #show p/1.";
    let rule = "p(X):-not not X=1<2.";
    fs::write(&root, source).unwrap();
    fs::write(&rules, rule).unwrap();
    let program = admit_bundle_formula(
        SourceBundle::load(&root, BundleLimits::default()).unwrap(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let rule_source = program
        .bundle()
        .sources()
        .iter()
        .find(|source| source.source().text() == rule)
        .unwrap()
        .id();
    assert!(
        program
            .formula_origins()
            .iter()
            .flatten()
            .any(|location| location.source == rule_source)
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
        .map(|index| program.atoms().at(index).unwrap())
        .filter(|atom| program.metadata().output().includes(*atom))
        .map(atom_text)
        .collect();
    assert_eq!(shown, ["p(1)"]);
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
#[ignore = "requires external clingo; exact generators and explicit validity boundaries"]
fn comparison_generators_match_clingo() {
    let mut valid = 0;
    let mut unsafe_sources = 0;
    for row in cases() {
        let source = row["source"].as_str().unwrap();
        let actual = external(source, row["valid"] == true);
        if row["valid"] != true {
            assert_eq!(actual["Result"], "UNKNOWN");
            refusal(&input(source).unwrap_err(), row["native"].as_str().unwrap());
            unsafe_sources += 1;
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
        assert!(witnesses.iter().all(|witness| witness["Costs"].is_null()));
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
        // Keep the historical fixture and fresh oracle checks unchanged;
        // the mixed-family policy now admits this row with one warning.
        let mixed_arithmetic = row["name"] == "double_runtime_undefined";
        if row["native"] == "admit" || mixed_arithmetic {
            let program = input(source).unwrap_or_else(|error| panic!("{}: {error}", row["name"]));
            if mixed_arithmetic {
                assert_eq!(program.warnings().len(), 1);
            }
            assert_eq!(native(&program), models, "{}", row["name"]);
        } else {
            refusal(&input(source).unwrap_err(), row["native"].as_str().unwrap());
        }
    }
    assert_eq!((valid, unsafe_sources), (124, 13));
}
