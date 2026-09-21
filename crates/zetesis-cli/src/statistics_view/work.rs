//! Compact projections of retained receipts; requested settings never fill gaps.

use std::{fmt::Display, io};

use zetesis_presentation::{Alignment, Column, Row, Table};

use crate::{GroundingPhase, GroundingTimings, SemanticOutcome};

pub(super) fn execution(semantic: Option<&SemanticOutcome>) -> String {
    let Some(semantic) = semantic else {
        return "unavailable".into();
    };
    if let Some(execution) = semantic.batch_execution() {
        let operation = match execution.operation {
            zetesis_solve::MembershipOperation::General => "general reduct",
            zetesis_solve::MembershipOperation::Tight => "tight support",
        };
        return format!("Custom executor ({operation}); hardware unspecified");
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
    if let Some(execution) = semantic.lazy_execution() {
        return format!(
            "GPU lazy closure; {} ({})",
            execution.adapter, execution.backend
        );
    }
    if semantic.shared_execution().is_some() {
        return "CPU shared lazy closure".into();
    }
    if let Some(execution) = semantic.closure_execution() {
        return format!("CPU independent {} closure", execution.route.label());
    }
    if let Some(statistics) = semantic.countermodel_statistics() {
        let operation = match statistics.certified.and_then(|receipt| receipt.plan) {
            Some(zetesis_sat::CertificatePlanStatistics::Tight(_)) => "tight support",
            Some(zetesis_sat::CertificatePlanStatistics::Positive(_)) => "positive consequences",
            None => "general reduct",
        };
        return format!("CPU native formula ({operation})");
    }
    if semantic
        .candidate_statistics()
        .is_some_and(|statistics| statistics.root_refuted)
    {
        return "No membership check; root refuted".into();
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
        rows.push(count(
            "Verified memberships",
            semantic.verified_models(),
            "includes queued results",
        ));
        if let Some(statistics) = semantic.countermodel_statistics() {
            formula(&mut rows, statistics);
        }
        if let Some(statistics) = semantic.candidate_statistics() {
            rows.push(count(
                "Candidate restriction work",
                statistics.restriction_work,
                "construction and traversal; includes stopped work",
            ));
        }
        if let Some(execution) = semantic.closure_execution() {
            closure(&mut rows, execution);
        }
        if let Some(execution) = semantic.shared_execution() {
            rows.push(count(
                "Shared source work",
                execution.source_work,
                "host source operations; includes attempted batches",
            ));
            rows.push(count(
                "Shared world work",
                execution.world_work,
                "per-world record visits and tests",
            ));
        }
        if let Some(execution) = semantic.lazy_execution() {
            rows.push(count(
                "Lazy source work",
                execution.source_work,
                "charged host operations",
            ));
            rows.push(count(
                "Lazy device dispatches",
                execution.dispatches,
                "submitted nonempty chunks; not decoded results",
            ));
        }
        if let Some(execution) = semantic.formula_execution() {
            device(&mut rows, execution);
            completion(&mut rows, &execution.completion);
        }
        if let Some(execution) = semantic.batch_execution() {
            rows.push(count(
                "Custom checker calls",
                execution.batches.checker_calls,
                "includes failed calls; no hardware work implied",
            ));
            rows.push(count(
                "Committed batch candidates",
                execution.batches.committed,
                "membership completed and committed",
            ));
            completion(&mut rows, &execution.completion);
        }
        if let Some(objective) = semantic.incumbent() {
            rows.push(count(
                "Objective evaluation work",
                objective.work,
                "cumulative score operations",
            ));
        }
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

fn formula(rows: &mut Vec<Row>, statistics: &zetesis_sat::Statistics) {
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

fn closure(rows: &mut Vec<Row>, execution: &crate::ClosureExecutionStatistics) {
    let scope = match execution.route {
        crate::ClosureRoute::Eager => "eager scan units; completed checks only",
        crate::ClosureRoute::Lazy(_) => "join/copy units; completed checks only",
    };
    rows.push(count("Closure work", execution.work, scope));
    rows.push(count(
        "Stopped closure checks",
        execution.stopped_checks,
        "partial work omitted from closure totals",
    ));
    if let crate::ClosureRoute::Lazy(joins) = execution.route {
        rows.push(count(
            "Closure tuple probes",
            joins.tuple_probes,
            "rows offered to matching; completed checks only",
        ));
    }
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
    rows.push(count(
        "CPU residuals committed",
        execution.cpu_residuals,
        "completed exact residual results committed",
    ));
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
                "eager formula rule instantiation only".into(),
            ]));
        }
    } else {
        rows.push(Row::new([
            "Eager rule work",
            "unavailable",
            "phase not entered or attribution not retained",
        ]));
    }
}

fn count(label: &str, value: impl Display, scope: &str) -> Row {
    Row::new([label.to_owned(), value.to_string(), scope.to_owned()])
}
