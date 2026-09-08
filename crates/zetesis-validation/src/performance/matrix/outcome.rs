//! Failure census checks the authored footer independently of completed model decoding.
use serde_json::Value;

use super::Decision;
use crate::process::Exit;

type Failure = (Decision, String);

/// Success still requires the full native model decoder and telemetry checks.
pub(super) fn check(document: &Value, exit: Option<Exit>) -> Result<(), Failure> {
    if document["schema"] != 1 || document["format"] != "zetesis" {
        return Err(invalid("unsupported native envelope"));
    }
    let outcome = &document["outcome"];
    let status = outcome["status"].as_str();
    let error = outcome
        .get("error")
        .ok_or_else(|| invalid("missing native error field"))?;
    if matches!(status, Some("satisfiable" | "unsatisfiable")) {
        if !error.is_null() {
            return Err(invalid(
                "successful native status contradicts reported error",
            ));
        }
        return exited(exit, 0);
    }
    partial(document)?;
    match status {
        Some("incomplete") => {
            if !error.is_null()
                || outcome["completion"] != "interrupted"
                || outcome["coverage"] != "partial"
                || outcome["interruption"].is_null()
                || outcome["verified_models"].as_u64().is_none()
            {
                return Err(invalid(
                    "incomplete native status lacks coherent partial coverage",
                ));
            }
            exited(exit, 3)?;
            Err((
                Decision::Incomplete,
                "native reported incomplete coverage; full interruption retained".into(),
            ))
        }
        Some("failed") => {
            let kind = error["kind"]
                .as_str()
                .filter(|kind| !kind.is_empty())
                .ok_or_else(|| invalid("failed native status lacks an error kind"))?;
            if error["secondary_output_failure"].as_bool().is_none() {
                return Err(invalid(
                    "failed native status lacks secondary-output accounting",
                ));
            }
            exited(exit, 2)?;
            let decision = match kind {
                "backend_unavailable" => Decision::BackendUnavailable,
                "unsupported_combination"
                | "unsupported_oracle"
                | "admission"
                | "bundle_admission"
                | "formula_admission"
                | "formula_bundle_admission"
                | "expansion" => Decision::Refused,
                _ => Decision::InvocationFailure,
            };
            Err((
                decision,
                format!("native reported failure kind={kind}; full envelope retained"),
            ))
        }
        _ => Err(invalid("unknown native outcome status")),
    }
}

fn exited(exit: Option<Exit>, expected: i32) -> Result<(), Failure> {
    if exit.is_some_and(|exit| exit.signal.is_none() && exit.code == Some(expected)) {
        Ok(())
    } else {
        Err((
            Decision::InvocationFailure,
            format!("native exit contradicts footer; expected code {expected}"),
        ))
    }
}

/// Only envelope/count consistency is checked here; partial models are not a
/// complete answer family and their uninterpreted records remain in the capture.
fn partial(document: &Value) -> Result<(), Failure> {
    let outcome = &document["outcome"];
    let completion = coverage(outcome)?;
    interruption(outcome, completion)?;
    publication(document, completion)?;
    let optimization = outcome
        .get("optimization")
        .ok_or_else(|| invalid("missing optimization field"))?;
    if !optimization.is_null()
        && optimization["optimal"].as_bool() != Some(completion == "exhausted")
    {
        return Err(invalid("native optimization contradicts completion"));
    }
    if !document
        .get("statistics")
        .is_some_and(|value| value.is_null() || value.is_object())
    {
        return Err(invalid("missing native statistics field"));
    }
    Ok(())
}
fn coverage(outcome: &Value) -> Result<&Value, Failure> {
    let completion = outcome
        .get("completion")
        .ok_or_else(|| invalid("missing completion"))?;
    let expected = match completion.as_str() {
        Some("exhausted") => "exhausted",
        Some("requested_models" | "interrupted") => "partial",
        None if completion.is_null() => "unavailable",
        _ => return Err(invalid("unknown completion")),
    };
    if outcome["coverage"] != expected {
        return Err(invalid("native completion contradicts coverage"));
    }
    Ok(completion)
}
fn interruption(outcome: &Value, completion: &Value) -> Result<(), Failure> {
    let interruption = outcome
        .get("interruption")
        .ok_or_else(|| invalid("missing interruption"))?;
    if !interruption.is_null()
        && (!matches!(
            interruption["kind"].as_str(),
            Some("oracle" | "countermodel" | "objective" | "incumbent")
        ) || interruption["code"].as_str().is_none_or(str::is_empty)
            || interruption["detail"].as_str().is_none())
    {
        return Err(invalid("malformed native interruption"));
    }
    if interruption.is_null() == (completion == "interrupted") {
        return Err(invalid("native completion contradicts interruption"));
    }
    Ok(())
}
fn publication(document: &Value, completion: &Value) -> Result<(), Failure> {
    let outcome = &document["outcome"];
    let models = document["models"]
        .as_array()
        .ok_or_else(|| invalid("missing native records"))?;
    let published = outcome["published_models"]
        .as_u64()
        .ok_or_else(|| invalid("missing publication count"))?;
    if usize::try_from(published).ok() != Some(models.len()) {
        return Err(invalid("native publication count contradicts records"));
    }
    let verified = optional_count(outcome, "verified_models")?;
    let checked = optional_count(outcome, "checked")?;
    match (verified, checked) {
        (Some(verified), Some(checked)) if published <= verified && verified <= checked => {}
        (None, None) if published == 0 && completion.is_null() => {}
        _ => return Err(invalid("native partial progress counts disagree")),
    }
    Ok(())
}

fn optional_count(outcome: &Value, key: &str) -> Result<Option<u64>, Failure> {
    let value = outcome
        .get(key)
        .ok_or_else(|| invalid("missing native progress count"))?;
    if value.is_null() {
        Ok(None)
    } else {
        value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid("malformed native progress count"))
    }
}
fn invalid(message: &str) -> Failure {
    (Decision::InvalidReport, message.into())
}
