//! Adversarial contracts are checked independently of the manifest seal.
use super::{Error, Limits, Resource, contracts, cpp, document, files};
use std::path::Path;

fn document() -> document::Document {
    serde_json::from_str(include_str!(
        "../../../../validation/upstream/clingo-5.8.2/curated/manifest.json"
    ))
    .unwrap()
}
#[test]
fn a_parent_component_is_not_a_relative_corpus_path() {
    for path in [
        "../secret",
        "programs/../secret",
        "/root",
        "programs//p.lp",
        "programs/./p.lp",
        "",
        r"programs\p.lp",
        "C:source.lp",
    ] {
        assert!(
            matches!(files::relative(path), Err(Error::Path(_))),
            "{path}"
        );
    }
}
#[test]
fn duplicate_case_ids_cannot_change_the_selected_population() {
    let mut document = document();
    document.cases[1].id = document.cases[0].id.clone();
    assert!(
        matches!(document::validate(&document,Limits::default()),Err(Error::Contract(message)) if message.contains("duplicate"))
    );
}
#[test]
fn duplicate_case_paths_cannot_alias_independent_assertions() {
    let mut document = document();
    document.cases[1].path = document.cases[0].path.clone();
    assert!(matches!(
        document::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn a_case_path_must_derive_from_its_original_identity() {
    let mut document = document();
    document.cases[0].path = "programs/other.lp".into();
    assert!(matches!(
        document::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn a_case_cannot_claim_an_unretained_original_file() {
    let mut document = document();
    document.cases[0].provenance.source_file = "other.cc".into();
    assert!(matches!(
        document::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn an_assertion_span_must_have_its_exact_byte_length() {
    let mut document = document();
    document.cases[0].provenance.byte_end += 1;
    assert!(matches!(
        document::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn a_source_notice_cannot_lose_the_upstream_copyright() {
    let mut document = document();
    document.origins[0].copyright_notice.clear();
    assert!(matches!(
        document::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn helper_projection_preserves_repeated_empty_displays() {
    let mut document = document();
    let case = &mut document.cases[0];
    case.contract.full_models = vec![vec!["hidden(a)".into()], vec!["hidden(b)".into()]];
    case.contract.prefixes = vec!["shown(".into()];
    case.contract.helper_models = vec![vec![], vec![]];
    contracts::validate(&document, Limits::default()).unwrap();
    document.cases[0].contract.helper_models.pop();
    assert!(matches!(
        contracts::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn repeated_full_model_records_are_not_deduplicated() {
    let mut document = document();
    let case = &mut document.cases[0];
    case.contract.full_models = vec![vec!["a".into()], vec!["a".into()]];
    case.contract.prefixes = vec![String::new()];
    case.contract.helper_models = vec![vec!["a".into()], vec!["a".into()]];
    contracts::validate(&document, Limits::default()).unwrap();
    document.cases[0].contract.helper_models.pop();
    assert!(matches!(
        contracts::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn one_complete_model_cannot_repeat_an_atom() {
    let mut document = document();
    let case = &mut document.cases[0];
    case.contract.full_models = vec![vec!["a".into(), "a".into()]];
    case.contract.prefixes = vec![String::new()];
    case.contract.helper_models = vec![vec!["a".into(), "a".into()]];
    assert!(matches!(
        contracts::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn a_wrong_prefix_selection_changes_the_helper_contract() {
    let mut document = document();
    document.cases[0].contract.prefixes.clear();
    assert!(matches!(
        contracts::validate(&document, Limits::default()),
        Err(Error::Contract(_))
    ));
}
#[test]
fn model_occurrences_obey_their_inclusive_ceiling() {
    let document = document();
    let models = document
        .cases
        .iter()
        .map(|case| case.contract.full_models.len())
        .sum();
    contracts::validate(
        &document,
        Limits {
            models,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        contracts::validate(
            &document,
            Limits {
                models: models - 1,
                ..Limits::default()
            }
        ),
        Err(Error::Limit {
            resource: Resource::Models,
            ..
        })
    ));
}
#[test]
fn atom_occurrences_obey_their_inclusive_ceiling() {
    let document = document();
    let atoms = document
        .cases
        .iter()
        .flat_map(|case| &case.contract.full_models)
        .map(Vec::len)
        .sum();
    contracts::validate(
        &document,
        Limits {
            atoms,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        contracts::validate(
            &document,
            Limits {
                atoms: atoms - 1,
                ..Limits::default()
            }
        ),
        Err(Error::Limit {
            resource: Resource::Atoms,
            ..
        })
    ));
}
#[test]
fn adjacent_literals_retain_their_decoded_bytes() {
    assert_eq!(
        cpp::literals(r#""a.\n" /*x*/ "b(\"x\").""#).unwrap(),
        "a.\nb(\"x\")."
    );
}
#[test]
fn unsupported_cpp_literals_are_explicit_refusals() {
    for literal in [
        "prefix + \"p.\"",
        "R\"(p.)\"",
        r#""\x70.""#,
        r#""\u0070.""#,
        r#""\v""#,
        "",
    ] {
        assert!(cpp::literals(literal).is_err(), "{literal}");
    }
}
#[test]
fn delimiters_inside_strings_do_not_split_arguments() {
    let text = r#"("p(1;2).", {"x,]"}, /* ) */ {1,2})"#;
    let (end, commas) = cpp::balanced(text, 0).unwrap();
    assert_eq!(end, text.len() - 1);
    assert_eq!(commas.len(), 2);
}
#[test]
fn malformed_cpp_delimiters_are_refused() {
    for text in ["([)]", "(\"p.", "(p", "('x')", "abc()", "(α)"] {
        assert!(cpp::balanced(text, 0).is_err(), "{text}");
    }
}
#[test]
fn helper_output_keeps_model_multiplicity() {
    assert_eq!(
        cpp::helper_models("([[],[]],[])").unwrap(),
        vec![Vec::<String>::new(), Vec::new()]
    );
}
#[test]
fn an_existing_file_read_obeys_the_inclusive_byte_ceiling() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"abc").unwrap();
    assert_eq!(
        files::read(file.path(), 3, Resource::SourceBytes).unwrap(),
        b"abc"
    );
    assert!(matches!(
        files::read(file.path(), 2, Resource::SourceBytes),
        Err(Error::Limit {
            resource: Resource::SourceBytes,
            observed: 3,
            limit: 2
        })
    ));
}
#[test]
fn a_directory_is_not_a_source_file() {
    let root = tempfile::tempdir().unwrap();
    assert!(matches!(
        files::read(Path::new(root.path()), 10, Resource::SourceBytes),
        Err(Error::Path(_))
    ));
}
