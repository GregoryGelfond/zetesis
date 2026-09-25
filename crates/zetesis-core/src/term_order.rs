//! Typed storage and ASP descriptor orders over borrowed term descriptions.
//!
//! Heads describe one node, not an admitted tree. Callers traverse complete
//! validated preorder sequences iteratively and stop at the first difference.
//! Text comparison is supplied separately so ordinary order retains borrowed
//! string comparison while checked identity meters every visited byte.

use std::{cmp::Ordering, convert::Infallible};

use crate::{Sign, Value, ValueNodeRef};

/// A whole value has one structural class; nested nodes distinguish functions
/// from tuples. These ranks are storage order, never ASP order.
pub(crate) fn value_storage_rank(value: &Value) -> u8 {
    root_storage_rank(value.root_view())
}

pub(crate) fn root_storage_rank(node: ValueNodeRef<'_>) -> u8 {
    match node {
        ValueNodeRef::Infimum => 0,
        ValueNodeRef::Number(_) => 1,
        ValueNodeRef::String(_) => 2,
        ValueNodeRef::Symbol(_) => 3,
        ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. } => 4,
        ValueNodeRef::Supremum => 5,
    }
}

pub(crate) fn node_storage_rank(node: ValueNodeRef<'_>) -> u8 {
    match node {
        ValueNodeRef::Infimum => 0,
        ValueNodeRef::Number(_) => 1,
        ValueNodeRef::String(_) => 2,
        ValueNodeRef::Symbol(_) => 3,
        ValueNodeRef::Function { .. } => 4,
        ValueNodeRef::Tuple { .. } => 5,
        ValueNodeRef::Supremum => 6,
    }
}

pub(crate) fn storage(left: ValueNodeRef<'_>, right: ValueNodeRef<'_>) -> Ordering {
    let result = storage_with(left, right, |left, right| {
        Ok::<_, Infallible>(left.cmp(right))
    });
    match result {
        Ok(order) => order,
        Err(never) => match never {},
    }
}

/// Compare one storage descriptor. The caller supplies the descriptor work
/// boundary; `text` is called at most once, only after equal descriptor ranks.
pub(crate) fn storage_with<E>(
    left: ValueNodeRef<'_>,
    right: ValueNodeRef<'_>,
    text: impl FnOnce(&str, &str) -> Result<Ordering, E>,
) -> Result<Ordering, E> {
    let order = node_storage_rank(left).cmp(&node_storage_rank(right));
    if !order.is_eq() {
        return Ok(order);
    }
    match (left, right) {
        (ValueNodeRef::Number(left), ValueNodeRef::Number(right)) => Ok(left.cmp(&right)),
        (ValueNodeRef::String(left), ValueNodeRef::String(right))
        | (ValueNodeRef::Symbol(left), ValueNodeRef::Symbol(right)) => text(left, right),
        (
            ValueNodeRef::Function {
                name: left,
                sign: left_sign,
                arity: left_arity,
            },
            ValueNodeRef::Function {
                name: right,
                sign: right_sign,
                arity: right_arity,
            },
        ) => Ok(text(left, right)?
            .then_with(|| left_sign.cmp(&right_sign))
            .then_with(|| left_arity.cmp(&right_arity))),
        (ValueNodeRef::Tuple { arity: left }, ValueNodeRef::Tuple { arity: right }) => {
            Ok(left.cmp(&right))
        }
        _ => Ok(Ordering::Equal), // Equal ranks leave only the two extrema.
    }
}

fn asp_rank(node: ValueNodeRef<'_>) -> u8 {
    match node {
        ValueNodeRef::Infimum => 0,
        ValueNodeRef::Number(_) => 1,
        ValueNodeRef::Symbol(_) | ValueNodeRef::Tuple { arity: 0 } => 2,
        ValueNodeRef::Function { arity: 0, .. } => 3,
        ValueNodeRef::String(_) => 4,
        ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. } => 5,
        ValueNodeRef::Supremum => 6,
    }
}

fn asp_head(node: ValueNodeRef<'_>) -> (Sign, usize, Option<&str>) {
    match node {
        ValueNodeRef::Function { name, sign, arity } => (sign, arity, Some(name)),
        ValueNodeRef::Tuple { arity } => (Sign::Positive, arity, None),
        ValueNodeRef::Symbol(name) => (Sign::Positive, 0, Some(name)),
        _ => (Sign::Positive, 0, None),
    }
}

/// ASP head order assumes positive nullary functions were normalized at value
/// admission. It compares sign, arity and name, unlike function storage order.
pub(crate) fn asp(left: ValueNodeRef<'_>, right: ValueNodeRef<'_>) -> Ordering {
    match asp_with(left, right, |left, right| {
        Ok::<_, Infallible>(left.cmp(right))
    }) {
        Ok(order) => order,
        Err(never) => match never {},
    }
}

/// The same head order with an explicit text-comparison boundary.
pub(crate) fn asp_with<E>(
    left: ValueNodeRef<'_>,
    right: ValueNodeRef<'_>,
    text: impl FnOnce(&str, &str) -> Result<Ordering, E>,
) -> Result<Ordering, E> {
    let rank = asp_rank(left).cmp(&asp_rank(right));
    if !rank.is_eq() {
        return Ok(rank);
    }
    match (left, right) {
        (ValueNodeRef::Number(left), ValueNodeRef::Number(right)) => Ok(left.cmp(&right)),
        (ValueNodeRef::String(left), ValueNodeRef::String(right)) => text(left, right),
        _ => {
            let (left_sign, left_arity, left_name) = asp_head(left);
            let (right_sign, right_arity, right_name) = asp_head(right);
            let order = left_sign
                .cmp(&right_sign)
                .then_with(|| left_arity.cmp(&right_arity));
            if !order.is_eq() {
                return Ok(order);
            }
            match (left_name, right_name) {
                (Some(left), Some(right)) => text(left, right),
                _ => Ok(left_name.is_some().cmp(&right_name.is_some())),
            }
        }
    }
}
