//! Public options select algorithms and the shared ordinary resource policy.

use clap::{CommandFactory, FromArgMatches, Parser, error::ErrorKind};
use zetesis_cli::{Backend, Grounder, Options, Oracle, SearchMethod, SolveConfig, SourceBatching};

#[test]
fn algorithm_options_preserve_the_selected_policies() {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--grounder",
        "lazy",
        "--source-batching",
        "worlds",
        "--oracle",
        "closure",
        "--search",
        "regions",
        "--stats",
        "--models",
        "11",
        "--threads",
        "3",
        "--memory",
        "4GiB",
    ])
    .unwrap();
    let config = SolveConfig::from(&options);
    assert_eq!(config.backend, Backend::Cpu);
    assert_eq!(config.grounder, Grounder::Lazy);
    assert_eq!(config.source_batching, SourceBatching::Worlds);
    assert_eq!(config.oracle, Oracle::Closure);
    assert_eq!(config.search, SearchMethod::Regions);
    assert!(config.stats);
    assert_eq!(config.models, 11);
    assert_eq!(config.workers.get(), 3);
}

#[test]
fn resource_options_use_the_library_policy() {
    for memory in [0, 1, 2_147_483_648, 4_294_967_296] {
        let options =
            Options::try_parse_from(["zetesis", "--threads", "4", "--memory", &memory.to_string()])
                .unwrap();
        let expected = zetesis_solve::Resources::new(memory, options.workers).solve_config();
        let actual = SolveConfig::from(&options);
        assert_eq!(actual.max_closure_bytes, expected.max_closure_bytes);
        assert_eq!(
            actual.max_closure_batch_bytes,
            expected.max_closure_batch_bytes
        );
        assert_eq!(
            actual.max_completion_scratch_bytes,
            expected.max_completion_scratch_bytes
        );
        assert_eq!(actual.max_model_bytes, expected.max_model_bytes);
        assert_eq!(actual.constraints, expected.constraints);
        assert_eq!(actual.max_work, u64::MAX);
        assert_eq!(actual.max_search_work, u64::MAX);
        assert_eq!(actual.max_source_work, u64::MAX);
        assert_eq!(actual.max_objective_work, u64::MAX);
    }
}

#[test]
fn removed_stage_controls_are_parser_errors() {
    for flag in [
        "--max-json-record-bytes",
        "--max-search-work",
        "--max-search-decisions",
        "--max-projection-entries",
        "--max-projection-nodes",
        "--max-projection-bytes",
        "--max-expansion-work",
        "--max-support-bytes",
        "--max-expanded-templates",
        "--max-expansion-values",
        "--max-expansion-bytes",
        "--max-domain-values",
        "--max-assignment-values",
        "--max-generated-values",
        "--max-support-rounds",
        "--max-model-work",
        "--max-model-bytes",
        "--max-objective-work",
        "--max-observation-work",
        "--max-observation-bindings",
        "--max-observation-terms",
        "--max-observation-bytes",
        "--max-objective-bound-work",
        "--max-objective-bindings",
        "--max-objective-keys",
        "--max-objective-key-bytes",
        "--max-optimal-models",
        "--max-optimal-atoms",
        "--max-optimal-bytes",
        "--batch-size",
        "--completion-workers",
        "--max-reduct-bytes",
        "--max-completion-scratch-bytes",
        "--max-candidates",
        "--max-candidate-bytes",
        "--max-carrier-atoms",
        "--max-work",
        "--max-closure-bytes",
        "--max-closure-batch-bytes",
        "--gpu-formula-work",
        "--gpu-formula-rounds",
        "--max-source-work",
        "--max-atoms",
        "--max-source-bytes",
        "--max-source-roots",
        "--max-source-files",
        "--max-total-source-bytes",
        "--max-include-depth",
        "--max-substitutions",
        "--max-ground-rules",
        "--max-batch-bytes",
    ] {
        for prefix in [vec!["zetesis"], vec!["zetesis", "solve"]] {
            let error =
                zetesis_cli::Invocation::try_parse_from(prefix.into_iter().chain([flag, "1"]))
                    .unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnknownArgument, "{flag}");
        }
    }
}

#[test]
fn automatic_workers_use_the_host_parallelism() {
    let options = Options::try_parse_from(["zetesis"]).unwrap();
    let host = std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN);
    assert_eq!(options.workers, host);
}

#[test]
fn backend_spellings_select_typed_policies() {
    for expected in Backend::ALL {
        let options = Options::try_parse_from(["zetesis", "--backend", expected.label()]).unwrap();
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
fn search_spellings_select_typed_policies() {
    for (spelling, expected) in [
        ("regions", SearchMethod::Regions),
        ("clauses", SearchMethod::Clauses),
    ] {
        let options = Options::try_parse_from(["zetesis", "--search", spelling]).unwrap();
        assert_eq!(options.search, expected);
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
    assert_eq!(options.search, SolveConfig::DEFAULT.search);
}

#[test]
fn policy_values_retain_case_sensitive_refusal() {
    for (flag, value) in [
        ("--backend", "METAL"),
        ("--grounder", "LAZY"),
        ("--source-batching", "WORLDS"),
        ("--oracle", "COUNTERMODEL"),
        ("--search", "CLAUSES"),
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
        ("--backend", "cpu, gpu, metal, vulkan"),
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
        Backend::Gpu(Some(zetesis_backend::GpuApi::Metal))
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
