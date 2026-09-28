//! Objective ranking is subordinate to complete stable-model verification.

use clap::Parser;
use zetesis_cli::{Completion, Options, Report, run_with_diagnostics};
use zetesis_cpu::Cancellation;

fn solve(source: &str, arguments: &[&str]) -> (Report, String) {
    let options =
        Options::try_parse_from(["zetesis"].into_iter().chain(arguments.iter().copied())).unwrap();
    let mut output = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut Vec::new(),
        &Cancellation::default(),
    )
    .unwrap();
    (report, String::from_utf8(output).unwrap())
}

#[test]
fn default_request_proves_optimum_before_returning_one_model() {
    let (report, text) = solve("1 {a;b} 1. #minimize { 5,a:a; 2,b:b }.", &[]);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 1);
    let optimum = report.optimization.unwrap();
    assert_eq!(optimum.score.costs(), &[(0, 2)]);
    assert_eq!(optimum.tied_models, 1);
    assert!(
        text.starts_with("Answer: 1\nb\nOptimization: 2\n"),
        "{text}"
    );
    assert!(text.contains("OPTIMUM FOUND\nCoverage: exhausted"));
    // One worker reaches b first and the bound then prunes a; several
    // workers may decide both leaves before the bound reaches them.
    let (pruned, _) = solve(
        "1 {a;b} 1. #minimize { 5,a:a; 2,b:b }.",
        &["--workers", "1"],
    );
    let pruned_optimum = pruned.optimization.unwrap();
    assert_eq!(pruned_optimum.score.costs(), &[(0, 2)]);
    assert_eq!(pruned_optimum.scored_models, 1);
    assert_eq!(
        pruned
            .countermodel_statistics
            .unwrap()
            .candidate_restrictions,
        1
    );
    let (unpruned, _) = solve(
        "1 {a;b} 1. #minimize { 5,a:a; 2,b:b }.",
        &["--max-objective-bound-work", "0"],
    );
    assert_eq!(unpruned.optimization.unwrap().scored_models, 2);
}

#[test]
fn all_optimal_hidden_models_retain_multiplicity_and_fixed_priorities() {
    let source = "{hidden}. {a}. #show. #minimize { -1@2,a:a; 0@7,z:hidden }.";
    let (all, text) = solve(source, &["--models", "0"]);
    assert_eq!(all.completion, Completion::Exhausted);
    assert_eq!(all.models, 2);
    let best = all.optimization.unwrap();
    assert_eq!(best.score.costs(), &[(7, 0), (2, -1)]);
    assert_eq!(best.tied_models, 2);
    assert_eq!(best.scored_models, 4);
    assert_eq!(text.matches("Optimization: 0 -1\n").count(), 2);
    let (one, _) = solve(source, &[]);
    assert_eq!(one.models, 1);
    assert_eq!(one.optimization.unwrap().tied_models, 2);
}

#[test]
fn objective_keys_deduplicate_globally_and_constraints_never_supply_support() {
    let (report, _) = solve(
        "a. #minimize { 3,k:a; 3,k:a; 4,k:a; 3,j:a }. #minimize { 3,k:a }.",
        &[],
    );
    assert_eq!(report.optimization.unwrap().score.costs(), &[(0, 10)]);
    let (report, text) = solve("1 {a:a} 1. #minimize { -10,k:a }.", &[]);
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 0);
    assert!(report.optimization.is_none());
    assert!(text.contains("UNSATISFIABLE"));
    assert!(!text.contains("OPTIMUM FOUND"));
}

#[test]
fn absent_objectives_and_present_zero_cost_remain_distinct() {
    let (absent, text) = solve("a :- a. #minimize { 3,k:a }.", &["--models", "0"]);
    assert!(absent.optimization.is_none());
    assert!(!text.contains("Optimization:"));
    assert!(text.contains("SATISFIABLE\nCoverage: exhausted"));
    let (present, text) = solve("a. #minimize { 0,k:a }.", &[]);
    assert_eq!(present.optimization.unwrap().score.costs(), &[(0, 0)]);
    assert!(text.contains("Optimization: 0\n"));
}

