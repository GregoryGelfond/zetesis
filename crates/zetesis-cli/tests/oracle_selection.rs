//! Automatic language admission preserves explicit execution requests, typed
//! resource refusals, and stable-model semantics across the two reduct oracles.

use std::collections::BTreeSet;

use clap::Parser;
use zetesis_cli::{
    Backend, Completion, Grounder, Interruption, Options, Oracle, Report, RunError,
    run_with_diagnostics,
};
use zetesis_cpu::{Control, Stop};
use zetesis_themelios::{
    AdmissionFailure, ExpansionFailure, FormulaFailure, FormulaResource, InputLimit,
};

fn options(arguments: &[&str]) -> Options {
    Options::try_parse_from(
        ["zetesis", "--backend", "cpu", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .expect("valid CLI arguments")
}

fn run(source: &str, options: &Options) -> (Result<Report, RunError>, String, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_with_diagnostics(
        source.to_owned(),
        options,
        &mut output,
        &mut diagnostics,
        &Control::default(),
    );
    (
        result,
        String::from_utf8(output).expect("UTF-8 model output"),
        String::from_utf8(diagnostics).expect("UTF-8 diagnostics"),
    )
}

fn answers(output: &str) -> Vec<BTreeSet<&str>> {
    let mut result = Vec::new();
    let mut lines = output.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            result.push(
                lines
                    .next()
                    .expect("complete model line")
                    .split_ascii_whitespace()
                    .collect(),
            );
        }
    }
    result
}

fn assert_refused_without_output(source: &str, options: &Options) -> RunError {
    let (result, output, diagnostics) = run(source, options);
    assert!(output.is_empty(), "no partial semantic claim: {output}");
    assert!(!diagnostics.contains("Reduct search:"));
    result.expect_err("source or execution policy must be refused")
}

#[test]
fn supported_language_is_automatic_with_optional_reduct_oracle_selection() {
    assert_eq!(options(&[]).oracle, Oracle::Auto);
    for (name, oracle) in [
        ("auto", Oracle::Auto),
        ("closure", Oracle::Closure),
        ("countermodel", Oracle::Countermodel),
    ] {
        assert_eq!(options(&["--oracle", name]).oracle, oracle);
    }
    assert!(Options::try_parse_from(["zetesis", "--extended"]).is_err());
    assert!(Options::try_parse_from(["zetesis", "--search", "sat"]).is_err());
}

#[test]
fn auto_uses_closure_for_supported_scalar_normal_programs() {
    for source in [
        "a.",
        "#const n=1+1. d(1..n). p(X) :- d(X), X != 1.",
        "a :- not b. b :- not a.",
    ] {
        let (automatic, auto_output, auto_diagnostics) = run(source, &options(&[]));
        let (explicit, explicit_output, explicit_diagnostics) =
            run(source, &options(&["--oracle", "closure"]));
        let automatic = automatic.expect("automatic closure admission");
        let explicit = explicit.expect("explicit closure admission");
        assert_eq!(automatic.completion, Completion::Exhausted);
        assert_eq!(automatic.models, explicit.models);
        assert_eq!(answers(&auto_output), answers(&explicit_output));
        assert!(auto_diagnostics.contains("Oracle: reduct closure"));
        assert!(explicit_diagnostics.contains("Oracle: reduct closure"));
        assert!(automatic.countermodel_statistics.is_none());
    }
}

