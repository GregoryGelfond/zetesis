//! Ground Boolean/comparison guards preserve the original and every reduct.
//! Exact external records complement a separate finite definition evaluator.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_core::Sign;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource,
    SourceBundle, admit_bundle_formula, admit_formula,
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
    include_str!("../fixtures/ground-guards.jsonl")
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

#[test]
fn exact_sources_match_external_records_and_an_independent_reduct_evaluator() {
    let cases = cases();
    assert_eq!(cases.len(), 124);
    let mut models = 0;
    for row in cases {
        let source = row["source"].as_str().unwrap();
        let admitted = input(source).unwrap_or_else(|error| panic!("{}: {error}", row["name"]));
        assert_eq!(admitted.source().text(), source);
        let expected = expected(&row);
        models += expected.len();
        assert_eq!(exhaustive(&admitted), expected, "{}", row["name"]);
        assert_eq!(native(&admitted), expected, "{}", row["name"]);
    }
    assert_eq!(models, 120);
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
fn ground_guard_replacement_preserves_every_original_and_frozen_pair() {
    for (guard, truth) in [
        ("#true", true),
        ("#false", false),
        ("not #true", false),
        ("not #false", true),
        ("not not #true", true),
        ("not not #false", false),
        ("1<2<3", true),
        ("1<3<2", false),
        ("not 1<2<3", false),
        ("not 1<3<2", true),
        ("not 3<1<2", true),
        ("not 3<2<1", true),
        ("not not 1<2<3", true),
        ("not not 1<3<2", false),
    ] {
        let source = input(&format!("{{p;q}}.p:-q,{guard}.q:-p.")).unwrap();
        let replacement = if truth { "1=1" } else { "1=2" };
        let expanded = input(&format!("{{p;q}}.p:-q,{replacement}.q:-p.")).unwrap();
        assert_eq!(
            source.atoms().iter().collect::<BTreeSet<_>>(),
            expanded.atoms().iter().collect()
        );
        for outer in 0..1 << source.atoms().len() {
            let original = values(source.theory(), outer, None);
            let other = values(
                expanded.theory(),
                remap(outer, source.atoms(), expanded.atoms()),
                None,
            );
            assert_eq!(
                holds(source.theory(), &original),
                holds(expanded.theory(), &other),
                "{guard}"
            );
            for inner in 0..1 << source.atoms().len() {
                assert_eq!(
                    holds(
                        source.theory(),
                        &values(source.theory(), inner, Some(&original))
                    ),
                    holds(
                        expanded.theory(),
                        &values(
                            expanded.theory(),
                            remap(inner, source.atoms(), expanded.atoms()),
                            Some(&other)
                        )
                    ),
                    "{guard}, M={outer}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn guard_conditions_do_not_supply_missing_bindings_or_expand_other_source_profiles() {
    // These original profile refusals are now covered by the separate finite
    // generator plan. The guard itself remains in the compiled body.
    for (source, expected) in [
        ("p(X):-not not X=1.", "p(1)."),
        ("p(X):-not not X=1<2.", "p(1)."),
        ("p(X):-X=1<2.", "p(1)."),
        ("p(X):-0<X<3.", "p(1..2)."),
    ] {
        assert_eq!(
            native(&input(source).unwrap()),
            native(&input(expected).unwrap())
        );
    }
    for source in [
        "p(X):-not X=1.",
        "p(X):-#false.",
        "d(1).p(X):-#count{X:d(X),#true}>0.",
        "d(1).{p(X):#true}.",
    ] {
        let error = input(source).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert!(
            error
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
        );
    }
    // Boolean heads are qualified independently in boolean_heads.rs.
    assert_eq!(
        native(&input("#true.").unwrap()),
        BTreeSet::from([BTreeSet::new()])
    );
    assert_eq!(
        native(&input("q:-p(X):d(X).").unwrap()),
        BTreeSet::from([BTreeSet::from(["q".to_owned()])])
    );
}

#[test]
fn undefined_or_overflowing_guard_values_never_become_boolean_false() {
    for source in [
        "p:-not 1/0=0.",
        "p:-not not 1/0=0.",
        "p:-not 1/0=0=1.",
        "p:-not 0=1=1/0.",
        "p:-#false,not 1/0=0.",
        "p:-not 1/0=0,#false.",
        "{p:not 1/0=0}.",
        "p:-#count{1:not 1/0=0}=1.",
        "d(2147483647).p:-d(X),not X+1=0.",
        "d(0).p:-d(X),not (1,2)=(1,2,1/X).",
        "d(0).p:-d(X),not not (1,2,1/X)!=(1,2).",
    ] {
        let error = input(source).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn negated_guards_omit_undefined_substitutions() {
    let program = input("d(0..2).p(X):-d(X),not 1/X=1.").unwrap();
    let expected = native(&input("d(0..2).p(2).").unwrap());
    assert_eq!(program.warnings().len(), 1);
    assert_eq!(native(&program), expected);
    assert_eq!(exhaustive(&program), expected);
}

#[test]
fn incomplete_guard_construction_and_ground_evaluation_have_located_limits() {
    let source = "d(1..3).p(X):-d(X),not 0<X<3.";
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
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::TermWork, observed, limit, .. }) if observed > limit)
    );
    let error = admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_work: 0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, location } if observed > limit && location.source == SOURCE)
    );
    let admitted = input(source).unwrap();
    assert_eq!(native(&admitted), exhaustive(&admitted));
    assert!(
        admitted
            .formula_origins()
            .iter()
            .flatten()
            .all(|location| location.source == SOURCE)
    );
}

