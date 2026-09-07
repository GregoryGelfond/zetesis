//! Explicitly true/empty conditions retain signed whole-rule expansion semantics.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use zetesis_core::{Atom, Sign, Value};
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits,
    FormulaResource, InputLimit, ProfileFeature, SourceBundle, admit_bundle_formula,
    admit_extended, admit_formula,
};

type Names = BTreeSet<String>;
type Models = BTreeSet<Names>;

fn cases() -> Vec<Json> {
    serde_json::from_str(include_str!("fixtures/true-heads.json")).unwrap()
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
    if atom.values().is_empty() {
        return format!("{sign}{}", atom.predicate().name());
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Infimum => "#inf".to_owned(),
            Value::Supremum => "#sup".to_owned(),
            Value::Structured(value) => value.to_string(),
            Value::Number(number) => number.to_string(),
            Value::String(string) => serde_json::to_string(string).unwrap(),
            Value::Symbol(symbol) => symbol.clone(),
        })
        .collect();
    format!("{sign}{}({})", atom.predicate().name(), values.join(","))
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

#[test]
fn complete_models_match_explicit_families_and_recorded_reference_expectations() {
    let cases = cases();
    assert_eq!(cases.len(), 25);
    for case in cases {
        let source = input(case["source"].as_str().unwrap()).unwrap();
        let expanded = input(case["expanded"].as_str().unwrap()).unwrap();
        let predicted = expected(&case["models"]);
        assert_eq!(complete(&source), predicted, "{}: source", case["name"]);
        assert_eq!(complete(&expanded), predicted, "{}: expanded", case["name"]);
    }
}

fn remap(mask: usize, from: &[Atom], to: &[Atom]) -> usize {
    from.iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .fold(0, |bits, (_, atom)| {
            bits | (1 << to.iter().position(|other| atom == other).unwrap())
        })
}
#[test]
fn source_expansions_preserve_every_original_and_frozen_pair() {
    let mut pairs = 0;
    for case in cases() {
        let source = input(case["source"].as_str().unwrap()).unwrap();
        let expanded = input(case["expanded"].as_str().unwrap()).unwrap();
        assert_eq!(
            source.atoms().iter().collect::<BTreeSet<_>>(),
            expanded.atoms().iter().collect::<BTreeSet<_>>(),
            "{}: exact carrier",
            case["name"]
        );
        assert!(source.atoms().len() <= 6);
        for mask in 0..1_usize << source.atoms().len() {
            let outer = values(source.theory(), mask, None);
            let other = values(
                expanded.theory(),
                remap(mask, source.atoms(), expanded.atoms()),
                None,
            );
            assert_eq!(
                holds(source.theory(), &outer),
                holds(expanded.theory(), &other)
            );
            for inner in 0..1_usize << source.atoms().len() {
                assert_eq!(
                    holds(
                        source.theory(),
                        &values(source.theory(), inner, Some(&outer))
                    ),
                    holds(
                        expanded.theory(),
                        &values(
                            expanded.theory(),
                            remap(inner, source.atoms(), expanded.atoms()),
                            Some(&other)
                        )
                    ),
                    "{}: M={mask}, J={inner}",
                    case["name"]
                );
                pairs += 1;
            }
        }
    }
    assert!(pairs > 1_000);
}
// Every M-false subtree becomes falsum, including non-atomic implications.
// This evaluates JSON trees directly, without a production DAG or compiler.
fn truth(formula: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    if frozen.is_some_and(|outer| !truth(formula, outer, None)) {
        return false;
    }
    if let Some(atom) = formula.as_str() {
        return tested.contains(atom);
    }
    if formula == &Json::Bool(false) {
        return false;
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
#[test]
fn true_head_range_products_match_handwritten_frozen_formulas() {
    for (source, manual) in [
        (
            "p(1..2):#true;q:#true.",
            json!({"roots": [["or", ["and", "p(1)", "p(2)"], "q"]]}),
        ),
        (
            "p(1..2):#true;q(1..2):#true.",
            json!({"roots": [["or", ["and", "p(1)", "p(2)"], ["and", "q(1)", "q(2)"]]]}),
        ),
        ("p(2..1):#true;q:#true.", json!({"roots": []})),
        ("a|b:.", json!({"roots": [["or", "a", "b"]]})),
    ] {
        let admitted = input(source).unwrap();
        for outer in 0..1_usize << admitted.atoms().len() {
            let candidate = selected(&admitted, outer);
            let frozen = values(admitted.theory(), outer, None);
            assert_eq!(
                holds(admitted.theory(), &frozen),
                manual_holds(&manual, &candidate, None)
            );
            for inner in 0..1_usize << admitted.atoms().len() {
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&frozen))
                    ),
                    manual_holds(&manual, &selected(&admitted, inner), Some(&candidate)),
                    "{source}: M={outer}, J={inner}",
                );
            }
        }
    }
}

fn profile(error: &FormulaFailure, expected: ProfileFeature) -> bool {
    matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(
        AdmissionFailure::Profile { feature, .. }
    )) if *feature == expected)
}

