//! CLI spellings map directly to the library's execution policy values.

use clap::{CommandFactory, FromArgMatches, Parser, error::ErrorKind};
use zetesis_cli::{Backend, Grounder, Options, Oracle, SolveConfig, SourceBatching};

#[test]
fn backend_spellings_select_typed_policies() {
    for (spelling, expected) in [
        ("auto", Backend::Auto),
        ("cpu", Backend::Cpu),
        ("gpu", Backend::Gpu),
        ("metal", Backend::Metal),
        ("vulkan", Backend::Vulkan),
        ("dx12", Backend::Dx12),
        ("gl", Backend::Gl),
        ("nvidia", Backend::Nvidia),
    ] {
        let options = Options::try_parse_from(["zetesis", "--backend", spelling]).unwrap();
        assert_eq!(options.backend, expected);
    }
}

#[test]
fn grounder_spellings_select_typed_policies() {
    for (spelling, expected) in [
        ("auto", Grounder::Auto),
        ("lazy", Grounder::Lazy),
        ("eager", Grounder::Eager),
    ] {
        let options = Options::try_parse_from(["zetesis", "--grounder", spelling]).unwrap();
        assert_eq!(options.grounder, expected);
    }
}

#[test]
fn source_batching_spellings_select_typed_policies() {
    for (spelling, expected) in [
        ("independent", SourceBatching::Independent),
        ("union", SourceBatching::Union),
        ("worlds", SourceBatching::Worlds),
    ] {
        let options = Options::try_parse_from(["zetesis", "--source-batching", spelling]).unwrap();
        assert_eq!(options.source_batching, expected);
    }
}

#[test]
fn oracle_spellings_select_typed_policies() {
    for (spelling, expected) in [
        ("auto", Oracle::Auto),
        ("closure", Oracle::Closure),
        ("countermodel", Oracle::Countermodel),
    ] {
        let options = Options::try_parse_from(["zetesis", "--oracle", spelling]).unwrap();
        assert_eq!(options.oracle, expected);
    }
}

#[test]
fn policy_defaults_match_the_solver_configuration() {
    let options = Options::try_parse_from(["zetesis"]).unwrap();
    assert_eq!(options.backend, SolveConfig::DEFAULT.backend);
    assert_eq!(options.grounder, SolveConfig::DEFAULT.grounder);
    assert_eq!(
        options.source_batching,
        SolveConfig::DEFAULT.source_batching
    );
    assert_eq!(options.oracle, SolveConfig::DEFAULT.oracle);
}

#[test]
fn policy_values_retain_case_sensitive_refusal() {
    for (flag, value) in [
        ("--backend", "METAL"),
        ("--grounder", "LAZY"),
        ("--source-batching", "WORLDS"),
        ("--oracle", "COUNTERMODEL"),
    ] {
        let error = Options::try_parse_from(["zetesis", flag, value]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert!(
            error
                .to_string()
                .contains(&format!("invalid value '{value}'"))
        );
    }
}

#[test]
fn invalid_policy_values_report_ordered_choices() {
    for (flag, choices) in [
        (
            "--backend",
            "auto, cpu, gpu, metal, vulkan, dx12, gl, nvidia",
        ),
        ("--grounder", "auto, lazy, eager"),
        ("--source-batching", "independent, union, worlds"),
        ("--oracle", "auto, closure, countermodel"),
    ] {
        let error = Options::try_parse_from(["zetesis", flag, "bogus"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert!(
            error
                .to_string()
                .contains(&format!("[possible values: {choices}]"))
        );
    }
}

#[test]
fn misspelled_policy_retains_clap_suggestion() {
    let error = Options::try_parse_from(["zetesis", "--backend", "meta"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    assert!(
        error
            .to_string()
            .contains("tip: a similar value exists: 'metal'")
    );
}

#[test]
fn caller_case_policy_preserves_the_typed_value() {
    let matches = Options::command()
        .mut_arg("backend", |argument| argument.ignore_case(true))
        .try_get_matches_from(["zetesis", "--backend", "METAL"])
        .unwrap();
    assert_eq!(
        Options::from_arg_matches(&matches).unwrap().backend,
        Backend::Metal
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_policy_values_remain_parser_errors() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt as _};
    for flag in ["--backend", "--grounder", "--source-batching", "--oracle"] {
        let error = Options::try_parse_from([
            OsString::from("zetesis"),
            OsString::from(flag),
            OsString::from_vec(vec![0xff]),
        ])
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidUtf8);
    }
}
