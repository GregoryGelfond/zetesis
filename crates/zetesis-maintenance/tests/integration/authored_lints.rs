//! Authored source cannot suppress the project's dead-code diagnostics.
//! Compiler-generated attributes and procedural macro expansions are outside
//! this token audit. Rust's deny gate still applies to the compiled code.

mod lint_attributes;

use crate::support::{authored_sources, repository, source};

#[test]
fn authored_code_preserves_dead_code_diagnostics() {
    let root = repository();
    let sources = authored_sources(&root);
    assert!(
        !sources.is_empty(),
        "the maintained inventory must be present"
    );
    let mut violations = Vec::new();
    for path in &sources {
        let text = source(path).unwrap();
        let findings = lint_attributes::suppressions(&text)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for finding in findings {
            violations.push(format!(
                "{}:{finding}",
                path.strip_prefix(&root).unwrap().display()
            ));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
    println!("authored_rust_sources={}", sources.len());
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
        found.iter().map(|row| row.lint).collect::<Vec<_>>(),
        ["dead_code", "warnings"]
    );
}

#[test]
fn literal_macro_attributes_remain_visible() {
    let source = r"macro_rules! item { ($name:ident) => { #[allow(unused)] fn $name() {} }; }
macro_rules! inner { () => { #![expect(dead_code)] }; }";
    let found = lint_attributes::suppressions(source).unwrap();
    assert_eq!(
        found.iter().map(|row| row.lint).collect::<Vec<_>>(),
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
fn unrelated_specific_lints_remain_outside_this_policy() {
    let source = r#"#[expect(clippy::trivially_copy_pass_by_ref, reason = "serde's skip callback receives a borrowed field")]
    fn serialize() {}
    #[expect(clippy::struct_field_names, reason = "Preserve the established serialized field names")]
    struct Record {}"#;
    assert!(lint_attributes::suppressions(source).unwrap().is_empty());
}

#[test]
fn malformed_lint_metadata_cannot_pass_the_audit() {
    assert!(lint_attributes::suppressions("#[allow(dead_code,,)] fn f() {}").is_err());
}
