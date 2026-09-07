//! Flat native JSON value decoding through the shared core constructor.
use serde_json::Value as Json;
use zetesis_core::{Sign, Value, ValueNode};

use super::Limits;
use crate::answers::{Error, Issue, Resource, check, invalid};

pub(super) fn array<'a>(value: &'a Json, field: &'static str) -> Result<&'a [Json], Error> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(Issue::MalformedField, field))
}
pub(super) fn string<'a>(value: &'a Json, field: &'static str) -> Result<&'a str, Error> {
    value
        .as_str()
        .ok_or_else(|| invalid(Issue::MalformedField, field))
}
pub(super) fn unsigned(value: &Json, field: &'static str) -> Result<u64, Error> {
    value
        .as_u64()
        .ok_or_else(|| invalid(Issue::MalformedField, field))
}
pub(super) fn index(value: &Json, field: &'static str) -> Result<usize, Error> {
    usize::try_from(unsigned(value, field)?).map_err(|_| invalid(Issue::CountOverflow, field))
}
pub(super) fn signed(value: &Json, field: &'static str) -> Result<i32, Error> {
    value
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .ok_or_else(|| invalid(Issue::MalformedField, field))
}
pub(super) fn sign(value: &Json) -> Result<Sign, Error> {
    match value.as_str() {
        Some("positive") => Ok(Sign::Positive),
        Some("negative") => Ok(Sign::Negative),
        _ => Err(invalid(Issue::MalformedField, "native sign")),
    }
}

pub(super) fn value(value: &Json, limits: Limits, nodes: &mut usize) -> Result<Value, Error> {
    let raw = array(value, "native value nodes")?;
    *nodes = nodes
        .checked_add(raw.len())
        .ok_or_else(|| invalid(Issue::CountOverflow, "native value node count"))?;
    check(Resource::ValueNodes, limits.max_value_nodes, *nodes)?;
    // Check the per-value node ceiling before allocating its owned flat sequence.
    if raw.len() > limits.value.max_nodes {
        return Err(Error::Value(zetesis_core::ValueError::Limit {
            resource: zetesis_core::ValueResource::Nodes,
            observed: raw.len() as u128,
            limit: limits.value.max_nodes,
        }));
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(raw.len())
        .map_err(|_| Error::Allocation)?;
    for node in raw {
        decoded.push(match string(&node["kind"], "native node kind")? {
            "infimum" => ValueNode::Infimum,
            "supremum" => ValueNode::Supremum,
            "number" => ValueNode::Number(signed(&node["value"], "native i32 value")?),
            "string" => ValueNode::String(string(&node["value"], "native string")?.to_owned()),
            "symbol" => ValueNode::Symbol(string(&node["value"], "native symbol")?.to_owned()),
            "function" => ValueNode::Function {
                name: string(&node["name"], "native constructor name")?.to_owned(),
                sign: sign(&node["sign"])?,
                arity: index(&node["arity"], "native constructor arity")?,
            },
            "tuple" => ValueNode::Tuple {
                arity: index(&node["arity"], "native tuple arity")?,
            },
            _ => {
                return Err(invalid(
                    Issue::MalformedField,
                    "unsupported native node kind",
                ));
            }
        });
    }
    Value::from_nodes(decoded, limits.value).map_err(Error::Value)
}

pub(super) fn costs(value: &Json, limits: Limits) -> Result<Vec<(i32, i64)>, Error> {
    let raw = array(value, "native cost vector")?;
    check(
        Resource::CostDimensions,
        limits.report.max_cost_dimensions,
        raw.len(),
    )?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(raw.len())
        .map_err(|_| Error::Allocation)?;
    for cost in raw {
        let priority = signed(&cost["priority"], "native cost priority")?;
        let cost = cost["value"]
            .as_i64()
            .ok_or_else(|| invalid(Issue::MalformedField, "native i64 cost"))?;
        if result
            .last()
            .is_some_and(|(previous, _)| *previous <= priority)
        {
            return Err(invalid(
                Issue::Contradiction,
                "native priorities must be distinct and descending",
            ));
        }
        result.push((priority, cost));
    }
    Ok(result)
}