#[test]
fn auto_uses_formula_membership_for_choices_and_variable_arithmetic() {
    for (source, count) in [
        ("1 {a;b} 1.", 2),
        ("d(1;2). {p(X):d(X)}.", 4),
        ("d(1..3). p(X) :- d(X), X+1=3.", 1),
        ("d(1..3). p(X) :- d(X), X<3.", 1),
    ] {
        let (automatic, auto_output, diagnostics) = run(source, &options(&[]));
        let (explicit, explicit_output, _) = run(source, &options(&["--oracle", "countermodel"]));
        let automatic = automatic.expect("formula fallback preserves supported syntax");
        let explicit = explicit.expect("explicit formula admission");
        assert_eq!(automatic.completion, Completion::Exhausted);
        assert_eq!(automatic.models, count);
        assert_eq!(automatic.models, explicit.models);
        assert_eq!(
            answers(&auto_output).into_iter().collect::<BTreeSet<_>>(),
            answers(&explicit_output).into_iter().collect()
        );
        assert!(diagnostics.contains("oracle: Ferraris reduct membership"));
        assert!(!diagnostics.contains("Oracle: reduct closure"));
        assert!(automatic.countermodel_statistics.is_some());
        assert!(matches!(
            assert_refused_without_output(source, &options(&["--oracle", "closure"])),
            RunError::Expansion(_)
        ));
    }
}

#[test]
fn empty_and_unsupported_positive_cycles_have_one_empty_stable_model() {
    for source in ["", "a :- a.", "a :- b. b :- a."] {
        for oracle in ["auto", "closure", "countermodel"] {
            let (result, output, _) = run(source, &options(&["--oracle", oracle]));
            let report = result.expect("finite cyclic program");
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, 1);
            assert_eq!(answers(&output), vec![BTreeSet::new()], "{source}");
            assert!(output.contains("SATISFIABLE\nCoverage: exhausted"));
            assert!(!output.contains("UNSATISFIABLE"));
        }
    }
}

#[test]
fn source_bytes_syntax_depth_and_body_budgets_are_not_retried_away() {
    let mut bounded = options(&[]);
    bounded.max_source_bytes = 1;
    assert!(matches!(
        assert_refused_without_output("1 {a;b} 1.", &bounded),
        RunError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            ..
        }))
    ));
    assert!(matches!(
        assert_refused_without_output("a(", &options(&[])),
        RunError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Syntax(_)))
    ));
    let deeply_nested = format!("p({}1{}).", "(".repeat(80), ")".repeat(80));
    assert!(matches!(
        assert_refused_without_output(&deeply_nested, &options(&[])),
        RunError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::SyntaxDepth,
            ..
        }))
    ));
    let too_many_literals = format!("a :- {}.", vec!["b"; 1_025].join(","));
    assert!(matches!(
        assert_refused_without_output(&too_many_literals, &options(&[])),
        RunError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::BodyElements,
            ..
        }))
    ));
}

#[test]
fn undefined_arithmetic_and_expansion_budgets_remain_hard_refusals() {
    for source in ["p(1/0).", "p(2147483647+1)."] {
        assert!(matches!(
            assert_refused_without_output(source, &options(&[])),
            RunError::Expansion(ExpansionFailure::Evaluation { .. })
        ));
    }
    for arguments in [
        vec!["--max-expansion-work", "0"],
        vec!["--max-expanded-templates", "0"],
        vec!["--max-expansion-values", "0"],
    ] {
        assert!(matches!(
            assert_refused_without_output("p(1..2).", &options(&arguments)),
            RunError::Expansion(ExpansionFailure::Limit { .. })
        ));
    }
    for (flag, resource) in [
        ("--max-substitutions", FormulaResource::Substitutions),
        ("--max-atoms", FormulaResource::Atoms),
        ("--max-ground-rules", FormulaResource::Roots),
    ] {
        assert!(matches!(
            assert_refused_without_output("1 {a;b} 1.", &options(&[flag, "0"])),
            RunError::FormulaAdmission(FormulaFailure::Limit { resource: actual, .. })
                if actual == resource
        ));
    }
}

#[test]
fn support_byte_default_matches_formula_admission() {
    assert_eq!(
        options(&[]).max_support_bytes,
        zetesis_themelios::FormulaLimits::default().max_support_bytes,
    );
}

