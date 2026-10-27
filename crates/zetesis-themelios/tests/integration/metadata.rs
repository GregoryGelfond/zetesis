//! Display/declaration metadata preserves full stable models and source evidence.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::admission;
use crate::support::atom_models::{self, Models, answer_sets, bundle_answer_sets};
use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_clingo_support as oracle;
use zetesis_core::Atom;
use zetesis_reference_support::{BundleFixture, resource_limits};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, ExpansionFailure, ExpansionLimits,
    ExpansionResource, FormulaFailure, FormulaLimits, OutputSelection, SourceDirective,
    admit_bundle_formula, admit_formula,
};

type Displays = BTreeMap<BTreeSet<Atom>, usize>;

fn input(source: &str) -> AdmittedFormula {
    zetesis_reference_support::admit(source, &FormulaLimits::default())
        .unwrap_or_else(|error| panic!("metadata source {source}: {error}"))
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
    assert_eq!(accepted.theory().roots(), base.theory().roots());
    assert_eq!(accepted.atoms(), base.atoms());
    assert_eq!(accepted.theory().nodes(), base.theory().nodes());
    assert_eq!(accepted.theory().operands(), base.theory().operands());
    assert_eq!(answer_sets(&accepted), answer_sets(&base));
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
            .expect("source input")
            .slice(first.location().span)
            .expect("original declaration"),
        "#defined missing/3."
    );
}

#[test]
fn signatures_select_exact_arities_and_never_rename_through_constants() {
    let accepted = input("#const p=7. p. p(1). q. #show p/0.");
    let full = answer_sets(&accepted);
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
            displays(&answer_sets(&accepted), accepted.metadata().output())
                .first_key_value()
                .expect("display")
                .0
                .len(),
            1
        );
    }
    let accepted = input("p. q. #show.");
    assert_eq!(
        displays(&answer_sets(&accepted), accepted.metadata().output()),
        BTreeMap::from([(BTreeSet::new(), 1)])
    );
    let default = input("p. q.");
    assert_eq!(
        displays(&answer_sets(&default), default.metadata().output())
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
    let full = answer_sets(&accepted);
    assert_eq!(full.len(), 4);
    let shown = displays(&full, accepted.metadata().output());
    assert_eq!(shown.len(), 2);
    assert!(shown.values().all(|count| *count == 2));
    let hidden = input("{p}. {q}. #show.");
    assert_eq!(
        displays(&answer_sets(&hidden), hidden.metadata().output()),
        BTreeMap::from([(BTreeSet::new(), 4)])
    );
}

#[test]
fn duplicate_metadata_occurrences_keep_their_original_spans() {
    let source = "#show p/1.\r\n#show p/1.\r\n#defined q/2. #defined q/2.";
    let options = admission::options(SourceId::new(91));
    let accepted = admit_formula(
        source.to_owned(),
        options,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("located metadata");
    assert_eq!(accepted.metadata().directives().len(), 4);
    assert_eq!(accepted.metadata().output().signatures().len(), 1);
    for entry in accepted.metadata().directives().iter() {
        assert_eq!(entry.location().source, SourceId::new(91));
        let original = accepted
            .source()
            .expect("source input")
            .slice(entry.location().span)
            .expect("original metadata span");
        assert!(matches!(original, "#show p/1." | "#defined q/2."));
    }
}

#[test]
fn raw_metadata_occurrences_have_their_own_budget_before_deduplication() {
    let limits = resource_limits::expansion(ExpansionResource::MetadataStatements, 1);
    assert!(matches!(
        admit_formula(
            "#show p/1. #show p/1.".to_owned(),
            AdmissionOptions::default(),
            limits,
            FormulaLimits::default()
        ),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::MetadataStatements,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
    let options = AdmissionOptions {
        core_limits: zetesis_core::AdmissionLimits {
            max_templates: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let accepted = admit_formula(
        "#defined p/2. #show.".to_owned(),
        options,
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("metadata consumes no templates");
    assert!(accepted.theory().roots().is_empty());
    assert_eq!(answer_sets(&accepted).len(), 1);
    assert!(
        admit_formula(
            "p.".to_owned(),
            AdmissionOptions::default(),
            resource_limits::expansion(ExpansionResource::MetadataStatements, 0),
            FormulaLimits::default()
        )
        .is_ok()
    );
}

#[test]
fn declaration_and_signature_forms_are_accepted() {
    for source in [
        "#defined p/1.",
        "#show p/1.",
        "#show.",
        "#defined -p/1.",
        "#show -p/1.",
    ] {
        input(source);
    }
}

#[test]
fn malformed_defined_declarations_are_refused() {
    assert!(zetesis_reference_support::admit("#defined p.", &FormulaLimits::default()).is_err());
}

#[test]
fn bundle_metadata_unions_global_selection_and_keeps_file_identities() {
    let fixture = BundleFixture::new();
    fixture.write(
        "entry.lp",
        "#include \"child.lp\". #show p/0. #defined missing/1.",
    );
    fixture.write("child.lp", "p. q. r. #show q/0. #show. #show p/0.");
    let accepted = admit_bundle_formula(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("bundle metadata");
    assert_eq!(accepted.metadata().directives().len(), 5);
    assert_eq!(accepted.metadata().output().signatures().len(), 2);
    assert_eq!(
        bundle_answer_sets(&accepted)
            .first()
            .expect("full model")
            .len(),
        3
    );
    assert_eq!(
        displays(&bundle_answer_sets(&accepted), accepted.metadata().output())
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
    let error = admit_bundle_formula(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        resource_limits::expansion(ExpansionResource::MetadataStatements, 4),
        FormulaLimits::default(),
    )
    .expect_err("global metadata ceiling");
    assert!(matches!(
        error.error(),
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::MetadataStatements,
            observed: 5,
            limit: 4,
            ..
        })
    ));
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
                let shown = oracle::atoms(&witness["Value"])
                    .iter()
                    .map(|atom| atom_models::atom(atom))
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
#[ignore = "requires clingo: declarations and signature display match clingo without model projection"]
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
            let full = answer_sets(&accepted);
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
#[ignore = "requires clingo: included signature metadata matches clingo complete display multiplicities"]
fn included_signature_metadata_matches_clingo_complete_display_multiplicities() {
    let fixture = BundleFixture::new();
    for directives in ["#show.", "#show p/0.", "#show p/0. #show q/0."] {
        fixture.write(
            "entry.lp",
            &format!("#include \"child.lp\". #defined p/0. {directives}"),
        );
        fixture.write("child.lp", "{p}. {q}. #show.");
        let accepted = admit_bundle_formula(
            fixture.bundle(),
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .expect("metadata bundle");
        let run = oracle::run_in(
            fixture.directory(),
            ["entry.lp", "0", "--outf=2", "--warn=none"],
            &oracle::DECIDED,
            oracle::Limits::default(),
        );
        assert_eq!(
            displays(&bundle_answer_sets(&accepted), accepted.metadata().output()),
            oracle_json(run.stdout())
        );
    }
}
