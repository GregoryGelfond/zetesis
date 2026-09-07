//! Pure comparison against immutable complete-model and helper-view contracts.
use std::collections::BTreeMap;

use super::{Decision, InvocationRecord};
use crate::{answers, curated, selected::Limits};

pub(super) fn case(
    case: &curated::Case,
    reference: &InvocationRecord,
    native: Option<&InvocationRecord>,
    limits: Limits,
) -> (Decision, Option<String>) {
    let Some(native) = native else {
        return failed(
            Decision::InvocationFailure,
            "native launch withheld after unresolved child cleanup",
        );
    };
    if !reference.complete(true) || !native.complete(false) {
        return failed(
            Decision::InvocationFailure,
            "both captures and accepted solver exits are required",
        );
    }
    match models(case, reference, native, limits) {
        Ok(true) => (Decision::Pass, None),
        Ok(false) => failed(
            Decision::ModelMismatch,
            "complete full-model occurrences or original helper view differ",
        ),
        Err(error) => failed(Decision::InvalidReport, &error.to_string()),
    }
}

fn failed(decision: Decision, detail: &str) -> (Decision, Option<String>) {
    (decision, Some(detail.into()))
}

fn models(
    case: &curated::Case,
    reference: &InvocationRecord,
    native: &InvocationRecord,
    limits: Limits,
) -> Result<bool, answers::Error> {
    let reference = answers::clingo_json(reference.stdout(), limits.reference)?;
    let native = answers::native_json::parse(native.stdout(), limits.native)?;
    let native_models = native.full_model_symbols(limits.max_spelling_bytes)?;
    let contract = case.contract();
    let expected = occurrences(contract.full_models());
    let observed: BTreeMap<_, _> = reference.displays().iter().cloned().collect();
    Ok(reference.cost().is_none()
        && native.costs().is_none()
        && observed == expected
        && occurrences(&native_models) == expected
        && helper(contract.full_models(), contract.prefixes())
            == occurrences(contract.helper_models()))
}

fn occurrences(models: &[Vec<String>]) -> BTreeMap<Vec<String>, u64> {
    let mut result = BTreeMap::new();
    for model in models {
        let mut ordered = model.clone();
        ordered.sort();
        *result.entry(ordered).or_default() += 1;
    }
    result
}

fn helper(models: &[Vec<String>], prefixes: &[String]) -> BTreeMap<Vec<String>, u64> {
    let selected: Vec<_> = models
        .iter()
        .map(|model| {
            model
                .iter()
                .filter(|atom| {
                    prefixes.is_empty() || prefixes.iter().any(|prefix| atom.starts_with(prefix))
                })
                .cloned()
                .collect()
        })
        .collect();
    occurrences(&selected)
}
