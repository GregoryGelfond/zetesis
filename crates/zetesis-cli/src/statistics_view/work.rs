//! Compact projections of retained receipts; requested settings never fill gaps.

use std::{fmt::Display, io};

use zetesis_presentation::{Alignment, Column, Row, Table};

use crate::{GroundingPhase, GroundingTimings, SemanticOutcome};

pub(super) fn execution(semantic: Option<&SemanticOutcome>) -> String {
    let base = base_execution(semantic);
    if semantic.is_some_and(|value| value.terminal_execution().is_some()) {
        format!("{base}; eager base with host answer reconstruction")
    } else {
        base
    }
}

fn base_execution(semantic: Option<&SemanticOutcome>) -> String {
    let Some(semantic) = semantic else {
        return "unavailable".into();
    };
    if semantic.source_evaluation().is_some() {
        return "CPU direct source evaluation during admission".into();
    }
    if let Some(execution) = semantic.hybrid_execution() {
        return if execution.streamed.templates == 0 {
            "CPU hybrid formula; eager core, no streamed constraints".into()
        } else {
            "CPU hybrid formula; eager core, streamed constraints".into()
        };
    }
    if let Some(execution) = semantic.formula_execution() {
        return if execution.adapter.is_empty() {
            "CPU batched formula completion".into()
        } else if execution.tight_work_per_candidate.is_some() {
            format!("GPU tight support; {}", execution.adapter)
        } else {
            format!(
                "GPU propagation with CPU residual completion; {}",
                execution.adapter
            )
        };
    }
    if let Some(statistics) = semantic.countermodel_statistics() {
        let operation = match statistics.certified.and_then(|receipt| receipt.plan) {
            Some(zetesis_stable::CertificatePlanStatistics::Tight(_)) => "tight support",
            Some(zetesis_stable::CertificatePlanStatistics::Positive(_)) => "positive consequences",
            Some(zetesis_stable::CertificatePlanStatistics::Stratified(_)) => {
                "stratified consequences"
            }
            None => "general reduct",
        };
        return format!("CPU native formula ({operation})");
    }
    "unavailable (no retained execution receipt)".into()
}

pub(super) fn table(
    semantic: Option<&SemanticOutcome>,
    grounding: &GroundingTimings,
) -> io::Result<Table> {
    // Each branch appends a fixed set of fields. No model, region or captured
    // stream is copied; retained view text is independent of enumeration size.
    let mut rows = Vec::new();
    if let Some(semantic) = semantic {
        semantic_rows(&mut rows, semantic);
    } else {
        rows.push(Row::new([
            "Execution work",
            "unavailable",
            "no semantic session receipt",
        ]));
    }
    grounding_rows(&mut rows, grounding);
    Table::new(
        "Work receipts",
        vec![
            Column::new("Measure", Alignment::Left),
            Column::new("Count", Alignment::Right),
            Column::new("Scope", Alignment::Left),
        ],
        rows,
    )
    .map_err(io::Error::other)
}

fn semantic_rows(rows: &mut Vec<Row>, semantic: &SemanticOutcome) {
    rows.push(count(
        "Verified memberships",
        semantic.verified_models(),
        if semantic.source_evaluation().is_some() {
            "original source answers after exact evaluation"
        } else if semantic.terminal_execution().is_some() {
            "original answers after complete reconstruction"
        } else if semantic.hybrid_execution().is_some() {
            "original answers after completed constraint checks"
        } else {
            "includes queued results"
        },
    ));
    if let Some(execution) = semantic.hybrid_execution() {
        hybrid(rows, execution);
    }
    if let Some(execution) = semantic.terminal_execution() {
        terminal(rows, execution);
    }
    if let Some(statistics) = semantic.source_evaluation() {
        rows.push(count(
            "Source evaluation work",
            statistics.work,
            "cumulative source admission; support, arithmetic and exact activity",
        ));
        rows.push(count(
            "Source substitutions",
            statistics.substitutions,
            "completed bindings charged during source admission",
        ));
    }
    if let Some(statistics) = semantic.model_construction() {
        model_construction(rows, statistics);
    }
    if let Some(statistics) = semantic.countermodel_statistics() {
        formula(rows, statistics);
    }
    if let Some(execution) = semantic.formula_execution() {
        device(rows, execution);
        completion(rows, &execution.completion);
    }
    if let Some(objective) = semantic.incumbent() {
        rows.push(count(
            "Objective evaluation work",
            objective.work,
            "cumulative score operations",
        ));
    }
}

