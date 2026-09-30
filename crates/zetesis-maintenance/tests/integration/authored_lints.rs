//! Authored source suppresses no lint outside its named foreign-interface
//! exceptions. Every `#[allow]` is refused, as is every `#[expect]` the list
//! below does not name, and every entry naming no present `#[expect]`, so the
//! list cannot outlive its code. Compiler-generated attributes and procedural
//! macro expansions are outside this token audit; Rust's deny gate still applies
//! to the compiled code. Nor can a manifest set lints of its own: every
//! workspace member's `[lints]` table hands its policy to the workspace's.

mod lint_attributes;

use lint_attributes::Suppression;

use crate::support::{authored_sources, repository, source};

/// An `#[expect]` a foreign interface requires: the file, the item it
/// annotates and the lint, with the interface constraint that requires it, as
/// CONTRIBUTING asks. Adding one is a change to this audit, reviewed with the
/// code that needs it.
struct Exception {
    file: &'static str,
    item: &'static str,
    lint: &'static str,
    constraint: &'static str,
}

/// The named exceptions.
const EXCEPTIONS: &[Exception] = &[];

/// Whether `exception` names `suppression`, found in `file`.
fn names(exception: &Exception, file: &str, suppression: &Suppression) -> bool {
    exception.file == file
        && suppression.item.as_deref() == Some(exception.item)
        && suppression.lint == exception.lint
}

/// The suppressions among `found`, each with its file, that the policy refuses,
/// and the entries of `exceptions` naming no present `#[expect]`.
fn violations(found: &[(String, Suppression)], exceptions: &[Exception]) -> Vec<String> {
    let mut violations: Vec<String> = found
        .iter()
        .filter(|(file, suppression)| {
            suppression.level == "allow"
                || !exceptions
                    .iter()
                    .any(|exception| names(exception, file, suppression))
        })
        .map(|(file, suppression)| format!("{file}:{suppression} is outside the named exceptions"))
        .collect();
    violations.extend(
        exceptions
            .iter()
            .filter(|exception| {
                !found.iter().any(|(file, suppression)| {
                    suppression.level == "expect" && names(exception, file, suppression)
                })
            })
            .map(|exception| {
                format!(
                    "{}: the exception for {} on `{}` ({}) names no present #[expect]",
                    exception.file, exception.lint, exception.item, exception.constraint
                )
            }),
    );
    violations
}

#[test]
fn authored_code_suppresses_no_lint_outside_the_named_exceptions() {
    let root = repository();
    let sources = authored_sources(&root);
    assert!(
        !sources.is_empty(),
        "the maintained inventory must be present"
    );
    let mut found = Vec::new();
    for path in &sources {
        let text = source(path).unwrap();
        let file = path.strip_prefix(&root).unwrap().display().to_string();
        let suppressions =
            lint_attributes::suppressions(&text).unwrap_or_else(|error| panic!("{file}: {error}"));
        found.extend(
            suppressions
                .into_iter()
                .map(|suppression| (file.clone(), suppression)),
        );
    }
    let violations = violations(&found, EXCEPTIONS);
    assert!(violations.is_empty(), "{}", violations.join("\n"));
    println!("authored_rust_sources={}", sources.len());
}

/// Whether `manifest` hands its lint policy to the workspace: its `[lints]`
/// table is `workspace = true` alone, and it has no lint table of its own.
fn inherits_the_workspace_lints(manifest: &str) -> bool {
    let mut table = "";
    let mut inherited = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            table = line;
            if table.starts_with("[lints.") {
                return false;
            }
        } else if table == "[lints]" && !line.is_empty() && !line.starts_with('#') {
            if line.split_whitespace().collect::<String>() != "workspace=true" {
                return false;
            }
            inherited = true;
        }
    }
    inherited
}

#[test]
fn every_member_manifest_inherits_the_workspace_lints() {
    let members: Vec<_> = std::fs::read_dir(repository().join("crates"))
        .unwrap()
        .map(|entry| entry.unwrap().path().join("Cargo.toml"))
        .filter(|manifest| manifest.is_file())
        .collect();
    assert!(!members.is_empty(), "the workspace members must be present");
    let outside: Vec<_> = members
        .iter()
        .filter(|manifest| !inherits_the_workspace_lints(&source(manifest).unwrap()))
        .map(|manifest| manifest.display().to_string())
        .collect();
    assert!(outside.is_empty(), "{}", outside.join("\n"));
}

#[test]
fn a_manifest_with_lints_of_its_own_is_refused() {
    let package = "[package]\nname = \"member\"\n";
    assert!(inherits_the_workspace_lints(&format!(
        "{package}\n[lints]\nworkspace = true\n"
    )));
    for lints in [
        "",
        "\n[lints]\nworkspace = false\n",
        "\n[lints.clippy]\npedantic = \"allow\"\n",
        "\n[lints]\nworkspace = true\n\n[lints.rust]\nunused = \"allow\"\n",
    ] {
        let manifest = format!("{package}{lints}");
        assert!(!inherits_the_workspace_lints(&manifest), "{manifest}");
    }
}

/// An exception of the shape CONTRIBUTING describes, for the fixtures below.
const INTERFACE: Exception = Exception {
    file: "src/interface.rs",
    item: "field_name",
    lint: "clippy::struct_field_names",
    constraint: "the foreign schema names the field",
};

fn located(source: &str) -> Vec<(String, Suppression)> {
    lint_attributes::suppressions(source)
        .unwrap()
        .into_iter()
        .map(|suppression| (INTERFACE.file.to_owned(), suppression))
        .collect()
}

