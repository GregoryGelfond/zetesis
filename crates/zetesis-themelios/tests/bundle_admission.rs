//! Original multi-file extended programs: scopes, provenance, budgets, and an
//! optional independent clingo oracle. The solver never invokes that oracle.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_core::{Atom, Program, Term};
use zetesis_cpu::{CandidateLimits, Candidates, Control, Limits, check};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedBundle, BundleAdmissionError,
    BundleAdmissionFailure, BundleAdmissionOptions, BundleLimits, ExpansionFailure,
    ExpansionLimits, ExpansionResource, InputLimit, SourceBundle, admit, admit_bundle_extended,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
type Models = BTreeSet<BTreeSet<Atom>>;

struct Fixture {
    directory: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        loop {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "zetesis-bundle-admission-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&directory) {
                Ok(()) => return Self { directory },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("fixture directory: {error}"),
            }
        }
    }
    fn write(&self, name: &str, source: &str) {
        let path = self.path(name);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture parents");
        fs::write(path, source).expect("original fixture source");
    }
    fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }
    fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.path("entry.lp"), BundleLimits::default())
            .expect("loaded source graph")
    }
    fn admit(&self) -> Result<AdmittedBundle, BundleAdmissionFailure> {
        admit_bundle_extended(
            self.bundle(),
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this uniquely created test directory is ever removed.
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn native(program: &Program) -> Models {
    let control = Control::default();
    let mut models = BTreeSet::new();
    let mut candidates = Candidates::new(program, CandidateLimits::default(), control.clone());
    for seed in candidates.by_ref() {
        let seed = seed.expect("candidate budget permits full exhaustion");
        let result =
            check(program, &seed, Limits::default(), &control).expect("complete reduct closure");
        if result.accepted() {
            assert!(
                models.insert(
                    result
                        .closure()
                        .atoms()
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>()
                ),
                "one seed per model"
            );
        }
    }
    assert!(candidates.next().is_none(), "exhaustion is fused");
    models
}

fn explicit(source: &str) -> Models {
    native(
        admit(source.to_owned(), AdmissionOptions::default())
            .expect("explicit S0 fixture")
            .program(),
    )
}

#[test]
fn global_forward_constants_and_nested_includes_produce_native_templates() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#const upper=lower+2. #include \"parts/data.lp\". {pick(X)} :- d(X). :- pick(1),pick(3).",
    );
    fixture.write(
        "parts/data.lp",
        "#include \"../constants.lp\". d(lower..upper).",
    );
    fixture.write("constants.lp", "#const lower=1.");
    let admitted = fixture.admit().expect("global scalar definitions");
    assert_eq!(admitted.bundle().sources().len(), 3);
    assert_eq!(
        native(admitted.program()),
        explicit("d(1). d(2). d(3). {pick(X)} :- d(X). :- pick(1),pick(3).")
    );
}

#[test]
fn base_sections_preserve_included_rules() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#program base. p. #include \"child.lp\". #program base. r :- q.",
    );
    fixture.write("child.lp", "#program base. q :- p. #program base().");
    let admitted = fixture.admit().unwrap();
    assert_eq!(native(admitted.program()), explicit("p. q. r."));
    for origins in admitted.template_origins() {
        assert!(!origins.is_empty());
        for location in origins {
            let text = admitted
                .bundle()
                .get(location.source)
                .unwrap()
                .source()
                .slice(location.span)
                .unwrap();
            assert!(matches!(text, "p." | "q :- p." | "r :- q."), "{text}");
        }
    }
}

#[test]
fn base_sections_preserve_global_metadata() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#program base. #include \"child.lp\". p(n). #show p/1.",
    );
    fixture.write("child.lp", "#program base. #const n=2. #defined d/1.");
    let admitted = fixture.admit().unwrap();
    assert_eq!(native(admitted.program()), explicit("p(2)."));
    assert_eq!(admitted.metadata().directives().len(), 2);
}

