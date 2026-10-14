//! Ordinary solving presents the same typed attribution to Rust and JSON clients.

use clap::Parser;
use serde_json::Value;
use zetesis_cli::{GroundingOutcome, GroundingPhase, Options, run_detailed_with_diagnostics};
use zetesis_cpu::Cancellation;

fn options(extra: &[&str]) -> Options {
    Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
            "--json",
            "--stats",
        ]
        .into_iter()
        .chain(extra.iter().copied()),
    )
    .unwrap()
}

#[test]
fn json_attribution_preserves_typed_measurements() {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "1{p;q}1.".into(),
        &options(&[]),
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    let typed = report.phase_timings.unwrap().grounding;
    let document: Value = serde_json::from_slice(&output).unwrap();
    let view = &document["statistics"]["grounding_attribution"];
    assert_eq!(view["scope"], "eager_formula");
    for phase in GroundingPhase::ALL {
        let json = &view["measurements"][phase.label()];
        let measurement = typed.get(phase).unwrap();
        assert_eq!(json["work"].as_object().unwrap().len(), 52);
        assert_eq!(
            json["elapsed_ns"].as_u64().map(u128::from),
            measurement.elapsed.map(|value| value.as_nanos())
        );
        for outcome in GroundingOutcome::ALL {
            assert_eq!(
                json["outcomes"][outcome.label()].as_u64(),
                measurement.count(outcome)
            );
        }
        assert_work_fields(json, &measurement.work);
    }
    assert!(
        std::str::from_utf8(&diagnostics)
            .unwrap()
            .contains("Grounding attribution:")
    );
}

fn assert_work_fields(json: &Value, work: &zetesis_themelios::GroundingWork) {
    for (name, count) in [
        ("support_construction_work", work.support_construction_work),
        ("support_production_work", work.support_production_work),
        ("support_join_work", work.support_join_work),
        ("support_head_work", work.support_head_work),
        ("support_order_work", work.support_order_work),
        ("support_wake_work", work.support_wake_work),
        ("support_publication_work", work.support_publication_work),
        ("support_rounds", work.support_rounds),
        ("support_producer_visits", work.support_producer_visits),
        ("support_rich_traversals", work.support_rich_traversals),
        ("support_unchanged_visits", work.support_unchanged_visits),
        (
            "support_snapshot_preparations",
            work.support_snapshot_preparations,
        ),
        ("local_plan_requests", work.local_plan_requests),
        ("local_plan_preparations", work.local_plan_preparations),
        ("local_plan_reuses", work.local_plan_reuses),
        ("support_atoms", work.support_atoms),
        ("support_index_entries", work.support_index_entries),
        ("join_probes", work.join_probes),
        ("join_rows", work.join_rows),
        ("indexed_probes", work.indexed_probes),
        ("table_inapplicable_probes", work.table_inapplicable_probes),
        ("table_preparations", work.table_preparations),
        ("table_reuses", work.table_reuses),
        ("table_probes", work.table_probes),
        ("table_rows", work.table_rows),
        ("table_prepare_work", work.table_prepare_work),
        ("table_query_work", work.table_query_work),
        ("table_index_bytes", work.table_index_bytes),
        ("support_peak_bytes", work.support_peak_bytes),
        ("domain_prepare_work", work.domain_prepare_work),
        ("domain_guard_rows", work.domain_guard_rows),
        ("domain_guard_checks", work.domain_guard_checks),
        ("domain_rejected_rows", work.domain_rejected_rows),
        ("domain_excluded_values", work.domain_excluded_values),
        ("binding_snapshots", work.binding_snapshots),
        ("expression_evaluations", work.expression_evaluations),
        ("expression_nodes", work.expression_nodes),
        ("integer_comparisons", work.integer_comparisons),
        (
            "integer_comparison_declines",
            work.integer_comparison_declines,
        ),
        ("numeric_index_builds", work.numeric_index_builds),
        ("numeric_index_declines", work.numeric_index_declines),
        ("range_probes", work.range_probes),
        ("range_probe_rows", work.range_probe_rows),
        ("expression_key_probes", work.expression_key_probes),
        ("expression_key_rows", work.expression_key_rows),
        ("row_atoms", work.row_atoms),
        ("row_atom_reuses", work.row_atom_reuses),
        ("atom_lookups", work.atom_lookups),
        ("atoms_inserted", work.atoms_inserted),
        ("node_lookups", work.node_lookups),
        ("nodes_inserted", work.nodes_inserted),
        ("roots", work.roots),
    ] {
        assert_eq!(json["work"][name].as_u64(), count);
    }
}

#[test]
fn the_ordinary_command_requests_the_domain_analysis() {
    // Numeric analysis covers the choice producer. The separate finite-guard
    // profile remains inapplicable, so no guard visits or excludes a row.
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "1{p;q}1.".into(),
        &options(&[]),
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    let typed = report.phase_timings.unwrap().grounding;
    let analysis = typed.get(GroundingPhase::DomainAnalysis).unwrap();
    assert_eq!(analysis.count(GroundingOutcome::Completed), Some(1));
    assert!(analysis.work.domain_prepare_work.unwrap() > 0);
    let rules = typed.get(GroundingPhase::RuleInstantiation).unwrap();
    assert_eq!(rules.work.domain_guard_rows, Some(0));
    assert_eq!(rules.work.domain_excluded_values, Some(0));
}

