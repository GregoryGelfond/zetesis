//! Display/declaration metadata preserves full stable models and source evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_clingo_support as oracle;
use zetesis_core::{Atom, Program};
use zetesis_cpu::{Cancellation, CandidateLimits, Candidates, Limits, check};
use zetesis_themelios::{
    AdmissionOptions, Admitted, BundleAdmissionError, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, ExpansionResource, OutputSelection, SourceBundle,
    SourceDirective, admit, admit_bundle_extended, admit_extended,
};

type Models = BTreeSet<BTreeSet<Atom>>;
type Displays = BTreeMap<BTreeSet<Atom>, usize>;
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn input(source: &str) -> Admitted {
    admit_extended(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_or_else(|error| panic!("metadata source {source}: {error}"))
}

fn models(program: &Program) -> Models {
    let cancellation = Cancellation::default();
    let mut models = BTreeSet::new();
    for seed in Candidates::new(program, CandidateLimits::default(), cancellation.clone()) {
        let result = check(
            program,
            &seed.expect("full candidate enumeration"),
            Limits::default(),
            &cancellation,
        )
        .expect("complete reduct");
        if result.accepted() {
            assert!(
                models.insert(
                    result
                        .closure()
                        .atoms()
                        .iter()
                        .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
                        .collect::<BTreeSet<_>>()
                )
            );
        }
    }
    models
}

fn displays(models: &Models, selection: &OutputSelection) -> Displays {
    let mut displays = BTreeMap::new();
    for model in models {
        let shown = model
            .iter()
            .filter(|atom| selection.includes(*atom))
            .cloned()
            .collect();
        *displays.entry(shown).or_default() += 1;
    }
    displays
}

#[test]
fn defined_signatures_are_located_declarations_without_logical_effects() {
    let source = "#defined missing/3. p(1). {q}. #defined p/1.";
    let accepted = input(source);
    let base = input("p(1). {q}.");
    assert_eq!(
        accepted.program().templates().iter().collect::<Vec<_>>(),
        base.program().templates().iter().collect::<Vec<_>>()
    );
    assert_eq!(models(accepted.program()), models(base.program()));
    assert!(!accepted.metadata().output().is_explicit());
    assert!(
        accepted
            .metadata()
            .directives()
            .iter()
            .all(|entry| matches!(entry.directive(), SourceDirective::Defined(_)))
    );
    let first = accepted.metadata().directives().at(0).unwrap();
    assert_eq!(
        accepted
            .source()
            .slice(first.location().span)
            .expect("original declaration"),
        "#defined missing/3."
    );
}

#[test]
fn signatures_select_exact_arities_and_never_rename_through_constants() {
    let accepted = input("#const p=7. p. p(1). q. #show p/0.");
    let full = models(accepted.program());
    assert_eq!(full.first().expect("full model").len(), 3);
    let shown = displays(&full, accepted.metadata().output());
    let atom = shown
        .first_key_value()
        .expect("one displayed model")
        .0
        .first()
        .expect("p/0");
    assert_eq!(atom.predicate().name(), "p");
    assert_eq!(atom.predicate().arity(), 0);
    assert_eq!(shown.first_key_value().expect("one display").0.len(), 1);
}

#[test]
fn empty_show_activates_explicit_selection_without_erasing_other_signatures() {
    for suffix in ["#show p/0. #show.", "#show. #show p/0."] {
        let accepted = input(&format!("p. q. {suffix}"));
        assert!(accepted.metadata().output().is_explicit());
        assert_eq!(accepted.metadata().output().signatures().len(), 1);
        assert_eq!(
            displays(&models(accepted.program()), accepted.metadata().output())
                .first_key_value()
                .expect("display")
                .0
                .len(),
            1
        );
    }
    let accepted = input("p. q. #show.");
    assert_eq!(
        displays(&models(accepted.program()), accepted.metadata().output()),
        BTreeMap::from([(BTreeSet::new(), 1)])
    );
    let default = input("p. q.");
    assert_eq!(
        displays(&models(default.program()), default.metadata().output())
            .first_key_value()
            .expect("display")
            .0
            .len(),
        2
    );
}

#[test]
fn equal_displays_do_not_collapse_full_model_identity_or_counts() {
    let accepted = input("{p}. {q}. #show p/0.");
    let full = models(accepted.program());
    assert_eq!(full.len(), 4);
    let shown = displays(&full, accepted.metadata().output());
    assert_eq!(shown.len(), 2);
    assert!(shown.values().all(|count| *count == 2));
    let hidden = input("{p}. {q}. #show.");
    assert_eq!(
        displays(&models(hidden.program()), hidden.metadata().output()),
        BTreeMap::from([(BTreeSet::new(), 4)])
    );
}

#[test]
fn duplicate_metadata_occurrences_keep_their_original_spans() {
    let source = "#show p/1.\r\n#show p/1.\r\n#defined q/2. #defined q/2.";
    let options = AdmissionOptions {
        source_id: SourceId::new(91),
        ..AdmissionOptions::default()
    };
    let accepted = admit_extended(source.to_owned(), options, ExpansionLimits::default())
        .expect("located metadata");
    assert_eq!(accepted.metadata().directives().len(), 4);
    assert_eq!(accepted.metadata().output().signatures().len(), 1);
    for entry in accepted.metadata().directives().iter() {
        assert_eq!(entry.location().source, SourceId::new(91));
        let original = accepted
            .source()
            .slice(entry.location().span)
            .expect("original metadata span");
        assert!(matches!(original, "#show p/1." | "#defined q/2."));
    }
}

#[test]
fn raw_metadata_occurrences_have_their_own_budget_before_deduplication() {
    let limits = ExpansionLimits {
        max_metadata_statements: 1,
        ..ExpansionLimits::default()
    };
    assert!(matches!(
        admit_extended(
            "#show p/1. #show p/1.".to_owned(),
            AdmissionOptions::default(),
            limits
        ),
        Err(ExpansionFailure::Limit {
            resource: ExpansionResource::MetadataStatements,
            observed: 2,
            limit: 1,
            ..
        })
    ));
    let mut options = AdmissionOptions::default();
    options.core_limits.max_templates = 0;
    let accepted = admit_extended(
        "#defined p/2. #show.".to_owned(),
        options,
        ExpansionLimits::default(),
    )
    .expect("metadata consumes no templates");
    assert!(accepted.program().templates().is_empty());
    assert_eq!(models(accepted.program()).len(), 1);
    assert!(
        admit_extended(
            "p.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_metadata_statements: 0,
                ..ExpansionLimits::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn strict_s0_and_unsupported_display_forms_still_refuse() {
    for source in [
        "#defined p/1.",
        "#show p/1.",
        "#show.",
        "#defined -p/1.",
        "#show -p/1.",
    ] {
        assert!(admit(source.to_owned(), AdmissionOptions::default()).is_err());
        input(source);
    }
    for source in [
        "#show p.",
        "#show p(X) : q(X).",
        "#show 1+2.",
        "#project p/1.",
        "#defined p.",
    ] {
        assert!(
            admit_extended(
                source.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default()
            )
            .is_err(),
            "unsupported source {source}"
        );
    }
}

struct Fixture {
    directory: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        loop {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let directory =
                std::env::temp_dir().join(format!("zetesis-metadata-{}-{id}", std::process::id()));
            match fs::create_dir(&directory) {
                Ok(()) => return Self { directory },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("fixture directory: {error}"),
            }
        }
    }
    fn write(&self, name: &str, source: &str) {
        fs::write(self.directory.join(name), source).expect("fixture source");
    }
    fn bundle(&self) -> SourceBundle {
        SourceBundle::load(self.directory.join("entry.lp"), BundleLimits::default())
            .expect("original source graph")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn bundle_metadata_unions_global_selection_and_keeps_file_identities() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#include \"child.lp\". #show p/0. #defined missing/1.",
    );
    fixture.write("child.lp", "p. q. r. #show q/0. #show. #show p/0.");
    let accepted = admit_bundle_extended(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .expect("bundle metadata");
    assert_eq!(accepted.metadata().directives().len(), 5);
    assert_eq!(accepted.metadata().output().signatures().len(), 2);
    assert_eq!(
        models(accepted.program())
            .first()
            .expect("full model")
            .len(),
        3
    );
    assert_eq!(
        displays(&models(accepted.program()), accepted.metadata().output())
            .first_key_value()
            .expect("display")
            .0
            .len(),
        2
    );
    let identities: BTreeSet<_> = accepted
        .metadata()
        .directives()
        .iter()
        .map(|entry| entry.location().source)
        .collect();
    assert_eq!(
        identities,
        BTreeSet::from([SourceId::new(0), SourceId::new(1)])
    );
    for entry in accepted.metadata().directives().iter() {
        assert!(
            accepted
                .bundle()
                .get(entry.location().source)
                .expect("metadata source")
                .source()
                .slice(entry.location().span)
                .is_ok()
        );
    }
    let error = admit_bundle_extended(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits {
            max_metadata_statements: 4,
            ..ExpansionLimits::default()
        },
    )
    .expect_err("global metadata ceiling");
    assert!(matches!(
        error.error(),
        BundleAdmissionError::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::MetadataStatements,
            observed: 5,
            limit: 4,
            ..
        })
    ));
}

fn oracle_atom(source: &str) -> Atom {
    let accepted =
        admit(format!("{source}."), AdmissionOptions::default()).expect("whole scalar oracle atom");
    let head = accepted
        .program()
        .templates()
        .at(0)
        .unwrap()
        .head()
        .expect("oracle fact");
    let values = head
        .terms()
        .iter()
        .map(|term| match term {
            zetesis_core::TemplateTerm::Constant(value) => value
                .to_value(zetesis_core::ValueLimits::default())
                .unwrap(),
            zetesis_core::TemplateTerm::Variable(_) => panic!("ground oracle atom"),
        })
        .collect();
    Atom::new(
        zetesis_core::Predicate::with_sign(
            head.predicate().name(),
            head.predicate().arity(),
            head.predicate().sign(),
        )
        .unwrap(),
        values,
    )
    .expect("oracle atom arity")
}

fn oracle_json(output: &[u8]) -> Displays {
    let json: Json = serde_json::from_slice(output).expect("clingo JSON atom arrays");
    assert_eq!(json["Models"]["More"], "no");
    assert!(matches!(
        json["Result"].as_str(),
        Some("SATISFIABLE" | "UNSATISFIABLE")
    ));
    let mut displays = BTreeMap::new();
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                let shown = witness["Value"]
                    .as_array()
                    .expect("whole atom values")
                    .iter()
                    .map(|atom| oracle_atom(atom.as_str().expect("atom string")))
                    .collect();
                *displays.entry(shown).or_default() += 1;
            }
        }
    }
    assert_eq!(
        displays.values().sum::<usize>() as u64,
        json["Models"]["Number"].as_u64().expect("full model count")
    );
    displays
}

