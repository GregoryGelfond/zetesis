//! Preserve optional root-image receipts without inventing unobserved work.
use super::{field, number};
use crate::performance::matrix::RootImageStatistics;
use serde::Deserialize;
use serde_json::{Value, value::RawValue};

#[derive(Deserialize)]
struct Document<'a> {
    #[serde(borrow)]
    statistics: Statistics<'a>,
}
#[derive(Deserialize)]
struct Statistics<'a> {
    #[serde(borrow)]
    search: Search<'a>,
}
#[derive(Deserialize)]
struct Search<'a> {
    #[serde(borrow)]
    root_images: &'a RawValue,
}

pub(super) fn read(
    document: &Value,
    source: Option<&[u8]>,
    stderr: &[u8],
) -> Result<Option<RootImageStatistics>, String> {
    let search = &document["statistics"]["search"];
    let value = &search["root_images"];
    let text = std::str::from_utf8(stderr).map_err(|error| error.to_string())?;
    let mut lines = text
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("candidate root images: "));
    let line = lines.next();
    if lines.next().is_some() {
        return Err("ambiguous root-image text receipt".into());
    }
    if value.is_null() {
        if line.is_some() {
            return Err("root-image text lacks its typed observation".into());
        }
        return Ok(None);
    }
    let receipt = match RootImageStatistics::deserialize(value) {
        Ok(receipt) => receipt,
        Err(initial) => {
            let raw = source.ok_or_else(|| format!("invalid or inexact root images: {initial}"))?;
            let fields: Document<'_> =
                serde_json::from_slice(raw).map_err(|error| error.to_string())?;
            serde_json::from_str(fields.statistics.search.root_images.get())
                .map_err(|error| format!("invalid root images: {error}"))?
        }
    };
    validate(&receipt, search)?;
    text_receipt(
        &receipt,
        line.ok_or("root images lack their text observation")?,
    )?;
    Ok(Some(receipt))
}

fn validate(receipt: &RootImageStatistics, search: &Value) -> Result<(), String> {
    if !matches!(
        receipt.status.as_str(),
        "not_prepared" | "prepared" | "work_declined" | "storage_declined" | "incomplete"
    ) {
        return Err("unsupported root-image status".into());
    }
    let work = receipt
        .preparation_work
        .checked_add(receipt.copying_work)
        .and_then(|work| work.checked_add(receipt.closure_work))
        .ok_or("root-image work sum overflow")?;
    // Candidate reading/search also contain other components. These are
    // containment checks, not a false equality with total search work.
    if work > number(&search["candidate_regions"], "reading_work")?
        || work > number(search, "work")?
    {
        return Err("root-image work exceeds its recorded owner".into());
    }
    Ok(())
}

fn text_receipt(receipt: &RootImageStatistics, line: &str) -> Result<(), String> {
    if line.split("; ").count() != 10 || field(line, "status")? != receipt.status {
        return Err("root-image typed and text shapes disagree".into());
    }
    for (key, expected) in [
        ("groups", receipt.groups as u128),
        ("pairs", receipt.pairs as u128),
        ("preparation work", u128::from(receipt.preparation_work)),
        ("copying work", u128::from(receipt.copying_work)),
        ("closure work", u128::from(receipt.closure_work)),
        ("peak bytes", receipt.peak_bytes),
        ("completed rounds", u128::from(receipt.completed_rounds)),
        ("applied cuts", u128::from(receipt.applied_cuts)),
    ] {
        if field(line, key)?.parse::<u128>().ok() != Some(expected) {
            return Err(format!("root-image typed and text {key} disagree"));
        }
    }
    if field(line, "root refuted")?.parse::<bool>().ok() != Some(receipt.root_refuted) {
        return Err("root-image typed and text refutation disagree".into());
    }
    Ok(())
}
