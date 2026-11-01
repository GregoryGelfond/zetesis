//! Exact GPU residual handovers, independently of device reason totals.
mod samples;
use super::{Buffer, RunError, optional_number};
use zetesis_solve::{
    FormulaResidualStatistics, ResidualHandoverAttempt, ResidualHandoverStatistics,
    ResidualSearchStatistics,
};

pub(super) fn reasons(
    out: &mut Buffer,
    counts: &FormulaResidualStatistics,
) -> Result<(), RunError> {
    out.text("{\"fixed_point\":")?;
    out.text(&counts.fixed_point.to_string())?;
    out.number_field("round_limit", counts.round_limit)?;
    out.number_field("work_limit", counts.work_limit)?;
    out.text("}")
}

pub(super) fn write(
    out: &mut Buffer,
    handovers: Option<&ResidualHandoverStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = handovers else {
        return out.text("null");
    };
    out.text("{\"attempts\":")?;
    out.text(&stats.attempts.to_string())?;
    out.number_field("incomplete_attempts", stats.incomplete_attempts)?;
    out.number_field("committed_slates", stats.committed_slates)?;
    out.number_field("zero_residual_slates", stats.zero_residual_slates)?;
    out.number_field("query_work", stats.query_work)?;
    out.text(",\"complete_work\":")?;
    out.text(if stats.complete_work { "true" } else { "false" })?;
    out.text(",\"counts_exact\":")?;
    out.text(if stats.counts_exact { "true" } else { "false" })?;
    out.text(",\"last_attempt\":")?;
    if let Some(last) = &stats.last_attempt {
        attempt(out, last)?;
    } else {
        out.text("null")?;
    }
    if let Some(value) = &stats.samples {
        out.text(",\"samples\":")?;
        samples::write(out, value)?;
    }
    out.text("}")
}

fn attempt(out: &mut Buffer, last: &ResidualHandoverAttempt) -> Result<(), RunError> {
    out.text("{\"ordinal\":")?;
    out.text(&last.ordinal.to_string())?;
    out.number_field("slate_sequence", last.slate_sequence)?;
    for (name, flag) in [
        ("retry", last.retry),
        ("decode_complete", last.decode_complete),
        ("committed", last.committed),
    ] {
        out.text(",")?;
        out.string(name)?;
        out.text(if flag { ":true" } else { ":false" })?;
    }
    out.text(",\"residuals\":")?;
    reasons(out, &last.residuals)?;
    out.number_field("open_atoms", last.open_atoms)?;
    out.number_field("peak_observation_bytes", last.peak_observation_bytes)?;
    out.text(",\"search\":")?;
    if let Some(summary) = &last.search {
        search(out, summary)?;
    } else {
        out.text("null")?;
    }
    out.text(",\"by_reason\":")?;
    if let Some(groups) = &last.by_reason {
        out.text("{\"fixed_point\":")?;
        reason_search(out, &groups.fixed_point)?;
        out.text(",\"round_limit\":")?;
        reason_search(out, &groups.round_limit)?;
        out.text(",\"work_limit\":")?;
        reason_search(out, &groups.work_limit)?;
        out.text("}")?;
    } else {
        out.text("null")?;
    }
    out.text("}")
}

// Both wire records retain the same CPU totals, with a different qualifier.
fn start_totals(
    out: &mut Buffer,
    row: &zetesis_solve::ResidualReasonSearch,
) -> Result<(), RunError> {
    out.text("{\"unentered\":")?;
    out.text(&row.unentered.to_string())?;
    out.number_field("certified", row.certified)?;
    out.number_field("exhausted", row.exhausted)?;
    out.number_field("witnesses", row.witnesses)?;
    out.number_field("stopped", row.stopped)?;
    out.number_field("visited", row.visited)?;
    out.number_field("attempted_splits", row.attempted_splits)?;
    out.number_field("published_splits", row.published_splits)?;
    out.number_field("work", row.work)
}

fn search(out: &mut Buffer, value: &ResidualSearchStatistics) -> Result<(), RunError> {
    start_totals(out, &value.totals)?;
    out.text(",\"deepest\":")?;
    optional_number(out, value.deepest.map(|depth| depth as u128))?;
    out.text(",\"width\":")?;
    out.optional_number(value.width)?;
    out.text(",\"host_time_ns\":")?;
    optional_number(out, value.host_time.map(|duration| duration.as_nanos()))?;
    out.text("}")
}

fn reason_search(
    out: &mut Buffer,
    row: &zetesis_solve::ResidualReasonSearch,
) -> Result<(), RunError> {
    start_totals(out, row)?;
    out.number_field("open_atoms", row.open_atoms)?;
    out.text("}")
}