fn external(source: &str) -> Displays {
    let run = oracle::run(
        source,
        &["0", "--outf=2", "--warn=none"],
        oracle::Limits::default(),
    );
    oracle_json(run.stdout())
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn declarations_and_signature_display_match_clingo_without_model_projection() {
    for base in ["p. p(1). {q}.", "{p}. {q}.", "p. :- p.", ""] {
        for metadata in [
            "",
            "#defined p/0. #defined missing/3.",
            "#show.",
            "#show p/0.",
            "#show p/1.",
            "#show p/0. #show q/0.",
            "#show p/0. #show. #show p/0.",
            "#show unknown/7.",
        ] {
            let accepted = input(&format!("{base} {metadata}"));
            let full = models(accepted.program());
            assert_eq!(
                displays(&full, &OutputSelection::default()),
                external(base),
                "full models unaffected by {metadata}"
            );
            assert_eq!(
                displays(&full, accepted.metadata().output()),
                external(&format!("{base} {metadata}")),
                "display policy {metadata}"
            );
        }
    }
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn included_signature_metadata_matches_clingo_complete_display_multiplicities() {
    let fixture = Fixture::new();
    for directives in ["#show.", "#show p/0.", "#show p/0. #show q/0."] {
        fixture.write(
            "entry.lp",
            &format!("#include \"child.lp\". #defined p/0. {directives}"),
        );
        fixture.write("child.lp", "{p}. {q}. #show.");
        let accepted = admit_bundle_extended(
            fixture.bundle(),
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .expect("metadata bundle");
        let run = oracle::run_in(
            &fixture.directory,
            ["entry.lp", "0", "--outf=2", "--warn=none"],
            &oracle::DECIDED,
            oracle::Limits::default(),
        );
        assert_eq!(
            displays(&models(accepted.program()), accepted.metadata().output()),
            oracle_json(run.stdout())
        );
    }
}
