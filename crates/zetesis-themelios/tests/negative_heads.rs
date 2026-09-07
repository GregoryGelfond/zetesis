//! Default-negated heads preserve original formulas and only positive support.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use zetesis_core::{Atom, Sign, Value};
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits,
    FormulaResource, ProfileFeature, SourceBundle, admit_bundle_formula, admit_formula,
};

type Names = BTreeSet<String>;
type Models = BTreeSet<Names>;

fn cases() -> Vec<Json> {
    serde_json::from_str(include_str!("fixtures/negative-heads.json")).unwrap()
}
fn input(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    limited(
        source,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}
fn limited(
    source: &str,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(source.into(), options, expansion, limits)
}
fn name(atom: &Atom) -> String {
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let base = format!("{sign}{}", atom.predicate().name());
    match atom.values() {
        [] => base,
        [Value::Number(number)] => format!("{base}({number})"),
        _ => panic!("fixture has nullary or numeric unary atoms: {atom:?}"),
    }
}
fn names(value: &Json) -> Names {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect()
}
fn expected(value: &Json) -> Models {
    value.as_array().unwrap().iter().map(names).collect()
}
// Independent topological evaluation: every subtree false in M is falsum in
// F^M. No production evaluator, reduct mask, SAT search or subset enumeration
// helper is used to decide these finite stable models.
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
fn selected(admitted: &AdmittedFormula, mask: usize) -> Names {
    admitted
        .atoms()
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, atom)| name(atom))
        .collect()
}
fn complete(admitted: &AdmittedFormula) -> Models {
    assert!(admitted.atoms().len() <= 6, "tiny exhaustive carrier");
    let mut result = Models::new();
    for mask in 0..1_usize << admitted.atoms().len() {
        let outer = values(admitted.theory(), mask, None);
        if !holds(admitted.theory(), &outer) {
            continue;
        }
        let mut subset = mask;
        let mut countermodel = false;
        while subset != 0 {
            subset = (subset - 1) & mask;
            if holds(
                admitted.theory(),
                &values(admitted.theory(), subset, Some(&outer)),
            ) {
                countermodel = true;
                break;
            }
        }
        if !countermodel {
            assert!(result.insert(selected(admitted, mask)));
        }
    }
    result
}

fn truth(formula: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    if frozen.is_some_and(|outer| !truth(formula, outer, None)) {
        return false;
    }
    if let Some(atom) = formula.as_str() {
        return tested.contains(atom);
    }
    if let Json::Bool(value) = formula {
        return *value;
    }
    let left = truth(&formula[1], tested, frozen);
    let right = truth(&formula[2], tested, frozen);
    match formula[0].as_str().unwrap() {
        "and" => left && right,
        "or" => left || right,
        "imp" => !left || right,
        other => panic!("unknown manual formula {other}"),
    }
}
fn manual_holds(theory: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    theory["roots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|root| truth(root, tested, frozen))
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-negative-heads-{}-{}",
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

fn clingo(source: &str) -> Models {
    let directory = Directory::new();
    let output = directory.0.join("stdout");
    let errors = directory.0.join("stderr");
    let mut child = Command::new(std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into()))
        .args(["-", "--models=0", "--outf=2"])
        .stdin(Stdio::piped())
        .stdout(File::create(&output).unwrap())
        .stderr(File::create(&errors).unwrap())
        .spawn()
        .expect("external clingo oracle");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if start.elapsed() > Duration::from_secs(5)
            || fs::metadata(&output).unwrap().len() > 1_048_576
            || fs::metadata(&errors).unwrap().len() > 65_536
        {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("bounded clingo capture incomplete");
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
    assert!(fs::metadata(&output).unwrap().len() <= 1_048_576);
    assert!(fs::metadata(&errors).unwrap().len() <= 65_536);
    let raw: Json = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert!(
        raw["Solver"]
            .as_str()
            .unwrap()
            .starts_with("clingo version 5.8.")
    );
    assert!(matches!(
        raw["Result"].as_str().unwrap(),
        "SATISFIABLE" | "UNSATISFIABLE"
    ));
    assert_eq!(raw["Models"]["More"], "no");
    let witnesses: Vec<_> = raw["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(
        raw["Models"]["Number"].as_u64().unwrap(),
        witnesses.len() as u64
    );
    let models: Models = witnesses
        .iter()
        .map(|witness| {
            assert!(witness["Costs"].is_null());
            names(&witness["Value"])
        })
        .collect();
    assert_eq!(
        models.len(),
        witnesses.len(),
        "fixtures show complete models without projection"
    );
    models
}

fn supported(case: &Json, model: &Names) -> bool {
    model.iter().all(|atom| {
        case["support"]
            .get(atom)
            .is_some_and(|antecedent| truth(antecedent, model, None))
    })
}

#[test]
fn original_support_augmentation_and_frozen_formulas_are_distinct_contracts() {
    let mut pairs = 0;
    for case in cases() {
        let source = case["source"].as_str().unwrap();
        let admitted = input(source).unwrap_or_else(|error| panic!("{source}: {error}"));
        let actual_atoms: Names = admitted.atoms().iter().map(name).collect();
        // A positive join with no possible rows may erase a vacuous source rule;
        // compare it separately through full models, not a fabricated DAG carrier.
        if case["name"] == "recursive-body" {
            assert_eq!(complete(&admitted), expected(&case["models"]));
            continue;
        }
        assert_eq!(actual_atoms, names(&case["atoms"]), "{source}: carrier");
        for mask in 0..1_usize << admitted.atoms().len() {
            let model = selected(&admitted, mask);
            let outer = values(admitted.theory(), mask, None);
            let has_support = supported(&case, &model);
            assert_eq!(
                holds(admitted.theory(), &outer),
                manual_holds(&case, &model, None) && has_support,
                "{source}: original M={mask}"
            );
            if !has_support {
                continue;
            }
            // The added double-negated support guards are true in every J when
            // they hold in M. This compares arbitrary J, not just proper subsets.
            for inner in 0..1_usize << admitted.atoms().len() {
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&outer))
                    ),
                    manual_holds(&case, &selected(&admitted, inner), Some(&model)),
                    "{source}: frozen M={mask}, J={inner}"
                );
                pairs += 1;
            }
        }
    }
    assert!(pairs > 300);
}

