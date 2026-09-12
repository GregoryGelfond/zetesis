//! One bounded bridge between upstream closed symbols and core flat values.

use themelios_program::symbol::{Name, Sign, Symbol};
use zetesis_core::{Value, ValueError, ValueLimits, ValueNode, ValueNodeRef, ValueResource};

pub(crate) fn from_symbol(symbol: &Symbol) -> Result<Value, ValueError> {
    traverse(symbol, ValueLimits::default(), Construction::default())
}

/// Check the logical value without constructing nodes or rendering output.
/// Actual node capacity and construction/render scratch remain the construction
/// sink's responsibility. Both sinks use this same typed Symbol walk.
pub(crate) fn validate_symbol(symbol: &Symbol) -> Result<(), ValueError> {
    traverse(symbol, ValueLimits::default(), Validation::default())
}

trait Sink {
    type Output;
    fn node(&mut self, node: ValueNodeRef<'_>) -> Result<(), ValueError>;
    fn finish(self, bytes: u128, limits: ValueLimits) -> Result<Self::Output, ValueError>;
}

#[derive(Default)]
struct Construction {
    nodes: Vec<ValueNode>,
}

impl Sink for Construction {
    type Output = Value;

    fn node(&mut self, node: ValueNodeRef<'_>) -> Result<(), ValueError> {
        self.nodes
            .try_reserve_exact(1)
            .map_err(|_| ValueError::Allocation)?;
        self.nodes.push(node.into_owned());
        Ok(())
    }

    fn finish(self, _bytes: u128, limits: ValueLimits) -> Result<Value, ValueError> {
        // Keep actual capacity, validation frames, rendering and final ownership
        // checks at the existing independent core constructor.
        Value::from_nodes(self.nodes, limits)
    }
}

#[derive(Default)]
struct Validation {
    spelling_bytes: u128,
}

impl Sink for Validation {
    type Output = ();

    fn node(&mut self, node: ValueNodeRef<'_>) -> Result<(), ValueError> {
        self.spelling_bytes += node.rendered_bytes();
        Ok(())
    }

    fn finish(self, bytes: u128, limits: ValueLimits) -> Result<(), ValueError> {
        check(
            ValueResource::Bytes,
            bytes + self.spelling_bytes,
            limits.max_bytes,
        )
    }
}

fn check(resource: ValueResource, observed: u128, limit: usize) -> Result<(), ValueError> {
    if observed > limit as u128 {
        Err(ValueError::Limit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

/// `pending` contains the untouched subtrees in reverse visit order; `nodes`
/// counts the completed preorder prefix. Each typed Symbol is visited once,
/// independently of the sink. Work is O(nodes + text), with O(frontier) borrowed
/// traversal storage in addition to the chosen sink's owned output.
fn traverse<S: Sink>(
    symbol: &Symbol,
    limits: ValueLimits,
    mut sink: S,
) -> Result<S::Output, ValueError> {
    let mut pending = vec![(symbol, 1usize)];
    let mut nodes = 0_u128;
    let mut bytes = 0_u128;
    while let Some((symbol, depth)) = pending.pop() {
        for (resource, observed, limit) in [
            (ValueResource::Nodes, nodes + 1, limits.max_nodes),
            (ValueResource::Depth, depth as u128, limits.max_depth),
        ] {
            check(resource, observed, limit)?;
        }
        let node = view(symbol);
        bytes += std::mem::size_of::<ValueNode>() as u128 + node.text_bytes() as u128;
        check(ValueResource::Bytes, bytes, limits.max_bytes)?;
        let children = symbol.arguments();
        check(
            ValueResource::Nodes,
            nodes + pending.len() as u128 + children.len() as u128 + 1,
            limits.max_nodes,
        )?;
        pending
            .try_reserve(children.len())
            .map_err(|_| ValueError::Allocation)?;
        pending.extend(children.iter().rev().map(|child| (child, depth + 1)));
        sink.node(node)?;
        nodes += 1;
    }
    sink.finish(bytes, limits)
}

fn view(symbol: &Symbol) -> ValueNodeRef<'_> {
    match symbol {
        Symbol::Infimum => ValueNodeRef::Infimum,
        Symbol::Supremum => ValueNodeRef::Supremum,
        Symbol::Number(number) => ValueNodeRef::Number(*number),
        Symbol::String(text) => ValueNodeRef::String(text),
        Symbol::Function {
            name,
            arguments,
            sign,
        } => ValueNodeRef::Function {
            name: name.as_str(),
            arity: arguments.len(),
            sign: crate::coherence::core_sign(*sign),
        },
        Symbol::Tuple(arguments) => ValueNodeRef::Tuple {
            arity: arguments.len(),
        },
    }
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
            .map(|node| std::mem::size_of::<ValueNode>() as u128 + view(node).text_bytes() as u128)
            .sum()
    } else {
        view(symbol).text_bytes() as u128
    }
}

#[cfg(test)]
#[path = "structural_value_tests.rs"]
mod tests;