#[test]
fn dynamic_false_comparison_and_function_head_conditions_remain_located_refusals() {
    for source in [
        "p:#false;q:#true.",
        "p:not #true;q:#true.",
        "p:not not #false;q:#true.",
        "p:a;q:#true.",
        "p:not a;q:#true.",
        "p:not not a;q:#true.",
        "p:1=1;q:#true.",
        "p:#true,a;q:#true.",
        "p(X):X=1..2;q:#true.",
    ] {
        let error = input(source).expect_err(source);
        assert!(
            profile(&error, ProfileFeature::ConditionalDisjunction),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
    for (source, expected) in [
        ("1#count{X:p(X):X=1..4}2.", ProfileFeature::Head),
        ("not a.", ProfileFeature::NegatedHead),
        ("{not a}.", ProfileFeature::NegatedHead),
        ("p(1;2):#true;q:#true.", ProfileFeature::PooledArguments),
        (
            "p:#true;q:#true.#minimize{1:q}.",
            ProfileFeature::ObjectiveDisjunctionDependency,
        ),
    ] {
        let error = input(source).expect_err(source);
        assert!(profile(&error, expected), "{source}: {error}");
        assert!(!error.diagnostics().is_empty());
    }
    assert!(
        admit_extended(
            "p:#true;q:#true.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
}

#[test]
fn erased_conditions_do_not_bind_or_hide_unsafe_head_arguments() {
    for source in [
        "p(X):#true;q:#true.",
        "p(_):#true;q:#true.",
        "p(2..1,X):#true;q:#true.",
    ] {
        assert!(
            matches!(input(source), Err(FormulaFailure::UnsafeVariable { .. })),
            "{source}"
        );
    }
}

fn first_success(mut attempt: impl FnMut(u64) -> bool) -> u64 {
    let mut high = 1;
    while !attempt(high) {
        high *= 2;
        assert!(high <= 1_048_576);
    }
    let mut low = 0;
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if attempt(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

#[test]
fn condition_and_head_counts_have_inclusive_limits() {
    let source = "p:#true,#true;q:#true.";
    let mut options = AdmissionOptions {
        max_body_elements: 2,
        ..AdmissionOptions::default()
    };
    assert!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_ok()
    );
    options.max_body_elements = 1;
    assert!(matches!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Limit {
                resource: InputLimit::BodyElements,
                limit: 1,
                observed: 2,
                ..
            }
        )))
    ));
    let limits = FormulaLimits {
        max_disjunction_elements: 2,
        ..FormulaLimits::default()
    };
    assert!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        )
        .is_ok()
    );
    assert!(matches!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_disjunction_elements: 1,
                ..limits
            }
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::DisjunctionElements,
            limit: 1,
            observed: 2,
            ..
        })
    ));
}

#[test]
fn erased_conditions_do_not_consume_synthetic_variable_slots() {
    let source = "d(1).p(X+1):#true;q:#true:-d(X).";
    let mut options = AdmissionOptions::default();
    options.core_limits.max_variables_per_template = 2;
    assert!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        )
        .is_ok()
    );
    options.core_limits.max_variables_per_template = 1;
    assert!(matches!(
        limited(
            source,
            options,
            ExpansionLimits::default(),
            FormulaLimits::default()
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Variables,
            limit: 1,
            observed: 2,
            ..
        })
    ));
}

#[test]
fn construction_work_substitutions_and_nodes_fail_at_exact_boundaries() {
    let source = "d(1).p(X+1):#true;q:#true:-d(X).";
    for resource in [FormulaResource::Work, FormulaResource::Substitutions] {
        let attempt = |limit| {
            let mut limits = FormulaLimits::default();
            if resource == FormulaResource::Work {
                limits.max_work = limit;
            } else {
                limits.max_substitutions = limit;
            }
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                limits,
            )
        };
        let threshold = first_success(|limit| attempt(limit).is_ok());
        assert!(
            matches!(attempt(threshold-1), Err(FormulaFailure::Limit { resource: actual, observed, .. })
            if actual == resource && observed == u128::from(threshold))
        );
        assert_eq!(
            complete(&attempt(threshold).unwrap()),
            complete(&input(source).unwrap())
        );
    }
    let attempt = |limit| {
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits {
                max_term_work: usize::try_from(limit).unwrap(),
                ..ExpansionLimits::default()
            },
            FormulaLimits::default(),
        )
    };
    let threshold = first_success(|limit| attempt(limit).is_ok());
    assert!(matches!(
        attempt(threshold - 1),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::TermWork,
            ..
        }))
    ));
    let admitted = input(source).unwrap();
    let mut limits = FormulaLimits::default();
    limits.theory.max_nodes = admitted.theory().nodes().len();
    assert!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        )
        .is_ok()
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
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-true-heads-{}-{}",
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
fn duplicate_included_true_heads_retain_each_source_origin() {
    let directory = Directory::new();
    let rule = "p(1..2):#true;q:#true.";
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
    assert!(!admitted.formula_origins().is_empty());
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
#[test]
#[ignore = "requires external clingo 5.8; each original and expansion has a bounded complete capture"]
fn fresh_clingo_original_and_expanded_sources_match_complete_models() {
    for case in cases() {
        let predicted = expected(&case["models"]);
        for field in ["source", "expanded"] {
            assert_eq!(
                clingo(case[field].as_str().unwrap()),
                predicted,
                "{}: {field}",
                case["name"]
            );
        }
    }
}
