//! Optional bound refusal after planning must preserve every exact optimal tie.

use clap::Parser;
use zetesis_cli::{Completion, Options, Report, run_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_themelios::objective_bound::{ObjectivePlan, ObjectivePlanLimits};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const SOURCE: &str = "1 {a;b} 1. {hidden}. #minimize { 5,a:a; 2,b:b }.";
const STOPPED: &str = "Objective pruning stopped: objective candidate bound Limit(Work) \
(template None); exact search continues";

fn exact_plan_work(source: &str) -> u64 {
    let admitted = admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("the complete fixture is admitted");
    let build = |max_work| {
        ObjectivePlan::new(
            admitted.theory(),
            admitted.atoms(),
            admitted.objectives(),
            ObjectivePlanLimits {
                max_work,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .expect("planning succeeds at the inclusive work ceiling")
    };
    let work = build(ObjectivePlanLimits::default().max_work)
        .statistics()
        .work;
    assert!(work > 0, "the CLI's zero budget disables planning");
    assert_eq!(build(work).statistics().work, work);
    work
}

fn solve(source: &str, bound_work: u64) -> (Report, String, String) {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--models",
        "0",
        "--max-objective-bound-work",
        &bound_work.to_string(),
    ])
    .expect("valid CLI arguments");
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.to_owned(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .expect("optional pruning failure leaves exact search available");
    (
        report,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

fn records(text: &str) -> Vec<(&str, &str)> {
    let mut records = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            let atoms = lines.next().expect("complete model display");
            let cost = lines.next().expect("complete cost vector");
            assert_eq!(cost, "Optimization: 2");
            records.push((atoms, cost));
        }
    }
    records.sort_unstable();
    records
}

fn complete_without_restrictions(report: &Report, text: &str) {
    assert_eq!(report.completion, Completion::Exhausted);
    assert!(report.interruption.is_none());
    assert_eq!(report.models, 2);
    let optimum = report.optimization.as_ref().expect("verified optimum");
    assert_eq!(optimum.score.costs(), &[(0, 2)]);
    assert_eq!(optimum.tied_models, 2);
    assert_eq!(optimum.scored_models, 4);
    assert_eq!(
        report
            .countermodel_statistics
            .expect("original reduct search statistics")
            .candidate_restrictions,
        0
    );
    assert!(text.contains("OPTIMUM FOUND\nModels: 2\n"));
    assert!(!text.contains("INCOMPLETE"));
    assert!(!text.contains("UNSATISFIABLE"));
}

#[test]
fn first_bound_work_refusal_preserves_full_and_hidden_optimal_ties() {
    for project_hidden in [false, true] {
        let source = if project_hidden {
            format!("{SOURCE} #show a/0. #show b/0.")
        } else {
            SOURCE.to_owned()
        };
        let work = exact_plan_work(&source);
        let (actual, output, diagnostics) = solve(&source, work);
        let (baseline, expected, disabled_diagnostics) = solve(&source, 0);
        complete_without_restrictions(&actual, &output);
        complete_without_restrictions(&baseline, &expected);
        assert_eq!(records(&output), records(&expected));
        let second = if project_hidden { "b" } else { "b hidden" };
        assert_eq!(
            records(&output),
            vec![("b", "Optimization: 2"), (second, "Optimization: 2")]
        );
        assert_eq!(
            diagnostics
                .lines()
                .filter(|line| line.starts_with("Objective pruning"))
                .collect::<Vec<_>>(),
            vec![STOPPED],
            "planning must succeed, the first bound must fail once, and no restriction is installed"
        );
        assert!(!disabled_diagnostics.contains("Objective pruning"));
    }
}
