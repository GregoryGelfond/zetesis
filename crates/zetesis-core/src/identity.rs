//! Checked canonical identity comparisons, independent of ASP term order.

use std::cmp::Ordering;

use crate::{Atom, Predicate, Value, ValueNode};

pub(crate) fn bytes<E>(
    left: &[u8],
    right: &[u8],
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    for (a, b) in left.iter().zip(right) {
        before()?;
        let order = a.cmp(b);
        if !order.is_eq() {
            return Ok(order);
        }
    }
    before()?;
    Ok(left.len().cmp(&right.len()))
}

pub(crate) fn predicate<E>(
    left: &Predicate,
    right: &Predicate,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    before()?;
    let name = if left.shares_name(right) {
        Ordering::Equal
    } else {
        bytes(left.name().as_bytes(), right.name().as_bytes(), before)?
    };
    Ok(name
        .then_with(|| left.arity().cmp(&right.arity()))
        .then_with(|| left.sign().cmp(&right.sign())))
}

pub(crate) fn atom<E>(
    left: &Atom,
    right: &Atom,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    let order = predicate(left.predicate(), right.predicate(), before)?;
    if !order.is_eq() {
        return Ok(order);
    }
    values(left, right, before)
}

/// Compare the arguments of two atoms of one predicate, in column order.
pub(crate) fn values<E>(
    left: &Atom,
    right: &Atom,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    // Both atoms were arity-checked, so equal predicates give equal tuple widths.
    for (left, right) in left.values().iter().zip(right.values()) {
        let order = left.compare_identity_with(right, &mut *before)?;
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

impl Value {
    /// Compare canonical typed identities, with a fallible work boundary.
    ///
    /// The result agrees with [`Ord`], including string versus symbol, signed
    /// constructors and whole tuples. It is not [`Self::compare_terms`]. This
    /// borrows both values, allocates nothing and traverses structural nodes
    /// iteratively. Equal validated node sequences have equal derived depth and
    /// spelling, so those cached derivatives need no second comparison.
    ///
    /// Calls `before` before each value/structural-node descriptor comparison,
    /// each compared text-byte pair and each terminating sequence-length test.
    /// A descriptor includes its constant-size numeric/sign/arity fields. Only
    /// the visited prefix is charged; no opaque payload comparison follows it.
    /// The caller can charge work and poll its own control at this boundary.
    ///
    /// # Errors
    /// Returns the first callback error before performing that comparison;
    /// neither input changes and no partial ordering is returned.
    pub fn compare_identity_with<E>(
        &self,
        other: &Self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Ordering, E> {
        before()?;
        let order = value_rank(self).cmp(&value_rank(other));
        if !order.is_eq() {
            return Ok(order);
        }
        match (self, other) {
            (Self::Number(left), Self::Number(right)) => Ok(left.cmp(right)),
            (Self::String(left), Self::String(right))
            | (Self::Symbol(left), Self::Symbol(right)) => {
                bytes(left.as_bytes(), right.as_bytes(), &mut before)
            }
            (Self::Structured(left), Self::Structured(right)) => {
                for (left, right) in left.nodes().iter().zip(right.nodes()) {
                    let order = node(left, right, &mut before)?;
                    if !order.is_eq() {
                        return Ok(order);
                    }
                }
                before()?;
                Ok(left.nodes().len().cmp(&right.nodes().len()))
            }
            _ => Ok(Ordering::Equal), // Equal ranks leave only the two extrema.
        }
    }
}

fn value_rank(value: &Value) -> u8 {
    match value {
        Value::Infimum => 0,
        Value::Number(_) => 1,
        Value::String(_) => 2,
        Value::Symbol(_) => 3,
        Value::Structured(_) => 4,
        Value::Supremum => 5,
    }
}

fn node_rank(node: &ValueNode) -> u8 {
    match node {
        ValueNode::Infimum => 0,
        ValueNode::Number(_) => 1,
        ValueNode::String(_) => 2,
        ValueNode::Symbol(_) => 3,
        ValueNode::Function { .. } => 4,
        ValueNode::Tuple { .. } => 5,
        ValueNode::Supremum => 6,
    }
}

fn node<E>(
    left: &ValueNode,
    right: &ValueNode,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    before()?;
    let order = node_rank(left).cmp(&node_rank(right));
    if !order.is_eq() {
        return Ok(order);
    }
    match (left, right) {
        (ValueNode::Number(a), ValueNode::Number(b)) => Ok(a.cmp(b)),
        (ValueNode::String(a), ValueNode::String(b))
        | (ValueNode::Symbol(a), ValueNode::Symbol(b)) => bytes(a.as_bytes(), b.as_bytes(), before),
        (
            ValueNode::Function {
                name: a,
                sign: sa,
                arity: aa,
            },
            ValueNode::Function {
                name: b,
                sign: sb,
                arity: ab,
            },
        ) => Ok(bytes(a.as_bytes(), b.as_bytes(), before)?
            .then_with(|| sa.cmp(sb))
            .then_with(|| aa.cmp(ab))),
        (ValueNode::Tuple { arity: a }, ValueNode::Tuple { arity: b }) => Ok(a.cmp(b)),
        _ => Ok(Ordering::Equal),
    }
}