fn model_construction(
    rows: &mut Vec<Row>,
    statistics: &zetesis_solve::ModelConstructionStatistics,
) {
    rows.push(count(
        "Model construction work",
        statistics.work,
        "cumulative prepared semantic ranks and selected positions",
    ));
    rows.push(count(
        "Prepared model order bytes",
        statistics.prepared_bytes,
        "retained rank capacity; excludes catalog payload",
    ));
    rows.push(count(
        "Model construction peak bytes",
        statistics.peak_bytes,
        "order and active construction metadata; excludes earlier models",
    ));
    rows.push(count(
        "Constructed models",
        statistics.constructed,
        "complete selections before scoring or publication",
    ));
}

fn terminal(rows: &mut Vec<Row>, execution: &zetesis_solve::TerminalExecutionStatistics) {
    for (label, value, scope) in [
        (
            "Base answers consumed",
            execution.base_answers,
            "base membership before full reconstruction",
        ),
        (
            "Original answers reconstructed",
            execution.reconstructed,
            "complete original membership",
        ),
        (
            "Reconstruction pending",
            execution.pending,
            "no original membership established",
        ),
        (
            "Reconstruction work",
            execution.reconstruction.work,
            "cumulative source admission and reconstruction; excludes base search",
        ),
        (
            "Reconstruction substitutions",
            execution.reconstruction.substitutions,
            "cumulative source admission and reconstruction",
        ),
        (
            "Reconstruction allowance",
            execution.reconstruction.allowance.work,
            "work each answer may charge after admission",
        ),
        (
            "Largest answer reconstruction",
            execution.reconstruction.peak.work,
            "work of the costliest answer, refused included",
        ),
    ] {
        rows.push(count(label, value, scope));
    }
}

fn hybrid(rows: &mut Vec<Row>, execution: &zetesis_solve::HybridExecutionStatistics) {
    let streamed = execution.streamed;
    for (label, value, scope) in [
        (
            "Streamed constraint templates",
            streamed.templates as u64,
            "checked during search; they did not fit in the formula",
        ),
        (
            "Streamed constraint instances",
            streamed.instances,
            "possible instances, visited during admission",
        ),
        (
            "Grounded constraint templates",
            streamed.retained_templates as u64,
            "eligible for streaming, but fit in the formula",
        ),
        (
            "Grounded constraint instances",
            streamed.retained_instances,
            "checked as formula nodes",
        ),
    ] {
        rows.push(count(label, value, scope));
    }
    for (label, value, scope) in [
        (
            "Core answers checked",
            execution.core_answers,
            "consumed proposals, not original membership",
        ),
        (
            "Constraint checks accepted",
            execution.accepted,
            "original answer membership established",
        ),
        (
            "Constraint checks rejected",
            execution.rejected,
            "observed source constraint violation",
        ),
        (
            "Constraint checks pending",
            execution.pending,
            "no original membership established",
        ),
        (
            "Constraint checking work",
            execution.constraints.work,
            "cumulative; independent of admission and reduct work",
        ),
        (
            "Constraint substitutions",
            execution.constraints.substitutions,
            "completed rows, including false filters",
        ),
    ] {
        rows.push(count(label, value, scope));
    }
    rows.push(count(
        "Constraint scalar bytes",
        execution.constraints.scalar_bytes,
        "cumulative copied payload; not live memory or RSS",
    ));
}

fn formula(rows: &mut Vec<Row>, statistics: &zetesis_stable::Statistics) {
    if let Some(filter) = statistics.region_filter {
        let scope = if filter.overflowed {
            "original candidate regions; counts overflowed"
        } else {
            "original candidate regions; excludes reduct queries"
        };
        rows.push(count("Source region checks", filter.checks, scope));
        rows.push(count("Source regions rejected", filter.refuted, scope));
        rows.push(count("Source region failures", filter.failed, scope));
    }
    rows.push(count(
        "Formula search work",
        statistics.search.work,
        "encoding, certificates and search; includes stopped work",
    ));
    rows.push(count(
        "Formula decisions",
        statistics.search.decisions,
        "branch decisions",
    ));
    rows.push(count(
        "Reduct queries",
        statistics.countermodel_queries,
        "started queries, not just completed checks",
    ));
    if let Some(regions) = statistics.regions {
        rows.push(count(
            "Candidate regions visited",
            regions.counts.regions,
            "outer candidate tree",
        ));
        rows.push(count(
            "Candidate region reading work",
            regions.counts.work,
            "subtotal of formula search work",
        ));
    }
    rows.push(count(
        "Reduct regions visited",
        statistics.reduct.regions.regions,
        "zero when no region queries ran",
    ));
}

