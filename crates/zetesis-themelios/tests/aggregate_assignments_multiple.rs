//! Independent aggregate proposals form a bounded product; each equality stays
//! in the original formula. The reference evaluator implements the finite
//! reduct definition without the production reduct masks or countermodel search.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_core::{Atom, Sign, Value};
use zetesis_cpu::Control;
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature,
    admit_formula,
};

type Models = BTreeSet<BTreeSet<String>>;
const SOURCE: SourceId = SourceId::new(83);

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
    include_str!("fixtures/aggregate-assignments-multiple.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn expected(row: &Json) -> Models {
    row[2].as_array().unwrap().iter().map(json_model).collect()
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
        Control::default(),
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

#[test]
fn all_function_pairs_correlations_and_recursive_equalities_match_the_finite_reduct() {
    let cases = cases();
    assert_eq!(cases.len(), 54);
    for row in cases {
        let source = row[1].as_str().unwrap();
        let admitted = input(source).unwrap_or_else(|error| panic!("{}: {error}", row[0]));
        assert_eq!(admitted.source().text(), source);
        assert_eq!(exhaustive(&admitted), expected(&row), "{}", row[0]);
        assert_eq!(native(&admitted), expected(&row), "{}", row[0]);
    }
}

#[test]
fn independent_generator_order_preserves_scopes_and_full_models() {
    let aggregates = ["N=#count{X:p(X)}", "M=#sum{X:q(X)}", "K=#sum+{-2:p(1)}"];
    let mut reference = None;
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let body = order.map(|index| aggregates[index]).join(",");
        let source = format!("p(1).q(2).r(N,M,K):-{body}.");
        let admitted = input(&source).unwrap();
        let actual = native(&admitted);
        assert_eq!(actual, exhaustive(&admitted));
        assert_eq!(
            actual,
            BTreeSet::from([BTreeSet::from([
                "p(1)".into(),
                "q(2)".into(),
                "r(1,2,0)".into()
            ])])
        );
        if let Some(reference) = &reference {
            assert_eq!(&actual, reference);
        } else {
            reference = Some(actual);
        }
    }
}

fn profile(error: &FormulaFailure, expected: ProfileFeature) {
    assert!(
        matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature,
                ..
            })) if *feature == expected
        ),
        "{error}"
    );
    assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
}

#[test]
fn unsupported_aggregate_dependencies_remain_refused() {
    for source in [
        "r(N,M):-N=#count{M:p},M=#count{}.",
        "r(N,M):-N=#count{},M=#count{N:p}.",
        "r(N,M):-N=#count{X:p(X),X=M},M=#count{}.",
        "r(N,M):-N=#count{X:p(X),not q(M,_)},M=#count{}.",
        "r(N,M):-N=#count{X:p(X),Y=M+1},M=#count{}.",
        "r(N,M):-N=#count{X:p(X),X=1..M},M=#count{}.",
        "r(N,M):-N=#count{},M=#count{},M<=#count{}.",
        "r(N,M):-N=#count{},M=#count{},M=#sum{}.",
    ] {
        profile(
            &input(source).unwrap_err(),
            ProfileFeature::AggregateAssignment,
        );
    }
}

#[test]
fn own_target_and_unbound_variables_keep_their_safety_refusal() {
    for source in [
        "r(N,M):-N=#count{N:p},M=#sum{}.",
        "r(N,M):-N=#count{},M=#count{X:p(M,X)}.",
        "r(X,N,M):-N=#count{X:p(X)},M=#sum{}.",
        "r(N,M):-N=#count{},M=#count{X:not p(X)}.",
        "r(N,M):-not N=#count{},M=#count{}.",
    ] {
        let error = input(source).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert_eq!(error.diagnostics()[0].primary().location.source, SOURCE);
    }
}

#[test]
fn objective_dependency_contract_is_not_silently_broadened() {
    let source = "{p}.r(N,M):-N=#count{1:p},M=#sum{2:p}.#minimize{N@3,M:r(N,M)}.";
    profile(
        &input(source).unwrap_err(),
        ProfileFeature::ObjectiveAggregateDependency,
    );
    let unrelated = "{p}.r(N,M):-N=#count{1:p},M=#sum{2:p}.#minimize{1@3:p}.";
    assert_eq!(input(unrelated).unwrap().objectives().priorities(), &[3]);
}

