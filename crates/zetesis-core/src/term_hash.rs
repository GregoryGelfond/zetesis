//! Semantic hash operations shared by owned descriptions and canonical views.
//!
//! Root classes, node tags and sequence lengths delimit the typed description.
//! Cached spelling, allocation identity and physical sharing do not participate.
//! This is a Rust `Hash` contract, not a serialized or stable fingerprint format.

use std::hash::{Hash, Hasher};

use crate::{Value, ValueNode, ValueNodeRef};

pub(crate) fn value<H: Hasher>(value: &Value, state: &mut H) {
    root(value.root_view(), state);
    if let Value::Structured(value) = value {
        nodes(value.nodes(), state);
    } else {
        descriptor(value.root_view(), state);
    }
}

pub(crate) fn root<H: Hasher>(node: ValueNodeRef<'_>, state: &mut H) {
    crate::term_order::root_storage_rank(node).hash(state);
}

pub(crate) fn nodes<H: Hasher>(nodes: &[ValueNode], state: &mut H) {
    nodes.len().hash(state);
    for node in nodes {
        descriptor(node.view(), state);
    }
}

pub(crate) fn descriptor<H: Hasher>(node: ValueNodeRef<'_>, state: &mut H) {
    crate::term_order::node_storage_rank(node).hash(state);
    match node {
        ValueNodeRef::Number(number) => number.hash(state),
        ValueNodeRef::String(text) | ValueNodeRef::Symbol(text) => text.hash(state),
        ValueNodeRef::Function { name, sign, arity } => {
            name.hash(state);
            sign.hash(state);
            arity.hash(state);
        }
        ValueNodeRef::Tuple { arity } => arity.hash(state),
        ValueNodeRef::Infimum | ValueNodeRef::Supremum => {}
    }
}
