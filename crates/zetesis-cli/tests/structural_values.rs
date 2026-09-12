//! Structural values cross closure/reduct, scoring, retention and output boundaries.
use clap::Parser;
use zetesis_cli::{
    Completion, Interruption, OptimizationStop, Options, RunError, run_with_diagnostics,
};
use zetesis_cpu::Control;

fn solve(source: &str, extra: &[&str]) -> (Result<zetesis_cli::Report, RunError>, String) {
    let options = Options::try_parse_from(
        ["zetesis", "--backend", "cpu", "--models", "0"]
            .into_iter()
            .chain(extra.iter().copied()),
    )
    .unwrap();
    let mut output = Vec::new();
    let result = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut Vec::new(),
        &Control::default(),
    );
    (result, String::from_utf8(output).unwrap())
}
#[test]
fn closed_values_are_preserved_across_membership_routes() {
    for args in [
        vec!["--oracle", "closure", "--grounder", "lazy"],
        vec!["--oracle", "closure", "--grounder", "eager"],
        vec!["--oracle", "countermodel"],
    ] {
        let (report, output) = solve("p(f(1,g(2))).q(X):-p(X).", &args);
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, 1);
        assert!(output.contains("p(f(1,g(2))) q(f(1,g(2)))\n"), "{output}");
    }
}
#[test]
fn observation_channels_keep_hidden_full_ties_and_nested_whole_variables() {
    let source =
        "p(f(1)).{hidden}.#show p/1.#show p(X):p(X).#show seen(X):p(X).#minimize{0@1,X:p(X)}.";
    for limit in ["0", "10000000"] {
        let (report, output) = solve(
            source,
            &[
                "--oracle",
                "countermodel",
                "--max-objective-bound-work",
                limit,
            ],
        );
        let report = report.unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
        assert_eq!(report.models, 2);
        assert_eq!(report.optimization.unwrap().tied_models, 2);
        assert_eq!(
            output.matches("p(f(1)) p(f(1)) seen(f(1))\n").count(),
            2,
            "{output}"
        );
    }
}
#[test]
fn retained_structural_payload_is_admitted_before_publication() {
    // Catalog header8 + signed atom header17 + name1 + function header18/name1
    // + number5. Selection storage adds its length8 and one original index8.
    const CATALOG_BYTES: usize = 8 + 17 + 1 + 18 + 1 + 5;
    const SELECTION_BYTES: usize = 8 + 8;
    const RETAINED: usize = CATALOG_BYTES + SELECTION_BYTES;
    for ceiling in [RETAINED - 1, RETAINED] {
        let (report, output) = solve(
            "p(f(1)).#minimize{0:p(f(1))}.",
            &[
                "--oracle",
                "countermodel",
                "--max-optimal-bytes",
                &ceiling.to_string(),
            ],
        );
        let report = report.unwrap();
        if ceiling == RETAINED {
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, 1);
            assert!(output.contains("OPTIMUM FOUND"));
        } else {
            assert_eq!(report.completion, Completion::Interrupted);
            assert_eq!(report.models, 0);
            assert!(matches!(
                report.interruption,
                Some(Interruption::Incumbent(OptimizationStop::Bytes))
            ));
            assert!(!output.contains("Answer:"));
            assert!(!output.contains("OPTIMUM FOUND"));
        }
    }
}

#[test]
fn rendered_structural_output_is_admitted_before_publication() {
    for ceiling in [17, 18] {
        let (result, output) = solve(
            "p(f(1)).#show p/1.#show x:q.",
            &[
                "--oracle",
                "countermodel",
                "--max-observation-bytes",
                &ceiling.to_string(),
            ],
        );
        if ceiling == 18 {
            assert_eq!(result.unwrap().models, 1);
            assert!(output.contains("p(f(1))"));
        } else {
            assert!(matches!(
                result,
                Err(RunError::ObservationOutputLimit { .. })
            ));
            assert!(output.is_empty());
        }
    }
}
