//! Retention failures and partial evaluation cannot forge an optimum or erase
//! accounting for stable models already checked by the reduct oracle.

use zetesis_cli::{Completion, Interruption, OptimizationStop, PublicationConfig, Report};
use zetesis_cpu::Cancellation;

/// One worker: the exact charges these tests compare are the scalar walk's,
/// whose first leaf is the same on every run.
fn options() -> PublicationConfig {
    crate::support::prepared::config(&[])
}

fn solve(source: &str, options: &PublicationConfig) -> (Report, String) {
    let mut output = Vec::new();
    let report = crate::support::prepared::human(
        source,
        options,
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .expect("supported objective input");
    (report, String::from_utf8(output).expect("UTF-8 output"))
}

fn assert_incomplete(report: &Report, output: &str) {
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(output.contains("INCOMPLETE:"));
    assert!(output.contains(&format!("Models: {} (search incomplete)", report.models)));
    assert!(!output.contains("OPTIMUM FOUND"));
    assert!(!output.contains("UNSATISFIABLE"));
}

#[test]
fn failed_improving_replacement_preserves_a_valid_incumbent_and_full_score_count() {
    let source = "{a}. #minimize {-1@0,k:a}.";
    let mut limited = options();
    limited.solve.max_optimal_atoms = 0;
    let (report, output) = solve(source, &limited);
    assert_incomplete(&report, &output);
    assert!(matches!(
        report.interruption,
        Some(Interruption::Incumbent(OptimizationStop::Atoms))
    ));
    assert_eq!(report.models, 1);
    assert!(output.contains("\nAnswer: 1\n\nOptimization: 0\n"));
    let retained = report.optimization.expect("previous valid empty incumbent");
    assert_eq!(retained.score.costs(), &[(0, 0)]);
    assert_eq!(
        retained.scored_models, 2,
        "failed retention follows scoring"
    );
    assert_eq!(retained.tied_models, 1);
    let (complete, _) = solve(source, &options());
    let optimum = complete.optimization.expect("complete optimum");
    assert_eq!(optimum.score.costs(), &[(0, -1)]);
    assert_eq!(
        retained.work, optimum.work,
        "both verified models were scored"
    );
}

#[test]
fn a_fully_scored_tie_counts_even_when_its_retention_exceeds_the_limit() {
    let source = "{a}. #minimize {0@0,k:a}.";
    let mut limited = options();
    limited.solve.max_optimal_models = 1;
    let (report, output) = solve(source, &limited);
    assert_incomplete(&report, &output);
    assert!(matches!(
        report.interruption,
        Some(Interruption::Incumbent(OptimizationStop::Models))
    ));
    assert_eq!(report.models, 1);
    let retained = report.optimization.expect("one retained tie");
    assert_eq!(retained.tied_models, 2);
    assert_eq!(retained.scored_models, 2);
    let (complete, _) = solve(source, &options());
    assert_eq!(retained.work, complete.optimization.expect("optimum").work);
}

#[test]
fn display_limits_discard_extra_ties_but_do_not_stop_optimality_search() {
    let mut limited = options();
    limited.solve.models = 1;
    limited.solve.max_optimal_models = 1;
    let (report, output) = solve("{a}. {b}. #show. #minimize {0@0,k:a}.", &limited);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    let optimum = report.optimization.expect("verified optimum");
    assert_eq!(optimum.scored_models, 4);
    assert_eq!(optimum.tied_models, 4);
    assert_eq!(output.matches("Answer:").count(), 1);
    assert!(output.contains("OPTIMUM FOUND"));
}

#[test]
fn partial_prepared_score_preserves_the_incumbent() {
    let source = "{a}. #minimize {0@0,k:a}.";
    let mut first_only = options();
    first_only.solve.max_candidates = 1;
    let (prefix, _) = solve(source, &first_only);
    let first_work = prefix.optimization.expect("first verified incumbent").work;
    let mut limited = options();
    limited.solve.max_objective_work = first_work + 1;
    let (report, output) = solve(source, &limited);
    assert_incomplete(&report, &output);
    let Some(Interruption::PreparedObjective(error)) = &report.interruption else {
        panic!(
            "expected partial objective evaluation: {:?}",
            report.interruption
        );
    };
    assert_eq!(error.work(), 1);
    assert!(matches!(error.kind(),
        zetesis_themelios::objective_bound::ObjectiveScoreErrorKind::Eligibility(error)
        if error.kind() == zetesis_themelios::objective_bound::ObjectiveBoundErrorKind::Limit(
            zetesis_themelios::objective_bound::ObjectiveBoundResource::Work)
    ));
    let retained = report.optimization.expect("first score remains valid");
    assert_eq!(retained.scored_models, 1);
    assert_eq!(retained.tied_models, 1);
    assert_eq!(retained.work, first_work + error.work());
}

#[test]
fn replacement_releases_retained_atom_budget_and_filters_never_reduce_its_cost() {
    let mut bounded = options();
    bounded.solve.max_optimal_atoms = 1;
    let (report, output) = solve("1 {a;b} 1. #minimize {5@0,k:a;2@0,k:b}.", &bounded);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    assert_eq!(
        report.optimization.expect("best board").score.costs(),
        &[(0, 2)]
    );
    assert!(output.contains("OPTIMUM FOUND"));

    let (hidden, output) = solve("a. b. #show. #minimize {0@0,k:a}.", &bounded);
    assert_incomplete(&hidden, &output);
    assert!(matches!(
        hidden.interruption,
        Some(Interruption::Incumbent(OptimizationStop::Atoms))
    ));
    assert_eq!(
        hidden.models, 0,
        "display hiding does not shrink retained models"
    );
}