#[test]
fn rule_variables_remain_local_across_files_and_positive_cycles() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#include \"left.lp\". #include \"right.lp\". left(1). right(2).",
    );
    fixture.write("left.lp", "p(X) :- left(X). cycle(X) :- loop(X).");
    fixture.write("right.lp", "q(X) :- right(X). loop(X) :- cycle(X).");
    assert_eq!(
        native(fixture.admit().expect("local rule scopes").program()),
        explicit("left(1). right(2). p(1). q(2).")
    );
}

#[test]
fn merged_cross_file_rules_retain_both_original_source_locations() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "% unchanged\r\n#include \"child.lp\".\r\np(1..2).",
    );
    fixture.write("child.lp", "p(1..2).\n");
    let admitted = fixture.admit().expect("merged rule evidence");
    assert_eq!(admitted.program().templates().len(), 2);
    for locations in admitted.template_origins() {
        assert_eq!(locations.len(), 2);
        let identities: BTreeSet<_> = locations.iter().map(|location| location.source).collect();
        assert_eq!(
            identities,
            BTreeSet::from([SourceId::new(0), SourceId::new(1)])
        );
        for location in locations {
            let source = admitted
                .bundle()
                .get(location.source)
                .expect("original source identity");
            assert_eq!(
                source.source().slice(location.span).expect("rule span"),
                "p(1..2)."
            );
        }
    }
}

#[test]
fn repeated_identical_include_paths_are_not_duplicate_definitions() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#include \"shared.lp\". #include \"shared.lp\". q(n).",
    );
    fixture.write("shared.lp", "#const n=2. p(n).");
    let admitted = fixture.admit().expect("same lexical path included once");
    assert_eq!(admitted.bundle().sources()[0].includes().len(), 2);
    assert_eq!(native(admitted.program()), explicit("p(2). q(2)."));
}

#[test]
fn lexical_aliases_preserve_the_native_refusal_contract() {
    // External include-alias handling differs across clingo installations.
    // Assert the native contract directly, including its source provenance.
    let fixture = Fixture::new();
    fs::create_dir(fixture.path("sub")).expect("alias parent");
    fixture.write(
        "entry.lp",
        "#include \"shared.lp\". #include \"sub/../shared.lp\".",
    );
    fixture.write("shared.lp", "#const n=2. p(n).");
    let error = fixture
        .admit()
        .expect_err("distinct lexical paths must not be silently merged");
    assert_eq!(
        error.bundle().sources().len(),
        2,
        "loader still preserves the canonical graph"
    );
    match error.error() {
        BundleAdmissionError::IncludeAlias {
            first,
            repeated,
            location,
        } => {
            assert_ne!(first.as_os_str(), repeated.as_os_str());
            assert_eq!(location.source, SourceId::new(0));
        }
        other => panic!("expected alias refusal: {other}"),
    }
    assert!(!error.diagnostics().is_empty());
}

#[cfg(unix)]
#[test]
fn redirected_include_paths_refuse_instead_of_guessing_relative_scopes() {
    let fixture = Fixture::new();
    fixture.write("actual.lp", "p.");
    std::os::unix::fs::symlink(fixture.path("actual.lp"), fixture.path("linked.lp"))
        .expect("fixture symlink");
    fixture.write("entry.lp", "#include \"linked.lp\".");
    assert!(matches!(
        fixture.admit().expect_err("redirected include").error(),
        BundleAdmissionError::IncludeRedirection { .. }
    ));
}

#[test]
fn cross_file_constant_duplicates_and_cycles_keep_their_source_catalog() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#const n=2. #include \"child.lp\".");
    fixture.write("child.lp", "#const n=2.");
    let error = fixture.admit().expect_err("global duplicate");
    match error.error() {
        BundleAdmissionError::Expansion(ExpansionFailure::DuplicateConstant {
            first,
            duplicate,
            ..
        }) => assert_ne!(first.source, duplicate.source),
        other => panic!("duplicate constant: {other}"),
    }
    fixture.write("entry.lp", "#const a=b. #include \"child.lp\".");
    fixture.write("child.lp", "#const b=a.");
    let error = fixture.admit().expect_err("global cycle");
    assert!(matches!(
        error.error(),
        BundleAdmissionError::Expansion(ExpansionFailure::ConstantCycle { .. })
    ));
    assert_eq!(error.into_bundle().sources().len(), 2);
}