#[test]
fn an_expect_its_exception_names_is_accepted() {
    let found = located(
        "struct Record {\n    #[expect(clippy::struct_field_names, reason = \"schema\")]\n    field_name: u8,\n}",
    );
    assert!(violations(&found, &[INTERFACE]).is_empty());
}

#[test]
fn an_allow_is_refused_where_an_exception_names_its_lint() {
    let found = located(
        "struct Record {\n    #[allow(clippy::struct_field_names)]\n    field_name: u8,\n}",
    );
    // The allow itself, and the exception, which names no present expect.
    assert_eq!(violations(&found, &[INTERFACE]).len(), 2);
}

#[test]
fn an_expect_no_exception_names_is_refused() {
    let found = located("#[expect(clippy::too_many_lines, reason = \"long\")]\nfn long() {}");
    assert_eq!(violations(&found, &[]).len(), 1);
    // An exception for another item does not cover it.
    let found = located(
        "struct Record {\n    #[expect(clippy::struct_field_names)]\n    other_name: u8,\n}",
    );
    assert_eq!(violations(&found, &[INTERFACE]).len(), 2);
}

#[test]
fn an_exception_naming_no_present_expect_is_refused() {
    let violations = violations(&[], &[INTERFACE]);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("names no present #[expect]"));
}

#[test]
fn a_suppression_names_the_item_it_annotates() {
    let source = r#"#![expect(clippy::module_name_repetitions)]
#[expect(clippy::too_many_lines)]
pub(crate) async fn run() {}
#[doc = "a record"]
#[expect(clippy::struct_field_names)]
pub struct Record {
    #[expect(clippy::struct_field_names)]
    pub(super) record_name: u8,
}
#[expect(improper_ctypes_definitions)]
extern "C" fn callback() {}
fn body() {
    #[expect(clippy::cast_possible_truncation)]
    let mut narrow = 0;
}
#[expect(clippy::use_self)]
impl<T> Trait for Record {}"#;
    let items: Vec<(String, Option<String>)> = lint_attributes::suppressions(source)
        .unwrap()
        .into_iter()
        .map(|suppression| (suppression.lint, suppression.item))
        .collect();
    assert_eq!(
        items,
        [
            ("clippy::module_name_repetitions".into(), None),
            ("clippy::too_many_lines".into(), Some("run".into())),
            ("clippy::struct_field_names".into(), Some("Record".into())),
            (
                "clippy::struct_field_names".into(),
                Some("record_name".into())
            ),
            (
                "improper_ctypes_definitions".into(),
                Some("callback".into())
            ),
            (
                "clippy::cast_possible_truncation".into(),
                Some("narrow".into())
            ),
            ("clippy::use_self".into(), None),
        ]
    );
}

#[test]
fn suppression_levels_cannot_hide_required_lints() {
    for level in ["allow", "expect"] {
        for lint in ["dead_code", "unused", "warnings"] {
            for prefix in ["#", "#!"] {
                let source =
                    format!("{prefix}[{level}({lint}, reason = \"explanation\")] fn f() {{}}");
                let found = lint_attributes::suppressions(&source).unwrap();
                assert_eq!(found.len(), 1, "{source}");
                assert_eq!(found[0].level, level);
                assert_eq!(found[0].lint, lint);
                assert_eq!(found[0].line, 1);
                assert!(found[0].column > 0);
            }
        }
    }
}

#[test]
fn inactive_configuration_cannot_hide_suppression() {
    let source = r#"#[cfg_attr(any(), cfg_attr(feature = "future", allow(dead_code)), expect(warnings))] fn f() {}"#;
    let found = lint_attributes::suppressions(source).unwrap();
    assert_eq!(
        found
            .iter()
            .map(|row| row.lint.as_str())
            .collect::<Vec<_>>(),
        ["dead_code", "warnings"]
    );
}

#[test]
fn literal_macro_attributes_remain_visible() {
    let source = r"macro_rules! item { ($name:ident) => { #[allow(unused)] fn $name() {} }; }
macro_rules! inner { () => { #![expect(dead_code)] }; }";
    let found = lint_attributes::suppressions(source).unwrap();
    assert_eq!(
        found
            .iter()
            .map(|row| row.lint.as_str())
            .collect::<Vec<_>>(),
        ["unused", "dead_code"]
    );
}

#[test]
fn raw_identifiers_preserve_lint_identity() {
    let source = "#[r#allow(r#dead_code)] fn f() {}";
    let found = lint_attributes::suppressions(source).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].lint, "dead_code");
}

#[test]
fn quoted_attributes_are_not_authored_suppressions() {
    let source = r##"
        // #[allow(dead_code)]
        /* #![expect(warnings)] */
        const PLAIN: &str = "#[allow(unused)]";
        const RAW: &str = r#"#[cfg_attr(test, expect(dead_code))]"#;
        #[doc = "#[allow(dead_code)]"] fn f() {}
    "##;
    assert!(lint_attributes::suppressions(source).unwrap().is_empty());
}

#[test]
fn specific_lints_are_suppressions_too() {
    let source = r#"#[expect(clippy::trivially_copy_pass_by_ref, reason = "serde's skip callback receives a borrowed field")]
    fn serialize() {}
    #[expect(clippy::struct_field_names, reason = "Preserve the established serialized field names")]
    struct Record {}"#;
    let found = lint_attributes::suppressions(source).unwrap();
    assert_eq!(
        found
            .iter()
            .map(|row| (row.lint.as_str(), row.item.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("clippy::trivially_copy_pass_by_ref", Some("serialize")),
            ("clippy::struct_field_names", Some("Record")),
        ]
    );
}

#[test]
fn malformed_lint_metadata_cannot_pass_the_audit() {
    assert!(lint_attributes::suppressions("#[allow(dead_code,,)] fn f() {}").is_err());
}
