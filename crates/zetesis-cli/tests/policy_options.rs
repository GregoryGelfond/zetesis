//! CLI spellings map directly to the library's execution policy values.

use clap::{CommandFactory, FromArgMatches, Parser, error::ErrorKind};
use zetesis_cli::{Backend, Grounder, Options, Oracle, SearchMethod, SolveConfig, SourceBatching};

fn nondefault_options() -> Options {
    // Distinct values detect crossed fields whose defaults happen to coincide.
    Options::try_parse_from([
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
        "--max-search-work",
        "12",
        "--max-search-decisions",
        "13",
        "--max-objective-work",
        "14",
        "--max-objective-bound-work",
        "15",
        "--max-objective-bindings",
        "16",
        "--max-objective-keys",
        "17",
        "--max-objective-key-bytes",
        "18",
        "--max-optimal-models",
        "19",
        "--max-optimal-atoms",
        "20",
        "--max-optimal-bytes",
        "21",
        "--batch-size",
        "22",
        "--workers",
        "23",
        "--completion-workers",
        "24",
        "--max-completion-scratch-bytes",
        "25",
        "--max-candidates",
        "26",
        "--max-carrier-atoms",
        "27",
        "--max-work",
        "28",
        "--max-source-work",
        "29",
        "--max-atoms",
        "30",
        "--max-substitutions",
        "31",
        "--max-ground-rules",
        "32",
        "--max-batch-bytes",
        "33",
        "--max-candidate-bytes",
        "34",
        "--max-projection-entries",
        "35",
        "--max-projection-nodes",
        "36",
        "--max-projection-bytes",
        "37",
        "--max-closure-bytes",
        "38",
        "--max-closure-batch-bytes",
        "39",
        "--max-reduct-bytes",
        "40",
    ])
    .unwrap()
}

#[test]
fn nondefault_options_preserve_each_solver_field() {
    let config = SolveConfig::from(&nondefault_options());
    assert_eq!(config.backend, Backend::Cpu);
    assert_eq!(config.grounder, Grounder::Lazy);
    assert_eq!(config.source_batching, SourceBatching::Worlds);
    assert_eq!(config.oracle, Oracle::Closure);
    assert!(config.stats);
    assert_eq!(config.models, 11);
    assert_eq!(config.max_search_work, 12);
    assert_eq!(config.max_search_decisions, 13);
    assert_eq!(config.max_objective_work, 14);
    assert_eq!(config.max_objective_bound_work, 15);
    assert_eq!(config.max_objective_bindings, 16);
    assert_eq!(config.max_objective_keys, 17);
    assert_eq!(config.max_objective_key_bytes, 18);
    assert_eq!(config.max_optimal_models, 19);
    assert_eq!(config.max_optimal_atoms, 20);
    assert_eq!(config.max_optimal_bytes, 21);
    assert_eq!(config.batch_size.get(), 22);
    assert_eq!(config.workers.get(), 23);
    assert_eq!(config.completion_workers.get(), 24);
    assert_eq!(config.max_completion_scratch_bytes, 25);
    assert_eq!(config.search, SearchMethod::Regions);
    assert_eq!(config.max_candidates, 26);
    assert_eq!(config.max_carrier_atoms, 27);
    assert_eq!(config.max_work, 28);
    assert_eq!(config.max_source_work, 29);
    assert_eq!(config.max_atoms, 30);
    assert_eq!(config.max_substitutions, 31);
    assert_eq!(config.max_ground_rules, 32);
    assert_eq!(config.max_batch_bytes, 33);
    assert_eq!(config.max_candidate_bytes, 34);
    assert_eq!(config.max_projection_entries, 35);
    assert_eq!(config.max_projection_nodes, 36);
    assert_eq!(config.max_projection_bytes, 37);
    assert_eq!(config.max_closure_bytes, 38);
    assert_eq!(config.max_closure_batch_bytes, 39);
    assert_eq!(config.max_reduct_bytes, 40);
}

#[test]
fn an_omitted_closure_allowance_is_the_collective_share_per_worker() {
    let derived = SolveConfig::from(
        &Options::try_parse_from(["zetesis", "--workers", "8", "--memory", "2147483648"]).unwrap(),
    );
    assert_eq!(
        derived.max_closure_bytes,
        SolveConfig::DEFAULT.max_closure_batch_bytes / 8
    );
    assert!(derived.validate().is_ok());
    let explicit = SolveConfig::from(
        &Options::try_parse_from(["zetesis", "--workers", "8", "--max-closure-bytes", "5"])
            .unwrap(),
    );
    assert_eq!(explicit.max_closure_bytes, 5);
    let collective = SolveConfig::from(
        &Options::try_parse_from([
            "zetesis",
            "--workers",
            "3",
            "--max-closure-batch-bytes",
            "300",
        ])
        .unwrap(),
    );
    assert_eq!(collective.max_closure_bytes, 100);
}

