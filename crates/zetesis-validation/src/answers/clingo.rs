//! Clingo JSON witness and optN replay reconciliation.
use super::{
    Error, Issue, Limits, Model, ReportedAnswers, Resource, check, invalid, multiplicities,
};
use serde_json::Value;

pub(super) fn parse(text: &str, limits: Limits) -> Result<ReportedAnswers, Error> {
    let document: Value = serde_json::from_str(text).map_err(Error::Json)?;
    let result = document["Result"]
        .as_str()
        .ok_or_else(|| invalid(Issue::MissingField, "missing clingo Result"))?;
    if !matches!(result, "SATISFIABLE" | "UNSATISFIABLE" | "OPTIMUM FOUND")
        || document["Models"]["More"] != "no"
    {
        return Err(invalid(
            Issue::Incomplete,
            "reference did not complete enumeration/optimization",
        ));
    }
    let optimum = result == "OPTIMUM FOUND";
    if optimum && document["Models"]["Optimum"] != "yes" {
        return Err(invalid(
            Issue::Incomplete,
            "reference lacks completed optimality evidence",
        ));
    }
    let cost = if optimum {
        Some(costs(&document["Models"]["Costs"], limits)?)
    } else {
        None
    };
    if !optimum && document["Models"].get("Costs").is_some() {
        return Err(invalid(
            Issue::Contradiction,
            "nonoptimized reference result contains objective costs",
        ));
    }
    let Witnesses {
        selected,
        raw_count,
        best_count,
    } = witnesses(&document, cost.as_deref(), limits)?;
    let satisfiable = result != "UNSATISFIABLE";
    let count_key = if optimum { "Optimal" } else { "Number" };
    let model_count = document["Models"][count_key]
        .as_u64()
        .ok_or_else(|| invalid(Issue::MissingField, "missing completed model count"))?;
    if document["Models"]["Number"].as_u64() != Some(raw_count) {
        return Err(invalid(
            Issue::Contradiction,
            "reference witness count disagrees with Models.Number",
        ));
    }
    if optimum && model_count.checked_add(1) != Some(best_count) {
        return Err(invalid(
            Issue::Contradiction,
            "reference optN requires final optimal witnesses plus one incumbent replay",
        ));
    }
    if optimum {
        let (incumbent, optimal) = selected
            .split_first()
            .ok_or_else(|| invalid(Issue::MissingField, "missing final incumbent"))?;
        if !optimal.contains(incumbent) {
            return Err(invalid(
                Issue::Contradiction,
                "final incumbent is absent from optimal enumeration",
            ));
        }
    }
    // optN first discovers the final incumbent, then enumerates every optimal
    // model. Remove precisely that first discovery, never every equal display.
    let model_multiplicities = multiplicities(selected.into_iter().skip(usize::from(optimum)))?;
    if satisfiable == model_multiplicities.is_empty()
        || (satisfiable && model_count == 0)
        || (!satisfiable && model_count != 0)
    {
        return Err(invalid(
            Issue::Contradiction,
            "inconsistent clingo status, witnesses, or counts",
        ));
    }
    Ok(ReportedAnswers {
        satisfiable,
        cost,
        model_multiplicities,
        model_count,
        solver: document["Solver"]
            .as_str()
            .unwrap_or("unreported")
            .to_owned(),
    })
}

fn witness_symbols(value: &Value) -> Result<Model, Error> {
    let mut symbols = value
        .as_array()
        .ok_or_else(|| invalid(Issue::MissingField, "missing witness atoms"))?
        .iter()
        .map(|atom| {
            atom.as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid(Issue::MalformedField, "non-string witness atom"))
        })
        .collect::<Result<Model, _>>()?;
    symbols.sort_unstable();
    Ok(symbols)
}

fn costs(value: &Value, limits: Limits) -> Result<Vec<i64>, Error> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid(Issue::MalformedField, "missing cost vector"))?;
    check(
        Resource::CostDimensions,
        limits.max_cost_dimensions,
        values.len(),
    )?;
    values
        .iter()
        .map(|cost| {
            cost.as_i64().ok_or_else(|| {
                invalid(
                    Issue::MalformedField,
                    "cost outside signed 64-bit report representation",
                )
            })
        })
        .collect()
}

struct Witnesses {
    selected: Vec<Model>,
    raw_count: u64,
    best_count: u64,
}

fn witnesses(document: &Value, cost: Option<&[i64]>, limits: Limits) -> Result<Witnesses, Error> {
    let mut selected = Vec::new();
    let mut raw_count = 0u64;
    let mut best_count = 0u64;
    let mut symbol_count = 0usize;
    for call in document["Call"]
        .as_array()
        .ok_or_else(|| invalid(Issue::MissingField, "missing clingo calls"))?
    {
        let call = call
            .as_object()
            .ok_or_else(|| invalid(Issue::MalformedField, "invalid clingo call"))?;
        let Some(witnesses) = call.get("Witnesses") else {
            continue;
        };
        let witnesses = witnesses
            .as_array()
            .ok_or_else(|| invalid(Issue::MalformedField, "invalid clingo witnesses"))?;
        for witness in witnesses {
            raw_count = raw_count
                .checked_add(1)
                .ok_or_else(|| invalid(Issue::CountOverflow, "witness count overflow"))?;
            check(
                Resource::Witnesses,
                limits.max_witnesses,
                usize::try_from(raw_count).unwrap_or(usize::MAX),
            )?;
            let symbols = witness["Value"]
                .as_array()
                .ok_or_else(|| invalid(Issue::MalformedField, "missing witness atoms"))?;
            symbol_count = symbol_count
                .checked_add(symbols.len())
                .ok_or_else(|| invalid(Issue::CountOverflow, "symbol count overflow"))?;
            check(Resource::Symbols, limits.max_symbols, symbol_count)?;
            let model = witness_symbols(&witness["Value"])?;
            if let Some(best) = cost {
                let actual = costs(&witness["Costs"], limits)?;
                if actual.len() != best.len() || actual.as_slice() < best {
                    return Err(invalid(
                        Issue::Contradiction,
                        "witness contradicts the reported final cost vector",
                    ));
                }
                if actual.as_slice() != best {
                    if best_count != 0 {
                        return Err(invalid(
                            Issue::Contradiction,
                            "nonoptimal witness follows the final incumbent",
                        ));
                    }
                    continue;
                }
            } else if witness.get("Costs").is_some() {
                return Err(invalid(
                    Issue::Contradiction,
                    "unexpected objective vector in nonoptimized witness",
                ));
            }
            best_count = best_count
                .checked_add(1)
                .ok_or_else(|| invalid(Issue::CountOverflow, "witness count overflow"))?;
            selected.push(model);
        }
    }
    Ok(Witnesses {
        selected,
        raw_count,
        best_count,
    })
}