#[test]
fn eager_grounding_narrows_candidates_by_comparison() {
    // Static analysis excludes 198 candidates. The numeric range probe selects
    // the two remaining rows before the domain guard, which rejects neither.
    // The one answer contains exactly those two p instances.
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "d(1..200). p(X) :- d(X), X < 3.".into(),
        &options(&["--grounder", "eager"]),
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    let typed = report.phase_timings.unwrap().grounding;
    let analysis = typed.get(GroundingPhase::DomainAnalysis).unwrap();
    assert_eq!(analysis.work.domain_excluded_values, Some(198));
    let rules = typed.get(GroundingPhase::RuleInstantiation).unwrap();
    assert_eq!(rules.work.domain_rejected_rows, Some(0));
    assert_eq!(rules.work.domain_guard_rows, Some(2));
    assert_eq!(rules.work.range_probes, Some(1));
    assert_eq!(rules.work.range_probe_rows, Some(2));
    assert_eq!(rules.work.binding_snapshots, Some(0));
    // The driver also includes the 200 original fact roots in this phase.
    assert_eq!(rules.work.roots, Some(202));
    let document: Value = serde_json::from_slice(&output).unwrap();
    let measurements = &document["statistics"]["grounding_attribution"]["measurements"];
    assert_work_fields(
        &measurements[GroundingPhase::DomainAnalysis.label()],
        &analysis.work,
    );
    assert_work_fields(
        &measurements[GroundingPhase::RuleInstantiation.label()],
        &rules.work,
    );
    let models = document["models"].as_array().unwrap();
    assert_eq!(models.len(), 1);
    // The one record spells every atom of the document and refers to them.
    let atoms = models[0]["model"]["atoms"].as_array().unwrap();
    let instances: Vec<_> = models[0]["model"]["full_model"]
        .as_array()
        .unwrap()
        .iter()
        .map(|index| &atoms[usize::try_from(index.as_u64().unwrap()).unwrap()])
        .filter(|atom| atom["predicate"] == "p")
        .map(|atom| atom["arguments"][0][0]["value"].as_i64().unwrap())
        .collect();
    assert_eq!(instances, [1, 2]);
}

#[test]
fn eager_formula_solving_uses_requested_table_joins() {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "edge(1,1). edge(1,2). edge(2,1). edge(2,2). 1{choose(1);choose(2)}1. witness(X,Y):-choose(X),edge(X,Y). diagonal(X):-edge(X,X).".into(),
        &options(&["--grounder", "eager", "--formula-joins", "table"]),
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    ).unwrap();
    let grounding = report.phase_timings.unwrap().grounding;
    let work = GroundingPhase::ALL
        .into_iter()
        .filter_map(|phase| grounding.get(phase))
        .fold(
            zetesis_cli::GroundingWork::default(),
            |work, measurement| work.checked_sum(measurement.work),
        );
    assert!(work.table_preparations.unwrap() > 0);
    assert!(work.table_probes.unwrap() > 0);
    assert!(work.table_rows.unwrap() > 0);
    let document: Value = serde_json::from_slice(&output).unwrap();
    // The native decoder resolves shared dictionary entries across records.
    let mut family = zetesis_validation::answers::native_json::parse(
        &output,
        zetesis_validation::answers::native_json::Limits::default(),
    )
    .unwrap()
    .full_model_symbols(8 * 1024 * 1024)
    .unwrap();
    for model in &mut family {
        model.sort();
    }
    family.sort();
    let mut expected: Vec<_> = [1, 2]
        .into_iter()
        .map(|chosen| {
            let mut atoms = vec![
                "edge(1,1)".to_owned(),
                "edge(1,2)".to_owned(),
                "edge(2,1)".to_owned(),
                "edge(2,2)".to_owned(),
                format!("choose({chosen})"),
                format!("witness({chosen},1)"),
                format!("witness({chosen},2)"),
                "diagonal(1)".to_owned(),
                "diagonal(2)".to_owned(),
            ];
            atoms.sort();
            atoms
        })
        .collect();
    expected.sort();
    assert_eq!(family, expected);
    assert_eq!(document["outcome"]["completion"], "exhausted");
    for phase in GroundingPhase::ALL {
        let Some(measurement) = grounding.get(phase) else {
            assert_eq!(phase, GroundingPhase::DomainAnalysis);
            continue;
        };
        let view =
            &document["statistics"]["grounding_attribution"]["measurements"][phase.label()]["work"];
        assert_eq!(
            view["table_preparations"].as_u64(),
            measurement.work.table_preparations
        );
        assert_eq!(view["table_probes"].as_u64(), measurement.work.table_probes);
        assert_eq!(
            view["support_peak_bytes"].as_u64(),
            measurement.work.support_peak_bytes
        );
    }
}

#[test]
fn relational_grounding_stays_unmeasured_in_formula_attribution() {
    for grounder in ["eager", "lazy"] {
        let mut output = Vec::new();
        let report = run_detailed_with_diagnostics(
            "p.".into(),
            &options(&["--grounder", grounder]),
            &mut output,
            &mut std::io::sink(),
            &Cancellation::default(),
        )
        .unwrap();
        let typed = report.phase_timings.unwrap().grounding;
        let document: Value = serde_json::from_slice(&output).unwrap();
        for phase in GroundingPhase::ALL {
            assert!(typed.get(phase).is_none());
            assert!(
                document["statistics"]["grounding_attribution"]["measurements"][phase.label()]
                    .is_null()
            );
        }
    }
}

#[test]
fn records_report_how_comparisons_were_compiled() {
    let mut diagnostics = Vec::new();
    run_detailed_with_diagnostics(
        "p(1). p(a). r(X) :- p(X), X < 2, f(X) != 3.".into(),
        &options(&[]),
        &mut Vec::new(),
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    let records = std::str::from_utf8(&diagnostics).unwrap();
    assert!(
        records.contains(
            "Comparisons: with_form=1; long_operand=0; wide_slot=0; constructor=1; nonnumeric_constant=0; argument_position=0\n"
        ),
        "{records}"
    );
}
