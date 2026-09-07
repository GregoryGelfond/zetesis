//! One bounded bridge between upstream closed symbols and core flat values.

use themelios_program::symbol::{Name, Sign, Symbol};
use zetesis_core::{Value, ValueError, ValueLimits, ValueNode, ValueResource};

pub(crate) fn from_symbol(symbol: &Symbol) -> Result<Value, ValueError> {
    let limits = ValueLimits::default();
    let mut pending = vec![(symbol, 1usize)];
    let mut nodes = Vec::new();
    let mut bytes = 0_u128;
    while let Some((symbol, depth)) = pending.pop() {
        for (resource, observed, limit) in [
            (
                ValueResource::Nodes,
                nodes.len() as u128 + 1,
                limits.max_nodes,
            ),
            (ValueResource::Depth, depth as u128, limits.max_depth),
        ] {
            if observed > limit as u128 {
                return Err(ValueError::Limit {
                    resource,
                    observed,
                    limit,
                });
            }
        }
        let text = match symbol {
            Symbol::String(text) => text.len(),
            Symbol::Function { name, .. } => name.as_str().len(),
            _ => 0,
        };
        bytes += std::mem::size_of::<ValueNode>() as u128 + text as u128;
        if bytes > limits.max_bytes as u128 {
            return Err(ValueError::Limit {
                resource: ValueResource::Bytes,
                observed: bytes,
                limit: limits.max_bytes,
            });
        }
        let children = symbol.arguments();
        if nodes.len() as u128 + pending.len() as u128 + children.len() as u128 + 1
            > limits.max_nodes as u128
        {
            return Err(ValueError::Limit {
                resource: ValueResource::Nodes,
                observed: nodes.len() as u128 + pending.len() as u128 + children.len() as u128 + 1,
                limit: limits.max_nodes,
            });
        }
        pending
            .try_reserve(children.len())
            .map_err(|_| ValueError::Allocation)?;
        pending.extend(children.iter().rev().map(|child| (child, depth + 1)));
        nodes
            .try_reserve_exact(1)
            .map_err(|_| ValueError::Allocation)?;
        nodes.push(match symbol {
            Symbol::Infimum => ValueNode::Infimum,
            Symbol::Supremum => ValueNode::Supremum,
            Symbol::Number(n) => ValueNode::Number(*n),
            Symbol::String(s) => ValueNode::String(s.clone()),
            Symbol::Function {
                name,
                arguments,
                sign,
            } => ValueNode::Function {
                name: name.as_str().into(),
                arity: arguments.len(),
                sign: crate::coherence::core_sign(*sign),
            },
            Symbol::Tuple(arguments) => ValueNode::Tuple {
                arity: arguments.len(),
            },
        });
    }
    Value::from_nodes(nodes, limits)
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BridgeError {
    Allocation,
    InvalidName,
}

/// Reverse postorder reconstruction. The upstream Symbol also drops iteratively.
pub(crate) fn to_symbol(value: &zetesis_core::StructuralValue) -> Result<Symbol, BridgeError> {
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(value.nodes().len())
        .map_err(|_| BridgeError::Allocation)?;
    for node in value.nodes().iter().rev() {
        let symbol = match node {
            ValueNode::Infimum => Symbol::Infimum,
            ValueNode::Supremum => Symbol::Supremum,
            ValueNode::Number(n) => Symbol::Number(*n),
            ValueNode::String(s) => Symbol::String(s.clone()),
            ValueNode::Symbol(name) => Symbol::Function {
                name: Name::new(name.clone()).map_err(|_| BridgeError::InvalidName)?,
                arguments: Vec::new(),
                sign: Sign::Positive,
            },
            ValueNode::Function { name, sign, arity } => Symbol::Function {
                name: Name::new(name.clone()).map_err(|_| BridgeError::InvalidName)?,
                arguments: children(&mut stack, *arity)?,
                sign: match sign {
                    zetesis_core::Sign::Positive => Sign::Positive,
                    zetesis_core::Sign::Negative => Sign::Negative,
                },
            },
            ValueNode::Tuple { arity } => Symbol::Tuple(children(&mut stack, *arity)?),
        };
        stack.push(symbol);
    }
    stack.pop().ok_or(BridgeError::InvalidName)
}
fn children(stack: &mut Vec<Symbol>, arity: usize) -> Result<Vec<Symbol>, BridgeError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(arity)
        .map_err(|_| BridgeError::Allocation)?;
    for _ in 0..arity {
        result.push(stack.pop().ok_or(BridgeError::InvalidName)?);
    }
    Ok(result)
}

/// Conservative node/text payload before cloning an upstream structural symbol.
pub(crate) fn symbol_bytes(symbol: &Symbol) -> u128 {
    if matches!(symbol, Symbol::Tuple(_))
        || matches!(symbol, Symbol::Function { arguments, sign, .. } if !arguments.is_empty() || *sign == Sign::Negative)
    {
        symbol
            .subsymbols()
            .map(|node| {
                std::mem::size_of::<ValueNode>() as u128
                    + match node {
                        Symbol::String(text) => text.len() as u128,
                        Symbol::Function { name, .. } => name.as_str().len() as u128,
                        _ => 0,
                    }
            })
            .sum()
    } else {
        match symbol {
            Symbol::String(text) => text.len() as u128,
            Symbol::Function { name, .. } => name.as_str().len() as u128,
            _ => 0,
        }
    }
}
