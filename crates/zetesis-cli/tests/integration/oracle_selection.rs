//! Automatic language admission preserves explicit execution requests, typed
//! resource refusals, and stable-model semantics across the two reduct oracles.

use std::collections::BTreeSet;
use zetesis_reference_support::resource_limits;
use zetesis_test_support::thresholds::first_success;

use clap::Parser;
use zetesis_cli::{
    Completion, Interruption, Options, Oracle, Report, RunError, run_with_diagnostics,
};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_themelios::{
    AdmissionFailure, ExpansionFailure, FormulaFailure, FormulaResource, InputLimit,
};

fn options(arguments: &[&str]) -> Options {
    let mut options = Options::try_parse_from(
        ["zetesis", "--backend", "cpu", "--models", "0"]
            .into_iter()
            .chain(arguments.iter().copied()),
    )
    .expect("valid CLI arguments");
    options.stats = true;
    options
}

fn run(source: &str, options: &Options) -> (Result<Report, RunError>, String, String) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = run_with_diagnostics(
        source.to_owned(),
        options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
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
    assert!(
        crate::support::human::preamble(&output),
        "no partial semantic claim: {output}"
    );
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
fn auto_uses_formula_membership_for_choices() {
    for (source, count) in [("1 {a;b} 1.", 2), ("d(1;2). {p(X):d(X)}.", 4)] {
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
fn auto_evaluates_acyclic_arithmetic_sources_directly() {
    for source in [
        "d(1..3). p(X) :- d(X), X+1=3.",
        "d(1..3). p(X) :- d(X), X<3.",
    ] {
        let (automatic, auto_output, diagnostics) = run(source, &options(&[]));
        let (explicit, explicit_output, _) = run(source, &options(&["--oracle", "countermodel"]));
        let automatic = automatic.expect("certified source evaluation");
        let explicit = explicit.expect("explicit formula admission");
        assert_eq!(automatic.completion, Completion::Exhausted);
        assert_eq!(automatic.models, 1);
        assert_eq!(automatic.models, explicit.models);
        assert_eq!(
            answers(&auto_output).into_iter().collect::<BTreeSet<_>>(),
            answers(&explicit_output).into_iter().collect()
        );
        assert!(diagnostics.contains("dependency-ordered source evaluation"));
        assert!(automatic.source_evaluation.unwrap().work > 0);
        assert!(automatic.countermodel_statistics.is_none());
        assert!(explicit.source_evaluation.is_none());
        assert!(explicit.countermodel_statistics.is_some());
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
            assert!(output.contains("SATISFIABLE\nModels:"));
            assert!(!output.contains("UNSATISFIABLE"));
        }
    }
}

#[test]
fn source_limits_preserve_typed_admission_refusals() {
    use zetesis_themelios::{AdmissionOptions, ExpansionLimits, admit_extended};
    let bounded = AdmissionOptions {
        max_source_bytes: 1,
        ..AdmissionOptions::default()
    };
    assert!(matches!(
        admit_extended("1 {a;b} 1.".into(), bounded, ExpansionLimits::default()),
        Err(ExpansionFailure::Admission(AdmissionFailure::Limit {
            resource: InputLimit::SourceBytes,
            ..
        }))
    ));
    assert!(matches!(
        assert_refused_without_output("a(", &options(&[])),
        RunError::Expansion(ExpansionFailure::Admission(AdmissionFailure::Syntax(_)))
    ));
    for (source, resource) in [
        (
            format!("p({}1{}).", "(".repeat(80), ")".repeat(80)),
            InputLimit::SyntaxDepth,
        ),
        (
            format!("a :- {}.", vec!["b"; 1_025].join(",")),
            InputLimit::BodyElements,
        ),
    ] {
        assert!(
            matches!(admit_extended(source, AdmissionOptions::default(), ExpansionLimits::default()), Err(ExpansionFailure::Admission(AdmissionFailure::Limit {resource: actual, ..})) if actual == resource)
        );
    }
}

#[test]
fn undefined_arithmetic_and_expansion_limits_remain_refusals() {
    use zetesis_themelios::{AdmissionOptions, ExpansionLimits, ExpansionResource, admit_extended};
    for source in ["p(1/0).", "p(2147483647+1)."] {
        assert!(matches!(
            assert_refused_without_output(source, &options(&[])),
            RunError::Expansion(ExpansionFailure::Evaluation { .. })
        ));
    }
    for resource in [
        ExpansionResource::TermWork,
        ExpansionResource::Templates,
        ExpansionResource::Values,
    ] {
        let limits = resource_limits::expansion(resource, 0);
        assert!(matches!(
            admit_extended("p(1..2).".into(), AdmissionOptions::default(), limits),
            Err(ExpansionFailure::Limit { .. })
        ));
    }
    for resource in [
        FormulaResource::Substitutions,
        FormulaResource::Atoms,
        FormulaResource::Roots,
    ] {
        let limits = resource_limits::formula(resource, 0);
        let (result, output) = crate::support::prepared::bounded_admission(
            "1 {a;b} 1.",
            ExpansionLimits::default(),
            &limits,
        );
        assert!(
            matches!(result, Err(RunError::FormulaAdmission(FormulaFailure::Limit {resource:actual,..})) if actual==resource)
        );
        assert!(output.is_empty());
    }
}

#[test]
fn formula_work_admission_is_inclusive() {
    let source = "a|b. c:-a. c:-b.";
    let attempt = |work: u64| {
        crate::support::prepared::bounded_admission(
            source,
            zetesis_themelios::ExpansionLimits::default(),
            &resource_limits::formula(FormulaResource::Work, work),
        )
    };
    let low = first_success(
        resource_limits::formula_default(FormulaResource::Work),
        |middle| match attempt(middle).0 {
            Ok(report) => {
                assert_eq!(report.completion, Completion::Exhausted);
                true
            }
            Err(RunError::FormulaAdmission(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                observed,
                limit,
                ..
            })) => {
                assert_eq!(limit, u128::from(middle));
                assert!(observed > limit);
                false
            }
            other => panic!("unexpected source result: {other:?}"),
        },
    );
    let (result, output) = attempt(low);
    assert_eq!(result.unwrap().completion, Completion::Exhausted);
    assert_eq!(
        answers(&output).into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([BTreeSet::from(["a", "c"]), BTreeSet::from(["b", "c"])])
    );
    let (result, output) = attempt(low - 1);
    assert!(output.is_empty());
    assert!(
        matches!(result,Err(RunError::FormulaAdmission(FormulaFailure::Limit{resource:FormulaResource::Work,observed,limit,location})) if observed==u128::from(low) && limit==u128::from(low-1) && location.location().unwrap().source==zetesis_themelios::AdmissionOptions::default().source_id)
    );
}

#[test]
fn support_byte_limit_is_inclusive() {
    let source = "1 {a;b} 1.";
    let attempt = |bytes| {
        crate::support::prepared::bounded_admission(
            source,
            zetesis_themelios::ExpansionLimits::default(),
            &resource_limits::formula(FormulaResource::SupportBytes, bytes),
        )
    };
    let low = first_success(
        resource_limits::formula_default(FormulaResource::SupportBytes),
        |middle| match attempt(middle).0 {
            Ok(report) => {
                assert_eq!(report.completion, Completion::Exhausted);
                true
            }
            Err(RunError::FormulaAdmission(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed,
                limit,
                location,
            })) => {
                assert_eq!(limit, u128::from(middle));
                assert!(observed > limit);
                assert_eq!(
                    location.location().unwrap().source,
                    zetesis_themelios::AdmissionOptions::default().source_id
                );
                false
            }
            other => panic!("unexpected source result: {other:?}"),
        },
    );
    let (result, output) = attempt(low);
    assert_eq!(result.unwrap().completion, Completion::Exhausted);
    assert_eq!(
        answers(&output).into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([BTreeSet::from(["a"]), BTreeSet::from(["b"])])
    );
    let (result, output) = attempt(low - 1);
    assert!(output.is_empty());
    assert!(
        matches!(result,Err(RunError::FormulaAdmission(FormulaFailure::Limit{resource:FormulaResource::SupportBytes,observed,limit,..})) if observed==u128::from(low) && limit==u128::from(low-1))
    );
}