#[test]
fn product_limits_refuse_the_whole_source_and_preserve_original_locations() {
    let rule = "r(N,M):-N=#count{1:p},M=#count{1:q}.";
    let source = format!("{{p;q}}.\n{rule}");
    let reference = input(&source).unwrap();
    assert_eq!(native(&reference).len(), 4);
    for (limits, expected) in [
        (
            FormulaLimits {
                max_substitutions: 8,
                ..Default::default()
            },
            FormulaResource::Substitutions,
        ),
        (
            FormulaLimits {
                max_assignment_values: 1,
                ..Default::default()
            },
            FormulaResource::AssignmentValues,
        ),
        (
            FormulaLimits {
                max_aggregate_cache_rows: 1,
                ..Default::default()
            },
            FormulaResource::AggregateCacheRows,
        ),
        (
            FormulaLimits {
                max_aggregate_cache_roots: 1,
                ..Default::default()
            },
            FormulaResource::AggregateCacheRoots,
        ),
    ] {
        let error = admit_formula(
            source.clone(),
            options(),
            ExpansionLimits::default(),
            limits,
        )
        .unwrap_err();
        let FormulaFailure::Limit {
            resource,
            limit,
            observed,
            location,
        } = error
        else {
            panic!("expected formula product refusal: {error}");
        };
        assert_eq!(resource, expected);
        assert!(observed > limit);
        assert_eq!(location.source, SOURCE);
        assert_eq!(reference.source().slice(location.span).unwrap(), rule);
        assert_eq!(native(&input(&source).unwrap()), native(&reference));
    }
    let origins: BTreeSet<_> = reference
        .formula_origins()
        .iter()
        .flatten()
        .map(|location| reference.source().slice(location.span).unwrap())
        .collect();
    assert!(origins.contains(rule));
    // Keep upstream ASP-Core-2 safety facts intact; the frontend independently
    // establishes the clingo equality-binder extension.
    assert!(!reference.source_analysis().safety().is_safe());
}

#[test]
fn scope_work_is_bounded_and_can_be_retried_without_partial_admission() {
    let source = "r(N,M):-N=#count{},M=#sum{}.";
    let error = admit_formula(
        source.into(),
        options(),
        ExpansionLimits {
            max_term_work: 1,
            ..Default::default()
        },
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(error,
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::TermWork, observed, limit, location
        }) if observed > limit && location.source == SOURCE
    ));
    assert_eq!(
        native(&input(source).unwrap()),
        BTreeSet::from([BTreeSet::from(["r(0,0)".into()])])
    );
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-multiple-aggregates-{}-{}",
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

fn external(source: &str) -> Json {
    let directory = Directory::new();
    let source_path = directory.0.join("source.lp");
    let out = directory.0.join("stdout");
    let err = directory.0.join("stderr");
    fs::write(&source_path, source).unwrap();
    let mut child = Command::new(std::env::var_os("CLINGO").unwrap_or_else(|| "clingo".into()))
        .args(["--models=0", "--outf=2", "-"])
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
            assert!(
                matches!(status.code(), Some(10 | 20 | 30)),
                "{}",
                fs::read_to_string(&err).unwrap()
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

#[test]
#[ignore = "requires external clingo; exact original sources and complete full-model replay"]
fn multiple_aggregate_assignments_match_clingo() {
    for row in cases() {
        let source = row[1].as_str().unwrap();
        let result = external(source);
        assert!(
            result["Solver"]
                .as_str()
                .unwrap()
                .starts_with("clingo version ")
        );
        assert_eq!(result["Models"]["More"], "no", "{}", row[0]);
        let witnesses: Vec<_> = result["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .collect();
        let actual: Models = witnesses
            .iter()
            .map(|witness| json_model(&witness["Value"]))
            .collect();
        assert_eq!(
            actual.len(),
            witnesses.len(),
            "complete semantic multiplicity"
        );
        assert_eq!(
            result["Models"]["Number"].as_u64().unwrap(),
            u64::try_from(actual.len()).unwrap()
        );
        assert_eq!(actual, expected(&row), "{}: {source}", row[0]);
        assert_eq!(actual, native(&input(source).unwrap()), "{}", row[0]);
        assert_eq!(
            result["Result"],
            if actual.is_empty() {
                "UNSATISFIABLE"
            } else {
                "SATISFIABLE"
            }
        );
    }
}