#[test]
fn unsupported_directives_and_child_raiser_failures_do_not_return_partial_programs() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "p. #include \"child.lp\".");
    for child in [
        "#program base(x). q.",
        "#program step(t). q(t).",
        "#show p.",
        "{p;q}.",
    ] {
        fixture.write("child.lp", child);
        let error = fixture.admit().expect_err("unsupported original child");
        assert!(
            matches!(error.error(), BundleAdmissionError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile { location, .. })) if location.source == SourceId::new(1))
        );
        assert_eq!(error.bundle().sources()[1].source().text(), child);
    }
    fixture.write("child.lp", "q(-2147483648).");
    assert!(matches!(
        fixture.admit().expect_err("child numeric raising").error(),
        BundleAdmissionError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Raise(_)))
    ));
}

#[test]
fn syntax_and_constant_budgets_are_cumulative_across_original_sources() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#const a=1. #include \"child.lp\".");
    fixture.write("child.lp", "#const b=2. p(a,b).");
    let bundle = fixture.bundle();
    let total_nodes: usize = bundle
        .sources()
        .iter()
        .map(|source| source.parsed().syntax().descendants().count())
        .sum();
    let options = BundleAdmissionOptions {
        max_syntax_nodes: total_nodes - 1,
        ..BundleAdmissionOptions::default()
    };
    let error = admit_bundle_extended(bundle, options, ExpansionLimits::default())
        .expect_err("cumulative node budget");
    assert!(
        matches!(error.error(), BundleAdmissionError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit { resource: InputLimit::SyntaxNodes, limit, observed, location })) if *limit == total_nodes - 1 && *observed == total_nodes && location.source == SourceId::new(1))
    );
    let limits = ExpansionLimits {
        max_constants: 1,
        ..ExpansionLimits::default()
    };
    assert!(matches!(
        admit_bundle_extended(fixture.bundle(), BundleAdmissionOptions::default(), limits)
            .expect_err("global definitions budget")
            .error(),
        BundleAdmissionError::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Constants,
            observed: 2,
            ..
        })
    ));
}

#[test]
fn expansion_and_provenance_budgets_remain_global_across_files() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"child.lp\". p(1..2).");
    fixture.write("child.lp", "q(1..2).");
    let limits = ExpansionLimits {
        max_templates: 3,
        ..ExpansionLimits::default()
    };
    assert!(matches!(
        admit_bundle_extended(fixture.bundle(), BundleAdmissionOptions::default(), limits)
            .expect_err("global emitted templates")
            .error(),
        BundleAdmissionError::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            observed: 4,
            ..
        })
    ));
    fixture.write("child.lp", "p(1..2).");
    let limits = ExpansionLimits {
        max_origin_locations: 3,
        ..ExpansionLimits::default()
    };
    assert!(matches!(
        admit_bundle_extended(fixture.bundle(), BundleAdmissionOptions::default(), limits)
            .expect_err("global provenance copies")
            .error(),
        BundleAdmissionError::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Origins,
            observed: 4,
            ..
        })
    ));
}

