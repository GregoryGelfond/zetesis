//! Restricted import syntax must fail closed before provenance is trusted.

use super::*;

#[test]
fn adjacent_literals_preserve_decoded_source_bytes() {
    let input = r#" /* leading */ "p(\"x\").\n" // between
        "q(\\).\t\r" /* trailing */ "#;
    assert_eq!(literals(input).unwrap(), "p(\"x\").\nq(\\).\t\r");
}

#[test]
fn unsupported_literal_forms_fail_closed() {
    // Raw/prefixed strings, unsupported escapes and literal newlines require
    // a different import contract; none may silently change the source bytes.
    for input in [
        "",
        "/* empty */",
        "\"unfinished",
        "\"line\nbreak\"",
        "\"line\rbreak\"",
        r#""\x70""#,
        r#""\u0070""#,
        r#"R"(p.)""#,
        r#"u8"p.""#,
        "\"p.\" /* unfinished",
    ] {
        assert!(
            matches!(literals(input), Err(Error::Literal(_))),
            "{input:?}"
        );
    }
}

#[test]
fn balanced_arguments_ignore_nested_separators() {
    let input = r#"("x,]", {a, b}, [c, d] /* , */) suffix"#;
    let (end, separators) = balanced(input, 0).unwrap();
    assert_eq!(&input[end..], ") suffix");
    let pieces: Vec<_> = separators
        .iter()
        .map(|&index| &input[index..index + 2])
        .collect();
    assert_eq!(pieces, [", ", ", "]);
}

#[test]
fn malformed_delimiters_never_complete_an_expression() {
    for input in [
        "x",
        "(]",
        "([)",
        "(",
        "(// unfinished",
        "(/* unfinished",
        "('x')",
        "(λ)",
    ] {
        assert!(
            matches!(balanced(input, 0), Err(Error::Literal(_))),
            "{input:?}"
        );
    }
}

#[test]
fn assertion_decoding_preserves_its_contract() {
    let input = r#"REQUIRE("([[p]],[])" == IO::to_string(solve("p." "\n", {"p"})))"#;
    let (source, arguments, expected) = assertion(input).unwrap();
    assert_eq!(source, "p.\n");
    assert_eq!(prefixes(&arguments).unwrap(), ["p"]);
    assert_eq!(helper_models(&expected).unwrap(), [vec!["p"]]);
}

#[test]
fn unsupported_assertion_wrappers_are_refused() {
    for input in [
        "CHECK(1)",
        "REQUIRE 1",
        "REQUIRE(1)",
        r#"REQUIRE("[]" == solve("p."))"#,
        r#"REQUIRE("[]" == IO::to_string(other("p.")))"#,
        r#"REQUIRE("[]" == IO::to_string(solve("p.") trailing))"#,
        r#"REQUIRE("[]" == IO::to_string(solve("p.")) trailing)"#,
        r#"REQUIRE("[]" == IO::to_string(solve("p."))) trailing"#,
        r#"REQUIRE("[]" == IO::to_string(solve("p.", {}, 1)))"#,
    ] {
        assert!(
            matches!(assertion(input), Err(Error::Literal(_))),
            "{input:?}"
        );
    }
}

#[test]
fn omitted_prefixes_retain_the_full_helper_view() {
    assert_eq!(prefixes(&[]).unwrap(), [""]);
}

#[test]
fn empty_prefixes_retain_no_helper_atoms() {
    assert!(prefixes(&["{}".into()]).unwrap().is_empty());
}

#[test]
fn unsupported_prefix_contracts_are_refused() {
    for arguments in [
        vec!["{}".into(), "1".into()],
        vec!["all".into()],
        vec!["{} trailing".into()],
    ] {
        assert!(matches!(prefixes(&arguments), Err(Error::Literal(_))));
    }
}

#[test]
fn helper_models_preserve_nested_atom_arguments() {
    assert_eq!(
        helper_models("([[q(1,2),p],[p]],[])").unwrap(),
        [vec!["p"], vec!["p", "q(1,2)"]]
    );
}

#[test]
fn helper_models_preserve_duplicate_occurrences() {
    assert_eq!(
        helper_models("([[p],[p]],[])").unwrap(),
        [vec!["p"], vec!["p"]]
    );
}

#[test]
fn an_empty_model_differs_from_no_helper_models() {
    assert!(helper_models("([],[])").unwrap().is_empty());
    assert_eq!(helper_models("([[]],[])").unwrap(), [Vec::<String>::new()]);
}

#[test]
fn malformed_helper_families_fail_closed() {
    for input in [
        "[]",
        "([\"p\"],[])",
        "([p],[])",
        "([[p][q]],[])",
        "([[p]])",
        "([[p]],[]",
    ] {
        assert!(
            matches!(helper_models(input), Err(Error::Literal(_))),
            "{input:?}"
        );
    }
}

#[test]
fn assertion_identity_ignores_quoted_and_commented_tokens() {
    let input = "SECTION(\"old\") REQUIRE(1) SECTION(\"new\") \"REQUIRE\" /* REQUIRE */ REQUIRE(2) REQUIRE(3)";
    let target = input.rfind("REQUIRE").unwrap();
    assert_eq!(identity(input, target).unwrap(), ("new".into(), 2));
}

#[test]
fn nonassertion_coordinates_are_refused() {
    for (input, target) in [("REQUIRE(1)", 1), ("/* REQUIRE */", 3), ("λ REQUIRE(1)", 3)] {
        assert!(matches!(identity(input, target), Err(Error::Literal(_))));
    }
}
