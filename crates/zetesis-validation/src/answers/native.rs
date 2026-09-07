//! Historical native plain-text report reconciliation.
use super::display::{integers, split_atoms};
use super::{
    Error, Issue, Limits, Model, ReportedAnswers, Resource, check, invalid, multiplicities,
};

pub(super) fn parse(text: &str, optimized: bool, limits: Limits) -> Result<ReportedAnswers, Error> {
    let (satisfiable, reported_count) = native_summary(text, optimized)?;
    let mut witnesses: Vec<(Model, Option<Vec<i64>>)> = Vec::new();
    let mut lines = text.lines();
    let mut symbol_count = 0usize;
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            check(
                Resource::Witnesses,
                limits.max_witnesses,
                witnesses.len().saturating_add(1),
            )?;
            let model = split_atoms(
                lines
                    .next()
                    .ok_or_else(|| invalid(Issue::MissingField, "missing native model line"))?,
                false,
            )?;
            symbol_count = symbol_count
                .checked_add(model.len())
                .ok_or_else(|| invalid(Issue::CountOverflow, "symbol count overflow"))?;
            check(Resource::Symbols, limits.max_symbols, symbol_count)?;
            witnesses.push((model, None));
        } else if let Some(raw) = line.strip_prefix("Optimization:") {
            if !optimized {
                return Err(invalid(
                    Issue::Contradiction,
                    "unexpected objective vector in nonoptimized native output",
                ));
            }
            let cost = integers(raw, limits)?;
            let witness = witnesses.last_mut().ok_or_else(|| {
                invalid(Issue::MissingField, "cost without preceding native model")
            })?;
            if witness.1.replace(cost).is_some() {
                return Err(invalid(
                    Issue::Contradiction,
                    "duplicate native cost record",
                ));
            }
        }
    }
    if satisfiable == witnesses.is_empty() {
        return Err(invalid(
            Issue::Contradiction,
            "native model/status mismatch",
        ));
    }
    if u64::try_from(witnesses.len())
        .map_err(|_| invalid(Issue::CountOverflow, "witness count exceeds u64"))?
        != reported_count
    {
        return Err(invalid(
            Issue::Contradiction,
            "native answer count disagrees with Models summary",
        ));
    }
    let cost = if optimized && satisfiable {
        if witnesses.iter().any(|(_, cost)| cost.is_none()) {
            return Err(invalid(
                Issue::Contradiction,
                "native optimized output requires an Optimization: vector for every model",
            ));
        }
        let dimensions = witnesses[0].1.as_ref().map(Vec::len);
        if witnesses
            .iter()
            .any(|(_, cost)| cost.as_ref().map(Vec::len) != dimensions)
        {
            return Err(invalid(
                Issue::Contradiction,
                "native objective vectors have inconsistent dimensions",
            ));
        }
        witnesses
            .iter()
            .filter_map(|(_, cost)| cost.as_ref())
            .min()
            .cloned()
    } else {
        None
    };
    let selected: Vec<_> = witnesses
        .into_iter()
        .filter(|(_, value)| cost.is_none() || value == &cost)
        .collect();
    let model_count = u64::try_from(selected.len())
        .map_err(|_| invalid(Issue::CountOverflow, "witness count exceeds u64"))?;
    let model_multiplicities = multiplicities(selected.into_iter().map(|(model, _)| model))?;
    Ok(ReportedAnswers {
        satisfiable,
        cost,
        model_multiplicities,
        model_count,
        solver: "zetesis native output".into(),
    })
}

fn native_summary(text: &str, optimized: bool) -> Result<(bool, u64), Error> {
    let coverage: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("Coverage:"))
        .collect();
    if coverage != ["Coverage: exhausted"]
        || text.lines().any(|line| line.starts_with("INCOMPLETE:"))
    {
        return Err(invalid(
            Issue::Incomplete,
            "native output requires exactly one exhausted coverage record",
        ));
    }
    let statuses: Vec<_> = text
        .lines()
        .filter(|line| matches!(*line, "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND"))
        .collect();
    if statuses.len() != 1 || (!optimized && statuses[0] == "OPTIMUM FOUND") {
        return Err(invalid(
            Issue::Contradiction,
            "native status is missing, contradictory, or unexpectedly optimized",
        ));
    }
    let summaries: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Models:"))
        .collect();
    if summaries.len() != 1 {
        return Err(invalid(
            Issue::Contradiction,
            "native output requires exactly one Models summary",
        ));
    }
    let count = summaries[0]
        .split(';')
        .next()
        .ok_or_else(|| invalid(Issue::MissingField, "missing Models count"))?
        .trim()
        .parse()
        .map_err(|_| invalid(Issue::MalformedField, "invalid Models count"))?;
    Ok((statuses[0] != "UNSATISFIABLE", count))
}
