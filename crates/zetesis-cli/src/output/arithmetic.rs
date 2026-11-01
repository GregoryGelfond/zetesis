//! Arithmetic preparation records from the original candidate owner.

use super::{Buffer, RunError};
use zetesis_stable::{CardinalityStatistics, ParityStatistics, PreparationWork};

pub(super) fn parity(out: &mut Buffer, stats: Option<&ParityStatistics>) -> Result<(), RunError> {
    let Some(stats) = stats else {
        return out.text("null");
    };
    out.text("{\"status\":")?;
    out.string(stats.status.label())?;
    out.number_field("rows", stats.rows)?;
    out.number_field("one", stats.one)?;
    out.number_field("two", stats.two)?;
    out.number_field("wider", stats.wider)?;
    out.number_field("source_roots", stats.source_roots)?;
    out.number_field("inexact_hulls", stats.inexact_hulls)?;
    out.text(",\"refused\":{\"unsupported_clauses\":")?;
    out.text(&stats.unsupported_clauses.to_string())?;
    out.number_field("unsupported_definitions", stats.unsupported_definitions)?;
    out.text("}")?;
    out.number_field("provenance_rejected", stats.provenance_rejected)?;
    out.number_field("rank", stats.rank)?;
    out.number_field("row_additions", stats.row_additions)?;
    out.number_field("preparation_work", stats.preparation_work)?;
    out.number_field("preparation_bytes", stats.preparation_bytes)?;
    out.number_field("retained_bytes", stats.retained_bytes)?;
    out.text(",\"phases\":")?;
    phases(out, stats.phases)?;
    out.text(",\"root_refuted\":")?;
    out.text(if stats.root_refuted { "true" } else { "false" })?;
    out.number_field("units", stats.units)?;
    out.text("}")
}

pub(super) fn cardinality(
    out: &mut Buffer,
    stats: Option<&CardinalityStatistics>,
) -> Result<(), RunError> {
    let Some(stats) = stats else {
        return out.text("null");
    };
    out.text("{\"status\":")?;
    out.string(stats.status.label())?;
    out.number_field("rows", stats.rows)?;
    out.number_field("lower", stats.lower)?;
    out.number_field("upper", stats.upper)?;
    out.number_field("guarded", stats.guarded)?;
    out.number_field("carriers", stats.carriers)?;
    out.number_field("attempts", stats.attempts)?;
    out.number_field("refused", stats.refused)?;
    out.number_field("families", stats.families)?;
    out.number_field("occurrences", stats.occurrences)?;
    out.number_field("provenance_rejected", stats.provenance_rejected)?;
    out.number_field("preparation_work", stats.preparation_work)?;
    out.number_field("preparation_bytes", stats.preparation_bytes)?;
    out.number_field("retained_bytes", stats.retained_bytes)?;
    out.text(",\"phases\":")?;
    phases(out, stats.phases)?;
    out.text(",\"root_refuted\":")?;
    out.text(if stats.root_refuted { "true" } else { "false" })?;
    out.text("}")
}

fn phases(out: &mut Buffer, work: PreparationWork) -> Result<(), RunError> {
    out.text("{\"recognition\":")?;
    out.text(&work.recognition.to_string())?;
    out.number_field("provenance", work.provenance)?;
    out.number_field("reduction", work.reduction)?;
    out.number_field("discovery", work.discovery)?;
    out.number_field("candidate_checks", work.candidate_checks)?;
    out.number_field("index", work.index)?;
    out.number_field("root_seed", work.root_seed)?;
    out.text("}")
}