#[test]
fn original_required_bundles_report_semantic_refusal_after_include_admission() {
    let manifest: Json =
        serde_json::from_str(include_str!("../../../validation/corpus/manifest.json"))
            .expect("vendored target manifest");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus/kr-domains");
    let cases = manifest["cases"].as_array().expect("required case entries");
    assert_eq!(cases.len(), 94);
    let mut features = std::collections::BTreeMap::<String, usize>::new();
    for case in cases {
        let path = case["path"].as_str().expect("original entry path");
        let bundle = SourceBundle::load(root.join(path), BundleLimits::default())
            .expect("unchanged original include graph");
        let error = admit_bundle_extended(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .expect_err("broader original programs remain outside this semantic slice");
        match error.error() {
            BundleAdmissionError::Expansion(ExpansionFailure::Admission(
                AdmissionFailure::Profile { feature, location },
            )) => {
                assert!(
                    error.bundle().get(location.source).is_some(),
                    "original refusal source: {path}"
                );
                *features.entry(format!("{feature:?}")).or_default() += 1;
            }
            other => panic!("unexpected semantic boundary for {path}: {other}"),
        }
    }
    eprintln!("Original required bundle semantic refusals: {features:?}");
}

fn external(path: &Path) -> Models {
    let output = Command::new("clingo")
        .arg(path.canonicalize().expect("canonical reference entry"))
        .args(["0", "--outf=2", "--warn=none"])
        .output()
        .expect("independently installed clingo");
    assert!(
        matches!(output.status.code(), Some(10 | 20 | 30)),
        "clingo failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Json = serde_json::from_slice(&output.stdout).expect("clingo JSON");
    assert_eq!(
        json["Models"]["More"], "no",
        "complete external enumeration"
    );
    assert!(matches!(
        json["Result"].as_str(),
        Some("SATISFIABLE" | "UNSATISFIABLE")
    ));
    let mut models = BTreeSet::new();
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                let atoms = witness["Value"]
                    .as_array()
                    .expect("oracle atom identities")
                    .iter()
                    .map(|value| oracle_atom(value.as_str().expect("whole atom string")))
                    .collect();
                assert!(models.insert(atoms), "distinct oracle models");
            }
        }
    }
    assert_eq!(
        u64::try_from(models.len()).expect("model count"),
        json["Models"]["Number"]
            .as_u64()
            .expect("oracle model count")
    );
    models
}

fn oracle_atom(source: &str) -> Atom {
    // Parse each complete canonical atom separately, never split strings on
    // whitespace. Only scalar atoms from this campaign are expected here.
    let input =
        admit(format!("{source}."), AdmissionOptions::default()).expect("oracle scalar atom");
    assert_eq!(input.program().templates().len(), 1);
    let head = input.program().templates()[0].head().expect("oracle fact");
    let values = head
        .terms()
        .iter()
        .map(|term| match term {
            Term::Constant(value) => value.clone(),
            Term::Variable(_) => panic!("oracle atom is ground"),
        })
        .collect();
    Atom::new(head.predicate().clone(), values).expect("oracle arity")
}

fn compare(fixture: &Fixture) {
    let input = fixture.admit().expect("supported original bundle");
    assert_eq!(native(input.program()), external(&fixture.path("entry.lp")));
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn accepted_original_include_graphs_match_complete_clingo_models() {
    let fixture = Fixture::new();
    for lower in -2..=2 {
        for upper in -2..=2 {
            fixture.write(
                "entry.lp",
                &format!("#const hi={upper}. #include \"parts/rules.lp\". #const lo={lower}."),
            );
            fixture.write(
                "parts/rules.lp",
                "#include \"../data.lp\". {pick(X)} :- d(X). :- pick(X),pick(Y),X != Y.",
            );
            fixture.write("data.lp", "d(lo..hi).");
            compare(&fixture);
        }
    }
    fixture.write(
        "entry.lp",
        "#include \"data.lp\". #include \"data.lp\". {pick(X)} :- p(X).",
    );
    fixture.write(
        "data.lp",
        "#const name=\"a b\". p(name;\"quote \\\"x\\\"\";\"a\tb\").",
    );
    compare(&fixture);
    fixture.write(
        "entry.lp",
        "#const high=low+2. #include \"data.lp\". q(high).",
    );
    fixture.write("data.lp", "#const low=1. p(low..high).");
    compare(&fixture);
    fixture.write("entry.lp", "#include \"data.lp\". :- p(1).");
    fixture.write("data.lp", "p(1..2).");
    compare(&fixture);
}
