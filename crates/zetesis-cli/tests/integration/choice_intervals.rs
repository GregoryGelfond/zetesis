//! Automatic routing preserves choice groups, observed ties and objective slots.

use crate::support::runs::enumerated as solve;
use zetesis_cli::{Backend, Completion, RunError};
use zetesis_themelios::{
    AdmissionFailure, ExpansionFailure, FormulaFailure, FormulaResource, ProfileFeature,
};

fn displays(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            result.push(lines.next().expect("complete displayed model"));
        }
    }
    result.sort_unstable();
    result
}

#[test]
fn automatic_interval_choices_keep_group_bounds_products_costs_and_hidden_ties() {
    type Case = (&'static str, &'static [&'static str], Option<(i32, i64)>);
    let cases: &[Case] = &[
        ("1 {p(1..2)} 1.", &["p(1)", "p(2)"], None),
        ("1 {p(2..1)} 1.", &[], None),
        (
            "1 {p(1..2,1..2)} 1.",
            &["p(1,1)", "p(1,2)", "p(2,1)", "p(2,2)"],
            None,
        ),
        (
            "1 {p(1..2)} 1. #maximize{X:p(X)}.",
            &["p(2)"],
            Some((0, -2)),
        ),
        ("0 {p(1..2)} 0. #minimize{1@7:p(1)}.", &[""], Some((7, 0))),
        (
            "1 {p(1..2)} 1. #show. #show chosen.",
            &["chosen", "chosen"],
            None,
        ),
    ];
    for &(source, expected, cost) in cases {
        let (result, output, diagnostics) = solve(source, &[]);
        let report = result.unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(report.completion, Completion::Exhausted, "{source}");
        assert_eq!(report.models, expected.len(), "{source}");
        assert_eq!(displays(&output), expected, "{source}");
        assert!(diagnostics.contains("oracle: Ferraris reduct membership"));
        assert!(output.contains("Coverage: exhausted"));
        assert!(!output.contains("INCOMPLETE"));
        if let Some(cost) = cost {
            let optimum = report.optimization.expect("present objective priority");
            assert_eq!(optimum.score.costs(), &[cost]);
            assert_eq!(optimum.tied_models, u64::try_from(expected.len()).unwrap());
            assert_eq!(
                output
                    .matches(&format!("Optimization: {}\n", cost.1))
                    .count(),
                expected.len()
            );
            assert!(output.contains("OPTIMUM FOUND"));
        } else {
            assert!(report.optimization.is_none());
            assert!(!output.contains("Optimization:"));
            let status = if expected.is_empty() {
                "UNSATISFIABLE"
            } else {
                "SATISFIABLE"
            };
            assert!(output.lines().any(|line| line == status));
        }
    }
}

#[test]
fn explicit_closure_refuses_interval_choices_without_emitting_answers() {
    let (result, output, _) = solve("1 {p(1..4)} 1.", &["--oracle", "closure"]);
    assert!(matches!(
        result,
        Err(RunError::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Profile {
                feature: ProfileFeature::BoundedChoice,
                ..
            }
        )))
    ));
    assert!(output.is_empty(), "{output}");
}

#[test]
fn explicit_lazy_device_choices_are_refused_before_discovery() {
    for (arguments, expected) in [
        (
            vec!["--backend", "metal", "--grounder", "lazy"],
            Backend::Gpu(Some(zetesis_backend::GpuApi::Metal)),
        ),
        (
            vec!["--backend", "gpu", "--grounder", "lazy"],
            Backend::Gpu(None),
        ),
    ] {
        // Hybrid formula checking is CPU-only, independently of the existing
        // relational lazy device capability.
        let (result, output, diagnostics) = solve("1 {p(1..4)} 1.", &arguments);
        let error = result.expect_err("lazy choice route must be refused");
        assert!(matches!(error, RunError::HybridBackend { backend } if backend == expected));
        assert!(output.is_empty(), "{arguments:?}: {output}");
        assert!(diagnostics.is_empty(), "{arguments:?}: {diagnostics}");
    }
}

#[test]
fn exhausted_choice_admission_never_emits_answers() {
    for (flag, resource) in [
        ("--max-atoms", FormulaResource::Atoms),
        ("--max-substitutions", FormulaResource::Substitutions),
    ] {
        let (result, output, _) = solve("1 {p(1..4)} 1.", &[flag, "1"]);
        assert!(
            matches!(
                result,
                Err(RunError::FormulaAdmission(FormulaFailure::Limit {
                    resource: actual,
                    limit: 1,
                    ..
                })) if actual == resource
            ),
            "{flag}"
        );
        assert!(output.is_empty(), "{flag}: {output}");
    }
}