fn device(rows: &mut Vec<Row>, execution: &crate::FormulaExecutionStatistics) {
    if execution.adapter.is_empty() {
        return;
    }
    rows.push(count(
        "GPU submitted candidates",
        execution.gpu_submitted_candidates,
        "includes submissions without a returned result",
    ));
    rows.push(count(
        "GPU decoded candidates",
        execution.gpu_candidates,
        "successfully returned results",
    ));
    rows.push(count(
        "GPU decoded work",
        execution.gpu_work,
        "primitive units; unreturned work unknown",
    ));
    rows.push(Row::new([
        "GPU workgroup size".to_owned(),
        execution
            .gpu_workgroup_size
            .map_or_else(|| "unavailable".to_owned(), |size| size.to_string()),
        "invocations per candidate in the decoded batches".to_owned(),
    ]));
    rows.push(count(
        "CPU residuals committed",
        execution.cpu_residuals,
        "completed exact residual results committed",
    ));
    if let Some(routes) = execution.gpu_tight_routes {
        rows.push(Row::new([
            "Tight GPU slots by route".to_owned(),
            format!(
                "{} complete; {} dense reuse; {} update",
                routes.complete, routes.dense_reuse, routes.update
            ),
            format!(
                "dense reuse includes {} beyond the update threshold; updates re-evaluated {} nodes",
                routes.beyond_threshold, routes.nodes_evaluated
            ),
        ]));
    }
    if let Some(updates) = execution.gpu_tight_updates {
        let source = if updates.steady_state {
            "as the last batch with reuse authority reported"
        } else {
            "as the last batch reported; none held reuse authority"
        };
        rows.push(Row::new([
            "Tight GPU update route".to_owned(),
            updates.availability.label().to_owned(),
            updates.count_bits.map_or_else(
                || source.to_owned(),
                |bits| format!("{bits}-bit support counts; {source}"),
            ),
        ]));
        rows.push(count(
            "Tight GPU guard declines",
            updates.guard_declined_batches,
            "decoded batches the runtime guard declined the update route for",
        ));
    }
    rows.push(Row::new(compute_passes(&execution.gpu_compute_timing)));
}

// The total covers measured batches only; an unresolved batch, whose interval
// was not read before the record was taken, leaves it incomplete.
fn compute_passes(timing: &zetesis_backend::ComputeTiming) -> [String; 3] {
    [
        "GPU compute passes".to_owned(),
        timing.elapsed.map_or_else(
            || format!("unavailable ({})", timing.status.label()),
            |elapsed| format!("{:.3} ms", elapsed.as_secs_f64() * 1_000.0),
        ),
        format!(
            "{} measured batches; {} unavailable; {} unresolved; host preparation, transfers and waits excluded{}",
            timing.measured_batches,
            timing.unavailable_batches,
            timing.unresolved_batches,
            if timing.overflowed {
                "; accounting incomplete"
            } else if timing.unresolved_batches != 0 {
                "; total incomplete"
            } else {
                ""
            },
        ),
    ]
}

fn completion(rows: &mut Vec<Row>, execution: &crate::CompletionAccounting) {
    let scope = if execution.overflowed {
        "entered slots, including retries; accounting saturated"
    } else {
        "entered slots, including certificates and retries"
    };
    rows.push(count("Host completion attempts", execution.entered, scope));
}

fn grounding_rows(rows: &mut Vec<Row>, grounding: &GroundingTimings) {
    if let Some(measurement) = grounding.get(GroundingPhase::RuleInstantiation) {
        for (label, value) in [
            ("Rule join probes", measurement.work.join_probes),
            (
                "Rule table preparations",
                measurement.work.table_preparations,
            ),
            ("Rule table reuses", measurement.work.table_reuses),
            ("Rule table probes", measurement.work.table_probes),
        ] {
            rows.push(Row::new([
                label.to_owned(),
                value.map_or_else(|| "unavailable".into(), |value| value.to_string()),
                "formula instantiation or certified source evaluation".into(),
            ]));
        }
    } else {
        rows.push(Row::new([
            "Rule work",
            "unavailable",
            "phase not entered or attribution not retained",
        ]));
    }
}