#[test]
fn complete_signed_models_match_handwritten_theories_and_explicit_expansions() {
    let mut count = 0;
    for case in cases() {
        let predicted = expected(&case["models"]);
        for field in ["source", "expanded"] {
            let admitted = input(case[field].as_str().unwrap()).unwrap();
            assert_eq!(complete(&admitted), predicted, "{}: {field}", case["name"]);
            assert_eq!(
                admitted.formula_origins().len(),
                admitted.theory().roots().len()
            );
            assert!(
                admitted
                    .formula_origins()
                    .iter()
                    .all(|origins| !origins.is_empty())
            );
        }
        count += predicted.len();
    }
    assert_eq!(cases().len(), 27);
    assert_eq!(count, 49);
}

#[test]
#[ignore = "requires external clingo 5.8; bounded original and explicit-expansion captures"]
fn fresh_clingo_preserves_every_complete_negative_head_contract() {
    for case in cases() {
        for field in ["source", "expanded"] {
            assert_eq!(
                clingo(case[field].as_str().unwrap()),
                expected(&case["models"]),
                "{}: {field}",
                case["name"]
            );
        }
    }
}

#[test]
fn polarity_never_binds_variables_or_opens_other_head_profiles() {
    for source in [
        "not a(X)|b.",
        "not not a(X)|b.",
        "not a(X+1)|b.",
        "not a(2..1,X)|b.",
        "d(1).not a(Y)|b(X):-d(X).",
    ] {
        assert!(
            matches!(input(source), Err(FormulaFailure::UnsafeVariable { .. })),
            "{source}"
        );
    }
    for (source, predicted) in [
        ("not a.", ProfileFeature::NegatedHead),
        ("not not a.", ProfileFeature::NegatedHead),
        ("{not a}.", ProfileFeature::NegatedHead),
        ("not a:b|c.", ProfileFeature::ConditionalDisjunction),
        ("not a(1;2)|b.", ProfileFeature::PooledArguments),
        ("#true|b.", ProfileFeature::Head),
        (
            "not a|b.#minimize{1:a}.",
            ProfileFeature::ObjectiveDisjunctionDependency,
        ),
        (
            "not not a|b.c:-a.#minimize{1:c}.",
            ProfileFeature::ObjectiveDisjunctionDependency,
        ),
    ] {
        let error = input(source).expect_err(source);
        assert!(
            matches!(&error, FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Profile { feature, .. })) if *feature == predicted),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn negation_nodes_and_occurrences_obey_exact_resource_boundaries() {
    for source in [
        "not a|not not a|a.",
        "d(1).not a(X+1)|b:-d(X).",
        "not not p(1..2)|q.",
    ] {
        let admitted = input(source).unwrap();
        let mut limits = FormulaLimits::default();
        limits.theory.max_nodes = admitted.theory().nodes().len();
        assert_eq!(
            complete(
                &limited(
                    source,
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    limits
                )
                .unwrap()
            ),
            complete(&admitted)
        );
        limits.theory.max_nodes -= 1;
        assert!(matches!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                limits
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Nodes,
                ..
            })
        ));
        for (work, expansion_work) in [(0, usize::MAX), (u64::MAX, 0)] {
            let limits = FormulaLimits {
                max_work: work,
                ..FormulaLimits::default()
            };
            let expansion = ExpansionLimits {
                max_term_work: expansion_work,
                ..ExpansionLimits::default()
            };
            assert!(matches!(
                limited(source, AdmissionOptions::default(), expansion, limits),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                } | FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::TermWork,
                    ..
                }))
            ));
        }
    }
    let limits = FormulaLimits {
        max_disjunction_elements: 2,
        ..FormulaLimits::default()
    };
    assert!(
        limited(
            "a|not a.",
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        )
        .is_ok()
    );
    assert!(matches!(
        limited(
            "a|not a|not not a.",
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::DisjunctionElements,
            observed: 3,
            ..
        })
    ));
}

#[test]
fn negative_heads_keep_original_duplicate_bundle_origins() {
    let directory = Directory::new();
    let rule = "not not a(1..2)|b.";
    fs::write(
        directory.0.join("entry.lp"),
        format!("#include \"other.lp\".\n{rule}"),
    )
    .unwrap();
    fs::write(directory.0.join("other.lp"), rule).unwrap();
    let bundle = SourceBundle::load(directory.0.join("entry.lp"), BundleLimits::default()).unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    for origins in admitted.formula_origins() {
        assert_eq!(
            origins
                .iter()
                .map(|origin| origin.source)
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        for origin in origins {
            assert_eq!(
                admitted
                    .bundle()
                    .get(origin.source)
                    .unwrap()
                    .source()
                    .slice(origin.span)
                    .unwrap(),
                rule
            );
        }
    }
}