#[test]
fn statistics_report_memory_derived_support_bytes() {
    let configured = options(&["--stats", "--memory", "1048576"]);
    let (result, _, diagnostics) = run("1 {a;b} 1.", &configured);
    assert_eq!(result.unwrap().completion, Completion::Exhausted);
    assert!(
        diagnostics.contains(&format!(
            "support bytes={}",
            configured.resources().formula_limits().max_support_bytes
        )),
        "{diagnostics}"
    );
}

#[test]
fn incomplete_oracles_do_not_claim_unsatisfiable() {
    for (countermodel, carrier) in [(false, false), (false, true), (true, false)] {
        let mut config = crate::support::prepared::config(&[
            "--oracle",
            if countermodel {
                "countermodel"
            } else {
                "closure"
            },
        ]);
        if countermodel {
            config.solve.max_search_work = 0;
        } else if carrier {
            config.solve.max_carrier_atoms = 0;
        } else {
            config.solve.max_work = 0;
        }
        let mut output = Vec::new();
        let mut renderer = zetesis_cli::HumanRenderer::new(
            &mut output,
            zetesis_cli::ColorMode::Never,
            config.observations.max_output_bytes,
        );
        let report = if countermodel {
            crate::support::prepared::formula(
                "{a}.",
                &config,
                &mut renderer,
                &mut std::io::sink(),
                &Cancellation::default(),
            )
        } else {
            crate::support::prepared::relational(
                "{a}.",
                &config,
                &mut renderer,
                &mut std::io::sink(),
                &Cancellation::default(),
            )
        }
        .unwrap();
        assert_eq!(report.completion, Completion::Interrupted);
        if countermodel {
            assert!(matches!(
                report.interruption,
                Some(Interruption::Countermodel(_))
            ));
        } else {
            assert!(matches!(
                report.interruption,
                Some(Interruption::Oracle(Stop::WorkLimit | Stop::CarrierLimit))
            ));
            assert!(report.countermodel_statistics.is_none());
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("INCOMPLETE:"));
        assert!(!output.contains("UNSATISFIABLE"));
    }
}