#[test]
fn bundle_guards_keep_original_sources_signatures_and_rule_origins() {
    let directory = Directory::new();
    let root = directory.0.join("root.lp");
    let rules = directory.0.join("rules.lp");
    let source = "#include \"rules.lp\". d(1..3). #show p/1. #defined d/1.";
    let rule = "p(X):-d(X),not 0<X<3.";
    fs::write(&root, source).unwrap();
    fs::write(&rules, rule).unwrap();
    let bundle = SourceBundle::load(&root, BundleLimits::default()).unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let retained: BTreeSet<_> = admitted
        .bundle()
        .sources()
        .iter()
        .map(|source| source.source().text())
        .collect();
    assert_eq!(retained, BTreeSet::from([source, rule]));
    let rule_id = admitted
        .bundle()
        .sources()
        .iter()
        .find(|source| source.source().text() == rule)
        .unwrap()
        .id();
    let origins: Vec<_> = admitted.formula_origins().iter().flatten().collect();
    assert!(origins.iter().any(|location| location.source == rule_id));
    assert!(origins.iter().all(|location| {
        admitted
            .bundle()
            .get(location.source)
            .is_some_and(|source| source.source().slice(location.span).is_ok())
    }));
    assert!(
        admitted
            .atoms()
            .iter()
            .any(|atom| admitted.metadata().output().includes(atom))
    );
    assert!(
        admitted
            .atoms()
            .iter()
            .filter(|atom| admitted.metadata().output().includes(*atom))
            .all(|atom| atom.predicate().name() == "p")
    );
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zetesis-ground-guards-{}-{}",
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
#[ignore = "requires external clingo; exact sources and complete full-model replay"]
fn ground_guards_match_clingo() {
    for row in cases() {
        let actual = external(row["source"].as_str().unwrap());
        assert_eq!(actual["Models"]["More"], "no");
        let witnesses: Vec<_> = actual["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .collect();
        let records: Models = witnesses
            .iter()
            .map(|witness| {
                assert!(witness["Costs"].is_null());
                json_model(&witness["Value"])
            })
            .collect();
        assert_eq!(records.len(), witnesses.len());
        assert_eq!(
            actual["Models"]["Number"].as_u64(),
            Some(witnesses.len() as u64)
        );
        assert_eq!(records, expected(&row), "{}", row["name"]);
        assert_eq!(
            native(&input(row["source"].as_str().unwrap()).unwrap()),
            records,
            "{}",
            row["name"]
        );
    }
}