#[test]
fn every_objective_and_incumbent_bound_is_incomplete_never_optimal() {
    let source = "{a}. #minimize { 0,k:a }.";
    for arguments in [
        vec!["--max-objective-work", "0"],
        vec!["--max-objective-bindings", "0"],
        vec!["--max-objective-keys", "0"],
        vec!["--max-objective-key-bytes", "0"],
        vec!["--max-optimal-models", "0"],
        vec!["--max-optimal-atoms", "0"],
        vec!["--max-optimal-bytes", "0"],
        vec!["--max-candidates", "1"],
    ] {
        let mut args = vec!["--models", "0"];
        args.extend(arguments);
        let (report, text) = solve(source, &args);
        assert_eq!(
            report.completion,
            Completion::Interrupted,
            "{args:?}: {text}"
        );
        assert!(text.contains("INCOMPLETE:"));
        assert!(!text.contains("OPTIMUM FOUND"));
        assert!(!text.contains("UNSATISFIABLE"));
    }
}

#[test]
fn incumbent_bounds_preserve_all_optimal_models_and_lexicographic_costs() {
    for source in [
        "{a;b;c}. #minimize { 2@9,k:a; -3@1,j:b; 1@1,c:c }.",
        "{a;b;hidden}. #minimize { -2@1,k:a; -2@1,k:b }. #show.",
        "{a;b;c}. #minimize { 2,k:a; 2,k:b; -1,j:c }. #minimize { 2,k:c }.",
        "d(1..3). {a(X):d(X)}. #minimize { X,k:a(X); 0@7,z:d(1) }.",
        "{a;b}. r :- a. r :- b. #minimize { 1,k:r }.",
        "{a;b}. #minimize { 0,k:a; 0,j:b }.",
    ] {
        let (pruned, output) = solve(source, &["--models", "0"]);
        let (unpruned, baseline) = solve(
            source,
            &["--models", "0", "--max-objective-bound-work", "0"],
        );
        assert_eq!(
            pruned.completion,
            Completion::Exhausted,
            "{source}: {output}"
        );
        assert_eq!(unpruned.completion, Completion::Exhausted);
        assert_eq!(
            answer_records(&output),
            answer_records(&baseline),
            "{source}"
        );
        let best = pruned.optimization.unwrap();
        let expected = unpruned.optimization.unwrap();
        assert_eq!(best.score, expected.score, "{source}");
        assert_eq!(best.tied_models, expected.tied_models, "{source}");
        assert!(best.scored_models <= expected.scored_models, "{source}");
        assert!(
            pruned
                .countermodel_statistics
                .unwrap()
                .candidate_restrictions
                > 0
        );
    }
}

#[test]
fn refused_optional_bound_preserves_exact_unpruned_completion() {
    let source = "1 {a;b} 1. #minimize { 5,a:a; 2,b:b }.";
    let (report, output) = solve(
        source,
        &["--models", "0", "--max-objective-bound-work", "1"],
    );
    let (baseline, expected) = solve(
        source,
        &["--models", "0", "--max-objective-bound-work", "0"],
    );
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(answer_records(&output), answer_records(&expected));
    assert_eq!(
        report.optimization.unwrap().score,
        baseline.optimization.unwrap().score
    );
    assert_eq!(
        report
            .countermodel_statistics
            .unwrap()
            .candidate_restrictions,
        0
    );
}

fn answer_records(text: &str) -> Vec<(&str, &str)> {
    let mut records = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            let atoms = lines.next().unwrap();
            let cost = lines.next().unwrap();
            assert!(cost.starts_with("Optimization:"));
            records.push((atoms, cost));
        }
    }
    records.sort_unstable();
    records
}
