//! CLI rendering of library-owned host measurements.

use crate::{
    GroundingMeasurement, GroundingOutcome, GroundingPhase, GroundingTimings, GroundingWork,
};
use std::io::{self, Write};

/// Shared field vocabulary for the human and JSON views of the same data.
pub(crate) fn work_fields(work: &GroundingWork) -> [(&'static str, Option<u64>); 30] {
    [
        ("support_rounds", work.support_rounds),
        ("support_producer_visits", work.support_producer_visits),
        (
            "support_snapshot_preparations",
            work.support_snapshot_preparations,
        ),
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
        (
            "domain_narrowed_candidates",
            work.domain_narrowed_candidates,
        ),
        ("binding_snapshots", work.binding_snapshots),
        ("expression_evaluations", work.expression_evaluations),
        ("expression_nodes", work.expression_nodes),
        ("atom_lookups", work.atom_lookups),
        ("atoms_inserted", work.atoms_inserted),
        ("node_lookups", work.node_lookups),
        ("nodes_inserted", work.nodes_inserted),
        ("roots", work.roots),
    ]
}

pub(crate) fn write(sink: &mut impl Write, timings: &GroundingTimings) -> io::Result<()> {
    writeln!(
        sink,
        "Grounding attribution: scope=eager_formula; clock=host-monotonic; schema=1"
    )?;
    for phase in GroundingPhase::ALL {
        write!(sink, "  grounding {}:", phase.label())?;
        if let Some(value) = timings.get(phase) {
            write_measurement(sink, value)?;
        } else {
            writeln!(sink, " unmeasured")?;
        }
    }
    writeln!(
        sink,
        "  grounding scope: counters=selected_work_populations; rule_work=joins_filters_emission_interleaved; overhead=included; relational_grounding=unmeasured; kernel_time=unmeasured"
    )
}

fn write_measurement(sink: &mut impl Write, value: &GroundingMeasurement) -> io::Result<()> {
    field(
        sink,
        "elapsed_ns",
        value.elapsed.map(|value| value.as_nanos()),
    )?;
    for outcome in GroundingOutcome::ALL {
        field(sink, outcome.label(), value.count(outcome).map(u128::from))?;
    }
    for (name, count) in work_fields(&value.work) {
        field(sink, name, count.map(u128::from))?;
    }
    writeln!(sink)
}

fn field(sink: &mut impl Write, name: &str, value: Option<u128>) -> io::Result<()> {
    match value {
        Some(value) => write!(sink, " {name}={value};"),
        None => write!(sink, " {name}=unavailable;"),
    }
}

#[cfg(test)]
#[path = "../tests/support/grounding_timing_contracts.rs"]
mod tests;