#[test]
fn support_byte_limit_is_inclusive() {
    let source = "1 {a;b} 1.";
    let attempt = |bytes: usize| {
        let configured = options(&["--max-support-bytes", &bytes.to_string()]);
        run(source, &configured)
    };
    let (mut low, mut high) = (0_usize, options(&[]).max_support_bytes);
    assert!(attempt(high).0.is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        let (result, output, _) = attempt(middle);
        match result {
            Ok(report) => {
                assert_eq!(report.completion, Completion::Exhausted);
                high = middle;
            }
            Err(RunError::FormulaAdmission(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed,
                limit,
                location,
            })) => {
                assert!(output.is_empty());
                assert_eq!(limit, middle as u128);
                assert!(observed > limit);
                assert_eq!(
                    location.source,
                    zetesis_themelios::AdmissionOptions::default().source_id
                );
                low = middle + 1;
            }
            other => panic!("unexpected source result: {other:?}"),
        }
    }
    assert!(low > 0);
    let (result, output, _) = attempt(low);
    assert_eq!(result.unwrap().completion, Completion::Exhausted);
    assert_eq!(
        answers(&output).into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([BTreeSet::from(["a"]), BTreeSet::from(["b"])]),
    );
    let configured = options(&["--max-support-bytes", &(low - 1).to_string()]);
    assert!(matches!(
        assert_refused_without_output(source, &configured),
        RunError::FormulaAdmission(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, observed, limit, ..
        }) if observed == low as u128 && limit == (low - 1) as u128
    ));
}

#[test]
fn statistics_report_configured_support_bytes() {
    let configured = options(&["--stats", "--max-support-bytes", "65536"]);
    let (result, _, diagnostics) = run("1 {a;b} 1.", &configured);
    assert_eq!(result.unwrap().completion, Completion::Exhausted);
    assert!(diagnostics.contains("eager support bytes=65536"));
}

#[test]
fn incomplete_oracle_limits_do_not_switch_algorithms_or_claim_unsatisfiable() {
    for (arguments, countermodel) in [
        (vec!["--max-work", "0"], false),
        (vec!["--max-carrier-atoms", "0"], false),
        (
            vec!["--oracle", "countermodel", "--max-search-work", "0"],
            true,
        ),
    ] {
        let (result, output, diagnostics) = run("{a}.", &options(&arguments));
        let report = result.expect("bounded logical search report");
        assert_eq!(report.completion, Completion::Interrupted);
        if countermodel {
            assert!(matches!(
                report.interruption,
                Some(Interruption::Countermodel(_))
            ));
            assert!(!diagnostics.contains("Oracle: reduct closure"));
        } else {
            assert!(matches!(
                report.interruption,
                Some(Interruption::Oracle(Stop::WorkLimit | Stop::CarrierLimit))
            ));
            assert!(diagnostics.contains("Oracle: reduct closure"));
            assert!(report.countermodel_statistics.is_none());
        }
        assert!(output.contains("INCOMPLETE:"));
        assert!(!output.contains("UNSATISFIABLE"));
    }
}

#[test]
fn automatic_formula_selection_preserves_explicit_hardware_and_grounder_requests() {
    for backend in [
        Backend::Gpu,
        Backend::Metal,
        Backend::Vulkan,
        Backend::Dx12,
        Backend::Gl,
        Backend::Nvidia,
    ] {
        let mut configured = options(&[]);
        configured.backend = backend;
        configured.grounder = Grounder::Lazy;
        assert!(matches!(
            assert_refused_without_output("1 {a;b} 1.", &configured),
            RunError::UnsupportedOracle { backend: requested, .. } if requested == backend
        ));
    }
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        let mut configured = options(&[]);
        configured.oracle = oracle;
        configured.grounder = Grounder::Lazy;
        assert!(matches!(
            assert_refused_without_output("1 {a;b} 1.", &configured),
            RunError::UnsupportedOracle {
                grounder: Grounder::Lazy,
                ..
            }
        ));
    }
}