fn count(label: &str, value: impl Display, scope: &str) -> Row {
    Row::new([label.to_owned(), value.to_string(), scope.to_owned()])
}

#[cfg(test)]
mod tests {
    use super::{compute_passes, device};
    use zetesis_backend::{BatchComputeTime, ComputeTiming, ComputeTimingStatus};

    fn measured() -> ComputeTiming {
        let mut timing = ComputeTiming::new(ComputeTimingStatus::Enabled);
        timing.record(BatchComputeTime::Measured(
            std::time::Duration::from_micros(1_500),
        ));
        timing
    }

    #[test]
    fn a_settled_total_is_not_marked_incomplete() {
        let [_, total, scope] = compute_passes(&measured());
        assert_eq!(total, "1.500 ms");
        assert!(scope.starts_with("1 measured batches; 0 unavailable; 0 unresolved;"));
        assert!(!scope.contains("incomplete"), "{scope}");
    }

    #[test]
    fn an_unresolved_batch_marks_the_total_incomplete() {
        let mut timing = measured();
        timing.unresolved_batches = 1;
        let [_, total, scope] = compute_passes(&timing);
        assert_eq!(total, "1.500 ms");
        assert!(scope.contains("1 unresolved"), "{scope}");
        assert!(scope.ends_with("; total incomplete"), "{scope}");
    }

    #[test]
    fn device_rows_report_the_dispatched_workgroup_size() {
        let mut execution = crate::FormulaExecutionStatistics::default();
        "FORMAT FIXTURE: no physical execution".clone_into(&mut execution.adapter);
        execution.gpu_workgroup_size = Some(256);
        let mut rows = Vec::new();
        device(&mut rows, &execution);
        let rendered = format!("{rows:?}");
        assert!(
            rendered.contains(r#""GPU workgroup size", "256""#),
            "{rendered}"
        );
    }

    #[test]
    fn tight_device_rows_report_routes_and_update_availability() {
        let mut execution = crate::FormulaExecutionStatistics::default();
        "FORMAT FIXTURE: no physical execution".clone_into(&mut execution.adapter);
        let mut routes = zetesis_solve::TightRouteStatistics::default();
        routes.complete = 4;
        routes.dense_reuse = 3;
        routes.beyond_threshold = 1;
        routes.update = 9;
        routes.nodes_evaluated = 85;
        execution.gpu_tight_routes = Some(routes);
        let mut updates = zetesis_solve::TightUpdateStatistics::default();
        updates.availability = zetesis_solve::TightUpdateAvailability::Admitted;
        updates.count_bits = Some(16);
        updates.steady_state = true;
        updates.guard_declined_batches = 7;
        execution.gpu_tight_updates = Some(updates);
        let mut rows = Vec::new();
        device(&mut rows, &execution);
        let text = format!("{rows:?}");
        for expected in [
            "Tight GPU slots by route",
            "4 complete; 3 dense reuse; 9 update",
            "dense reuse includes 1 beyond the update threshold; updates re-evaluated 85 nodes",
            "Tight GPU update route",
            "admitted",
            "16-bit support counts; as the last batch with reuse authority reported",
            r#"["Tight GPU guard declines", "7", "decoded batches the runtime guard declined the update route for"]"#,
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
    }

    #[test]
    fn tight_update_row_says_when_no_batch_held_authority() {
        let mut execution = crate::FormulaExecutionStatistics::default();
        "FORMAT FIXTURE: no physical execution".clone_into(&mut execution.adapter);
        let mut updates = zetesis_solve::TightUpdateStatistics::default();
        updates.availability = zetesis_solve::TightUpdateAvailability::NarrowSchedule;
        execution.gpu_tight_updates = Some(updates);
        let mut rows = Vec::new();
        device(&mut rows, &execution);
        let text = format!("{rows:?}");
        for expected in [
            "narrow_schedule",
            "as the last batch reported; none held reuse authority",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
    }
}