#[test]
fn byte_ceilings_are_the_library_defaults_scaled_by_the_memory_allowance() {
    // At the reference allowance every byte ceiling is the library's; at
    // twice it, every one is doubled and the others are unchanged.
    let reference = SolveConfig::from(
        &Options::try_parse_from(["zetesis", "--workers", "4", "--memory", "2147483648"]).unwrap(),
    );
    assert_eq!(
        format!("{reference:?}"),
        format!("{:?}", SolveConfig::DEFAULT)
    );
    let doubled = SolveConfig::from(
        &Options::try_parse_from(["zetesis", "--workers", "4", "--memory", "4294967296"]).unwrap(),
    );
    let library = SolveConfig::DEFAULT;
    assert_eq!(
        doubled.max_projection_bytes,
        2 * library.max_projection_bytes
    );
    assert_eq!(
        doubled.max_objective_key_bytes,
        2 * library.max_objective_key_bytes
    );
    assert_eq!(doubled.max_optimal_bytes, 2 * library.max_optimal_bytes);
    assert_eq!(doubled.max_reduct_bytes, 2 * library.max_reduct_bytes);
    assert_eq!(
        doubled.max_completion_scratch_bytes,
        2 * library.max_completion_scratch_bytes
    );
    assert_eq!(doubled.max_candidate_bytes, 2 * library.max_candidate_bytes);
    assert_eq!(
        doubled.max_closure_batch_bytes,
        2 * library.max_closure_batch_bytes
    );
    assert_eq!(doubled.max_closure_bytes, 2 * library.max_closure_bytes);
    assert_eq!(doubled.max_batch_bytes, 2 * library.max_batch_bytes);
    let unscaled = SolveConfig {
        max_projection_bytes: library.max_projection_bytes,
        max_objective_key_bytes: library.max_objective_key_bytes,
        max_optimal_bytes: library.max_optimal_bytes,
        max_reduct_bytes: library.max_reduct_bytes,
        max_completion_scratch_bytes: library.max_completion_scratch_bytes,
        max_candidate_bytes: library.max_candidate_bytes,
        max_closure_batch_bytes: library.max_closure_batch_bytes,
        max_closure_bytes: library.max_closure_bytes,
        max_batch_bytes: library.max_batch_bytes,
        ..doubled
    };
    assert_eq!(format!("{unscaled:?}"), format!("{library:?}"));
    // A given ceiling is taken as given, whatever the allowance.
    let given = SolveConfig::from(
        &Options::try_parse_from([
            "zetesis",
            "--memory",
            "4294967296",
            "--max-reduct-bytes",
            "7",
        ])
        .unwrap(),
    );
    assert_eq!(given.max_reduct_bytes, 7);
    // The default allowance is at least the reference.
    let host = Options::try_parse_from(["zetesis"]).unwrap();
    assert!(host.memory >= 2_147_483_648);
    assert!(host.max_reduct_bytes.is_none());
}

#[test]
fn workers_default_to_the_host_parallelism() {
    let options = Options::try_parse_from(["zetesis"]).unwrap();
    let host = std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN);
    assert_eq!(options.workers, host);
    assert_eq!(options.completion_workers.get(), 1);
}

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

#[test]
fn formula_device_limits_remain_distinct_from_cpu_work() {
    for (work, rounds) in [(0, 0), (789, 17), (u32::MAX, u32::MAX)] {
        let options = Options::try_parse_from([
            "zetesis",
            "--max-work",
            "18446744073709551615",
            "--gpu-formula-work",
            &work.to_string(),
            "--gpu-formula-rounds",
            &rounds.to_string(),
        ])
        .unwrap();
        let config = SolveConfig::from(&options);
        assert_eq!(config.max_work, u64::MAX);
        assert_eq!(config.gpu_formula_work, work);
        assert_eq!(config.gpu_formula_rounds, rounds);
    }
    let defaults = SolveConfig::from(&Options::try_parse_from(["zetesis"]).unwrap());
    assert_eq!(defaults.gpu_formula_work, 100_000_000);
    assert_eq!(defaults.gpu_formula_rounds, 64);
}

#[test]
fn formula_device_limits_reject_unrepresentable_values() {
    for flag in ["--gpu-formula-work", "--gpu-formula-rounds"] {
        for value in ["4294967296", "18446744073709551615"] {
            let error = Options::try_parse_from(["zetesis", flag, value]).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::ValueValidation);
        }
    }
}

#[test]
fn the_expansion_byte_budget_is_an_option_with_the_library_default() {
    let options = Options::try_parse_from(["zetesis"]).unwrap();
    assert_eq!(
        options.max_expansion_bytes,
        zetesis_themelios::ExpansionLimits::default().max_scalar_bytes
    );
    let raised = Options::try_parse_from(["zetesis", "--max-expansion-bytes", "5"]).unwrap();
    assert_eq!(raised.max_expansion_bytes, 5);
}
