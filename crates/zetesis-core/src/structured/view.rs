//! Borrowed descriptions share the owned node's spelling and payload measures.

use crate::{Sign, ValueNode};

/// One borrowed preorder node, retaining its complete typed description.
///
/// Text borrows its source for `'a`; this view owns no node or text allocation.
/// A constructor's arity describes its following subtrees, which are not owned
/// or checked by this view. As with [`ValueNode`], a node description alone does
/// not certify a complete value. Positive nullary functions remain distinct
/// from Symbol descriptions until [`crate::Value::from_nodes`] canonicalizes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueNodeRef<'a> {
    /// Least ASP value.
    Infimum,
    /// Checked-width integer leaf.
    Number(i32),
    /// Decoded string leaf.
    String(&'a str),
    /// Positive named constant leaf.
    Symbol(&'a str),
    /// Named constructor with ordered immediate children.
    Function {
        /// Exact constructor name.
        name: &'a str,
        /// Constructor sign, independent of the outer predicate.
        sign: Sign,
        /// Number of following immediate subtrees.
        arity: usize,
    },
    /// Anonymous constructor, including `()` and `(x,)`.
    Tuple {
        /// Number of following immediate subtrees.
        arity: usize,
    },
    /// Greatest ASP value.
    Supremum,
}

impl ValueNodeRef<'_> {
    /// Own this node, copying its text once. Time and additional space are O(text
    /// bytes); no child traversal or value admission occurs. Apply construction
    /// limits before using this operation to build an owned tree.
    #[must_use]
    pub fn into_owned(self) -> ValueNode {
        match self {
            Self::Infimum => ValueNode::Infimum,
            Self::Number(number) => ValueNode::Number(number),
            Self::String(text) => ValueNode::String(text.into()),
            Self::Symbol(name) => ValueNode::Symbol(name.into()),
            Self::Function { name, sign, arity } => ValueNode::Function {
                name: name.into(),
                sign,
                arity,
            },
            Self::Tuple { arity } => ValueNode::Tuple { arity },
            Self::Supremum => ValueNode::Supremum,
        }
    }

    /// UTF-8 bytes referenced by this node, excluding child text, the node cell
    /// and allocator capacity. Constant time; no allocation occurs.
    #[must_use]
    pub fn text_bytes(self) -> usize {
        match self {
            Self::String(text) | Self::Symbol(text) | Self::Function { name: text, .. } => {
                text.len()
            }
            _ => 0,
        }
    }

    /// This node's contribution to the canonical ASP spelling, including its
    /// punctuation and string escapes but excluding child contributions. A sum
    /// over a valid preorder tree is its complete spelling length. String nodes
    /// scan their UTF-8 bytes; other nodes take constant work. No rendering or
    /// allocation occurs, and malformed constructor shape is not validated here.
    #[must_use]
    pub fn rendered_bytes(self) -> u128 {
        match self {
            Self::Infimum | Self::Supremum => 4,
            Self::Number(number) => {
                u128::from(number.unsigned_abs().checked_ilog10().unwrap_or(0) + 1)
                    + u128::from(number < 0)
            }
            Self::String(text) => {
                2 + text.len() as u128
                    + text
                        .bytes()
                        .filter(|byte| matches!(byte, b'"' | b'\\' | b'\n'))
                        .count() as u128
            }
            Self::Symbol(name) => name.len() as u128,
            Self::Function { name, sign, arity } => {
                name.len() as u128
                    + u128::from(sign == Sign::Negative)
                    + if arity == 0 { 0 } else { arity as u128 + 1 }
            }
            Self::Tuple { arity } => {
                if arity == 0 {
                    2
                } else {
                    arity as u128 + 1 + u128::from(arity == 1)
                }
            }
        }
    }
}
